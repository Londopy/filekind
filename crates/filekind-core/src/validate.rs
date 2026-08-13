//! Validation. The documented rules, plus the checks that stop a spec from
//! producing artifacts that look fine and quietly do nothing.
//!
//! Two severities. **Errors** block generation — the output would be wrong or
//! dangerous. **Warnings** do not — the filesystem is not a registry, plenty of
//! formats share an extension, and a tool that refuses to build because someone
//! else also uses `.dat` is a tool people work around.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::spec::{Container, Spec};

/// Extensions we refuse to register, parsed from the bundled blocklist.
const RESERVED_RAW: &str = include_str!("../data/reserved-extensions.txt");
/// Extensions somebody already uses, parsed from the bundled dataset.
const KNOWN_RAW: &str = include_str!("../data/known-extensions.tsv");

/// Media types defined by RFC 6838 §4.2. Anything else is a typo.
const TOP_LEVEL_TYPES: &[&str] = &[
    "application",
    "audio",
    "example",
    "font",
    "haptics",
    "image",
    "message",
    "model",
    "multipart",
    "text",
    "video",
];

/// Registered freedesktop main categories. A `.desktop` file with an
/// unrecognised category is not invalid, but it usually means a typo, and the
/// entry silently lands in the wrong menu.
const MAIN_CATEGORIES: &[&str] = &[
    "AudioVideo",
    "Audio",
    "Video",
    "Development",
    "Education",
    "Game",
    "Graphics",
    "Network",
    "Office",
    "Science",
    "Settings",
    "System",
    "Utility",
];

#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Generation is refused.
    Error,
    /// Generation proceeds; the user should read this anyway.
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One validation finding, addressed to a specific field so the GUI can put a
/// red underline in the right box.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Dotted path, e.g. `format.extension`.
    pub field: String,
    pub message: String,
    /// What to do about it. Present whenever there is an obvious fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub hint: Option<String>,
}

impl Diagnostic {
    fn error(field: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            field: field.into(),
            message: message.into(),
            hint: None,
        }
    }

    fn warn(field: &str, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            field: field.into(),
            message: message.into(),
            hint: None,
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.severity, self.field, self.message)?;
        if let Some(h) = &self.hint {
            write!(f, "\n    hint: {h}")?;
        }
        Ok(())
    }
}

/// Every reserved extension, lowercase.
pub fn reserved_extensions() -> impl Iterator<Item = &'static str> {
    RESERVED_RAW
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// The bundled collision dataset as `(extension, description)` pairs.
pub fn known_extensions() -> impl Iterator<Item = (&'static str, &'static str)> {
    KNOWN_RAW
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once('\t'))
        .map(|(e, d)| (e.trim(), d.trim()))
}

/// Look up an extension in the bundled dataset.
pub fn known_extension(ext: &str) -> Option<&'static str> {
    let ext = ext.to_ascii_lowercase();
    known_extensions().find(|(e, _)| *e == ext).map(|(_, d)| d)
}

/// Run every rule against a spec.
///
/// Order is stable: errors before warnings, and within a severity, source
/// order. The GUI relies on this to avoid a list that reshuffles as you type.
pub fn validate(spec: &Spec) -> Vec<Diagnostic> {
    let mut d = Vec::new();

    check_format(spec, &mut d);
    check_magic(spec, &mut d);
    check_mime(spec, &mut d);
    check_targets(spec, &mut d);
    check_association(spec, &mut d);
    check_windows(spec, &mut d);
    check_linux(spec, &mut d);
    check_macos(spec, &mut d);
    check_schema(spec, &mut d);

    d.sort_by_key(|x| x.severity);
    d
}

