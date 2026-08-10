#!/usr/bin/env bash

set -e

# [package]
# version = "0.0.0-alpha.0-dev"
sed -i '/^\[package\]/,/^\[/ s/^version = "\(.*\)-dev"$/version = "\1"/' ln_drawer/Cargo.toml

# [package.metadata.android]
# package = "dev.linn.lndrawer"
sed -i '/^\[package\.metadata\.android\]/,/^\[/ s/^package = "dev\.\(.*\)"$/package = "org.\1"/' ln_drawer/Cargo.toml

# [package.metadata.android.application]
# label = "LnDrawer Dev"
sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^label = "\(.*\) Dev"$/label = "\1"/' ln_drawer/Cargo.toml

# icon.png: res/icon_hicolor_dev-512x.png -> res/icon_hicolor_lime-512x.png
ln -sf ../../../res/icon_hicolor_lime-512x.png ln_drawer/build/android/mipmap/icon.png

# src/save.rs::get_file_path
export LN_SAVE_FILE_LOCATION=LnDrawer
