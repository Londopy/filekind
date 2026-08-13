//! `filekind verify` — does this file actually match what the spec claims?
//!
//! Useful in two directions: checking that your writer emits the header you
//! declared, and checking that a file someone sent you is what its extension
//! says. Byte inspection only; no parsing, no execution. Whether the contents
//! are *meaningful* is Kaitai's question, not filekind's.

use serde::{Deserialize, Serialize};

use crate::spec::{Container, Spec};

/// The result of checking bytes against a spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// True when every performed check passed.
    pub matches: bool,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub outcome: Outcome,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Pass,
    Fail,
    /// Nothing to check — the spec did not declare anything to check against.
    Skipped,
}

impl VerifyReport {
    fn push(&mut self, name: &str, outcome: Outcome, detail: impl Into<String>) {
        if outcome == Outcome::Fail {
            self.matches = false;
        }
        self.checks.push(Check {
            name: name.to_string(),
            outcome,
            detail: detail.into(),
        });
    }
}

/// Check `bytes` against `spec`. The filename, if known, is checked too.
pub fn verify_bytes(spec: &Spec, bytes: &[u8], file_name: Option<&str>) -> VerifyReport {
    let mut report = VerifyReport {
        matches: true,
        checks: Vec::new(),
    };

    match file_name {
        Some(name) => {
            let actual = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
            match actual {
                Some(ext) if ext == spec.format.extension => {
                    report.push("extension", Outcome::Pass, format!(".{ext}"))
                }
                Some(ext) => report.push(
                    "extension",
                    Outcome::Fail,
                    format!("file is .{ext}, spec declares .{}", spec.format.extension),
                ),
                None => report.push("extension", Outcome::Fail, "file has no extension"),
            }
        }
        None => report.push("extension", Outcome::Skipped, "no filename supplied"),
    }

    match spec.magic_bytes() {
        Err(e) => report.push(
            "magic",
            Outcome::Fail,
            format!("spec's magic is unparseable: {e}"),
        ),
        Ok(None) => report.push("magic", Outcome::Skipped, "spec declares no magic"),
        Ok(Some(expected)) => {
            if bytes.len() < expected.len() {
                report.push(
                    "magic",
                    Outcome::Fail,
                    format!(
                        "file is {} bytes, shorter than the {}-byte signature",
                        bytes.len(),
                        expected.len()
                    ),
                );
            } else if bytes.starts_with(&expected) {
                report.push("magic", Outcome::Pass, crate::magic::to_hex_dump(&expected));
            } else {
                report.push(
                    "magic",
                    Outcome::Fail,
                    format!(
                        "expected {} ({}), found {} ({})",
                        crate::magic::to_hex_dump(&expected),
                        crate::magic::to_ascii_dump(&expected),
                        crate::magic::to_hex_dump(&bytes[..expected.len()]),
                        crate::magic::to_ascii_dump(&bytes[..expected.len()])
                    ),
                );
            }
        }
    }

    match spec.format.container {
        Container::None => report.push("container", Outcome::Skipped, "spec declares no container"),
        Container::Text => {
            if bytes.is_empty() {
                report.push("container", Outcome::Pass, "empty file is valid text");
            } else {
                match std::str::from_utf8(bytes) {
                    Ok(_) => report.push("container", Outcome::Pass, "valid UTF-8"),
                    Err(e) => report.push(
                        "container",
                        Outcome::Fail,
                        format!(
                            "declared text but is not valid UTF-8 (byte {})",
                            e.valid_up_to()
                        ),
                    ),
                }
            }
        }
        Container::Binary => report.push(
            "container",
            Outcome::Skipped,
            "binary imposes no structure to check",
        ),
        c => {
            let sig = c.signature().expect("zip and sqlite have signatures");
            if bytes.starts_with(sig) {
                report.push("container", Outcome::Pass, format!("{c} signature present"));
            } else {
                report.push(
                    "container",
                    Outcome::Fail,
                    format!("declared {c} but the file does not start with its signature"),
                );
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(magic: Option<&str>, container: Container) -> Spec {
        let mut s = Spec::scaffold("Londo Save", "londo");
        s.format.magic = magic.map(str::to_string);
        s.format.container = container;
        s
    }

    #[test]
    fn matching_file_passes() {
        let s = spec(Some("LNDO\\x01"), Container::Binary);
        let r = verify_bytes(&s, b"LNDO\x01rest of the file", Some("save.londo"));
        assert!(r.matches, "{r:?}");
    }

    #[test]
    fn wrong_magic_fails_with_both_dumps() {
        let s = spec(Some("LNDO\\x01"), Container::None);
        let r = verify_bytes(&s, b"XXXX\x01", Some("save.londo"));
        assert!(!r.matches);
        let d = &r.checks.iter().find(|c| c.name == "magic").unwrap().detail;
        assert!(d.contains("4C 4E 44 4F 01") && d.contains("XXXX"), "{d}");
    }

    #[test]
    fn truncated_file_fails_cleanly() {
        let s = spec(Some("LNDO\\x01"), Container::None);
        let r = verify_bytes(&s, b"LN", Some("save.londo"));
        assert!(!r.matches);
    }

    #[test]
    fn zip_container_is_sniffed() {
        let s = spec(None, Container::Zip);
        assert!(verify_bytes(&s, b"PK\x03\x04junk", Some("a.londo")).matches);
        assert!(!verify_bytes(&s, b"nope", Some("a.londo")).matches);
    }

    #[test]
    fn text_container_rejects_invalid_utf8() {
        let s = spec(None, Container::Text);
        assert!(verify_bytes(&s, "héllo".as_bytes(), Some("a.londo")).matches);
        assert!(!verify_bytes(&s, &[0xff, 0xfe, 0x00], Some("a.londo")).matches);
    }

    #[test]
    fn wrong_extension_is_reported_but_magic_is_still_checked() {
        let s = spec(Some("LNDO"), Container::None);
        let r = verify_bytes(&s, b"LNDO...", Some("save.zip"));
        assert!(!r.matches);
        assert_eq!(
            r.checks.iter().find(|c| c.name == "magic").unwrap().outcome,
            Outcome::Pass
        );
    }

    #[test]
    fn nothing_declared_means_everything_skipped() {
        let s = spec(None, Container::None);
        let r = verify_bytes(&s, b"anything", None);
        assert!(r.matches);
        assert!(r.checks.iter().all(|c| c.outcome == Outcome::Skipped));
    }
}
