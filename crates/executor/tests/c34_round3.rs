//! M3 C3.4 round 3 (builder): the document thread never depends on caller code for longer than one bound
//! (C34-R3-01), and every refusal names its cause (C34-R3-02). ADR-0020 decision 8.
//! - Questions are counted (`test-hooks`): none for plain calls and queries; one at dequeue plus three per committing
//!   action step (start, acceptance, pre-commit) and two per step that changes no document (a 1000-step batch).
//! - A probe that never answers stalls the document thread once, whatever the step count; the request's outcome is the
//!   uncancelled one; an unrelated caller waits at most the bound plus its own work.
//! - A caller's cancellation seen on the work-account path reports `reason: "cancelled"`; a watchdog timeout does not.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};
use tt_vector::work::hooks as work;

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        work::set_spin_micros(0);
    }
}

fn creates(prefix: &str, n: usize) -> BatchRequest {
    let steps: Vec<Value> = (0..n)
        .map(|i| json!({ "id": "element.create", "params": { "type": "peg", "name": format!("{prefix}{i}"), "sceneId": "scene:#1" } }))
        .collect();
    serde_json::from_value(json!({ "label": prefix, "atomic": false, "steps": steps })).unwrap()
}

#[test]
fn plain_calls_and_queries_ask_nothing_and_a_probed_batch_asks_at_each_documented_boundary() {
    let _t = turn();
    let h = fixture();
    work::reset_asks();
    ok(&h, "element.create", json!({ "type": "peg", "name": "Plain", "sceneId": "scene:#1" }));
    let _ = h.invoke(Invocation::user("doc.state.get", json!({})));
    let _ = h.invoke_served(Invocation::user(
        "element.create",
        json!({ "type": "peg", "name": "Served", "sceneId": "scene:#1" }),
    ));
    assert!(h.batch(creates("Batch", 20), Origin::User).is_ok());
    assert!(h.batch_audited(creates("Audited", 20), Origin::User, "test").is_ok());
    // A probed query: the worker never asks.
    for _ in 0..50 {
        let _ = h.invoke_served_until(Invocation::user("doc.state.get", json!({})), &|| false);
    }
    assert_eq!(work::asks(), 0, "plain calls and queries cost no question");
    work::reset_asks();
    assert!(h.batch_until(creates("Probed", 100), Origin::User, &|| false).is_ok());
    assert_eq!(work::asks(), 1 + 3 * 100, "one at dequeue, three per committing step");
    work::reset_asks();
    let ui: Vec<Value> = (0..1000)
        .map(|_| json!({ "id": "edit.selection.set", "params": { "context": "drawing", "ids": [] } }))
        .collect();
    let req: BatchRequest = serde_json::from_value(json!({ "label": "ui", "atomic": false, "steps": ui })).unwrap();
    assert!(h.batch_until(req, Origin::User, &|| false).is_ok());
    assert_eq!(work::asks(), 1 + 2 * 1000, "one at dequeue, two per step that changes no document");
}

/// A probe that answers its first call (the check before enqueue) at once and then blocks `stall` on every call.
fn silent_after_first(stall: Duration) -> impl Fn() -> bool + Sync {
    let calls = AtomicUsize::new(0);
    move || {
        if calls.fetch_add(1, Ordering::SeqCst) > 0 {
            std::thread::sleep(stall);
        }
        false
    }
}

