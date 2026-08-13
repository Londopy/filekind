//! # filekind-core
//!
//! Turns one declarative [`Spec`] into the pile of unrelated artifacts an
//! operating system actually wants before it will admit your file extension
//! exists: a Windows registry tree, a freedesktop shared-mime-info XML plus
//! `.desktop` entry, a macOS UTI declaration, a libmagic pattern, and icons in
//! three container formats.
//!
//! ## Core invariant
//!
//! **This crate performs no I/O and no printing.** Every generator returns
//! `String` or `Vec<u8>` wrapped in an [`Artifact`]. The caller decides where
//! bytes land — the CLI writes them to disk, the GUI renders them into a live
//! preview pane.
//!
//! This is not fastidiousness for its own sake. Live preview requires
//! generating artifacts in memory on every keystroke. The moment this crate
//! calls `fs::write` or `println!`, the preview pane becomes impossible and the
//! GUI degrades into a form that shells out to a CLI.
//!
//! Two consequences the API makes explicit:
//!
//! - Icons are passed in as bytes ([`IconSet::from_png_bytes`]), not paths.
//! - Kaitai schemas are passed in as text ([`kaitai::plan`]), not paths.
//!
//! ## Example
//!
//! ```
//! use filekind_core::{Spec, GenerateOptions, generate};
//!
//! let spec = Spec::from_toml_str(r#"
//! [format]
//! name = "Londo Save"
//! extension = "londo"
//! description = "Londo project save file"
//!
//! [association]
//! mime = "application/vnd.london.londo"
//! handler = "londo-player"
//! "#).unwrap();
//!
//! let artifacts = generate(&spec, &GenerateOptions::default()).unwrap();
//! assert!(artifacts.iter().any(|a| a.path.ends_with("register.reg")));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod artifact;
pub mod error;
pub mod generate;
pub mod icon;
pub mod magic;
pub mod spec;
pub mod validate;
pub mod verify;

#[cfg(feature = "kaitai")]
pub mod kaitai;

pub use artifact::{Artifact, ArtifactBody, ArtifactKind, Platform};
pub use error::{Error, Result};
pub use generate::{find, generate, generate_for, GenerateOptions, Scope};
pub use icon::{IconSet, IconWarning};
pub use spec::{
    Association, Container, Format, LinuxOpts, MacosOpts, PerceivedType, Spec, Targets, WindowsOpts,
};
pub use validate::{validate, Diagnostic, Severity};
pub use verify::{verify_bytes, VerifyReport};

/// The version of filekind that produced a given artifact. Stamped into
/// generated file headers so a stale `.reg` on someone's desktop is traceable.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
