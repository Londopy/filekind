## What this changes

<!-- One or two sentences. If it fixes an issue: "Fixes #123". -->

## Why

<!-- Skip if it is obvious from the above. -->

## Generated output

<!--
If this changes what filekind emits, paste before/after for one artifact.
`filekind preview -t windows -k reg` prints a single one. This is the fastest
way for a reviewer to see whether the result is right.
-->

## Checklist

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --exclude filekind-gui --all-targets --features kaitai`
- [ ] `cargo test --workspace --exclude filekind-gui --all-features`
- [ ] Golden files reviewed with `cargo insta review`, and I can explain the diff
- [ ] `./scripts/generate-bindings.sh` if any type crossing the IPC boundary changed
- [ ] `CHANGELOG.md` entry under `[Unreleased]`, if this is user-visible

## Constraints

<!-- Tick the ones this PR touches, so a reviewer knows where to look. -->

- [ ] This does not add I/O or printing to `filekind-core`
- [ ] Handlers still come only from the spec, never from a data file
- [ ] No attempt to set a Windows default handler
- [ ] Any new register action has a matching unregister action
- [ ] New string interpolation goes through `generate/escape.rs`
