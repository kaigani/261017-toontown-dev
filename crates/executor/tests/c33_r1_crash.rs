//! Process-death probe for sequence staging cleanup (charter item 6).
mod common;

use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn crash_child_receipt() {
    let Some(dir) = std::env::var_os("TT_C33_R1_CRASH_DIRECTORY") else { return };
    let h = common::fixture();
    tt_vector::work::hooks::set_spin_micros(20_000);
    let job = common::ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":48,"width":512,"directory":dir.to_string_lossy(),"createDirs":true}),
    );
    common::ok(&h, "job.wait", json!({"jobId":job["jobId"],"timeoutMs":60000}));
}

#[test]
fn killed_sequence_leaves_no_staged_files() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("c33-r1-crash-{}", tt_persist::fsx::random_token(6)));
    std::fs::create_dir_all(&dir)?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--exact", "crash_child_receipt", "--nocapture"])
        .env("TT_C33_R1_CRASH_DIRECTORY", &dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let start = Instant::now();
    let mut saw_staged = false;
    while start.elapsed() < Duration::from_secs(15) {
        if let Ok(files) = std::fs::read_dir(&dir) {
            saw_staged = files.filter_map(Result::ok).any(|f| f.file_name().to_string_lossy().contains("tt-partial"));
        }
        if saw_staged {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill()?;
    child.wait()?;
    assert!(saw_staged, "child never reached the staged-file boundary");
    let remaining: Vec<_> = std::fs::read_dir(&dir)?.filter_map(Result::ok).map(|f| f.file_name()).collect();
    assert!(remaining.is_empty(), "charter item 6: killed sequence retained {remaining:?} in {}", dir.display());
    Ok(())
}
