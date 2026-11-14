//! Gesture equivalence widened after round 1 (G2, G3; 02 §5.2, §13.3): the remaining tools, every explicit setting
//! varied and visible in the produced command, a rotated, zoomed and panned view per tool family, a view change between
//! two gestures, the temporary overrides, and zero-size and outside-view shapes.

use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

use crate::common::*;
use crate::equivalence::{D1, canonical, d, drag, equivalent, history, painted_rect, pointer, ready, with_mods};

fn none(_: &ExecutorHandle) {}

/// A rotated, zoomed and panned Drawing view.
fn turned(h: &ExecutorHandle) {
    ok(h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2 }));
    ok(h, "view.rotation.set", json!({ "viewId": "drawingView", "degrees": 30 }));
    ok(h, "view.pan.set", json!({ "viewId": "drawingView", "centre": { "x": 330, "y": 320 } }));
}

/// A view point in drawing space through the view's own conversion (an independent route to the mapping).
fn conv(h: &ExecutorHandle, x: f64, y: f64) -> Value {
    ok(
        h,
        "view.coordinates.convert",
        json!({ "viewId": "drawingView", "point": { "x": x, "y": y }, "from": "view", "to": "drawing" }),
    )["point"]
        .clone()
}

fn with_pressure(p: Value) -> Value {
    let mut p = p;
    p["pressure"] = json!(1.0);
    p
}

fn pen(h: &ExecutorHandle, name: &str) -> String {
    ok(h, "tool.pen.list", json!({}))["pens"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap()["penId"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn colours(h: &ExecutorHandle) -> Vec<(String, String)> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes()
        .flat_map(|p| p.swatches().iter().map(|s| (s.colour_id().as_str().to_owned(), s.name().clone())))
        .collect()
}

#[test]
fn stroke_zoom_and_grabber() {
    let h = ready();
    let (p1, c) = (pen(&h, "Pen 1"), colours(&h)[0].0.clone());
    let pts = [(1000.0, 500.0), (1020.0, 480.0)];
    equivalent(
        &none,
        "stroke",
        drag(&pts),
        "draw.stroke.add",
        json!({ "kind": "invisible", "points": pts.iter().map(|(x, y)| with_pressure(d(*x, *y))).collect::<Vec<_>>(),
                "fit": true, "penId": p1, "colourId": c, "layer": "auto", "symmetry": true }),
    );
    equivalent(
        &none,
        "zoom",
        with_mods(drag(&[(1200.0, 300.0), (1200.0, 300.0)]), &["alt"]),
        "view.zoom.set",
        json!({ "viewId": "drawingView", "steps": -1, "anchor": { "x": 1200.0, "y": 300.0 } }),
    );
    equivalent(
        &none,
        "grabber",
        drag(&[(900.0, 500.0), (950.0, 450.0), (1000.0, 420.0)]),
        "view.pan.set",
        json!({ "viewId": "drawingView", "by": { "x": 100.0, "y": -80.0 } }),
    );
    // A marquee fits its rectangle: zoom, then the rectangle's centre at the view's centre (02 §7.5 line 658).
    let a = ready();
    let env = pointer(&a, "zoom", drag(&[(1000.0, 500.0), (1200.0, 380.0)]));
    let cmds = env.result.unwrap()["commands"].clone();
    assert_eq!(cmds[1]["params"], json!({ "viewId": "drawingView", "zoom": 9.0 }));
    assert_eq!(cmds[2]["params"], json!({ "viewId": "drawingView", "centre": { "x": 140.0, "y": 100.0 } }));
    let b = ready();
    ok(&b, "tool.active.set", json!({ "toolId": "zoom" }));
    ok(&b, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 9.0 }));
    ok(&b, "view.pan.set", json!({ "viewId": "drawingView", "centre": { "x": 140.0, "y": 100.0 } }));
    let t = |h: &ExecutorHandle| ok(h, "view.transform.get", json!({ "viewId": "drawingView" }));
    assert_eq!(t(&a), t(&b));
}

#[test]
fn a_gradient_paint_drag_defines_the_mapping_and_a_click_fits_it() {
    let sky = |h: &ExecutorHandle| colours(h).into_iter().find(|(_, n)| n == "Sky").unwrap().0;
    // The fixture's rectangle colour is its second swatch, which is Sky: repaint it solid first so Sky changes it.
    let setup = |h: &ExecutorHandle| {
        painted_rect(h);
        ok(
            h,
            "colour.paint.apply",
            json!({ "drawingId": D1, "points": [{ "x": 350, "y": 330 }], "colourId": colours(h)[0].0 }),
        );
        ok(h, "colour.current.set", json!({ "colourId": sky(h) }));
    };
    let h = ready();
    let c = sky(&h);
    equivalent(
        &setup,
        "paint",
        drag(&[(1270.0, 200.0), (1300.0, 210.0), (1350.0, 230.0)]),
        "colour.paint.apply",
        json!({ "points": [d(1270.0, 200.0)], "mapping": { "from": d(1270.0, 200.0), "to": d(1350.0, 230.0) },
                "space": "drawing", "view": "drawingView", "colourId": c, "gapClosing": "off" }),
    );
    equivalent(
        &setup,
        "paint",
        drag(&[(1310.0, 210.0), (1310.0, 210.0)]),
        "colour.paint.apply",
        json!({ "points": [d(1310.0, 210.0)], "space": "drawing", "view": "drawingView", "colourId": c, "gapClosing": "off" }),
    );
}

