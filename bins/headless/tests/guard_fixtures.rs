//! The headless dependency guard (scripts/check-headless-deps.py) must reject real executables that import a
//! graphics/GUI library (C01-R1-06). Each fixture is compiled with rustc and inspected, never executed.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

/// Compiles `fixture` into the test's temporary directory and returns the guard's (exit code, stdout).
fn guard_on_fixture(fixture: &Path, name: &str) -> Result<(Option<i32>, String), String> {
    let exe = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let built = Command::new("rustc")
        .args(["--edition", "2024", "-O"])
        .arg(fixture)
        .arg("-o")
        .arg(&exe)
        .current_dir(root())
        .output()
        .map_err(|e| format!("rustc is not on PATH: {e}"))?;
    if !built.status.success() {
        return Err(format!("fixture build failed: {}", String::from_utf8_lossy(&built.stderr)));
    }
    let out = Command::new(python())
        .args(["-I", "scripts/check-headless-deps.py"])
        .arg(&exe)
        .current_dir(root())
        .output()
        .map_err(|e| format!("python is not on PATH: {e}"))?;
    Ok((out.status.code(), String::from_utf8_lossy(&out.stdout).into_owned()))
}

#[cfg(windows)]
#[test]
fn rejects_a_real_direct_composition_import() {
    let (code, stdout) =
        guard_on_fixture(&root().join("tests/adversarial/fixtures/graphics_import.rs"), "graphics-import").unwrap();
    assert_eq!(code, Some(1), "{stdout}");
    assert!(stdout.contains("forbidden libraries: ['dcomp.dll']"), "{stdout}");
}

#[cfg(target_os = "macos")]
#[test]
fn rejects_a_real_appkit_import() {
    let (code, stdout) =
        guard_on_fixture(&root().join("tests/fixtures/headless-guard/appkit_import.rs"), "appkit-import").unwrap();
    assert_eq!(code, Some(1), "{stdout}");
    assert!(stdout.contains("AppKit.framework"), "{stdout}");
}
