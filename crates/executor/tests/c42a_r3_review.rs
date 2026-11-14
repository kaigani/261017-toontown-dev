//! Scoped closure probes against product 5d0c5d5.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;

#[test]
fn selection_frame_addresses_exposed_drawing() {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"", "frame":1}));
    let added = ok(
        &h,
        "draw.line.add",
        json!({"frame":200,"from":{"x":10000,"y":10000},"to":{"x":10100,"y":10000},"thickness":1}),
    );
    let explicit = ok(
        &h,
        "draw.selection.set",
        json!({"drawingId":added["drawingId"],"by":"point","point":{"x":10050,"y":10000}}),
    );
    assert!(!explicit["selectedIds"].as_array().unwrap().is_empty());
    let addressed = ok(&h, "draw.selection.set", json!({"frame":200,"by":"point","point":{"x":10050,"y":10000}}));
    assert_eq!(
        addressed["selectedIds"], explicit["selectedIds"],
        "frame must select in the drawing exposed at that frame (02 section 13.1 item 4)"
    );
}

#[test]
fn exact_hit_still_needs_view_when_default_tolerance_reaches_front_object() {
    let make = || {
        let h = fixture();
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"", "frame":1}));
        ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":4}));
        for y in [1000, 1002] {
            ok(
                &h,
                "draw.line.add",
                json!({"from":{"x":1000,"y":y},"to":{"x":1100,"y":y},"thickness":0.1,"layer":"new"}),
            );
        }
        h
    };
    let (a, b) = (make(), make());
    let expected = ok(&b, "draw.selection.set", json!({"by":"point","point":{"x":1050,"y":1000},"view":"drawingView"}));
    let p = ok(
        &a,
        "view.coordinates.convert",
        json!({"viewId":"drawingView","from":"drawing","to":"view","point":{"x":1050,"y":1000}}),
    )["point"]
        .clone();
    ok(&a, "tool.active.set", json!({"toolId":"select"}));
    let events = json!([{"type":"down","x":p["x"],"y":p["y"]},{"type":"up","x":p["x"],"y":p["y"]}]);
    let output = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":"select","events":events}));
    assert!(output.is_ok(), "{output:?}");
    let actual = ok(&a, "draw.selection.get", json!({}));
    assert_eq!(
        actual["selectedIds"], expected["selectedIds"],
        "02 section 13.3 requires the view for tolerances even on an exact hit"
    );
}
