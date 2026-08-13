//! v0.5: the Kaitai Struct seam.
//!
//! filekind does not reimplement binary format schemas — [Kaitai
//! Struct](https://kaitai.io) exists and is mature. This module is the join
//! between the two tools and deliberately nothing more:
//!
//! - **filekind is the association layer.** It tells the OS the type exists.
//! - **Kaitai is the format layer.** It tells your code how to read it.
//!
//! What this module actually does:
//!
//! 1. Reads the `meta` block of a `.ksy` — enough to cross-check that the
//!    schema and the spec agree about the extension and the magic bytes.
//!    Getting those out of sync is the failure mode this integration exists to
//!    prevent: a `.reg` that claims `LNDO` while the parser expects `LNDX`.
//! 2. Emits a build script that invokes `kaitai-struct-compiler`.
//!
//! What it does not do: run the compiler. Core has no I/O, and shelling out to
//! someone else's JVM is a decision for the caller, behind an explicit flag.
//!
//! The `.ksy` parsing here is deliberately shallow — a line-oriented reader for
//! the handful of `meta` keys we care about, not a YAML implementation. Kaitai
//! owns the schema language; if we needed to understand all of it we would have
//! reimplemented the very thing this module exists to avoid reimplementing.

use serde::Serialize;

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::{Error, Result};
use crate::generate::GenerateOptions;
use crate::spec::Spec;
use crate::validate::{Diagnostic, Severity};

/// The parts of a `.ksy` `meta` block filekind cares about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KsyMeta {
    /// `meta.id` — the generated class name.
    pub id: Option<String>,
    /// `meta.title`.
    pub title: Option<String>,
    /// `meta.file-extension`, as a list (the key accepts a scalar or a list).
    pub file_extensions: Vec<String>,
    /// `meta.endian`.
    pub endian: Option<String>,
    /// Bytes from a leading `contents:` in the first `seq` entry, when it is a
    /// fixed literal. This is the schema's own idea of the magic.
    pub leading_contents: Option<Vec<u8>>,
}

/// What the caller would have to run to produce parsers, and whether the
/// schema agrees with the spec.
#[derive(Debug, Clone, Serialize)]
pub struct KaitaiPlan {
    pub schema_path: String,
    pub meta: KsyMeta,
    pub languages: Vec<String>,
    /// One `kaitai-struct-compiler` invocation per target language.
    pub commands: Vec<Vec<String>>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Languages we default to when `[schema].languages` is empty. Rust because
/// that is what filekind users are already holding; Python because that is
/// what people reach for when poking at a new format.
pub const DEFAULT_LANGUAGES: &[&str] = &["rust", "python"];

/// Parse the `meta` block and the first `seq` entry of a `.ksy`.
///
/// Takes the schema **text**, not a path: this crate does no I/O.
pub fn parse_ksy(text: &str) -> Result<KsyMeta> {
    let mut meta = KsyMeta::default();
    let mut section: Option<&str> = None;
    let mut in_extension_list = false;
    let mut seq_entries_seen = 0usize;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("");
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Top-level key.
        if indent == 0 {
            in_extension_list = false;
            section = match trimmed.trim_end_matches(':') {
                "meta" => Some("meta"),
                "seq" => Some("seq"),
                _ => None,
            };
            continue;
        }

        match section {
            Some("meta") => {
                if in_extension_list {
                    if let Some(item) = trimmed.strip_prefix("- ") {
                        meta.file_extensions.push(unquote(item));
                        continue;
                    }
                    in_extension_list = false;
                }
                let Some((key, value)) = trimmed.split_once(':') else {
                    continue;
                };
                let value = value.trim();
                match key.trim() {
                    "id" => meta.id = Some(unquote(value)),
                    "title" => meta.title = Some(unquote(value)),
                    "endian" => meta.endian = Some(unquote(value)),
                    "file-extension" => {
                        if value.is_empty() {
                            in_extension_list = true;
                        } else if let Some(inner) =
                            value.strip_prefix('[').and_then(|v| v.strip_suffix(']'))
                        {
                            meta.file_extensions
                                .extend(inner.split(',').map(|s| unquote(s.trim())));
                        } else {
                            meta.file_extensions.push(unquote(value));
                        }
                    }
                    _ => {}
                }
            }
            Some("seq") => {
                if trimmed.starts_with("- ") {
                    seq_entries_seen += 1;
                }
                // Only the very first field can be the file's magic.
                if seq_entries_seen > 1 {
                    section = None;
                    continue;
                }
                let body = trimmed.trim_start_matches("- ").trim();
                if let Some((key, value)) = body.split_once(':') {
                    if key.trim() == "contents" {
                        meta.leading_contents = parse_contents(value.trim());
                    }
                }
            }
            _ => {}
        }
    }

