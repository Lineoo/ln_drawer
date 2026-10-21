#!/usr/bin/env bash
# pwd: should be executed at workspace root
# modify ln_drawer's configuration for release

set -e

#### Cargo ####

# `ln_drawer/Cargo.toml`
# [package]
# version = "0.0.0-alpha.0-dev"
sed -i '/^\[package\]/,/^\[/ s/^version = "\(.*\)-dev"$/version = "\1"/' ln_drawer/Cargo.toml
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' ln_drawer/Cargo.toml | head -n 1)

#### Android & Gradle ####

# `ln_drawer/build/android/app/build.gradle.kts`
# applicationId: "dev.linn.lndrawer" -> "org.linn.lndrawer"
# versionName: "0.0.0-alpha.0-dev"
sed -i 's/applicationId = "dev\./applicationId = "org./' ln_drawer/build/android/app/build.gradle.kts
sed -i "s/versionName = \".*\"/versionName = \"$version\"/" ln_drawer/build/android/app/build.gradle.kts

# `ln_drawer/build/android/app/src/main/res/values/strings.xml`
# app_name: LnDrawer Dev -> LnDrawer
sed -i 's/>LnDrawer Dev</>LnDrawer</' ln_drawer/build/android/app/src/main/res/values/strings.xml

# `ln_drawer/build/android/app/src/main/res/mipmap-xxxhdpi/icon.png`
# icon.png: res/icon_hicolor_dev-512x.png -> res/icon_hicolor_lime-512x.png
ln -sf ../../../../../../../res/icon_hicolor_lime-512x.png ln_drawer/build/android/app/src/main/res/mipmap-xxxhdpi/icon.png

#### Rustc ####

# `env!("LNDRAWER_RELEASE")`
# Release Mode:
# `ln_drawer/src/save.rs::get_file_path`
export LNDRAWER_RELEASE=1
