//! Command implementations. Everything that touches the filesystem lives here.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use filekind_core::{
    generate, generate_for, ArtifactKind, GenerateOptions, IconSet, Platform, Scope, Severity, Spec,
};

use crate::report;
use crate::{EXIT_GENERATION, EXIT_IO, EXIT_OK, EXIT_SPEC_INVALID};

pub struct Ctx {
    pub quiet: bool,
}

impl Ctx {
    fn say(&self, msg: impl AsRef<str>) {
        if !self.quiet {
            anstream::println!("{}", msg.as_ref());
        }
    }
}

/// Map any error back to one of the documented exit codes.
///
/// `anyhow` flattens everything into one type, so the mapping is recovered by
/// downcasting: a core error knows which bucket it belongs to, and anything
/// else that reached us came from the filesystem.
pub fn exit_code(e: &anyhow::Error) -> i32 {
    if let Some(fk) = e.downcast_ref::<filekind_core::Error>() {
        return fk.exit_code();
    }
    if e.downcast_ref::<std::io::Error>().is_some() {
        return EXIT_IO;
    }
    for cause in e.chain() {
        if let Some(fk) = cause.downcast_ref::<filekind_core::Error>() {
            return fk.exit_code();
        }
        if cause.downcast_ref::<std::io::Error>().is_some() {
            return EXIT_IO;
        }
    }
    EXIT_GENERATION
}

// Locating and loading specs

/// Find the spec to operate on.
///
/// An explicit path always wins. Otherwise we look for exactly one `.filekind`
/// file in the working directory — one is unambiguous, several is not, and
/// guessing between them would be worse than asking.
fn find_spec(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.exists() {
            bail!("no such spec file: {}", p.display());
        }
        return Ok(p);
    }

    let mut found: Vec<PathBuf> = std::fs::read_dir(".")
        .context("could not read the current directory")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "filekind"))
        .collect();
    found.sort();

    match found.len() {
        0 => bail!(
            "no .filekind spec in this directory\n       run `filekind init` to create one, \
             or pass a path"
        ),
        1 => Ok(found.remove(0)),
        n => {
            let names: Vec<String> = found.iter().map(|p| p.display().to_string()).collect();
            bail!(
                "{n} spec files here; say which one:\n       {}",
                names.join("\n       ")
            )
        }
    }
}

struct Loaded {
    spec: Spec,
    diagnostics: Vec<filekind_core::Diagnostic>,
    path: PathBuf,
}

impl Loaded {
    /// The directory relative paths in the spec resolve against.
    fn base_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn load(path: &Path) -> Result<Loaded> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let spec = Spec::from_toml_str(&text).with_context(|| format!("in {}", path.display()))?;
    let diagnostics = filekind_core::validate(&spec);
    Ok(Loaded {
        spec,
        diagnostics,
        path: path.to_path_buf(),
    })
}

/// Load, and refuse to continue if anything is an error.
fn load_valid(path: &Path) -> Result<Loaded> {
    let loaded = load(path)?;
    if loaded
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        report::diagnostics(&loaded.diagnostics);
        bail!(filekind_core::Error::Invalid(loaded.diagnostics));
    }
    Ok(loaded)
}

/// Read and render the icon referenced by the spec, if there is one.
///
/// Core will not open files, so this is where a path becomes bytes.
fn load_icon(ctx: &Ctx, loaded: &Loaded) -> Result<Option<IconSet>> {
    let Some(rel) = loaded.spec.association.icon.as_deref() else {
        return Ok(None);
    };
    let path = loaded.base_dir().join(rel);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "could not read the icon at {}\n       [association].icon is resolved relative to \
             the spec file",
            path.display()
        )
    })?;
    let set = IconSet::from_png_bytes(&bytes).with_context(|| format!("in {}", path.display()))?;
    for w in &set.warnings {
        if !ctx.quiet {
            anstream::eprintln!("{}: {w}", report::yellow("warning"));
        }
    }
    Ok(Some(set))
}

