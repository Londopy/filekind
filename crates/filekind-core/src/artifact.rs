//! What a generator returns. Never a path on disk — always bytes plus enough
//! metadata for the caller to decide what to do with them.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::fmt;

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Path relative to the output directory, using forward slashes on every
    /// platform. The caller joins it onto a real directory.
    pub path: String,
    pub body: ArtifactBody,
    pub platform: Platform,
    pub kind: ArtifactKind,
    /// Whether the caller should set the executable bit (`install.sh`).
    pub executable: bool,
    /// One line explaining what this file is for, shown in the GUI's artifact
    /// list and the CLI's build summary.
    pub description: String,
}

impl Artifact {
    pub fn text(
        path: impl Into<String>,
        contents: impl Into<String>,
        platform: Platform,
        kind: ArtifactKind,
        description: impl Into<String>,
    ) -> Self {
        Artifact {
            path: path.into(),
            body: ArtifactBody::Text {
                text: contents.into(),
            },
            platform,
            kind,
            executable: false,
            description: description.into(),
        }
    }

    pub fn binary(
        path: impl Into<String>,
        contents: Vec<u8>,
        platform: Platform,
        kind: ArtifactKind,
        description: impl Into<String>,
    ) -> Self {
        Artifact {
            path: path.into(),
            body: ArtifactBody::Binary { bytes: contents },
            platform,
            kind,
            executable: false,
            description: description.into(),
        }
    }

    /// Mark this artifact as needing the executable bit. Chainable.
    pub fn exec(mut self) -> Self {
        self.executable = true;
        self
    }

    /// Raw bytes, whichever variant the body is.
    pub fn bytes(&self) -> &[u8] {
        self.body.as_bytes()
    }

    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Text content if this is a text artifact — what the GUI preview pane
    /// renders. Binary artifacts return `None` and get a hex/thumbnail view.
    pub fn as_text(&self) -> Option<&str> {
        match &self.body {
            ArtifactBody::Text { text } => Some(text),
            ArtifactBody::Binary { .. } => None,
        }
    }
}

/// Text or bytes.
///
/// Serialises as an internally-tagged object with binary payloads base64
/// encoded:
///
/// ```json
/// { "encoding": "text",   "text":   "Windows Registry Editor…" }
/// { "encoding": "binary", "base64": "AAABAAYAEBA…" }
/// ```
///
/// Struct variants rather than newtypes, because serde cannot internally tag
/// a newtype variant that wraps a primitive — and base64 rather than the
/// default `Vec<u8>` representation, because a 256×256 icon would otherwise
/// cross the Tauri IPC boundary as ~700 KB of decimal digits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "lowercase")]
pub enum ArtifactBody {
    Text {
        text: String,
    },
    Binary {
        #[serde(rename = "base64", with = "b64")]
        bytes: Vec<u8>,
    },
}

mod b64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}

impl ArtifactBody {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ArtifactBody::Text { text } => text.as_bytes(),
            ArtifactBody::Binary { bytes } => bytes,
        }
    }

    /// Base64 of the payload — for `<img src="data:image/png;base64,…">` in
    /// the GUI's icon preview.
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.as_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
    Macos,
    /// Not tied to one OS: the libmagic pattern, the generated README, icons.
    Universal,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Macos => "macos",
            Platform::Universal => "universal",
        }
    }

    /// Output subdirectory for this platform.
    pub fn dir(self) -> &'static str {
        self.as_str()
    }

    pub const ALL: [Platform; 4] = [
        Platform::Windows,
        Platform::Linux,
        Platform::Macos,
        Platform::Universal,
    ];
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Platform {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "windows" | "win" => Ok(Platform::Windows),
            "linux" => Ok(Platform::Linux),
            "macos" | "mac" | "osx" | "darwin" => Ok(Platform::Macos),
            "universal" | "all" => Ok(Platform::Universal),
            other => Err(format!("unknown platform `{other}`")),
        }
    }
}

