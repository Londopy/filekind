//! `filekind` — the automation interface.
//!
//! Same engine as the GUI, no feature gap. This exists so a build can run in
//! CI, in a Makefile, and over SSH, where no window can open.
//!
//! This crate owns all the I/O. `filekind-core` reads and writes nothing; the
//! division is what makes the GUI's live preview possible, so it is worth
//! keeping even when it means passing icon bytes around by hand.

mod commands;
mod report;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Exit codes. Scripts depend on these, so treat them as part of the interface.
pub const EXIT_OK: i32 = 0;
pub const EXIT_SPEC_INVALID: i32 = 1;
pub const EXIT_IO: i32 = 2;
pub const EXIT_GENERATION: i32 = 3;

#[derive(Debug, Parser)]
#[command(
    name = "filekind",
    version,
    about = "Compile one spec file into a real, recognised file type on Windows, Linux and macOS.",
    long_about = "filekind takes a single declarative spec and generates everything an operating \
system needs before it will admit your file extension exists: a Windows registry tree, a \
freedesktop shared-mime-info XML plus .desktop entry, a macOS UTI declaration, a libmagic \
pattern, and icons in three container formats.\n\n\
It emits artifacts. It does not install them — read the generated files, then run them.",
    after_help = "Exit codes: 0 ok · 1 spec invalid · 2 I/O error · 3 generation failure"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Suppress progress output. Errors still go to stderr.
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold a new spec file.
    Init {
        /// Human-readable format name, e.g. "Londo Save".
        name: Option<String>,
        /// Extension without a leading dot.
        #[arg(short, long)]
        extension: Option<String>,
        /// Where to write the spec. Defaults to <slug>.filekind.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Accept defaults instead of prompting. Required in CI.
        #[arg(short, long)]
        yes: bool,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },

    /// Generate all artifacts for the enabled targets.
    Build {
        /// Path to a .filekind spec. Defaults to the only one in this directory.
        spec: Option<PathBuf>,
        /// Output directory.
        #[arg(short, long, default_value = "dist")]
        output: PathBuf,
        /// Restrict to one platform.
        #[arg(short, long, value_enum)]
        target: Option<TargetArg>,
        /// Emit for all users instead of the current one (HKLM, /usr/share).
        #[arg(long)]
        system: bool,
        /// Skip the .deb and .rpm packaging.
        #[arg(long)]
        no_packaging: bool,
        /// Also emit a skeleton .app bundle for macOS.
        #[arg(long)]
        stub_app: bool,
        /// Write into a non-empty directory.
        #[arg(short, long)]
        force: bool,
    },

    /// Validate a spec: syntax, reserved words, collisions.
    Check {
        spec: Option<PathBuf>,
        /// Machine-readable diagnostics.
        #[arg(long)]
        json: bool,
        /// Exit non-zero on warnings too.
        #[arg(long)]
        strict: bool,
    },

    /// Check whether a file matches the magic and container a spec declares.
    Verify {
        /// The file to inspect.
        file: PathBuf,
        #[arg(short, long)]
        spec: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },

    /// Print one artifact to stdout.
    Preview {
        spec: Option<PathBuf>,
        #[arg(short, long, value_enum, default_value = "windows")]
        target: TargetArg,
        /// Artifact kind: reg, mime-xml, desktop, plist, magic, readme, …
        #[arg(short, long, default_value = "reg")]
        kind: String,
        /// List what is available instead of printing one.
        #[arg(short, long)]
        list: bool,
        #[arg(long)]
        system: bool,
    },

    /// Run the icon pipeline on its own.
    Icons {
        /// Source PNG, 512×512 or larger.
        png: PathBuf,
        #[arg(short, long, default_value = "icons")]
        output: PathBuf,
        /// Base name for the generated files.
        #[arg(short, long, default_value = "icon")]
        name: String,
    },

    /// Launch the desktop app, optionally on an existing spec.
    Gui { spec: Option<PathBuf> },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TargetArg {
    Windows,
    Linux,
    Macos,
    Universal,
}

impl From<TargetArg> for filekind_core::Platform {
    fn from(t: TargetArg) -> Self {
        match t {
            TargetArg::Windows => filekind_core::Platform::Windows,
            TargetArg::Linux => filekind_core::Platform::Linux,
            TargetArg::Macos => filekind_core::Platform::Macos,
            TargetArg::Universal => filekind_core::Platform::Universal,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let ctx = commands::Ctx { quiet: cli.quiet };

    let result = match cli.command {
        Command::Init {
            name,
            extension,
            output,
            yes,
            force,
        } => commands::init(&ctx, name, extension, output, yes, force),
        Command::Build {
            spec,
            output,
            target,
            system,
            no_packaging,
            stub_app,
            force,
        } => commands::build(
            &ctx,
            spec,
            output,
            target.map(Into::into),
            system,
            !no_packaging,
            stub_app,
            force,
        ),
        Command::Check { spec, json, strict } => commands::check(&ctx, spec, json, strict),
        Command::Verify { file, spec, json } => commands::verify(&ctx, file, spec, json),
        Command::Preview {
            spec,
            target,
            kind,
            list,
            system,
        } => commands::preview(&ctx, spec, target.into(), kind, list, system),
        Command::Icons { png, output, name } => commands::icons(&ctx, png, output, name),
        Command::Gui { spec } => commands::gui(&ctx, spec),
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            report::error(&e);
            std::process::exit(commands::exit_code(&e));
        }
    }
}