pub fn init(
    ctx: &Ctx,
    name: Option<String>,
    extension: Option<String>,
    output: Option<PathBuf>,
    yes: bool,
    force: bool,
) -> Result<()> {
    let interactive = !yes && std::io::stdin().is_terminal();

    let name = match name {
        Some(n) => n,
        None if interactive => prompt("Format name", "My Format")?,
        None => "My Format".to_string(),
    };
    let extension = match extension {
        Some(e) => e,
        None if interactive => {
            let default = default_extension(&name);
            prompt("Extension (no dot)", &default)?
        }
        None => default_extension(&name),
    };

    let mut spec = Spec::scaffold(&name, &extension);

    if interactive {
        spec.format.description = prompt("Description", &spec.format.description)?;
        spec.association.mime = prompt("MIME type", &spec.association.mime)?;
        let handler = prompt("Handler (blank for none)", "")?;
        spec.association.handler = if handler.trim().is_empty() {
            None
        } else {
            Some(handler)
        };
    }

    let diags = filekind_core::validate(&spec);
    let errors: Vec<_> = diags.iter().filter(|d| d.is_error()).cloned().collect();
    if !errors.is_empty() {
        report::diagnostics(&errors);
        bail!(filekind_core::Error::Invalid(errors));
    }

    let path = output.unwrap_or_else(|| PathBuf::from(format!("{}.filekind", spec.slug())));
    if path.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            path.display()
        );
    }

    std::fs::write(&path, scaffold_toml(&spec))
        .with_context(|| format!("could not write {}", path.display()))?;

    ctx.say(format!("{} {}", report::green("created"), path.display()));
    if !diags.is_empty() {
        report::diagnostics(&diags);
    }
    ctx.say("\nNext: edit the spec, then `filekind check` and `filekind build -o dist`.");
    Ok(())
}

fn default_extension(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if s.is_empty() {
        "myfmt".into()
    } else {
        s.chars().take(8).collect()
    }
}

fn prompt(label: &str, default: &str) -> Result<String> {
    if default.is_empty() {
        anstream::print!("{label}: ");
    } else {
        anstream::print!("{label} [{}]: ", report::dim(default));
    }
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let line = line.trim();
    Ok(if line.is_empty() {
        default.to_string()
    } else {
        line.to_string()
    })
}

