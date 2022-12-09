# Provenance Ledger app

An [Ledger](https://www.ledger.com/) application for the [Provenance Blockchain](https://provenance.io/).

## Device Compatability

This application is compatible with
- Ledger Nano S, running firmware 2.1.0 and above
- Ledger Nano S+, running firmware 1.0.3
- Ledger Nano X

Note: Compatibility with Ledger Nano X is only possible to check on [Speculos](https://github.com/ledgerHQ/speculos/) emulator,
because the Nano X does not support side-loading apps under development.

## Installing the app

If you don't want to develop the app but just use it, installation should be very simple.
The first step is to obtain a release tarball.
The second step is to load that app from the tarball.

### Obtaining a release tarball

#### Download an official build

Check the [releases page](https://github.com/obsidiansystems/provenance/releases) of this app to see if an official build has been uploaded for this release.
There is a separate tarball for each device.

#### Build one yourself, with Nix

There is a separate tarball for each device.
To build one, run:
```bash
nix-build -A $DEVICE.tarball
```
where `DEVICE` is one of
 - `nanos`, for Nano S
 - `nanox`, for Nano X
 - `nanosplus`, for Nano S+

The last line printed out will be the path of the tarball.

### Installation using the pre-packaged tarball

Before installing please ensure that your device is plugged, unlocked, and on the device home screen.
Installing the app from a tarball can be done using [`ledgerctl`](https://github.com/ledgerHQ/ledgerctl).

#### With Nix

By using Nix, this can be done simply by using the `load-app` command, without manually installing the `ledgerctl` on your system.

```bash
tar xzf /path/to/release.tar.gz
cd rust-app
nix-shell
load-app
```

`/path/to/release.tar.gz` you should replace with the actual path to the tarball.
For example, it might be `~/Downloads/release.tar.gz` if you downloaded a pre-built official release from GitHub, or `/nix/store/adsfijadslifjaslif-release.tar.gz` if you built it yourself with Nix.

#### Without Nix

Without using Nix, the `ledgerctl` can be used directly to install the app with the following commands.
For more information on how to install and use that tool see the [instructions from LedgerHQ](https://github.com/LedgerHQ/ledgerctl).

```bash
tar xzf release.tar.gz
cd rust-app
ledgerctl install -f app.json
```

## Using the app with generic CLI tool

The bundled `generic-cli` tool can be used to obtaining the public key and do signing.

To use this tool using Nix, from the root level of this repo, run this command to enter a shell with all the tools you'll need:
```bash
nix-shell -A $DEVICE.appShell
```
where `DEVICE` is one of
 - `nanos`, for Nano S
 - `nanox`, for Nano X
 - `nanosplus`, for Nano S+

Then, one can use `generic-cli` like this:

- Get a public key for a BIP-32 derivation:
  ```shell-session
  $ generic-cli getAddress "44'/505'/0'/0/0" --useBlock
  a42e71c004770d1a48956090248a8d7d86ee02726b5aab2a5cd15ca9f57cbd71
  ```

- Sign a transaction:
  ```shell-session
  $ generic-cli sign "44'/505'/0'/0/0" --useBlock
  '0a660a640a1e2f636f736d6f732e676f762e763162657461312e4d73674465706f7369741242084b1229747031673575676665676b6c356
  76d6e3034396e35613968676a6e3367656430656b703866326677781a130a056e68617368120a3530303030303030303012560a500a460a1
  f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf824
  2ff5277e6f07aadd935257a3712040a0208011852120210001a00'
    
  Signing:  <Buffer 0a 66 0a 64 0a 1e 2f 63 6f 73 6d 6f 73 2e 67 6f 76 2e 76 31 62 65 74 61 31 2e 4d 73 67 44 65 
  70 6f 73 69 74 12 42 08 4b 12 29 74 70 31 67 35 75 67 66 ... 144 more bytes>
    
  6e1b75cc90370c8d95626dd31b92e547f044f64fd43b489e2f2a6eeefc6078c320b831d5260e01978c9bf54146b6fc8d88fd652d625b8762
  6900e332fdbb6c09
    ```

The exact output you see will vary, since Ledger devices should not be configured to have the same private key!

## Development

See [CONTRIBUTING.md](./CONTRIBUTING.md).
