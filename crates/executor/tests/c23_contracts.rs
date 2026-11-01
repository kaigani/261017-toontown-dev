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
    // The read-back omits stored mappings of lines: add every stroke's fill matrix (SI-068).
    let doc = h.snapshot().unwrap().unwrap();
    let d = doc.drawing(&tt_model::Id::parse(D1).unwrap()).unwrap();
    let matrices: Vec<Value> = tt_vector::drawing::objects(d)
        .iter()
        .filter_map(|o| tt_vector::drawing::stroke_at(d, &o.pointer))
        .map(|s| json!([s.id().as_str(), s.fill_matrix()]))
        .collect();
    v["fillMatrices"] = json!(matrices);
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
        (
            "colour.fillMapping.set",
            q(json!({ "objectIds": [PENCIL], "mapping": {"matrix": [40, 0, 0, 20, -40, -10]} })),
        ),
        ("draw.breakApart.apply", q(json!({ "textIds": [CAPTION] }))),
        ("draw.drawing.rename", q(json!({ "name": "renamed" }))),
        ("draw.lock.set", q(json!({ "locked": true }))),
        ("draw.optimize.apply", q(json!({}))),
        (
            "draw.repositionAll.apply",
            json!({ "elementId": "scene:#1/element:\"Bob\"", "translate": {"x": 0, "y": 20} }),
        ),
        // The SI-061 override, explicit (the protection itself: c23_colour::protected_colours_are_skipped_unless_overridden).
        (
            "colour.selection.apply",
            q(json!({ "colourId": colours[4], "objectIds": [BRUSH], "respectProtected": false })),
        ),
        (
            "colour.recolour.apply",
            json!({ "fromColourId": colours[4], "toColourId": colours[0], "scope": "drawing", "drawingIds": [D1], "respectProtected": false }),
        ),
    ]
}

/// Commands a case needs set up first (static IDs only).
fn prelude(cmd: &str, colours: &[String]) -> Vec<(&'static str, Value)> {
    match cmd {
        // A gradient on the pencil line gives it a mapping to set.
        "colour.fillMapping.set" => {
            vec![("colour.paint.apply", json!({ "drawingId": D1, "objectIds": [PENCIL], "colourId": colours[1] }))]
        }
        // Two disjoint layers beside the sample's, all selected: Optimize merges them.
        "draw.optimize.apply" => vec![
            (
                "draw.rectangle.add",
                json!({ "drawingId": D1, "from": {"x": 1000, "y": 1000}, "to": {"x": 1050, "y": 1050}, "layer": "new" }),
            ),
            (
                "draw.rectangle.add",
                json!({ "drawingId": D1, "from": {"x": 1200, "y": 1000}, "to": {"x": 1250, "y": 1050}, "layer": "new" }),
            ),
            ("draw.selection.set", json!({ "drawingId": D1, "by": "all" })),
        ],
        _ => Vec::new(),
    }
}

/// The C2.1/C2.2 mutations; every other implemented drawing or colour mutation must have a case (C23-R1-14).
const BEFORE_C23: &[&str] = &[
    "colour.gap.close",
    "colour.paint.apply",
    "colour.paint.remove",
    "colour.paintUnpainted.apply",
    "colour.recovery.run",
    "draw.content.add",
    "draw.group.create",
    "draw.group.delete",
    "draw.object.delete",
    "draw.pivot.set",
    "draw.stroke.add",
    "draw.stroke.set",
    "draw.text.add",
    "draw.text.set",
    "draw.textFormat.set",
];

/// Every object ID of the drawing: nodes, strokes and zones.
fn object_ids(h: &ExecutorHandle) -> std::collections::BTreeSet<String> {
    let flat = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let mut ids: std::collections::BTreeSet<String> =
        flat["nodes"].as_array().unwrap().iter().filter_map(|n| n["id"].as_str().map(str::to_owned)).collect();
    fn zones(nodes: &Value, out: &mut std::collections::BTreeSet<String>) {
        for n in nodes.as_array().into_iter().flatten() {
            for z in n["zones"].as_array().into_iter().flatten() {
                out.extend(z["id"].as_str().map(str::to_owned));
            }
            zones(&n["children"], out);
        }
    }
    zones(&ok(h, "draw.drawing.get", json!({ "drawingId": D1 }))["nodes"], &mut ids);
    ids
}

