//! M3 C3.4 round 4 (builder). ADR-0020 decision 8.
//! - C34-R4-01: every copy of a caller-cancellation refusal says `reason: "cancelled"` with the same hint: the step,
//!   the error nested in `batch.stepFailed` (atomic and sequential batches) and the activity-log entry. A watchdog
//!   expiry in the same atomic shape is unlabelled in every copy.
//! - C34-R4-02: an answer at or after the question bound is silence, whatever it says, and latches the link.
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
    // A trial is valid only if every answer stayed inside the bound. A runner that deschedules the probe past it makes
    // that answer late, which latches the link as designed (C34-R4-02) and leaves the premise untested; such a trial is
    // retried, up to five times.
    for attempt in 1..=5 {
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
        let longest = Mutex::new(Duration::ZERO);
        let start = Instant::now();
        let env = h.batch_until(req, Origin::User, &|| {
            let asks = work::asks();
            if asks > answered.load(Ordering::SeqCst) {
                answered.store(asks, Ordering::SeqCst);
                // A yielding spin, not a sleep: macOS runners overshoot a 200 ms sleep by 70-120 ms (run 43213759353),
                // past the bound.
                let at = Instant::now();
                while at.elapsed() < bound * 4 / 5 {
                    std::thread::yield_now();
                }
                let mut l = longest.lock().unwrap();
                *l = (*l).max(at.elapsed());
            }
            false
        });
        let took = start.elapsed();
        let unrelated = Instant::now();
        assert!(run(&h, "doc.state.get", json!({})).is_ok());
        let unrelated = unrelated.elapsed();
        let longest = *longest.lock().unwrap();
        eprintln!(
            "CUMULATIVE attempt={attempt} budget_ms={} bound_ms={} asks={} longest_answer_ms={} took_ms={} committed={} unrelated_ms={}",
            budget.as_millis(),
            bound.as_millis(),
            work::asks(),
            longest.as_secs_f64() * 1000.0,
            took.as_secs_f64() * 1000.0,
            rev(&h) - before,
            unrelated.as_secs_f64() * 1000.0
        );
        // A late answer latched the link: by the probe's own clock, or (a finished batch that asked fewer than its
        // 1 + 3 x 200 questions) by the worker's.
        if longest >= bound * 9 / 10 || (env.is_ok() && work::asks() < 1 + 3 * 200) {
            continue;
        }
        let e = env.error.expect("every answer was inside the bound, so the request ended by its watchdog");
        assert_eq!(e.code, "command.timeout", "{e:?}");
        assert_eq!(e.details["reason"], Value::Null, "a watchdog timeout, not a cancellation: {e:?}");
        assert!(work::asks() >= 3, "several questions answered inside the bound: {}", work::asks());
        crate::common::timing::record("c34_round4.rs:225", took, budget + bound * 2);
        assert!(took <= budget + bound * 2, "the request ended by its watchdog within budget + 2 x bound: {took:?}");
        // Whole: every step that committed is one revision and one element; the rest changed nothing.
        let committed = rev(&h) - before;
        assert!(committed < 200, "the watchdog stopped the batch: {committed} steps committed");
        let validate = ok(&h, "doc.validate", json!({ "minSeverity": "error" }));
        assert_eq!(validate["issues"], json!([]), "{validate}");
        crate::common::timing::record("c34_round4.rs:231", unrelated, bound);
        assert!(unrelated < bound, "an unrelated caller is served at once afterwards: {unrelated:?}");
        return;
    }
    panic!("no trial in five kept every answer inside the bound on this runner");
}

/// Milliseconds since the Unix epoch now (the clock the job records' `startedAt` uses).
fn unix_ms_now() -> i64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    i64::try_from(d.as_millis()).unwrap()
}

/// Milliseconds since the Unix epoch of an executor timestamp `YYYY-MM-DDTHH:MM:SS.mmmZ` (proleptic Gregorian).
fn unix_ms(iso: &str) -> i64 {
    let n = |r: std::ops::Range<usize>| iso[r].parse::<i64>().unwrap_or_else(|_| panic!("timestamp `{iso}`"));
    let (y, m, d) = (n(0..4), n(5..7), n(8..10));
    let (hh, mm, ss) = (n(11..13), n(14..16), n(17..19));
    let ms = if iso.as_bytes().get(19) == Some(&b'.') { n(20..23) } else { 0 };
    // Days from civil (H. Hinnant).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 86_400 + hh * 3_600 + mm * 60 + ss) * 1_000) + ms
}

