//! Permanent round-three protocol regressions against the locally built binary.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression")]

#[test]
fn real_stdio_round_three_regressions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = if cfg!(windows) { "python" } else { "python3" };
    let status = std::process::Command::new(python)
        .arg("-I")
        .arg(root.join("tests/adversarial/c14-r3-mcp.py"))
        .env("TT_REVIEW_BINARY", env!("CARGO_BIN_EXE_toontown-headless"))
        .status()
        .unwrap();
    assert!(status.success(), "round-three protocol suite failed: {status}");
}
