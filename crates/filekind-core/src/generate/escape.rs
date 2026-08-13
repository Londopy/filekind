//! Per-dialect escaping.
//!
//! Five output formats, five sets of rules about what a backslash means. This
//! is where "no shell interpolation of user-supplied strings" is actually
//! enforced, so every function here is total: it never fails, and it never
//! passes a metacharacter through unchanged.

/// Escape a value for a `.reg` string literal.
///
/// Registry files use C-style escaping inside the quotes: backslash and
/// double-quote both need doubling/escaping. A raw newline cannot appear at
/// all, so control characters are dropped rather than mangled.
pub fn reg(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape text for XML character data or an attribute value.
pub fn xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Escape a value for an Inno Setup string constant.
///
/// Inno doubles embedded double-quotes, and `{` starts a constant, so a literal
/// brace is written `{{`.
pub fn inno(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '{' => out.push_str("{{"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape a value for an NSIS string literal.
///
/// NSIS treats `$` as the start of a variable and uses `$\"` for a literal
/// quote. Backslash is *not* special, which trips up everyone coming from C.
pub fn nsis(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '$' => out.push_str("$$"),
            '"' => out.push_str("$\\\""),
            '`' => out.push_str("$\\`"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape a value for a `.desktop` string entry.
///
/// The freedesktop spec reserves backslash and defines `\\s \\n \\t \\r \\\\` as the
/// escape sequences for a value of type *string*. Note what is **not** here:
/// semicolons. Those only need escaping in *list* values — see
/// [`desktop_list_item`].
pub fn desktop_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape one entry of a `.desktop` list value such as `Categories` or
/// `MimeType`. Semicolons are the separator, so a literal one must be escaped
/// or it silently splits the list into two wrong entries.
pub fn desktop_list_item(s: &str) -> String {
    desktop_string(s).replace(';', "\\;")
}

/// Quote a single argument for POSIX `sh`.
///
/// Single quotes, with the standard `'\''` dance for embedded single quotes.
/// Nothing inside single quotes is interpreted by the shell, which is the
/// entire point: a handler path containing `$(rm -rf ~)` becomes a filename,
/// not a command.
pub fn sh(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Build a `.desktop` `Exec=` line from a handler and its arguments.
///
/// freedesktop quoting rules are their own dialect: the whole argument is
/// wrapped in double quotes, and `"`, `$`, `` ` `` and `\` are backslash-escaped
/// inside them. Field codes like `%f` must sit **outside** any quotes, so they
/// are appended after the quoted program.
pub fn desktop_exec(program: &str, args: &str) -> String {
    let needs_quotes = program.is_empty()
        || program
            .chars()
            .any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c));

    let mut out = String::new();
    if needs_quotes {
        out.push('"');
        for c in program.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
    } else {
        out.push_str(program);
    }

    let args = args.trim();
    if !args.is_empty() {
        out.push(' ');
        out.push_str(args);
    }
    // The value still has to survive .desktop string escaping.
    desktop_string(&out)
}

/// Build a Windows shell `command` value: `"program" args`.
pub fn windows_command(program: &str, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        format!("\"{program}\"")
    } else {
        format!("\"{program}\" {args}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_escapes_paths() {
        assert_eq!(reg(r"C:\Program Files\x.exe"), r"C:\\Program Files\\x.exe");
        assert_eq!(reg(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(reg("a\nb"), "ab");
    }

    #[test]
    fn xml_escapes_all_five() {
        assert_eq!(
            xml(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;"
        );
    }

    #[test]
    fn inno_doubles_quotes_and_braces() {
        assert_eq!(inno(r#"a"b{c"#), r#"a""b{{c"#);
    }

    #[test]
    fn nsis_escapes_dollars() {
        assert_eq!(nsis("$INSTDIR"), "$$INSTDIR");
        assert_eq!(nsis(r#"say "hi""#), r#"say $\"hi$\""#);
    }

    #[test]
    fn desktop_list_items_escape_the_separator_but_strings_do_not() {
        assert_eq!(desktop_list_item("Utility;Game"), "Utility\\;Game");
        assert_eq!(desktop_string("Utility;Game"), "Utility;Game");
        assert_eq!(desktop_string(r"a\b"), r"a\\b");
    }

    #[test]
    fn sh_quoting_neutralises_substitution() {
        assert_eq!(sh("$(rm -rf ~)"), "'$(rm -rf ~)'");
        assert_eq!(sh("it's"), r#"'it'\''s'"#);
    }

    #[test]
    fn exec_quotes_only_when_needed_and_keeps_field_codes_bare() {
        assert_eq!(desktop_exec("londo-player", "%f"), "londo-player %f");
        assert_eq!(
            desktop_exec("/opt/Londo Player/bin/lp", "%f"),
            r#""/opt/Londo Player/bin/lp" %f"#
        );
        // A field code is never wrapped in quotes, even when the program is.
        assert!(desktop_exec("/a b/c", "%f").ends_with(" %f"));
    }

    #[test]
    fn exec_neutralises_a_hostile_handler() {
        // validate() rejects this before it reaches a generator; belt and braces.
        let out = desktop_exec("evil; rm -rf ~", "%f");
        assert!(out.starts_with('"'), "the program must be quoted: {out}");
        assert!(
            out.ends_with("\" %f"),
            "the quote must close before the field code: {out}"
        );
    }
}
