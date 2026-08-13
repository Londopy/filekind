//! The Tauri side of the filekind desktop app.
//!
//! This crate is a thin shell: it owns the window, the file dialogs, and the
//! filesystem. Every artifact it shows is produced by `filekind-core`, linked
//! directly — **the GUI never shells out to `filekind-cli`.**
//!
//! That matters for one concrete reason. The signature feature is a preview
//! pane that re-renders the actual `.reg` / `.xml` / `.plist` as you type. At a
//! 150 ms debounce that is several generations a second; a subprocess per
//! keystroke would make it feel like a build system instead of an editor. Core
//! does no I/O precisely so this can be an in-memory function call.
//!
//! Types are shared rather than mirrored: commands take and return
//! `filekind_core::Spec` and friends as-is. TypeScript definitions are
//! generated from the Rust with `cargo test -p filekind-core --features
//! typescript`, not maintained by hand.

use std::path::{Path, PathBuf};

use filekind_core::{
    generate, generate_for, Artifact, Diagnostic, GenerateOptions, IconSet, Platform, Scope, Spec,
    SpecDocument,
};
use serde::{Deserialize, Serialize};

/// Anything a command can fail with, in a shape the frontend can render.
///
/// Tauri requires command errors to be serialisable. Rather than stringify and
/// lose the structure, validation failures keep their diagnostics so the UI can
/// underline the offending field.
#[derive(Debug, Serialize)]
pub struct CommandError {
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<Diagnostic>,
}

impl From<filekind_core::Error> for CommandError {
    fn from(e: filekind_core::Error) -> Self {
        CommandError {
            message: e.to_string(),
            diagnostics: e.diagnostics().to_vec(),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError {
            message: e.to_string(),
            diagnostics: Vec::new(),
        }
    }
}

impl CommandError {
    fn msg(m: impl Into<String>) -> Self {
        CommandError {
            message: m.into(),
            diagnostics: Vec::new(),
        }
    }
}

type CmdResult<T> = Result<T, CommandError>;

/// A spec as the editor holds it: the typed view, the raw text, and where it
/// came from.
#[derive(Debug, Serialize, Deserialize)]
pub struct SpecFile {
    /// `None` for an unsaved scaffold.
    pub path: Option<String>,
    /// The document text, comments and all.
    pub text: String,
    pub spec: Spec,
    pub diagnostics: Vec<Diagnostic>,
}

/// An artifact prepared for display. Text is inlined; binaries come across as
/// base64 so a 256px icon does not arrive as an array of 260 000 integers.
#[derive(Debug, Serialize)]
pub struct ArtifactView {
    pub path: String,
    pub platform: String,
    pub kind: String,
    pub description: String,
    pub size: usize,
    pub text: Option<String>,
    pub base64: Option<String>,
    pub executable: bool,
}

impl From<&Artifact> for ArtifactView {
    fn from(a: &Artifact) -> Self {
        let (text, base64) = match a.as_text() {
            Some(t) => (Some(t.to_string()), None),
            None => (None, Some(a.body.to_base64())),
        };
        ArtifactView {
            path: a.path.clone(),
            platform: a.platform.to_string(),
            kind: a.kind.to_string(),
            description: a.description.clone(),
            size: a.len(),
            text,
            base64,
            executable: a.executable,
        }
    }
}

/// What the Build screen's controls map to.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOptions {
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub packaging: bool,
    #[serde(default)]
    pub stub_app: bool,
    /// Directory the spec's relative paths resolve against. The frontend never
    /// touches the filesystem itself; it just remembers where the spec lives.
    #[serde(default)]
    pub base_dir: Option<String>,
}

impl BuildOptions {
    fn into_core(self, icon: Option<IconSet>, kaitai_schema: Option<String>) -> GenerateOptions {
        GenerateOptions {
            scope: if self.system {
                Scope::System
            } else {
                Scope::User
            },
            icon,
            packaging: self.packaging,
            stub_app: self.stub_app,
            kaitai_schema,
        }
    }
}