#[test]
fn the_contract_covers_every_new_mutation() {
    let covered: Vec<&str> = cases().iter().map(|c| c.0).collect();
    let registry = tt_executor::registry().unwrap();
    let missing: Vec<&str> = registry
        .entries()
        .filter(|e| e.id().starts_with("draw.") || e.id().starts_with("colour."))
        .filter(|e| e.status() == tt_registry::Status::Implemented)
        .filter(|e| matches!(e.spec.kind, tt_registry::Kind::Action | tt_registry::Kind::Toggle))
        .map(|e| e.id())
        .filter(|id| !BEFORE_C23.contains(id) && !covered.contains(id))
        .collect();
    assert!(missing.is_empty(), "mutations without a contract case: {missing:?}");
}

#[test]
fn every_new_mutation_keeps_the_common_contract() {
    let colours: Vec<String> = {
        let h = fixture();
        let v = h
            .snapshot()
            .unwrap()
            .unwrap()
            .palettes()
            .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
            .collect();
        v
    };
    let mut no_change = Vec::new();
    for (i, (cmd, params)) in cases().into_iter().enumerate() {
        let h = fixture();
        for (pc, pp) in prelude(cmd, &colours) {
            ok(&h, pc, pp);
        }
        let before = readback(&h);
        let before_ids = object_ids(&h);
        let start = rev(&h);
        let dry = h.invoke(Invocation {
            command: cmd.into(),
            params: params.clone(),
            options: InvokeOptions { dry_run: true, ..Default::default() },
            origin: Origin::User,
        });
        if cmd == "draw.breakApart.apply" && dry.error.as_ref().is_some_and(|e| e.code == "text.glyphsUnavailable") {
            continue;
        }
        assert!(dry.is_ok(), "{cmd} dry run: {:?}", dry.error);
        assert_eq!((rev(&h), readback(&h)), (start, before.clone()), "{cmd}: a dry run writes nothing");
        // A stale expected revision refuses and writes nothing.
        let mut stale = InvokeOptions::default();
        stale.expect_rev.insert(D1.to_owned(), 0);
        let refused = with(&h, cmd, params.clone(), stale);
        assert_eq!(refused.error.map(|e| e.code), Some("rev.conflict".to_owned()), "{cmd}: stale expectRev");
        assert_eq!((rev(&h), readback(&h)), (start, before.clone()), "{cmd}: the refusal writes nothing");
        // A batch whose second step fails rolls the first back.
        let failed = batch(
            &h,
            json!({ "label": "contract rollback", "steps": [{"id": cmd, "params": params.clone()},
                {"id": "draw.transform.apply", "params": {"drawingId": D1, "ids": ["st_missing"], "translate": {"x": 1, "y": 0}}}] }),
        );
        assert!(!failed.is_ok(), "{cmd}: the batch must fail");
        assert_eq!((rev(&h), readback(&h)), (start, before.clone()), "{cmd}: the failed batch rolls back");
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
        // Exact created and removed IDs: what appeared and what went.
        let r = env.result.clone().unwrap_or(Value::Null);
        let after_ids = object_ids(&h);
        let set = |k: &str| -> Option<std::collections::BTreeSet<String>> {
            r.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        };
        if let (Some(created), Some(removed)) = (set("createdIds"), set("removedIds")) {
            let appeared: std::collections::BTreeSet<String> = after_ids.difference(&before_ids).cloned().collect();
            let went: std::collections::BTreeSet<String> = before_ids.difference(&after_ids).cloned().collect();
            assert_eq!(created, appeared, "{cmd}: createdIds are exactly the new objects");
            assert_eq!(removed, went, "{cmd}: removedIds are exactly the objects that went");
        }
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
        // Locked drawings are refused (lock and rename act on locked drawings, SI-069; Replace Colour and
        // Reposition All skip and report them).
        if !["colour.recolour.apply", "draw.lock.set", "draw.drawing.rename", "draw.repositionAll.apply"].contains(&cmd)
        {
            let h3 = fixture();
            for (pc, pp) in prelude(cmd, &colours) {
                ok(&h3, pc, pp);
            }
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
