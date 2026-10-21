#!/usr/bin/env bash
# Build an AppImage from an already compiled ln_drawer binary.
#
# Usage: ln_drawer/build/appimage/build.sh [output]
#
# Environment:
#   BIN          path to the compiled ln_drawer binary
#   APPIMAGETOOL path to appimagetool (default: appimagetool in PATH)

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
OUT="${1:-$ROOT/target/appimage/ln_drawer.AppImage}"
APPDIR="$(dirname "$OUT")/AppDir"

VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/ln_drawer/Cargo.toml" | head -n1)"
if [[ -z "$VERSION" ]]; then
    echo "cannot read version from ln_drawer/Cargo.toml" >&2
    exit 1
fi

case "$VERSION" in
    *-dev) ICON="$ROOT/ln_drawer/res/icon_hicolor_dev-512x.png" ;;
    *) ICON="$ROOT/ln_drawer/res/icon_hicolor_lime-512x.png" ;;
esac

TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
BIN="${BIN:-$ROOT/target/$TARGET/release/ln_drawer}"
[[ -x "$BIN" ]] || BIN="$ROOT/target/release/ln_drawer"
if [[ ! -x "$BIN" ]]; then
    echo "missing ln_drawer binary; build it first or set BIN" >&2
    exit 1
fi

APPIMAGETOOL="${APPIMAGETOOL:-appimagetool}"
if ! command -v "$APPIMAGETOOL" >/dev/null 2>&1; then
    echo "appimagetool not found; install it or set APPIMAGETOOL" >&2
    exit 1
fi

rm -rf "$APPDIR"
install -Dm755 "$BIN" "$APPDIR/usr/bin/ln_drawer"
install -Dm755 "$HERE/AppRun" "$APPDIR/AppRun"
install -Dm644 "$ICON" "$APPDIR/usr/share/icons/hicolor/512x512/apps/ln_drawer.png"
ln -s usr/share/icons/hicolor/512x512/apps/ln_drawer.png "$APPDIR/ln_drawer.png"
ln -s ln_drawer.png "$APPDIR/.DirIcon"

sed "s|@VERSION@|$VERSION|" "$HERE/ln_drawer.desktop" > "$APPDIR/ln_drawer.desktop"
install -Dm644 "$APPDIR/ln_drawer.desktop" "$APPDIR/usr/share/applications/ln_drawer.desktop"

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUT"

echo "$OUT"