fn check_format(spec: &Spec, d: &mut Vec<Diagnostic>) {
    let f = &spec.format;

    if f.name.trim().is_empty() {
        d.push(Diagnostic::error(
            "format.name",
            "name is empty; it is what the OS shows the user",
        ));
    }
    if f.description.trim().is_empty() {
        d.push(Diagnostic::warn(
            "format.description",
            "no description; Explorer and Nautilus will show the raw MIME type instead",
        ));
    }

    let ext = &f.extension;
    if ext.is_empty() {
        d.push(Diagnostic::error(
            "format.extension",
            "extension is required",
        ));
        return;
    }
    if ext.starts_with('.') {
        d.push(
            Diagnostic::error("format.extension", "extension must not start with a dot").with_hint(
                format!("write `{}` instead of `{ext}`", ext.trim_start_matches('.')),
            ),
        );
    }
    if ext.len() > 16 {
        d.push(Diagnostic::error(
            "format.extension",
            format!("{} characters is over the 16-character limit", ext.len()),
        ));
    }
    if let Some(bad) = ext
        .chars()
        .find(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit())
    {
        let hint = if bad.is_ascii_uppercase() {
            "extensions are case-insensitive on Windows and macOS; declare it lowercase"
        } else {
            "only a-z and 0-9 are portable across Windows, Linux and macOS"
        };
        d.push(
            Diagnostic::error(
                "format.extension",
                format!("`{bad}` is not allowed in an extension"),
            )
            .with_hint(hint),
        );
    }
    if ext.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        d.push(Diagnostic::warn(
            "format.extension",
            "extension starts with a digit; some tooling treats these as version suffixes",
        ));
    }

    let lower = ext.to_ascii_lowercase();
    if reserved_extensions().any(|r| r == lower) {
        d.push(
            Diagnostic::error(
                "format.extension",
                format!("`{lower}` is a reserved executable or script extension"),
            )
            .with_hint(
                "registering it would shadow a type the OS executes; pick a different extension",
            ),
        );
    } else if let Some(owner) = known_extension(&lower) {
        d.push(
            Diagnostic::warn(
                "format.extension",
                format!("`{lower}` is already commonly used for: {owner}"),
            )
            .with_hint("not fatal — extensions are not a namespace — but expect confused users"),
        );
    }
}

fn check_magic(spec: &Spec, d: &mut Vec<Diagnostic>) {
    let Some(raw) = &spec.format.magic else {
        if matches!(spec.format.container, Container::Binary) {
            d.push(
                Diagnostic::warn(
                    "format.magic",
                    "binary container with no magic; `file` and Nautilus can only guess from the extension",
                )
                .with_hint("add a 4-8 byte signature at offset 0"),
            );
        }
        return;
    };

    match crate::magic::decode(raw) {
        Err(e) => d.push(Diagnostic::error("format.magic", e.to_string())),
        Ok(bytes) => {
            if bytes.is_empty() {
                d.push(Diagnostic::warn(
                    "format.magic",
                    "magic decodes to zero bytes",
                ));
                return;
            }
            if bytes.len() < 2 {
                d.push(Diagnostic::warn(
                    "format.magic",
                    "a single-byte magic will collide with almost everything",
                ));
            }
            if let Some(sig) = spec.format.container.signature() {
                if !bytes.starts_with(sig) && !sig.starts_with(bytes.as_slice()) {
                    d.push(
                        Diagnostic::warn(
                            "format.magic",
                            format!(
                                "container is `{}`, whose files always start with {}, but the declared magic is {}",
                                spec.format.container,
                                crate::magic::to_ascii_dump(sig),
                                crate::magic::to_ascii_dump(&bytes)
                            ),
                        )
                        .with_hint(
                            "a zip-container format cannot also have its own bytes at offset 0; \
                             either drop the magic or drop the container",
                        ),
                    );
                }
            }
        }
    }
}

