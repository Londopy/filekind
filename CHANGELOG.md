# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is validated in CI by
[patchnotes](https://github.com/Londopy/patchnotes); a malformed entry fails the
build, and on a tag push the latest section below becomes the GitHub Release
notes verbatim.

## [0.5.1] - 2026-08-17

### Fixed

- The CLI printed raw ANSI escapes in `cmd.exe` instead of colour
  (`←[1mspec:←[0m ...`). Colour was gated on `stdout().is_terminal()`,
  which is true in a Windows console — but conhost does not interpret escape
  sequences until virtual terminal processing is enabled on the handle, which
  Windows Terminal does for you and the classic console does not. Output now
  goes through `anstream`, which enables VT where the console supports it and
  strips the codes where it does not. `NO_COLOR` and `CLICOLOR_FORCE` are
  honoured as before; piped output and the `--json` and `preview` paths are
  unchanged and remain byte-exact.

## [0.5.0] - 2026-08-13

First public release. The v0.1–v0.4 milestones from the design sheet landed
together rather than in sequence, so they are recorded here as one release
rather than backdated into versions that were never tagged.

Three limitations are worth knowing before you install anything. macOS output
is experimental — a UTI declaration only takes effect inside a real `.app`
bundle, and Launch Services ignores bundles that are not codesigned and
notarised, so `targets.macos` is off by default. Windows default-handler
assignment is not attempted, because Windows 10 and later do not permit it from
a script; filekind registers the ProgID and populates `OpenWithProgids`, and
the user picks the default in Settings. Release binaries are unsigned — see
[SECURITY.md](SECURITY.md) for how to verify them.

### Added

- `filekind-core`: parse a `.filekind` spec and generate every artifact an
  operating system needs to recognise a custom extension. The crate performs no
  I/O and no printing — every generator returns `String` or `Vec<u8>`, which is
  what makes the GUI's live preview possible.
- Windows target: `register.reg`, `unregister.reg`, an Inno Setup `[Registry]`
  snippet and NSIS register/unregister macros.
- Linux target: freedesktop `shared-mime-info` XML, a `.desktop` entry, the
  eight-size hicolor icon tree, and `install.sh` / `uninstall.sh` built on
  `xdg-mime` and `xdg-icon-resource`.
- macOS target (experimental): `UTExportedTypeDeclarations` and
  `CFBundleDocumentTypes` fragments, an `.icns`, and an optional stub `.app`
  bundle behind `--stub-app`.
- Universal artifacts: a libmagic pattern and a generated per-platform README.
- Icon pipeline: one square PNG in; a multi-resolution `.ico`, an `.icns`
  carrying the full Retina set (`ic11`–`ic14`, `ic10`), and eight hicolor PNGs
  out. Lanczos3 resampling, alpha preserved, non-square input padded rather
  than silently cropped.
- Validation with two severities: reserved executable extensions are errors,
  collisions against a bundled 220-entry dataset are warnings. Diagnostics
  carry a dotted field path so the GUI can underline the box that caused them.
- `filekind-cli`: `init`, `build`, `check`, `verify`, `preview`, `icons` and
  `gui`, with the documented exit codes (`0` ok, `1` spec invalid, `2` I/O
  error, `3` generation failure). `check --json` and `verify --json` for
  scripting.
- `filekind-gui`: Tauri v2 + SvelteKit desktop app with four screens, a live
  preview pane debounced at 150 ms, format-preserving round-trip editing via
  `toml_edit`, a wizard, a recent-specs list and drag-and-drop icon input. It
  links `filekind-core` directly and never shells out to the CLI.
- Distribution packaging: Debian control files with a `build-deb.sh` that
  assembles from the already-generated artifacts, and an RPM spec.
- Optional Kaitai Struct integration behind the `kaitai` feature. filekind
  stays the association layer and Kaitai the format layer; the seam
  cross-checks that the spec's magic bytes and the schema's leading `contents`
  agree, and emits a build script rather than running a compiler for you.
- Golden-file test suite: 53 snapshots plus structural tests asserting
  determinism, unique relative output paths, and that no Windows artifact ever
  touches `UserChoice` or `FileExts`.
- TypeScript definitions generated from the Rust types with `ts-rs`, so the
  IPC boundary cannot drift. CI fails if the committed bindings are stale.

### Security

- Handlers are declared in the spec and are never read from the file being
  opened. A data file that can specify what executes when it is double-clicked
  is a malware delivery vehicle, and this is an architectural constraint rather
  than a policy.
- No shell interpolation of user-supplied strings. Every generated command goes
  through per-dialect escaping — registry, XML, Inno, NSIS, freedesktop and
  POSIX `sh` each have their own rules, and a test asserts a hostile handler
  cannot escape any of them.
- `HKEY_CURRENT_USER` and `~/.local/share` by default; `--system` is opt-in and
  explicit.
- Every register action ships with a matching unregister action. The Windows
  uninstall is deliberately conservative: it removes filekind's own values and
  leaves keys another application may share.
- filekind never writes to the registry or the MIME database itself. It emits
  scripts you can read before running them.

[0.5.1]: https://github.com/Londopy/filekind/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/Londopy/filekind/releases/tag/v0.5.0
