//! Gesture equivalence (02 §13.3; ruling D-4; design note 3 §3, invariants G1, G2, G4, G5): for every covered tool the
//! headless `pointer` gesture and the directly sent equivalent command give the same parameters value for value, the
//! same canonical saved bytes (so the same ids) and one undo step with the same label. No tolerance is used: 02 §13.3
//! grants 1e-6 du, and the tests assert exact equality.

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin, Status};

use crate::common::*;

pub const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
pub const BOB: &str = "scene:#1/element:\"Bob\"";

/// The fixture with Bob's frame 1 (drawing `D1`) current.
pub fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

/// The headless Drawing view's default mapping (SI-063, SI-069): view pixel → drawing unit.
pub fn d(vx: f64, vy: f64) -> Value {
    json!({ "x": vx - 960.0, "y": 540.0 - vy })
}

/// One event.
pub fn ev(ty: &str, x: f64, y: f64) -> Value {
    json!({ "type": ty, "x": x, "y": y })
}

/// A press, moves and a release through the given view points.
pub fn drag(points: &[(f64, f64)]) -> Vec<Value> {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            ev(
                if i == 0 {
                    "down"
                } else if i + 1 == n {
                    "up"
                } else {
                    "move"
                },
                *x,
                *y,
            )
        })
        .collect()
}

/// Adds modifiers to every event.
pub fn with_mods(mut events: Vec<Value>, mods: &[&str]) -> Vec<Value> {
    for e in &mut events {
        e["modifiers"] = json!(mods);
    }
    events
}

pub fn pointer(h: &ExecutorHandle, tool: &str, events: Vec<Value>) -> tt_executor::Envelope {
    h.pointer(Origin::User, json!({ "view": "drawing", "tool": tool, "events": events }))
}

/// Every entry of the document as canonical JSON (the saved bytes).
pub fn canonical(h: &ExecutorHandle) -> Vec<(String, String)> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let v = doc.entry_raw_json(&k).unwrap();
            (format!("{k:?}"), tt_model::canonical::to_canonical_string(&v).unwrap())
        })
        .collect()
}

pub fn history(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!({ "label": e["label"], "rev": e["rev"] }))
        .collect()
}

/// The gesture on one executor and the expected command on another, from the same state: equal parameters, equal
/// canonical bytes, equal history with exactly one new step. Returns the gesture's command result.
pub fn equivalent(setup: &dyn Fn(&ExecutorHandle), tool: &str, events: Vec<Value>, id: &str, params: Value) -> Value {
    let a = ready();
    setup(&a);
    let steps = history(&a).len();
    let env = pointer(&a, tool, events);
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    // `tool.active.set` first (the `tool` member), then exactly one command for the gesture (G1).
    assert_eq!(commands.len(), 2, "{commands:#?}");
    assert_eq!(commands[0]["commandId"], "tool.active.set");
    assert_eq!(commands[1]["commandId"], id, "{commands:#?}");
    assert_eq!(commands[1]["params"], params, "parameters value for value (D-4)");
    let b = ready();
    setup(&b);
    ok(&b, "tool.active.set", json!({ "toolId": tool }));
    ok(&b, id, params);
    assert_eq!(canonical(&a), canonical(&b), "the same canonical saved bytes and ids");
    let (ha, hb) = (history(&a), history(&b));
    assert_eq!(ha, hb, "the same undo steps and labels");
    if !id.starts_with("view.") && id != "draw.selection.set" && id != "colour.pick" {
        assert_eq!(ha.len(), steps + 1, "one undo step (G4): {tool} {id}");
    }
    commands[1]["result"].clone()
}

fn none(_: &ExecutorHandle) {}

/// A painted 100 x 60 rectangle at drawing (300, 300)-(400, 360) on a new layer.
pub fn painted_rect(h: &ExecutorHandle) {
    ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
    );
    // Painted with the second swatch, so the current colour (the first) repaints it.
    let doc = h.snapshot().unwrap().unwrap();
    let c: Vec<String> =
        doc.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect();
    ok(h, "colour.paint.apply", json!({ "drawingId": D1, "points": [{"x": 350, "y": 330}], "colourId": c[1] }));
}