#[test]
fn a_constrained_polyline_handle_turns_in_15_degree_steps() {
    let h = ready();
    let (p1, c) = (pen(&h, "Pen 1"), colours(&h)[0].0.clone());
    let max = ok(&h, "tool.pen.list", json!({}))["pens"][0]["maxSize"].as_f64().unwrap();
    let mut events = Vec::new();
    events.extend(drag(&[(900.0, 500.0), (900.0, 500.0)]));
    // A handle pulled 40 px right and 13 px up (18°) snaps to 15°, keeping its length.
    events.extend(with_mods(drag(&[(1000.0, 500.0), (1040.0, 487.0)]), &["shift"]));
    events.extend(drag(&[(1100.0, 600.0), (1100.0, 600.0)]));
    events.push(json!({ "type": "doubleClick", "x": 1100.0, "y": 600.0 }));
    let env = pointer(&h, "polyline", events.clone());
    let params = env.result.unwrap()["commands"][1]["params"].clone();
    let out = &params["points"][1]["outHandle"];
    let (x, y) = (out["x"].as_f64().unwrap(), out["y"].as_f64().unwrap());
    assert!((y.atan2(x).to_degrees() - 15.0).abs() < 1e-9, "{out}");
    assert!((x.hypot(y) - 40f64.hypot(13.0)).abs() < 1e-9);
    let mut a1 = d(1000.0, 500.0);
    a1["outHandle"] = out.clone();
    a1["inHandle"] = json!({ "x": -x, "y": -y });
    equivalent(
        &none,
        "polyline",
        events,
        "draw.polyline.add",
        json!({ "points": [d(900.0, 500.0), a1, d(1100.0, 600.0)], "closed": false, "snap": true, "snapRadius": 8.0,
                "colourId": c, "thickness": max, "penId": p1, "layer": "auto", "symmetry": true }),
    );
}

#[test]
fn every_explicit_setting_is_varied_and_visible_in_the_command() {
    let h = ready();
    let cs = colours(&h);
    let p2 = pen(&h, "Pen 2");
    let second = cs[1].0.clone();
    let setup = |h: &ExecutorHandle| {
        ok(h, "tool.pen.select", json!({ "penId": pen(h, "Pen 2") }));
        ok(h, "colour.current.set", json!({ "colourId": colours(h)[1].0 }));
        ok(h, "tool.drawTopLayer.set", json!({ "enabled": true }));
        ok(h, "tool.placeBehind.set", json!({ "enabled": true }));
        ok(h, "tool.pencil.options.set", json!({ "thicknessSource": "fixed", "thickness": 1.25 }));
        ok(h, "tool.symmetry.set", json!({ "mode": "mirror" }));
        ok(h, "colour.gapClosing.set", json!({ "mode": "small" }));
    };
    let pts = [(1000.0, 500.0), (1100.0, 400.0)];
    let r = equivalent(
        &setup,
        "rectangle",
        drag(&pts),
        "draw.rectangle.add",
        json!({ "from": d(1000.0, 500.0), "to": d(1100.0, 400.0), "square": false, "fromCentre": false,
                "colourId": second, "thickness": 1.25, "penId": p2, "layer": "top", "symmetry": true }),
    );
    assert!(r["strokeIds"].as_array().unwrap().len() >= 2, "mirror symmetry draws a mirrored copy: {r}");
    equivalent(
        &|h: &ExecutorHandle| {
            setup(h);
            painted_rect(h);
        },
        "unpaint",
        drag(&[(1310.0, 210.0), (1310.0, 210.0)]),
        "colour.paint.remove",
        json!({ "points": [d(1310.0, 210.0)], "space": "drawing", "view": "drawingView", "gapClosing": "small" }),
    );
}

