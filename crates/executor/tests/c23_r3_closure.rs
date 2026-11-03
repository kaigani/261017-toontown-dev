//! Independent final-round closure variants; existing suites remain unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const D: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";
fn shape(cmd: &str, create: bool) -> Value {
    let mut p = if cmd == "draw.polyline.add" {
        json!({"points":[{"x":2000,"y":2000},{"x":2100,"y":2060}],"snap":true})
    } else {
        json!({"from":{"x":2000,"y":2000},"to":{"x":2100,"y":2060}})
    };
    p["elementId"] = json!(BOB);
    p["frame"] = json!(1);
    p["createDrawing"] = json!(create);
    p
}
fn state(h: &ExecutorHandle) -> Value {
    {
        let d = h.snapshot().unwrap().unwrap();
        json!({"drawings":d.drawings().collect::<Vec<_>>(),"banks":d.banks().collect::<Vec<_>>(),"elements":d.elements().collect::<Vec<_>>()})
    }
}
#[test]
fn all_shapes_false_in_second_batch_step_roll_back_and_two_true_share_one_cell() {
    for cmd in ["draw.rectangle.add", "draw.ellipse.add", "draw.line.add", "draw.polyline.add"] {
        let h = fixture();
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[BOB],"ops":[{"op":"replace","path":"/exposure","value":[[null,24]]}]}),
        );
        let before = state(&h);
        let v = rev(&h);
        let e = batch(
            &h,
            json!({"label":"false second step","steps":[{"id":"draw.drawing.rename","params":{"drawingId":D,"name":"must roll back"}},{"id":cmd,"params":shape(cmd,false)}]}),
        );
        assert!(!e.is_ok(), "{cmd}: {e:?}");
        assert_eq!(rev(&h), v);
        assert_eq!(state(&h), before, "{cmd}: failed atomic batch");
        let e = batch(
            &h,
            json!({"label":"same empty cell twice","steps":[{"id":cmd,"params":shape(cmd,true)},{"id":cmd,"params":shape(cmd,true)}]}),
        );
        assert!(e.is_ok(), "{cmd}: {e:?}");
        let steps = e.steps.unwrap();
        assert_eq!(steps[0].result.as_ref().unwrap()["drawingId"], steps[1].result.as_ref().unwrap()["drawingId"]);
        let after = state(&h);
        assert_eq!(after["drawings"].as_array().unwrap().len(), before["drawings"].as_array().unwrap().len() + 1);
        assert_eq!(rev(&h), v + 1);
        ok(&h, "edit.undo", json!({}));
        let mut restored = state(&h);
        let mut expected = before;
        strip_revs(&mut restored);
        strip_revs(&mut expected);
        assert_eq!(restored, expected);
    }
}
fn strip_revs(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.shift_remove("rev");
            for x in m.values_mut() {
                strip_revs(x)
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_revs),
        _ => {}
    }
}
fn rect(h: &ExecutorHandle, d: &str, x: i32) -> Value {
    ok(h, "draw.rectangle.add", json!({"drawingId":d,"layer":"new","from":{"x":x,"y":2000},"to":{"x":x+100,"y":2100}}))
}
fn selected(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.selection.get", json!({"drawingId":d}))["selectedIds"].clone()
}
#[test]
fn zone_pruning_undo_redo_is_session_state_and_drawing_isolation_holds() {
    let h = fixture();
    let doc = h.snapshot().unwrap().unwrap();
    let other = doc.drawings().find(|d| d.id().as_str() != D && !*d.locked()).unwrap().id().to_string();
    drop(doc);
    let a = rect(&h, D, 2000);
    let b = rect(&h, &other, 2000);
    let za = a["zoneIds"][0].clone();
    let zb = b["zoneIds"][0].clone();
    ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":[za]}));
    ok(&h, "draw.selection.set", json!({"drawingId":other,"ids":[zb]}));
    rect(&h, D, 3000);
    assert_eq!(selected(&h, D), json!([za]));
    assert_eq!(selected(&h, &other), json!([zb]));
    ok(
        &h,
        "draw.stroke.add",
        json!({"drawingId":D,"layer":a["layerId"],"kind":"invisible","fit":false,"points":[{"x":2050,"y":1990},{"x":2050,"y":2110}]}),
    );
    assert_eq!(selected(&h, D), json!([]));
    assert_eq!(selected(&h, &other), json!([zb]));
    // 02 7.1/13.1.10 require valid readback, not history of selection. Selection is transient (10 2.7).
    // Undo restores geometry/zone ID, but does not resurrect a pruned session selection.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(selected(&h, D), json!([]));
    ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":[za]}));
    assert_eq!(selected(&h, D), json!([za]));
    ok(&h, "edit.redo", json!({}));
    assert_eq!(selected(&h, D), json!([]));
    assert_eq!(selected(&h, &other), json!([zb]));
}
#[test]
fn set_colour_default_selection_changes_painted_zone_only() {
    let h = fixture();
    let cs: Vec<_> = h
        .snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().to_string()))
        .collect();
    let a = rect(&h, D, 2000);
    let b = rect(&h, D, 2200);
    let za = a["zoneIds"][0].clone();
    let zb = b["zoneIds"][0].clone();
    ok(&h, "colour.paint.apply", json!({"drawingId":D,"objectIds":[za],"colourId":cs[0]}));
    ok(&h, "draw.selection.set", json!({"drawingId":D,"ids":[za,zb]}));
    let v = rev(&h);
    let r = ok(&h, "colour.selection.apply", json!({"drawingId":D,"colourId":cs[4],"respectProtected":false}));
    assert_eq!(r["changed"], json!([za]));
    assert_eq!(rev(&h), v + 1);
    let d = ok(&h, "draw.drawing.get", json!({"drawingId":D}));
    for (layer, z, c) in [(&a["layerId"], &za, json!(cs[4])), (&b["layerId"], &zb, Value::Null)] {
        let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == *layer).unwrap();
        let actual = n["zones"].as_array().unwrap().iter().find(|v| v["id"] == *z).unwrap();
        assert_eq!(actual["colourId"], c);
    }
    assert_eq!(selected(&h, D), json!([za, zb]));
}
