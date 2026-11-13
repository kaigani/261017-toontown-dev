//! M3 C3.4 round 2 (builder, token hygiene): watchdog tokens are bounded to outstanding calls. A caller that marks its
//! token after the worker forgot it (a late timeout, a late cancellation) leaves nothing behind; after a few thousand
//! cancelled and timed-out calls the marked and outstanding token sets return to zero. One test in its own binary: the
//! counters (`test-hooks`) are process-wide.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::time::{Duration, Instant};

use common::*;
use serde_json::json;
use tt_executor::{BatchRequest, ExecutorConfig, Invocation, Origin};

#[test]
fn thousands_of_cancelled_and_timed_out_calls_leave_no_token_behind() {
    let h = fixture();
    let short = fixture_with(ExecutorConfig { budget: Duration::from_millis(3) });
    for i in 0..3000u32 {
        let start = Instant::now();
        let flip = Duration::from_micros(u64::from(i % 7) * 40);
        let probe = || start.elapsed() >= flip;
        match i % 4 {
            0 => {
                let _ = h.invoke_served_until(
                    Invocation::user(
                        "element.create",
                        json!({ "type": "peg", "name": format!("T{i}"), "sceneId": "scene:#1" }),
                    ),
                    &probe,
                );
            }
            1 => {
                let _ = h.invoke_served_until(Invocation::user("doc.state.get", json!({})), &probe);
            }
            2 => {
                let req: BatchRequest = serde_json::from_value(json!({
                    "label": "t", "atomic": false,
                    "steps": [{ "id": "doc.state.get", "params": {} }, { "id": "element.create", "params": { "type": "peg", "name": format!("U{i}"), "sceneId": "scene:#1" } }]
                }))
                .unwrap();
                let _ = h.batch_until(req, Origin::User, &probe);
            }
            _ => {
                // A 3 ms watchdog budget: some calls time out as the worker finishes them (the late-mark interleaving).
                let _ = run(&short, "doc.export.json", json!({ "scope": "project", "binaries": "omit" }));
            }
        }
    }
    // Every call has been answered; the workers forget their last tokens when they finish them.
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    let start = Instant::now();
    while tt_vector::work::hooks::queued_tokens() != 0 || tt_vector::work::hooks::outstanding_tokens() != 0 {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "tokens left: {} marked, {} outstanding",
            tt_vector::work::hooks::queued_tokens(),
            tt_vector::work::hooks::outstanding_tokens()
        );
        let _ = run(&short, "doc.state.get", json!({}));
        std::thread::sleep(Duration::from_millis(5));
        if tt_vector::work::hooks::queued_tokens() == 0 && tt_vector::work::hooks::outstanding_tokens() <= 1 {
            // The probe call above may itself be outstanding for a moment.
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
