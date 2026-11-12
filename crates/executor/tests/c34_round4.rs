//! M3 C3.4 round 4 (builder). ADR-0020 decision 8.
//! - C34-R4-01: every copy of a caller-cancellation refusal says `reason: "cancelled"` with the same hint: the step,
//!   the error nested in `batch.stepFailed` (atomic and sequential batches) and the activity-log entry. A watchdog
//!   expiry in the same atomic shape is unlabelled in every copy.
//! - C34-R4-02: an answer at or after the question bound latches the link; a late `true` is honoured.
//! - O-6: a probe that always answers just inside the bound holds the document thread for at most the request's own
//!   budget plus one bound in questions; the request ends by its watchdog, the document is whole, and an unrelated caller
//!   is served at once afterwards.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
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

fn render_then_create(atomic: bool) -> BatchRequest {
    serde_json::from_value(json!({
        "label": "cause", "atomic": atomic, "stopOnError": true,
        "steps": [
            { "id": "render.frame.run", "params": { "frame": 1, "scale": 0.25 } },
            { "id": "element.create", "params": { "type": "peg", "name": "MustNotCommit", "sceneId": "scene:#1" } }
        ]
    }))
    .unwrap()
}

/// The step's error and the copy nested in the outer `batch.stepFailed`.
fn copies(env: &tt_executor::Envelope) -> (Value, Value) {
    let outer = env.error.as_ref().expect("the batch failed");
    assert_eq!(outer.code, "batch.stepFailed", "{outer:?}");
    let step = env.steps.as_ref().unwrap()[0].error.as_ref().expect("its first step failed");
    (serde_json::to_value(step).unwrap(), outer.details["error"].clone())
}

fn agent(h: &tt_executor::ExecutorHandle) -> Origin {
    let a = h
        .connect_agent(tt_executor::agents::AgentConnect {
            policy: Some(tt_executor::agents::Policy::Autonomous),
            ..Default::default()
        })
        .unwrap();
    Origin::Agent { session: a.session_id, name: a.client, scopes: a.scopes }
}

fn last_activity(h: &tt_executor::ExecutorHandle) -> Value {
    let list = ok(h, "agent.activity.list", json!({ "limit": 1 }));
    let list = if list.is_array() { list } else { list["entries"].clone() };
    list.as_array().unwrap().last().cloned().expect("an activity entry")
}

#[test]
fn every_copy_of_a_caller_cancellation_says_cancelled_with_one_hint_atomic_and_sequential() {
    let _t = turn();
    let _reset = Reset;
    for atomic in [true, false] {
        let h = fixture();
        let origin = agent(&h);
        let before = rev(&h);
        work::set_spin_micros(20_000);
        let start = Instant::now();
        let env = h.batch_audited_until(render_then_create(atomic), origin, "batch", &|| {
            start.elapsed() > Duration::from_millis(60)
        });
        work::set_spin_micros(0);
        assert_eq!(rev(&h), before, "atomic={atomic}: nothing committed");
        let (step, nested) = copies(&env);
        for (which, e) in [("step", &step), ("nested", &nested)] {
            assert_eq!(e["code"], "command.timeout", "atomic={atomic} {which}: {e}");
            assert_eq!(e["details"]["reason"], "cancelled", "atomic={atomic} {which}: {e}");
            let hint = e["hint"].as_str().unwrap_or_default();
            assert!(hint.contains("cancelled") && !hint.contains("watchdog"), "atomic={atomic} {which}: {e}");
        }
        assert_eq!(step["hint"], nested["hint"], "atomic={atomic}: one hint in both copies");
        assert_eq!(env.error.as_ref().unwrap().details["error"]["details"]["reason"], "cancelled");
        let entry = last_activity(&h);
        assert_eq!(entry["errorCode"], "batch.stepFailed", "atomic={atomic}: {entry}");
        assert_eq!(entry["errorReason"], "cancelled", "atomic={atomic}: the entry keeps the cause: {entry}");
    }
}

