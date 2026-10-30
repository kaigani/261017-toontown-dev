//! Never-block through MCP (11 §14.5, §14.6): every registered command, called with its first declared example by
//! `run_command` over the stdio protocol, is answered within the watchdog, and no call shows a modal UI (headless:
//! there is none to show; a command that would need one answers `confirmation.*`, `needs_confirmation` or an error).
//! Queued-call responsiveness is measured separately from the jobs those calls start (C1.4 obligation 7). Results:
//! `target/evidence/c14/never-block-mcp-<os>.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "statistics")]

mod support;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::json;
use support::*;

const WATCHDOG: Duration = Duration::from_secs(10);

#[test]
fn every_command_answers_through_mcp_within_the_watchdog() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let reg = tt_executor::registry().unwrap();
    let mut by_status: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_code: BTreeMap<String, u64> = BTreeMap::new();
    let mut latencies = Vec::new();
    let mut jobs = Vec::new();
    let mut ids: Vec<&str> =
        reg.entries().filter(|e| e.spec.kind != tt_registry::Kind::Event).map(|e| e.id()).collect();
    ids.sort_unstable();
    // Commands that end the session's project run last, so the others see one.
    ids.sort_by_key(|id| {
        matches!(*id, "project.close" | "project.open" | "app.quit" | "project.recovery.open" | "project.backup.open")
    });
    let env = c.run("project.create", json!({ "name": "NB", "path": path(&dir.join("NB.animpkg")) }));
    assert_eq!(env["status"], "ok", "{env}");
    for id in &ids {
        let d = reg.get(id).unwrap().describe();
        let params = d["examples"].get(0).and_then(|e| e.get("params")).cloned().unwrap_or_else(|| json!({}));
        let started = Instant::now();
        let req = c.send("tools/call", json!({ "name": "run_command", "arguments": { "id": id, "params": params } }));
        let r = c.wait(req, WATCHDOG).unwrap_or_else(|| panic!("`{id}` was not answered within {WATCHDOG:?}"));
        latencies.push(started.elapsed().as_secs_f64() * 1e3);
        let env = &r["result"]["structuredContent"];
        let status = env["status"].as_str().unwrap_or("protocol-error").to_owned();
        *by_status.entry(status).or_default() += 1;
        if let Some(code) = env["error"]["code"].as_str() {
            *by_code.entry(code.to_owned()).or_default() += 1;
        }
        if let Some(job) = env["result"]["jobId"].as_str() {
            jobs.push(job.to_owned());
        }
    }
    // Eventual job state, measured apart from call responsiveness.
    let jobs_started = Instant::now();
    let mut job_states: BTreeMap<String, u64> = BTreeMap::new();
    for j in &jobs {
        let r = c.call("job_wait", json!({ "jobId": j, "timeoutMs": 30000 }));
        let state = r["structuredContent"]["result"]["record"]["state"]
            .as_str()
            .or_else(|| r["structuredContent"]["result"]["state"].as_str())
            .unwrap_or("unknown")
            .to_owned();
        *job_states.entry(state).or_default() += 1;
    }
    let jobs_ms = jobs_started.elapsed().as_secs_f64() * 1e3;
    latencies.sort_by(f64::total_cmp);
    let at = |q: f64| latencies[((latencies.len() - 1) as f64 * q).round() as usize];
    let report = json!({
        "suite": "11 §14.5 never-block through MCP (stdio protocol, run_command, first declared example)",
        "platform": std::env::consts::OS,
        "commands": ids.len(),
        "byStatus": by_status,
        "byErrorCode": by_code,
        "callLatencyMs": { "p50": at(0.5), "p99": at(0.99), "max": latencies[latencies.len() - 1] },
        "watchdogMs": WATCHDOG.as_millis(),
        "jobs": { "started": jobs.len(), "finalStates": job_states, "waitedMs": jobs_ms },
    });
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join(format!("never-block-mcp-{}.json", std::env::consts::OS)),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert!(!by_status.contains_key("protocol-error"), "{report}");
    assert_eq!(by_status.values().sum::<u64>(), ids.len() as u64);
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn queued_calls_stay_responsive_while_jobs_run() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let env = c.run("project.create", json!({ "name": "Q", "path": path(&dir.join("Q.animpkg")) }));
    let scene = env["result"]["sceneId"].as_str().unwrap().to_owned();
    // Queue 50 calls at once (pipelined), half of them starting export jobs.
    let started = Instant::now();
    let mut sent = Vec::new();
    for i in 0..50 {
        let id = if i % 2 == 0 {
            c.send("tools/call", json!({ "name": "set_properties", "arguments": { "targets": [scene], "props": { "meta": { "i": i } } } }))
        } else {
            c.send("tools/call", json!({ "name": "run_command", "arguments": { "id": "doc.package.export", "params": { "path": path(&dir.join(format!("e{i}.animpkg"))) } } }))
        };
        sent.push((id, Instant::now()));
    }
    let mut answered = Vec::new();
    let mut revs = Vec::new();
    for (id, at) in &sent {
        let r = c.wait(*id, WATCHDOG).unwrap();
        answered.push(at.elapsed().as_secs_f64() * 1e3);
        if let Some(rev) = r["result"]["structuredContent"]["documentRev"].as_u64() {
            revs.push(rev);
        }
    }
    let all_answered_ms = started.elapsed().as_secs_f64() * 1e3;
    assert!(revs.windows(2).all(|w| w[0] <= w[1]), "pipelined calls apply in the order sent: {revs:?}");
    answered.sort_by(f64::total_cmp);
    let report = json!({
        "queued": 50, "allAnsweredMs": all_answered_ms,
        "perCallMs": { "p50": answered[answered.len() / 2], "max": answered[answered.len() - 1] },
    });
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/evidence/c14");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join(format!("queued-calls-{}.json", std::env::consts::OS)),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    c.close().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
