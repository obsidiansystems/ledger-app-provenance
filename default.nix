rec {
  cosmos_hook = alamgu.pkgs.runCommand "cosmos-hook" {} ''
    mkdir -p $out/nix-support/
    cat <<EOF >$out/nix-support/setup-hook
      export COSMOS_SDK=${cosmos-sdk}
    EOF
  '';
  buf-nixpkgs = import ./dep/nixpkgs {};
  buf_hook = alamgu.pkgs.runCommand "buf-hooks" {} ''
    mkdir -p $out/nix-support/
    cat <<EOF >$out/nix-support/setup-hook
      export PROTO_INCLDUE="${alamgu.pkgs.protobuf}/include"
      export PATH=${alamgu.pkgs.protobuf}/bin:$PATH
    EOF
  '';

  alamgu = import ./dep/alamgu {
    extraAppInputs=[cosmos_hook buf_hook];
    extraNativeAppInputs=[buf-nixpkgs.buf];
  };

  cosmos-sdk = alamgu.thunkSource ./dep/cosmos-sdk;

  inherit (alamgu)
    lib
    pkgs ledgerPkgs
    crate2nix
    buildRustCrateForPkgsLedger
    buildRustCrateForPkgsWrapper
    ;

  makeApp = { rootFeatures ? [ "default" ], release ? true }: import ./Cargo.nix {
    inherit rootFeatures release;
    pkgs = ledgerPkgs;
    buildRustCrateForPkgs = pkgs: let
      fun = buildRustCrateForPkgsWrapper
        pkgs
        ((buildRustCrateForPkgsLedger pkgs).override {
          defaultCrateOverrides = pkgs.defaultCrateOverrides // {
            proto-gen = attrs: {
              PROTO_INCLUDE = "${pkgs.buildPackages.protobuf}/include";
              nativeBuildInputs = (attrs.nativeBuildInputs or []) ++ [
                pkgs.buildPackages.protobuf
              ];
            };
            provenance = attrs: let
              sdk = lib.findFirst (p: lib.hasPrefix "rust_nanos_sdk" p.name) (builtins.throw "no sdk!") attrs.dependencies;
            in {
              preHook = alamgu.gccLibsPreHook;
              preConfigure = let
                conf = pkgs.runCommand "fetch-buf" {
                  outputHashMode = "recursive";
                  outputHashAlgo = "sha256";
                  outputHash = "0c0wacvgb800acyw7n91dxll3fmibyhayi2l6ijl24sv1wykr3ni";

                  COSMOS_SDK = cosmos-sdk;
                  PROTO_INCLUDE = "${pkgs.buildPackages.protobuf}/include";
                  nativeBuildInputs = [
                    buf-nixpkgs.cacert buf-nixpkgs.buf
                    pkgs.buildPackages.protobuf
                  ];
                } ''
                   mkdir -p $out
                   HOME=$(mktemp -d)
                   buf build $COSMOS_SDK \
                     --type=cosmos.tx.v1beta1.Tx \
                     --type=cosmos.tx.v1beta1.SignDoc \
                     --type=cosmos.tx.v1beta1.SignDoc \
                     --type=cosmos.staking.v1beta1.MsgDelegate \
                     --type=cosmos.gov.v1beta1.MsgDeposit \
                     --output $out/buf_out.bin
                   mv ~/.cache $out
                '';
              in ''
                HOME=$(mktemp -d)
                cp -r --no-preserve=mode ${conf}/.cache ~/.cache
              '';
              extraRustcOpts = attrs.extraRustcOpts or [] ++ [
                "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/script.ld"
                "-C" "linker=${pkgs.stdenv.cc.targetPrefix}clang"
              ];
              COSMOS_SDK = cosmos-sdk;
              PROTO_INCLUDE = "${pkgs.buildPackages.protobuf}/include";
              nativeBuildInputs = (attrs.nativeBuildInputs or []) ++ [
                pkgs.buildPackages.protobuf buf-nixpkgs.buf
              ];
              buildInputs = (attrs.buildInputs or []) ++ [
                cosmos-sdk
              ];
            };
          };
        });
    in
      args: fun (args // lib.optionalAttrs pkgs.stdenv.hostPlatform.isAarch32 {
        dependencies = map (d: d // { stdlib = true; }) [
          alamgu.ledgerCore
          alamgu.ledgerCompilerBuiltins
        ] ++ args.dependencies;
      });
  };

  app = makeApp {};
  app-with-logging = makeApp {
    release = false;
    rootFeatures = [ "default" "speculos" "extra_debug" ];
  };

  # For CI
  rootCrate = app.rootCrate.build;
  rootCrate-with-logging = app-with-logging.rootCrate.build;

  tarSrc = ledgerPkgs.runCommandCC "tarSrc" {
    nativeBuildInputs = [
      alamgu.cargo-ledger
      alamgu.ledgerRustPlatform.rust.cargo
    ];
  } (alamgu.cargoLedgerPreHook + ''

    cp ${./rust-app/Cargo.toml} ./Cargo.toml
    # So cargo knows it's a binary
    mkdir src
    touch src/main.rs

    cargo-ledger --use-prebuilt ${rootCrate}/bin/provenance --hex-next-to-json

    mkdir -p $out/provenance
    cp app.json app.hex $out/provenance
    cp ${./tarball-default.nix} $out/provenance/default.nix
    cp ${./tarball-shell.nix} $out/provenance/shell.nix
    cp ${./rust-app/crab.gif} $out/provenance/crab.gif
  '');

  impureTarSrc = ledgerPkgs.runCommandCC "tarSrc" {
  } (''
    mkdir -p $out/rust-app
    cp ${./rust-app/target/thumbv6m-none-eabi/release/provenance} $out/rust-app/provenance
    cp ${./rust-app/app.json} $out/rust-app/app.json
    cp ${./rust-app/app.hex} $out/rust-app/app.hex
    cp ${./tarball-default.nix} $out/rust-app/default.nix
    cp ${./tarball-shell.nix} $out/rust-app/shell.nix
    cp ${./rust-app/crab.gif} $out/rust-app/crab.gif
  '');

  tarball = pkgs.runCommandNoCC "app-tarball.tar.gz" { } ''
    tar -czvhf $out -C ${tarSrc} provenance
  '';

  impureTarball = pkgs.runCommandNoCC "app-tarball.tar.gz" { } ''
    tar -czvhf $out -C ${impureTarSrc} rust-app
  '';

  loadApp = pkgs.writeScriptBin "load-app" ''
    #!/usr/bin/env bash
    cd ${tarSrc}/provenance
    ${alamgu.ledgerctl}/bin/ledgerctl install -f ${tarSrc}/provenance/app.json
  '';

  testPackage = (import ./ts-tests/override.nix { inherit pkgs; }).package;

  testScript = pkgs.writeShellScriptBin "mocha-wrapper" ''
    cd ${testPackage}/lib/node_modules/*/
    export NO_UPDATE_NOTIFIER=true
    exec ${pkgs.nodejs-14_x}/bin/npm --offline test -- "$@"
  '';

  runTests = { appExe ? rootCrate + "/bin/provenance" }: pkgs.runCommandNoCC "run-tests" {
    nativeBuildInputs = [
      pkgs.wget alamgu.speculos.speculos testScript
    ];
  } ''
    RUST_APP=${rootCrate}/bin/*
    echo RUST APP IS $RUST_APP
    # speculos -k 2.0 $RUST_APP --display headless &
    mkdir $out
    (
    speculos -k 2.0 ${appExe} --display headless &
    SPECULOS=$!

    until wget -O/dev/null -o/dev/null http://localhost:5000; do sleep 0.1; done;

    ${testScript}/bin/mocha-wrapper
    rv=$?
    kill -9 $SPECULOS
    exit $rv) | tee $out/short |& tee $out/full
    rv=$?
    cat $out/short
    exit $rv
  '';

  test-with-loging = runTests {
    appExe = rootCrate-with-logging + "/bin/provenance";
  };
  test = runTests {
    appExe = rootCrate + "/bin/provenance";
  };

  inherit (pkgs.nodePackages) node2nix;

  appShell = pkgs.mkShell {
    packages = [ loadApp alamgu.generic-cli pkgs.jq ];
  };

  provenanced = pkgs.stdenv.mkDerivation {
    name = "provenance-bin";
    src = builtins.fetchurl {
      # url = "https://github.com/provenance-io/provenance/releases/download/v1.12.0/provenance-linux-amd64-v1.12.0.zip";
      url = "https://github.com/provenance-io/provenance/releases/download/v1.11.1/provenance-linux-amd64-v1.11.1.zip";
      # sha256="0bj8ay1vxplx5l9w19vwgv254s60c804zx11h9jlk0lvd6rz2xa0";
      sha256="0afznyw7gh4h8sswdw8b7bjc6594vgi4ldzv74cy4mk1sgjib4h4";
    };
    buildInputs = [ pkgs.leveldb ];
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    unpackPhase = ":";
    buildPhase = ":";
    installPhase = ''
      mkdir $out
      cd $out
      ${pkgs.unzip}/bin/unzip $src
    '';
  };
}
