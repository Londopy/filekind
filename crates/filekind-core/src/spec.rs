//! The `.filekind` spec file: types, parsing, derived values, round-tripping.
//!
//! One TOML file is the single source of truth. Everything the generators emit
//! is a pure function of this struct — which is also why the GUI and the CLI
//! cannot drift: they both hand a [`Spec`] to the same code.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::error::{Error, Result};

/// A parsed `.filekind` spec.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub format: Format,
    pub association: Association,
    #[serde(default)]
    pub targets: Targets,
    #[serde(default)]
    pub windows: WindowsOpts,
    #[serde(default)]
    pub linux: LinuxOpts,
    #[serde(default)]
    pub macos: MacosOpts,
    /// v0.5, optional. filekind is the association layer; Kaitai is the format
    /// layer. This table is the seam between them and nothing more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub schema: Option<SchemaOpts>,
}

/// `[format]` — what the file *is*.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Format {
    /// Human-readable name, shown in OS UI ("Londo Save").
    pub name: String,
    /// Extension without a leading dot. Validated hard; see [`crate::validate`].
    pub extension: String,
    /// One-line description, shown in Explorer's Type column and Nautilus.
    pub description: String,
    /// Optional escaped-byte string matched at offset 0, e.g. `"LNDO\x01"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub magic: Option<String>,
    #[serde(default)]
    pub container: Container,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub version: Option<String>,
}

/// What the bytes are wrapped in. Informs the magic rule and the generated
/// README; it does **not** cause filekind to generate a parser.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    Zip,
    Sqlite,
    Text,
    Binary,
    #[default]
    None,
}

impl Container {
    /// Bytes this container always starts with, if any. Used to cross-check a
    /// declared `magic` against a declared `container`, and by `verify`.
    pub fn signature(self) -> Option<&'static [u8]> {
        match self {
            Container::Zip => Some(b"PK\x03\x04"),
            Container::Sqlite => Some(b"SQLite format 3\0"),
            Container::Text | Container::Binary | Container::None => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Container::Zip => "zip",
            Container::Sqlite => "sqlite",
            Container::Text => "text",
            Container::Binary => "binary",
            Container::None => "none",
        }
    }
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `[association]` — how the OS should treat it.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Association {
    /// RFC 6838 media type. Vendor tree (`vnd.`) strongly recommended.
    pub mime: String,
    /// Path to a source PNG, relative to the spec file. Core never opens it —
    /// the caller reads the bytes and hands them to [`crate::IconSet`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub icon: Option<String>,
    /// Executable name, absolute path, or macOS bundle id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub handler: Option<String>,
    /// Argument template. `%1` is normalised per platform (`%f` on Linux).
    #[serde(default = "default_handler_args")]
    pub handler_args: String,
}

fn default_handler_args() -> String {
    "\"%1\"".to_string()
}

/// `[targets]` — which platforms to emit for.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Targets {
    #[serde(default = "yes")]
    pub windows: bool,
    #[serde(default = "yes")]
    pub linux: bool,
    /// Off by default: a UTI declaration without a signed app bundle does
    /// nothing. See the warning `validate` emits when you turn it on.
    #[serde(default)]
    pub macos: bool,
}

fn yes() -> bool {
    true
}

impl Default for Targets {
    fn default() -> Self {
        Targets {
            windows: true,
            linux: true,
            macos: false,
        }
    }
}

impl Targets {
    pub fn any(&self) -> bool {
        self.windows || self.linux || self.macos
    }
}

/// `[windows]` overrides.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsOpts {
    /// Programmatic identifier. Defaults to a value derived from the MIME
    /// vendor tree; see [`Spec::progid`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub progid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub perceived_type: Option<PerceivedType>,
    /// Where the `.ico` will live once installed, e.g.
    /// `C:\\Program Files\\Londo\\icon.ico`.
    ///
    /// A loose `.reg` file cannot know your install directory, and writing a
    /// guess would register an icon path that does not exist. When this is
    /// unset filekind points `DefaultIcon` at the handler executable instead,
    /// and if there is no handler it omits the key and says so in the
    /// generated README. The Inno and NSIS snippets do not need this — an
    /// installer knows where it put things.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub icon_path: Option<String>,
    /// Extra verbs beyond `open`, e.g. `edit = "notepad.exe \"%1\""`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub verbs: BTreeMap<String, String>,
}

