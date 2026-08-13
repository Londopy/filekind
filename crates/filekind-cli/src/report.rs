//! Terminal output. Kept away from core, which must never print.
//!
//! Colour is applied only when stdout is a terminal and `NO_COLOR` is unset,
//! so piping `filekind check` into a log does not fill it with escape codes.

use std::io::IsTerminal;
use std::sync::OnceLock;

use filekind_core::{Diagnostic, Severity};

fn colour() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

fn paint(code: &str, s: &str) -> String {
    if colour() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}

pub fn dim(s: &str) -> String {
    paint("2", s)
}

pub fn red(s: &str) -> String {
    paint("31;1", s)
}

pub fn yellow(s: &str) -> String {
    paint("33;1", s)
}

pub fn green(s: &str) -> String {
    paint("32;1", s)
}

/// Print a top-level failure to stderr, including any nested causes.
pub fn error(e: &anyhow::Error) {
    eprintln!("{} {e}", red("error:"));
    for cause in e.chain().skip(1) {
        eprintln!("  {} {cause}", dim("caused by:"));
    }
    // A validation failure carries its own diagnostics; surface them rather
    // than making the user re-run `check`.
    if let Some(fk) = e.downcast_ref::<filekind_core::Error>() {
        if !fk.diagnostics().is_empty() {
            eprintln!();
            diagnostics(fk.diagnostics());
        }
    }
}

/// Render a diagnostic list the way a compiler would.
pub fn format_diagnostics(diags: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diags {
        let label = match d.severity {
            Severity::Error => red("error"),
            Severity::Warning => yellow("warning"),
        };
        out.push_str(&format!("{label}: {}: {}\n", bold(&d.field), d.message));
        if let Some(h) = &d.hint {
            out.push_str(&format!("       {} {h}\n", dim("hint:")));
        }
    }
    out
}

/// Print diagnostics to stderr, flushing stdout first so the two streams do
/// not interleave when both are redirected to the same pipe.
pub fn diagnostics(diags: &[Diagnostic]) {
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
    eprint!("{}", format_diagnostics(diags));
    let _ = std::io::stderr().flush();
}

/// Print diagnostics to stdout. `check` uses this: for that command the
/// diagnostics *are* the output, not a side-channel.
pub fn diagnostics_stdout(diags: &[Diagnostic]) {
    print!("{}", format_diagnostics(diags));
}

/// One-line summary of a diagnostic run.
pub fn summary(diags: &[Diagnostic]) -> String {
    let errors = diags.iter().filter(|d| d.is_error()).count();
    let warnings = diags.len() - errors;
    match (errors, warnings) {
        (0, 0) => green("no problems found").to_string(),
        (0, w) => yellow(&format!("{w} warning{}", plural(w))).to_string(),
        (e, 0) => red(&format!("{e} error{}", plural(e))).to_string(),
        (e, w) => format!(
            "{}, {}",
            red(&format!("{e} error{}", plural(e))),
            yellow(&format!("{w} warning{}", plural(w)))
        ),
    }
}

pub fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Human-readable byte count for the build summary.
pub fn bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
    }
}