/// Rendered icons, ready for `<img src="data:image/png;base64,…">`.
#[derive(Debug, Serialize)]
pub struct IconPreview {
    pub source_width: u32,
    pub source_height: u32,
    pub warnings: Vec<String>,
    /// `(size, base64 png)` for every hicolor size.
    pub previews: Vec<(u32, String)>,
    pub ico_bytes: usize,
    pub icns_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct BuildResult {
    pub output_dir: String,
    pub files: Vec<String>,
    pub total_bytes: usize,
}

/// Parse text into a spec, without touching disk.
///
/// Called on every keystroke behind the frontend's debounce, so it must stay
/// cheap and must never throw away the diagnostics — a half-typed spec is the
/// normal case here, not an error case.
#[tauri::command]
fn parse_spec(text: String) -> CmdResult<SpecFile> {
    let spec = Spec::from_toml_str(&text)?;
    let diagnostics = filekind_core::validate(&spec);
    Ok(SpecFile {
        path: None,
        text,
        spec,
        diagnostics,
    })
}

/// Open a `.filekind` file.
#[tauri::command]
fn load_spec(path: String) -> CmdResult<SpecFile> {
    let text = std::fs::read_to_string(&path)?;
    let spec = Spec::from_toml_str(&text)?;
    let diagnostics = filekind_core::validate(&spec);
    Ok(SpecFile {
        path: Some(path),
        text,
        spec,
        diagnostics,
    })
}

/// Write a spec back to disk **without destroying the file**.
///
/// Round-tripping is a requirement, not a nicety. The typed edits are applied onto the
/// existing document with `toml_edit`, so comments, key order and formatting
/// survive. A GUI that can only create, never edit, is a wizard pretending to
/// be an editor.
#[tauri::command]
fn save_spec(path: String, spec: Spec, original_text: Option<String>) -> CmdResult<String> {
    let mut doc = match original_text {
        Some(t) if !t.trim().is_empty() => SpecDocument::parse(&t)?,
        _ => match std::fs::read_to_string(&path) {
            Ok(t) => SpecDocument::parse(&t)?,
            Err(_) => SpecDocument::from_spec(&spec)?,
        },
    };
    doc.apply(&spec)?;
    let text = doc.to_toml_string();
    std::fs::write(&path, &text)?;
    Ok(text)
}

/// Render the artifacts for the preview pane, in memory.
#[tauri::command]
fn preview_artifacts(
    spec: Spec,
    options: BuildOptions,
    platform: Option<String>,
) -> CmdResult<Vec<ArtifactView>> {
    let base = options.base_dir.clone();
    let icon = read_icon(&spec, base.as_deref()).unwrap_or(None);
    let schema = read_schema(&spec, base.as_deref()).unwrap_or(None);
    let opts = options.into_core(icon, schema);

    let artifacts = match platform.as_deref() {
        Some(p) => {
            let platform: Platform = p.parse().map_err(|e: String| CommandError::msg(e))?;
            generate_for(&spec, &opts, platform)?
        }
        None => generate(&spec, &opts)?,
    };
    Ok(artifacts.iter().map(ArtifactView::from).collect())
}

/// Write the artifacts to disk.
#[tauri::command]
fn build_artifacts(
    spec: Spec,
    options: BuildOptions,
    output_dir: String,
) -> CmdResult<BuildResult> {
    let base = options.base_dir.clone();
    let icon = read_icon(&spec, base.as_deref())?;
    let schema = read_schema(&spec, base.as_deref())?;
    let opts = options.into_core(icon, schema);

    let diagnostics = filekind_core::validate(&spec);
    if diagnostics.iter().any(|d| d.is_error()) {
        return Err(filekind_core::Error::Invalid(diagnostics).into());
    }

    let artifacts = generate(&spec, &opts)?;
    let root = PathBuf::from(&output_dir);
    let mut files = Vec::with_capacity(artifacts.len());
    let mut total = 0usize;

    for a in &artifacts {
        let dest = root.join(&a.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, a.bytes())?;
        if a.executable {
            set_executable(&dest)?;
        }
        total += a.len();
        files.push(a.path.clone());
    }

    Ok(BuildResult {
        output_dir,
        files,
        total_bytes: total,
    })
}

/// Run the icon pipeline on a PNG the user picked, for the Icon screen.
#[tauri::command]
fn render_icons(png_path: String) -> CmdResult<IconPreview> {
    let bytes = std::fs::read(&png_path)?;
    let set = IconSet::from_png_bytes(&bytes)?;
    Ok(icon_preview(&set))
}

/// A blank spec for the wizard's first screen.
#[tauri::command]
fn scaffold_spec(name: String, extension: String) -> CmdResult<SpecFile> {
    let spec = Spec::scaffold(&name, &extension);
    let text = spec.to_toml_string()?;
    let diagnostics = filekind_core::validate(&spec);
    Ok(SpecFile {
        path: None,
        text,
        spec,
        diagnostics,
    })
}

/// Values the spec derives rather than stores, so the Identity screen can show
/// what the user is actually getting.
#[tauri::command]
fn derived_values(spec: Spec) -> serde_json::Value {
    serde_json::json!({
        "progid": spec.progid(),
        "uti": spec.uti(),
        "utiConformsTo": spec.uti_conforms_to(),
        "mimeIconName": spec.mime_icon_name(),
        "desktopId": spec.desktop_id(),
        "slug": spec.slug(),
        "dottedExtension": spec.dotted_extension(),
        "magicHex": spec
            .magic_bytes()
            .ok()
            .flatten()
            .map(|b| filekind_core::magic::to_hex_dump(&b)),
    })
}

/// The recently-opened list (v0.3). Stored next to the app's own config, as a
/// plain JSON array — small enough not to justify a database, and readable if
/// someone wants to clear it by hand.
#[tauri::command]
fn recent_specs(app: tauri::AppHandle) -> Vec<String> {
    read_recents(&app)
        .into_iter()
        .filter(|p| Path::new(p).exists())
        .collect()
}

#[tauri::command]
fn remember_spec(app: tauri::AppHandle, path: String) -> Vec<String> {
    let mut recents = read_recents(&app);
    recents.retain(|p| p != &path);
    recents.insert(0, path);
    recents.truncate(10);
    if let Some(file) = recents_path(&app) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&recents) {
            let _ = std::fs::write(file, json);
        }
    }
    recents
}

