rec {
  alamgu = import ./dep/alamgu {};

  cosmos-sdk = alamgu.thunkSource ./dep/cosmos-sdk;

  inherit (alamgu) lib pkgs crate2nix;

  protobufOverrides = pkgs: attrs: {
    PROTO_INCLUDE = "${pkgs.buildPackages.protobuf}/include";
    nativeBuildInputs = (attrs.nativeBuildInputs or []) ++ (with pkgs.buildPackages; [
      protobuf
    ]);
  };

  bufCosmosOverrides = pkgs: attrs: let
    super = protobufOverrides pkgs attrs;
    self = super // {
      COSMOS_SDK = cosmos-sdk;
      nativeBuildInputs = (attrs.nativeBuildInputs or []) ++ (with pkgs.buildPackages; [
        buf
      ]);
    };
  in self;

  makeApp = { rootFeatures ? [ "default" ], release ? true, device }:
    let collection = alamgu.perDevice.${device};
    in import ./Cargo.nix {
      inherit rootFeatures release;
      pkgs = collection.ledgerPkgs;
      buildRustCrateForPkgs = pkgs: let
        fun = collection.buildRustCrateForPkgsWrapper
          pkgs
          ((collection.buildRustCrateForPkgsLedger pkgs).override {
            defaultCrateOverrides = pkgs.defaultCrateOverrides // {
              proto-gen = protobufOverrides pkgs;
              provenance = attrs: let
                sdk = lib.findFirst (p: lib.hasPrefix "rust_nanos_sdk" p.name) (builtins.throw "no sdk!") attrs.dependencies;
              in bufCosmosOverrides pkgs attrs // {
                preHook = collection.gccLibsPreHook;
                preConfigure = let
                  conf = pkgs.runCommand "fetch-buf" (let
                    super = {
                      outputHashMode = "recursive";
                      outputHashAlgo = "sha256";
                      outputHash = "0c0wacvgb800acyw7n91dxll3fmibyhayi2l6ijl24sv1wykr3ni";

                      nativeBuildInputs = [
                        pkgs.buildPackages.cacert pkgs.buildPackages.buf
                      ];
                    };
                    self = super // protobufOverrides pkgs super;
                  in self) ''
                     mkdir -p $out
                     HOME=$(mktemp -d)
                     curl https://api.buf.build
                     buf build ${cosmos-sdk} \
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
                  "-C" "linker=${pkgs.stdenv.cc.targetPrefix}clang"
                  "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/link.ld"
                ] ++ (if (device == "nanos") then
                  [ "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/nanos_layout.ld" ]
                else if (device == "nanosplus") then
                  [ "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/nanosplus_layout.ld" ]
                else if (device == "nanox") then
                  [ "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/nanox_layout.ld" ]
                else throw ("Unknown target device: `${device}'"));
              };
            };
          });
      in
        args: fun (args // lib.optionalAttrs pkgs.stdenv.hostPlatform.isAarch32 {
          dependencies = map (d: d // { stdlib = true; }) [
            collection.ledgerCore
            collection.ledgerCompilerBuiltins
          ] ++ args.dependencies;
        });
  };

  makeTarSrc = { appExe, device }: pkgs.runCommandCC "makeTarSrc" {
    nativeBuildInputs = [
      alamgu.cargo-ledger
      alamgu.ledgerRustPlatform.rust.cargo
    ];
  } (alamgu.cargoLedgerPreHook + ''

    cp ${./rust-app/Cargo.toml} ./Cargo.toml
    # So cargo knows it's a binary
    mkdir src
    touch src/main.rs

    cargo-ledger --use-prebuilt ${appExe} --hex-next-to-json ledger ${device}

    mkdir -p $out/provenance
    # Create a file to indicate what device this is for
    echo ${device} > $out/provenance/device
    cp app_${device}.json $out/provenance/app.json
    cp app.hex $out/provenance
    cp ${./tarball-default.nix} $out/provenance/default.nix
    cp ${./tarball-shell.nix} $out/provenance/shell.nix
    cp ${./rust-app/crab.gif} $out/provenance/crab.gif
    cp ${./rust-app/crab-small.gif} $out/provenance/crab-small.gif
  '');

  testPackage = (import ./ts-tests/override.nix { inherit pkgs; }).package;

  testScript = pkgs.writeShellScriptBin "mocha-wrapper" ''
    cd ${testPackage}/lib/node_modules/*/
    export NO_UPDATE_NOTIFIER=true
    exec ${pkgs.nodejs-14_x}/bin/npm --offline test -- "$@"
  '';

  runTests = { appExe, speculosCmd }: pkgs.runCommandNoCC "run-tests" {
    nativeBuildInputs = [
      pkgs.wget alamgu.speculos.speculos testScript
    ];
  } ''
    mkdir $out
    (
    ${speculosCmd} ${appExe} --display headless &
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

  appForDevice = device : rec {
    rootCrate = (makeApp { inherit device; }).rootCrate.build;
    appExe = rootCrate + "/bin/provenance";

    rootCrate-with-logging = (makeApp {
      inherit device;
      release = false;
      rootFeatures = [ "default" "speculos" "extra_debug" ];
    }).rootCrate.build;

    rustShell = alamgu.perDevice.${device}.rustShell.overrideAttrs (bufCosmosOverrides alamgu.ledgerPkgs);

    tarSrc = makeTarSrc { inherit appExe device; };

    tarball = pkgs.runCommandNoCC "app-tarball.tar.gz" { } ''
      tar -czvhf $out -C ${tarSrc} provenance
    '';

    loadApp = pkgs.writeScriptBin "load-app" ''
      #!/usr/bin/env bash
      cd ${tarSrc}/provenance
      ${alamgu.ledgerctl}/bin/ledgerctl install -f ${tarSrc}/provenance/app_${device}.json
    '';

    speculosCmd =
      if (device == "nanos") then "speculos -m nanos"
      else if (device == "nanosplus") then "speculos  -m nanosp -k 1.0.3"
      else if (device == "nanox") then "speculos -m nanox"
      else throw ("Unknown target device: `${device}'");

    test-with-loging = runTests {
      inherit speculosCmd;
      appExe = rootCrate-with-logging + "/bin/provenance";
    };
    test = runTests { inherit appExe speculosCmd; };

    appShell = pkgs.mkShell {
      packages = [ loadApp alamgu.generic-cli pkgs.jq ];
    };
  };

  nanos = appForDevice "nanos";
  nanosplus = appForDevice "nanosplus";
  nanox = appForDevice "nanox";

  inherit (pkgs.nodePackages) node2nix;

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
