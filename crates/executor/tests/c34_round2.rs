//! M3 C3.4 round 2 (builder): the request-cancellation contract of ADR-0020 decision 8.
//! - C34-R2-01: once a cancellation is observable, no later step acceptance is passed; the worker asks the caller
//!   directly. Driven deterministically: a boundary hook flips the probe between step 1's commit and step 2's
//!   acceptance, 1000 times; a job step refused there never starts its job.
//! - C34-R2-04: a queued request discarded because it was cancelled answers an honest cancellation; a worker that
//!   really dies answers `internal`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use serde_json::json;
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};
use tt_persist::lock::{Boundary, hooks};

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        hooks::set(None);
        tt_vector::work::hooks::set_spin_micros(0);
    }
}

#[test]
fn a_cancellation_between_two_steps_refuses_the_second_every_time() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture();
    let flipped = Arc::new(AtomicBool::new(false));
    let f = flipped.clone();
    // Set on the document thread right after a step commits, before the next step's boundary.
    hooks::set(Some(Arc::new(move |at: Boundary, _: &Path| {
        if at == Boundary::Committed {
            f.store(true, Ordering::SeqCst);
        }
    })));
    let dir = c13_support::tmp();
    let before = rev(&h);
    let mut refused_jobs = 0;
    for i in 0..1000 {
        flipped.store(false, Ordering::SeqCst);
        let second = if i % 2 == 0 {
            json!({ "id": "element.create", "params": { "type": "peg", "name": format!("B{i}"), "sceneId": "scene:#1" } })
        } else {
            json!({ "id": "render.range.run", "params": { "from": 1, "to": 1, "width": 8, "directory": dir.join(format!("j{i}")).to_string_lossy(), "createDirs": true } })
        };
        let req: BatchRequest = serde_json::from_value(json!({
            "label": "two steps", "atomic": false, "stopOnError": false,
            "steps": [{ "id": "element.create", "params": { "type": "peg", "name": format!("A{i}"), "sceneId": "scene:#1" } }, second]
        }))
        .unwrap();
        let env = h.batch_until(req, Origin::User, &|| flipped.load(Ordering::SeqCst));
        let steps = env.steps.unwrap();
        assert!(steps[0].is_ok(), "iteration {i}: step 1 committed before the cancellation and stands: {:?}", steps[0]);
        let e = steps[1].error.as_ref().unwrap_or_else(|| panic!("iteration {i}: step 2 passed its acceptance"));
        assert_eq!(e.code, "command.timeout", "iteration {i}: {e:?}");
        if i % 2 == 1 {
            refused_jobs += 1;
        }
    }
    hooks::set(None);
    assert_eq!(rev(&h), before + 1000, "exactly the 1000 first steps committed");
    let jobs = ok(&h, "job.list", json!({}));
    let jobs = if jobs.is_array() { jobs } else { jobs["jobs"].clone() };
    assert!(
        jobs.as_array().unwrap().iter().all(|j| j["commandId"] != "render.range.run"),
        "none of the {refused_jobs} refused job steps started its job: {jobs}"
    );
}

#[test]
fn a_queued_request_discarded_after_its_cancellation_answers_an_honest_refusal() {
    let _t = turn();
    let _reset = Reset;
    // 3000 ms: the busy batch's waited step leaves a tenth of the budget for the rest of the batch, which must cover a
    // slow runner's scheduling; at 1500 ms a macOS runner once overran it (run 43164814284).
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(3000) });
    let dir = c13_support::tmp().join("busy");
    tt_vector::work::hooks::set_spin_micros(20_000);
    let busy: BatchRequest = serde_json::from_value(json!({
        "label": "busy", "atomic": false,
        "steps": [{ "id": "render.range.run", "params": { "from": 1, "to": 10000, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true } }]
    }))
    .unwrap();
    let before = rev(&h);
    let refused = std::thread::scope(|s| {
        let blocker = s.spawn(|| h.batch(busy, Origin::User));
        let start = Instant::now();
        while std::fs::read_dir(&dir).map_or(true, |mut d| d.next().is_none()) {
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(2));
        }
        // Not cancelled when the call is made (it is enqueued behind the batch), cancelled while it waits in the queue.
        let at = Instant::now();
        let refused = h
            .invoke_served_until(
                Invocation::user("element.create", json!({ "type": "peg", "name": "Queued", "sceneId": "scene:#1" })),
                &|| at.elapsed() > Duration::from_millis(50),
            )
            .0;
        let env = blocker.join().unwrap();
        let steps = env.steps.unwrap_or_else(|| panic!("the busy batch answered no steps: {:?}", env.error));
        let job = steps[0].error.as_ref().unwrap().details["jobId"].as_str().unwrap().to_owned();
        ok(&h, "job.cancel", json!({ "jobId": job }));
        tt_vector::work::hooks::set_spin_micros(0);
        h.jobs().wait(&job, Duration::from_secs(60)).unwrap();
        refused
    });
    let e = refused.error.expect("refused");
    assert_eq!(e.code, "command.timeout", "an honest cancellation, not a dead worker: {e:?}");
    assert_ne!(e.message, "the document thread has stopped");
    assert_eq!(rev(&h), before, "the discarded request changed nothing");
    assert!(run(&h, "doc.state.get", json!({})).is_ok(), "the worker is alive");
}

#[test]
fn a_worker_that_really_dies_answers_internal() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture();
    // The document thread dies inside a request (a test hook panics on it).
    hooks::set(Some(Arc::new(|at: Boundary, _: &Path| {
        if at == Boundary::Committed {
            panic!("test: the document thread dies");
        }
    })));
    let never = AtomicBool::new(false);
    let died = h
        .invoke_served_until(
            Invocation::user("element.create", json!({ "type": "peg", "name": "Dies", "sceneId": "scene:#1" })),
            &|| never.load(Ordering::SeqCst),
        )
        .0;
    hooks::set(None);
    assert_eq!(died.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{died:?}");
    let after = run(&h, "doc.state.get", json!({}));
    assert_eq!(after.error.as_ref().map(|e| e.code.as_str()), Some("internal"), "{after:?}");
}
