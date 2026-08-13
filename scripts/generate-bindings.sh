#!/bin/sh
# Regenerate the TypeScript definitions the GUI imports.
#
# Do not maintain parallel TypeScript-shaped structs — generate them from the
# Rust definitions. This is that generator. The types come from the
# `#[derive(TS)]` annotations on the real definitions in filekind-core, so
# they cannot drift from what actually crosses the IPC boundary.
#
# CI runs this and fails if the result differs from what is committed.

set -eu
cd "$(dirname "$0")/.."

rm -rf crates/filekind-gui/src/lib/bindings

cd crates/filekind-core
TS_RS_EXPORT_DIR=../filekind-gui/src/lib/bindings \
    cargo test --features typescript export_bindings

cd ../..
printf 'Wrote %s files to crates/filekind-gui/src/lib/bindings\n' \
    "$(find crates/filekind-gui/src/lib/bindings -name '*.ts' | wc -l | tr -d ' ')"