#[test]
fn executor_timestamps_parse_to_epoch_milliseconds() {
    assert_eq!(unix_ms("1970-01-01T00:00:00.000Z"), 0);
    assert_eq!(unix_ms("2026-11-17T20:31:05.123Z"), 1_794_947_465_123);
    assert_eq!(unix_ms("2000-02-29T12:00:00.000Z"), 951_825_600_000);
}

#[test]
fn a_queued_batch_wait_is_bounded_from_its_callers_enqueue_not_from_its_start() {
    let _t = turn();
    let _reset = Reset;
    // D-45 (as D-64 for `c34_r1_contract`): a budget whose margin (a tenth) a loaded runner meets. With 1,000 ms the
    // queued batch's reply had to arrive within 100 ms of its waited step's end; the 3-core macOS runner missed it
    // once (run 44668936063: `command.timeout`, no step envelopes). The rule checked (the deadline counts from the
    // enqueue) is the same at any budget.
    let budget = Duration::from_millis(5000);
    let h = fixture_with(ExecutorConfig { budget });
    let base = std::env::temp_dir().join(format!("c34-round4-queued-wait-{}", std::process::id()));
    let waited = |dir: &std::path::Path| -> BatchRequest {
        serde_json::from_value(json!({
            "label": "waited", "atomic": false,
            "steps": [{ "id": "render.range.run", "params": { "from": 1, "to": 10000, "width": 32, "directory": dir.to_string_lossy(), "createDirs": true, "wait": true } }]
        }))
        .unwrap()
    };
    let (first_dir, second_dir) = (base.join("first"), base.join("second"));
    work::set_spin_micros(20_000);
    let (first, second, queued_for, enqueued_ms) = std::thread::scope(|s| {
        // The first batch holds the document thread for most of its budget, waiting on its job.
        let first = s.spawn(|| h.batch(waited(&first_dir), Origin::User));
        let start = Instant::now();
        while std::fs::read_dir(&first_dir).map_or(true, |mut d| d.next().is_none()) {
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(2));
        }
        // The second is enqueued behind it, so it starts several hundred milliseconds into its own budget.
        let enqueued_ms = unix_ms_now();
        let enqueued = Instant::now();
        let second = h.batch(waited(&second_dir), Origin::User);
        let took = enqueued.elapsed();
        (first.join().unwrap(), second, took, enqueued_ms)
    });
    work::set_spin_micros(0);
    let mut jobs = Vec::new();
    for env in [&first, &second] {
        let steps = env.steps.as_ref().unwrap_or_else(|| panic!("the batch answered with its steps: {:?}", env.error));
        let e = steps[0].error.as_ref().expect("its waited step timed out");
        assert_eq!(e.code, "job.waitTimeout", "{e:?}");
        jobs.push(e.details["jobId"].as_str().unwrap().to_owned());
    }
    // Ruling D-46 item 4 (D-45): which instant the deadline counts from, not how fast the runner is. Counted from the
    // caller's enqueue (the rule), the queued batch answers about `budget` after it was enqueued; counted from its
    // start, it would answer about `budget + queue_delay` after. The midpoint `budget + queue_delay / 2` separates the
    // two whatever the runner's speed: a slow runner moves both instants together (a macOS runner overran the bare
    // `budget` once, run 43772579796). The queue delay must be large enough to tell them apart.
    // C43B-R3S-06 (ruling D-48): the queue delay is the executor's, not a caller thread's: the second batch's waited step
    // submitted its job when the executor started that batch, and the job record's `startedAt` (stamped by the executor,
    // milliseconds since the epoch on the same clock as `enqueued_ms`) is that instant. A caller descheduled after its
    // batch answered cannot inflate it.
    let started = h.jobs().wait(&jobs[1], Duration::ZERO).unwrap().0.started_at;
    let queue_delay = Duration::from_millis(u64::try_from(unix_ms(&started) - enqueued_ms).unwrap_or(0));
    assert!(
        queue_delay >= Duration::from_millis(100),
        "the second batch waited {queue_delay:?} behind the first: too little to tell the rules apart"
    );
    let midpoint = budget + queue_delay / 2;
    assert!(
        queued_for < midpoint,
        "the queued batch answered {queued_for:?} after its enqueue: its deadline counted from its start (queue delay {queue_delay:?}, midpoint {midpoint:?})"
    );
    for job in jobs {
        ok(&h, "job.cancel", json!({ "jobId": job }));
        h.jobs().wait(&job, Duration::from_secs(60)).unwrap();
    }
    let _ = std::fs::remove_dir_all(&base);
}
