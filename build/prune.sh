#!/usr/bin/env bash

set -e

cp Cargo.toml Cargo.toml.old

# [package.metadata.android]
# version = "0.0.0-alpha.0-dev"
sed -i '/^\[package\]/,/^\[/ s/^version = "\(.*\)-dev"$/version = "\1"/' Cargo.toml

# [package.metadata.android]
# package = "dev.linn.lndrawer"
sed -i '/^\[package\.metadata\.android\]/,/^\[/ s/^package = "dev\.\(.*\)"$/package = "org.\1"/' Cargo.toml

# [package.metadata.android.application]
# icon = "LnDrawer Dev"
sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^label = "\(.*\) Dev"$/label = "\1"/' Cargo.toml

# [package.metadata.android.application]
# icon = "@mipmap/icon-dev"
sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^icon = "\(.*\)-dev"$/icon = "\1"/' Cargo.toml
