//! M3 C3.4 round 1 (builder, orchestrator item O-1): an MCP client cancels its `batch` request (`notifications/cancelled`)
//! while a `wait: true` export step waits. No answer is sent (11 §4), the later step changes nothing, and the job stays
//! the job system's: listed in `job_status`, cancellable, and leaving no staging once terminal (ADR-0020 decision 8).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use std::time::{Duration, Instant};

use serde_json::{Value, json};

fn staged(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.contains(".tt-partial-") || n.contains(".tt-backup-"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_cancelled_batch_request_stops_its_steps_and_leaves_the_job_to_the_job_system() {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let out = dir.join("out");
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let id = client.send(
        "tools/call",
        json!({ "name": "batch", "arguments": {
            "label": "export then edit",
            "atomic": false,
            "steps": [
                { "id": "render.range.run", "params": { "from": 1, "to": 1000, "width": 256, "directory": out, "createDirs": true, "wait": true } },
                { "id": "element.create", "params": { "type": "peg", "name": "Late", "sceneId": "scene:#1" } }
            ]
        } }),
    );
    let start = Instant::now();
    while staged(&out).is_empty() {
        assert!(start.elapsed() < Duration::from_secs(60), "the export never staged");
        std::thread::sleep(Duration::from_millis(5));
    }
    client.notify("notifications/cancelled", json!({ "requestId": id, "reason": "test" }));
    // No answer to a cancelled request; the server stays responsive and the later step never ran.
    assert!(client.wait(id, Duration::from_millis(1500)).is_none(), "a cancelled request is not answered");
    let late = client.run("doc.object.get", json!({ "ref": "scene:#1/element:\"Late\"" }));
    assert_eq!(late["error"]["code"], "ref.notFound", "{late}");
    let jobs = client.call("job_status", json!({}));
    let list: Vec<Value> = jobs["structuredContent"]["result"].as_array().cloned().unwrap_or_default();
    let job = list
        .iter()
        .find(|j| j["commandId"] == "render.range.run")
        .unwrap_or_else(|| panic!("the job is listed: {jobs}"))
        .clone();
    assert_eq!(job["state"], "running", "{job}");
    let id = job["jobId"].as_str().unwrap().to_owned();
    let cancel = client.run("job.cancel", json!({ "jobId": id }));
    assert!(cancel.get("error").is_none_or(Value::is_null), "{cancel}");
    let rec = h.jobs().wait(&id, Duration::from_secs(60)).unwrap().0;
    assert_eq!(rec.state, tt_executor::JobState::Cancelled, "{rec:?}");
    client.close().unwrap();
    assert!(staged(&out).is_empty(), "no staging once terminal: {:?}", staged(&out));
    assert!(std::fs::read_dir(&out).map(|rd| rd.count()).unwrap_or(0) == 0, "nothing published");
}
