//! M4 C4.4b, the Perspective and Reposition All Drawings tools' gestures (design note 4 §2-§4; SI-188, SI-189): every
//! row through the headless `pointer` gives the command an agent sends for the same edit, value for value, with the
//! same canonical bytes and undo steps as that command sent directly (02 §13.3 lines 1227-1228; labels 02 line 473
//! "Perspective Tool" and line 481 "Reposition All Drawings"; ruling D-4, D-32). The handles sit on the box the scene
//! answer publishes (`toolBox`), the one the command maps.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin, Status};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

/// The headless Drawing view's default mapping: drawing unit -> view pixel.
fn v(x: f64, y: f64) -> (f64, f64) {
    (x + 960.0, 540.0 - y)
}

fn drag(points: &[(f64, f64)], mods: &[&str]) -> Vec<Value> {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let ty = if i == 0 {
                "down"
            } else if i + 1 == n {
                "up"
            } else {
                "move"
            };
            json!({ "type": ty, "x": x, "y": y, "modifiers": mods })
        })
        .collect()
}

fn canonical(h: &ExecutorHandle) -> Vec<(String, String)> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let v = doc.entry_raw_json(&k).unwrap();
            (format!("{k:?}"), tt_model::canonical::to_canonical_string(&v).unwrap())
        })
        .collect()
}

fn history(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!({ "label": e["label"], "rev": e["rev"] }))
        .collect()
}

/// A 100 x 60 rectangle at drawing (300, 300)-(400, 360) on a new layer, selected.
fn rect_selected(h: &ExecutorHandle) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}, "layer": "new" }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    ok(h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
}

fn tool_box(h: &ExecutorHandle, view: &str) -> Value {
    serde_json::to_value(h.canvas_scene(view).unwrap()).unwrap()["toolBox"].clone()
}

/// `[minX, minY, maxX, maxY]` of the active tool's box.
fn bbox(h: &ExecutorHandle) -> [f64; 4] {
    let b = tool_box(h, "drawingView");
    let a = b["box"].as_array().unwrap_or_else(|| panic!("no tool box: {b}"));
    std::array::from_fn(|i| a[i].as_f64().unwrap())
}