fn check_mime(spec: &Spec, d: &mut Vec<Diagnostic>) {
    let mime = spec.association.mime.trim();
    if mime.is_empty() {
        d.push(Diagnostic::error("association.mime", "mime is required"));
        return;
    }
    let Some((top, sub)) = mime.split_once('/') else {
        d.push(
            Diagnostic::error("association.mime", "not a media type: no `/`")
                .with_hint("e.g. application/vnd.example.thing"),
        );
        return;
    };
    if !TOP_LEVEL_TYPES.contains(&top) {
        d.push(
            Diagnostic::error(
                "association.mime",
                format!("`{top}` is not a registered top-level type"),
            )
            .with_hint(format!("one of: {}", TOP_LEVEL_TYPES.join(", "))),
        );
    }
    if sub.is_empty() {
        d.push(Diagnostic::error("association.mime", "subtype is empty"));
        return;
    }
    // RFC 6838 restricted-name: first char alphanumeric, then a limited set.
    let mut chars = sub.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphanumeric() {
        d.push(Diagnostic::error(
            "association.mime",
            "subtype must start with a letter or digit",
        ));
    }
    if let Some(bad) = sub
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || "!#$&-^_.+".contains(*c)))
    {
        d.push(Diagnostic::error(
            "association.mime",
            format!("`{bad}` is not valid in a subtype (RFC 6838 restricted-name)"),
        ));
    }
    if sub.len() > 127 {
        d.push(Diagnostic::error(
            "association.mime",
            "subtype exceeds 127 characters",
        ));
    }
    if mime != mime.to_ascii_lowercase() {
        d.push(
            Diagnostic::warn(
                "association.mime",
                "media types are case-insensitive; use lowercase",
            )
            .with_hint(mime.to_ascii_lowercase()),
        );
    }
    if !sub.starts_with("vnd.") && !sub.starts_with("prs.") && !sub.starts_with("x-") {
        d.push(
            Diagnostic::warn(
                "association.mime",
                "subtype is outside the vendor (`vnd.`), personal (`prs.`) and unregistered (`x-`) trees",
            )
            .with_hint(
                "unless this type is IANA-registered, use application/vnd.<you>.<thing>",
            ),
        );
    }
}

fn check_targets(spec: &Spec, d: &mut Vec<Diagnostic>) {
    if !spec.targets.any() {
        d.push(
            Diagnostic::error(
                "targets",
                "every target is disabled; there is nothing to generate",
            )
            .with_hint("set at least one of targets.windows / targets.linux / targets.macos"),
        );
    }
}

fn check_association(spec: &Spec, d: &mut Vec<Diagnostic>) {
    let a = &spec.association;

    match a.handler.as_deref().map(str::trim) {
        None | Some("") => d.push(
            Diagnostic::warn(
                "association.handler",
                "no handler; the type will be named and iconised but double-clicking it opens nothing",
            )
            .with_hint("that is a legitimate choice for a data-only format — ignore this if deliberate"),
        ),
        Some(h) => {
            if h.contains('%') {
                d.push(
                    Diagnostic::error(
                        "association.handler",
                        "handler contains an argument placeholder",
                    )
                    .with_hint("put arguments in handler_args, not handler"),
                );
            }
            if h.contains('&') || h.contains('|') || h.contains(';') || h.contains('`') {
                d.push(
                    Diagnostic::error(
                        "association.handler",
                        "handler contains shell metacharacters",
                    )
                    .with_hint(
                        "filekind quotes handler paths but will not emit a command line that \
                         chains processes",
                    ),
                );
            }
        }
    }

    if !a.handler_args.contains("%1")
        && !a.handler_args.contains("%f")
        && !a.handler_args.contains('%')
    {
        d.push(
            Diagnostic::warn(
                "association.handler_args",
                "no `%1` placeholder; the handler will launch without the file path",
            )
            .with_hint(r#"the usual value is "%1""#),
        );
    }

    if let Some(icon) = a.icon.as_deref() {
        if icon.trim().is_empty() {
            d.push(Diagnostic::warn("association.icon", "icon path is blank"));
        } else if !icon.to_ascii_lowercase().ends_with(".png") {
            d.push(
                Diagnostic::error("association.icon", "icon source must be a PNG")
                    .with_hint("one PNG in, .ico/.icns/hicolor out"),
            );
        }
    } else {
        d.push(Diagnostic::warn(
            "association.icon",
            "no icon; the type inherits the generic document icon on every platform",
        ));
    }
}

fn check_windows(spec: &Spec, d: &mut Vec<Diagnostic>) {
    if !spec.targets.windows {
        return;
    }
    let progid = spec.progid();

    // Rules from the ProgID documentation: <=39 chars, alphanumeric and periods
    // only, no leading digit, no consecutive periods.
    if progid.len() > 39 {
        d.push(
            Diagnostic::error(
                "windows.progid",
                format!(
                    "ProgID `{progid}` is {} characters; the limit is 39",
                    progid.len()
                ),
            )
            .with_hint("set [windows].progid explicitly to something shorter"),
        );
    }
    if let Some(bad) = progid
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '.')
    {
        d.push(Diagnostic::error(
            "windows.progid",
            format!(
                "ProgID `{progid}` contains `{bad}`; only letters, digits and periods are allowed"
            ),
        ));
    }
    if progid.starts_with(|c: char| c.is_ascii_digit()) {
        d.push(Diagnostic::error(
            "windows.progid",
            format!("ProgID `{progid}` starts with a digit"),
        ));
    }
    if progid.contains("..") {
        d.push(Diagnostic::error(
            "windows.progid",
            format!("ProgID `{progid}` contains consecutive periods"),
        ));
    }
    for (verb, cmd) in &spec.windows.verbs {
        if verb.chars().any(|c| !c.is_ascii_alphanumeric()) {
            d.push(Diagnostic::error(
                "windows.verbs",
                format!("verb `{verb}` must be alphanumeric"),
            ));
        }
        if !cmd.contains("%1") {
            d.push(Diagnostic::warn(
                "windows.verbs",
                format!("verb `{verb}` has no %1; it will launch without the file"),
            ));
        }
    }
}

