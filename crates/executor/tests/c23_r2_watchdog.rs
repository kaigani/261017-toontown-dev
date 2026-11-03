//! A request that times out while queued must never commit later (C23-R1-07).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::Duration;
use tt_executor::ExecutorConfig;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn timed_out_queued_mutation_is_cancelled_instead_of_committing_later() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(600) });
    let points: Vec<_> = (0..400).map(|i| json!({"x":2000+i,"y":2000})).collect();
    let s =
        ok(&h, "draw.stroke.add", json!({"drawingId":D,"kind":"invisible","fit":false,"layer":"new","points":points}));
    let initial = get(&h, D)["name"].clone();
    let initial_rev = rev(&h);
    // A single charge stays inside the test spin for two seconds. Both callers time out before it exits.
    tt_vector::work::hooks::set_spin_micros(2_000_000);
    let first = run(&h, "draw.smooth.apply", json!({"drawingId":D,"ids":s["strokeIds"]}));
    let second = run(&h, "draw.drawing.rename", json!({"drawingId":D,"name":"Timed out queued rename"}));
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(first.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"));
    assert_eq!(second.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"));
    std::thread::sleep(Duration::from_millis(1100));
    let after = get(&h, D);
    println!("initial name={initial}, final name={}, revisions {initial_rev} -> {}", after["name"], rev(&h));
    assert_eq!(after["name"], initial, "a timed-out queued caller must not mutate after returning an error");
    assert_eq!(rev(&h), initial_rev);
}
