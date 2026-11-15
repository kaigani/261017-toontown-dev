//! The scene tools in the Camera view (gestures note §7: G7-G11; 05 §3.5, §5.0-§5.2, §17; SI-144, SI-148, SI-150).
//! Ruling D-23: each row names its spec line and asserts the spec's command, target, keyed channels and label, then
//! compares the gesture's path with the command sent directly.

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

use crate::camera::{BOB, apply, id_of, matrix, scene};
use crate::common::*;

fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

fn drag(points: &[(f64, f64)], mods: &[&str]) -> Vec<Value> {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let ty = if i == 0 {
                "down"
            } else if i + 1 == n {
                "up"
            } else {
                "move"
            };
            json!({ "type": ty, "x": p.0, "y": p.1, "modifiers": mods })
        })
        .collect()
}

fn click(p: (f64, f64), mods: &[&str]) -> Vec<Value> {
    drag(&[p, p], mods)
}

fn pointer(h: &ExecutorHandle, tool: Option<&str>, events: Vec<Value>) -> Value {
    let mut req = json!({ "view": "cameraView", "events": events });
    if let Some(t) = tool {
        req["tool"] = json!(t);
    }
    let env = h.pointer(Origin::User, req);
    assert!(env.is_ok(), "{env:?}");
    env.result.unwrap()
}

/// The last command a gesture produced (the one after any tool selection).
fn last(out: &Value) -> Value {
    out["commands"].as_array().unwrap().last().cloned().unwrap()
}

fn last_label(h: &ExecutorHandle) -> Value {
    let hist = ok(h, "edit.history.get", json!({}));
    let p = hist["position"].as_u64().unwrap() as usize;
    hist["entries"][p - 1]["label"].clone()
}

fn steps(h: &ExecutorHandle) -> u64 {
    ok(h, "edit.history.get", json!({}))["position"].as_u64().unwrap()
}

fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .into_iter()
        .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(&k).unwrap()).unwrap())
        .collect()
}

/// Bob's drawing units → Camera view pixels.
fn bob_to_view(h: &ExecutorHandle) -> [f64; 6] {
    let s = scene(h);
    let bob = id_of(h, BOB);
    let e = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == json!(bob)).cloned().unwrap();
    let (m, v) = (matrix(&e["matrix"]), matrix(&s["camera"]["cameraToView"]));
    [
        v[0] * m[0] + v[2] * m[1],
        v[1] * m[0] + v[3] * m[1],
        v[0] * m[2] + v[2] * m[3],
        v[1] * m[2] + v[3] * m[3],
        v[0] * m[4] + v[2] * m[5] + v[4],
        v[1] * m[4] + v[3] * m[5] + v[5],
    ]
}

/// The selection's union box and the first pivot, in Camera view pixels (scene answer overlays).
fn furniture(h: &ExecutorHandle) -> ([f64; 4], (f64, f64)) {
    let s = scene(h);
    let v = matrix(&s["camera"]["cameraToView"]);
    let u: Vec<f64> = s["overlays"]["selectionUnion"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let a = apply(&v, u[0], u[1]);
    let b = apply(&v, u[2], u[3]);
    let p = &s["overlays"]["pivots"][0];
    let pv = apply(&v, p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
    ([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)], pv)
}

fn selection(h: &ExecutorHandle) -> Value {
    ok(h, "edit.selection.get", json!({ "context": "staging" }))["ids"].clone()
}

/// G7 (05 §5.0 lines 395-397, §3.5 line 352, §17): a click picks the front-most unlocked element (`edit.selection.set`,
/// replace), the extend modifier toggles, a click on nothing clears, a rubber band selects every unlocked element it
/// touches; selection is not undoable (05 §17).
#[test]
fn g7_selection_by_click_extend_and_rubber_band() {
    let h = ready();
    let bob = id_of(&h, BOB);
    let c = apply(&bob_to_view(&h), 0.0, 0.0);
    let before = steps(&h);
    let out = pointer(&h, Some("select"), click(c, &[]));
    assert_eq!(
        out["commands"][0]["commandId"], "anim.tool.set",
        "SI-148: select names the scene Select in a Camera view"
    );
    let cmd = last(&out);
    assert_eq!(cmd["commandId"], "edit.selection.set");
    assert_eq!(cmd["params"], json!({ "context": "staging", "ids": [bob], "mode": "replace" }));
    assert_eq!(selection(&h), json!([bob]));
    assert_eq!(steps(&h), before, "05 §17: selection changes are not undoable");
    // The extend modifier toggles (Shift is the extend modifier here, as in the Drawing view).
    pointer(&h, None, click(c, &["shift"]));
    assert_eq!(selection(&h), json!([]));
    pointer(&h, None, click(c, &[]));
    // A click on nothing clears.
    let out = pointer(&h, None, click((2.0, 2.0), &[]));
    assert_eq!(last(&out)["params"]["ids"], json!([]));
    assert_eq!(selection(&h), json!([]));
    // A rubber band across the whole view selects Bob; a locked element is never picked (05 §5.0 line 400).
    let out = pointer(&h, None, drag(&[(1.0, 1.0), (959.0, 539.0)], &[]));
    assert!(last(&out)["params"]["ids"].as_array().unwrap().contains(&json!(bob)), "{out}");
    ok(&h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true }));
    pointer(&h, None, click((2.0, 2.0), &[]));
    let out = pointer(&h, None, click(c, &[]));
    assert_eq!(last(&out)["params"]["ids"], json!([]), "a locked element is not picked");
    let out = pointer(&h, None, drag(&[(1.0, 1.0), (959.0, 539.0)], &[]));
    assert!(!last(&out)["params"]["ids"].as_array().unwrap().contains(&json!(bob)));
}

