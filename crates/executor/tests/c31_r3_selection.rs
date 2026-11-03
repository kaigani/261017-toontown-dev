//! Timed selection refusal/cancellation with a queued read, in debug and release.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Status};
#[test]
fn measured_refusal_and_cancelled_scan_have_no_late_mutation() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(700) });
    let cols = ok(&h, "element.create", json!({"type":"drawing","name":"Scan928","count":11}))["elementIds"]
        .as_array()
        .unwrap()
        .clone();
    let before = ok(&h, "app.state.get", json!({}));
    let revision = rev(&h);
    let start = Instant::now();
    let e = err(&h, "cell.select", json!({"elementIds":cols,"from":1,"to":100000}));
    let refused = start.elapsed();
    assert_eq!(e.code, "limit.exceeded");
    let read_start = Instant::now();
    assert_eq!(run(&h, "doc.state.get", json!({})).status, Status::Ok);
    let refusal_read = read_start.elapsed();
    tt_vector::work::hooks::set_spin_micros(17000);
    let copied = h.clone();
    let first_ten = cols[..10].to_vec();
    let start = Instant::now();
    let worker = std::thread::spawn(move || {
        run(&copied, "cell.select", json!({"elementIds":first_ten,"from":1,"to":100000,"mode":"replace"}))
    });
    std::thread::sleep(Duration::from_millis(120));
    let read_start = Instant::now();
    let queued = run(&h, "doc.state.get", json!({}));
    let queued_elapsed = read_start.elapsed();
    let result = worker.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(result.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{result:?}");
    assert_eq!(queued.status, Status::Ok, "{queued:?}");
    assert_eq!(ok(&h, "app.state.get", json!({})), before);
    assert_eq!(rev(&h), revision);
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(ok(&h, "app.state.get", json!({})), before);
    println!(
        "{}",
        json!({"profile":if cfg!(debug_assertions){"debug"}else{"release"},"refusalSeconds":refused.as_secs_f64(),"readAfterRefusalSeconds":refusal_read.as_secs_f64(),"cancelAndReadSeconds":start.elapsed().as_secs_f64(),"queuedReadSeconds":queued_elapsed.as_secs_f64(),"noLateMutation":true})
    );
}
