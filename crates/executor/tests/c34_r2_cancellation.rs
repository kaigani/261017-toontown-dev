//! Independent request cancellation boundary probes for C3.4 round 2.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod c13_support;
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::Invocation;

#[test]
fn already_cancelled_mutation_never_commits() {
    let h = fixture();
    let before = rev(&h);
    let (env, _) = h.invoke_served_until(
        Invocation::user("element.create", json!({"type":"peg","name":"Cancelled","sceneId":"scene:#1"})),
        &|| true,
    );
    assert!(!env.is_ok(), "already cancelled mutation committed: {env:?}");
    assert_eq!(rev(&h), before);
}

#[test]
fn direct_job_wait_observes_cancellation_probe() {
    let h = fixture();
    let dir = c13_support::tmp().join("direct-wait");
    let submitted =
        ok(&h, "render.range.run", json!({"from":1,"to":1000,"width":32,"directory":dir,"createDirs":true}));
    let job = submitted["jobId"].as_str().unwrap();
    let started = Instant::now();
    let (env, _) = h.invoke_served_until(Invocation::user("job.wait", json!({"jobId":job,"timeoutMs":500})), &|| true);
    let elapsed = started.elapsed();
    ok(&h, "job.cancel", json!({"jobId":job}));
    h.jobs().wait(job, Duration::from_secs(60)).unwrap();
    eprintln!("direct_job_wait elapsed_ms={} envelope={env:?}", elapsed.as_secs_f64() * 1000.0);
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"));
    assert_eq!(env.error.unwrap().details["reason"], "cancelled");
}
