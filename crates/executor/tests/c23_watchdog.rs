//! M2 C2.3 round 1 (C23-R1-07 b): the watchdog. A call that overruns its budget answers its caller with the typed
//! `command.timeout`; the worker observes the cancellation at its next work-account check, commits nothing, and answers
//! the request queued behind it. Verified generically with the work-account spin hook (any work-charging command
//! overruns on demand), in its own test binary because the hook is process-global.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::json;
use tt_executor::ExecutorConfig;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// The spin hook is process-global: the tests of this binary take turns.
static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn an_overrunning_call_times_out_typed_and_the_worker_answers_the_next_request() {
    let _turn = HOOK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let budget = Duration::from_millis(600);
    let h = fixture_with(ExecutorConfig { budget });
    let pts: Vec<_> = (0..400).map(|i| json!({ "x": 2000 + i, "y": 2000 })).collect();
    let s = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "fit": false, "layer": "new", "points": pts }),
    );
    let start_rev = rev(&h);
    // Every work charge now spins 3 ms: Smooth's evaluations overrun the 600 ms budget.
    tt_vector::work::hooks::set_spin_micros(3000);
    let t = Instant::now();
    let e = run(&h, "draw.smooth.apply", json!({ "drawingId": D1, "ids": s["strokeIds"] }));
    let waited = t.elapsed();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{e:?}");
    assert!(waited < budget + Duration::from_millis(400), "the caller waited {waited:?}");
    // The worker stops at its next check and answers the queued read within the budget.
    let t = Instant::now();
    let state = run(&h, "doc.state.get", json!({}));
    assert!(state.is_ok(), "{state:?}");
    assert!(t.elapsed() < budget, "the queued read waited {:?}", t.elapsed());
    // The cancelled call committed nothing.
    assert_eq!(rev(&h), start_rev);
}

/// C23-R2-01 race controls: a timeout cancels exactly its own request. A read queued behind a cancelled call still
/// answers inside its own budget, and the next caller's mutation commits normally.
#[test]
fn one_callers_timeout_never_cancels_another_callers_request() {
    let _turn = HOOK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let budget = Duration::from_millis(800);
    let h = fixture_with(ExecutorConfig { budget });
    let pts: Vec<_> = (0..400).map(|i| json!({ "x": 2000 + i, "y": 2000 })).collect();
    let s = ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "fit": false, "layer": "new", "points": pts }),
    );
    let start_rev = rev(&h);
    tt_vector::work::hooks::set_spin_micros(3000);
    let a = h.clone();
    let ids = s["strokeIds"].clone();
    let first = std::thread::spawn(move || run(&a, "draw.smooth.apply", json!({ "drawingId": D1, "ids": ids })));
    // Queued behind the spinning call, with a deadline after the first caller's timeout.
    std::thread::sleep(Duration::from_millis(150));
    let read = run(&h, "doc.state.get", json!({}));
    let first = first.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(first.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{first:?}");
    assert!(read.is_ok(), "the queued read is another caller's request: {read:?}");
    assert_eq!(rev(&h), start_rev, "the cancelled call committed nothing");
    // The next mutation is not affected by the earlier cancellation.
    ok(&h, "draw.drawing.rename", json!({ "drawingId": D1, "name": "after the timeout" }));
    assert_eq!(rev(&h), start_rev + 1);
    assert_eq!(get(&h, D1)["name"], "after the timeout");
}
