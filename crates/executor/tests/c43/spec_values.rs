//! Every scene-tool drag against the value the spec gives (ruling D-23; C4.3a round 1: C43A-R1-21, C43A-R1-22 and the
//! observation that the table rows did not all state a spec-derived value). Each row names its spec line and asserts
//! the command, its parameters to 1e-9 (derived here from 05 §2.5-§2.6, §3.5 and SI-150's furniture, not read from the
//! gesture code), the target, the channels keyed (read back from the document with `anim.key.list`), the label
//! (SI-144) and one undo step; then the command sent directly gives the same document and label.
//!
//! The element is a 1-field square (100 x 100 drawing units) at (0.5, 0.3) on the table plane, top level, with no
//! rotation, skew or scale, seen through the default camera, so a field at depth 0 is `focal / camera Z` output pixels
//! (05 §2.6) and the Camera view's transform scales that by `sqrt(det cameraToView)`.

use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, Origin};

use crate::camera::{apply, matrix, scene, square};
use crate::common::*;

fn world() -> (ExecutorHandle, String) {
    let h = fixture();
    let id = square(&h, "Sq", (0.5, 0.3, 0.0), 0.0);
    ok(&h, "app.context.set", json!({ "elementId": id, "frame": 1 }));
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [id] }));
    (h, id)
}

/// View pixels per field on the table plane (05 §2.6): focal = W / (2 tan(FOV / 2)) output pixels at unit depth,
/// divided by the camera's distance, times the Camera view's scale.
fn pixels_per_field(h: &ExecutorHandle) -> f64 {
    let s = scene(h);
    let cam = ok(h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    let c = ok(h, "anim.camera.get", json!({ "cameraId": cam, "frame": 1 }));
    let fov = c["fov"].as_f64().unwrap();
    let z = c["position"]["z"].as_f64().unwrap();
    let w = s["camera"]["size"][0].as_f64().unwrap();
    let v = matrix(&s["camera"]["cameraToView"]);
    w / (2.0 * (fov.to_radians() / 2.0).tan()) / z * (v[0] * v[3] - v[1] * v[2]).abs().sqrt()
}

/// The selection box in view pixels (`[minX, minY, maxX, maxY]`, y down: SI-150's furniture sits on it) and the pivot
/// marker (the element's rotation pivot, its origin here).
fn furniture(h: &ExecutorHandle) -> ([f64; 4], (f64, f64)) {
    let s = scene(h);
    let v = matrix(&s["camera"]["cameraToView"]);
    let u: Vec<f64> = s["overlays"]["selectionUnion"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let (a, b) = (apply(&v, u[0], u[1]), apply(&v, u[2], u[3]));
    let p = &s["overlays"]["pivots"][0];
    let pv = apply(&v, p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
    ([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)], pv)
}

fn drag(a: (f64, f64), b: (f64, f64), mods: &[&str]) -> Vec<Value> {
    vec![
        json!({ "type": "down", "x": a.0, "y": a.1, "modifiers": mods }),
        json!({ "type": "move", "x": (a.0 + b.0) / 2.0, "y": (a.1 + b.1) / 2.0, "modifiers": mods }),
        json!({ "type": "up", "x": b.0, "y": b.1, "modifiers": mods }),
    ]
}

fn pointer(h: &ExecutorHandle, tool: &str, events: Vec<Value>) -> Value {
    let env = h.pointer(Origin::User, json!({ "view": "cameraView", "tool": tool, "events": events }));
    assert!(env.is_ok(), "{env:?}");
    env.result.unwrap()
}

fn transform_command(out: &Value) -> Option<Value> {
    out["commands"].as_array().unwrap().iter().find(|c| c["commandId"] == "anim.transform.set").cloned()
}

fn steps(h: &ExecutorHandle) -> u64 {
    ok(h, "edit.history.get", json!({}))["position"].as_u64().unwrap()
}

fn last_label(h: &ExecutorHandle) -> Value {
    let hist = ok(h, "edit.history.get", json!({}));
    let p = hist["position"].as_u64().unwrap() as usize;
    hist["entries"][p - 1]["label"].clone()
}

/// The saved content without the creation times the clock stamps on the square's element and drawing in each world.
fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .into_iter()
        .map(|k| {
            let mut v = d.entry_raw_json(&k).unwrap();
            if let Some(o) = v.as_object_mut() {
                o.remove("createdAt");
            }
            tt_model::canonical::to_canonical_string(&v).unwrap()
        })
        .collect()
}

/// The channels keyed on `id` at frame 1, read from the document.
fn keyed_at_1(h: &ExecutorHandle, id: &str) -> Vec<String> {
    let k = ok(h, "anim.key.list", json!({ "nodeId": id, "from": 1, "to": 1 }));
    let mut out: Vec<String> = k["keys"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["channels"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()))
        .collect();
    out.sort();
    out
}

/// A row's gesture: from the box, the pivot marker and the view pixels per field, the press, the release and the
/// expected fields.
type Gesture = fn(&[f64; 4], (f64, f64), f64) -> ((f64, f64), (f64, f64), Value);

/// One row: the gesture, its spec line, the tool, the modifiers, the expected changed fields, the label, the channels
/// keyed at frame 1 (`position` stands for the position channels a move keys).
struct Row {
    what: &'static str,
    spec: &'static str,
    tool: &'static str,
    mods: &'static [&'static str],
    /// From the box, the pivot marker and the view pixels per field: the press, the release and the expected fields.
    gesture: Gesture,
    target: &'static str,
    label: &'static str,
    keys: &'static [&'static str],
}

