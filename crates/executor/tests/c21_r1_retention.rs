//! The C2.1 object budget must not change historical diffs or checkpoints.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};
use tt_model::{document::Document, entities::drawing::Drawing};

#[test]
fn eviction_by_object_budget_preserves_exact_diff_and_checkpoint() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let at = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut drawing = serde_json::to_value(&parts.drawings[at]).unwrap();
    let template = drawing["nodes"][0]["strokes"][1].clone();
    drawing["nodes"][0]["strokes"] = Value::Array(
        (0..2100)
            .map(|i| {
                let mut s = template.clone();
                s["id"] = json!(format!("budget{i}"));
                s
            })
            .collect(),
    );
    parts.drawings[at] = tt_model::object::from_json::<Drawing>(&drawing).unwrap();
    let id = parts.drawings[at].id().as_str().to_owned();
    let original_pivot = drawing["pivot"].clone();
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: std::time::Duration::from_secs(120) }).unwrap();
    assert!(h.open_document(Document::from_parts(parts).unwrap(), None, false).is_ok());
    let before = rev(&h);
    ok(&h, "checkpoint.create", json!({"name":"object-budget-base"}));
    // 200 copies of >2100 objects exceed 400000 before the 256-record window.
    for i in 1..=200 {
        ok(&h, "draw.pivot.set", json!({"drawingId":id,"point":{"x":i,"y":0}}));
    }
    let by_rev = ok(&h, "doc.diff", json!({"sinceRev":before}));
    let by_cp = ok(&h, "doc.diff", json!({"sinceCheckpoint":"object-budget-base"}));
    assert_eq!(by_rev, by_cp, "budget-evicted snapshot reconstructed exactly");
    let early = ok(&h, "doc.diff", json!({"sinceRev":before,"untilRev":before+1}));
    assert!(early["changes"].to_string().contains("pivot"));
    ok(&h, "checkpoint.restore", json!({"name":"object-budget-base"}));
    assert_eq!(ok(&h, "draw.drawing.get", json!({"drawingId":id,"include":[]}))["pivot"], original_pivot);
}
