//! End-to-end tests of the built `toontown-headless` binary (exit codes and output streams).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_toontown-headless"))
}

#[test]
fn version_exits_zero() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("toontown-headless "), "{stdout}");
}

#[test]
fn selftest_exits_zero_with_json_report() {
    let out = bin().args(["--selftest", "--json"]).output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["ok"], true);
}

#[test]
fn selftest_runs_with_display_environment_removed() {
    let out = bin()
        .args(["--selftest"])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn usage_error_exits_two() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn mcp_stdio_not_yet_available_exits_three() {
    let out = bin().args(["--headless", "--mcp-stdio"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("M1"), "{stderr}");
}
