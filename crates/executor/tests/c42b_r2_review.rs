//! Round-two adversarial executor review.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
#[path = "../../render/tests/c42b_r2_review.rs"]
mod independent_decoder;
#[test]
fn five_executor_drawings_pass_independent_decoder() {
    for kind in ["brush", "pencil", "invisible", "rectangle", "ellipse"] {
        let h = ready();
        if kind == "rectangle" || kind == "ellipse" {
            ok(
                &h,
                if kind == "rectangle" { "draw.rectangle.add" } else { "draw.ellipse.add" },
                json!({"from":{"x":300,"y":100},"to":{"x":400,"y":160}}),
            );
        } else {
            ok(
                &h,
                "draw.stroke.add",
                json!({"kind":kind,"points":[{"x":300,"y":100,"pressure":0.3},{"x":330,"y":140,"pressure":0.8},{"x":400,"y":160,"pressure":0.6}],"fit":false}),
            );
        }
        let p = h.canvas_drawing(None, 7, 0, Default::default()).unwrap();
        let identity = independent_decoder::decode(&p.bytes).unwrap();
        assert_eq!(identity, (7, p.rev));
    }
}
#[cfg(test)]
mod selection_review {
    use super::*;
    fn rectangle(h: &ExecutorHandle) -> [f64; 4] {
        let r = ok(h, "draw.rectangle.add", json!({"from":{"x":300,"y":100},"to":{"x":400,"y":160}}));
        ok(h, "draw.selection.set", json!({"by":"ids","ids":r["strokeIds"]}));
        bounds(h)
    }
    fn bounds(h: &ExecutorHandle) -> [f64; 4] {
        let b = ok(h, "draw.selection.get", json!({}))["bounds"].clone();
        let f = |k: &str| b[k].as_f64().unwrap();
        [f("x"), f("y"), f("x") + f("width"), f("y") + f("height")]
    }
    fn pointer(h: &ExecutorHandle, a: (f64, f64), b: (f64, f64), zoom: f64, shift: bool) -> tt_executor::Envelope {
        let ev = |kind, x, y| json!({"type":kind,"x":960.0+zoom*x,"y":540.0-zoom*y,"modifiers":if shift{vec!["shift"]}else{vec![]}});
        h.pointer(
            tt_executor::Origin::User,
            json!({"view":"drawing","tool":"select","events":[ev("down",a.0,a.1),ev("up",b.0,b.1)]}),
        )
    }
    fn history(h: &ExecutorHandle) -> Vec<Value> {
        ok(h, "edit.history.get", json!({}))["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| json!({"label":e["label"],"rev":e["rev"]}))
            .collect()
    }
    #[test]
    fn handle_equivalence() {
        // Note 7.3: scale ratios about opposite corner/side; constrained corner uses larger magnitude;
        // rotate by signed angular displacement about centre; interior moves by release minus press.
        for case in 0..5 {
            for zoom in [1.0, 2.0] {
                let (a, b) = (ready(), ready());
                let q = rectangle(&a);
                assert_eq!(rectangle(&b), q);
                for h in [&a, &b] {
                    ok(h, "view.zoom.set", json!({"viewId":"drawingView","zoom":zoom}));
                }
                let [x0, y0, x1, y1] = q;
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let (w, ht) = (x1 - x0, y1 - y0);
                let (press, release, params) = match case {
                    0 => (
                        (x1, y1),
                        (x0 + 1.5 * w, y0 + 2.0 * ht),
                        json!({"scale":{"x":1.5,"y":2.0},"pivot":{"x":x0,"y":y0}}),
                    ),
                    1 => (
                        (x1, cy),
                        (x0 + 1.5 * w, cy + 20.0),
                        json!({"scale":{"x":1.5,"y":1.0},"pivot":{"x":x0,"y":cy}}),
                    ),
                    2 => (
                        (x1, y1),
                        (x0 + 1.5 * w, y0 + 2.0 * ht),
                        json!({"scale":{"x":2.0,"y":2.0},"pivot":{"x":x0,"y":y0}}),
                    ),
                    3 => {
                        let p = (x1 + 8.0 / zoom, y1 + 8.0 / zoom);
                        let r = (cx - (p.1 - cy), cy + (p.0 - cx));
                        let angle = ((r.1 - cy).atan2(r.0 - cx) - (p.1 - cy).atan2(p.0 - cx)).to_degrees();
                        (p, r, json!({"rotate":angle,"pivot":{"x":cx,"y":cy}}))
                    }
                    _ => ((cx, cy), (cx + 30.0, cy - 20.0), json!({"translate":{"x":30.0,"y":-20.0},"copy":false})),
                };
                let before = content(&a);
                let steps = history(&a).len();
                let e = pointer(&a, press, release, zoom, case == 2);
                assert!(e.is_ok(), "case {case} zoom {zoom}: {e:?}");
                let c = e.result.unwrap()["commands"][1].clone();
                assert_eq!(c["commandId"], "draw.transform.apply");
                assert_eq!(c["params"], params, "case {case} zoom {zoom}");
                ok(&b, "tool.active.set", json!({"toolId":"select"}));
                ok(&b, "draw.transform.apply", params);
                assert_eq!(canonical(&a), canonical(&b));
                assert_eq!(history(&a), history(&b));
                assert_eq!(history(&a).len(), steps + 1);
                ok(&a, "edit.undo", json!({}));
                assert_eq!(content(&a), before);
            }
        }
    }
    #[test]
    fn scene_selection_tracks_queries_and_changes() {
        let h = ready();
        ok(&h, "draw.selection.set", json!({"by":"none"}));
        assert!(h.canvas_scene("drawingView").unwrap().selection.is_none());
        let a = rectangle(&h);
        let ids = ok(&h, "draw.selection.get", json!({}))["selectedIds"].clone();
        let s = h.canvas_scene("drawingView").unwrap().selection.unwrap();
        assert_eq!(s.bounds, Some(a));
        assert_eq!(s.objects, vec![a]);
        let r = ok(&h, "draw.rectangle.add", json!({"from":{"x":500,"y":200},"to":{"x":550,"y":250}}));
        let mut all = ids.as_array().unwrap().clone();
        all.extend(r["strokeIds"].as_array().unwrap().clone());
        ok(&h, "draw.selection.set", json!({"by":"ids","ids":all}));
        assert_eq!(h.canvas_scene("drawingView").unwrap().selection.unwrap().bounds, Some(bounds(&h)));
        ok(&h, "draw.transform.apply", json!({"translate":{"x":10,"y":20}}));
        let s = h.canvas_scene("drawingView").unwrap().selection.unwrap();
        assert_eq!(s.bounds, Some(bounds(&h)));
        ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":2}));
        assert_eq!(h.canvas_scene("drawingView").unwrap().selection.unwrap(), s);
        ok(&h, "view.pan.set", json!({"viewId":"drawingView","centre":{"x":20,"y":30}}));
        ok(&h, "view.rotation.set", json!({"viewId":"drawingView","degrees":30}));
        assert_eq!(h.canvas_scene("drawingView").unwrap().selection.unwrap(), s);
        ok(&h, "draw.selection.set", json!({"by":"none"}));
        ok(&h, "draw.pointSelection.set", json!({"points":[{"strokeId":ids[0],"anchor":0}]}));
        let s = h.canvas_scene("drawingView").unwrap().selection.unwrap();
        assert_eq!(s.bounds, None);
        assert_eq!(s.points.len(), 1);
        assert_eq!(s.points, vec![[310.0, 120.0]]);
        assert_eq!(ok(&h, "draw.selection.get", json!({}))["selectedPoints"], json!([{"strokeId":ids[0],"anchor":0}]));
    }
    #[test]
    fn zoomed_priority_minimum_and_flip() {
        let h = ready();
        let [x0, y0, x1, y1] = rectangle(&h);
        ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":2}));
        let before = canonical(&h);
        let e = pointer(&h, (x1, y1), (x0, y0), 2.0, false);
        assert_eq!(e.error.unwrap().code, "arg.invalid");
        assert_eq!(canonical(&h), before);
        // Four view pixels beyond a corner is still within its six-pixel disc at zoom two.
        let e = pointer(&h, (x1 + 2.0, y1), (x1 + 20.0, y1 + 10.0), 2.0, false);
        assert!(e.is_ok());
        let p = e.result.unwrap()["commands"][1]["params"].clone();
        assert!(p.get("scale").is_some(), "handle precedes rotation zone");
        ok(&h, "edit.undo", json!({}));
        let e = pointer(&h, (x1 + 4.0, y1), (x1 + 20.0, y1 + 20.0), 2.0, false);
        assert!(e.is_ok());
        assert!(
            e.result.unwrap()["commands"][1]["params"].get("rotate").is_some(),
            "eight view pixels is outside the six-pixel handle disc"
        );
        ok(&h, "edit.undo", json!({}));
        let e = pointer(&h, (x1 - 2.0, y1 - 1.0), (x1 + 20.0, y1 + 20.0), 2.0, false);
        assert!(e.is_ok());
        assert!(e.result.unwrap()["commands"][1]["params"].get("scale").is_some(), "handle precedes box interior");
        ok(&h, "edit.undo", json!({}));
        let before = canonical(&h);
        let e = pointer(&h, (x1, y1), (x0 + 0.0005 * (x1 - x0), y0 + 0.0005 * (y1 - y0)), 2.0, false);
        assert_eq!(e.error.unwrap().code, "arg.invalid");
        assert_eq!(canonical(&h), before);
        let e = pointer(&h, (x1, y1), (x0 - (x1 - x0), y0 - (y1 - y0)), 2.0, false);
        assert!(e.is_ok(), "negative factors are permitted: {e:?}");
        assert_eq!(e.result.unwrap()["commands"][1]["params"]["scale"], json!({"x":-1.0,"y":-1.0}));
        ok(&h, "edit.undo", json!({}));
        ok(&h, "draw.pivot.set", json!({"point":{"x":x1,"y":y1}}));
        let e = pointer(&h, (x1, y1), (x1 + 20.0, y1 + 20.0), 2.0, false);
        assert_eq!(e.result.unwrap()["commands"][1]["commandId"], "draw.pivot.set");
        // D-16: pixel hit tolerance must name the zoomed view on a point-selection gesture.
        let e=h.pointer(tt_executor::Origin::User,json!({"view":"drawing","tool":"select","events":[{"type":"down","x":100,"y":100},{"type":"up","x":100,"y":100}]}));
        assert_eq!(e.result.unwrap()["commands"][1]["params"]["view"], "drawingView");
    }
    #[test]
    fn zero_extent_boxes() {
        for vertical in [false, true] {
            let h = ready();
            let p = if vertical {
                json!([{"x":300,"y":100},{"x":300,"y":160}])
            } else {
                json!([{"x":300,"y":100},{"x":400,"y":100}])
            };
            let r = ok(&h, "draw.stroke.add", json!({"kind":"invisible","points":p,"fit":false}));
            ok(&h, "draw.selection.set", json!({"by":"ids","ids":r["strokeIds"]}));
            let [x0, y0, x1, y1] = bounds(&h);
            assert!(x0 == x1 || y0 == y1);
            let before = content(&h);
            let e = pointer(&h, (x1, y1), (x1 + 30.0, y1 + 30.0), 1.0, false);
            assert!(e.is_ok(), "{e:?}");
            let p = e.result.unwrap()["commands"][1]["params"].clone();
            assert_eq!(p["scale"][if vertical { "x" } else { "y" }], 1.0, "zero extent axis retains factor one");
            ok(&h, "edit.undo", json!({}));
            assert_eq!(content(&h), before);
        }
    }
}
use common::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tt_executor::ExecutorHandle;
fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    h
}
fn canonical(h: &ExecutorHandle) -> Vec<u8> {
    let d = h.snapshot().unwrap().unwrap();
    let entries: Vec<_> =
        d.entry_keys().iter().map(|k| tt_persist::save::entry_text(&d, &Default::default(), k).unwrap()).collect();
    serde_json::to_vec(&entries).unwrap()
}
fn content(h: &ExecutorHandle) -> Value {
    let d = h.snapshot().unwrap().unwrap();
    let mut v: Value = serde_json::from_slice(&canonical(h)).unwrap();
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("rev");
                for v in m.values_mut() {
                    strip(v)
                }
            }
            Value::Array(a) => {
                for v in a {
                    strip(v)
                }
            }
            _ => {}
        }
    }
    let a = v.as_array_mut().unwrap();
    for x in a {
        let mut obj: Value = serde_json::from_str(x.as_str().unwrap()).unwrap();
        strip(&mut obj);
        *x = obj;
    }
    drop(d);
    v
}
fn stroke(h: &ExecutorHandle, n: u32) {
    ok(
        h,
        "draw.stroke.add",
        json!({"kind":"pencil","points":[{"x":300+n,"y":100},{"x":320+n,"y":120}],"fit":false,"layer":"new"}),
    );
}
#[test]
fn fifty_append_undo_redo_cycles() {
    let h = ready();
    for i in 0..50 {
        let before = content(&h);
        stroke(&h, i);
        let after = content(&h);
        ok(&h, "edit.undo", json!({}));
        assert_eq!(content(&h), before, "undo cycle {i}");
        ok(&h, "edit.redo", json!({}));
        assert_eq!(content(&h), after, "redo cycle {i}");
    }
}
#[test]
fn seeded_script_profile_receipt() {
    let h = ready();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":9}));
    ok(&h, "element.drawingLock.set", json!({"locked":false}));
    let seed = 0xc42b_0201u64;
    let mut rng = seed;
    let rx = h.subscribe();
    let mut events = Vec::new();
    eprintln!("seed={seed:#x}");
    for i in 0..320 {
        if i % 16 == 0 {
            ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":if i%32==0{1}else{9}}));
        }
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let x = 300 + (rng % 100) as u32;
        match rng % 8 {
            0 => {
                ok(&h, "draw.rectangle.add", json!({"from":{"x":x,"y":100},"to":{"x":x+20,"y":130},"layer":"new"}));
            }
            1 => {
                ok(&h, "draw.ellipse.add", json!({"from":{"x":x,"y":100},"to":{"x":x+20,"y":130},"layer":"new"}));
            }
            2 => {
                ok(
                    &h,
                    "draw.stroke.add",
                    json!({"kind":"brush","points":[{"x":x,"y":100,"pressure":0.2},{"x":x+10,"y":120,"pressure":0.8}],"fit":false,"layer":"top"}),
                );
            }
            3 => {
                ok(&h, "draw.selection.set", json!({"by":"all"}));
                ok(&h, "draw.transform.apply", json!({"translate":{"x":1,"y":-1}}));
            }
            4 => {
                ok(&h, "edit.undo", json!({}));
                ok(&h, "edit.redo", json!({}));
            }
            6 => {
                let before = canonical(&h);
                let e = run(&h, "draw.erase.apply", json!({"path":[{"x":1000,"y":1000},{"x":1010,"y":1010}]}));
                if let Some(e) = e.error {
                    assert!(["drawing.nothingToDo", "colour.zoneNotClosed"].contains(&e.code.as_str()));
                    assert_eq!(canonical(&h), before);
                }
            }
            7 => {
                let before = canonical(&h);
                let e = run(&h, "colour.paint.apply", json!({"points":[{"x":310,"y":110}]}));
                if let Some(e) = e.error {
                    assert!(["drawing.nothingToDo", "colour.zoneNotClosed"].contains(&e.code.as_str()));
                    assert_eq!(canonical(&h), before);
                }
            }
            _ => stroke(&h, i % 100),
        }
        events.extend(rx.try_iter());
    }
    let bytes = canonical(&h);
    let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reviews/evidence/M4-C4.2b-r2");
    std::fs::create_dir_all(&root).unwrap();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    std::fs::write(root.join(format!("A2-{profile}.sha256")), format!("{hash}\n")).unwrap();
    let other = root.join(if cfg!(debug_assertions) { "A2-release.sha256" } else { "A2-debug.sha256" });
    if other.exists() {
        assert_eq!(std::fs::read_to_string(other).unwrap().trim(), hash);
    }
    let event_bytes = serde_json::to_vec(&events).unwrap();
    let event_hash: String = Sha256::digest(&event_bytes).iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(root.join(format!("A2-{profile}-events.sha256")), format!("{event_hash}\n")).unwrap();
    let other = root.join(if cfg!(debug_assertions) { "A2-release-events.sha256" } else { "A2-debug-events.sha256" });
    if other.exists() {
        assert_eq!(std::fs::read_to_string(other).unwrap().trim(), event_hash);
    }
    if !cfg!(debug_assertions) {
        let doc = h.snapshot().unwrap().unwrap();
        let input = tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &Default::default(),
            blobs: &Default::default(),
            session: None,
            saved_at: "2026-11-14T00:00:00Z".into(),
        };
        tt_persist::save::save_folder(&root.join("release-project"), &input, None, Default::default()).unwrap();
    } else if root.join("release-project").exists() {
        let loaded = tt_persist::load::load(&root.join("release-project"), &Default::default()).unwrap();
        let entries: Vec<_> = loaded
            .doc
            .entry_keys()
            .iter()
            .map(|k| tt_persist::save::entry_text(&loaded.doc, &loaded.preserved, k).unwrap())
            .collect();
        assert_eq!(serde_json::to_vec(&entries).unwrap(), bytes, "release saved project reopened in debug");
    }
}
#[test]
fn locked_and_invalid_append_leave_content_unchanged() {
    let h = ready();
    let before = canonical(&h);
    err(&h, "draw.stroke.add", json!({"kind":"pencil","points":[{"x":0,"y":0,"pressure":-1}]}));
    assert_eq!(canonical(&h), before);
    ok(&h, "element.drawingLock.set", json!({"locked":true}));
    let before = canonical(&h);
    err(&h, "draw.stroke.add", json!({"kind":"pencil","points":[{"x":1,"y":1},{"x":2,"y":2}]}));
    assert_eq!(canonical(&h), before);
}