fn mid_x(b: &[f64; 4]) -> f64 {
    (b[0] + b[2]) / 2.0
}

fn mid_y(b: &[f64; 4]) -> f64 {
    (b[1] + b[3]) / 2.0
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            what: "move",
            spec: "05 §5.1 line 405; §3.5 lines 347-349 (X/Y at the node's depth)",
            tool: "select",
            mods: &[],
            gesture: |_, p, k| {
                (
                    (p.0 + 20.0, p.1 + 15.0),
                    (p.0 + 60.0, p.1 + 25.0),
                    json!({ "position": { "x": 40.0 / k, "y": -10.0 / k } }),
                )
            },
            target: "static",
            label: "Move Element",
            keys: &[],
        },
        Row {
            what: "move",
            spec: "05 §5.2 line 416 (keys position)",
            tool: "transform",
            mods: &[],
            gesture: |_, p, k| {
                (
                    (p.0 + 20.0, p.1 + 15.0),
                    (p.0 + 60.0, p.1 + 25.0),
                    json!({ "position": { "x": 40.0 / k, "y": -10.0 / k } }),
                )
            },
            target: "key",
            label: "Move Element",
            keys: &["position"],
        },
        Row {
            what: "corner scale",
            spec: "05 §5.2 line 417; line 398 (scale.x and scale.y)",
            tool: "transform",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((b[2], b[1]), (b[2] + 30.0, b[1] - 20.0));
                // About the opposite corner (SI-150).
                let (sx, sy) = ((p1.0 - b[0]) / (p0.0 - b[0]), (p1.1 - b[3]) / (p0.1 - b[3]));
                (p0, p1, json!({ "scale": { "x": sx, "y": sy } }))
            },
            target: "key",
            label: "Scale Element",
            keys: &["scale.x", "scale.y"],
        },
        Row {
            what: "right side scale",
            spec: "05 §5.2 line 417; line 398: a horizontal side keys scale.x only (C43A-R1-22)",
            tool: "transform",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((b[2], mid_y(b)), (b[2] + 25.0, mid_y(b)));
                (p0, p1, json!({ "scale": { "x": (p1.0 - b[0]) / (p0.0 - b[0]) } }))
            },
            target: "key",
            label: "Scale Element",
            keys: &["scale.x"],
        },
        Row {
            what: "top side scale",
            spec: "05 §5.2 line 417; line 398: a vertical drag of the top side keys scale.y only (C43A-R1-22)",
            tool: "transform",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((mid_x(b), b[1]), (mid_x(b), b[1] - 20.0));
                (p0, p1, json!({ "scale": { "y": (p1.1 - b[3]) / (p0.1 - b[3]) } }))
            },
            target: "key",
            label: "Scale Element",
            keys: &["scale.y"],
        },
        Row {
            what: "right side scale",
            spec: "05 §5.1 line 406 (static: the other axis is not written)",
            tool: "select",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((b[2], mid_y(b)), (b[2] + 25.0, mid_y(b)));
                (p0, p1, json!({ "scale": { "x": (p1.0 - b[0]) / (p0.0 - b[0]) } }))
            },
            target: "static",
            label: "Scale Element",
            keys: &[],
        },
        Row {
            what: "rotate",
            spec: "05 §5.2 line 417 (keys rotation); 11 line 148 (anticlockwise positive)",
            tool: "transform",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((b[2] + 9.0, b[1] - 9.0), (b[2] + 9.0, b[3] + 9.0));
                let c = (mid_x(b), mid_y(b));
                let a0 = (p0.1 - c.1).atan2(p0.0 - c.0);
                let a1 = (p1.1 - c.1).atan2(p1.0 - c.0);
                let mut deg = -(a1 - a0).to_degrees();
                if deg > 180.0 {
                    deg -= 360.0;
                } else if deg <= -180.0 {
                    deg += 360.0;
                }
                (p0, p1, json!({ "rotation": deg }))
            },
            target: "key",
            label: "Rotate Element",
            keys: &["rotation"],
        },
        Row {
            what: "skew handle",
            spec: "05 §5.2 line 417 (keys skew); SI-150 (tan = drag / half the height)",
            tool: "transform",
            mods: &[],
            gesture: |b, _, _| {
                let (p0, p1) = ((mid_x(b), b[1] - 12.0), (mid_x(b) + 20.0, b[1] - 12.0));
                (p0, p1, json!({ "skew": (20.0 / ((b[3] - b[1]) / 2.0)).atan().to_degrees() }))
            },
            target: "key",
            label: "Skew Element",
            keys: &["skew"],
        },
        Row {
            what: "pivot",
            spec: "05 §5.2 line 418 (moves the static pivot, no key)",
            tool: "transform",
            mods: &[],
            gesture: |_, p, k| {
                let q = json!({ "x": 15.0 / k, "y": 5.0 / k });
                (p, (p.0 + 15.0, p.1 - 5.0), json!({ "rotationPivot": q, "scalePivot": q }))
            },
            target: "static",
            label: "Move Pivot Element",
            keys: &[],
        },
        Row {
            what: "temporary Skew, top side (horizontal shear)",
            spec: "05 §5.0 line 394 (alternate modifier with Transform: the Skew tool); §5.4 (top/bottom: skew); SI-151",
            tool: "transform",
            mods: &["alt"],
            gesture: |b, _, _| {
                let (p0, p1) = ((mid_x(b), b[1]), (mid_x(b) + 20.0, b[1]));
                (p0, p1, json!({ "skew": (20.0 / ((b[3] - b[1]) / 2.0)).atan().to_degrees() }))
            },
            target: "key",
            label: "Skew Element",
            keys: &["skew"],
        },
        Row {
            what: "temporary Skew, right side (vertical shear)",
            spec: "05 §5.0 line 394; §5.4 (left/right: rotation + skew + scale by the §2.5 decomposition); SI-151",
            tool: "transform",
            mods: &["alt"],
            gesture: |b, _, _| {
                let (p0, p1) = ((b[2], mid_y(b)), (b[2], mid_y(b) - 20.0));
                // [[1, 0], [k, 1]] = R(θ)·K(κ)·S(sx, sy) with tan θ = k, κ = θ, sx = sqrt(1 + k²), sy = 1 / sx.
                let k = 20.0 / ((b[2] - b[0]) / 2.0);
                let theta = k.atan().to_degrees();
                let sx = (1.0 + k * k).sqrt();
                (p0, p1, json!({ "rotation": theta, "skew": theta, "scale": { "x": sx, "y": 1.0 / sx } }))
            },
            target: "key",
            label: "Transform Element",
            keys: &["rotation", "scale.x", "scale.y", "skew"],
        },
        Row {
            what: "temporary Select over the temporary Skew",
            spec: "05 §5.0 line 393 (the temporary-select modifier, Skew included, gives Select)",
            tool: "transform",
            mods: &["alt", "ctrl"],
            gesture: |_, p, k| {
                (
                    (p.0 + 20.0, p.1 + 15.0),
                    (p.0 + 60.0, p.1 + 25.0),
                    json!({ "position": { "x": 40.0 / k, "y": -10.0 / k } }),
                )
            },
            target: "static",
            label: "Move Element",
            keys: &[],
        },
        Row {
            what: "the alternate modifier with Select",
            spec: "05 §5.0 line 394 (only the Transform tool has a temporary Skew)",
            tool: "select",
            mods: &["alt"],
            gesture: |_, p, k| {
                (
                    (p.0 + 20.0, p.1 + 15.0),
                    (p.0 + 60.0, p.1 + 25.0),
                    json!({ "position": { "x": 40.0 / k, "y": -10.0 / k } }),
                )
            },
            target: "static",
            label: "Move Element",
            keys: &[],
        },
    ]
}