/// The gesture on one executor and the expected command sent directly on another, from the same start.
fn equivalent(
    setup: &dyn Fn(&ExecutorHandle),
    tool: &str,
    events: Vec<Value>,
    expected: Option<(&str, Value)>,
) -> Value {
    let a = ready();
    setup(&a);
    ok(&a, "tool.active.set", json!({ "toolId": tool }));
    let steps = history(&a).len();
    let env = a.pointer(Origin::User, json!({ "view": "drawing", "tool": tool, "events": events }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    let got: Vec<(String, Value)> = commands
        .iter()
        .filter(|c| c["commandId"] != "tool.active.set")
        .map(|c| (c["commandId"].as_str().unwrap().to_owned(), c["params"].clone()))
        .collect();
    let want: Vec<(String, Value)> = expected.iter().map(|(i, p)| ((*i).to_owned(), p.clone())).collect();
    assert_eq!(got, want, "the agent's command, value for value (D-4)");
    let b = ready();
    setup(&b);
    ok(&b, "tool.active.set", json!({ "toolId": tool }));
    if let Some((id, p)) = &expected {
        ok(&b, id, p.clone());
    }
    assert_eq!(canonical(&a), canonical(&b), "the same canonical saved bytes and ids");
    assert_eq!(history(&a), history(&b), "the same undo steps and labels");
    let h = history(&a);
    match &expected {
        Some((id, _)) if id.starts_with("draw.envelope") => {
            assert_eq!(h.len(), steps + 1);
            assert_eq!(h.last().unwrap()["label"], "Perspective Tool", "02 line 473");
        }
        Some((id, _)) if id.starts_with("draw.repositionAll") => {
            assert_eq!(h.len(), steps + 1);
            assert_eq!(h.last().unwrap()["label"], "Reposition All Drawings", "02 line 481");
        }
        _ => assert_eq!(h.len(), steps, "selection rows and clicks make no step"),
    }
    commands.last().map(|c| c["envelope"].clone()).unwrap_or(Value::Null)
}

fn corners(b: [f64; 4]) -> [(f64, f64); 4] {
    [(b[0], b[1]), (b[2], b[1]), (b[2], b[3]), (b[0], b[3])]
}

fn cj(c: &[(f64, f64)]) -> Value {
    json!(c.iter().map(|p| json!({ "x": p.0, "y": p.1 })).collect::<Vec<_>>())
}

// ----- Perspective (02 §7.1.3; SI-188) ---------------------------------------------------------------------------------

#[test]
fn perspective_corner_drag_is_an_envelope_with_the_mapping_explicit() {
    let h = ready();
    rect_selected(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
    let b = bbox(&h);
    let mut c = corners(b);
    let p = v(c[2].0, c[2].1);
    c[2] = (c[2].0 - 24.0, c[2].1 + 0.0);
    equivalent(
        &rect_selected,
        "perspective",
        drag(&[p, (p.0 - 12.0, p.1), (p.0 - 24.0, p.1)], &[]),
        Some(("draw.envelope.apply", json!({ "corners": cj(&c), "mapping": "auto" }))),
    );
    // The option is sent as set.
    let setup = |h: &ExecutorHandle| {
        rect_selected(h);
        ok(h, "tool.options.set", json!({ "toolId": "perspective", "options": { "mapping": "bilinear" } }));
    };
    let mut c = corners(b);
    c[0] = (c[0].0 + 5.0, c[0].1 + 5.0);
    let p = v(b[0], b[1]);
    equivalent(
        &setup,
        "perspective",
        drag(&[p, (p.0 + 5.0, p.1 - 5.0)], &[]),
        Some(("draw.envelope.apply", json!({ "corners": cj(&c), "mapping": "bilinear" }))),
    );
}

#[test]
fn perspective_side_drags_plain_and_symmetric() {
    let h = ready();
    rect_selected(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
    let b = bbox(&h);
    let top = v((b[0] + b[2]) / 2.0, b[3]);
    let mut c = corners(b);
    c[2].1 += 10.0;
    c[3].1 += 10.0;
    equivalent(
        &rect_selected,
        "perspective",
        drag(&[top, (top.0, top.1 - 10.0)], &[]),
        Some(("draw.envelope.apply", json!({ "corners": cj(&c), "mapping": "auto" }))),
    );
    // Alternate: a symmetric keystone, the top-right corner following the drag along the side.
    let mut c = corners(b);
    c[2] = (c[2].0 + 15.0, c[2].1 + 4.0);
    c[3] = (c[3].0 - 15.0, c[3].1 + 4.0);
    equivalent(
        &rect_selected,
        "perspective",
        drag(&[top, (top.0 + 15.0, top.1 - 4.0)], &["alt"]),
        Some(("draw.envelope.apply", json!({ "corners": cj(&c), "mapping": "auto" }))),
    );
}

#[test]
fn perspective_off_the_handles_selects_as_select_does() {
    let far = v(-500.0, -400.0);
    let p = json!({ "x": -500.0, "y": -400.0 });
    equivalent(
        &rect_selected,
        "perspective",
        drag(&[far, far], &[]),
        Some(("draw.selection.set", json!({ "by": "point", "point": p, "mode": "replace" }))),
    );
    let (a, z) = (v(-500.0, -400.0), v(-450.0, -350.0));
    equivalent(
        &rect_selected,
        "perspective",
        drag(&[a, z], &["shift"]),
        Some((
            "draw.selection.set",
            json!({ "by": "rect", "rect": {"x": -500.0, "y": -400.0, "width": 50.0, "height": 50.0},
                    "rectRule": "inside", "mode": "add" }),
        )),
    );
}

#[test]
fn perspective_on_a_locked_drawing_is_refused_with_nothing_changed() {
    let h = ready();
    rect_selected(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
    let b = bbox(&h);
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [D1], "locked": true }));
    assert_eq!(tool_box(&h, "drawingView")["locked"], true, "grey handles");
    let (bytes, hist) = (canonical(&h), history(&h));
    let p = v(b[2], b[3]);
    let env = h.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": "perspective", "events": drag(&[p, (p.0 + 9.0, p.1)], &[]) }),
    );
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("drawing.locked"), "{env:?}");
    assert_eq!((canonical(&h), history(&h)), (bytes, hist));
}

#[test]
fn handles_are_six_view_pixels_at_any_zoom() {
    // At 200 % a press 3.5 du (7 px) beyond the corner misses the handle: a marquee.
    let h = ready();
    rect_selected(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2.0 }));
    let b = bbox(&h);
    let conv = |x: f64, y: f64| {
        let r = ok(
            &h,
            "view.coordinates.convert",
            json!({ "viewId": "drawingView", "point": {"x": x, "y": y}, "from": "drawing", "to": "view" }),
        );
        (r["point"]["x"].as_f64().unwrap(), r["point"]["y"].as_f64().unwrap())
    };
    let run = |p: (f64, f64)| {
        let env = h.pointer(
            Origin::User,
            json!({ "view": "drawing", "tool": "perspective", "events": drag(&[p, (p.0 + 20.0, p.1 + 20.0)], &[]) }),
        );
        let id = env.result.as_ref().unwrap()["commands"].as_array().unwrap().last().unwrap()["commandId"].clone();
        if id == "draw.envelope.apply" {
            ok(&h, "edit.undo", json!({}));
        }
        id
    };
    let c = conv(b[2], b[3]);
    assert_eq!(run((c.0 + 6.0, c.1)), "draw.envelope.apply");
    assert_eq!(run((c.0 + 6.01, c.1)), "draw.selection.set");
}