fn pen(h: &ExecutorHandle, name: &str) -> String {
    ok(h, "tool.pen.list", json!({}))["pens"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap()["penId"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn colour(h: &ExecutorHandle) -> String {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes().find_map(|p| p.swatches().first().map(|s| s.colour_id().as_str().to_owned())).unwrap()
}

#[test]
fn brush_samples_with_pressure_are_the_strokes_points_in_order() {
    let h = ready();
    let (p, c) = (pen(&h, "Pen 2"), colour(&h));
    // Pen 2 is current only if made so: the default current brush pen is Pen 3, else the first (Pen 1).
    let p1 = pen(&h, "Pen 1");
    let pts = [(1000.0, 500.0), (1010.0, 497.0), (1025.0, 490.0), (1040.0, 470.0)];
    let mut events = drag(&pts);
    for (i, e) in events.iter_mut().enumerate() {
        e["pressure"] = json!(0.25 + 0.2 * i as f64);
    }
    let points: Vec<Value> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let mut q = d(*x, *y);
            q["pressure"] = json!(0.25 + 0.2 * i as f64);
            q
        })
        .collect();
    let r = equivalent(
        &none,
        "brush",
        events,
        "draw.stroke.add",
        json!({ "kind": "brush", "points": points, "fit": true, "penId": p1, "colourId": c, "layer": "auto",
                "symmetry": true }),
    );
    assert!(!r["strokeIds"].as_array().unwrap().is_empty(), "{r}");
    // After selecting Pen 2, the gesture names it.
    let select = |h: &ExecutorHandle| {
        ok(h, "tool.pen.select", json!({ "penId": pen(h, "Pen 2") }));
    };
    equivalent(
        &select,
        "brush",
        drag(&[(900.0, 600.0), (950.0, 620.0)]),
        "draw.stroke.add",
        json!({ "kind": "brush", "points": [
            {"x": -60.0, "y": -60.0, "pressure": 1.0}, {"x": -10.0, "y": -80.0, "pressure": 1.0}],
            "fit": true, "penId": p, "colourId": c, "layer": "auto", "symmetry": true }),
    );
}

#[test]
fn pencil_takes_the_pen_maximum_or_the_fixed_thickness_and_the_layer_setting() {
    let h = ready();
    let (p1, c) = (pen(&h, "Pen 1"), colour(&h));
    let max = ok(&h, "tool.pen.list", json!({}))["pens"][0]["maxSize"].as_f64().unwrap();
    let pts = [(960.0, 540.0), (980.0, 560.0), (1000.0, 600.0)];
    let points: Vec<Value> = pts
        .iter()
        .map(|(x, y)| {
            let mut q = d(*x, *y);
            q["pressure"] = json!(1.0);
            q
        })
        .collect();
    equivalent(
        &none,
        "pencil",
        drag(&pts),
        "draw.stroke.add",
        json!({ "kind": "pencil", "points": points, "fit": true, "penId": p1, "colourId": c, "thickness": max,
                "layer": "auto", "symmetry": true }),
    );
    let fixed = |h: &ExecutorHandle| {
        ok(h, "tool.pencil.options.set", json!({ "thicknessSource": "fixed", "thickness": 3.5 }));
        ok(h, "tool.placeBehind.set", json!({ "enabled": true }));
    };
    equivalent(
        &fixed,
        "pencil",
        drag(&pts),
        "draw.stroke.add",
        json!({ "kind": "pencil", "points": points, "fit": true, "penId": p1, "colourId": c, "thickness": 3.5,
                "layer": "behind", "symmetry": true }),
    );
}

#[test]
fn rectangle_ellipse_and_line_take_press_and_release_with_their_modifiers() {
    let h = ready();
    let (p1, c) = (pen(&h, "Pen 1"), colour(&h));
    let max = ok(&h, "tool.pen.list", json!({}))["pens"][0]["maxSize"].as_f64().unwrap();
    let style = |mut v: Value| {
        v["colourId"] = json!(c);
        v["thickness"] = json!(max);
        v["penId"] = json!(p1);
        v["layer"] = json!("auto");
        v["symmetry"] = json!(true);
        v
    };
    let pts = [(1000.0, 500.0), (1020.0, 480.0), (1100.0, 400.0)];
    equivalent(
        &none,
        "rectangle",
        drag(&pts),
        "draw.rectangle.add",
        style(json!({ "from": d(1000.0, 500.0), "to": d(1100.0, 400.0), "square": false, "fromCentre": false })),
    );
    equivalent(
        &none,
        "ellipse",
        with_mods(drag(&pts), &["shift", "alt"]),
        "draw.ellipse.add",
        style(json!({ "from": d(1000.0, 500.0), "to": d(1100.0, 400.0), "circle": true, "fromCentre": true })),
    );
    equivalent(
        &none,
        "line",
        with_mods(drag(&pts), &["shift"]),
        "draw.line.add",
        style(json!({ "to": d(1100.0, 400.0), "constrainAngle": true, "continuePrevious": false,
                      "from": d(1000.0, 500.0) })),
    );
}

#[test]
fn polyline_clicks_and_click_drags_become_anchors_and_handles() {
    let h = ready();
    let (p1, c) = (pen(&h, "Pen 1"), colour(&h));
    let max = ok(&h, "tool.pen.list", json!({}))["pens"][0]["maxSize"].as_f64().unwrap();
    let mut events = Vec::new();
    events.extend(drag(&[(900.0, 500.0), (900.0, 500.0)]));
    events.extend(drag(&[(1000.0, 500.0), (1010.0, 480.0), (1020.0, 460.0)]));
    events.extend(drag(&[(1100.0, 600.0), (1100.0, 600.0)]));
    events.push(ev("doubleClick", 1100.0, 600.0));
    let mut a1 = d(1000.0, 500.0);
    a1["outHandle"] = json!({ "x": 20.0, "y": 40.0 });
    a1["inHandle"] = json!({ "x": -20.0, "y": -40.0 });
    equivalent(
        &none,
        "polyline",
        events,
        "draw.polyline.add",
        json!({ "points": [d(900.0, 500.0), a1, d(1100.0, 600.0)], "closed": false, "snap": true, "snapRadius": 8.0,
                "colourId": c, "thickness": max, "penId": p1, "layer": "auto", "symmetry": true }),
    );
    // Clicking the first anchor again closes it.
    let mut events = Vec::new();
    for (x, y) in [(900.0, 500.0), (1000.0, 500.0), (1000.0, 600.0), (903.0, 502.0)] {
        events.extend(drag(&[(x, y), (x, y)]));
    }
    equivalent(
        &none,
        "polyline",
        events,
        "draw.polyline.add",
        json!({ "points": [d(900.0, 500.0), d(1000.0, 500.0), d(1000.0, 600.0)], "closed": true, "snap": true,
                "snapRadius": 8.0, "colourId": c, "thickness": max, "penId": p1, "layer": "auto", "symmetry": true }),
    );
}

#[test]
fn eraser_cutter_and_scissor_cut_with_the_mapped_path() {
    let h = ready();
    let e1 = pen(&h, "Eraser 1");
    let pts = [(1250.0, 230.0), (1300.0, 210.0), (1350.0, 200.0)];
    let path: Vec<Value> = pts
        .iter()
        .map(|(x, y)| {
            let mut q = d(*x, *y);
            q["pressure"] = json!(1.0);
            q
        })
        .collect();
    equivalent(&painted_rect, "eraser", drag(&pts), "draw.erase.apply", json!({ "path": path, "penId": e1 }));
    // The pen's eraser end erases whatever tool is active (02 §5.2).
    let a = ready();
    painted_rect(&a);
    let env = a.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": "brush", "pointerType": "penEraser", "events": drag(&pts) }),
    );
    assert_eq!(env.result.unwrap()["commands"][1]["commandId"], "draw.erase.apply");
    let lasso = [(1290.0, 250.0), (1310.0, 250.0), (1310.0, 150.0), (1290.0, 150.0), (1290.0, 250.0)];
    let poly: Vec<Value> = lasso.iter().map(|(x, y)| d(*x, *y)).collect();
    equivalent(&painted_rect, "cutter", drag(&lasso), "draw.cut.apply", json!({ "polygon": poly }));
    equivalent(
        &painted_rect,
        "scissor",
        drag(&[(1290.0, 250.0), (1300.0, 200.0), (1310.0, 150.0)]),
        "draw.cut.apply",
        json!({ "rect": { "x": 330.0, "y": 290.0, "width": 20.0, "height": 100.0 } }),
    );
    equivalent(
        &painted_rect,
        "scissor",
        with_mods(drag(&lasso), &["alt"]),
        "draw.cut.apply",
        json!({ "polygon": poly }),
    );
}

