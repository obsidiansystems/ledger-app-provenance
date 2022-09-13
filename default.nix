rec {
  nix-thunk = import ./dep/nix-thunk {};
  thunk-src-in-env = name: var: path: let
	  src = nix-thunk.thunkSource path;
  in alamgu.pkgs.runCommand name {} ''
          mkdir -p $out/nix-support/
          cat <<EOF >$out/nix-support/setup-hook
	  export ${var}=${src}
EOF
  '';
  buf-nixpkgs = import ./dep/nixpkgs {};
  cosmos-sdk = thunk-src-in-env "cosmos-sdk-hook" "COSMOS_SDK" ./dep/cosmos-sdk;
  buf_hook = alamgu.pkgs.runCommand "buf-hooks" {} ''
    mkdir -p $out/nix-support/
    cat <<EOF >$out/nix-support/setup-hook
    export PROTO_INCLDUE="${alamgu.pkgs.protobuf}/include"
    export PATH=${alamgu.pkgs.protobuf}/bin:$PATH
EOF
  '';
  
  alamgu = import ./dep/alamgu {
    extraAppInputs=[cosmos-sdk buf_hook];
    extraNativeAppInputs=[buf-nixpkgs.buf];
  };

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
            rust-app = attrs: let
              sdk = lib.findFirst (p: lib.hasPrefix "rust_nanos_sdk" p.name) (builtins.throw "no sdk!") attrs.dependencies;
            in {
              preHook = alamgu.gccLibsPreHook;
              extraRustcOpts = attrs.extraRustcOpts or [] ++ [
                "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/script.ld"
                "-C" "linker=${pkgs.stdenv.cc.targetPrefix}clang"
              ];
	      PROTO_INCLUDE = "${pkgs.protobuf}/include";
              nativeBuildInputs = [pkgs.protobuf buf-nixpkgs.buf];
              buildInputs = [buf_hook cosmos-sdk];
              
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

    cargo-ledger --use-prebuilt ${rootCrate}/bin/rust-app --hex-next-to-json

    mkdir -p $out/rust-app
    cp app.json app.hex $out/rust-app
    cp ${./tarball-default.nix} $out/rust-app/default.nix
    cp ${./tarball-shell.nix} $out/rust-app/shell.nix
    cp ${./rust-app/crab.gif} $out/rust-app/crab.gif
  '');

  tarball = pkgs.runCommandNoCC "app-tarball.tar.gz" { } ''
    tar -czvhf $out -C ${tarSrc} rust-app
  '';

  loadApp = pkgs.writeScriptBin "load-app" ''
    #!/usr/bin/env bash
    cd ${tarSrc}/rust-app
    ${alamgu.ledgerctl}/bin/ledgerctl install -f ${tarSrc}/rust-app/app.json
  '';

  testPackage = (import ./ts-tests/override.nix { inherit pkgs; }).package;

  testScript = pkgs.writeShellScriptBin "mocha-wrapper" ''
    cd ${testPackage}/lib/node_modules/*/
    export NO_UPDATE_NOTIFIER=true
    exec ${pkgs.nodejs-14_x}/bin/npm --offline test -- "$@"
  '';

  runTests = { appExe ? rootCrate + "/bin/rust-app" }: pkgs.runCommandNoCC "run-tests" {
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
    appExe = rootCrate-with-logging + "/bin/rust-app";
  };
  test = runTests {
    appExe = rootCrate + "/bin/rust-app";
  };

  inherit (pkgs.nodePackages) node2nix;

  appShell = pkgs.mkShell {
    packages = [ loadApp alamgu.generic-cli pkgs.jq ];
  };

  provenanced = pkgs.stdenv.mkDerivation {
    name = "provenance-bin";
    src = builtins.fetchurl {
      url = "https://github.com/provenance-io/provenance/releases/download/v1.12.0/provenance-linux-amd64-v1.12.0.zip";
      sha256="0bj8ay1vxplx5l9w19vwgv254s60c804zx11h9jlk0lvd6rz2xa0";
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