// ----- Reposition All Drawings (02 §7.1.4; SI-189) --------------------------------------------------------------------

fn reposition_setup(h: &ExecutorHandle) {
    rect_selected(h);
}

#[test]
fn reposition_offset_scale_rotate_and_click() {
    let h = ready();
    reposition_setup(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "repositionAll" }));
    let b = bbox(&h);
    // A click: nothing.
    let inside = v((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    equivalent(&reposition_setup, "repositionAll", drag(&[inside, inside], &[]), None);
    // Any other drag offsets every drawing (alternate ignored).
    for mods in [&[][..], &["alt"][..]] {
        equivalent(
            &reposition_setup,
            "repositionAll",
            drag(&[inside, (inside.0 + 30.0, inside.1 - 10.0)], mods),
            Some(("draw.repositionAll.apply", json!({ "translate": {"x": 30.0, "y": 10.0} }))),
        );
    }
    // The top-right corner scales about the bottom-left corner.
    let c = corners(b);
    let p = v(c[2].0, c[2].1);
    let (sx, sy) = ((c[2].0 + 20.0 - c[0].0) / (c[2].0 - c[0].0), (c[2].1 + 10.0 - c[0].1) / (c[2].1 - c[0].1));
    equivalent(
        &reposition_setup,
        "repositionAll",
        drag(&[p, (p.0 + 20.0, p.1 - 10.0)], &[]),
        Some(("draw.repositionAll.apply", json!({ "scale": {"x": sx, "y": sy}, "pivot": {"x": c[0].0, "y": c[0].1} }))),
    );
    // The rotation zone, just outside the top-right corner: degrees counter-clockwise about the box's centre.
    let o = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let press = (c[2].0 + 8.0, c[2].1 + 8.0);
    let release = (c[2].0 - 8.0, c[2].1 + 20.0);
    let deg = ((release.1 - o.1).atan2(release.0 - o.0) - (press.1 - o.1).atan2(press.0 - o.0)).to_degrees();
    let env = equivalent(
        &reposition_setup,
        "repositionAll",
        drag(&[v(press.0, press.1), v(release.0, release.1)], &[]),
        Some(("draw.repositionAll.apply", json!({ "rotate": deg, "pivot": {"x": o.0, "y": o.1} }))),
    );
    assert_eq!(env["status"], "ok");
}

#[test]
fn reposition_alternate_handles_are_envelope_corners() {
    let h = ready();
    reposition_setup(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "repositionAll" }));
    let b = bbox(&h);
    let mut c = corners(b);
    let p = v(c[1].0, c[1].1);
    c[1] = (c[1].0 + 6.0, c[1].1 - 4.0);
    equivalent(
        &reposition_setup,
        "repositionAll",
        drag(&[p, (p.0 + 6.0, p.1 + 4.0)], &["alt"]),
        Some(("draw.repositionAll.apply", json!({ "corners": cj(&c) }))),
    );
    // Alternate + constrain on a side: the symmetric side drag.
    let right = v(b[2], (b[1] + b[3]) / 2.0);
    let mut c = corners(b);
    c[2] = (c[2].0 + 3.0, c[2].1 + 5.0);
    c[1] = (c[1].0 + 3.0, c[1].1 - 5.0);
    equivalent(
        &reposition_setup,
        "repositionAll",
        drag(&[right, (right.0 + 3.0, right.1 - 5.0)], &["alt", "shift"]),
        Some(("draw.repositionAll.apply", json!({ "corners": cj(&c) }))),
    );
}

