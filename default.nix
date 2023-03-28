{ localSystem ? { system = builtins.currentSystem; }
}:

let
  alamgu = import ./dep/alamgu { inherit localSystem; };
  inherit (alamgu) lib pkgs crate2nix alamguLib;

in rec {

  inherit alamgu;

  cosmos-sdk = alamgu.thunkSource ./dep/cosmos-sdk;

  inherit (alamgu) lib pkgs crate2nix alamguLib;

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
      nativeBuildInputs = (super.nativeBuildInputs or []) ++ (with pkgs.buildPackages; [
        buf
      ]);
    };
  in self;

  appName = "provenance";

  app-nix = alamgu.crate2nix-tools.generatedCargoNix {
    name = "${appName}-nix";
    src = builtins.filterSource (p: _: p != toString "./rust-app/target") ./rust-app;
    additionalCrateHashes = builtins.fromJSON (builtins.readFile ./crate-hashes.json);
  };

  makeLinkerScript = { pkgs, sdkSrc }:
    pkgs.stdenvNoCC.mkDerivation {
      name = "alamgu-linker-wrapper";
      dontUnpack = true;
      dontBuild = true;
      installPhase = ''
        mkdir -p "$out/bin"
        cp "${sdkSrc}/scripts/link_wrap.sh" "$out/bin"
        chmod +x "$out/bin/link_wrap.sh"
      '';
    };

  makeApp = { rootFeatures ? [ "default" ], release ? true, device }:
    let collection = alamgu.perDevice.${device};
    in import app-nix {
      inherit rootFeatures release;
      pkgs = collection.ledgerPkgs;
      buildRustCrateForPkgs = alamguLib.combineWrappers [
        # The callPackage of `buildRustPackage` overridden with various
        # modified arguemnts.
        (pkgs: (collection.buildRustCrateForPkgsLedger pkgs).override {
          defaultCrateOverrides = pkgs.defaultCrateOverrides // {
            ledger-proto-gen = protobufOverrides pkgs;
            nanos_sdk = attrs: {
              passthru = (attrs.passthru or {}) // {
                link_wrap = makeLinkerScript {
                  pkgs = pkgs.buildPackages;
                  sdkSrc = attrs.src;
                };
              };
            };
            ${appName} = attrs: let
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
                      pkgs.buildPackages.cacert pkgs.buildPackages.buf pkgs.buildPackages.curl
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
                "-C" "linker=${sdk.link_wrap}/bin/link_wrap.sh"
                "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/link.ld"
                "-C" "link-arg=-T${sdk.lib}/lib/nanos_sdk.out/${device}_layout.ld"
              ];
              passthru = (attrs.passthru or {}) // { inherit sdk; };
            };
          };
        })

        # Default Alamgu wrapper
        alamguLib.extraArgsForAllCrates

        # Another wrapper specific to this app, but applying to all packages
        (pkgs: args: args // lib.optionalAttrs (alamguLib.platformIsBolos pkgs.stdenv.hostPlatform) {
          dependencies = map (d: d // { stdlib = true; }) [
            collection.ledgerCore
            collection.ledgerCompilerBuiltins
          ] ++ args.dependencies;
        })
      ];
  };

  makeArchiveSource = { appExe, device }:
  let collection = alamgu.perDevice.${device};
  in collection.ledgerPkgs.runCommandCC "${appName}-${device}-tar-src" {
    nativeBuildInputs = [
      alamgu.cargo-ledger
      alamgu.ledgerRustPlatform.rust.cargo
    ];
    strictDeps = true;
  } (alamgu.cargoLedgerPreHook + ''

    cp ${./rust-app/Cargo.toml} ./Cargo.toml
    # So cargo knows it's a binary
    mkdir src
    touch src/main.rs

    cargo-ledger --use-prebuilt ${appExe} --hex-next-to-json ledger ${device}

    dest=$out/${appName}-${device}
    mkdir -p $dest/dep

    # Copy Alamgu build infra thunk
    cp -r ${./dep/alamgu} $dest/dep/alamgu

    # Create a file to indicate what device this is for
    echo ${device} > $dest/device
    cp app_${device}.json $dest/app.json
    cp app.hex $dest
    cp ${./tarball-default.nix} $dest/default.nix
    cp ${./tarball-shell.nix} $dest/shell.nix
    cp ${./rust-app/provenance.gif} $dest/provenance.gif
    cp ${./rust-app/provenance-small.gif} $dest/provenance-small.gif
  '');

  inherit
    (import ./ts-tests { inherit pkgs; })
    testModules
    testScript
    testPackage
    ;

  apiPort = 5005;

  # Tests don't yet run on Darwin
  runTests = { appExe, device, variant ? "", speculosCmd }:
  if pkgs.stdenv.hostPlatform.isDarwin
  then null
  else
  pkgs.runCommandNoCC "run-tests-${device}${variant}" {
    nativeBuildInputs = [
      pkgs.wget alamgu.speculos.speculos testScript
    ];
    strictDeps = true;
  } ''
    mkdir $out
    (
    set +e # Dont exit on error, do the cleanup/kill of background processes
    ${toString speculosCmd} ${appExe} --display headless &
    SPECULOS=$!

    until wget -O/dev/null -o/dev/null http://localhost:${toString apiPort}; do sleep 0.1; done;

    ${testScript}/bin/mocha-wrapper
    rv=$?
    kill -9 $SPECULOS
    exit $rv) | tee $out/short |& tee $out/full &
    TESTS=$!
    (sleep 3m; kill $TESTS) &
    TESTKILLER=$!
    wait $TESTS
    rv=$?
    kill $TESTKILLER
    cat $out/short
    exit $rv
  '';

  makeStackCheck = { rootCrate, device, memLimit, variant ? "" }:
  pkgs.runCommandNoCC "stack-check-${device}${variant}" {
    nativeBuildInputs = [ alamgu.stack-sizes ];
    strictDeps = true;
  } ''
    stack-sizes --mem-limit=${toString memLimit} ${rootCrate}/bin/${appName} ${rootCrate}/bin/*.o | tee $out
  '';

  appForDevice = device: rec {
    app = makeApp { inherit device; };
    app-with-logging = makeApp {
      inherit device;
      release = false;
      rootFeatures = [ "default" "speculos" "extra_debug" ];
    };

    memLimit = {
      nanos = 4500;
      nanosplus = 400000;
      nanox = 400000;
    }.${device} or (throw "Unknown target device: `${device}'");

    stack-check = makeStackCheck { inherit memLimit rootCrate device; };
    stack-check-with-logging = makeStackCheck {
      inherit memLimit device;
      rootCrate = rootCrate-with-logging;
      variant = "-with-logging";
    };

    rootCrate = app.rootCrate.build;
    rootCrate-with-logging = app-with-logging.rootCrate.build;

    appExe = rootCrate + "/bin/" + appName;

    rustShell = alamgu.perDevice.${device}.rustShell.overrideAttrs (old: let
      super = bufCosmosOverrides alamgu.ledgerPkgs old;
    in super // {
      nativeBuildInputs = super.nativeBuildInputs ++ [
        pkgs.yarn
        pkgs.wget
        (makeLinkerScript {
          inherit pkgs;
          sdkSrc = alamgu.thunkSource ./dep/ledger-nanos-sdk;
        })
      ];
    });

    archiveSource = makeArchiveSource { inherit appExe device; };

    tarball = pkgs.runCommandNoCC "${appName}-${device}.tar.gz" {} ''
      dir="${appName}-${device}"
      cp -r "${archiveSource}/$dir" ./
      chmod -R ugo+w "$dir"
      tar -czvhf $out -C . "${appName}-${device}"
    '';

    zip = pkgs.runCommandNoCC "${appName}-${device}.zip" {
      nativeBuildInputs = [ pkgs.zip ];
    } ''
      dir="${appName}-${device}"
      cp -r "${archiveSource}/$dir" ./
      chmod -R ugo+w "$dir"
      zip -r $out "${appName}-${device}"
    '';

    loadApp = pkgs.writeScriptBin "load-app" ''
      #!/usr/bin/env bash
      cd ${archiveSource}/${appName}-${device}
      ${alamgu.ledgerctl}/bin/ledgerctl install -f ${archiveSource}/${appName}-${device}/app.json
    '';

    tarballShell = import (archiveSource + "/${appName}-${device}/shell.nix");

    speculosDeviceFlags = {
      nanos = [ "-m" "nanos" ];
      nanosplus = [ "-m" "nanosp" "-a" "1" ];
      nanox = [ "-m" "nanox" "-a" "1" ];
    }.${device} or (throw "Unknown target device: `${device}'");

    speculosCmd = [
      "speculos"
      "--api-port" (toString apiPort)
    ] ++ speculosDeviceFlags;

    test = runTests { inherit appExe speculosCmd device; };
    test-with-logging = runTests {
      inherit speculosCmd device;
      appExe = rootCrate-with-logging + "/bin/" + appName;
      variant = "-with-logging";
    };

    appShell = pkgs.mkShell {
      packages = [ alamgu.ledgerctl loadApp alamgu.generic-cli pkgs.jq ];
    };
  };

  nanos = appForDevice "nanos";
  nanosplus = appForDevice "nanosplus";
  nanox = appForDevice "nanox";

  cargoFmtCheck = pkgs.stdenv.mkDerivation {
    pname = "cargo-fmt-${appName}";
    inherit (nanos.rootCrate) version src;
    nativeBuildInputs = [
      pkgs.alamguRustPackages.cargo
      pkgs.alamguRustPackages.rustfmt
    ];
    buildPhase = ''
      cargo fmt --all --check
    '';
    installPhase = ''
      touch "$out"
    '';
  };

  inherit (pkgs.nodePackages) node2nix;

} // lib.optionalAttrs pkgs.stdenv.isLinux {

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
