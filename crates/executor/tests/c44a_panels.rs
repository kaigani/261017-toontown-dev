//! M4 C4.4a kernel rules behind the panels (`reviews/M4-design-panels.md`).
mod common;

use common::*;
use serde_json::{Value, json};

fn current_brush(h: &tt_executor::ExecutorHandle) -> String {
    let list = ok(h, "tool.pen.list", json!({ "type": "brush" }));
    list["currentBrushPenId"].as_str().unwrap_or_default().to_owned()
}

fn pen_id(h: &tt_executor::ExecutorHandle, name: &str) -> String {
    let list = ok(h, "tool.pen.list", json!({ "type": "brush" }));
    let pens = list["pens"].as_array().cloned().unwrap_or_default();
    pens.iter()
        .find(|p| p["name"] == name)
        .and_then(|p| p["penId"].as_str().or_else(|| p["id"].as_str()))
        .unwrap_or_else(|| panic!("no pen {name}: {list}"))
        .to_owned()
}

fn selected_events(rx: &std::sync::mpsc::Receiver<tt_executor::Event>) -> Vec<Value> {
    rx.try_iter()
        .filter(|e| e.name == "tool.pen.changed" && e.payload["change"] == "selected")
        .map(|e| e.payload)
        .collect()
}

/// SI-167 (design note 1 §3): deleting the current pen makes the next pen of its type current, else the previous, and
/// announces it, so the Pen panel and the cursor follow; deleting another pen leaves the current one.
#[test]
fn deleting_the_current_pen_makes_the_next_one_current_and_says_so() {
    let h = fixture();
    let a =
        ok(&h, "tool.pen.create", json!({ "type": "brush", "name": "Panel A" }))["penId"].as_str().unwrap().to_owned();
    let b =
        ok(&h, "tool.pen.create", json!({ "type": "brush", "name": "Panel B" }))["penId"].as_str().unwrap().to_owned();
    ok(&h, "tool.pen.select", json!({ "penId": a }));
    assert_eq!(current_brush(&h), a);
    let rx = h.subscribe();
    // A pen that is not current: the current one stays, nothing is selected.
    let other = pen_id(&h, "Panel B");
    assert_eq!(other, b);
    ok(&h, "tool.pen.select", json!({ "penId": a }));
    let _ = selected_events(&rx);
    // The current pen deleted: the next of its type is current and announced.
    let deleted = ok(&h, "tool.pen.delete", json!({ "penId": a }));
    assert_eq!(deleted["currentPenId"], b, "{deleted}");
    assert_eq!(current_brush(&h), b);
    let ev = selected_events(&rx);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["penId"], b);
    // The last of the list deleted while current: the previous one is current.
    let before_b = {
        let list = ok(&h, "tool.pen.list", json!({ "type": "brush" }));
        let ids: Vec<String> = list["pens"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p["penId"].as_str().or_else(|| p["id"].as_str()).map(str::to_owned))
            .collect();
        let at = ids.iter().position(|x| *x == b).unwrap();
        assert_eq!(at + 1, ids.len(), "Panel B is the last brush pen");
        ids[at - 1].clone()
    };
    ok(&h, "tool.pen.delete", json!({ "penId": b }));
    assert_eq!(current_brush(&h), before_b);
    assert_eq!(selected_events(&rx).iter().map(|e| e["penId"].clone()).collect::<Vec<_>>(), vec![json!(before_b)]);
}
