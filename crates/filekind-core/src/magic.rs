//! Magic-byte strings: decoding, and re-encoding for three different
//! escaping dialects that all mean the same thing.
//!
//! The spec writes magic once, as a Rust/C-style escaped string. libmagic wants
//! octal, freedesktop wants its own hex form, and a human reading the generated
//! README wants hex with spaces. Decode once, encode per consumer.

use crate::error::{Error, Result};

/// Hard cap. A magic longer than this is not a signature, it is a parser, and
/// that is Kaitai's job.
pub const MAX_MAGIC_BYTES: usize = 64;

/// Decode an escaped-byte string into raw bytes.
///
/// Supported escapes: `\xNN`, `\0`, `\n`, `\r`, `\t`, `\\`, `\"`, `\'`.
/// Non-ASCII characters are encoded as UTF-8, which is what you want for a
/// textual magic like `<?xml`.
pub fn decode(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        let esc = chars
            .next()
            .ok_or_else(|| Error::Magic("trailing backslash".into()))?;
        match esc {
            'x' | 'X' => {
                let mut hex = String::new();
                for _ in 0..2 {
                    match chars.peek() {
                        Some(h) if h.is_ascii_hexdigit() => {
                            hex.push(*h);
                            chars.next();
                        }
                        _ => break,
                    }
                }
                if hex.is_empty() {
                    return Err(Error::Magic("\\x with no hex digits".into()));
                }
                let b = u8::from_str_radix(&hex, 16)
                    .map_err(|e| Error::Magic(format!("bad hex escape \\x{hex}: {e}")))?;
                out.push(b);
            }
            '0' => out.push(0),
            'n' => out.push(b'\n'),
            'r' => out.push(b'\r'),
            't' => out.push(b'\t'),
            '\\' => out.push(b'\\'),
            '"' => out.push(b'"'),
            '\'' => out.push(b'\''),
            other => {
                return Err(Error::Magic(format!(
                    "unknown escape \\{other} (supported: \\xNN \\0 \\n \\r \\t \\\\ \\\" \\')"
                )))
            }
        }
    }

    if out.len() > MAX_MAGIC_BYTES {
        return Err(Error::Magic(format!(
            "{} bytes exceeds the {MAX_MAGIC_BYTES}-byte limit",
            out.len()
        )));
    }
    Ok(out)
}

/// Re-encode for the `value=` attribute of a freedesktop `<match type="string">`.
///
/// shared-mime-info accepts `\xNN`; printable ASCII is left alone so the XML
/// stays readable. `&`, `<`, `>` and quotes are XML-escaped by the caller's
/// template — here we only handle the byte-level escaping, and we escape the
/// backslash so a literal `\` in the magic cannot be read as an escape.
pub fn to_freedesktop(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02X}")),
        }
    }
    out
}

/// Re-encode for a libmagic pattern file, which uses C octal escapes.
pub fn to_libmagic(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b' ' => out.push_str("\\ "),
            0x21..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

/// `4C 4E 44 4F 01` — for humans, in the generated README.
pub fn to_hex_dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The same bytes as printable ASCII with dots for non-printables, so a reader
/// can see `LNDO.` and recognise it.
pub fn to_ascii_dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..=0x7e).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_hex_and_literals() {
        assert_eq!(decode("LNDO\\x01").unwrap(), b"LNDO\x01");
        assert_eq!(
            decode("\\0\\n\\r\\t").unwrap(),
            vec![0, b'\n', b'\r', b'\t']
        );
        assert_eq!(decode("a\\\\b").unwrap(), b"a\\b");
    }

    #[test]
    fn utf8_passes_through() {
        assert_eq!(decode("é").unwrap(), vec![0xC3, 0xA9]);
    }

    #[test]
    fn rejects_nonsense() {
        assert!(decode("\\q").is_err());
        assert!(decode("\\").is_err());
        assert!(decode("\\x").is_err());
        assert!(decode(&"A".repeat(MAX_MAGIC_BYTES + 1)).is_err());
    }

    #[test]
    fn encodes_per_dialect() {
        let b = decode("LNDO\\x01").unwrap();
        assert_eq!(to_freedesktop(&b), "LNDO\\x01");
        assert_eq!(to_libmagic(&b), "LNDO\\001");
        assert_eq!(to_hex_dump(&b), "4C 4E 44 4F 01");
        assert_eq!(to_ascii_dump(&b), "LNDO.");
    }

    #[test]
    fn backslash_survives_a_round_trip_through_each_dialect() {
        let b = decode("a\\\\b").unwrap();
        assert_eq!(to_freedesktop(&b), "a\\\\b");
        assert_eq!(to_libmagic(&b), "a\\\\b");
    }
}
