//! C23-R3-01 - intentionally failing proving test: a UI-only drawing selection must not apply after its caller timed out.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, ExecutorHandle};
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
#[test]
fn timed_out_selection_must_not_apply_session_state_later() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let index = parts.drawings.iter().position(|d| d.id().as_str() == D).unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[index]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    let mut layer = drawing["nodes"][0].clone();
    layer["strokes"] = json!(
        (0..3000)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("st_review_{i}"));
                s["points"] = json!([[i * 3, 2000, 0, 0, 0, 0], [i * 3 + 1, 2001, 0, 0, 0, 0]]);
                s
            })
            .collect::<Vec<_>>()
    );
    drawing["nodes"] = json!([layer]);
    parts.drawings[index] = tt_model::object::from_json(&drawing).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_millis(200) }).unwrap();
    let open = h.open_document(doc, None, false);
    assert!(open.is_ok(), "setup: {open:?}");
    let v = rev(&h);
    let t = Instant::now();
    let e = run(&h, "draw.selection.set", json!({"drawingId":D,"by":"all"}));
    println!("callerSeconds={} error={:?}", t.elapsed().as_secs_f64(), e.error);
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "the probe must cross the budget");
    let mut state = None;
    for _ in 0..100 {
        let e = run(&h, "app.state.get", json!({}));
        if e.is_ok() {
            state = e.result;
            break;
        }
    }
    let state = state.expect("worker must eventually recover");
    let ids = state["selection"]["ids"].as_array().unwrap();
    println!("afterSeconds={} selected={} rev={}", t.elapsed().as_secs_f64(), ids.len(), rev(&h));
    assert_eq!(rev(&h), v);
    assert!(
        ids.is_empty(),
        "a timed-out selection changed session state after returning its error: {} selected",
        ids.len()
    );
}
