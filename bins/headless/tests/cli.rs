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
fn mcp_stdio_exits_zero_when_the_client_closes_its_input() {
    let dir = std::env::temp_dir().join(format!("tt-headless-cli-{}", std::process::id()));
    let out = bin()
        .args(["--headless", "--mcp-stdio", "--user-dir"])
        .arg(&dir)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "nothing is written before the client speaks");
}

#[test]
fn a_bad_policy_is_a_usage_error() {
    let out = bin().args(["--mcp-stdio", "--policy", "root"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
