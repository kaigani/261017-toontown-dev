//! A separate local process holds a real range job during recovery.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[test]
fn child_job() -> Result<(), Box<dyn std::error::Error>> {
    let Some(dir) = std::env::var_os("TT_C33_R3_CHILD_DIR") else { return Ok(()) };
    let dir = PathBuf::from(dir);
    let h = fixture();
    tt_vector::work::hooks::set_spin_micros(100_000);
    let start =
        ok(&h, "render.range.run", json!({"from":1,"to":48,"width":32,"directory":dir,"namePattern":"child-{frame}"}));
    let id = start["jobId"].as_str().ok_or("job id")?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while !dir.join("release-child").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    ok(&h, "job.cancel", json!({"jobId":id}));
    tt_vector::work::hooks::set_spin_micros(0);
    h.jobs().wait(id, Duration::from_secs(60))?;
    Ok(())
}
#[test]
fn separate_live_process_holding_job_is_untouched() -> Result<(), Box<dyn std::error::Error>> {
    let dir = c13_support::tmp();
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(["--exact", "child_job", "--nocapture"])
        .env("TT_C33_R3_CHILD_DIR", &dir)
        .spawn()?;
    let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let side = loop {
            let found = std::fs::read_dir(&dir)?
                .filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().ends_with(".job"));
            if let Some(e) = found
                && std::fs::read(e.path()).ok().is_some_and(|b| serde_json::from_slice::<serde_json::Value>(&b).is_ok())
            {
                break e.path();
            }
            if Instant::now() > deadline {
                return Err("child sidecar timeout".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let bytes = std::fs::read(&side)?;
        let h = fixture();
        let result = c13_support::job_ok(
            &h,
            "render.range.run",
            json!({"from":1,"to":1,"width":32,"directory":dir,"namePattern":"parent-{frame}"}),
        );
        assert_eq!(std::fs::read(&side)?, bytes);
        assert!(!result["warnings"].to_string().contains("render.staleStagingRemoved"));
        Ok(())
    })();
    std::fs::write(dir.join("release-child"), b"release")?;
    let status = child.wait()?;
    assert!(status.success());
    outcome
}
