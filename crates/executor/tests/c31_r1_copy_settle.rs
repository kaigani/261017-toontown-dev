//! A copied drawing overwriting an existing entry must remain verbatim when the settle pass actually runs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
#[test]
fn overwritten_copy_is_verbatim_under_settle_not_only_when_new() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let notes = "scene:\"Intro\"/element:\"Notes\"";
    let source = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
    let nodes = get(&h, source)["nodes"].clone();
    let target = ok(&h, "element.drawing.create", json!({"elementId":notes,"name":"1","exposeFrom":1}))["drawingId"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(
        &h,
        "cell.range.move",
        json!({"sourceElementIds":[bob],"from":1,"to":6,"targetElementId":notes,"targetFrame":1,"mode":"copy","options":{"overwriteDrawings":true,"exposure":false}}),
    );
    assert_eq!(
        get(&h, &target)["nodes"],
        nodes,
        "M2 handoff item 6: an overwrite copy must preserve local IDs, strokes, owner/stretch records; keep_verbatim applies to existing targets too"
    );
}