#[test]
fn a_probe_that_never_answers_stalls_the_document_thread_once_for_a_thousand_steps() {
    let _t = turn();
    let h = fixture();
    let bound = tt_executor::ask_bound(Duration::from_secs(60));
    assert_eq!(bound, Duration::from_millis(250));
    // Steps that change no document, so both batches meet the same document.
    let steps = || -> BatchRequest {
        let ui: Vec<Value> = (0..1000)
            .map(|_| json!({ "id": "edit.selection.set", "params": { "context": "drawing", "ids": [] } }))
            .collect();
        serde_json::from_value(json!({ "label": "ui", "atomic": false, "steps": ui })).unwrap()
    };
    let start = Instant::now();
    assert!(h.batch(steps(), Origin::User).is_ok());
    let plain = start.elapsed();
    work::reset_asks();
    let stall = Duration::from_millis(600);
    let probe = silent_after_first(stall);
    let start = Instant::now();
    let env = h.batch_until(steps(), Origin::User, &probe);
    let elapsed = start.elapsed();
    assert!(env.is_ok(), "the silent request's outcome is the uncancelled one: {:?}", env.error);
    assert!(env.steps.unwrap().iter().all(|s| s.is_ok()), "every step ran");
    assert_eq!(work::asks(), 1, "the first unanswered question latches the link: one stall, not one per boundary");
    // The document thread waited one bound; the caller itself sits in its probe for `stall` per call (at most twice
    // before it sees the reply).
    let limit = plain + bound + stall * 2 + Duration::from_millis(750);
    eprintln!("silent 1000-step batch {elapsed:?} (plain {plain:?}, bound {bound:?}, probe stall {stall:?})");
    assert!(elapsed < limit, "{elapsed:?} >= {limit:?}");
    // A committing silent request keeps its outcome too.
    let before = rev(&h);
    let probe = silent_after_first(Duration::from_millis(400));
    assert!(h.batch_until(creates("Silent", 50), Origin::User, &probe).is_ok());
    assert_eq!(rev(&h), before + 50);
}

#[test]
fn an_unrelated_caller_waits_at_most_the_bound_plus_its_own_work() {
    let _t = turn();
    let budget = Duration::from_millis(2000);
    let h = fixture_with(ExecutorConfig { budget });
    let bound = tt_executor::ask_bound(budget);
    let before = rev(&h);
    let (tx, rx) = std::sync::mpsc::channel();
    let calls = AtomicUsize::new(0);
    let (silent, waited) = std::thread::scope(|s| {
        let caller = s.spawn(|| {
            h.invoke_served_until(
                Invocation::user("element.create", json!({ "type": "peg", "name": "Silent", "sceneId": "scene:#1" })),
                &|| {
                    if calls.fetch_add(1, Ordering::SeqCst) == 1 {
                        tx.send(()).unwrap();
                        std::thread::sleep(Duration::from_millis(1200));
                    }
                    false
                },
            )
            .0
        });
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let start = Instant::now();
        let other = run(&h, "doc.state.get", json!({}));
        let waited = start.elapsed();
        assert!(other.is_ok(), "{other:?}");
        (caller.join().unwrap(), waited)
    });
    eprintln!("unrelated query waited {waited:?} (bound {bound:?})");
    assert!(waited < bound + Duration::from_millis(400), "{waited:?}");
    assert!(silent.is_ok(), "a silent caller is not cancelled: {:?}", silent.error);
    assert_eq!(rev(&h), before + 1);
}

#[test]
fn a_caller_cancellation_on_the_work_account_path_says_cancelled() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture();
    work::set_spin_micros(20_000);
    let start = Instant::now();
    // A query: its worker is never asked, so the cancellation reaches it through the token its caller marks.
    let env = h
        .invoke_served_until(Invocation::user("render.frame.run", json!({ "frame": 1, "scale": 0.25 })), &|| {
            start.elapsed() > Duration::from_millis(60)
        })
        .0;
    work::set_spin_micros(0);
    let e = env.error.expect("refused at a work-account check");
    assert_eq!(e.code, "command.timeout", "{e:?}");
    assert_eq!(e.details["reason"], "cancelled", "{e:?}");
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
}

#[test]
fn a_watchdog_timeout_on_the_work_account_path_is_not_a_cancellation() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(300) });
    work::set_spin_micros(20_000);
    let env =
        h.invoke_served_until(Invocation::user("render.frame.run", json!({ "frame": 1, "scale": 0.25 })), &|| false).0;
    work::set_spin_micros(0);
    let e = env.error.expect("timed out");
    assert_eq!(e.code, "command.timeout", "{e:?}");
    assert_ne!(e.details["reason"], "cancelled", "a watchdog expiry is not relabelled: {e:?}");
    // The token keeps the cause the worker reports: a timeout mark is a timeout, a caller's mark a cancellation.
    let state = tt_vector::work::CancelState::default();
    let (a, b) = (state.token(), state.token());
    state.mark(a);
    state.mark_cancelled(b);
    assert_eq!(state.cause(a), Some(tt_vector::work::Cause::Timeout));
    assert_eq!(state.cause(b), Some(tt_vector::work::Cause::Cancelled));
    state.forget(a);
    state.forget(b);
    assert_eq!(state.cause(a), None);
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
}

