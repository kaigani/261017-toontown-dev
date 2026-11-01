//! M2 C2.3 readiness item 2: every new mutation passes the common contract on the sample project: a dry run writes
//! nothing; the call is one revision with one `draw.drawing.changed` event; undo restores the exact read-back and redo
//! re-applies it; the saved and reopened project reads back the same; a locked drawing is refused.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const BRUSH: &str = "st_01K742SG0QD1F2HFZSX8R4V7WP";
const CAPTION: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";
const GROUP: &str = "nd_01K742SG0TX3WF1BSWWGMWM5WG";

fn strip_revs(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.shift_remove("rev");
            for x in m.values_mut() {
                strip_revs(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_revs),
        _ => {}
    }
}

fn readback(h: &ExecutorHandle) -> Value {
    let mut v = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    strip_revs(&mut v);
    v
}

fn cases() -> Vec<(&'static str, Value)> {
    let colours = {
        let h = fixture();
        let v: Vec<String> = h
            .snapshot()
            .unwrap()
            .unwrap()
            .palettes()
            .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
            .collect();
        v
    };
    let q = |p: Value| {
        let mut p = p;
        p["drawingId"] = json!(D1);
        p
    };
    vec![
        ("draw.transform.apply", q(json!({ "ids": [PENCIL], "translate": {"x": 7, "y": -3}, "rotate": 10 }))),
        ("draw.flip.apply", q(json!({ "ids": [BRUSH], "axis": "vertical" }))),
        ("draw.rotate.apply", q(json!({ "ids": [CAPTION], "by": "90cw" }))),
        (
            "draw.envelope.apply",
            q(
                json!({ "ids": [PENCIL], "corners": [{"x": -60, "y": -60}, {"x": 60, "y": -50}, {"x": 40, "y": 60}, {"x": -40, "y": 50}] }),
            ),
        ),
        ("draw.point.add", json!({ "strokeId": format!("{D1}/{PENCIL}"), "segment": 0, "t": 0.25 })),
        ("draw.point.move", q(json!({ "points": [{"strokeId": PENCIL, "anchor": 0}], "delta": {"x": -5, "y": 2} }))),
        (
            "draw.point.set",
            json!({ "strokeId": format!("{D1}/{PENCIL}"), "anchor": 1, "inHandle": {"x": -10, "y": 4}, "handleMode": "symmetric" }),
        ),
        ("draw.point.delete", q(json!({ "points": [{"strokeId": BRUSH, "anchor": 1}] }))),
        (
            "draw.segment.move",
            json!({ "strokeId": format!("{D1}/{PENCIL}"), "segment": 0, "delta": {"x": 0, "y": 12} }),
        ),
        ("draw.cut.apply", q(json!({ "rect": {"x": -20, "y": -20, "width": 40, "height": 40} }))),
        (
            "draw.cut.apply",
            q(json!({ "polygon": [{"x": -30, "y": -30}, {"x": 30, "y": -30}, {"x": 0, "y": 30}], "then": "delete" })),
        ),
        ("draw.cut.apply", q(json!({ "segment": {"from": {"x": -100, "y": 10}, "to": {"x": 100, "y": 10}} }))),
        (
            "draw.erase.apply",
            q(json!({ "path": [{"x": -60, "y": 5}, {"x": 60, "y": 5}], "pen": {"minSize": 6, "maxSize": 6} })),
        ),
        ("draw.arrange.apply", q(json!({ "ids": [CAPTION], "to": "back" }))),
        ("draw.arrange.apply", q(json!({ "ids": [PENCIL], "to": "front" }))),
        ("draw.flatten.apply", q(json!({ "ids": [GROUP, PENCIL] }))),
        ("draw.smooth.apply", q(json!({ "ids": [BRUSH], "tolerance": 8 }))),
        ("draw.linesToBrush.apply", q(json!({ "ids": [PENCIL] }))),
        ("draw.centreLine.apply", q(json!({ "ids": [BRUSH] }))),
        ("draw.rectangle.add", q(json!({ "from": {"x": 300, "y": 300}, "to": {"x": 360, "y": 340} }))),
        ("draw.ellipse.add", q(json!({ "from": {"x": 400, "y": 300}, "to": {"x": 460, "y": 340} }))),
        ("draw.line.add", q(json!({ "from": {"x": 300, "y": 400}, "to": {"x": 380, "y": 420} }))),
        (
            "draw.polyline.add",
            q(json!({ "points": [{"x": 300, "y": 500}, {"x": 340, "y": 540}, {"x": 380, "y": 500}] })),
        ),
        ("colour.selection.apply", q(json!({ "colourId": colours[1], "objectIds": [PENCIL, BRUSH] }))),
        (
            "colour.recolour.apply",
            json!({ "fromColourId": colours[0], "toColourId": colours[4], "scope": "drawing", "drawingIds": [D1] }),
        ),
    ]
}

