//! Error type for the core library.
//!
//! Core uses `thiserror`; the CLI layers `anyhow` on top for context. Nothing
//! in here formats for a terminal — the GUI shows these strings too.

use std::fmt;

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The spec file was not valid TOML, or did not match the schema.
    #[error("could not parse spec: {0}")]
    Parse(#[from] toml::de::Error),

    /// The spec parsed but broke one or more validation rules.
    #[error("spec is invalid ({} error{})", .0.len(), if .0.len() == 1 { "" } else { "s" })]
    Invalid(Vec<crate::validate::Diagnostic>),

    /// A magic-byte string used an escape we do not understand, or was too long.
    #[error("invalid magic string: {0}")]
    Magic(String),

    /// The icon source could not be decoded, or was unusable.
    #[error("icon pipeline failed: {0}")]
    Icon(String),

    /// A template failed to render. Always a filekind bug, never user input.
    #[error("template error (this is a filekind bug, please report it): {0}")]
    Template(#[from] minijinja::Error),

    /// Serialising a spec back to TOML failed.
    #[error("could not serialise spec: {0}")]
    Serialize(#[from] toml::ser::Error),

    /// Format-preserving edit of an existing spec failed.
    #[error("could not edit spec in place: {0}")]
    Edit(#[from] toml_edit::TomlError),

    /// A `.ksy` schema could not be understood.
    #[error("kaitai schema error: {0}")]
    Kaitai(String),
}

impl Error {
    /// The process exit code the CLI should use for this error.
    ///
    /// Core does not exit — it just knows which bucket a failure belongs to.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Parse(_) | Error::Invalid(_) | Error::Magic(_) | Error::Kaitai(_) => 1,
            Error::Icon(_) | Error::Template(_) | Error::Serialize(_) | Error::Edit(_) => 3,
        }
    }

    /// Diagnostics attached to this error, if it is a validation failure.
    pub fn diagnostics(&self) -> &[crate::validate::Diagnostic] {
        match self {
            Error::Invalid(d) => d,
            _ => &[],
        }
    }
}

/// Wrapper that renders a validation error with all of its diagnostics,
/// one per line. `Display` on `Error` stays short so it composes with
/// `anyhow`'s chain formatting.
#[derive(Debug)]
pub struct Verbose<'a>(pub &'a Error);

impl fmt::Display for Verbose<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        for d in self.0.diagnostics() {
            write!(f, "\n  {d}")?;
        }
        Ok(())
    }
}
