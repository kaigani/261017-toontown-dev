//! Independent round-four cancelled record and session read-back.
mod c13_support;
mod common;
use common::{fixture, ok};
use serde_json::json;
#[test]
fn cancelled_record_keeps_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let h = fixture();
    let dir = c13_support::tmp();
    tt_vector::work::hooks::set_spin_micros(20_000);
    let started = ok(&h, "render.range.run", json!({"from":1,"to":48,"width":32,"directory":dir}));
    let id = started["jobId"].as_str().ok_or("job id")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !h.jobs().list(Some(id), None).iter().any(|r| r.message.starts_with("Rendering frame")) {
        if std::time::Instant::now() > deadline {
            return Err("range did not enter frame work".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    std::thread::sleep(std::time::Duration::from_millis(30));
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
    for response in [&listed, &waited] {
        let rows = response.get("jobs").unwrap_or(response);
        let row = rows.get("record").unwrap_or(rows);
        let row = if row.is_array() { &row[0] } else { row };
        assert_eq!(row["state"], "cancelled", "{response}");
        assert_eq!(row["error"]["code"], "job.cancelled", "{response}");
        assert!(row["warnings"].to_string().contains("render.stagingDiscarded"));
    }
    let project = dir.join("cancelled.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":project}));
    c13_support::job_ok(&h, "project.open", json!({"path":project}));
    let after = ok(&h, "job.wait", json!({"jobId":id,"timeoutMs":1000}));
    let before = waited.get("record").unwrap_or(&waited);
    let after = after.get("record").unwrap_or(&after);
    assert_eq!(after["state"], "cancelled");
    assert_eq!(before["error"], after["error"]);
    assert_eq!(before["warnings"], after["warnings"]);
    assert_eq!(common::err(&fixture(), "job.wait", json!({"jobId":id,"timeoutMs":1000})).code, "job.notFound");
    Ok(())
}