#[test]
fn a_probed_query_cancelled_while_queued_still_runs_and_a_cancelled_batch_refuses_its_query_step() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(1500) });
    let dir = std::env::temp_dir().join(format!("c34-round3-busy-{}", std::process::id()));
    work::set_spin_micros(20_000);
    let busy: BatchRequest = serde_json::from_value(json!({
        "label": "busy", "atomic": false,
        "steps": [{ "id": "render.range.run", "params": { "from": 1, "to": 10000, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true } }]
    }))
    .unwrap();
    let (query, cached) = std::thread::scope(|s| {
        let blocker = s.spawn(|| h.batch(busy, Origin::User));
        let start = Instant::now();
        while std::fs::read_dir(&dir).map_or(true, |mut d| d.next().is_none()) {
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(2));
        }
        // Cancelled from the start, so its caller marks it while it waits behind the busy batch.
        let mut inv = Invocation::user("doc.state.get", json!({}));
        inv.options.request_id = Some("queued-query".to_owned());
        let (query, _) = h.invoke_served_until(inv.clone(), &|| true);
        let job = blocker.join().unwrap().steps.unwrap()[0].error.as_ref().unwrap().details["jobId"]
            .as_str()
            .unwrap()
            .to_owned();
        ok(&h, "job.cancel", json!({ "jobId": job }));
        work::set_spin_micros(0);
        h.jobs().wait(&job, Duration::from_secs(60)).unwrap();
        let (_, cached) = h.invoke_served_until(inv, &|| false);
        (query, cached)
    });
    assert!(query.is_ok(), "a query is not refused for its caller's cancellation: {:?}", query.error);
    assert!(cached, "and its answer is remembered under its requestId");
    // The same query as a step of a cancelled batch is refused, like every later step.
    let req: BatchRequest = serde_json::from_value(json!({
        "label": "q", "atomic": false, "stopOnError": false,
        "steps": [{ "id": "element.create", "params": { "type": "peg", "name": "Q", "sceneId": "scene:#1" } }, { "id": "doc.state.get", "params": {} }]
    }))
    .unwrap();
    // The probe flips on the document thread right after step 1 commits (the `Committed` boundary hook).
    let flipped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f = flipped.clone();
    tt_persist::lock::hooks::set(Some(std::sync::Arc::new(
        move |at: tt_persist::lock::Boundary, _: &std::path::Path| {
            if at == tt_persist::lock::Boundary::Committed {
                f.store(true, Ordering::SeqCst);
            }
        },
    )));
    let env = h.batch_until(req, Origin::User, &|| flipped.load(Ordering::SeqCst));
    tt_persist::lock::hooks::set(None);
    let steps = env.steps.unwrap();
    assert!(steps[0].is_ok(), "{:?}", steps[0]);
    assert_eq!(steps[1].error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{:?}", steps[1]);
}

#[test]
fn a_job_command_cancelled_after_its_handler_started_the_job_commits_and_names_the_job() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture();
    let dir = std::env::temp_dir().join(format!("c34-round3-job-{}", std::process::id()));
    // The probe turns true once the handler has started the job (it is in the registry), so the question at acceptance
    // and the caller's own poll both see a cancellation that came after the job started.
    let env = h
        .invoke_served_until(
            Invocation::user(
                "render.range.run",
                json!({ "from": 1, "to": 10000, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true }),
            ),
            &|| !h.jobs().list(None, None).is_empty(),
        )
        .0;
    let e = env.error.expect("the wait is cancelled");
    assert_eq!(e.code, "command.timeout", "{e:?}");
    assert_eq!(e.details["reason"], "cancelled", "{e:?}");
    let job = e.details["jobId"].as_str().expect("the started job is named").to_owned();
    ok(&h, "job.cancel", json!({ "jobId": job }));
    let record = h.jobs().wait(&job, Duration::from_secs(60)).unwrap().0;
    assert_eq!(record.state, tt_executor::JobState::Cancelled);
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
}
