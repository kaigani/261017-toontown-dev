//! Independent waited-step latency and queue probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};

fn staged(dir: &std::path::Path) {
    let at = Instant::now();
    while !std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some()) {
        assert!(at.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn one_hundred_wait_cancellations_settle_jobs_and_leave_handle_usable() {
    let h = fixture();
    let mut ms = Vec::new();
    for trial in 0..100 {
        let dir = c13_support::tmp().join(format!("latency-{trial}"));
        let flag = AtomicBool::new(false);
        let env = std::thread::scope(|s| {
            let t = s.spawn(|| {
                h.invoke_served_until(
                    Invocation::user(
                        "render.range.run",
                        json!({"from":1,"to":10000,"width":32,"directory":dir,"createDirs":true,"wait":true}),
                    ),
                    &|| flag.load(Ordering::SeqCst),
                )
                .0
            });
            staged(&dir);
            let at = Instant::now();
            flag.store(true, Ordering::SeqCst);
            let env = t.join().unwrap();
            ms.push(at.elapsed().as_secs_f64() * 1000.0);
            env
        });
        let error = env.error.unwrap();
        assert_eq!(error.code, "command.timeout");
        assert_eq!(error.details["reason"], "cancelled");
        let job = error.details["jobId"].as_str().unwrap();
        assert!(run(&h, "job.list", json!({"jobId":job})).is_ok());
        ok(&h, "job.cancel", json!({"jobId":job}));
        let record = h.jobs().wait(job, Duration::from_secs(60)).unwrap().0;
        assert_eq!(record.state, tt_executor::JobState::Cancelled);
        assert!(
            !std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().contains(".tt-partial-"))
        );
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
    }
    ms.sort_by(f64::total_cmp);
    eprintln!(
        "LATENCY_MS n=100 min={} p50={} p95={} p99={} max={} samples={ms:?}",
        ms[0], ms[49], ms[94], ms[98], ms[99]
    );
}

#[test]
fn queued_cancelled_request_is_skipped_and_following_read_survives() {
    for (cancel_batch, stop_on_error) in [(false, true), (true, true), (true, false)] {
        let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(1000) });
        let dir = c13_support::tmp().join("blocked");
        let batch:BatchRequest=serde_json::from_value(json!({"label":"review","atomic":false,"stopOnError":stop_on_error,"steps":[{"id":"render.range.run","params":{"from":1,"to":10000,"width":32,"directory":dir,"createDirs":true,"wait":true}},{"id":"element.create","params":{"type":"peg","name":"Late","sceneId":"scene:#1"}}]})).unwrap();
        let mut queued_inv =
            Invocation::user("element.create", json!({"type":"peg","name":"QueuedCancelled","sceneId":"scene:#1"}));
        queued_inv.options.request_id = Some("queued-retry".to_owned());
        let flag = AtomicBool::new(false);
        std::thread::scope(|s| {
            let batch_thread = s.spawn(|| h.batch_until(batch, Origin::User, &|| flag.load(Ordering::SeqCst)));
            staged(&dir);
            let at = Instant::now();
            let cancelled = s.spawn(|| h.invoke_served_until(queued_inv.clone(), &|| true).0);
            std::thread::sleep(Duration::from_millis(50));
            let read = s.spawn(|| {
                let start = Instant::now();
                let env = run(&h, "doc.state.get", json!({}));
                (env, start.elapsed())
            });
            if cancel_batch {
                flag.store(true, Ordering::SeqCst);
            }
            let (read_env, read_elapsed) = read.join().unwrap();
            assert!(read_env.is_ok());
            eprintln!(
                "READ_MS cancellation={cancel_batch} stopOnError={stop_on_error} elapsed={}",
                read_elapsed.as_secs_f64() * 1000.0
            );
            assert!(read_elapsed < Duration::from_millis(1000));
            eprintln!("BLOCKING_MS cancellation={cancel_batch} read_and_queue={}", at.elapsed().as_secs_f64() * 1000.0);
            assert!(!cancelled.join().unwrap().is_ok());
            let env = batch_thread.join().unwrap();
            let steps = env.steps.unwrap();
            let job = steps[0].error.as_ref().unwrap().details["jobId"].as_str().unwrap();
            ok(&h, "job.cancel", json!({"jobId":job}));
            h.jobs().wait(job, Duration::from_secs(60)).unwrap();
        });
        assert_eq!(err(&h, "doc.object.get", json!({"ref":"scene:#1/element:\"Late\""})).code, "ref.notFound");
        assert_eq!(
            err(&h, "doc.object.get", json!({"ref":"scene:#1/element:\"QueuedCancelled\""})).code,
            "ref.notFound"
        );
        let before = rev(&h);
        let (retry, cached) = h.invoke_served(queued_inv.clone());
        assert!(retry.is_ok());
        assert!(!cached);
        assert!(retry.undo.is_some());
        assert!(h.invoke_served(queued_inv).1);
        assert_eq!(rev(&h), before + 1);
    }
}

#[test]
fn atomic_and_dry_run_cancel_mid_batch_discard_changes() {
    for dry in [false, true] {
        let h = fixture();
        let before = rev(&h);
        let steps:Vec<Value>=(0..1000).map(|i|json!({"id":"element.create","params":{"type":"peg","name":format!("Step{i}"),"sceneId":"scene:#1"}})).collect();
        let req: BatchRequest =
            serde_json::from_value(json!({"label":"review","atomic":true,"dryRun":dry,"steps":steps})).unwrap();
        let start = Instant::now();
        let env = h.batch_until(req, Origin::User, &|| start.elapsed() > Duration::from_millis(5));
        eprintln!("ATOMIC dry={dry} status={:?} error={:?}", env.status, env.error);
        assert!(!env.is_ok());
        assert_eq!(rev(&h), before);
    }
}

#[test]
fn discarded_queued_cancellation_does_not_report_a_dead_document_thread() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(1000) });
    let dir = c13_support::tmp().join("queue-error");
    let req:BatchRequest=serde_json::from_value(json!({"label":"queue error","atomic":false,"steps":[{"id":"render.range.run","params":{"from":1,"to":10000,"width":32,"directory":dir,"createDirs":true,"wait":true}}]})).unwrap();
    let refused = std::thread::scope(|s| {
        let blocker = s.spawn(|| h.batch(req, Origin::User));
        staged(&dir);
        let refused = h
            .invoke_served_until(
                Invocation::user("element.create", json!({"type":"peg","name":"MustNotBegin","sceneId":"scene:#1"})),
                &|| true,
            )
            .0;
        let env = blocker.join().unwrap();
        let steps = env.steps.unwrap();
        let job = steps[0].error.as_ref().unwrap().details["jobId"].as_str().unwrap();
        ok(&h, "job.cancel", json!({"jobId":job}));
        h.jobs().wait(job, Duration::from_secs(60)).unwrap();
        refused
    });
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    let error = refused.error.unwrap();
    eprintln!("QUEUED_ERROR code={} message={}", error.code, error.message);
    assert_ne!(error.code, "internal", "intentional request cancellation is not a dead document thread");
}
