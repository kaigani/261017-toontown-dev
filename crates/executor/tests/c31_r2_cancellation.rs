//! Independent session timeout and selection work-account variants; hook use serialized within this binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, Status};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn r14_over_million_selection_refuses_before_scan_and_next_read_answers() {
    let _lock = HOOK.lock().unwrap();
    let h = fixture();
    let cols = ok(&h, "element.create", json!({"type":"drawing","name":"Million731","count":11}))["elementIds"].clone();
    let start = Instant::now();
    let err = err(&h, "cell.select", json!({"elementIds":cols,"from":1,"to":100000}));
    assert_eq!(err.code, "limit.exceeded");
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(run(&h, "doc.state.get", json!({})).status, Status::Ok);
}
#[test]
fn r02_r14_timeout_after_solo_write_rolls_back_and_queued_read_answers() {
    let _lock = HOOK.lock().unwrap();
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(600) });
    let cols = ok(&h, "element.create", json!({"type":"drawing","name":"Scan731","count":10}))["elementIds"].clone();
    ok(&h, "element.solo.set", json!({"elementIds":[B],"enabled":true}));
    let before = ok(&h, "app.state.get", json!({}));
    let selection = ok(&h, "cell.selection.get", json!({}));
    tt_vector::work::hooks::set_spin_micros(20_000);
    let copy = h.clone();
    let worker = std::thread::spawn(move || {
        batch(
            &copy,
            json!({"label":"Timed out after solo","atomic":true,"steps":[
                {"id":"element.solo.set","params":{"enabled":false}},
                {"id":"cell.select","params":{"elementIds":cols,"from":1,"to":100000,"mode":"add"}}
            ]}),
        )
    });
    // Leave 350 ms of the read budget after cancellation on shared runners.
    std::thread::sleep(Duration::from_millis(350));
    let read = run(&h, "doc.state.get", json!({}));
    let result = worker.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(result.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{result:?}");
    assert_eq!(read.status, Status::Ok, "queued read after cancellation: {read:?}");
    assert_eq!(ok(&h, "app.state.get", json!({})), before);
    assert_eq!(ok(&h, "cell.selection.get", json!({})), selection);
}
