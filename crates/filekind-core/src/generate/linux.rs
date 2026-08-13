//! Linux artifacts: shared-mime-info, a desktop entry, the hicolor icon tree,
//! and the two scripts that install and remove them.

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::Result;
use crate::generate::{packaging, templates, GenerateOptions};
use crate::spec::Spec;

pub(super) fn generate(spec: &Spec, opts: &GenerateOptions, out: &mut Vec<Artifact>) -> Result<()> {
    let ctx = templates::context(spec, opts);
    let p = Platform::Linux;

    out.push(Artifact::text(
        format!("linux/{}", ctx.mime_file),
        templates::render("linux_mime", &ctx)?,
        p,
        ArtifactKind::MimeXml,
        format!("shared-mime-info definition for {}", spec.association.mime),
    ));

    out.push(Artifact::text(
        format!("linux/{}", ctx.desktop_file),
        templates::render("linux_desktop", &ctx)?,
        p,
        ArtifactKind::Desktop,
        "Desktop entry claiming the MIME type",
    ));

    out.push(
        Artifact::text(
            "linux/install.sh",
            templates::render("linux_install", &ctx)?,
            p,
            ArtifactKind::Script,
            "Installs the type via xdg-mime and xdg-icon-resource",
        )
        .exec(),
    );

    out.push(
        Artifact::text(
            "linux/uninstall.sh",
            templates::render("linux_uninstall", &ctx)?,
            p,
            ArtifactKind::Script,
            "Removes everything install.sh added",
        )
        .exec(),
    );

    if let Some(icons) = &opts.icon {
        for (size, png) in &icons.hicolor {
            out.push(Artifact::binary(
                format!(
                    "linux/icons/hicolor/{size}x{size}/mimetypes/{}.png",
                    spec.mime_icon_name()
                ),
                png.clone(),
                p,
                ArtifactKind::Icon,
                format!("{size}×{size} MIME icon"),
            ));
        }
    }

    if opts.packaging {
        packaging::generate(spec, opts, &ctx, out)?;
    }

    Ok(())
}
