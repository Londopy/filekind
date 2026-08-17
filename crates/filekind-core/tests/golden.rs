//! Golden-file tests (v0.2).
//!
//! Registry syntax, freedesktop XML and plist structure are the sort of thing
//! that is easy to break by accident and hard to notice: a stray quote in a
//! `.reg` does not fail a build, it just silently registers the wrong string.
//! Snapshotting the exact bytes turns "did I change the output?" into a diff.
//!
//! Review changes with `cargo insta review`.

use filekind_core::{generate, ArtifactKind, GenerateOptions, IconSet, Platform, Scope, Spec};

/// The worked example from the design notes, with `magic` written the way TOML
/// actually accepts it — as a literal string. See docs/spec-notes.md.
const LONDO: &str = r#"
[format]
name        = "Londo Save"
extension   = "londo"
description = "Londo project save file"
magic       = 'LNDO\x01'
container   = "binary"
version     = "1.0"

[association]
mime         = "application/vnd.london.londo"
icon         = "assets/icon.png"
handler      = "londo-player"
handler_args = '"%1"'

[targets]
windows = true
linux   = true
macos   = true

[windows]
progid         = "London.LondoSave.1"
perceived_type = "document"

[linux]
desktop_categories = ["Utility"]
generic_name       = "Save File"

[macos]
bundle_id       = "wtf.londo.player"
uti_conforms_to = ["public.data", "public.archive"]
"#;

/// A zip container with no handler and no magic — the other end of the space.
const ZIPPED: &str = r#"
[format]
name        = "Studio Project"
extension   = "studioproj"
description = "Studio project bundle"
container   = "zip"

[association]
mime = "application/vnd.example.studioproj"

[targets]
windows = true
linux   = true
macos   = false

[windows]
perceived_type = "compressed"
"#;

fn spec(src: &str) -> Spec {
    Spec::from_toml_str(src).expect("test spec parses")
}

/// A tiny valid PNG so the icon-bearing artifacts appear in the snapshots.
fn test_icon() -> IconSet {
    let mut img = image::RgbaImage::new(512, 512);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([
            (x / 2) as u8,
            (y / 2) as u8,
            128,
            if x < 32 { 0 } else { 255 },
        ]);
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    IconSet::from_png_bytes(&png).unwrap()
}

fn opts() -> GenerateOptions {
    GenerateOptions {
        icon: Some(test_icon()),
        packaging: true,
        stub_app: true,
        ..GenerateOptions::default()
    }
}

/// Snapshot every text artifact of a spec under one name.
///
/// The crate version is stamped into every generated banner, so without a
/// filter a routine version bump rewrites all fifty-odd snapshots and buries
/// the one real change in the diff. Redact it: these tests are about the
/// artifacts, and there is a separate assertion that the stamp exists at all.
fn snapshot_all(name: &str, src: &str, options: GenerateOptions) {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r"filekind \d+\.\d+\.\d+", "filekind [VERSION]");
    let _guard = settings.bind_to_scope();

    let s = spec(src);
    let artifacts = generate(&s, &options).expect("generation succeeds");

    let mut manifest = String::new();
    for a in &artifacts {
        manifest.push_str(&format!(
            "{:<10} {:<14} {}{}\n",
            a.platform.to_string(),
            a.kind.to_string(),
            a.path,
            if a.executable { "  [+x]" } else { "" }
        ));
    }
    insta::assert_snapshot!(format!("{name}__manifest"), manifest);

    for a in &artifacts {
        let Some(text) = a.as_text() else { continue };
        let key = a.path.replace(['/', '.', ' '], "_");
        insta::assert_snapshot!(format!("{name}__{key}"), text);
    }
}

#[test]
fn londo_user_scope() {
    snapshot_all("londo", LONDO, opts());
}

#[test]
fn londo_system_scope() {
    // --system flips the hive, the xdg mode and the install prefix. Three
    // independent substitutions across five files is exactly the kind of thing
    // that rots quietly.
    let mut o = opts();
    o.scope = Scope::System;
    o.stub_app = false;
    snapshot_all("londo_system", LONDO, o);
}

#[test]
fn zip_container_without_handler_or_icon() {
    snapshot_all(
        "zipped",
        ZIPPED,
        GenerateOptions {
            packaging: true,
            ..Default::default()
        },
    )
}

// Structural properties the snapshots alone would not catch

#[test]
fn every_register_artifact_has_a_matching_unregister() {
    // Mandatory, so assert it rather than trusting the templates to stay right.
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let has = |k: ArtifactKind| artifacts.iter().any(|a| a.kind == k);
    assert!(has(ArtifactKind::Reg) && has(ArtifactKind::RegUninstall));

    let install = artifacts
        .iter()
        .find(|a| a.path.ends_with("install.sh"))
        .unwrap();
    let uninstall = artifacts
        .iter()
        .find(|a| a.path.ends_with("uninstall.sh"))
        .unwrap();
    assert!(install.executable && uninstall.executable);

    let nsis = artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::Nsis)
        .unwrap();
    let nsis = nsis.as_text().unwrap();
    assert!(nsis.contains("!macro FilekindRegister"));
    assert!(nsis.contains("!macro FilekindUnregister"));
}

