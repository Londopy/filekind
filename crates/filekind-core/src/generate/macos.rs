//! macOS artifacts. Experimental, and the generated files say so in their own
//! comments rather than only in the docs — a plist that silently does nothing
//! is worse than no plist at all.

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::Result;
use crate::generate::{templates, GenerateOptions};
use crate::spec::Spec;

pub(super) fn generate(spec: &Spec, opts: &GenerateOptions, out: &mut Vec<Artifact>) -> Result<()> {
    let ctx = templates::context(spec, opts);
    let p = Platform::Macos;

    out.push(Artifact::text(
        "macos/Info.plist.fragment",
        templates::render("macos_fragment", &ctx)?,
        p,
        ArtifactKind::Plist,
        "UTExportedTypeDeclarations + CFBundleDocumentTypes to paste into a signed app",
    ));

    if let Some(icons) = &opts.icon {
        out.push(Artifact::binary(
            "macos/icon.icns",
            icons.icns.clone(),
            p,
            ArtifactKind::Icon,
            "Icon family for the bundle",
        ));
    }

    if opts.stub_app {
        // The bundle directory name comes from user input, so it is sanitised
        // rather than interpolated. A format called `Bad" Name` must not
        // produce a path with a quote in it — that is a filesystem problem on
        // Windows and a quoting problem in every script that touches it.
        let app = format!(
            "macos/{}.app",
            safe_dir_name(&spec.format.name, &spec.slug())
        );

        out.push(Artifact::text(
            format!("{app}/Contents/Info.plist"),
            templates::render("macos_app_plist", &ctx)?,
            p,
            ArtifactKind::Plist,
            "Info.plist for the unsigned stub bundle",
        ));

        // Eight bytes, mandatory, universally ignored, and macOS still wants it.
        out.push(Artifact::text(
            format!("{app}/Contents/PkgInfo"),
            "APPL????",
            p,
            ArtifactKind::Bundle,
            "Legacy bundle type/creator record",
        ));

        out.push(
            Artifact::text(
                format!("{app}/Contents/MacOS/{}", spec.slug()),
                templates::render("macos_app_stub", &ctx)?,
                p,
                ArtifactKind::Script,
                "Bundle executable (CFBundleExecutable must exist and run)",
            )
            .exec(),
        );

        if let Some(icons) = &opts.icon {
            out.push(Artifact::binary(
                format!("{app}/Contents/Resources/icon.icns"),
                icons.icns.clone(),
                p,
                ArtifactKind::Icon,
                "Bundle icon",
            ));
        }
    }

    Ok(())
}

/// Make a directory name out of a human-readable format name.
///
/// Keeps letters, digits, spaces, dots, hyphens and underscores — enough that
/// `Londo Save` stays `Londo Save.app` — and falls back to the slug if nothing
/// usable survives.
fn safe_dir_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::safe_dir_name;

    #[test]
    fn ordinary_names_are_left_alone() {
        assert_eq!(safe_dir_name("Londo Save", "x"), "Londo Save");
    }

    #[test]
    fn quotes_slashes_and_traversal_are_stripped() {
        assert_eq!(safe_dir_name(r#"Bad" Name"#, "x"), "Bad Name");
        assert_eq!(safe_dir_name("../../etc/passwd", "x"), "etcpasswd");
        assert_eq!(safe_dir_name("..", "fallback"), "fallback");
        assert_eq!(safe_dir_name("", "fallback"), "fallback");
    }
}