/// A commented spec, rather than `toml::to_string`'s bare key-value dump.
///
/// The scaffold is documentation: it is where most people will learn that
/// magic bytes need a TOML *literal* string, and that macOS is off by default
/// for a reason.
fn scaffold_toml(spec: &Spec) -> String {
    let handler = spec.association.handler.clone().unwrap_or_default();
    format!(
        r##"# {name} — a filekind spec.
#
# Generate artifacts with:  filekind build -o dist
# Check it first with:      filekind check

[format]
name        = "{name}"
extension   = "{ext}"              # no leading dot; a-z and 0-9 only
description = "{desc}"

# Magic bytes at offset 0. Use a TOML *literal* string (single quotes):
# basic strings only understand \n, \t, \u1234 and friends, so "\x01" is a
# TOML syntax error and "\u0001" silently becomes UTF-8 for bytes above 0x7F.
# magic     = 'MYFM\x01'

container   = "{container}"           # zip | sqlite | text | binary | none
version     = "{version}"

[association]
mime         = "{mime}"
# icon       = "assets/icon.png"   # one square PNG, 512x512 or larger
{handler}handler_args = '"%1"'              # normalised to %f on Linux

[targets]
windows = {windows}
linux   = {linux}
# macOS needs a signed, notarised app bundle before a UTI declaration does
# anything at all. Turn this on only if you have one. See the generated
# README for what you get.
macos   = {macos}

[windows]
# progid       = "Vendor.Format.1" # derived from the MIME vendor tree if unset
# perceived_type = "document"      # text|image|audio|video|compressed|document|…
# icon_path    = 'C:\Program Files\MyApp\icon.ico'

[linux]
desktop_categories = [{cats}]
# generic_name     = "Save File"

[macos]
# bundle_id       = "com.example.myapp"
# uti_conforms_to = ["public.data"]
"##,
        name = spec.format.name,
        ext = spec.format.extension,
        desc = spec.format.description,
        container = spec.format.container,
        version = spec.format.version.clone().unwrap_or_else(|| "1.0".into()),
        mime = spec.association.mime,
        handler = if handler.is_empty() {
            // Commented out rather than empty: a blank handler is a valid
            // choice, but an empty string looks like a mistake.
            "# handler    = \"myapp\"             # executable name, path, or macOS bundle id\n"
                .to_string()
        } else {
            format!("handler      = \"{handler}\"\n")
        },
        windows = spec.targets.windows,
        linux = spec.targets.linux,
        macos = spec.targets.macos,
        cats = spec
            .linux
            .desktop_categories
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    ctx: &Ctx,
    spec_path: Option<PathBuf>,
    output: PathBuf,
    target: Option<Platform>,
    system: bool,
    packaging: bool,
    stub_app: bool,
    force: bool,
) -> Result<()> {
    let path = find_spec(spec_path)?;
    let loaded = load_valid(&path)?;

    if !loaded.diagnostics.is_empty() {
        report::diagnostics(&loaded.diagnostics);
        if !ctx.quiet {
            anstream::eprintln!();
        }
    }

    if output.exists() && !force {
        let non_empty = std::fs::read_dir(&output)
            .with_context(|| format!("could not read {}", output.display()))?
            .next()
            .is_some();
        if non_empty {
            bail!(
                "{} is not empty (use --force to write into it anyway)",
                output.display()
            );
        }
    }

    let opts = GenerateOptions {
        scope: if system { Scope::System } else { Scope::User },
        icon: load_icon(ctx, &loaded)?,
        packaging,
        stub_app,
        kaitai_schema: load_kaitai(&loaded)?,
    };

    let artifacts = match target {
        Some(p) => generate_for(&loaded.spec, &opts, p)?,
        None => generate(&loaded.spec, &opts)?,
    };

    let mut total = 0usize;
    for a in &artifacts {
        let dest = output.join(&a.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        std::fs::write(&dest, a.bytes())
            .with_context(|| format!("could not write {}", dest.display()))?;
        if a.executable {
            set_executable(&dest)?;
        }
        total += a.len();
        ctx.say(format!(
            "  {} {}  {}",
            report::dim("+"),
            a.path,
            report::dim(&report::bytes(a.len()))
        ));
    }

    ctx.say(format!(
        "\n{} {} file{} ({}) in {}",
        report::green("wrote"),
        artifacts.len(),
        report::plural(artifacts.len()),
        report::bytes(total),
        output.display()
    ));
    ctx.say(report::dim(
        "nothing has been installed — read the generated README, then run the scripts",
    ));
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms)
        .with_context(|| format!("could not chmod +x {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    // Windows has no executable bit. The scripts are for Linux anyway, and
    // git will restore the mode on checkout there.
    Ok(())
}

fn load_kaitai(loaded: &Loaded) -> Result<Option<String>> {
    let Some(schema) = &loaded.spec.schema else {
        return Ok(None);
    };
    let Some(rel) = &schema.kaitai else {
        return Ok(None);
    };
    let path = loaded.base_dir().join(rel);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read the kaitai schema at {}", path.display()))?;
    Ok(Some(text))
}

pub fn check(ctx: &Ctx, spec_path: Option<PathBuf>, json: bool, strict: bool) -> Result<()> {
    let path = find_spec(spec_path)?;
    let loaded = load(&path)?;

    if json {
        let out = serde_json::json!({
            "spec": path.display().to_string(),
            "valid": !loaded.diagnostics.iter().any(|d| d.is_error()),
            "derived": {
                "progid": loaded.spec.progid(),
                "uti": loaded.spec.uti(),
                "mime_icon_name": loaded.spec.mime_icon_name(),
                "desktop_id": loaded.spec.desktop_id(),
                "slug": loaded.spec.slug(),
            },
            "diagnostics": loaded.diagnostics,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        ctx.say(format!("{} {}", report::bold("spec:"), path.display()));
        for (label, value) in [
            ("progid", loaded.spec.progid()),
            ("uti", loaded.spec.uti()),
            ("desktop id", loaded.spec.desktop_id()),
            ("mime icon", loaded.spec.mime_icon_name()),
        ] {
            ctx.say(format!("  {:<12} {value}", report::dim(label)));
        }
        if !loaded.diagnostics.is_empty() {
            ctx.say("");
            report::diagnostics_stdout(&loaded.diagnostics);
        }
        ctx.say(format!("\n{}", report::summary(&loaded.diagnostics)));
    }

    let failed = loaded.diagnostics.iter().any(|d| d.is_error())
        || (strict && !loaded.diagnostics.is_empty());
    if failed {
        std::process::exit(EXIT_SPEC_INVALID);
    }
    std::process::exit(EXIT_OK);
}

pub fn verify(ctx: &Ctx, file: PathBuf, spec_path: Option<PathBuf>, json: bool) -> Result<()> {
    let path = find_spec(spec_path)?;
    let loaded = load(&path)?;

    let bytes =
        std::fs::read(&file).with_context(|| format!("could not read {}", file.display()))?;
    let name = file.file_name().and_then(|s| s.to_str());
    let report_ = filekind_core::verify_bytes(&loaded.spec, &bytes, name);

    if json {
        println!("{}", serde_json::to_string_pretty(&report_)?);
    } else {
        ctx.say(format!("{} {}", report::bold("file:"), file.display()));
        for c in &report_.checks {
            let mark = match c.outcome {
                filekind_core::verify::Outcome::Pass => report::green("pass"),
                filekind_core::verify::Outcome::Fail => report::red("FAIL"),
                filekind_core::verify::Outcome::Skipped => report::dim("skip"),
            };
            ctx.say(format!("  {mark}  {:<10} {}", c.name, c.detail));
        }
        ctx.say(format!(
            "\n{}",
            if report_.matches {
                report::green("matches the spec")
            } else {
                report::red("does not match the spec")
            }
        ));
    }

    if !report_.matches {
        std::process::exit(EXIT_SPEC_INVALID);
    }
    Ok(())
}

pub fn preview(
    ctx: &Ctx,
    spec_path: Option<PathBuf>,
    target: Platform,
    kind: String,
    list: bool,
    system: bool,
) -> Result<()> {
    let path = find_spec(spec_path)?;
    let loaded = load_valid(&path)?;

    let opts = GenerateOptions {
        scope: if system { Scope::System } else { Scope::User },
        icon: load_icon(ctx, &loaded)?,
        packaging: true,
        stub_app: true,
        kaitai_schema: load_kaitai(&loaded)?,
    };
    let artifacts = generate_for(&loaded.spec, &opts, target)?;

    if list {
        for a in &artifacts {
            println!(
                "{:<14} {:<44} {}",
                a.kind.to_string(),
                a.path,
                a.description
            );
        }
        return Ok(());
    }

    let wanted: ArtifactKind = kind
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{e}\n       try `filekind preview --list`"))?;

    let Some(found) = artifacts.iter().find(|a| a.kind == wanted) else {
        let mut available: Vec<String> = artifacts.iter().map(|a| a.kind.to_string()).collect();
        available.dedup();
        bail!(
            "no `{wanted}` artifact for {target}\n       available: {}",
            available.join(", ")
        );
    };

    match found.as_text() {
        Some(t) => print!("{t}"),
        None => bail!(
            "{} is binary ({}); use `filekind build` or `filekind icons` instead",
            found.path,
            report::bytes(found.len())
        ),
    }
    Ok(())
}

pub fn icons(ctx: &Ctx, png: PathBuf, output: PathBuf, name: String) -> Result<()> {
    let bytes = std::fs::read(&png).with_context(|| format!("could not read {}", png.display()))?;
    let set = IconSet::from_png_bytes(&bytes)?;

    for w in &set.warnings {
        anstream::eprintln!("{}: {w}", report::yellow("warning"));
    }

    std::fs::create_dir_all(&output)
        .with_context(|| format!("could not create {}", output.display()))?;

    let ico = output.join(format!("{name}.ico"));
    std::fs::write(&ico, &set.ico)?;
    ctx.say(format!("  {} {}", report::dim("+"), ico.display()));

    let icns = output.join(format!("{name}.icns"));
    std::fs::write(&icns, &set.icns)?;
    ctx.say(format!("  {} {}", report::dim("+"), icns.display()));

    for (size, data) in &set.hicolor {
        let dir = output.join(format!("hicolor/{size}x{size}/mimetypes"));
        std::fs::create_dir_all(&dir)?;
        let p = dir.join(format!("{name}.png"));
        std::fs::write(&p, data)?;
        ctx.say(format!("  {} {}", report::dim("+"), p.display()));
    }

    ctx.say(format!(
        "\n{} from {}×{} source",
        report::green("done"),
        set.source_size.0,
        set.source_size.1
    ));
    Ok(())
}

/// Launch the desktop app.
///
/// The CLI spawns the GUI binary; the GUI never spawns the CLI. That asymmetry
/// is deliberate — the GUI links `filekind-core` directly so its preview pane
/// can render without a subprocess per keystroke.
pub fn gui(ctx: &Ctx, spec: Option<PathBuf>) -> Result<()> {
    let exe = locate_gui()?;
    ctx.say(format!("{} {}", report::dim("launching"), exe.display()));

    let mut cmd = std::process::Command::new(&exe);
    if let Some(s) = spec {
        cmd.arg(s);
    }
    let status = cmd
        .status()
        .with_context(|| format!("could not launch {}", exe.display()))?;

    if !status.success() {
        bail!("filekind-gui exited with {status}");
    }
    Ok(())
}

fn locate_gui() -> Result<PathBuf> {
    let name = if cfg!(windows) {
        "filekind-gui.exe"
    } else {
        "filekind-gui"
    };

    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    bail!(
        "could not find {name}\n       \
         the GUI ships as a separate binary; install it alongside `filekind`, \
         or build it with `cargo build -p filekind-gui`"
    )
}

use std::io::IsTerminal as _;
