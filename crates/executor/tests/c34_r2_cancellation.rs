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
        ok(&h, "render.range.run", json!({"from":1,"to":100000,"width":32,"directory":dir,"createDirs":true}));
    let job = submitted["jobId"].as_str().unwrap();
    let started = Instant::now();
    let env = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            h.invoke_served_until(Invocation::user("job.wait", json!({"jobId":job,"timeoutMs":500})), &|| true).0
        });
        std::thread::sleep(Duration::from_millis(20));
        let other = run(&h, "job.wait", json!({"jobId":job,"timeoutMs":0}));
        assert!(other.is_ok());
        assert_eq!(other.result.as_ref().unwrap()["jobId"], job);
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
        first.join().unwrap()
    });
    let elapsed = started.elapsed();
    ok(&h, "job.cancel", json!({"jobId":job}));
    h.jobs().wait(job, Duration::from_secs(60)).unwrap();
    let terminal = ok(&h, "job.wait", json!({"jobId":job,"timeoutMs":0}));
    assert_eq!(terminal["state"], "cancelled");
    eprintln!("direct_job_wait elapsed_ms={} status={:?}", elapsed.as_secs_f64() * 1000.0, env.status);
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"));
    assert_eq!(env.error.unwrap().details["reason"], "cancelled");
}

#[test]
fn retry_cache_and_late_cancellation_are_stable_for_three_thousand_calls() {
    let h = fixture();
    for i in 0..3000 {
        let mut inv = Invocation::user("doc.state.get", json!({}));
        inv.options.request_id = Some(format!("r2-{i}"));
        let (first, _) = h.invoke_served_until(inv.clone(), &|| true);
        let before = rev(&h);
        let (again, cached) = h.invoke_served_until(inv, &|| false);
        assert!(cached, "committed fast call must be remembered");
        assert_eq!(serde_json::to_value(&first).unwrap(), serde_json::to_value(&again).unwrap());
        assert_eq!(rev(&h), before);
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
    }
}

#[test]
fn fast_commit_cancel_retry_matrix_preserves_one_change_and_one_answer() {
    for trial in 0..100 {
        let h = fixture();
        let events = h.subscribe();
        let mut inv = Invocation::user("element.create", json!({"type":"peg","name":"Race","sceneId":"scene:#1"}));
        inv.options.request_id = Some(format!("race-{trial}"));
        let start = Instant::now();
        let delay = [Duration::ZERO, Duration::from_micros(50), Duration::from_millis(20)][trial % 3];
        let (first, _) = h.invoke_served_until(inv.clone(), &|| start.elapsed() >= delay);
        let emitted: Vec<_> = events.try_iter().collect();
        if first.is_ok() {
            assert!(first.undo.is_some());
            assert_eq!(emitted.iter().filter(|e| e.name == "doc.changed").count(), 1);
            let at = rev(&h);
            let (again, cached) = h.invoke_served_until(inv, &|| false);
            assert!(cached);
            assert_eq!(serde_json::to_value(first).unwrap(), serde_json::to_value(again).unwrap());
            assert_eq!(rev(&h), at);
            assert_eq!(events.try_iter().filter(|e| e.name == "doc.changed").count(), 0);
        } else {
            assert_eq!(rev(&h), 1);
            let (retry, _) = h.invoke_served_until(inv.clone(), &|| false);
            assert!(retry.is_ok());
            assert!(h.invoke_served_until(inv, &|| false).1);
            assert_eq!(rev(&h), 2);
        }
    }
}