fn check_linux(spec: &Spec, d: &mut Vec<Diagnostic>) {
    if !spec.targets.linux {
        return;
    }
    let cats = &spec.linux.desktop_categories;
    if cats.is_empty() {
        d.push(Diagnostic::warn(
            "linux.desktop_categories",
            "no categories; the entry may not appear in any application menu",
        ));
    }
    if !cats.iter().any(|c| MAIN_CATEGORIES.contains(&c.as_str())) && !cats.is_empty() {
        d.push(
            Diagnostic::warn(
                "linux.desktop_categories",
                "none of the categories is a registered freedesktop main category",
            )
            .with_hint(format!("add one of: {}", MAIN_CATEGORIES.join(", "))),
        );
    }
    for c in cats {
        if c.contains(';') {
            d.push(
                Diagnostic::error(
                    "linux.desktop_categories",
                    format!("category `{c}` contains a semicolon"),
                )
                .with_hint("list categories as separate array entries; filekind joins them"),
            );
        }
    }
}

fn check_macos(spec: &Spec, d: &mut Vec<Diagnostic>) {
    if !spec.targets.macos {
        return;
    }

    d.push(
        Diagnostic::warn(
            "targets.macos",
            "macOS output is experimental: a UTI declaration only takes effect inside a real app \
             bundle, and Launch Services ignores bundles that are not codesigned and notarised",
        )
        .with_hint(
            "the generated plist fragment is correct and useful for pasting into an existing \
             signed app; on its own it will appear to do nothing",
        ),
    );

    match spec.macos.bundle_id.as_deref() {
        None => d.push(
            Diagnostic::warn(
                "macos.bundle_id",
                "no bundle_id; the UTI will be derived from the MIME type instead",
            )
            .with_hint(format!("filekind will use `{}`", spec.uti())),
        ),
        Some(b) => {
            if !b.contains('.') {
                d.push(
                    Diagnostic::warn("macos.bundle_id", "bundle_id is not in reverse-DNS form")
                        .with_hint("e.g. wtf.londo.player"),
                );
            }
            if b.chars()
                .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
            {
                d.push(Diagnostic::error(
                    "macos.bundle_id",
                    "bundle_id may only contain letters, digits, hyphens and periods",
                ));
            }
        }
    }

    for u in &spec.macos.uti_conforms_to {
        if !u.contains('.') {
            d.push(Diagnostic::warn(
                "macos.uti_conforms_to",
                format!("`{u}` does not look like a UTI"),
            ));
        }
    }
}