#[test]
fn nothing_generated_tries_to_set_itself_as_the_windows_default() {
    // filekind registers and prompts, nothing more. If a future template reaches
    // for UserChoice or FileExts, this test is the tripwire.
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    for a in artifacts.iter().filter(|a| a.platform == Platform::Windows) {
        let Some(text) = a.as_text() else { continue };
        for forbidden in ["UserChoice", "FileExts", "ProgId\"=\"", "SetUserFTA"] {
            assert!(
                !text.contains(forbidden),
                "{} contains `{forbidden}` — that path is default-handler hijacking",
                a.path
            );
        }
    }
}

#[test]
fn a_hostile_handler_cannot_escape_any_quoting() {
    // The handler is declared in the spec and never read from a data file, but a
    // spec can still be hostile. Every generator must neutralise it.
    let mut s = spec(LONDO);
    s.association.handler = Some(r#"evil" & calc.exe & "#.to_string());
    s.format.name = "Bad\" Name".to_string();
    s.format.description = "</comment><script>".to_string();

    let artifacts = generate(&s, &opts()).unwrap();

    for a in &artifacts {
        let Some(text) = a.as_text() else { continue };
        match a.kind {
            ArtifactKind::MimeXml | ArtifactKind::Plist => {
                assert!(!text.contains("<script>"), "{} leaked raw markup", a.path);
                assert!(
                    text.contains("&lt;/comment&gt;"),
                    "{} did not escape",
                    a.path
                );
            }
            ArtifactKind::Reg | ArtifactKind::RegUninstall => {
                // Every quote inside a .reg value must be backslash-escaped;
                // an unescaped one ends the string early.
                for line in text.lines().filter(|l| l.contains("=\"")) {
                    let value = line.split_once("=\"").unwrap().1;
                    let body = value.strip_suffix('"').unwrap_or(value);
                    let mut chars = body.chars().peekable();
                    while let Some(c) = chars.next() {
                        if c == '\\' {
                            chars.next();
                        } else {
                            assert!(c != '"', "unescaped quote in {}: {line}", a.path);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn xml_artifacts_are_well_formed_enough_to_have_balanced_tags() {
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let xml = artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::MimeXml)
        .unwrap();
    let text = xml.as_text().unwrap();
    assert_eq!(
        text.matches("<mime-type").count(),
        text.matches("</mime-type>").count()
    );
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(
        text.contains(r#"value="LNDO\x01""#),
        "magic must use the freedesktop dialect"
    );
}

#[test]
fn the_libmagic_pattern_uses_octal_not_hex() {
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let magic = artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::Magic)
        .unwrap();
    let text = magic.as_text().unwrap();
    assert!(
        text.contains("LNDO\\001"),
        "libmagic wants C octal escapes:\n{text}"
    );
    assert!(text.contains("!:mime\tapplication/vnd.london.londo"));
}

#[test]
fn disabled_targets_produce_no_artifacts_for_that_platform() {
    let mut s = spec(LONDO);
    s.targets.macos = false;
    s.targets.windows = false;
    let artifacts = generate(&s, &opts()).unwrap();
    assert!(artifacts.iter().all(|a| a.platform != Platform::Macos));
    assert!(artifacts.iter().all(|a| a.platform != Platform::Windows));
    // Universal artifacts are not gated on a target.
    assert!(artifacts.iter().any(|a| a.kind == ArtifactKind::Readme));
}

#[test]
fn generation_is_deterministic() {
    // Golden files are worthless if the output depends on hash ordering.
    let s = spec(LONDO);
    let a = generate(&s, &opts()).unwrap();
    let b = generate(&s, &opts()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn artifact_paths_are_unique_and_relative() {
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for a in &artifacts {
        assert!(
            seen.insert(a.path.clone()),
            "duplicate output path {}",
            a.path
        );
        assert!(!a.path.starts_with('/'), "absolute path {}", a.path);
        assert!(!a.path.contains(".."), "path escape in {}", a.path);
        assert!(
            !a.path.contains('\\'),
            "backslash in {} — paths are forward-slashed",
            a.path
        );
    }
}

#[test]
fn artifacts_survive_a_json_round_trip() {
    // The GUI receives these over Tauri IPC; binary bodies must not become
    // arrays of integers.
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let json = serde_json::to_string(&artifacts).unwrap();
    let icon_json =
        serde_json::to_value(artifacts.iter().find(|a| a.path.ends_with(".ico")).unwrap()).unwrap();
    assert_eq!(icon_json["body"]["encoding"], "binary");
    assert!(icon_json["body"]["base64"].is_string());

    let back: Vec<filekind_core::Artifact> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, artifacts);
}

#[test]
fn generated_files_are_stamped_with_the_version() {
    // The golden files redact the version, so assert it directly: a generated
    // file found on someone's desktop two years from now should say what
    // produced it.
    //
    // Two artifacts legitimately cannot carry the banner. A `.desktop` entry
    // is a keyfile whose parsers vary in how well they tolerate leading
    // comments, and `PkgInfo` is eight fixed bytes with no comment syntax at
    // all. Naming them here means adding a third silently is a test failure.
    let artifacts = generate(&spec(LONDO), &opts()).unwrap();
    let stamp = format!("filekind {}", filekind_core::VERSION);

    let unstamped: Vec<&str> = artifacts
        .iter()
        .filter(|a| a.as_text().is_some_and(|t| !t.contains(&stamp)))
        .map(|a| a.path.as_str())
        .collect();

    assert_eq!(
        unstamped,
        [
            "linux/londo-player.desktop",
            "macos/Londo Save.app/Contents/PkgInfo"
        ],
        "the set of files without a version stamp changed"
    );
}
