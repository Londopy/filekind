# Contributing to filekind

Thanks for looking. This document covers how to build the thing, what the
architecture will not let you do, and what a reviewable change looks like.

## Getting set up

```sh
git clone https://github.com/Londopy/filekind
cd filekind
cargo test --workspace --exclude filekind-gui --all-features
```

That is the whole loop for the library and the CLI. Rust 1.88 or newer.

The GUI needs more. It is a workspace member but **not** a default member, on
purpose: a headless CI box should be able to run the full core test suite
without Node or webkit installed.

```sh
# Linux only — Tauri needs webkit2gtk at build *and* runtime
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
                 libayatana-appindicator3-dev librsvg2-dev patchelf

cd crates/filekind-gui
npm install
./../../scripts/stage-cli-sidecar.sh   # the bundler expects the CLI beside it
npm run tauri dev
```

If `tauri dev` complains about a missing sidecar, you skipped
`stage-cli-sidecar.sh`. The installer ships both interfaces, so the bundler
checks for both.

## The one rule that shapes everything

**`filekind-core` performs no I/O and no printing.** Every generator returns
`String` or `Vec<u8>`. The CLI decides where the bytes land; the GUI renders
them into a preview pane.

This is not tidiness. Live preview means regenerating every artifact a few
times a second while someone types. The moment core calls `fs::write` or
`println!` that becomes impossible, and the GUI degrades into a form that
shells out to a CLI.

Two consequences you will run into:

- Icons arrive as bytes (`IconSet::from_png_bytes`), not paths. If your
  generator needs a file, the *caller* reads it.
- Kaitai schemas arrive as text. Same reason.

If you find yourself wanting to open a file in core, the answer is an extra
field on `GenerateOptions`.

## Other constraints that are not up for negotiation

These come from the design sheet and are enforced by tests, so a PR that
crosses one will fail rather than get argued about in review.

- **Handlers come from the spec, never from the data file.** A `.yourformat`
  file must never influence what executes when it is opened.
- **Never set the Windows default handler.** Windows 10+ blocks it, and tools
  that work around the block get flagged as malware. `register.reg` populates
  `OpenWithProgids` and stops there. A test greps every Windows artifact for
  `UserChoice` and `FileExts`.
- **Every register action ships with a matching unregister action.**
- **No shell interpolation of user-supplied strings.** Six output dialects,
  six escaping rules, all in `generate/escape.rs`. Add to that module rather
  than formatting a string inline.
- **filekind never writes to the registry or the MIME database.** It emits
  scripts you can read first.

## Making a change

### Adding a spec field

1. Add it to the struct in `crates/filekind-core/src/spec.rs`, with a
   `#[serde(default)]` if it is optional.
2. Add validation in `validate.rs`. Ask yourself whether it is an error or a
   warning — errors block generation, and the bar for that is "the output would
   be wrong or dangerous", not "this is unusual".
3. Handle it in `SpecDocument::apply` so the GUI's round-trip does not drop it.
4. Add it to the scaffold in `filekind-cli`'s `scaffold_toml`.
5. Use it in a template.
6. `./scripts/generate-bindings.sh` — CI fails if the TypeScript is stale.
7. `cargo insta review` for the golden files it changed.

### Adding a generator

Generators live in `crates/filekind-core/src/generate/`, one module per
platform, with templates in `crates/filekind-core/templates/`.

Template filenames must not contain a dot before the `.j2` — minijinja picks an
autoescape mode from the extension, and a template named `something.xml.j2`
registered as `something.xml` would silently HTML-escape everything. Register
them under dotless names in `templates.rs` and escape explicitly with the
per-dialect filters.

Anything requiring a decision belongs in `templates.rs`'s `context()`, in Rust,
where it can be tested. Templates should read values, not compute them.

### Golden files

Generated artifacts are snapshot-tested. A `.reg` with a stray quote does not
fail a build; it silently registers the wrong string. Snapshots turn "did the
output change?" into a diff.

```sh
cargo test -p filekind-core --test golden
cargo insta review        # accept intentional changes
```

If a change modifies 40 snapshots, that is fine — but say so in the PR
description, and be able to explain one of them.

## Changelog

Add an entry under `## [Unreleased]` for anything a user would notice. Create
the section if it does not exist yet:

```markdown
## [Unreleased]

### Fixed

- `.desktop` entries no longer escape semicolons in `Exec=`, which is a list
  rule and does not apply to string values.
```

CI validates the file with
[patchnotes](https://github.com/Londopy/patchnotes) in strict mode, so
off-spec dates and invented section headings fail the build with an annotation
on the offending line. Valid sections are `Added`, `Changed`, `Deprecated`,
`Removed`, `Fixed`, `Security` and `Breaking`.

You can check locally:

```sh
pip install patchnotes
patchnotes CHANGELOG.md validate --strict
```

Docs-only, CI-only and refactor PRs do not need an entry. The workflow leaves a
note rather than failing, because a bot that demands an entry for every change
just teaches people to write "misc changes".

## Before you open a PR

```sh
cargo fmt --all
cargo clippy --workspace --exclude filekind-gui --all-targets --features kaitai
cargo test --workspace --exclude filekind-gui --all-features
./scripts/generate-bindings.sh && git diff --exit-code crates/filekind-gui/src/lib/bindings
```

CI runs the same four on Linux, Windows and macOS, plus an MSRV check and a
GUI build.

Note the deliberate `--features kaitai` rather than `--all-features` in the
clippy line: the `typescript` feature pulls in `ts-rs`, whose derive macro
warns about serde attributes it does not parse. Those are noise, and CI runs
with `-D warnings`.

## Commit messages and PRs

No enforced format. Write a subject line that says what changed and a body that
says why, if why is not obvious. Keep unrelated changes in separate PRs — a
reformat mixed into a behaviour change is very hard to review.

If your change affects generated output, paste a before/after of one artifact
in the PR description. It is the fastest way for a reviewer to see whether the
result is right.

## Releasing

Maintainers only.

1. Move `[Unreleased]` into a dated section: `patchnotes CHANGELOG.md bump 0.6.0`
2. Bump `version` in the workspace `Cargo.toml` and
   `crates/filekind-gui/src-tauri/tauri.conf.json` to match. CI cross-checks
   the changelog against `Cargo.toml`, so a mismatch fails before anything is
   built.
3. `git tag v0.6.0 && git push --tags`

The release workflow validates the changelog, builds the CLI for five targets
and installers for four, generates `SHA256SUMS`, and publishes a GitHub Release
whose body is the changelog section for that tag.

## Code of conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

Contributions are dual-licensed under MIT OR Apache-2.0, matching the project.
