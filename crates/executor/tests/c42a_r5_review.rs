//! Independent named-element closure probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
const A: &str = "scene:#1/element:\"Bob\"";
fn setup() -> (ExecutorHandle, Value, Value, Value) {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":A,"frame":1}));
    let b = ok(&h, "element.create", json!({"type":"drawing","name":"Review B","sceneId":"scene:#1"}))["elementIds"][0]
        .clone();
    let d=ok(&h,"element.drawing.create",json!({"elementId":b,"name":"Target","exposeFrom":5,"exposeTo":5}))["drawingId"].clone();
    let s=ok(&h,"draw.rectangle.add",json!({"drawingId":d,"from":{"x":1000,"y":1000},"to":{"x":1100,"y":1100}}))["strokeIds"][0].clone();
    (h, b, d, s)
}
fn params(id: &str, s: &Value) -> Value {
    match id {
        "draw.line.add" | "draw.ellipse.add" => json!({"from":{"x":1020,"y":1020},"to":{"x":1080,"y":1080}}),
        "draw.selection.set" => json!({"by":"rect","rect":{"x":990,"y":990,"width":120,"height":120}}),
        "draw.pointSelection.set" => json!({"rect":{"x":990,"y":990,"width":120,"height":120}}),
        "draw.erase.apply" => json!({"path":[{"x":990,"y":1050},{"x":1110,"y":1050}]}),
        "draw.transform.apply" => json!({"ids":[s],"translate":{"x":10,"y":20}}),
        "colour.paint.apply" => json!({"points":[{"x":1050,"y":1050}]}),
        _ => json!({"point":{"x":1000,"y":1050}}),
    }
}
#[test]
fn second_element_eight_families_and_refusals() {
    for id in [
        "draw.line.add",
        "draw.ellipse.add",
        "draw.selection.set",
        "draw.pointSelection.set",
        "draw.erase.apply",
        "draw.transform.apply",
        "colour.paint.apply",
        "draw.hitTest.get",
    ] {
        let (h, b, d, s) = setup();
        let (r, _, rd, rs) = setup();
        let before = ok(&h, "draw.drawing.get", json!({"elementId":A,"frame":1}));
        let mut p = params(id, &s);
        p["elementId"] = b.clone();
        p["frame"] = json!(5);
        let mut q = params(id, &rs);
        q["drawingId"] = rd.clone();
        assert_eq!(ok(&h, id, p.clone()), ok(&r, id, q), "{id}");
        assert_eq!(
            ok(&h, "draw.drawing.get", json!({"drawingId":d})),
            ok(&r, "draw.drawing.get", json!({"drawingId":rd})),
            "target {id}"
        );
        assert_eq!(before, ok(&h, "draw.drawing.get", json!({"elementId":A,"frame":1})), "A unchanged {id}");
        for (element, frame) in [
            (json!("scene:#1/element:\"Absent\""), 5),
            (b.clone(), 999999),
            (json!("scene:#1/element:\"Dialogue\""), 5),
        ] {
            p["elementId"] = element;
            p["frame"] = json!(frame);
            let mut refusal = json!({"point":{"x":1000,"y":1050},"elementId":p["elementId"],"frame":p["frame"]});
            let e = err(&h, "draw.hitTest.get", refusal.take());
            println!("REFUSAL {id} {p}: {}", e.code);
            if p["elementId"].as_str().unwrap().contains("Absent") {
                assert_eq!(e.code, "ref.notFound");
            } else if p["elementId"].as_str().unwrap().contains("Dialogue") {
                assert_eq!(e.code, "drawing.notVectorElement");
            }
            if frame == 999999 {
                assert_eq!(e.code, "drawing.noTarget");
            }
        }
        if ["draw.line.add", "draw.erase.apply", "draw.transform.apply"].contains(&id) {
            p["elementId"] = b;
            p["frame"] = json!(5);
            ok(&h, "element.drawingLock.set", json!({"drawingIds":[d],"locked":true}));
            let av = h.availability(vec![(id.into(), p.as_object().unwrap().clone())]).unwrap();
            let e = err(&h, id, p);
            println!("LOCK {id}: {:?} {}", av[0], e.code);
            assert!(!av[0].enabled);
            assert_eq!(av[0].reason.as_deref(), Some(e.message.as_str()));
            assert_eq!(e.code, "drawing.locked");
        }
    }
}