#[test]
fn append_other_drawing_edit_and_symmetry() {
    let h = ready();
    let before = content(&h);
    stroke(&h, 1);
    let appended = content(&h);
    let d = h.snapshot().unwrap().unwrap();
    let other = d.drawings().find(|x| x.name() == "2").unwrap().id().as_str().to_owned();
    drop(d);
    ok(&h, "doc.object.patch", json!({"targets":[other],"props":{"name":"second"}}));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), appended);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), before);
    ok(&h, "tool.symmetry.set", json!({"mode":"kaleidoscope","mirrors":4,"angle":0}));
    let before = content(&h);
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({"kind":"pencil","points":[{"x":300,"y":100},{"x":320,"y":120}],"symmetry":true}),
    );
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 8);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(content(&h), before);
}
#[test]
fn atomic_appends_to_two_layers_or_drawings() {
    for two_drawings in [false, true] {
        let h = ready();
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":9}));
        ok(&h, "element.drawingLock.set", json!({"locked":false}));
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
        let doc = h.snapshot().unwrap().unwrap();
        let a = doc.drawings().find(|d| d.name() == "1").unwrap().id().as_str().to_owned();
        let b = if two_drawings {
            doc.drawings().find(|d| d.name() == "2").unwrap().id().as_str().to_owned()
        } else {
            a.clone()
        };
        drop(doc);
        let before = content(&h);
        let steps:Vec<_>=[a,b].into_iter().map(|id|json!({"id":"draw.stroke.add","params":{"drawingId":id,"kind":"pencil","points":[{"x":300,"y":100},{"x":320,"y":120}],"layer":"new"}})).collect();
        let e = batch(&h, json!({"label":"two appends","steps":steps,"atomic":true}));
        assert!(e.is_ok(), "{e:?}");
        let after = content(&h);
        assert_ne!(after, before);
        ok(&h, "edit.undo", json!({}));
        assert_eq!(content(&h), before);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(content(&h), after);
    }
}
