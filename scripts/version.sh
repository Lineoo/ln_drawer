#!/usr/bin/env bash
# pwd: should be executed at workspace root
# Set the project version and update every version reference.

set -euo pipefail

## validation ##

if [[ ! $# -eq 1 ]]; then
    echo "usage: $(basename "$0") <version>" >&2
    echo "  version: semver without leading 'v', e.g. 1.2.3 or 1.2.3-beta.1" >&2
    exit 1
fi

version="$1"

if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
    echo "invalid version '$version': expected semver without leading 'v'" >&2
    exit 1
fi

major=$((10#${BASH_REMATCH[1]}))
minor=$((10#${BASH_REMATCH[2]}))
patch=$((10#${BASH_REMATCH[3]}))

for part in "$major" "$minor" "$patch"; do
    if ((part > 255)); then
        echo "semver component out of range: major, minor and patch must be <= 255" >&2
        exit 1
    fi
done

# versionCode = 1 << 24 | major << 16 | minor << 8 | patch
version_code=$(((1 << 24) | (major << 16) | (minor << 8) | patch))

## string match ##

# `ln_drawer/Cargo.toml`
sed -i "/^\[package\]/,/^\[/ s/^version = \".*\"$/version = \"$version\"/" ln_drawer/Cargo.toml

# `Cargo.lock`
if [[ -f Cargo.lock ]]; then
    sed -i "/^name = \"ln_drawer\"$/,/^$/ s/^version = \".*\"$/version = \"$version\"/" Cargo.lock
fi

# `ln_drawer/build/android/app/build.gradle.kts`
gradle=ln_drawer/build/android/app/build.gradle.kts
sed -i "s/^\( *\)versionName = \".*\"$/\1versionName = \"$version\"/" "$gradle"
sed -i "s/^\( *\)versionCode = .*$/\1versionCode = $version_code/" "$gradle"

echo "version: $version"
echo "versionCode: $version_code"