#[test]
fn paint_tools_click_and_drag_with_the_view_and_gap_mode_explicit() {
    let h = ready();
    let c = colour(&h);
    equivalent(
        &painted_rect,
        "unpaint",
        drag(&[(1310.0, 210.0), (1310.0, 210.0)]),
        "colour.paint.remove",
        json!({ "points": [d(1310.0, 210.0)], "space": "drawing", "view": "drawingView", "gapClosing": "off" }),
    );
    equivalent(
        &painted_rect,
        "paint",
        drag(&[(1310.0, 210.0), (1311.0, 211.0)]),
        "colour.paint.apply",
        json!({ "points": [d(1310.0, 210.0)], "space": "drawing", "view": "drawingView", "colourId": c,
                "gapClosing": "off" }),
    );
    let pts = [(1250.0, 210.0), (1310.0, 210.0), (1370.0, 210.0)];
    equivalent(
        &painted_rect,
        "paint",
        drag(&pts),
        "colour.paint.apply",
        json!({ "path": pts.iter().map(|(x, y)| d(*x, *y)).collect::<Vec<_>>(), "space": "drawing",
                "view": "drawingView", "colourId": c, "gapClosing": "off" }),
    );
    let lasso = [(1250.0, 250.0), (1400.0, 250.0), (1400.0, 150.0), (1250.0, 150.0)];
    let unpainted = |h: &ExecutorHandle| {
        ok(
            h,
            "draw.rectangle.add",
            json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
        );
    };
    equivalent(
        &unpainted,
        "paintUnpainted",
        drag(&lasso),
        "colour.paintUnpainted.apply",
        json!({ "lasso": lasso.iter().map(|(x, y)| d(*x, *y)).collect::<Vec<_>>(), "space": "drawing",
                "view": "drawingView", "colourId": c, "gapClosing": "off" }),
    );
    equivalent(
        &painted_rect,
        "dropper",
        drag(&[(1310.0, 210.0), (1310.0, 210.0)]),
        "colour.pick",
        json!({ "point": d(1310.0, 210.0), "space": "drawing", "view": "drawingView" }),
    );
    // The fixture drawing's two stroke ends of the `colour.gap.close` example: drawing (-48, -50) and (2, 2).
    equivalent(
        &none,
        "closeGap",
        drag(&[(912.0, 590.0), (962.0, 538.0)]),
        "colour.gap.close",
        json!({ "from": d(912.0, 590.0), "to": d(962.0, 538.0), "space": "drawing", "view": "drawingView" }),
    );
}

