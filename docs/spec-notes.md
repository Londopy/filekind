# Implementation notes against the v0.1 spec sheet

Where the code departs from `filekind-spec-v0.1.md`, and why. Also: answers to
the §10 open questions, decided so the code could be written.

---

## 1. Corrections to the spec sheet

### `magic = "LNDO\x01"` is not valid TOML

The example in §3 does not parse. TOML basic strings define exactly these
escapes — `\b \t \n \f \r \" \\ \uXXXX \UXXXXXXXX` — and `\x` is not among
them. A spec containing it fails at the TOML layer before filekind sees it.

Reaching for a `\u` escape instead is worse than a syntax error, because it
*works* for bytes below 0x80 and silently corrupts everything above: TOML
decodes a `\u00FF` escape to the character U+00FF, which is two bytes in
UTF-8, not one.

**The fix is a TOML literal string:**

```toml
magic = 'LNDO\x01'    # single quotes: no escape processing, filekind decodes it
```

Everything in `examples/` uses this form, `filekind init` scaffolds it with a
comment explaining why, and the CLI test suite asserts that the wrong form
produces an error mentioning the escape sequence.

### `[windows].icon_path` added

§4.1 lists `ProgID\DefaultIcon` among the Windows artifacts, but a loose `.reg`
cannot know where an installer will put `icon.ico`. Writing a guessed path
registers an icon that does not exist, which looks like a filekind bug forever
after.

The resolution order is now:

1. `[windows].icon_path`, if set — an absolute path you control.
2. Otherwise `"<handler>",0`, borrowing the handler executable's own icon. This
   is what most real registrations do.
3. Otherwise no `DefaultIcon` key at all, plus a comment in the `.reg` and a
   section in the generated README explaining the omission.

The Inno and NSIS snippets never need this: an installer knows its own
`{app}` / `$INSTDIR`.

### MSRV is 1.77, not 1.75

`toml_edit` and the `image`/`png` chain both need it. 1.75 is not reachable
without pinning dependencies to versions old enough to have their own problems.

### `ArtifactBody` is a struct-variant enum

The obvious `enum { Text(String), Binary(Vec<u8>) }` cannot be serialised with
serde's internal tagging — "cannot serialize tagged newtype variant". Since
these types cross the Tauri IPC boundary as-is (§2), the representation is
fixed:

```json
{ "encoding": "text",   "text":   "Windows Registry Editor…" }
{ "encoding": "binary", "base64": "AAABAAYAEBA…" }
```

Base64 rather than serde's default `Vec<u8>`, which would send a 256×256 icon
as roughly 700 KB of decimal digits.

### Additions worth naming

- `[windows].verbs` — extra shell verbs beyond `open`.
- `[linux].no_display` — register the handler without cluttering the app menu.
  The dogfood spec uses it.
- `[macos].uti` — override the derived UTI.
- `ArtifactKind::Bundle` — `PkgInfo` is neither a plist nor a script.
- `filekind check --strict` and `--json`; `verify --json`; `preview --list`.

---

## 2. The §10 open questions, answered

### Collision dataset: bundle or query?

**Bundled**, as the sheet suspected. `filekind check` must give the same answer
offline, in CI, and in five years. `crates/filekind-core/data/known-extensions.tsv`
holds ~220 entries and is a **warning**, never an error — the filesystem has no
registry and plenty of formats already share an extension. The separate
`reserved-extensions.txt` (~80 entries: `exe`, `dll`, `ps1`, `desktop`, `so`, …)
*is* an error, because shadowing an executable type is either a mistake or an
attack. A unit test asserts the two lists never overlap.

### Should `container = "zip"` scaffold an archive layout?

**No — it informs, it does not scaffold.** Generating a starter archive would
make filekind a format designer, which §1 rules out. What the container does
affect:

- the `<generic-icon>` in the MIME XML (`package-x-generic` for zip),
- the default `UTTypeConformsTo` (`public.archive`),
- `filekind verify`, which sniffs `PK\x03\x04`,
- a validation warning if you declare both a zip container *and* your own magic
  at offset 0 — those cannot both be true,