    if meta.id.is_none() {
        return Err(Error::Kaitai(
            "no `meta.id` — this does not look like a Kaitai schema".into(),
        ));
    }
    Ok(meta)
}

/// Parse a Kaitai `contents:` value into bytes.
///
/// Kaitai accepts `contents: "LNDO"`, `contents: [0x4c, 0x4e, 1]`, and mixed
/// lists like `[LNDO, 0x01]`. Anything with an expression in it returns `None`
/// — an unknown magic is fine, a wrongly-guessed one is not.
fn parse_contents(value: &str) -> Option<Vec<u8>> {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        let mut out = Vec::new();
        for item in inner.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            if let Some(hex) = item.strip_prefix("0x").or_else(|| item.strip_prefix("0X")) {
                out.push(u8::from_str_radix(hex, 16).ok()?);
            } else if let Ok(n) = item.parse::<u8>() {
                out.push(n);
            } else {
                // A bare or quoted string contributes its UTF-8 bytes.
                out.extend_from_slice(unquote(item).as_bytes());
            }
        }
        return (!out.is_empty()).then_some(out);
    }
    let s = unquote(value);
    (!s.is_empty() && !s.contains(['_', '('])).then(|| s.as_bytes().to_vec())
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Build the plan: what to run, and whether the schema and spec agree.
pub fn plan(spec: &Spec, schema_text: &str) -> Result<KaitaiPlan> {
    let schema_path = spec
        .schema
        .as_ref()
        .and_then(|s| s.kaitai.clone())
        .unwrap_or_else(|| "schema.ksy".to_string());

    let meta = parse_ksy(schema_text)?;
    let mut diagnostics = Vec::new();

    if !meta.file_extensions.is_empty()
        && !meta
            .file_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&spec.format.extension))
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            field: "schema.kaitai".into(),
            message: format!(
                "the schema declares file-extension {:?} but the spec declares `{}`",
                meta.file_extensions, spec.format.extension
            ),
            hint: Some("one of the two is out of date".into()),
        });
    }

    // The whole reason this integration exists: a registration that claims one
    // signature while the parser expects another is worse than no integration.
    match (spec.magic_bytes()?, &meta.leading_contents) {
        (Some(spec_magic), Some(ksy_magic)) => {
            let agrees = spec_magic.starts_with(ksy_magic) || ksy_magic.starts_with(&spec_magic);
            if !agrees {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    field: "format.magic".into(),
                    message: format!(
                        "spec magic {} disagrees with the schema's leading contents {}",
                        crate::magic::to_hex_dump(&spec_magic),
                        crate::magic::to_hex_dump(ksy_magic)
                    ),
                    hint: Some(
                        "the OS would advertise one signature while your parser rejects it".into(),
                    ),
                });
            }
        }
        (None, Some(ksy_magic)) => diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            field: "format.magic".into(),
            message: format!(
                "the schema starts with a fixed {} and the spec declares no magic",
                crate::magic::to_hex_dump(ksy_magic)
            ),
            hint: Some(format!(
                "add  magic = '{}'  to [format] and you get content-based detection for free",
                escape_for_toml_literal(ksy_magic)
            )),
        }),
        _ => {}
    }

    let languages: Vec<String> = spec
        .schema
        .as_ref()
        .map(|s| s.languages.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| DEFAULT_LANGUAGES.iter().map(|s| s.to_string()).collect());

    let commands = languages
        .iter()
        .map(|lang| {
            vec![
                "kaitai-struct-compiler".to_string(),
                "--target".to_string(),
                lang.clone(),
                "--outdir".to_string(),
                format!("parsers/{lang}"),
                schema_path.clone(),
            ]
        })
        .collect();

    Ok(KaitaiPlan {
        schema_path,
        meta,
        languages,
        commands,
        diagnostics,
    })
}