/// The expected fields within 1e-9, and no other changed field (a channel the gesture did not change is not sent).
fn assert_fields(what: &str, params: &Value, expected: &Value, target: &str) {
    let mut want: Map<String, Value> = expected.as_object().unwrap().clone();
    for (k, v) in &want {
        let got = &params[k];
        match v {
            Value::Number(n) => {
                let (g, n) = (got.as_f64().unwrap_or(f64::NAN), n.as_f64().unwrap());
                assert!((g - n).abs() <= 1e-9 * n.abs().max(1.0), "{what}: {k} = {got}, the spec gives {n}");
            }
            Value::Object(o) => {
                let keys: Vec<&String> = got.as_object().unwrap().keys().collect();
                assert_eq!(keys, o.keys().collect::<Vec<_>>(), "{what}: {k} has exactly the changed axes");
                for (axis, n) in o {
                    let (g, n) = (got[axis].as_f64().unwrap(), n.as_f64().unwrap());
                    assert!((g - n).abs() <= 1e-9 * n.abs().max(1.0), "{what}: {k}.{axis} = {g}, the spec gives {n}");
                }
            }
            _ => unreachable!(),
        }
    }
    want.insert("nodeIds".into(), params["nodeIds"].clone());
    want.insert("target".into(), json!(target));
    if target == "key" {
        want.insert("frame".into(), json!(1));
    }
    if expected.get("rotationPivot").is_none() {
        want.insert("relative".into(), json!(true));
    }
    let mut got: Vec<&String> = params.as_object().unwrap().keys().collect();
    got.sort();
    let mut keys: Vec<&String> = want.keys().collect();
    keys.sort();
    assert_eq!(got, keys, "{what}: exactly these parameters");
    assert_eq!(params["target"], json!(target), "{what}: target");
    if target == "key" {
        assert_eq!(params["frame"], json!(1), "{what}: keyed at the current frame (05 line 397)");
    }
}

