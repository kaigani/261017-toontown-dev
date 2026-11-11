//! Independent wire-level silence and session reuse after cancellation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod support;
use serde_json::json;
use std::time::Duration;
#[test]
fn cancelled_calls_are_silent_and_following_calls_are_answered() {
    for kind in ["waited", "batch", "nonjob"] {
        let dir = support::tmp();
        let h = support::executor(&dir.join("user"));
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let mut options = support::opts(&dir);
        options.policy = Some(tt_executor::agents::Policy::Autonomous);
        let (mut c, _) = support::Client::connect(&h, options, json!({}));
        let out = dir.join("out");
        let render = json!({"from":1,"to":100000,"width":256,"directory":out,"createDirs":true,"wait":true});
        let args = match kind {
            "waited" => json!({"name":"run_command","arguments":{"id":"render.range.run","params":render}}),
            "batch" => {
                json!({"name":"batch","arguments":{"label":"review","atomic":false,"steps":[{"id":"render.range.run","params":render}]}})
            }
            _ => {
                json!({"name":"run_command","arguments":{"id":"render.frame.run","params":{"frame":1,"width":8192,"height":8192,"output":"file","path":dir.join("large.png")}}})
            }
        };
        let id = c.send("tools/call", args);
        std::thread::sleep(Duration::from_millis(30));
        c.notify("notifications/cancelled", json!({"requestId":id}));
        assert!(c.wait(id, Duration::from_millis(1500)).is_none(), "cancelled {kind} answered");
        assert_eq!(c.run("doc.state.get", json!({}))["status"], "ok");
        let jobs = c.run("job.list", json!({}));
        for j in jobs["result"].as_array().unwrap() {
            c.run("job.cancel", json!({"jobId":j["jobId"]}));
            h.jobs().wait(j["jobId"].as_str().unwrap(), Duration::from_secs(60)).unwrap();
        }
        let audit = c.run("agent.activity.list", json!({"limit":100}));
        let command = match kind {
            "waited" => "render.range.run",
            "batch" => "batch",
            _ => "render.frame.run",
        };
        let entries: Vec<_> =
            audit["result"].as_array().unwrap().iter().filter(|a| a["commandId"] == command).collect();
        assert_eq!(entries.len(), 1, "{audit}");
        eprintln!("WIRE {kind} audit_status={} errorCode={}", entries[0]["status"], entries[0]["errorCode"]);
        c.close().unwrap();
    }
}
