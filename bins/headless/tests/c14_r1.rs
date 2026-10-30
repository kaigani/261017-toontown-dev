//! Permanent M1-C1.4 round-1 regressions against the real stdio binary.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regression test")]

#[test]
fn real_stdio_boundary_regressions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = if cfg!(windows) { "python" } else { "python3" };
    let status = std::process::Command::new(python)
        .arg("-I")
        .arg(root.join("tests/adversarial/c14-r1-mcp.py"))
        .env("TT_REVIEW_BINARY", env!("CARGO_BIN_EXE_toontown-headless"))
        .status()
        .unwrap();
    assert!(status.success(), "the real-binary regression suite failed: {status}");
}
