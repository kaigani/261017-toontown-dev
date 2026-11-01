//! Request ownership, dequeue-within-deadline and atomic batch cancellation variants.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::ExecutorConfig;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn cancellation_during_running_queued_call_batch_and_planar_settle() {
    let h = fixture_with(ExecutorConfig { budget: Duration::from_millis(800) });
    let points: Vec<_> = (0..400).map(|i| json!({"x":2000+i,"y":2000})).collect();
    let s =
        ok(&h, "draw.stroke.add", json!({"drawingId":D,"kind":"invisible","fit":false,"layer":"new","points":points}));
    let initial = rev(&h);
    let name = get(&h, D)["name"].clone();
    tt_vector::work::hooks::set_spin_micros(3000);
    let a = h.clone();
    let ids = s["strokeIds"].clone();
    let start = Instant::now();
    let first = std::thread::spawn(move || run(&a, "draw.smooth.apply", json!({"drawingId":D,"ids":ids})));
    std::thread::sleep(Duration::from_millis(200));
    // This request is dequeued after the first cancels, before its own caller deadline.
    let second = run(&h, "draw.smooth.apply", json!({"drawingId":D,"ids":s["strokeIds"]}));
    let elapsed = start.elapsed();
    let first = first.join().unwrap();
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(first.error.unwrap().code, "command.timeout");
    assert_eq!(second.error.unwrap().code, "command.timeout");
    assert!(
        elapsed >= Duration::from_millis(980),
        "second request must have started and consumed its remaining timeout window: {elapsed:?}"
    );
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    assert_eq!(rev(&h), initial);
    tt_vector::work::hooks::set_spin_micros(3000);
    let t = Instant::now();
    let b = batch(
        &h,
        json!({"label":"cancel after first step","steps":[{"id":"draw.drawing.rename","params":{"drawingId":D,"name":"must not commit"}},{"id":"draw.smooth.apply","params":{"drawingId":D,"ids":s["strokeIds"]}}]}),
    );
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(b.error.unwrap().code, "command.timeout");
    assert!(run(&h, "doc.state.get", json!({})).is_ok());
    assert_eq!(rev(&h), initial);
    assert_eq!(get(&h, D)["name"], name);
    println!("atomic batch raw seconds={}", t.elapsed().as_secs_f64());
    // New geometry reaches planar work, times out, and leaves no stroke or revision behind.
    let before = get(&h, D);
    tt_vector::work::hooks::set_spin_micros(1_000_000);
    let t = Instant::now();
    let e = run(
        &h,
        "draw.rectangle.add",
        json!({"drawingId":D,"from":{"x":3000,"y":3000},"to":{"x":3100,"y":3100},"layer":"new"}),
    );
    tt_vector::work::hooks::set_spin_micros(0);
    assert_eq!(e.error.unwrap().code, "command.timeout");
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(get(&h, D), before);
    assert_eq!(rev(&h), initial);
    println!("planar raw seconds={}", t.elapsed().as_secs_f64());
    // Distinct executor instances may allocate the same local numbers; their cancellation domains are independent.
    let other = fixture_with(ExecutorConfig { budget: Duration::from_millis(800) });
    for i in 0..8 {
        ok(&other, "draw.drawing.rename", json!({"drawingId":D,"name":format!("other {i}")}));
        ok(&h, "draw.drawing.rename", json!({"drawingId":D,"name":format!("first {i}")}));
    }
}