#[test]
fn select_clicks_marquees_lassos_moves_and_the_pivot() {
    equivalent(
        &painted_rect,
        "select",
        drag(&[(1260.0, 240.0), (1260.0, 240.0)]),
        "draw.selection.set",
        json!({ "by": "point", "point": d(1260.0, 240.0), "mode": "replace" }),
    );
    equivalent(
        &painted_rect,
        "select",
        with_mods(drag(&[(1200.0, 300.0), (1400.0, 100.0)]), &["shift"]),
        "draw.selection.set",
        json!({ "by": "rect", "rect": { "x": 240.0, "y": 240.0, "width": 200.0, "height": 200.0 },
                "rectRule": "inside", "mode": "add" }),
    );
    let lasso = [(1200.0, 300.0), (1400.0, 300.0), (1400.0, 100.0)];
    equivalent(
        &painted_rect,
        "select",
        with_mods(drag(&lasso), &["alt"]),
        "draw.selection.set",
        json!({ "by": "lasso", "polygon": lasso.iter().map(|(x, y)| d(*x, *y)).collect::<Vec<_>>(),
                "rectRule": "inside", "mode": "replace" }),
    );
    let selected = |h: &ExecutorHandle| {
        painted_rect(h);
        ok(h, "draw.selection.set", json!({ "by": "all" }));
    };
    equivalent(
        &selected,
        "select",
        with_mods(drag(&[(1310.0, 210.0), (1330.0, 200.0)]), &["alt"]),
        "draw.transform.apply",
        json!({ "translate": { "x": 20.0, "y": 10.0 }, "copy": true }),
    );
    // A drag starting on the drawing pivot's handle moves the pivot.
    let piv = ok(&ready(), "draw.drawing.get", json!({ "drawingId": D1 }))["pivot"].clone();
    let (px, py) = (piv["x"].as_f64().unwrap() + 960.0, 540.0 - piv["y"].as_f64().unwrap());
    equivalent(
        &none,
        "select",
        drag(&[(px + 1.0, py + 1.0), (1000.0, 500.0)]),
        "draw.pivot.set",
        json!({ "point": d(1000.0, 500.0) }),
    );
}