#[test]
fn every_scene_drag_has_the_spec_value_keys_and_label() {
    for row in rows() {
        let (a, id) = world();
        let (b, _) = world();
        let k = pixels_per_field(&a);
        let (bx, pv) = furniture(&a);
        let (p0, p1, expected) = (row.gesture)(&bx, pv, k);
        let what = format!("{} {} ({})", row.tool, row.what, row.spec);
        let before = steps(&a);
        let keys_before = keyed_at_1(&a, &id);
        assert!(keys_before.is_empty(), "{what}: no keys before");
        let out = pointer(&a, row.tool, drag(p0, p1, row.mods));
        let cmd = transform_command(&out).unwrap_or_else(|| panic!("{what}: an anim.transform.set: {out}"));
        assert_eq!(cmd["params"]["nodeIds"], json!([id]), "{what}");
        assert_fields(&what, &cmd["params"], &expected, row.target);
        let mut keys = keyed_at_1(&a, &id);
        keys.iter_mut().for_each(|c| {
            if c.starts_with("position") {
                *c = "position".to_owned();
            }
        });
        keys.dedup();
        assert_eq!(keys, row.keys, "{what}: the channels keyed (05 line 398)");
        assert_eq!(last_label(&a), json!(row.label), "{what}: SI-144");
        assert_eq!(steps(&a), before + 1, "{what}: one undo step (05 line 399)");
        assert_eq!(ok(&a, "anim.tool.get", json!({}))["tool"], json!(row.tool), "{what}: the stored tool is unchanged");
        // G8: the command sent directly.
        ok(&b, "anim.transform.set", cmd["params"].clone());
        assert_eq!(canonical(&a), canonical(&b), "{what}: the same document");
        assert_eq!(last_label(&b), json!(row.label), "{what}: the same label");
    }
}

/// SI-151: under the temporary Skew a drag from anywhere but a side commits nothing, and the stored tool stays
/// Transform (05 §5.0 line 394: "while ... held"); a click still selects.
#[test]
fn the_temporary_skew_acts_only_from_a_side() {
    let (h, id) = world();
    let (b, p) = furniture(&h);
    let saved = canonical(&h);
    let before = steps(&h);
    for (from, to) in [((p.0 + 20.0, p.1 + 15.0), (p.0 + 60.0, p.1 + 25.0)), ((b[2], b[1]), (b[2] + 30.0, b[1] - 20.0))]
    {
        let out = pointer(&h, "transform", drag(from, to, &["alt"]));
        assert!(transform_command(&out).is_none(), "inside the box or at a corner: nothing: {out}");
        assert_eq!(canonical(&h), saved);
        assert_eq!(steps(&h), before);
    }
    assert_eq!(ok(&h, "anim.tool.get", json!({})), json!({ "tool": "transform" }));
    let c = (p.0 + 20.0, p.1 + 15.0);
    let out = pointer(&h, "transform", drag(c, c, &["alt"]));
    let last = out["commands"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["commandId"], "edit.selection.set", "{out}");
    assert_eq!(last["params"]["ids"], json!([id]));
}