/// One scene-drag row of gestures note §7's table: the gesture, the spec line, the expected command fields, the
/// expected label (SI-144), the channels a Transform drag keys (05 line 398).
struct Row {
    what: &'static str,
    spec: &'static str,
    tool: &'static str,
    events: fn(&[f64; 4], (f64, f64)) -> Vec<Value>,
    field: &'static str,
    label: &'static str,
    keys: &'static [&'static str],
}

fn centre(b: &[f64; 4]) -> (f64, f64) {
    ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0)
}

/// G7, G8, G9 (05 §5.1 lines 403-409, §5.2 lines 411-420, line 398, §17 line 1135): every drag of the table ends in
/// one `anim.transform.set` with the tool's target (Select: static, no keys; Transform: keyed at the current frame,
/// only the changed channels; a pivot drag: static, no key), the SI-144 label, one undo step; the same command sent
/// directly gives the same document and label.
#[test]
fn g7_g8_g9_every_scene_drag_ends_in_its_command_target_keys_and_label() {
    let rows = [
        Row {
            what: "move",
            spec: "05 §5.1 line 405",
            tool: "select",
            events: |b, _| {
                let c = centre(b);
                drag(&[c, (c.0 + 40.0, c.1 + 10.0)], &[])
            },
            field: "position",
            label: "Move Element",
            keys: &[],
        },
        Row {
            what: "move",
            spec: "05 §5.2 line 416",
            tool: "transform",
            events: |b, _| {
                let c = centre(b);
                drag(&[c, (c.0 + 40.0, c.1 + 10.0)], &[])
            },
            field: "position",
            label: "Move Element",
            keys: &["position"],
        },
        Row {
            what: "corner scale",
            spec: "05 §5.1 line 406",
            tool: "select",
            events: |b, _| drag(&[(b[2], b[1]), (b[2] + 30.0, b[1] - 20.0)], &[]),
            field: "scale",
            label: "Scale Element",
            keys: &[],
        },
        Row {
            what: "corner scale",
            spec: "05 §5.2 line 417",
            tool: "transform",
            events: |b, _| drag(&[(b[2], b[1]), (b[2] + 30.0, b[1] - 20.0)], &[]),
            field: "scale",
            label: "Scale Element",
            keys: &["scale"],
        },
        Row {
            what: "side scale",
            spec: "05 §5.1 line 406",
            tool: "select",
            events: |b, _| {
                let y = (b[1] + b[3]) / 2.0;
                drag(&[(b[2], y), (b[2] + 25.0, y)], &[])
            },
            field: "scale",
            label: "Scale Element",
            keys: &[],
        },
        Row {
            what: "rotate",
            spec: "05 §5.1 line 407",
            tool: "select",
            events: |b, _| drag(&[(b[2] + 9.0, b[1] - 9.0), (b[2] + 9.0, b[3] + 9.0)], &[]),
            field: "rotation",
            label: "Rotate Element",
            keys: &[],
        },
        Row {
            what: "rotate",
            spec: "05 §5.2 line 417",
            tool: "transform",
            events: |b, _| drag(&[(b[2] + 9.0, b[1] - 9.0), (b[2] + 9.0, b[3] + 9.0)], &[]),
            field: "rotation",
            label: "Rotate Element",
            keys: &["rotation"],
        },
        Row {
            what: "skew",
            spec: "05 §5.1 line 407",
            tool: "select",
            events: |b, _| {
                let x = (b[0] + b[2]) / 2.0;
                drag(&[(x, b[1] - 12.0), (x + 20.0, b[1] - 12.0)], &[])
            },
            field: "skew",
            label: "Skew Element",
            keys: &[],
        },
        Row {
            what: "skew",
            spec: "05 §5.2 line 417",
            tool: "transform",
            events: |b, _| {
                let x = (b[0] + b[2]) / 2.0;
                drag(&[(x, b[1] - 12.0), (x + 20.0, b[1] - 12.0)], &[])
            },
            field: "skew",
            label: "Skew Element",
            keys: &["skew"],
        },
        Row {
            what: "pivot",
            spec: "05 §5.2 line 418",
            tool: "transform",
            events: |_, p| drag(&[p, (p.0 + 15.0, p.1 - 5.0)], &[]),
            field: "rotationPivot",
            label: "Move Pivot Element",
            keys: &[],
        },
    ];
    for row in rows {
        let (a, b) = (ready(), ready());
        for h in [&a, &b] {
            ok(h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
        }
        let bob = id_of(&a, BOB);
        let (bx, pv) = furniture(&a);
        let before = steps(&a);
        let out = pointer(&a, Some(row.tool), (row.events)(&bx, pv));
        let cmd = last(&out);
        let what = format!("{} {} ({})", row.tool, row.what, row.spec);
        assert_eq!(cmd["commandId"], "anim.transform.set", "{what}: {out}");
        let p = &cmd["params"];
        assert_eq!(p["nodeIds"], json!([bob]), "{what}");
        assert!(!p[row.field].is_null(), "{what}: changes {}", row.field);
        let static_target = row.tool == "select" || row.what == "pivot";
        assert_eq!(p["target"], if static_target { "static" } else { "key" }, "{what}: target");
        if !static_target {
            assert_eq!(p["frame"], 1, "{what}: keyed at the current frame");
        }
        let keyed: Vec<String> = cmd["envelope"]["result"]["keyedChannels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_owned())
            .collect();
        if row.keys.is_empty() {
            assert!(keyed.is_empty(), "{what}: no keys (Select never keys; a pivot drag keys nothing): {keyed:?}");
        } else {
            assert!(
                !keyed.is_empty() && keyed.iter().all(|k| row.keys.iter().any(|w| k.starts_with(w))),
                "{what}: keys only {:?}: {keyed:?}",
                row.keys
            );
        }
        assert_eq!(last_label(&a), json!(row.label), "{what}: SI-144");
        assert_eq!(steps(&a), before + 1, "{what}: one undo step");
        // G8: the command sent directly.
        let direct = ok(&b, "anim.transform.set", p.clone());
        let _ = direct;
        assert_eq!(canonical(&a), canonical(&b), "{what}: the same document");
        assert_eq!(last_label(&b), json!(row.label), "{what}: the same label");
    }
}

/// 05 §3.5 line 347-349: a Camera-view move keeps the element under the pointer: its centre moves by the drag in view
/// pixels.
#[test]
fn a_camera_view_move_keeps_the_element_under_the_pointer() {
    let h = ready();
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let c = apply(&bob_to_view(&h), 0.0, 0.0);
    pointer(&h, Some("select"), drag(&[c, (c.0 + 40.0, c.1 + 10.0)], &[]));
    let after = apply(&bob_to_view(&h), 0.0, 0.0);
    assert!((after.0 - c.0 - 40.0).abs() < 1e-6 && (after.1 - c.1 - 10.0).abs() < 1e-6, "{c:?} -> {after:?}");
}

/// A gesture still pressed at the end of the events commits nothing (D-7c), in the Camera view too.
#[test]
fn an_unreleased_scene_drag_commits_nothing() {
    let h = ready();
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let c = apply(&bob_to_view(&h), 0.0, 0.0);
    let before = canonical(&h);
    let out = pointer(
        &h,
        Some("select"),
        vec![json!({ "type": "down", "x": c.0, "y": c.1 }), json!({ "type": "move", "x": c.0 + 30.0, "y": c.1 })],
    );
    assert_eq!(out["commands"].as_array().unwrap().len(), 1, "only the tool selection: {out}");
    assert_eq!(canonical(&h), before);
}

/// How a G10 row's gesture is made from its drawing-unit points.
#[derive(Clone, Copy)]
enum Shape {
    /// One press, moves through the points, a release.
    Drag,
    /// A click at the first point.
    Click,
    /// A click at each point, then a double click at the last (the Polyline, 02 §7.2.5).
    Clicks,
}

fn gesture(shape: Shape, pts: &[(f64, f64)]) -> Vec<Value> {
    match shape {
        Shape::Drag => drag(pts, &[]),
        Shape::Click => click(pts[0], &[]),
        Shape::Clicks => {
            let mut e: Vec<Value> = pts.iter().flat_map(|p| click(*p, &[])).collect();
            let l = pts[pts.len() - 1];
            e.push(json!({ "type": "doubleClick", "x": l.0, "y": l.1, "modifiers": [] }));
            e
        }
    }
}

/// A rectangle of Bob's current drawing, unpainted (for the Paint tools).
fn outlined(h: &ExecutorHandle) {
    ok(
        h,
        "draw.rectangle.add",
        json!({ "from": { "x": 300, "y": 300 }, "to": { "x": 400, "y": 360 }, "layer": "new" }),
    );
}

/// The same rectangle painted (for Unpaint, the Dropper and the cuts).
fn painted(h: &ExecutorHandle) {
    outlined(h);
    ok(h, "colour.paint.apply", json!({ "points": [{ "x": 350, "y": 330 }] }));
}

/// G10 (02 §7, line 61, line 413): every drawing tool's gesture in the Camera view (each C4.2 tool but the two
/// navigation tools, which navigate the view they are used in) produces the command, the parameters (in drawing units)
/// and the 02 §7 label of the same gesture made in the Drawing view.
#[test]
fn g10_drawing_tools_in_the_camera_view_equal_the_drawing_view() {
    let line = [(-60.0, 20.0), (-20.0, 45.0), (30.0, 10.0)];
    // Across the painted rectangle (300..400 x 300..360), so a cut divides it.
    let across = [(350.0, 380.0), (420.0, 380.0), (420.0, 280.0), (350.0, 280.0)];
    let lasso = [(280.0, 380.0), (420.0, 380.0), (420.0, 280.0), (280.0, 280.0)];
    let none: fn(&ExecutorHandle) = |_| {};
    // Each row: the tool, its gesture, its drawing-unit points, a setup, and 02 §7's label with its line (`None`: not
    // an undo step: the Dropper sets the current colour, a Select rubber band selects).
    type G10Row = (&'static str, Shape, Vec<(f64, f64)>, fn(&ExecutorHandle), Option<&'static str>);
    let rows: Vec<G10Row> = vec![
        ("brush", Shape::Drag, line.to_vec(), none, Some("Brush Tool")), // 02 line 495
        ("pencil", Shape::Drag, line.to_vec(), none, Some("Pencil Tool")), // line 504
        ("rectangle", Shape::Drag, line.to_vec(), none, Some("Rectangle Tool")), // line 513
        ("ellipse", Shape::Drag, line.to_vec(), none, Some("Ellipse Tool")), // line 520
        ("line", Shape::Drag, line.to_vec(), none, Some("Line Tool")),   // line 543
        ("polyline", Shape::Clicks, line.to_vec(), none, Some("Polyline Tool")), // line 537
        ("stroke", Shape::Drag, line.to_vec(), none, Some("Stroke Tool")), // line 614
        ("eraser", Shape::Drag, line.to_vec(), none, Some("Eraser Tool")), // line 639
        ("cutter", Shape::Drag, across.to_vec(), painted, Some("Cutter Tool")), // line 647
        ("scissor", Shape::Drag, vec![(350.0, 380.0), (420.0, 280.0)], painted, Some("Scissor Tool")), // line 654
        ("paint", Shape::Click, vec![(350.0, 330.0)], outlined, Some("Paint")), // line 564
        ("unpaint", Shape::Click, vec![(350.0, 330.0)], painted, Some("Unpaint")), // line 573
        ("paintUnpainted", Shape::Drag, lasso.to_vec(), outlined, Some("Paint Unpainted")), // line 582
        ("dropper", Shape::Click, vec![(350.0, 330.0)], painted, None),
        // The fixture drawing's two stroke ends of the `colour.gap.close` example.
        ("closeGap", Shape::Drag, vec![(-48.0, -50.0), (2.0, 2.0)], none, Some("Close Gap")), // line 603
        ("select", Shape::Drag, vec![(-150.0, 150.0), (150.0, -150.0)], none, None),
    ];
    for (tool, shape, pts, setup, label) in rows {
        let (cam, drw) = (ready(), ready());
        setup(&cam);
        setup(&drw);
        let (steps_a, steps_b) = (steps(&cam), steps(&drw));
        let m = bob_to_view(&cam);
        let view_pts: Vec<(f64, f64)> = pts.iter().map(|p| apply(&m, p.0, p.1)).collect();
        // `select` names the scene Select in a Camera view (SI-148): the drawing Select is chosen first.
        let a = if tool == "select" {
            ok(&cam, "tool.active.set", json!({ "toolId": "select" }));
            pointer(&cam, None, gesture(shape, &view_pts))
        } else {
            let a = pointer(&cam, Some(tool), gesture(shape, &view_pts));
            assert_eq!(
                a["commands"][0]["commandId"], "tool.active.set",
                "{tool}: a drawing tool id selects the drawing tool"
            );
            a
        };
        // The headless Drawing view's default mapping (SI-069): view = (x + 960, 540 - y).
        let dv: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 + 960.0, 540.0 - p.1)).collect();
        let env =
            drw.pointer(Origin::User, json!({ "view": "drawingView", "tool": tool, "events": gesture(shape, &dv) }));
        assert!(env.is_ok(), "{tool}: {env:?}");
        let b = env.result.unwrap();
        let (ca, cb) = (last(&a), last(&b));
        assert_eq!(ca["commandId"], cb["commandId"], "{tool}");
        assert!(params_close(&ca["params"], &cb["params"]), "{tool}: {} vs {}", ca["params"], cb["params"]);
        match label {
            Some(l) => {
                assert_eq!(last_label(&cam), json!(l), "{tool}: 02 §7");
                assert_eq!(last_label(&drw), json!(l), "{tool}");
                assert_eq!((steps(&cam), steps(&drw)), (steps_a + 1, steps_b + 1), "{tool}: one undo step each");
            }
            None => assert_eq!((steps(&cam), steps(&drw)), (steps_a, steps_b), "{tool}: no undo step"),
        }
    }
}