- the generated `magic.txt`, which explains that `file` will only ever report
  `application/zip` and that telling two zip-based formats apart needs a real
  parser.

### Windows: `.reg` only, or a `register.exe` helper?

**`.reg` only.** A binary you cannot read is exactly the thing §8 is about. The
Inno and NSIS snippets cover the installer case, and they run inside a program
the user already chose to run.

### Do per-target tables scale, or should overrides nest under `[association]`?

**Per-target tables scale; keep them.** The overrides are not variations on one
shared concept — a ProgID, a `Categories=` list and a UTI conformance array
have nothing in common but the file they live in. Nesting them under
`[association]` would produce `association.windows.progid`, which is longer and
implies a relationship that does not exist. The current shape also lets
`[windows]` grow platform-only ideas like `verbs` without polluting the others.

### GUI distribution: signed installers, or unsigned plus `cargo install`?

Not resolved by code, but the shape of the answer is in the repo. The CLI is a
single static binary (`cargo install filekind-cli`, musl for Linux) and needs no
signing. The GUI does: SmartScreen flagging an unsigned binary is a bad first
impression for a tool *about* file associations. CI builds the app unsigned on
all three platforms with `--no-bundle`, which keeps the code honest without
committing to a certificate. Add signing when there is a certificate to sign
with.

### crates.io name

Unverified from here — check `filekind` before the first publish. The workspace
is already arranged for the fallback: the binary is named `filekind` inside a
crate named `filekind-cli`, so publishing under the longer name changes nothing
a user types.

---

## 3. What is verified, and what is not

Verified by running it:

- 43 unit tests, 12 golden-file tests over 53 snapshots, 13 CLI integration
  tests. `cargo clippy --all-targets --all-features` is clean; `cargo fmt` is
  clean.
- Generated XML is well-formed (`xmllint`), generated shell scripts parse
  (`sh -n`), the `.ico` and `.icns` are readable by `file`, and the `.icns`
  contains the full Retina set (`ic11`–`ic14`, `ic10`) alongside the legacy
  entries.
- The full `init → check → build → verify` loop, on the examples in this repo.

**Not verified: the GUI.** `crates/filekind-gui` was written but never
compiled. The build box had no `webkit2gtk` or `gtk+-3.0` development headers
and no root to install them, so `cargo check -p filekind-gui` cannot get past
`system-deps`. The Rust source parses (`rustfmt` accepts it) and the
TypeScript imports only generated bindings that do exist, but "parses" is a
much weaker claim than "compiles", and neither is "runs". Expect to fix things
on the first real build. The CI workflow has the job that would have caught
them.

Also unverified: nothing was installed on a real Windows, Linux or macOS
machine. The artifacts are correct as far as `xmllint`, `sh -n` and the
documentation go, which is not the same as watching Explorer show the icon.

**Partly verified: the release pipeline.** GitHub Actions cannot run here, so
the workflows have never executed. What *was* checked locally: every workflow
and issue-template YAML parses; `CHANGELOG.md` passes the real
`patchnotes ... validate --strict` and `check-version` against both the tag and
the workspace `Cargo.toml`; the packaging, checksum and release-body shell
logic was replayed by hand and produced a valid `SHA256SUMS` that
`sha256sum -c` accepts; the WiX fragment is well-formed XML; and both helper
scripts pass `sh -n` / run correctly. The parts that genuinely need a runner —
`cross`, the Tauri bundler, `gh release` — are unexercised.

Two things to expect on the first real release run. The MSI job stages the CLI
as a Tauri sidecar, which means `bundle.externalBin` now requires
`scripts/stage-cli-sidecar.sh` to have run before *any* `tauri dev` or
`tauri build`, including locally. And `npm ci` needs a committed
`package-lock.json`; until one exists the workflows fall back to `npm install`,
so commit the lock file after the first local `npm install`.