#[test]
fn in_a_rotated_zoomed_panned_view_each_tool_family_maps_through_the_view() {
    let h = ready();
    turned(&h);
    let (p1, c) = (pen(&h, "Pen 1"), colours(&h)[0].0.clone());
    let e1 = pen(&h, "Eraser 1");
    let pts = [(900.0, 500.0), (960.0, 540.0), (1020.0, 560.0)];
    let mapped: Vec<Value> = pts.iter().map(|(x, y)| conv(&h, *x, *y)).collect();
    // Freehand.
    equivalent(
        &turned,
        "brush",
        drag(&pts),
        "draw.stroke.add",
        json!({ "kind": "brush", "points": mapped.iter().cloned().map(with_pressure).collect::<Vec<_>>(), "fit": true,
                "penId": p1, "colourId": c, "layer": "auto", "symmetry": true }),
    );
    // Shapes.
    let max = ok(&h, "tool.pen.list", json!({}))["pens"][0]["maxSize"].as_f64().unwrap();
    equivalent(
        &turned,
        "ellipse",
        drag(&pts),
        "draw.ellipse.add",
        json!({ "from": mapped[0], "to": mapped[2], "circle": false, "fromCentre": false, "colourId": c,
                "thickness": max, "penId": p1, "layer": "auto", "symmetry": true }),
    );
    // Erasing and cutting.
    let setup = |h: &ExecutorHandle| {
        turned(h);
        painted_rect(h);
    };
    equivalent(
        &setup,
        "eraser",
        drag(&pts),
        "draw.erase.apply",
        json!({ "path": mapped.iter().cloned().map(with_pressure).collect::<Vec<_>>(), "penId": e1 }),
    );
    equivalent(&setup, "cutter", drag(&pts), "draw.cut.apply", json!({ "polygon": mapped }));
    // Painting: the painted rectangle's centre (350, 330) is the view's centre here.
    equivalent(
        &setup,
        "paint",
        drag(&[(960.0, 540.0), (960.0, 540.0)]),
        "colour.paint.apply",
        json!({ "points": [conv(&h, 960.0, 540.0)], "space": "drawing", "view": "drawingView", "colourId": c,
                "gapClosing": "off" }),
    );
    // Selecting.
    equivalent(
        &setup,
        "select",
        drag(&[(960.0, 540.0), (960.0, 540.0)]),
        "draw.selection.set",
        json!({ "by": "point", "point": conv(&h, 960.0, 540.0), "mode": "replace" }),
    );
}

#[test]
fn g3_a_view_change_between_two_gestures_maps_the_second_with_the_new_view() {
    let h = ready();
    let mut wheel = json!({ "type": "wheel", "x": 960, "y": 540, "wheelDelta": -120 });
    wheel["modifiers"] = json!([]);
    let mut events = drag(&[(1000.0, 500.0), (1100.0, 400.0)]);
    events.push(wheel);
    events.extend(drag(&[(1000.0, 500.0), (1100.0, 400.0)]));
    let env = pointer(&h, "line", events);
    let c = env.result.unwrap()["commands"].clone();
    assert_eq!(c[2]["commandId"], "view.zoom.set");
    assert_eq!(c[1]["params"]["to"], d(1100.0, 400.0), "the first gesture: the view at its press (zoom 1)");
    assert_eq!(c[3]["params"]["to"], json!({ "x": 70.0, "y": 70.0 }), "the second: zoom 2 at its press");
}

#[test]
fn temporary_overrides_of_5_2() {
    let h = ready();
    painted_rect(&h);
    // The temporary-select modifier on a drawing tool: Select.
    let env = pointer(&h, "brush", with_mods(drag(&[(1310.0, 210.0), (1310.0, 210.0)]), &["ctrl"]));
    assert_eq!(env.result.unwrap()["commands"][1]["commandId"], "draw.selection.set");
    // The alternate modifier on Paint: the Dropper (pick, then select the colour).
    let env = pointer(&h, "paint", with_mods(drag(&[(1310.0, 210.0), (1310.0, 210.0)]), &["alt"]));
    let ids: Vec<Value> =
        env.result.unwrap()["commands"].as_array().unwrap().iter().map(|c| c["commandId"].clone()).collect();
    assert_eq!(ids, vec![json!("tool.active.set"), json!("colour.pick"), json!("colour.current.set")]);
    // On a shape tool the alternate modifier is the tool's own (from the centre), not the Dropper.
    let env = pointer(&h, "rectangle", with_mods(drag(&[(1000.0, 500.0), (1100.0, 400.0)]), &["alt"]));
    let p = env.result.unwrap()["commands"][1]["params"].clone();
    assert_eq!(p["fromCentre"], true, "{p}");
    // The tool stays the user's after the gesture (overrides are per gesture).
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "rectangle");
}

#[test]
fn zero_size_and_outside_view_shapes_answer_as_their_direct_command() {
    for (tool, id) in [("rectangle", "draw.rectangle.add"), ("ellipse", "draw.ellipse.add"), ("line", "draw.line.add")]
    {
        for pts in [[(1000.0, 500.0), (1000.0, 500.0)], [(-400.0, -300.0), (2400.0, 1500.0)]] {
            let a = ready();
            let before = history(&a).len();
            let env = pointer(&a, tool, drag(&pts));
            let gesture = if env.is_ok() {
                env.result.clone().unwrap()["commands"][1].clone()
            } else {
                json!({ "error": env.error.clone().unwrap().code })
            };
            let b = ready();
            ok(&b, "tool.active.set", json!({ "toolId": tool }));
            let params = if env.is_ok() { gesture["params"].clone() } else { Value::Null };
            if env.is_ok() {
                assert_eq!(gesture["commandId"], id);
                let direct = run(&b, id, params);
                assert_eq!(direct.is_ok(), gesture["envelope"]["status"] == "ok", "{tool} {pts:?}");
            } else {
                let e = env.error.unwrap();
                assert_eq!(e.details["gestureCommandId"], id, "{tool} {pts:?}: {e:?}");
                assert_eq!(history(&a).len(), before, "a refused gesture adds no step");
            }
            assert_eq!(canonical(&a), canonical(&b), "{tool} {pts:?}");
        }
    }
}