#[test]
fn every_new_mutation_keeps_the_common_contract() {
    let mut no_change = Vec::new();
    for (i, (cmd, params)) in cases().into_iter().enumerate() {
        let h = fixture();
        let before = readback(&h);
        let start = rev(&h);
        let dry = h.invoke(Invocation {
            command: cmd.into(),
            params: params.clone(),
            options: InvokeOptions { dry_run: true, ..Default::default() },
            origin: Origin::User,
        });
        assert!(dry.is_ok(), "{cmd} dry run: {:?}", dry.error);
        assert_eq!((rev(&h), readback(&h)), (start, before.clone()), "{cmd}: a dry run writes nothing");
        let rx = h.subscribe();
        let env = run(&h, cmd, params.clone());
        assert!(env.is_ok(), "{cmd}: {:?}", env.error);
        if rev(&h) == start {
            no_change.push(cmd);
            continue;
        }
        assert_eq!(rev(&h), start + 1, "{cmd}: one revision");
        let events = rx.try_iter().filter(|e| e.name == "draw.drawing.changed" && e.payload["drawingId"] == D1).count();
        assert_eq!(events, 1, "{cmd}: one drawing event");
        let after = readback(&h);
        assert_ne!(after, before, "{cmd} changed the drawing");
        ok(&h, "edit.undo", json!({}));
        assert_eq!(readback(&h), before, "{cmd}: undo restores the read-back");
        ok(&h, "edit.redo", json!({}));
        assert_eq!(readback(&h), after, "{cmd}: redo re-applies it");
        // Saved-project replay.
        let dir = c13_support::tmp();
        let package = dir.join(format!("c23-{i}.animpkg"));
        c13_support::job_ok(&h, "project.saveAs", json!({ "path": package.to_string_lossy() }));
        drop(h);
        let h2 = c13_support::spawn(&dir.join("prefs"));
        c13_support::job_ok(&h2, "project.open", json!({ "path": package.to_string_lossy() }));
        assert_eq!(readback(&h2), after, "{cmd}: the saved project reads back the same");
        drop(h2);
        let _ = std::fs::remove_dir_all(&dir);
        // Locked drawings are refused.
        if cmd != "colour.recolour.apply" {
            let h3 = fixture();
            ok(&h3, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
            let start = rev(&h3);
            let e = run(&h3, cmd, params.clone());
            assert_eq!(e.error.map(|e| e.code), Some("drawing.locked".to_owned()), "{cmd}: locked");
            assert_eq!(rev(&h3), start);
        }
    }
    // Only Smooth may legitimately find nothing to remove on the sample.
    assert!(no_change.iter().all(|c| *c == "draw.smooth.apply"), "{no_change:?}");
}

/// Break Text Apart (02 §8.7): one group per visible character, each a brush shape in the text colour placed where the
/// layout puts the glyph; the text goes; one undo step. Machines without an outline font refuse instead.
#[test]
fn break_text_apart_makes_one_group_per_character() {
    let h = fixture();
    let before_bounds = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": [CAPTION] }))["bounds"].clone();
    let text = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == CAPTION)
        .cloned()
        .unwrap();
    let env = run(&h, "draw.breakApart.apply", json!({ "drawingId": D1, "textIds": [CAPTION] }));
    if let Some(e) = &env.error {
        assert_eq!(e.code, "text.glyphsUnavailable", "{e:?}");
        return;
    }
    let r = env.result.unwrap();
    let chars = text["text"].as_str().unwrap().chars().filter(|c| !c.is_whitespace()).count();
    assert_eq!(r["groupIds"].as_array().unwrap().len(), chars, "{r}");
    let flat = ok(&h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let brushes: Vec<&Value> = flat["nodes"].as_array().unwrap().iter().filter(|n| n["kind"] == "brush").collect();
    let colour = text["runs"][0]["colourId"].clone();
    assert!(brushes.iter().filter(|b| b["colourId"] == colour).count() >= chars);
    let gb = ok(&h, "draw.bounds.get", json!({ "drawingId": D1, "ids": r["groupIds"] }))["bounds"].clone();
    // The glyphs sit inside the text box (the box adds line height above and below the outlines).
    let (bx, gx) = (before_bounds["x"].as_f64().unwrap(), gb["x"].as_f64().unwrap());
    let (bw, gw) = (before_bounds["width"].as_f64().unwrap(), gb["width"].as_f64().unwrap());
    assert!(gx >= bx - 2.0 && gx + gw <= bx + bw + 2.0, "{gb} within {before_bounds}");
    ok(&h, "edit.undo", json!({}));
    assert!(
        ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"] == CAPTION)
    );
}
