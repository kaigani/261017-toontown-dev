//! Independent scoped closure probes. Run serially: work counters are process-wide.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};
use tt_vector::work::hooks as work;
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn edit() -> Invocation {
    Invocation::user("element.create", json!({"type":"peg","name":"Closure","sceneId":"scene:#1"}))
}
#[test]
fn deadline_precedes_reply_but_retry_recovers_exactly_one_commit() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(200) });
    let before = rev(&h);
    let n = AtomicUsize::new(0);
    let mut inv = edit();
    inv.options.request_id = Some("r4-deadline".into());
    let (env, _) = h.invoke_served_until(inv.clone(), &|| {
        if n.fetch_add(1, Ordering::SeqCst) == 1 {
            std::thread::sleep(Duration::from_millis(400));
        }
        false
    });
    let e = env.error.unwrap();
    assert_eq!(e.code, "command.timeout");
    assert_ne!(e.details["reason"], "cancelled");
    assert_eq!(rev(&h), before + 1);
    let (answer, cached) = h.invoke_served(inv);
    assert!(answer.is_ok(), "{answer:?}");
    assert!(cached);
    assert!(answer.undo.is_some());
    assert_eq!(rev(&h), before + 1);
    let settled = Instant::now();
    while work::outstanding_tokens() != 0 && settled.elapsed() < Duration::from_secs(1) {
        std::thread::yield_now();
    }
    assert_eq!(work::outstanding_tokens(), 0);
    assert_eq!(work::queued_tokens(), 0);
}
#[test]
fn bounded_questions_latency_distribution_and_latch() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut samples = Vec::new();
    for trial in 0..100 {
        let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(800) });
        let (tx, rx) = std::sync::mpsc::channel();
        let n = AtomicUsize::new(0);
        let elapsed = std::thread::scope(|s| {
            let call = s.spawn(|| {
                h.invoke_served_until(edit(), &|| {
                    if n.fetch_add(1, Ordering::SeqCst) == 1 {
                        tx.send(()).unwrap();
                        std::thread::sleep(Duration::from_millis(350));
                    }
                    false
                })
            });
            rx.recv_timeout(Duration::from_secs(10)).unwrap();
            let start = Instant::now();
            assert!(run(&h, "doc.state.get", json!({})).is_ok());
            let elapsed = start.elapsed();
            call.join().unwrap();
            elapsed
        });
        eprintln!("LATENCY trial={trial} ms={}", elapsed.as_secs_f64() * 1000.0);
        samples.push(elapsed);
    }
    samples.sort();
    eprintln!(
        "LATENCY min={:?} median={:?} p95={:?} p99={:?} max={:?} bound=200ms",
        samples[0], samples[50], samples[94], samples[98], samples[99]
    );
    for (stall, block_index) in [(240, 1), (260, 1), (350, 2)] {
        let h = fixture();
        let n = AtomicUsize::new(0);
        work::reset_asks();
        let steps = (0..1000)
            .map(|_| json!({"id":"edit.selection.set","params":{"context":"drawing","ids":[]}}))
            .collect::<Vec<_>>();
        let req: BatchRequest =
            serde_json::from_value(json!({"label":"r4 latch","atomic":false,"steps":steps})).unwrap();
        let env = h.batch_until(req, Origin::User, &|| {
            if n.fetch_add(1, Ordering::SeqCst) == block_index {
                std::thread::sleep(Duration::from_millis(stall));
            }
            false
        });
        assert!(env.is_ok(), "{env:?}");
        let asks = work::asks();
        eprintln!("LATCH stall={stall} index={block_index} asks={asks}");
        if stall > 250 {
            assert_eq!(asks, block_index as i64);
        } else {
            assert!(asks >= 1);
        }
    }
}
#[test]
fn consistently_slow_answers_on_each_probe_call_are_bounded_and_latched() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for delay in [240, 260] {
        let h = fixture();
        let n = AtomicUsize::new(0);
        work::reset_asks();
        let start = Instant::now();
        let env = h
            .invoke_served_until(edit(), &|| {
                if n.fetch_add(1, Ordering::SeqCst) > 0 {
                    std::thread::sleep(Duration::from_millis(delay));
                }
                false
            })
            .0;
        eprintln!(
            "SLOW_EVERY delay={delay} asks={} probe_calls={} elapsed={:?}",
            work::asks(),
            n.load(Ordering::SeqCst),
            start.elapsed()
        );
        assert!(env.is_ok(), "{env:?}");
        if delay > 250 {
            assert_eq!(work::asks(), 1);
        }
    }
}