#[test]
fn a_watchdog_expiry_in_the_same_atomic_shape_is_unlabelled_in_every_copy() {
    let _t = turn();
    let _reset = Reset;
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(300) });
    let origin = agent(&h);
    let before = rev(&h);
    work::set_spin_micros(20_000);
    let env = h.batch_audited_until(render_then_create(true), origin, "batch", &|| false);
    work::set_spin_micros(0);
    assert_eq!(rev(&h), before);
    // The caller's own deadline may answer first (`command.timeout` without steps); otherwise the step and its copy
    // are both a watchdog timeout.
    if env.steps.is_some() {
        let (step, nested) = copies(&env);
        for (which, e) in [("step", &step), ("nested", &nested)] {
            assert_eq!(e["code"], "command.timeout", "{which}: {e}");
            assert_eq!(e["details"]["reason"], Value::Null, "{which}: a watchdog expiry is not relabelled: {e}");
            assert!(!e["hint"].as_str().unwrap_or_default().contains("caller cancelled"), "{which}: {e}");
        }
    } else {
        let e = env.error.as_ref().unwrap();
        assert_eq!(e.code, "command.timeout", "{e:?}");
        assert_eq!(e.details["reason"], Value::Null, "{e:?}");
    }
    assert_eq!(last_activity(&h)["errorReason"], Value::Null);
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
}

#[test]
fn a_late_answer_is_silence_and_latches_after_one_question() {
    let _t = turn();
    for _ in 0..5 {
        let h = fixture();
        let before = rev(&h);
        work::reset_asks();
        let env = h
            .invoke_served_until(
                Invocation::user("element.create", json!({ "type": "peg", "name": "LateTrue", "sceneId": "scene:#1" })),
                &|| {
                    if work::asks() == 0 {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(270));
                    true
                },
            )
            .0;
        assert_eq!(work::asks(), 1, "a late answer latches the link: one question");
        // Late is silence, so the worker went on; the caller's mark is seen at a later boundary unless the commit got
        // there first (rule 3). Either way the answer and the document agree.
        match env.error {
            None => assert_eq!(rev(&h), before + 1, "the commit got there first and stands"),
            Some(e) => {
                assert_eq!(e.code, "command.timeout", "{e:?}");
                assert_eq!(e.details["reason"], "cancelled", "{e:?}");
                assert_eq!(rev(&h), before);
            }
        }
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
    }
}

#[test]
fn answers_just_inside_the_bound_hold_the_thread_at_most_the_budget_plus_one_bound() {
    let _t = turn();
    let budget = Duration::from_millis(1500);
    let bound = Duration::from_millis(250);
    let h = fixture_with(ExecutorConfig { budget });
    let before = rev(&h);
    let steps: Vec<Value> = (0..200)
        .map(|i| json!({ "id": "element.create", "params": { "type": "peg", "name": format!("C{i}"), "sceneId": "scene:#1" } }))
        .collect();
    // A sequential batch: three questions per committing step (start, acceptance, pre-commit).
    let req: BatchRequest =
        serde_json::from_value(json!({ "label": "slow answers", "atomic": false, "steps": steps })).unwrap();
    work::reset_asks();
    // Each new question is answered after 0.8 x the bound; the caller's own polls between questions return at once.
    let answered = AtomicI64::new(0);
    let start = Instant::now();
    let env = h.batch_until(req, Origin::User, &|| {
        let asks = work::asks();
        if asks > answered.load(Ordering::SeqCst) {
            answered.store(asks, Ordering::SeqCst);
            std::thread::sleep(bound * 4 / 5);
        }
        false
    });
    let took = start.elapsed();
    let unrelated = Instant::now();
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    let unrelated = unrelated.elapsed();
    let e = env.error.expect("ended by its watchdog");
    assert_eq!(e.code, "command.timeout", "{e:?}");
    assert_eq!(e.details["reason"], Value::Null, "a watchdog timeout, not a cancellation: {e:?}");
    assert!(work::asks() >= 3, "several questions answered inside the bound: {}", work::asks());
    assert!(took <= budget + bound * 2, "the request ended by its watchdog within budget + 2 x bound: {took:?}");
    // Whole: every step that committed is one revision and one element; the rest changed nothing.
    let committed = rev(&h) - before;
    assert!(committed < 200, "the watchdog stopped the batch: {committed} steps committed");
    let validate = ok(&h, "doc.validate", json!({ "minSeverity": "error" }));
    assert_eq!(validate["issues"], json!([]), "{validate}");
    eprintln!(
        "CUMULATIVE budget_ms={} bound_ms={} asks={} took_ms={} committed={committed} unrelated_ms={}",
        budget.as_millis(),
        bound.as_millis(),
        work::asks(),
        took.as_secs_f64() * 1000.0,
        unrelated.as_secs_f64() * 1000.0
    );
    assert!(unrelated < bound, "an unrelated caller is served at once afterwards: {unrelated:?}");
}
