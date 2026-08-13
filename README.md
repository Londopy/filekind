# filekind

[![CI](https://github.com/Londopy/filekind/actions/workflows/ci.yml/badge.svg)](https://github.com/Londopy/filekind/actions/workflows/ci.yml)
[![Release](https://github.com/Londopy/filekind/actions/workflows/release.yml/badge.svg)](https://github.com/Londopy/filekind/actions/workflows/release.yml)
[![changelog](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Londopy/filekind/gh-pages/changelog-badge.json)](CHANGELOG.md)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#licence)
[![MSRV](https://img.shields.io/badge/rustc-1.77%2B-orange)](#development)

**Make a custom file extension a real, recognised file type — on Windows, Linux and macOS — from one spec file.**

Registering a file type today means hand-writing four unrelated artifacts in
four dialects: a Windows `.reg` tree, a freedesktop shared-mime-info XML plus a
`.desktop` entry, a macOS `Info.plist` UTI block, and a libmagic pattern. Each
is separately and poorly documented. filekind generates all of them, plus icons
in three container formats and the scripts that install and remove them, from a
single declarative file.

```toml
# londo-save.filekind
[format]
name        = "Londo Save"
extension   = "londo"
description = "Londo project save file"
magic       = 'LNDO\x01'
container   = "binary"

[association]
mime    = "application/vnd.london.londo"
icon    = "assets/icon.png"
handler = "londo-player"
```

```console
$ filekind build londo-save.filekind -o dist
  + windows/register.reg          1.9 KiB
  + windows/unregister.reg        1.2 KiB
  + windows/inno-snippet.iss      2.2 KiB
  + windows/nsis-snippet.nsh      2.0 KiB
  + windows/icon.ico             28.1 KiB
  + linux/londo-save-mime.xml       612 B
  + linux/londo-player.desktop      260 B
  + linux/install.sh              2.3 KiB
  + linux/uninstall.sh            1.2 KiB
  + linux/icons/hicolor/…            ×8
  + linux/packaging/…                ×5
  + macos/Info.plist.fragment     2.0 KiB
  + macos/icon.icns              55.8 KiB
  + magic.txt                       609 B
  + README.md                     3.5 KiB

wrote 30 files (182.6 KiB) in dist
nothing has been installed — read the generated README, then run the scripts
```

## Two interfaces, one engine

filekind ships a desktop app and a CLI. Neither is an afterthought, and the
project has not shipped until both have.

- **`filekind-gui`** is how most people will use it. Its signature feature is a
  live preview pane: the actual generated `.reg`, `.xml` and `.plist` update as
  you type, at a 150 ms debounce.
- **`filekind`** (the CLI) exists so builds can run in CI, in Makefiles and over
  SSH, where no window can open.

Both are thin clients over `filekind-core`, and **anything expressible in one is
expressible in the other**.

That symmetry is enforced by an architectural rule rather than by discipline:
`filekind-core` performs **no I/O and no printing**. Every generator returns
`String` or `Vec<u8>`. The CLI decides where the bytes land; the GUI renders
them into the preview pane. If core ever called `fs::write`, live preview would
become impossible and the GUI would degrade into a form that shells out to a
CLI.

## Install

### Prebuilt

Grab the [latest release](https://github.com/Londopy/filekind/releases/latest).

| Platform | File | What you get |
|---|---|---|
| Windows | `filekind-*-x86_64-windows.msi` | Desktop app **and** `filekind.exe` on your PATH |
| Windows (CLI only) | `filekind-cli-*-x86_64-pc-windows-msvc.zip` | Just the binary |
| Linux | `.deb`, `.rpm`, `.AppImage` | Desktop app |
| Linux (CLI only) | `filekind-cli-*-linux-musl.tar.gz` | Static binary, no runtime |
| macOS | `.dmg` | Desktop app (unsigned — see below) |
| macOS (CLI only) | `filekind-cli-*-apple-darwin.tar.gz` | Just the binary |

**Verify what you downloaded.** Releases are not codesigned, so SmartScreen
and Gatekeeper will complain. Every release ships `SHA256SUMS` plus a per-file
`.sha256`:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

[SECURITY.md](SECURITY.md) covers verification on each platform, and why the
binaries are unsigned.

### From source

```sh
cargo install filekind-cli          # the CLI; binary is named `filekind`
cargo build -p filekind-gui         # the desktop app
```

Building the GUI needs a Node toolchain, and on Linux `webkit2gtk` at build
*and* runtime:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev patchelf
cd crates/filekind-gui && npm install && npm run tauri dev
```

## Commands

```
filekind init [name]                  scaffold a spec file interactively
filekind build [spec] -o <dir>        generate all artifacts for enabled targets
filekind build --target windows       restrict to one platform
filekind check [spec]                 validate: syntax, reserved words, collisions
filekind verify <file> --spec <s>     does this file match the declared magic?
filekind preview [spec] -t windows -k reg
                                      print one artifact to stdout
filekind icons <png> -o <dir>         icon pipeline, standalone
filekind gui [spec]                   launch the desktop app
```

Exit codes: `0` ok · `1` spec invalid · `2` I/O error · `3` generation failure.

## What it will not do

These are design constraints, not missing features.

**It will not make your type the default handler on Windows.** Windows 10 and
later deliberately prevent programmatic default-handler assignment. filekind
registers the ProgID and populates `OpenWithProgids`, so the type appears in
*Open with* with your name, description and icon. The user picks the default in
Settings. Tools that circumvent this get flagged as malware.

**It will not install anything.** filekind is a compiler. It emits `.reg` files
and shell scripts you can read before running. It never writes to the registry
or the MIME database itself.

**A data file can never say what opens it.** Handlers are declared in the spec
and are *never* read from the file being opened. If a `.londo` file could
specify what executes when it is double-clicked, the format would be a malware
delivery vehicle. Inno Setup is already abused this way; filekind will not ship
a friendlier version of that.

**Every register action has a matching unregister action.** Mandatory, not
optional.

**macOS is experimental.** A UTI declaration only takes effect inside a real
`.app` bundle, and Launch Services ignores bundles that are not codesigned and
notarised — which needs an Apple Developer account. The generated fragment is
correct and useful for pasting into an app you already ship; on its own it will
appear to do nothing. `targets.macos` is off by default and says so.

**It does not define binary formats.** [Kaitai Struct](https://kaitai.io)
already does that well. The optional `[schema]` table is the seam between the
two: filekind stays the association layer, Kaitai stays the format layer, and
filekind cross-checks that both agree about the magic bytes.

## Layout

```
filekind/
├── crates/
│   ├── filekind-core/     library: parse spec → generate artifacts. No I/O.
│   ├── filekind-cli/      thin binary wrapper
│   └── filekind-gui/      Tauri v2 + SvelteKit; links core directly
├── examples/              sample .filekind specs, including filekind's own
├── scripts/               TypeScript binding generation
└── crates/filekind-core/tests/snapshots/   golden files, one per artifact
```

## Development

```sh
cargo test --workspace --exclude filekind-gui --all-features
cargo insta review                  # when a golden file changes on purpose
./scripts/generate-bindings.sh      # after changing any type crossing the IPC
```

The `filekind-gui` crate is a workspace member but not a default member: a
headless CI box can run the full core test suite without `webkit2gtk` or Node.

Releases are tag-driven. `git tag v0.6.0 && git push --tags` validates
[CHANGELOG.md](CHANGELOG.md), checks it agrees with the workspace version,
builds the CLI for five targets and installers for four, generates checksums,
and publishes a release whose notes *are* the changelog section for that tag.

## Dogfooding

`examples/filekind.filekind` describes filekind's own spec format. Per the
design sheet: if the tool cannot make its own file type work, it is not
finished. CI builds it on every push.

```sh
filekind build examples/filekind.filekind -o dist
cd dist/linux && ./install.sh
xdg-mime query filetype ../../examples/filekind.filekind
# → application/vnd.londo.filekind
```

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) covers the build, the architectural rules a
change has to respect, and what a reviewable PR looks like. Bug reports and
feature requests have templates; security issues go through
[private reporting](SECURITY.md), not the issue tracker.

Everyone participating is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
