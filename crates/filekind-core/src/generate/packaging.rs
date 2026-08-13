//! v0.4: distribution packaging.
//!
//! Neither of these duplicates the generated files on disk. The `.deb` build
//! script assembles the package from the artifacts already sitting in
//! `linux/`, and the RPM spec installs from a tarball of the same directory.
//! Emitting a second copy of every icon would double the output for no reason.

use crate::artifact::{Artifact, ArtifactKind, Platform};
use crate::error::Result;
use crate::generate::templates::{self, Ctx};
use crate::generate::GenerateOptions;
use crate::spec::Spec;

pub(super) fn generate(
    spec: &Spec,
    _opts: &GenerateOptions,
    ctx: &Ctx,
    out: &mut Vec<Artifact>,
) -> Result<()> {
    let p = Platform::Linux;

    out.push(Artifact::text(
        "linux/packaging/deb/control",
        templates::render("deb_control", ctx)?,
        p,
        ArtifactKind::Deb,
        "Debian package metadata",
    ));

    out.push(
        Artifact::text(
            "linux/packaging/deb/postinst",
            templates::render("deb_postinst", ctx)?,
            p,
            ArtifactKind::Deb,
            "Refreshes the MIME, desktop and icon caches after install",
        )
        .exec(),
    );

    out.push(
        Artifact::text(
            "linux/packaging/deb/postrm",
            templates::render("deb_postrm", ctx)?,
            p,
            ArtifactKind::Deb,
            "Refreshes the same caches after removal",
        )
        .exec(),
    );

    out.push(
        Artifact::text(
            "linux/packaging/build-deb.sh",
            templates::render("deb_build", ctx)?,
            p,
            ArtifactKind::Deb,
            "Assembles a .deb from the files in linux/",
        )
        .exec(),
    );

    out.push(Artifact::text(
        format!("linux/packaging/{}-mime.spec", spec.slug()),
        templates::render("rpm_spec", ctx)?,
        p,
        ArtifactKind::Rpm,
        "RPM spec file",
    ));

    Ok(())
}