/// Windows `PerceivedType`. Drives grouping in Explorer and the default
/// preview handler.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerceivedType {
    Text,
    Image,
    Audio,
    Video,
    Compressed,
    Document,
    System,
    Application,
    Gamemedia,
    Contacts,
}

impl PerceivedType {
    pub fn as_str(self) -> &'static str {
        match self {
            PerceivedType::Text => "text",
            PerceivedType::Image => "image",
            PerceivedType::Audio => "audio",
            PerceivedType::Video => "video",
            PerceivedType::Compressed => "compressed",
            PerceivedType::Document => "document",
            PerceivedType::System => "system",
            PerceivedType::Application => "application",
            PerceivedType::Gamemedia => "gamemedia",
            PerceivedType::Contacts => "contacts",
        }
    }
}

/// `[linux]` overrides.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxOpts {
    #[serde(default = "default_categories")]
    pub desktop_categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub generic_name: Option<String>,
    /// Emitted as `NoDisplay=true` — the handler registers for the type but
    /// does not clutter the application menu.
    #[serde(default)]
    pub no_display: bool,
}

fn default_categories() -> Vec<String> {
    vec!["Utility".to_string()]
}

impl Default for LinuxOpts {
    fn default() -> Self {
        LinuxOpts {
            desktop_categories: default_categories(),
            generic_name: None,
            no_display: false,
        }
    }
}

/// `[macos]` overrides.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacosOpts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uti_conforms_to: Vec<String>,
    /// Override the exported UTI. Defaults to `<bundle_id>.<extension>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub uti: Option<String>,
}

/// `[schema]` — v0.5 Kaitai seam.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaOpts {
    /// Path to a `.ksy`, relative to the spec file. Core never opens it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional = nullable))]
    pub kaitai: Option<String>,
    /// Target languages for `kaitai-struct-compiler -t`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
}

// Parsing and derived values

impl Spec {
    /// Parse a spec from TOML text. Does **not** validate; call
    /// [`crate::validate`] or use [`Spec::parse_and_validate`].
    pub fn from_toml_str(s: &str) -> Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Parse and reject anything that fails a hard validation rule. Warnings
    /// are returned alongside the spec rather than swallowed.
    pub fn parse_and_validate(s: &str) -> Result<(Self, Vec<crate::validate::Diagnostic>)> {
        let spec = Self::from_toml_str(s)?;
        let diags = crate::validate(&spec);
        if diags.iter().any(|d| d.severity == crate::Severity::Error) {
            return Err(Error::Invalid(diags));
        }
        Ok((spec, diags))
    }

    /// Serialise to TOML. Used by `filekind init`; the GUI uses
    /// [`SpecDocument`] instead so it does not destroy comments.
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Extension with a leading dot: `.londo`.
    pub fn dotted_extension(&self) -> String {
        format!(".{}", self.format.extension)
    }

