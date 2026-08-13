//! End-to-end CLI tests. The exit codes are part of the interface, so they are
//! asserted here rather than assumed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_filekind")
}

/// A scratch directory that cleans itself up.
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Tmp {
        let dir = std::env::temp_dir().join(format!(
            "filekind-test-{tag}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("filekind runs")
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("process exited normally")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

const GOOD: &str = r#"
[format]
name = "Londo Save"
extension = "londo"
description = "Londo project save file"
magic = 'LNDO\x01'
container = "binary"

[association]
mime = "application/vnd.london.londo"
handler = "londo-player"
"#;

#[test]
fn init_then_check_then_build_then_verify() {
    let tmp = Tmp::new("happy");

    let out = run(tmp.path(), &["init", "Londo Save", "-e", "londo", "--yes"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(tmp.path().join("londo-save.filekind").exists());

    // With exactly one spec in the directory, the path can be omitted.
    let out = run(tmp.path(), &["check"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));

    let out = run(tmp.path(), &["build", "-o", "dist"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    for expected in [
        "dist/windows/register.reg",
        "dist/windows/unregister.reg",
        "dist/linux/install.sh",
        "dist/magic.txt",
        "dist/README.md",
    ] {
        assert!(tmp.path().join(expected).exists(), "missing {expected}");
    }

    // Rebuilding into a non-empty directory needs --force.
    let out = run(tmp.path(), &["build", "-o", "dist"]);
    assert_ne!(code(&out), 0);
    assert!(stderr(&out).contains("not empty"), "{}", stderr(&out));
    assert_eq!(
        code(&run(tmp.path(), &["build", "-o", "dist", "--force"])),
        0
    );
}

#[test]
fn a_reserved_extension_exits_one() {
    let tmp = Tmp::new("reserved");
    std::fs::write(
        tmp.path().join("bad.filekind"),
        GOOD.replace("londo\"", "exe\""),
    )
    .unwrap();
    let out = run(tmp.path(), &["check", "bad.filekind"]);
    assert_eq!(code(&out), 1);
    assert!(stdout(&out).contains("reserved"), "{}", stdout(&out));
}

#[test]
fn broken_toml_exits_one_and_says_where() {
    let tmp = Tmp::new("broken");
    std::fs::write(tmp.path().join("bad.filekind"), "[format\nname = ").unwrap();
    let out = run(tmp.path(), &["check", "bad.filekind"]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
}

#[test]
fn the_specs_own_example_of_x_escapes_is_rejected_with_a_useful_message() {
    // TOML basic strings have no \x escape. This is the single most likely
    // first mistake, so the failure needs to be legible.
    let tmp = Tmp::new("escape");
    std::fs::write(
        tmp.path().join("bad.filekind"),
        GOOD.replace(r"magic = 'LNDO\x01'", r#"magic = "LNDO\x01""#),
    )
    .unwrap();
    let out = run(tmp.path(), &["check", "bad.filekind"]);
    assert_eq!(code(&out), 1);
    assert!(
        stderr(&out).contains("escape"),
        "the error should mention the escape sequence:\n{}",
        stderr(&out)
    );
}

#[test]
fn a_missing_spec_is_an_io_style_failure_not_a_panic() {
    let tmp = Tmp::new("missing");
    let out = run(tmp.path(), &["check", "nope.filekind"]);
    assert_ne!(code(&out), 0);
    assert!(!stderr(&out).contains("panicked"), "{}", stderr(&out));

    // An empty directory is a different, friendlier error.
    let out = run(tmp.path(), &["check"]);
    assert!(stderr(&out).contains("filekind init"), "{}", stderr(&out));
}

#[test]
fn ambiguous_directories_ask_rather_than_guess() {
    let tmp = Tmp::new("ambiguous");
    std::fs::write(tmp.path().join("a.filekind"), GOOD).unwrap();
    std::fs::write(tmp.path().join("b.filekind"), GOOD).unwrap();
    let out = run(tmp.path(), &["check"]);
    assert_ne!(code(&out), 0);
    assert!(stderr(&out).contains("2 spec files"), "{}", stderr(&out));
}

#[test]
fn verify_agrees_with_the_bytes() {
    let tmp = Tmp::new("verify");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();
    std::fs::write(tmp.path().join("good.londo"), b"LNDO\x01payload").unwrap();
    std::fs::write(tmp.path().join("bad.londo"), b"XXXX\x01payload").unwrap();

    assert_eq!(code(&run(tmp.path(), &["verify", "good.londo"])), 0);
    assert_eq!(code(&run(tmp.path(), &["verify", "bad.londo"])), 1);

    let out = run(tmp.path(), &["verify", "good.londo", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["matches"], true);
}

#[test]
fn check_json_is_machine_readable() {
    let tmp = Tmp::new("json");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();
    let out = run(tmp.path(), &["check", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["valid"], true);
    assert_eq!(v["derived"]["progid"], "London.Londo.1");
}

#[test]
fn strict_mode_fails_on_warnings() {
    let tmp = Tmp::new("strict");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();
    assert_eq!(code(&run(tmp.path(), &["check"])), 0);
    // GOOD has no icon, which is a warning.
    assert_eq!(code(&run(tmp.path(), &["check", "--strict"])), 1);
}

#[test]
fn preview_prints_one_artifact_to_stdout() {
    let tmp = Tmp::new("preview");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();

    let out = run(tmp.path(), &["preview", "-t", "windows", "-k", "reg"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).starts_with("Windows Registry Editor Version 5.00"));

    let out = run(tmp.path(), &["preview", "-t", "linux", "--list"]);
    assert!(stdout(&out).contains("mime-xml"));
}

#[test]
fn system_scope_targets_hklm() {
    let tmp = Tmp::new("system");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();
    let out = run(
        tmp.path(),
        &["preview", "-t", "windows", "-k", "reg", "--system"],
    );
    assert!(stdout(&out).contains("HKEY_LOCAL_MACHINE"));
    assert!(!stdout(&out).contains("HKEY_CURRENT_USER"));
}

#[test]
fn generated_scripts_are_executable_and_parse_as_sh() {
    let tmp = Tmp::new("scripts");
    std::fs::write(tmp.path().join("s.filekind"), GOOD).unwrap();
    assert_eq!(code(&run(tmp.path(), &["build", "-o", "dist"])), 0);

    for script in ["dist/linux/install.sh", "dist/linux/uninstall.sh"] {
        let path = tmp.path().join(script);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "{script} is not executable");
        }
        // `sh -n` is a syntax check that runs nothing.
        if let Ok(out) = Command::new("sh").arg("-n").arg(&path).output() {
            assert!(
                out.status.success(),
                "{script} is not valid sh: {:?}",
                stderr(&out)
            );
        }
    }
}

#[test]
fn the_icons_command_works_standalone() {
    let tmp = Tmp::new("icons");
    let png = tmp.path().join("src.png");
    std::fs::write(&png, tiny_png()).unwrap();

    let out = run(
        tmp.path(),
        &["icons", "src.png", "-o", "out", "-n", "thing"],
    );
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(tmp.path().join("out/thing.ico").exists());
    assert!(tmp.path().join("out/thing.icns").exists());
    assert!(tmp
        .path()
        .join("out/hicolor/256x256/mimetypes/thing.png")
        .exists());
}

/// A 512×512 PNG built by hand so the test needs no binary fixture.
fn tiny_png() -> Vec<u8> {
    let mut img = image::RgbaImage::new(512, 512);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x / 2) as u8, (y / 2) as u8, 200, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}
