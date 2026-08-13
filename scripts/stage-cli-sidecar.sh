#!/bin/sh
# Build filekind-cli and place it where the Tauri bundler expects a sidecar.
#
# The installer ships both interfaces — the design sheet is explicit that the
# project has not shipped until it does — so `filekind.exe` rides along inside
# the GUI's MSI and the MSI adds its directory to PATH.
#
# Tauri requires sidecars to be named `<name>-<target-triple><ext>`, which is
# fiddly enough by hand that forgetting it is the normal outcome. Run this once
# before `npm run tauri build` (and before `tauri dev`, which also checks).
#
#     ./scripts/stage-cli-sidecar.sh              # host target
#     ./scripts/stage-cli-sidecar.sh aarch64-apple-darwin
#
# CI calls it in .github/workflows/release.yml.

set -eu
cd "$(dirname "$0")/.."

TARGET="${1:-$(rustc -vV | awk '/^host:/ {print $2}')}"
DEST="crates/filekind-gui/src-tauri/binaries"

EXT=""
case "$TARGET" in
    *windows*) EXT=".exe" ;;
esac

printf 'Building filekind-cli for %s\n' "$TARGET"
cargo build --release --locked -p filekind-cli --target "$TARGET"

mkdir -p "$DEST"
SRC="target/$TARGET/release/filekind$EXT"
[ -f "$SRC" ] || { printf 'filekind: expected %s to exist\n' "$SRC" >&2; exit 1; }

cp "$SRC" "$DEST/filekind-$TARGET$EXT"
printf 'Staged %s\n' "$DEST/filekind-$TARGET$EXT"
