//! M3 C3.3 round 3 (builder): through MCP, `job_status` of a cancelled range job answers its state, its typed error
//! and its recovery receipt (C33-R3-01, C33-R2-02).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::json;

#[test]
fn job_status_of_a_cancelled_job_carries_state_error_and_warnings() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    std::fs::create_dir(dir.join("out")).unwrap();
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let started =
        client.run("render.range.run", json!({ "from": 1, "to": 48, "width": 512, "directory": dir.join("out") }));
    let id = started["result"]["jobId"].as_str().unwrap().to_owned();
    let cancel = client.run("job.cancel", json!({ "jobId": id }));
    assert!(cancel.get("error").is_none_or(serde_json::Value::is_null), "{cancel}");
    let rec = h.jobs().wait(&id, std::time::Duration::from_secs(60)).unwrap().0;
    assert_eq!(rec.state, tt_executor::JobState::Cancelled);
    let answer = client.call("job_status", json!({ "jobId": id }));
    client.close().unwrap();
    let result = &answer["structuredContent"]["result"];
    let r = result.get("record").unwrap_or(result);
    let r = if r.is_array() { &r[0] } else { r };
    assert_eq!(r["state"], "cancelled", "{answer}");
    assert_eq!(r["error"]["code"], "job.cancelled", "{answer}");
    assert!(
        r["warnings"].as_array().is_some_and(|w| w.iter().any(|w| w["code"] == "render.stagingDiscarded")),
        "{answer}"
    );
    assert_eq!(std::fs::read_dir(dir.join("out")).unwrap().count(), 0, "nothing published, nothing left");
}
