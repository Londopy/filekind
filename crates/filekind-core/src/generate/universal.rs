//! Artifacts that are not tied to one operating system: the libmagic pattern
//! and the generated install instructions.

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::Result;
use crate::generate::{templates, GenerateOptions};
use crate::spec::Spec;

pub(super) fn generate(spec: &Spec, opts: &GenerateOptions, out: &mut Vec<Artifact>) -> Result<()> {
    let ctx = templates::context(spec, opts);
    let p = Platform::Universal;

    out.push(Artifact::text(
        "magic.txt",
        templates::render("universal_magic", &ctx)?,
        p,
        ArtifactKind::Magic,
        if spec.format.magic.is_some() {
            "libmagic pattern for content-based detection"
        } else {
            "libmagic placeholder — no magic bytes declared"
        },
    ));

    out.push(Artifact::text(
        "README.md",
        templates::render("universal_readme", &ctx)?,
        p,
        ArtifactKind::Readme,
        "Per-platform install and uninstall instructions",
    ));

    Ok(())
}
