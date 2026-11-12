//! M3 C3.4 round 3 (builder): the document thread never depends on caller code for longer than one bound
//! (C34-R3-01), and every refusal names its cause (C34-R3-02). ADR-0020 decision 8.
//! - Questions are counted (`test-hooks`): none for plain calls and queries; one at dequeue plus three per committing
//!   action step (start, acceptance, pre-commit) and two per step that changes no document.
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
    assert!(h.batch_until(creates("Probed", 1000), Origin::User, &|| false).is_ok());
    assert_eq!(work::asks(), 1 + 3 * 1000, "one at dequeue, three per committing step");
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
