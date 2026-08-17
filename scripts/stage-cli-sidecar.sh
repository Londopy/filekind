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

mkdir -p "$DEST"

# universal-apple-darwin is not a real target triple — it is a lipo of two.
#
# The bundler does not consume it as one, either. `tauri build --target
# universal-apple-darwin` builds the app once per architecture and fuses the
# results at the end, so each of those passes resolves `externalBin` against
# its *own* triple. Staging only the fused binary fails the aarch64 pass with:
#
#     resource path `binaries/filekind-aarch64-apple-darwin` doesn't exist
#
# Both real triples therefore have to be present. The fused copy is written
# too: it costs nothing and is what a Tauri that resolves the alias directly
# would look for.
if [ "$TARGET" = "universal-apple-darwin" ]; then
    for arch in x86_64-apple-darwin aarch64-apple-darwin; do
        printf 'Building filekind-cli for %s\n' "$arch"
        cargo build --release --locked -p filekind-cli --target "$arch"
        cp "target/$arch/release/filekind" "$DEST/filekind-$arch"
        printf 'Staged %s\n' "$DEST/filekind-$arch"
    done

    lipo -create -output "$DEST/filekind-universal-apple-darwin" \
        "$DEST/filekind-x86_64-apple-darwin" \
        "$DEST/filekind-aarch64-apple-darwin"
    printf 'Staged %s (fused)\n' "$DEST/filekind-universal-apple-darwin"
    ls -l "$DEST"
    exit 0
fi

printf 'Building filekind-cli for %s\n' "$TARGET"
cargo build --release --locked -p filekind-cli --target "$TARGET"

SRC="target/$TARGET/release/filekind$EXT"
[ -f "$SRC" ] || { printf 'filekind: expected %s to exist\n' "$SRC" >&2; exit 1; }

cp "$SRC" "$DEST/filekind-$TARGET$EXT"
printf 'Staged %s\n' "$DEST/filekind-$TARGET$EXT"
ls -l "$DEST"
