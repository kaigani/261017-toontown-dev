//! Independent ownership and terminal diagnostic checks for round three.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;

#[test]
fn positive_ownership_and_published_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = c13_support::tmp();
    let token = "012345abcdef";
    let names = [
        ".x.tt-partial-draft",
        ".x.tt-partial-aaaaaaaaaaaa",
        ".x.tt-backup-aaaaaaaaaaaa",
        ".tt-partial-bbbbbbbbbbbb.job",
        ".unlisted.tt-partial-012345abcdef",
    ];
    for name in names {
        std::fs::write(dir.join(name), b"user bytes")?;
    }
    let side = json!({"kind":"toontown.render.range.staging","version":1,"token":token,
        "jobId":"dead","head":"unknown","productVersion":"0.0.1",
        "host":tt_persist::host::host(),"pid":std::process::id(),"published":true,
        "targets":["owned"],"staged":[".owned.tt-partial-012345abcdef"],
        "backups":[".owned.tt-backup-012345abcdef"]});
    std::fs::write(dir.join(".tt-partial-012345abcdef.job"), side.to_string())?;
    std::fs::write(dir.join("owned"), b"published bytes")?;
    std::fs::write(dir.join(".owned.tt-backup-012345abcdef"), b"old bytes")?;
    std::fs::write(dir.join(".owned.tt-partial-012345abcdef"), b"stage bytes")?;
    let result = c13_support::job_ok(&h, "render.range.run", json!({"from":1,"to":1,"width":32,"directory":dir}));
    let receipt = result["warnings"].to_string();
    for name in names {
        assert_eq!(std::fs::read(dir.join(name))?, b"user bytes");
        assert!(!receipt.contains(name), "unowned name reported: {receipt}");
    }
    assert_eq!(std::fs::read(dir.join("owned"))?, b"published bytes");
    assert!(!dir.join(".owned.tt-backup-012345abcdef").exists());
    assert!(!dir.join(".owned.tt-partial-012345abcdef").exists());
    assert!(receipt.contains("render.staleStagingRemoved"));
    Ok(())
}

#[test]
fn cancelled_record_keeps_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = c13_support::tmp();
    tt_vector::work::hooks::set_spin_micros(20_000);
    let started = ok(&h, "render.range.run", json!({"from":1,"to":48,"width":32,"directory":dir}));
    let id = started["jobId"].as_str().ok_or("job id")?;
    ok(&h, "job.cancel", json!({"jobId":id}));
    tt_vector::work::hooks::set_spin_micros(0);
    let record = h.jobs().wait(id, std::time::Duration::from_secs(60))?.0;
    assert_eq!(record.state, tt_executor::JobState::Cancelled);
    let listed = ok(&h, "job.list", json!({"jobId":id}));
    let waited = ok(&h, "job.wait", json!({"jobId":id,"timeoutMs":1000}));
    assert!(
        record.error.is_some(),
        "cancelled record must retain typed cancellation error; list={listed}; wait={waited}"
    );
    Ok(())
}
