#!/usr/bin/env bash

set -e

cp Cargo.toml Cargo.toml.old

sed -i '/^\[package\]/,/^\[/ s/^version = "\(.*\)-dev"$/version = "\1"/' Cargo.toml

sed -i '/^\[package\.metadata\.android\]/,/^\[/ s/^package = "dev\.\(.*\)"$/package = "org.\1"/' Cargo.toml

sed -i '/^\[package\.metadata\.android\.application\]/,/^\[/ s/^label = "\(.*\) Dev"$/label = "\1"/' Cargo.toml