/// Render bytes as the `\xNN` form a TOML literal string can carry.
fn escape_for_toml_literal(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..=0x7e).contains(&b) && b != b'\\' && b != b'\'' {
                (b as char).to_string()
            } else {
                format!("\\x{b:02X}")
            }
        })
        .collect()
}

/// Emit the build glue, if the caller supplied a schema.
pub(crate) fn generate(spec: &Spec, opts: &GenerateOptions, out: &mut Vec<Artifact>) -> Result<()> {
    let Some(text) = &opts.kaitai_schema else {
        return Ok(());
    };
    let plan = plan(spec, text)?;

    if plan
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return Err(Error::Invalid(plan.diagnostics));
    }

    let mut script = String::new();
    script.push_str("#!/bin/sh\n");
    script.push_str(&crate::generate::banner(spec, "#"));
    script.push_str(
        "#\n# Runs kaitai-struct-compiler over the schema this spec points at.\n\
         # filekind does not run it for you: it emits artifacts, and a JVM\n\
         # compiler invocation is your decision, not a side effect of a build.\n\
         #\n# Install: https://kaitai.io/#download\n\nset -eu\n\n",
    );
    script.push_str(
        "command -v kaitai-struct-compiler >/dev/null 2>&1 || {\n    \
         printf 'kaitai-struct-compiler not found; see https://kaitai.io/#download\\n' >&2\n    \
         exit 1\n}\n\n",
    );
    for cmd in &plan.commands {
        let quoted: Vec<String> = cmd.iter().map(|a| crate::generate::escape::sh(a)).collect();
        script.push_str(&quoted.join(" "));
        script.push('\n');
    }
    script.push_str("\nprintf 'Parsers written to parsers/\\n'\n");

    out.push(
        Artifact::text(
            "schema/build-parsers.sh",
            script,
            Platform::Universal,
            ArtifactKind::Kaitai,
            format!(
                "Generates {} parser{} from {}",
                plan.languages.join(" + "),
                if plan.languages.len() == 1 { "" } else { "s" },
                plan.schema_path
            ),
        )
        .exec(),
    );

    let mut notes = String::new();
    notes.push_str(&format!("# {} — schema notes\n\n", spec.format.name));
    notes.push_str(&format!(
        "filekind is the association layer; Kaitai Struct is the format layer.\n\
         This directory is the seam between them.\n\n\
         | | |\n|---|---|\n| Schema | `{}` |\n| Class | `{}` |\n",
        plan.schema_path,
        plan.meta.id.clone().unwrap_or_default()
    ));
    if let Some(t) = &plan.meta.title {
        notes.push_str(&format!("| Title | {t} |\n"));
    }
    if let Some(e) = &plan.meta.endian {
        notes.push_str(&format!("| Endianness | `{e}` |\n"));
    }
    if let Some(c) = &plan.meta.leading_contents {
        notes.push_str(&format!(
            "| Leading contents | `{}` (`{}`) |\n",
            crate::magic::to_hex_dump(c),
            crate::magic::to_ascii_dump(c)
        ));
    }
    notes.push_str(&format!(
        "| Languages | {} |\n\n",
        plan.languages.join(", ")
    ));
    notes.push_str("```sh\n./build-parsers.sh\n```\n");
    if !plan.diagnostics.is_empty() {
        notes.push_str("\n## Warnings\n\n");
        for d in &plan.diagnostics {
            notes.push_str(&format!("- **{}**: {}\n", d.field, d.message));
        }
    }

    out.push(Artifact::text(
        "schema/README.md",
        notes,
        Platform::Universal,
        ArtifactKind::Kaitai,
        "What filekind understood from the schema",
    ));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KSY: &str = r#"