#[test]
fn reposition_with_everything_locked_is_grey_and_refused() {
    let h = ready();
    reposition_setup(&h);
    ok(&h, "tool.active.set", json!({ "toolId": "repositionAll" }));
    let b = bbox(&h);
    let ids: Vec<Value> = ok(&h, "element.drawing.list", json!({ "elementId": BOB }))["drawings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].clone())
        .collect();
    assert!(!ids.is_empty());
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": ids, "locked": true }));
    assert_eq!(tool_box(&h, "drawingView")["locked"], true, "grey handles: nothing can move");
    let p = v((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let (bytes, hist) = (canonical(&h), history(&h));
    let env = h.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": "repositionAll", "events": drag(&[p, (p.0 + 9.0, p.1)], &[]) }),
    );
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("drawing.nothingToDo"), "{env:?}");
    assert_eq!((canonical(&h), history(&h)), (bytes, hist));
}

// ----- the golden maps (design note 4 §6, §9) -------------------------------------------------------------------------

/// `desktop/src/canvas/testdata/envelopes.json`: for each target (a keystone, a concave quadrilateral, a
/// parallelogram, and each with the mapping forced), the box the command maps, its corners, the
/// mapping, a pencil line's anchors and where `draw.envelope.apply` put them. Vitest's `envelope.test.ts` maps the
/// same anchors with the window's port (`envelopeOf`, `mapPoint`). (A self-intersecting target folds the line, which
/// noding then splits; it takes the concave target's bilinear branch.) `TT_UPDATE_CANVAS_CORPUS=1` rewrites it.
#[test]
fn the_envelope_golden_is_the_kernels_map() {
    let pts: Vec<(f64, f64)> =
        vec![(300.5, 359.0), (310.0, 305.0), (333.0, 351.5), (345.5, 312.25), (372.25, 341.0), (390.0, 330.0)];
    // (x increasing: the line does not cross itself, so noding keeps it one stroke.)
    // Corner offsets from the box's corners (bottom-left first, counter-clockwise).
    type Case<'a> = (&'a str, [(f64, f64); 4], &'a str);
    let cases: Vec<Case> = vec![
        ("keystone", [(0.0, 0.0), (0.0, 0.0), (-24.0, 0.0), (24.0, 0.0)], "auto"),
        ("concave", [(0.0, 0.0), (0.0, 0.0), (-60.0, -40.0), (0.0, 0.0)], "auto"),
        ("parallelogram", [(10.0, 0.0), (10.0, 0.0), (30.0, 0.0), (30.0, 0.0)], "auto"),
        ("keystone-bilinear", [(0.0, 0.0), (0.0, 0.0), (-24.0, 0.0), (24.0, 0.0)], "bilinear"),
        ("parallelogram-projective", [(10.0, 0.0), (10.0, 0.0), (30.0, 0.0), (30.0, 0.0)], "projective"),
    ];
    let mut out = Vec::new();
    for (name, offsets, mapping) in cases {
        let h = ready();
        let line: Vec<Value> = pts.iter().map(|p| json!({ "x": p.0, "y": p.1 })).collect();
        let r = ok(&h, "draw.polyline.add", json!({ "drawingId": D1, "points": line, "layer": "new" }));
        let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
        ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
        ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
        let b = bbox(&h);
        let c: Vec<(f64, f64)> = corners(b).iter().zip(offsets).map(|(p, o)| (p.0 + o.0, p.1 + o.1)).collect();
        let before = anchor_points(&h, &sid);
        ok(&h, "draw.envelope.apply", json!({ "drawingId": D1, "corners": cj(&c), "mapping": mapping }));
        let after = anchor_points(&h, &sid);
        out.push(json!({ "name": name, "box": b, "corners": c.iter().map(|p| [p.0, p.1]).collect::<Vec<_>>(),
                         "mapping": mapping, "points": before, "mapped": after }));
    }
    let text = serde_json::to_string_pretty(&json!({ "cases": out })).unwrap() + "\n";
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/canvas/testdata/envelopes.json");
    if std::env::var_os("TT_UPDATE_CANVAS_CORPUS").is_some() {
        std::fs::write(&path, &text).unwrap();
    } else {
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("envelopes.json: {e} (TT_UPDATE_CANVAS_CORPUS=1 writes it)"));
        assert!(committed.replace("\r\n", "\n") == text, "envelopes.json differs from the kernel's map");
    }
}

/// The anchors of a stroke, `[x, y]` each.
fn anchor_points(h: &ExecutorHandle, sid: &str) -> Vec<[f64; 2]> {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let ids: Vec<Value> = d["nodes"].as_array().unwrap().iter().map(|n| n["id"].clone()).collect();
    let n = d["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == sid)
        .cloned()
        .unwrap_or_else(|| panic!("{sid} in {ids:?}"));
    let b = n["bezier"].as_array().unwrap();
    let at = |i: usize, c: &str| b[i][c].as_f64().unwrap_or_else(|| panic!("{n}"));
    (0..b.len().div_ceil(3)).map(|k| [at(k * 3, "x"), at(k * 3, "y")]).collect()
}
