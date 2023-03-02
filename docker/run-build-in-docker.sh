#!/usr/bin/env bash
set -eu

OUT_DIR=/app/docker-outputs

RUST_NANOS_SDK=`mktemp -d`
TARGET_DIR=`mktemp -d`

git clone "$RUST_NANOS_SDK_GIT" "$RUST_NANOS_SDK"
cd "$RUST_NANOS_SDK"; git checkout "$RUST_NANOS_SDK_REV"; cd -

PATH=$RUST_NANOS_SDK/scripts:$PATH
export OBJCOPY="llvm-objcopy"
export NM="llvm-nm"

# The following are needed for protobuf related codegen
export COSMOS_SDK=`mktemp -d`
git clone https://github.com/cosmos/cosmos-sdk.git "$COSMOS_SDK"
cd "$COSMOS_SDK"; git checkout 518003ec29455e0eeb3b46219a940d32b860973f; cd -

BIN="/usr/local/bin" && \
    VERSION="1.14.0" && \
    curl -sSL \
         "https://github.com/bufbuild/buf/releases/download/v${VERSION}/buf-$(uname -s)-$(uname -m)" \-o "${BIN}/buf" && \
    chmod +x "${BIN}/buf"

export PROTO_INCLUDE=$(readlink -e $(dirname $(which protoc))/../include)

cd rust-app

for device in nanos nanosplus nanox
do
   cargo +nightly build --target-dir=$TARGET_DIR --release --target=$device.json -Z build-std=core
   cp $TARGET_DIR/$device/release/$APP_NAME $OUT_DIR/$device
   chown $HOST_UID:$HOST_GID $OUT_DIR/$device/$APP_NAME
done
