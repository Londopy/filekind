//! Mirrors every call `filekind-gui` makes into this crate.
//!
//! The GUI is the slowest thing in CI to compile — a Tauri build pulls three
//! hundred crates and takes minutes — and it is the only consumer of parts of
//! this API. A missing re-export in `lib.rs` once turned a one-word omission
//! into a four-minute failure at the very end of the pipeline.
//!
//! This file does the same sequence of calls, with the same types, in a test
//! that runs in under a second. It cannot catch Tauri-specific mistakes, but
//! it catches every signature change the GUI would trip over.
//!
//! If you change something here to make it compile, change the GUI too.

use filekind_core::{
    generate, generate_for, validate, Artifact, ArtifactBody, Diagnostic, Error, GenerateOptions,
    IconSet, Platform, Scope, Spec, SpecDocument,
};

const SRC: &str = r#"
[format]
name = "Londo Save"
extension = "londo"
description = "Londo project save file"
magic = 'LNDO\x01'

[association]
mime = "application/vnd.london.londo"
handler = "londo-player"
"#;

fn png() -> Vec<u8> {
    let mut img = image::RgbaImage::new(512, 512);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x / 2) as u8, (y / 2) as u8, 90, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// `parse_spec`, `load_spec` and `scaffold_spec` all build the same struct.
#[test]
fn spec_file_commands() {
    let spec: Spec = Spec::from_toml_str(SRC).unwrap();
    let diagnostics: Vec<Diagnostic> = validate(&spec);
    assert!(diagnostics.iter().all(|d| !d.is_error()));

    let scaffolded: Spec = Spec::scaffold("My Format", "myfmt");
    let text: String = scaffolded.to_toml_string().unwrap();
    assert!(text.contains("myfmt"));
}

/// `save_spec` — the format-preserving write path.
#[test]
fn save_spec_command() {
    let mut doc: SpecDocument = SpecDocument::parse(SRC).unwrap();
    let spec: Spec = doc.spec().unwrap();
    doc.apply(&spec).unwrap();
    let _: String = doc.to_toml_string();

    // The fallback when there is no file on disk yet.
    let _: SpecDocument = SpecDocument::from_spec(&spec).unwrap();
}

/// `preview_artifacts` and `build_artifacts`.
#[test]
fn generate_commands() {
    let spec = Spec::from_toml_str(SRC).unwrap();
    let icon: Option<IconSet> = Some(IconSet::from_png_bytes(&png()).unwrap());

    let opts = GenerateOptions {
        scope: Scope::System,
        icon,
        packaging: true,
        stub_app: false,
        kaitai_schema: None,
    };

    let all: Vec<Artifact> = generate(&spec, &opts).unwrap();
    let platform: Platform = "windows".parse().unwrap();
    let one: Vec<Artifact> = generate_for(&spec, &opts, platform).unwrap();
    assert!(!all.is_empty() && !one.is_empty());

    // ArtifactView is built from each of these.
    for a in &all {
        let _: &str = &a.path;
        let _: String = a.platform.to_string();
        let _: String = a.kind.to_string();
        let _: &str = &a.description;
        let _: usize = a.len();
        let _: bool = a.executable;
        let _: &[u8] = a.bytes();
        match a.as_text() {
            Some(t) => assert!(!t.is_empty()),
            None => {
                let b64: String = a.body.to_base64();
                assert!(!b64.is_empty());
            }
        }
    }

    // The error path: validation failure carries its diagnostics across IPC.
    let mut bad = spec.clone();
    bad.format.extension = "exe".into();
    let diagnostics = validate(&bad);
    let err = Error::Invalid(diagnostics);
    assert!(!err.diagnostics().is_empty());
}

/// `render_icons`.
#[test]
fn icon_command() {
    let set = IconSet::from_png_bytes(&png()).unwrap();
    let (w, h): (u32, u32) = set.source_size;
    assert_eq!((w, h), (512, 512));
    let _: Vec<String> = set.warnings.iter().map(|w| w.to_string()).collect();
    let _: usize = set.ico.len();
    let _: usize = set.icns.len();
    for (size, bytes) in &set.hicolor {
        let _: u32 = *size;
        let _: &Vec<u8> = bytes;
    }
}

/// `derived_values`.
#[test]
fn derived_values_command() {
    let spec = Spec::from_toml_str(SRC).unwrap();
    let _: String = spec.progid();
    let _: String = spec.uti();
    let _: Vec<String> = spec.uti_conforms_to();
    let _: String = spec.mime_icon_name();
    let _: String = spec.desktop_id();
    let _: String = spec.slug();
    let _: String = spec.dotted_extension();

    let hex: Option<String> = spec
        .magic_bytes()
        .ok()
        .flatten()
        .map(|b| filekind_core::magic::to_hex_dump(&b));
    assert_eq!(hex.as_deref(), Some("4C 4E 44 4F 01"));
}

/// `read_icon` / `read_schema` hand core an `Option`, and the spec fields they
/// read must stay optional strings.
#[test]
fn optional_paths_the_gui_resolves_itself() {
    let spec = Spec::from_toml_str(SRC).unwrap();
    let _: Option<&str> = spec.association.icon.as_deref();
    let _: Option<&str> = spec.schema.as_ref().and_then(|s| s.kaitai.as_deref());
}

/// `CommandError` converts from both of these.
#[test]
fn error_conversions() {
    let e: Error = Spec::from_toml_str("[format").unwrap_err();
    let _: String = e.to_string();
    let _: i32 = e.exit_code();

    // ArtifactBody is matched on directly by ArtifactView.
    let body = ArtifactBody::Text { text: "x".into() };
    assert_eq!(body.as_bytes(), b"x");
}
