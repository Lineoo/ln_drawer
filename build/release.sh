#!/usr/bin/env bash

set -e

if [ ! -f "./Cargo.toml.old" ]; then
    echo "Cargo.toml is not ready"
    exit
fi

echo -e "\033[34m:: Compiling for x86_64-unknown-linux-gnu...\033[0m"
cargo build --release --target x86_64-unknown-linux-gnu

echo -e "\033[34m:: Compiling for x86_64-pc-windows-gnu...\033[0m"
cargo build --release --target x86_64-pc-windows-gnu

echo -e "\033[34m:: Compiling for Android (cargo-apk)...\033[0m"
source ~/org.linn.keyconf
cargo apk build --release --lib --target aarch64-linux-android

echo -e "\033[32m:: Compiling finished.\033[0m"