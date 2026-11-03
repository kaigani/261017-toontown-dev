//! Notes are cell properties: verify every shifting timing path with an independent expected destination.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
#[test]
fn every_shifting_operation_retimes_cell_notes() {
    let cases = vec![
        ("cell.exposure.add", json!({"elementIds":[BOB],"from":1,"to":1}), 10),
        ("cell.exposure.remove", json!({"elementIds":[BOB],"from":1,"to":1}), 8),
        ("cell.exposure.set", json!({"elementIds":[BOB],"from":1,"to":1,"length":2}), 5),
        ("cell.blank.add", json!({"elementIds":[BOB],"frame":1,"count":2}), 11),
        ("cell.blank.remove", json!({"elementIds":[BOB],"frame":7,"count":1}), 8),
        ("cell.range.add", json!({"elementIds":[BOB],"frame":1,"position":"before","count":2}), 11),
        ("cell.range.delete", json!({"elementIds":[BOB],"from":1,"to":2,"shift":true}), 7),
        ("cell.cycle.create", json!({"elementId":BOB,"from":1,"to":2,"cycles":2}), 11),
        (
            "cell.columnate.apply",
            json!({"elementId":BOB,"frame":1,"firstDrawing":"1","lastDrawing":"2","placement":"insert"}),
            11,
        ),
        ("cell.range.move", json!({"sourceElementIds":[BOB],"from":1,"to":2,"targetFrame":20,"mode":"move"}), 7),
        ("cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":2,"cut":true}), 7),
        ("cell.clipboard.apply", json!({"elementId":BOB,"frame":1}), 11),
    ];
    let mut failures = vec![];
    for (command, params, expected) in cases {
        let h = fixture();
        ok(&h, "cell.note.set", json!({"elementId":BOB,"frame":9,"text":"Review note"}));
        ok(&h, "cell.clipboard.set", json!({"elementIds":[BOB],"from":1,"to":2}));
        ok(&h, command, params);
        let state = get(&h, BOB);
        if state["cellNotes"][expected.to_string()] != "Review note" || state["cellNotes"]["9"] == "Review note" {
            failures.push(format!("{command}: expected note at {expected}, observed {}", state["cellNotes"]));
        }
    }
    assert!(failures.is_empty(), "04 sections 7.2/13: {}", failures.join("; "));
}
