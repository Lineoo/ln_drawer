#!/usr/bin/env bash

set -e

cargo build --target wasm32-unknown-unknown --lib
wasm-bindgen --target web --out-dir ./pkg ./target/wasm32-unknown-unknown/debug/ln_drawer.wasm