/// Two parameter values equal, numbers within 1e-6 (the two views' mappings round differently).
fn params_close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-6,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| params_close(p, q)),
        // `view` names the view a gesture was made in (its tolerances), so it is the one member that differs.
        (Value::Object(x), Value::Object(y)) => {
            let keys = |o: &serde_json::Map<String, Value>| o.keys().filter(|k| *k != "view").count();
            keys(x) == keys(y) && x.iter().all(|(k, v)| k == "view" || y.get(k).is_some_and(|w| params_close(v, w)))
        }
        _ => a == b,
    }
}

/// G11 (05 lines 29-31): the scene tool and the drawing tool are independent; the family a Camera view runs is the one
/// chosen last (SI-148).
#[test]
fn g11_the_scene_tool_and_the_drawing_tool_are_independent() {
    let h = ready();
    ok(&h, "tool.active.set", json!({ "toolId": "brush" }));
    ok(&h, "anim.tool.set", json!({ "tool": "transform" }));
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "brush");
    assert_eq!(ok(&h, "anim.tool.get", json!({}))["tool"], "transform");
    ok(&h, "tool.active.set", json!({ "toolId": "pencil" }));
    assert_eq!(ok(&h, "anim.tool.get", json!({}))["tool"], "transform");
    // The other scene tools are M6's (precheck equal to the handler).
    for t in ["rotate", "skew", "scale", "motion", "record", "boneCreate"] {
        let e = err(&h, "anim.tool.set", json!({ "tool": t }));
        assert_eq!((e.code.as_str(), e.details["owner"].as_str()), ("command.notImplemented", Some("M6")), "{t}");
    }
    assert_eq!(
        err(&h, "anim.tool.set", json!({ "tool": "select", "cycleRotate": true })).code,
        "command.notImplemented"
    );
    // The family: after tool.active.set a Camera-view press runs the drawing tool (a pencil stroke), after anim.tool.set
    // the scene tool (a selection change).
    let c = apply(&bob_to_view(&h), 0.0, 0.0);
    let out = pointer(&h, None, drag(&[c, (c.0 + 30.0, c.1)], &[]));
    assert_eq!(last(&out)["commandId"], "draw.stroke.add");
    ok(&h, "anim.tool.set", json!({ "tool": "select" }));
    let out = pointer(&h, None, click(c, &[]));
    assert_eq!(last(&out)["commandId"], "edit.selection.set");
}