#[test]
fn queries_ask_zero_and_work_cancellation_names_its_cause() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    work::reset_asks();
    assert!(h.invoke_served_until(Invocation::user("doc.state.get", json!({})), &|| true).0.is_ok());
    assert_eq!(work::asks(), 0);
    work::set_spin_micros(20000);
    let start = Instant::now();
    let env = h
        .invoke_served_until(Invocation::user("render.frame.run", json!({"frame":1,"scale":0.25})), &|| {
            start.elapsed() > Duration::from_millis(60)
        })
        .0;
    work::set_spin_micros(0);
    let e = env.error.unwrap();
    assert_eq!(e.code, "command.timeout");
    assert_eq!(e.details["reason"], "cancelled");
    assert_eq!(work::asks(), 0);
}
#[test]
fn job_before_handler_refuses_and_after_start_keeps_identity() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for after in [false, true] {
        let h = fixture();
        let dir = std::env::temp_dir().join(format!("c34-r4-job-{}-{after}", std::process::id()));
        let env = h
            .invoke_served_until(
                Invocation::user(
                    "render.range.run",
                    json!({"from":1,"to":1000,"width":8,"directory":dir,"createDirs":true,"wait":true}),
                ),
                &|| !after || !h.jobs().list(None, None).is_empty(),
            )
            .0;
        let e = env.error.unwrap();
        assert_eq!(e.details["reason"], "cancelled");
        if after {
            let id = e.details["jobId"].as_str().unwrap();
            ok(&h, "job.cancel", json!({"jobId":id}));
            h.jobs().wait(id, Duration::from_secs(60)).unwrap();
        } else {
            assert!(h.jobs().list(None, None).is_empty());
        }
        assert!(
            !std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .any(|x| x.file_name().to_string_lossy().contains("tt-partial"))
        );
    }
}

#[test]
fn sequential_job_start_cancellation_refuses_the_later_query() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c34-r4-batch-{}", std::process::id()));
    let req: BatchRequest =
        serde_json::from_value(json!({"label":"job acceptance","atomic":false,"stopOnError":false,"steps":[
 {"id":"render.range.run","params":{"from":1,"to":1000,"width":8,"directory":dir,"createDirs":true,"wait":true}},
 {"id":"doc.state.get","params":{}}]}))
        .unwrap();
    let env = h.batch_until(req, Origin::User, &|| !h.jobs().list(None, None).is_empty());
    let steps = env.steps.unwrap();
    let e = steps[0].error.as_ref().unwrap();
    let id = e.details["jobId"].as_str().unwrap();
    assert_eq!(e.details["reason"], "cancelled");
    assert_eq!(steps[1].error.as_ref().unwrap().details["reason"], "cancelled");
    ok(&h, "job.cancel", json!({"jobId":id}));
    h.jobs().wait(id, Duration::from_secs(60)).unwrap();
    assert!(
        !std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|x| x.file_name().to_string_lossy().contains("tt-partial"))
    );
}
#[test]
fn timed_out_queued_query_is_dropped_and_not_cached() {
    let _guard = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(200) });
    let (tx, rx) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let wait = std::sync::Mutex::new(wait);
    tt_persist::lock::hooks::set(Some(std::sync::Arc::new(move |at, _| {
        if at == tt_persist::lock::Boundary::Committed {
            tx.send(()).unwrap();
            wait.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
        }
    })));
    let mut query = Invocation::user("doc.state.get", json!({}));
    query.options.request_id = Some("r4-expired-query".into());
    std::thread::scope(|s| {
        let blocker = s.spawn(|| h.invoke(edit()));
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let env = h.invoke_served_until(query.clone(), &|| false).0;
        assert_eq!(env.error.unwrap().code, "command.timeout");
        release.send(()).unwrap();
        blocker.join().unwrap();
    });
    tt_persist::lock::hooks::set(None);
    let (env, cached) = h.invoke_served(query);
    assert!(env.is_ok());
    assert!(!cached);
}
