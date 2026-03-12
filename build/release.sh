#!/usr/bin/env bash

set -e

if [ ! -f "./Cargo.toml.old" ]; then
    echo "Cargo.toml is not ready"
    exit
fi

echo "Compiling for x86_64-unknown-linux-gnu..."
cargo build --release --target x86_64-unknown-linux-gnu

echo "Compiling for x86_64-pc-windows-gnu..."
cargo build --release --target x86_64-pc-windows-gnu

echo "Compiling for Android (cargo-apk)..."
source ~/org.linn.keyconf
cargo apk build --release --lib