#!/usr/bin/env bash

set -e

cp ln_drawer/Cargo.toml ln_drawer/Cargo.toml.old

# [package.metadata.android]
# version = "0.0.0-alpha.0-dev"
sed -i '/^\[package\]/,/^\[/ s/^version = "\(.*\)-dev"$/version = "\1"/' ln_drawer/Cargo.toml

# [package.metadata.android]
# package = "dev.linn.lndrawer"
sed -i '/^\[package\.metadata\.android\]/,/^\[/ s/^package = "dev\.\(.*\)"$/package = "org.\1"/' ln_drawer/Cargo.toml

# [package.metadata.android.application]
# icon = "LnDrawer Dev"
sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^label = "\(.*\) Dev"$/label = "\1"/' ln_drawer/Cargo.toml

# [package.metadata.android.application]
# icon = "@mipmap/icon_dev"
sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^icon = "\(.*\)_dev"$/icon = "\1"/' ln_drawer/Cargo.toml