// Helpers — the I/O core refuses to do

fn resolve(base: Option<&str>, rel: &str) -> PathBuf {
    match base {
        Some(b) => Path::new(b).join(rel),
        None => PathBuf::from(rel),
    }
}

fn read_icon(spec: &Spec, base: Option<&str>) -> CmdResult<Option<IconSet>> {
    let Some(rel) = spec.association.icon.as_deref() else {
        return Ok(None);
    };
    if rel.trim().is_empty() {
        return Ok(None);
    }
    let path = resolve(base, rel);
    let bytes = std::fs::read(&path)
        .map_err(|e| CommandError::msg(format!("could not read icon {}: {e}", path.display())))?;
    Ok(Some(IconSet::from_png_bytes(&bytes)?))
}

fn read_schema(spec: &Spec, base: Option<&str>) -> CmdResult<Option<String>> {
    let Some(rel) = spec.schema.as_ref().and_then(|s| s.kaitai.as_deref()) else {
        return Ok(None);
    };
    let path = resolve(base, rel);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| CommandError::msg(format!("could not read schema {}: {e}", path.display())))?;
    Ok(Some(text))
}

fn icon_preview(set: &IconSet) -> IconPreview {
    use base64_lite::encode;
    IconPreview {
        source_width: set.source_size.0,
        source_height: set.source_size.1,
        warnings: set.warnings.iter().map(|w| w.to_string()).collect(),
        previews: set.hicolor.iter().map(|(s, b)| (*s, encode(b))).collect(),
        ico_bytes: set.ico.len(),
        icns_bytes: set.icns.len(),
    }
}

/// Base64 without pulling the crate in twice — core already depends on it, but
/// it is not re-exported, and one 20-line encoder is cheaper than a public API
/// commitment.
mod base64_lite {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
            out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
            out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
            out.push(if chunk.len() > 1 {
                ALPHABET[(n >> 6 & 63) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                ALPHABET[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }
}

fn recents_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager as _;
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("recent-specs.json"))
}

fn read_recents(app: &tauri::AppHandle) -> Vec<String> {
    recents_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            parse_spec,
            load_spec,
            save_spec,
            preview_artifacts,
            build_artifacts,
            render_icons,
            scaffold_spec,
            derived_values,
            recent_specs,
            remember_spec,
        ])
        .run(tauri::generate_context!())
        .expect("error while running filekind");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_tripping_a_commented_spec_keeps_the_comments() {
        let original = "# my notes\n[format]\nname = \"A\"  # inline\nextension = \"aaa\"\n\
                        description = \"d\"\n\n[association]\nmime = \"application/vnd.x.aaa\"\n";
        let mut doc = SpecDocument::parse(original).unwrap();
        let mut spec = doc.spec().unwrap();
        spec.format.description = "changed".into();
        doc.apply(&spec).unwrap();
        let out = doc.to_toml_string();
        assert!(out.contains("# my notes"));
        assert!(out.contains("# inline"));
        assert!(out.contains("changed"));
    }

    #[test]
    fn base64_matches_the_reference_vectors() {
        assert_eq!(base64_lite::encode(b""), "");
        assert_eq!(base64_lite::encode(b"f"), "Zg==");
        assert_eq!(base64_lite::encode(b"fo"), "Zm8=");
        assert_eq!(base64_lite::encode(b"foo"), "Zm9v");
        assert_eq!(base64_lite::encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_half_typed_spec_yields_diagnostics_rather_than_an_error() {
        let f = parse_spec(
            "[format]\nname=\"\"\nextension=\"EXE\"\ndescription=\"\"\n\
             [association]\nmime=\"nope\"\n"
                .into(),
        )
        .expect("parses as TOML even though it is nonsense as a spec");
        assert!(f.diagnostics.iter().any(|d| d.is_error()));
    }
}