meta:
  id: londo_save
  title: Londo Save
  file-extension: londo
  endian: le
doc: |
  A save file.
seq:
  - id: magic
    contents: [LNDO, 0x01]
  - id: version
    type: u2
"#;

    fn spec() -> Spec {
        let mut s = Spec::scaffold("Londo Save", "londo");
        s.format.magic = Some("LNDO\\x01".into());
        s.schema = Some(crate::spec::SchemaOpts {
            kaitai: Some("londo.ksy".into()),
            languages: vec!["rust".into()],
        });
        s
    }

    #[test]
    fn parses_the_meta_block() {
        let m = parse_ksy(KSY).unwrap();
        assert_eq!(m.id.as_deref(), Some("londo_save"));
        assert_eq!(m.title.as_deref(), Some("Londo Save"));
        assert_eq!(m.file_extensions, vec!["londo"]);
        assert_eq!(m.endian.as_deref(), Some("le"));
        assert_eq!(m.leading_contents, Some(b"LNDO\x01".to_vec()));
    }

    #[test]
    fn parses_extension_lists_in_both_yaml_forms() {
        let inline = parse_ksy("meta:\n  id: a\n  file-extension: [foo, bar]\n").unwrap();
        assert_eq!(inline.file_extensions, vec!["foo", "bar"]);
        let block = parse_ksy("meta:\n  id: a\n  file-extension:\n    - foo\n    - bar\n").unwrap();
        assert_eq!(block.file_extensions, vec!["foo", "bar"]);
    }

    #[test]
    fn a_string_contents_is_read_as_utf8() {
        let m = parse_ksy("meta:\n  id: a\nseq:\n  - id: m\n    contents: \"RIFF\"\n").unwrap();
        assert_eq!(m.leading_contents, Some(b"RIFF".to_vec()));
    }

    #[test]
    fn only_the_first_seq_field_can_be_magic() {
        let m = parse_ksy(
            "meta:\n  id: a\nseq:\n  - id: m\n    type: u4\n  - id: n\n    contents: [0xff]\n",
        )
        .unwrap();
        assert_eq!(m.leading_contents, None);
    }

    #[test]
    fn not_a_schema_is_an_error_not_a_guess() {
        assert!(parse_ksy("hello: world\n").is_err());
    }

    #[test]
    fn agreeing_spec_and_schema_produce_no_diagnostics() {
        let p = plan(&spec(), KSY).unwrap();
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        assert_eq!(p.commands.len(), 1);
        assert!(p.commands[0].contains(&"rust".to_string()));
    }

    #[test]
    fn disagreeing_magic_is_a_hard_error() {
        let mut s = spec();
        s.format.magic = Some("LNDX".into());
        let p = plan(&s, KSY).unwrap();
        assert!(p.diagnostics.iter().any(|d| d.severity == Severity::Error));
    }

    #[test]
    fn a_schema_magic_with_no_spec_magic_suggests_the_toml_line() {
        let mut s = spec();
        s.format.magic = None;
        let p = plan(&s, KSY).unwrap();
        let hint = p.diagnostics[0].hint.clone().unwrap();
        assert!(hint.contains(r"'LNDO\x01'"), "{hint}");
    }

    #[test]
    fn mismatched_extension_only_warns() {
        let mut s = spec();
        s.format.extension = "lndo".into();
        let p = plan(&s, KSY).unwrap();
        assert!(p
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning));
    }
}
