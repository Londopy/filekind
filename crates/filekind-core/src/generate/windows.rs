//! Windows artifacts: the registry tree, its undo, and installer snippets.

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::Result;
use crate::generate::{templates, GenerateOptions};
use crate::spec::Spec;

pub(super) fn generate(spec: &Spec, opts: &GenerateOptions, out: &mut Vec<Artifact>) -> Result<()> {
    let ctx = templates::context(spec, opts);
    let p = Platform::Windows;

    out.push(Artifact::text(
        "windows/register.reg",
        templates::render("windows_register", &ctx)?,
        p,
        ArtifactKind::Reg,
        format!(
            "Registers {} under {}",
            spec.dotted_extension(),
            opts.scope.hive_short()
        ),
    ));

    out.push(Artifact::text(
        "windows/unregister.reg",
        templates::render("windows_unregister", &ctx)?,
        p,
        ArtifactKind::RegUninstall,
        "Reverses register.reg",
    ));

    out.push(Artifact::text(
        "windows/inno-snippet.iss",
        templates::render("windows_inno", &ctx)?,
        p,
        ArtifactKind::Inno,
        "[Registry] section for an Inno Setup script",
    ));

    out.push(Artifact::text(
        "windows/nsis-snippet.nsh",
        templates::render("windows_nsis", &ctx)?,
        p,
        ArtifactKind::Nsis,
        "Register/unregister macros for an NSIS script",
    ));

    if let Some(icons) = &opts.icon {
        out.push(Artifact::binary(
            "windows/icon.ico",
            icons.ico.clone(),
            p,
            ArtifactKind::Icon,
            format!(
                "Multi-resolution icon ({})",
                size_list(crate::icon::ICO_SIZES)
            ),
        ));
    }

    Ok(())
}

fn size_list(sizes: &[u32]) -> String {
    sizes
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("/")
}