fn check_schema(spec: &Spec, d: &mut Vec<Diagnostic>) {
    let Some(schema) = &spec.schema else { return };
    if let Some(k) = &schema.kaitai {
        if !k.ends_with(".ksy") {
            d.push(Diagnostic::warn(
                "schema.kaitai",
                "kaitai schemas conventionally use the .ksy extension",
            ));
        }
        if cfg!(not(feature = "kaitai")) {
            d.push(
                Diagnostic::warn(
                    "schema.kaitai",
                    "this build of filekind has the `kaitai` feature disabled; the schema will be ignored",
                )
                .with_hint("rebuild with --features kaitai"),
            );
        }
    }
    if schema.kaitai.is_none() && !schema.languages.is_empty() {
        d.push(Diagnostic::warn(
            "schema.languages",
            "languages are listed but no kaitai schema is set",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_with(extra: &str) -> Spec {
        let base = format!(
            r#"
[format]
name = "Londo Save"
extension = "londo"
description = "Londo project save file"

[association]
mime = "application/vnd.london.londo"
handler = "londo-player"
icon = "assets/icon.png"
{extra}
"#
        );
        Spec::from_toml_str(&base).expect("test spec parses")
    }

    fn errors(spec: &Spec) -> Vec<String> {
        validate(spec)
            .into_iter()
            .filter(|d| d.is_error())
            .map(|d| d.field)
            .collect()
    }

    #[test]
    fn a_good_spec_has_no_errors() {
        assert!(errors(&spec_with("")).is_empty());
    }

    #[test]
    fn reserved_extensions_are_rejected() {
        let mut s = spec_with("");
        s.format.extension = "exe".into();
        assert!(errors(&s).contains(&"format.extension".to_string()));
    }

    #[test]
    fn known_extensions_only_warn() {
        let mut s = spec_with("");
        s.format.extension = "zip".into();
        assert!(errors(&s).is_empty(), "collision must not block generation");
        assert!(validate(&s).iter().any(|d| d.field == "format.extension"));
    }

    #[test]
    fn uppercase_and_dots_in_extensions_are_rejected() {
        let mut s = spec_with("");
        s.format.extension = ".Londo".into();
        let e = errors(&s);
        assert!(e.iter().filter(|f| *f == "format.extension").count() >= 2);
    }

    #[test]
    fn bad_mime_is_an_error_but_a_non_vendor_tree_is_a_warning() {
        let mut s = spec_with("");
        s.association.mime = "notatype/thing".into();
        assert!(errors(&s).contains(&"association.mime".to_string()));

        let mut s = spec_with("");
        s.association.mime = "application/londo".into();
        assert!(errors(&s).is_empty());
        assert!(validate(&s).iter().any(|d| d.field == "association.mime"));
    }

    #[test]
    fn shell_metacharacters_in_a_handler_are_an_error() {
        let mut s = spec_with("");
        s.association.handler = Some("londo-player && rm -rf ~".into());
        assert!(errors(&s).contains(&"association.handler".to_string()));
    }

    #[test]
    fn overlong_progid_is_caught() {
        let mut s = spec_with("");
        s.windows.progid = Some("A".repeat(40));
        assert!(errors(&s).contains(&"windows.progid".to_string()));
    }

    #[test]
    fn zip_container_with_its_own_magic_warns() {
        let mut s = spec_with("");
        s.format.container = Container::Zip;
        s.format.magic = Some("LNDO\\x01".into());
        assert!(validate(&s).iter().any(|d| d.field == "format.magic"));
        assert!(errors(&s).is_empty());
    }

    #[test]
    fn macos_target_always_warns_about_codesigning() {
        let mut s = spec_with("");
        s.targets.macos = true;
        assert!(validate(&s).iter().any(|d| d.field == "targets.macos"));
    }

    #[test]
    fn no_targets_is_an_error() {
        let mut s = spec_with("");
        s.targets = crate::Targets {
            windows: false,
            linux: false,
            macos: false,
        };
        assert!(errors(&s).contains(&"targets".to_string()));
    }

    #[test]
    fn datasets_parse() {
        assert!(reserved_extensions().any(|e| e == "exe"));
        assert!(reserved_extensions().all(|e| !e.contains(' ')));
        assert!(known_extension("zip").is_some());
        assert!(known_extension("londo").is_none());
        // No extension may be in both lists: one blocks, the other warns.
        for r in reserved_extensions() {
            assert!(
                known_extension(r).is_none(),
                "`{r}` is both reserved and known"
            );
        }
    }
}
