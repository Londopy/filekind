# Security policy

filekind generates files that change how an operating system treats data, and
ships installers that put a binary on your PATH. Both are worth taking
seriously.

## Reporting a vulnerability

**Do not open a public issue.**

Use GitHub's private reporting: **Security → Advisories → Report a
vulnerability** on <https://github.com/Londopy/filekind>. That opens a private
thread with the maintainers and gives you a CVE if one is warranted.

What helps:

- the `.filekind` spec that triggered it, or the smallest one that does
- the generated artifact that is wrong, and what you expected instead
- filekind version (`filekind --version`), OS and version
- whether you needed elevated privileges

You should get an acknowledgement within **72 hours** and an assessment within
**7 days**. If a fix is warranted, expect a patch release and an advisory
crediting you unless you would rather stay anonymous.

## Supported versions

| Version | Supported |
|---|---|
| 0.5.x | Yes |
| < 0.5 | No — pre-release, upgrade |

filekind is pre-1.0. Security fixes go to the latest minor release only.

## Threat model

filekind is a **compiler**, not an installer. It reads a spec you wrote and
writes files to a directory you named. It never applies a change to your system
— you run the generated scripts yourself, after reading them. That boundary is
where most of the risk lives, so it is enforced architecturally rather than by
convention.

### What we consider a vulnerability

- **Handler injection from a data file.** Handlers are declared in the spec and
  are *never* read from the file being opened. If you find any path where the
  contents of a `.yourformat` file influence what gets executed, that is the
  most serious bug filekind can have. Report it.
- **Escaping failures.** A generated artifact where a spec value breaks out of
  its quoting — a `.reg` string that terminates early, a `.desktop` `Exec=`
  that gains an argument, a shell script where a handler path becomes a
  command. Six dialects, six sets of rules; a miss in any of them is a
  vulnerability.
- **Path traversal in output.** An artifact path that escapes the output
  directory, or a spec value that reaches a filename unsanitised.
- **Privilege escalation.** Anything that writes outside the user's own scope
  without `--system`, or an install script that touches a root-owned path when
  it said it would not.
- **Reserved-extension bypass.** A spec that gets `exe`, `dll`, `ps1` or
  another executable extension past validation and shadows a type the OS
  executes.

### What we do not consider a vulnerability

- **A spec that registers a type you did not want.** You wrote the spec.
- **Generated scripts doing what they say.** `install.sh` installs. Read it
  first; that is why filekind emits it instead of running it.
- **A collision warning you ignored.** Extensions are not a namespace.
- **The macOS target not working.** It is experimental and documented as such:
  a UTI declaration needs a codesigned, notarised bundle.
- **Not becoming the Windows default handler.** Windows deliberately prevents
  that from a script, and working around it is what gets tools flagged as
  malware. filekind will not.
- **SmartScreen or Gatekeeper warnings on release binaries.** They are
  unsigned. See below.

## Verifying a release

Releases are **not codesigned**. Windows SmartScreen will warn about the MSI,
and macOS Gatekeeper will refuse the `.dmg` until you clear its quarantine
attribute. A code-signing certificate is an unresolved cost, and pretending
otherwise would be worse than saying so.

Until then, checksums are how you verify a download. Every release ships a
`SHA256SUMS` file and a per-file `.sha256`.

```sh
# everything you downloaded, in one go
sha256sum -c SHA256SUMS --ignore-missing

# just one file
sha256sum -c filekind-v0.5.0-x86_64-windows.msi.sha256
```

```powershell
# Windows
(Get-FileHash filekind-v0.5.0-x86_64-windows.msi -Algorithm SHA256).Hash.ToLower()
# compare against the line in SHA256SUMS
```

```sh
# macOS, after verifying the checksum
xattr -d com.apple.quarantine filekind-v0.5.0-aarch64-apple-darwin.dmg
```

Every artifact is built in GitHub Actions from a tagged commit, and the build
logs are public. If a checksum does not match what the release page lists,
**do not run it** — report it.

## Reviewing what filekind generated

The generated files are meant to be read. If you are handed a `dist/` directory
by someone else, these are the things to look at:

```sh
# What will actually execute when the file is opened?
grep -r 'shell\\open\\command' dist/windows/register.reg
grep '^Exec=' dist/linux/*.desktop

# What do the install scripts touch?
grep -nE 'rm |sudo|curl|wget|eval|\$\(' dist/linux/install.sh

# Does the uninstall actually reverse the install?
diff <(grep -o 'xdg-[a-z-]*' dist/linux/install.sh | sort -u) \
     <(grep -o 'xdg-[a-z-]*' dist/linux/uninstall.sh | sort -u)
```

filekind never emits `curl`, `eval`, or command substitution into a generated
script. If you find any of those in output filekind produced, that is a
finding — report it.

## Hardening notes

- Prefer user scope. `--system` writes to `HKEY_LOCAL_MACHINE` and
  `/usr/share`, and needs elevation. The default does not.
- The `kaitai` feature is off by default. With it enabled, filekind still only
  *emits* a script that would invoke `kaitai-struct-compiler` — it does not run
  it. Running another project's compiler should be your decision, not a side
  effect of a build.
- The GUI's Tauri capability set is deliberately small: file dialogs and
  nothing else. All filesystem access happens in Rust, inside named commands.
  The webview is never handed a path it can read on its own.