    /// A filesystem-safe lowercase slug derived from the format name, used for
    /// generated filenames (`londo-save-mime.xml`).
    pub fn slug(&self) -> String {
        let mut out = String::new();
        let mut prev_dash = true; // suppress a leading dash
        for ch in self.format.name.chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_lowercase());
                prev_dash = false;
            } else if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        }
        while out.ends_with('-') {
            out.pop();
        }
        if out.is_empty() {
            out = self.format.extension.clone();
        }
        out
    }

    /// The Windows ProgID.
    ///
    /// Explicit `[windows].progid` wins. Otherwise it is derived from the MIME
    /// vendor tree, because that is the one part of the spec already required
    /// to be globally unique: `application/vnd.london.londo` yields
    /// `London.Londo.1`. Without a `vnd.` tree we fall back to a PascalCase of
    /// the format name, which is unique enough for HKCU and no worse than what
    /// most installers do.
    pub fn progid(&self) -> String {
        if let Some(p) = &self.windows.progid {
            return p.clone();
        }
        let subtype = self.association.mime.split('/').nth(1).unwrap_or("");
        let vendor_path = subtype.strip_prefix("vnd.").unwrap_or("");
        if !vendor_path.is_empty() {
            let parts: Vec<String> = vendor_path
                .split('.')
                .filter(|s| !s.is_empty())
                .map(pascal_case)
                .collect();
            if !parts.is_empty() {
                return format!("{}.1", parts.join("."));
            }
        }
        format!("{}.1", pascal_case(&self.format.name))
    }

    /// The exported UTI. `[macos].uti` wins, then `<bundle_id>.<ext>`, then a
    /// reverse-DNS form derived from the MIME vendor tree.
    pub fn uti(&self) -> String {
        if let Some(u) = &self.macos.uti {
            return u.clone();
        }
        if let Some(b) = &self.macos.bundle_id {
            return format!("{}.{}", b, self.format.extension);
        }
        // The vendor tree is already reverse-DNS-shaped: `vnd.londo.filekind`
        // has the organisation first, exactly like a UTI. Reversing it would
        // produce `filekind.londo`, which reads backwards and conforms to
        // nothing. Keep the order and let `validate` nag for a real bundle_id.
        let subtype = self.association.mime.split('/').nth(1).unwrap_or("");
        if let Some(vendor_path) = subtype.strip_prefix("vnd.") {
            let parts: Vec<&str> = vendor_path.split('.').filter(|s| !s.is_empty()).collect();
            if parts.len() >= 2 {
                return parts.join(".");
            }
        }
        format!("public.{}", self.format.extension)
    }

    /// UTIs this type conforms to, with a sane default derived from the
    /// container so that Quick Look and Spotlight do something reasonable.
    pub fn uti_conforms_to(&self) -> Vec<String> {
        if !self.macos.uti_conforms_to.is_empty() {
            return self.macos.uti_conforms_to.clone();
        }
        match self.format.container {
            Container::Zip => vec!["public.data".into(), "public.archive".into()],
            Container::Text => vec!["public.text".into()],
            Container::Sqlite | Container::Binary | Container::None => vec!["public.data".into()],
        }
    }

    /// The freedesktop icon name for the MIME type: `application-vnd.london.londo`.
    /// The slash becomes a dash — that is the convention, not a workaround.
    pub fn mime_icon_name(&self) -> String {
        self.association.mime.replace('/', "-")
    }

    /// Basename of the generated `.desktop` file, without extension.
    pub fn desktop_id(&self) -> String {
        match &self.association.handler {
            Some(h) if !h.trim().is_empty() => {
                let base = h.rsplit(['/', '\\']).next().unwrap_or(h);
                let base = base.strip_suffix(".exe").unwrap_or(base);
                let cleaned: String = base
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                            c
                        } else {
                            '-'
                        }
                    })
                    .collect();
                if cleaned.is_empty() {
                    self.slug()
                } else {
                    cleaned
                }
            }
            _ => self.slug(),
        }
    }

    /// Handler arguments normalised for a platform. Windows uses `%1`,
    /// freedesktop uses `%f` (single local file) and forbids quoting the
    /// placeholder. Anything else is passed through untouched.
    pub fn handler_args_for(&self, platform: crate::Platform) -> String {
        let raw = self.association.handler_args.trim();
        match platform {
            crate::Platform::Linux => raw
                .replace("\"%1\"", "%f")
                .replace("'%1'", "%f")
                .replace("%1", "%f")
                .replace("\"%*\"", "%F")
                .replace("%*", "%F"),
            _ => raw.to_string(),
        }
    }

    /// The magic bytes, decoded. `Ok(None)` when no magic is declared.
    pub fn magic_bytes(&self) -> Result<Option<Vec<u8>>> {
        match &self.format.magic {
            Some(m) => crate::magic::decode(m).map(Some),
            None => Ok(None),
        }
    }

    /// A minimal, valid spec — the shape `filekind init` scaffolds from.
    pub fn scaffold(name: &str, extension: &str) -> Spec {
        let ext = extension.trim_start_matches('.').to_ascii_lowercase();
        let vendor = {
            let mut v: String = name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect();
            if v.is_empty() {
                v = "example".into();
            }
            v
        };
        Spec {
            format: Format {
                name: name.to_string(),
                extension: ext.clone(),
                description: format!("{name} file"),
                magic: None,
                container: Container::None,
                version: Some("1.0".to_string()),
            },
            association: Association {
                mime: format!("application/vnd.{vendor}.{ext}"),
                icon: None,
                handler: None,
                handler_args: default_handler_args(),
            },
            targets: Targets::default(),
            windows: WindowsOpts::default(),
            linux: LinuxOpts::default(),
            macos: MacosOpts::default(),
            schema: None,
        }
    }
}

fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                out.extend(ch.to_uppercase());
                upper_next = false;
            } else {
                out.push(ch);
            }
        } else {
            upper_next = true;
        }
    }
    if out.is_empty() {
        "Filekind".to_string()
    } else {
        out
    }
}

// Format-preserving round-trip

/// A spec file kept as an editable document rather than a parsed struct.
///
/// The GUI opens real files that people have hand-edited and commented. Writing
/// them back through `toml::to_string` would silently reorder keys and delete
/// every comment, which is the difference between an editor and a wizard that
/// overwrites your work. [`SpecDocument::apply`] mutates values in place and
/// leaves everything else — comments, ordering, whitespace, inline tables —
/// exactly as the user wrote it.
#[derive(Debug, Clone)]
pub struct SpecDocument {
    doc: toml_edit::DocumentMut,
}

impl SpecDocument {
    pub fn parse(text: &str) -> Result<Self> {
        let doc: toml_edit::DocumentMut = text.parse()?;
        Ok(SpecDocument { doc })
    }

    /// Build a document from scratch, with the section comments `init` emits.
    pub fn from_spec(spec: &Spec) -> Result<Self> {
        let text = spec.to_toml_string()?;
        Self::parse(&text)
    }

    /// The strongly-typed view.
    pub fn spec(&self) -> Result<Spec> {
        Spec::from_toml_str(&self.doc.to_string())
    }

    /// Write the desired state into the document, touching only what changed.
    pub fn apply(&mut self, spec: &Spec) -> Result<()> {
        use toml_edit::{value, Array, Item, Table};

        fn table<'a>(doc: &'a mut toml_edit::DocumentMut, key: &str) -> &'a mut Table {
            if !doc.contains_key(key) {
                let mut t = Table::new();
                t.set_implicit(false);
                doc.insert(key, Item::Table(t));
            }
            doc[key].as_table_mut().expect("inserted as table")
        }

        fn set_opt(t: &mut Table, key: &str, v: Option<&str>) {
            match v {
                Some(s) => t[key] = value(s),
                None => {
                    t.remove(key);
                }
            }
        }

        fn set_arr(t: &mut Table, key: &str, items: &[String], drop_if_empty: bool) {
            if items.is_empty() && drop_if_empty {
                t.remove(key);
                return;
            }
            let mut arr = Array::new();
            for i in items {
                arr.push(i.as_str());
            }
            t[key] = value(arr);
        }

        {
            let t = table(&mut self.doc, "format");
            t["name"] = value(&spec.format.name);
            t["extension"] = value(&spec.format.extension);
            t["description"] = value(&spec.format.description);
            set_opt(t, "magic", spec.format.magic.as_deref());
            t["container"] = value(spec.format.container.as_str());
            set_opt(t, "version", spec.format.version.as_deref());
        }
        {
            let t = table(&mut self.doc, "association");
            t["mime"] = value(&spec.association.mime);
            set_opt(t, "icon", spec.association.icon.as_deref());
            set_opt(t, "handler", spec.association.handler.as_deref());
            t["handler_args"] = value(&spec.association.handler_args);
        }
        {
            let t = table(&mut self.doc, "targets");
            t["windows"] = value(spec.targets.windows);
            t["linux"] = value(spec.targets.linux);
            t["macos"] = value(spec.targets.macos);
        }
        {
            let t = table(&mut self.doc, "windows");
            set_opt(t, "progid", spec.windows.progid.as_deref());
            set_opt(
                t,
                "perceived_type",
                spec.windows.perceived_type.map(|p| p.as_str()),
            );
            set_opt(t, "icon_path", spec.windows.icon_path.as_deref());
        }
        {
            let t = table(&mut self.doc, "linux");
            set_arr(
                t,
                "desktop_categories",
                &spec.linux.desktop_categories,
                false,
            );
            set_opt(t, "generic_name", spec.linux.generic_name.as_deref());
        }
        {
            let t = table(&mut self.doc, "macos");
            set_opt(t, "bundle_id", spec.macos.bundle_id.as_deref());
            set_arr(t, "uti_conforms_to", &spec.macos.uti_conforms_to, true);
            set_opt(t, "uti", spec.macos.uti.as_deref());
        }
        Ok(())
    }

    /// The document as text, ready to write to disk. Byte-identical to the
    /// input when nothing changed.
    pub fn to_toml_string(&self) -> String {
        self.doc.to_string()
    }
}

impl fmt::Display for SpecDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.doc.to_string())
    }
}
