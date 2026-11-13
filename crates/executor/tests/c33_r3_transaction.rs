//! Independent refused rollback and retry probe.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
use std::{path::Path, sync::Arc};
use tt_persist::lock::{Boundary, hooks};
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        hooks::set(None);
    }
}
#[test]
fn rollback_retains_original_and_retries_without_partial_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let _reset = Reset;
    let h = fixture();
    let dir = c13_support::tmp();
    std::fs::write(dir.join("r-1.png"), b"independent original")?;
    hooks::set(Some(Arc::new(|at: Boundary, path: &Path| {
        if at == Boundary::RangePublish && path.file_name().is_some_and(|n| n == "r-2.png") {
            let _ = std::fs::create_dir(path);
        }
        if at == Boundary::RangeRestore && path.file_name().is_some_and(|n| n == "r-1.png") {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::create_dir(path);
        }
    })));
    let started = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":2,"width":32,"directory":dir,"namePattern":"r-{frame}","onConflict":"overwrite"}),
    );
    let rec = c13_support::wait(&h, &started);
    hooks::set(None);
    assert_eq!(rec["state"], "failed");
    assert_eq!(rec["error"]["code"], "export.rollbackIncomplete");
    assert!(rec["error"]["details"]["unrestored"].to_string().contains("r-1.png"));
    let entries = std::fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
    let backup = entries
        .iter()
        .find(|e| e.file_name().to_string_lossy().starts_with(".r-1.png.tt-backup-"))
        .ok_or("backup missing")?;
    assert_eq!(std::fs::read(backup.path())?, b"independent original");
    assert!(entries.iter().any(|e| e.file_name().to_string_lossy().ends_with(".job")));
    assert!(!entries.iter().any(|e| e.file_name().to_string_lossy().ends_with("manifest.json")));
    std::fs::remove_dir(dir.join("r-1.png"))?;
    std::fs::remove_dir(dir.join("r-2.png"))?;
    let result = c13_support::job_ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":1,"width":32,"directory":dir,"namePattern":"retry-{frame}"}),
    );
    assert_eq!(std::fs::read(dir.join("r-1.png"))?, b"independent original");
    assert!(result["warnings"].to_string().contains("r-1.png"));
    Ok(())
}

#[test]
fn failed_receipt_survives_save_open_only_in_existing_session() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = c13_support::tmp();
    std::fs::create_dir(dir.join("f-1.png"))?;
    let started = ok(
        &h,
        "render.range.run",
        json!({"from":1,"to":1,"width":32,"directory":dir,"namePattern":"f-{frame}","onConflict":"overwrite"}),
    );
    let before = c13_support::wait(&h, &started);
    assert_eq!(before["state"], "failed");
    assert_eq!(before["error"]["code"], "export.targetInvalid");
    let project = dir.join("session.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":project}));
    c13_support::job_ok(&h, "project.open", json!({"path":project}));
    let after = ok(&h, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":1000}));
    let after = after.get("record").unwrap_or(&after);
    assert_eq!(after["warnings"], before["warnings"]);
    assert_eq!(after["error"], before["error"]);
    let fresh = fixture();
    assert_eq!(
        common::err(&fresh, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":1000})).code,
        "job.notFound"
    );
    Ok(())
}