/// What sort of artifact this is. Lets the CLI's `--kind` filter and the GUI's
/// preview tabs talk about files without matching on filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    /// `register.reg`
    Reg,
    /// `unregister.reg`
    RegUninstall,
    /// Inno Setup `[Registry]` snippet
    Inno,
    /// NSIS `.nsh` snippet
    Nsis,
    /// freedesktop shared-mime-info XML
    MimeXml,
    /// `.desktop` entry
    Desktop,
    /// macOS `Info.plist` fragment
    Plist,
    /// libmagic pattern
    Magic,
    /// Install / uninstall shell script
    Script,
    /// An icon in any container format
    Icon,
    /// Debian packaging
    Deb,
    /// RPM spec file
    Rpm,
    /// Generated install instructions
    Readme,
    /// Kaitai build glue (v0.5)
    Kaitai,
    /// Bundle scaffolding that is neither a plist nor a script (`PkgInfo`)
    Bundle,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Reg => "reg",
            ArtifactKind::RegUninstall => "reg-uninstall",
            ArtifactKind::Inno => "inno",
            ArtifactKind::Nsis => "nsis",
            ArtifactKind::MimeXml => "mime-xml",
            ArtifactKind::Desktop => "desktop",
            ArtifactKind::Plist => "plist",
            ArtifactKind::Magic => "magic",
            ArtifactKind::Script => "script",
            ArtifactKind::Icon => "icon",
            ArtifactKind::Deb => "deb",
            ArtifactKind::Rpm => "rpm",
            ArtifactKind::Readme => "readme",
            ArtifactKind::Kaitai => "kaitai",
            ArtifactKind::Bundle => "bundle",
        }
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ArtifactKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let k = s.to_ascii_lowercase();
        for cand in [
            ArtifactKind::Reg,
            ArtifactKind::RegUninstall,
            ArtifactKind::Inno,
            ArtifactKind::Nsis,
            ArtifactKind::MimeXml,
            ArtifactKind::Desktop,
            ArtifactKind::Plist,
            ArtifactKind::Magic,
            ArtifactKind::Script,
            ArtifactKind::Icon,
            ArtifactKind::Deb,
            ArtifactKind::Rpm,
            ArtifactKind::Readme,
            ArtifactKind::Kaitai,
            ArtifactKind::Bundle,
        ] {
            if cand.as_str() == k {
                return Ok(cand);
            }
        }
        Err(format!("unknown artifact kind `{s}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_bodies_serialise_as_readable_json() {
        let a = Artifact::text("a.reg", "hello", Platform::Windows, ArtifactKind::Reg, "d");
        let v: serde_json::Value = serde_json::to_value(&a).unwrap();
        assert_eq!(v["body"]["encoding"], "text");
        assert_eq!(v["body"]["text"], "hello");
    }

    #[test]
    fn binary_bodies_serialise_as_base64_not_an_integer_array() {
        let a = Artifact::binary(
            "icon.ico",
            vec![0, 0, 1, 0],
            Platform::Windows,
            ArtifactKind::Icon,
            "d",
        );
        let v: serde_json::Value = serde_json::to_value(&a).unwrap();
        assert_eq!(v["body"]["encoding"], "binary");
        assert_eq!(v["body"]["base64"], "AAABAA==");
        assert!(!v["body"]["base64"].is_array());
    }

    #[test]
    fn both_variants_round_trip() {
        for a in [
            Artifact::text("a", "x", Platform::Linux, ArtifactKind::Desktop, "d"),
            Artifact::binary("b", vec![1, 2, 3], Platform::Linux, ArtifactKind::Icon, "d"),
        ] {
            let json = serde_json::to_string(&a).unwrap();
            let back: Artifact = serde_json::from_str(&json).unwrap();
            assert_eq!(a, back);
        }
    }

    #[test]
    fn platform_and_kind_parse_back_from_their_display_form() {
        for p in Platform::ALL {
            assert_eq!(p.as_str().parse::<Platform>().unwrap(), p);
        }
        for k in [
            ArtifactKind::Reg,
            ArtifactKind::RegUninstall,
            ArtifactKind::MimeXml,
        ] {
            assert_eq!(k.as_str().parse::<ArtifactKind>().unwrap(), k);
        }
    }
}
