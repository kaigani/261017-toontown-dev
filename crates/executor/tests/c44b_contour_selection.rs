//! M4 C4.4b, the Contour Editor (design note 3 §3): the point selection keeps naming the same anchors when anchors are
//! inserted or deleted (it used to keep indices while they were in range, so after a deletion it named the anchors
//! that took the deleted ones' places).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod common;
use common::*;
use serde_json::{Value, json};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

/// A closed four-anchor stroke on a new layer, away from the sample's content: its local id.
fn square(h: &tt_executor::ExecutorHandle) -> String {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 400}, "layer": "new" }),
    );
    r["strokeIds"][0].as_str().unwrap().to_owned()
}

fn selected(h: &tt_executor::ExecutorHandle) -> Vec<(String, u64)> {
    let s = ok(h, "draw.selection.get", json!({ "drawingId": D1 }));
    let mut v: Vec<(String, u64)> = s["selectedPoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p: &Value| (p["strokeId"].as_str().unwrap().to_owned(), p["anchor"].as_u64().unwrap()))
        .collect();
    v.sort();
    v
}

fn select(h: &tt_executor::ExecutorHandle, sid: &str, anchors: &[u32]) {
    let points: Vec<Value> = anchors.iter().map(|a| json!({ "strokeId": sid, "anchor": a })).collect();
    ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": points, "mode": "replace" }));
}

#[test]
fn deleting_the_selected_anchors_empties_the_selection() {
    let h = fixture();
    let sid = square(&h);
    select(&h, &sid, &[0, 1]);
    ok(&h, "draw.point.delete", json!({ "drawingId": D1 }));
    assert_eq!(selected(&h), vec![], "nothing selected names the anchors that took the deleted ones' places");
}

#[test]
fn deleting_an_earlier_anchor_moves_the_selection_down_with_its_anchors() {
    let h = fixture();
    let sid = square(&h);
    let corner = |h: &tt_executor::ExecutorHandle, k: usize| {
        let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
        let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == sid.as_str()).cloned().unwrap();
        n["bezier"][k * 3].clone()
    };
    // Anchors 2 and 3 selected; anchor 0 deleted (the square keeps three anchors).
    let (a2, a3) = (corner(&h, 2), corner(&h, 3));
    select(&h, &sid, &[2, 3]);
    ok(&h, "draw.point.delete", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
    assert_eq!(selected(&h), vec![(sid.clone(), 1), (sid.clone(), 2)]);
    assert_eq!((corner(&h, 1), corner(&h, 2)), (a2, a3), "the same anchors stay selected");
}

#[test]
fn inserting_an_anchor_before_a_selected_one_moves_it_up() {
    let h = fixture();
    let sid = square(&h);
    select(&h, &sid, &[0, 2]);
    // A new anchor in segment 0 becomes anchor 1: the selected anchor 2 is now anchor 3; anchor 0 stays.
    ok(&h, "draw.point.add", json!({ "drawingId": D1, "strokeId": sid, "segment": 0, "t": 0.5 }));
    assert_eq!(selected(&h), vec![(sid.clone(), 0), (sid.clone(), 3)]);
}
