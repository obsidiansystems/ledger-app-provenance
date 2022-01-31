# Rust Nano S Application

A simple application that receives a message, displays it, and requests user approval to sign. Can also display an example menu.

## Building

This application has been packaged up with [Nix](https://nixos.org/).
If you are on Linux and have Nix installed, builds and development environments are one command away.

### Prerequisites

This project requires ledger firmware version: 2.1.0 or greater
This project will try to build [nanos-secure-sdk](https://github.com/LedgerHQ/nanos-secure-sdk), so you will need:

#### Linux

1. A standard ARM gcc (`sudo apt-get install gcc-arm-none-eabi binutils-arm-none-eabi`)
2. Cross compilation headers (`sudo apt-get install gcc-multilib`)
2. Python3 (`sudo apt-get install python3`)
3. Pip3 (`sudo apt-get install python3-pip`)

#### Windows

1. install [Clang](http://releases.llvm.org/download.html)
2. install an [ARM GCC toolchain](https://developer.arm.com/tools-and-software/open-source-software/developer-tools/gnu-toolchain/gnu-rm/downloads)
3. [Python](https://www.python.org/)


Other things you will need:
- [Cargo-ledger](https://github.com/LedgerHQ/cargo-ledger.git)
- [Speculos](https://github.com/LedgerHQ/speculos) (make sure you add speculos.py to your PATH by running `export PATH=/path/to/speculos:$PATH`)
- The correct target for rustc: `rustup target add thumbv6m-none-eabi`

You can build on either Windows or Linux with a simple `cargo build` or `cargo build --release`.
It currently builds on stable.

## Loading

You can use [cargo-ledger](https://github.com/LedgerHQ/cargo-ledger.git) which builds, outputs a `hex` file and a manifest file for `ledgerctl`, and loads it on a device in a single `cargo-ledger load` command in the rust-app folder within app directory.

This application is compatible with Ledger Nano S devices running FW 2.1.0 and above. Before installing, please ensure that your device is plugged, unlocked, and on the device home screen. 

### Nix/Linux

Using Nix, from the root level of this repo, run:
```bash
nix-shell -A ledger-platform.rustShell
cd rust-app/
cargo-ledger load
````
You do not need to install cargo-ledger outside of the nix-shell.

Some options of the manifest file can be configured directly in `Cargo.toml` under a custom section:

```yaml
[package.metadata.nanos]
curve = "secp256k1"
flags = "0x40"
icon = "btc.gif"
```

### Using the pre-packaged tarball (any OS)
Installing the app from a tarball can be done using `ledgerctl`. For more information on how to install and use that tool see the [instructions from LedgerHQ](https://github.com/LedgerHQ/ledgerctl).

```bash
tar xzf nano-s-release.tar.gz
cd nano-s-release
ledgerctl install -f app.json
```

## Testing

One can for example use [speculos](https://github.com/LedgerHQ/speculos)

`cargo run --release` defaults to running speculos on the generated binary with the appropriate flags, if `speculos.py` is in your `PATH`.

There is a small test script that sends some of the available commands in `test/test_cmds.py`, or raw APDUs that can be used with `ledgerctl`.
