//! M3 C3.2 (builder): the semantics of the animation commands against independent values: transform composition and
//! pivots (05 §2), channels and frames (05 §1.6, §2.10, §10), motion paths and velocity (05 §9, §10.5), SI-089 and loops
//! (05 §8.4), cameras and projection (05 §2.6, §12), `anim.evaluate` (05 §19.4), AG-02. Expected values come from the
//! small independent matrix type below or are derived by hand, never from the evaluator.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const NOTES: &str = "scene:\"Intro\"/element:\"Notes\"";
const CAMERA: &str = "scene:\"Intro\"/element:\"Camera\"";
const SOUND: &str = "scene:\"Intro\"/element:\"Dialogue\"";

/// An independent 3 × 3 homogeneous X/Y matrix with Z tracked apart (05 §2.1: only translation changes Z).
#[derive(Clone, Copy)]
struct M([f64; 6], f64);

impl M {
    const I: M = M([1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.0);
    fn t(x: f64, y: f64, z: f64) -> M {
        M([1.0, 0.0, 0.0, 1.0, x, y], z)
    }
    fn r(deg: f64) -> M {
        let (s, c) = deg.to_radians().sin_cos();
        M([c, s, -s, c, 0.0, 0.0], 0.0)
    }
    fn k(deg: f64) -> M {
        M([1.0, 0.0, deg.to_radians().tan(), 1.0, 0.0, 0.0], 0.0)
    }
    fn s(x: f64, y: f64) -> M {
        M([x, 0.0, 0.0, y, 0.0, 0.0], 0.0)
    }
    fn then(self, o: M) -> M {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = o.0;
        M([a * p + c * q, b * p + d * q, a * r + c * s, b * r + d * s, a * t + c * u + e, b * t + d * u + f], self.1 + o.1)
    }
    /// The row-major 4 × 4 of 05 §2.1.
    fn rows(self) -> [f64; 16] {
        let [a, b, c, d, e, f] = self.0;
        [a, c, 0.0, e, b, d, 0.0, f, 0.0, 0.0, 1.0, self.1, 0.0, 0.0, 0.0, 1.0]
    }
}

/// `M_local` of 05 §2.2, written independently of the product.
fn local(p: [f64; 3], th: f64, ka: f64, s: [f64; 2], cr: [f64; 2], cs: [f64; 2]) -> M {
    M::t(p[0], p[1], p[2])
        .then(M::t(cr[0], cr[1], 0.0))
        .then(M::r(th))
        .then(M::t(-cr[0], -cr[1], 0.0))
        .then(M::t(cs[0], cs[1], 0.0))
        .then(M::k(ka))
        .then(M::s(s[0], s[1]))
        .then(M::t(-cs[0], -cs[1], 0.0))
}

/// The whole element, with its channels and keys (a plain read reports channels as stubs).
fn full(h: &ExecutorHandle, node: &str) -> Value {
    ok(h, "doc.object.get", json!({ "ref": node, "depth": 3 }))["object"].clone()
}

/// A copy without the revision: undo and redo always move revisions forward (10 REV-01).
fn sans_rev(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("rev");
    v
}

fn eval(h: &ExecutorHandle, node: &str, frame: f64) -> Value {
    ok(h, "anim.evaluate", json!({ "nodeIds": [node], "frames": [frame] }))["frames"][0]["nodes"][0].clone()
}

fn assert_matrix(got: &Value, want: [f64; 16], what: &str) {
    let got: Vec<f64> = got.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    for i in 0..16 {
        assert!((got[i] - want[i]).abs() <= 1e-6, "{what}: entry {i}: {got:?} vs {want:?}");
    }
}

fn near(v: &Value, want: f64, what: &str) {
    let got = v.as_f64().unwrap_or(f64::NAN);
    assert!((got - want).abs() <= 1e-6, "{what}: {got} vs {want}");
}

/// A fresh drawing element with its own bar-independent channels.
fn drawing(h: &ExecutorHandle, name: &str) -> String {
    ok(h, "element.create", json!({ "type": "drawing", "name": name }))["elementIds"][0].as_str().unwrap().to_owned()
}

fn peg(h: &ExecutorHandle, name: &str) -> String {
    ok(h, "element.create", json!({ "type": "peg", "name": name }))["elementIds"][0].as_str().unwrap().to_owned()
}

fn attach(h: &ExecutorHandle, child: &str, parent: &str) {
    ok(h, "element.parent.set", json!({ "elementIds": [child], "parentId": parent }));
}

fn patch(h: &ExecutorHandle, target: &str, props: Value) {
    ok(h, "doc.object.patch", json!({ "targets": [target], "props": props }));
}

// ----- transforms -----------------------------------------------------------------------------------------------------

#[test]
fn composition_through_nested_pegs_matches_the_independent_formula() {
    let h = fixture();
    let (master, arm) = (peg(&h, "Master"), peg(&h, "ArmPeg"));
    let part = drawing(&h, "Part");
    attach(&h, &arm, &master);
    attach(&h, &part, &arm);
    // Master: a mirror with a pivot; arm: skew and non-uniform scale; part: rotation pivot and scale pivot.
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [master], "target": "static", "position": { "x": 3, "y": -2, "z": 1 }, "rotation": 30, "scale": { "x": -1.5, "y": 2 }, "pivot": { "x": 1, "y": 1 } }),
    );
    patch(&h, &master, json!({ "transform": { "applyPivotTo": "both" } }));
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [arm], "target": "static", "position": { "x": -1, "y": 0.5, "z": 0.25 }, "skew": 20, "scale": { "x": 0.5, "y": 3 } }),
    );
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [part], "target": "static", "position": { "x": 0.25, "y": 4, "z": -0.5 }, "rotation": -75, "skew": -10, "scale": { "x": 2, "y": -0.75 }, "rotationPivot": { "x": 0.5, "y": -0.5 }, "scalePivot": { "x": -1, "y": 2 } }),
    );
    patch(&h, &part, json!({ "transform": { "rotateAboutPivot": true } }));
    let wm = local([3.0, -2.0, 1.0], 30.0, 0.0, [-1.5, 2.0], [1.0, 1.0], [1.0, 1.0]);
    let wa = wm.then(local([-1.0, 0.5, 0.25], 0.0, 20.0, [0.5, 3.0], [0.0, 0.0], [0.0, 0.0]));
    let wp = wa.then(local([0.25, 4.0, -0.5], -75.0, -10.0, [2.0, -0.75], [0.5, -0.5], [-1.0, 2.0]));
    let m = eval(&h, &master, 1.0);
    assert_matrix(&m["world"]["matrix"], wm.rows(), "master");
    let a = eval(&h, &arm, 1.0);
    assert_matrix(&a["world"]["matrix"], wa.rows(), "arm");
    let p = eval(&h, &part, 1.0);
    assert_matrix(&p["world"]["matrix"], wp.rows(), "part");
    assert_matrix(&p["local"]["matrix"], local([0.25, 4.0, -0.5], -75.0, -10.0, [2.0, -0.75], [0.5, -0.5], [-1.0, 2.0]).rows(), "local");
    // Mirrored ancestors make the determinant negative: the decomposition reports a negative sy (05 §2.5).
    let r = wp.0;
    assert!(r[0] * r[3] - r[1] * r[2] > 0.0, "two mirrors: positive");
    assert!(p["world"]["scale"]["y"].as_f64().unwrap() > 0.0);
    let [a0, b0, c0, d0, _, _] = wm.0;
    assert!(a0 * d0 - b0 * c0 < 0.0, "one mirror: negative");
    assert!(m["world"]["scale"]["y"].as_f64().unwrap() < 0.0, "{}", m["world"]);
    near(&m["world"]["scale"]["x"], 1.5, "sx");
    near(&m["world"]["position"]["z"], 1.0, "z");
    near(&p["world"]["position"]["z"], 0.75, "z adds through the hierarchy");
}

#[test]
fn world_position_targets_invert_the_parent_and_static_plus_animated_combine() {
    let h = fixture();
    let parent = peg(&h, "Parent");
    let child = drawing(&h, "Child");
    attach(&h, &child, &parent);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [parent], "target": "static", "position": { "x": 5, "y": 1, "z": 2 }, "rotation": 90, "scale": { "x": 2, "y": 2 } }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [child], "target": "static", "position": { "x": 1, "y": 0, "z": 0 } }));
    // A world target: world (3, 3, 2) ← parent-space P with W(parent)·P; (3, 3) − (5, 1) = (−2, 2); R(−90)/2 → (1, 1).
    ok(&h, "anim.transform.set", json!({ "nodeIds": [child], "frame": 1, "position": { "x": 3, "y": 3, "z": 2 }, "positionSpace": "world" }));
    let e = eval(&h, &child, 1.0);
    near(&e["world"]["position"]["x"], 3.0, "world x");
    near(&e["world"]["position"]["y"], 3.0, "world y");
    near(&e["world"]["position"]["z"], 2.0, "world z");
    // Effective = static + animated, so the key holds the difference.
    near(&e["animated"]["position"]["x"], 0.0, "animated x");
    near(&e["animated"]["position"]["y"], 1.0, "animated y");
    near(&e["local"]["position"]["x"], 1.0, "effective x");
    // Rotation adds, scale multiplies, skew adds and clamps to ±89 (05 §1.5).
    ok(&h, "anim.transform.set", json!({ "nodeIds": [child], "target": "static", "rotation": 10, "scale": { "x": 2, "y": 3 }, "skew": 80 }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [child], "frame": 1, "rotation": 5, "scale": { "x": 4, "y": -1 }, "skew": 20 }));
    let e = eval(&h, &child, 1.0);
    near(&e["local"]["rotation"], 15.0, "rotation");
    near(&e["local"]["scale"]["x"], 8.0, "scale x");
    near(&e["local"]["scale"]["y"], -3.0, "scale y");
    near(&e["local"]["skew"], 89.0, "skew clamps");
    // `relative` adds and multiplies at the frame, for both layers.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [child], "frame": 1, "relative": true, "rotation": 1, "scale": { "x": 0.5 } }));
    let e = eval(&h, &child, 1.0);
    near(&e["animated"]["rotation"], 6.0, "relative rotation");
    near(&e["animated"]["scale"]["x"], 2.0, "relative scale");
}

#[test]
fn pivots_follow_05_2_3_per_node_kind() {
    let h = fixture();
    let p = peg(&h, "Pivoted");
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "target": "static", "rotation": 90, "scale": { "x": 2, "y": 1 }, "pivot": { "x": 1, "y": 0 } }));
    // Default: rotation only.
    let cases = [("rotation", [1.0, 0.0], [0.0, 0.0]), ("scaling", [0.0, 0.0], [1.0, 0.0]), ("both", [1.0, 0.0], [1.0, 0.0])];
    for (mode, cr, cs) in cases {
        patch(&h, &p, json!({ "transform": { "applyPivotTo": mode } }));
        let e = eval(&h, &p, 1.0);
        near(&e["local"]["rotationPivot"]["x"], cr[0], mode);
        near(&e["local"]["scalePivot"]["x"], cs[0], mode);
        assert_matrix(&e["local"]["matrix"], local([0.0; 3], 90.0, 0.0, [2.0, 1.0], cr, cs).rows(), mode);
    }
    // An animated pivot adds to the static one.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "frame": 1, "pivot": { "x": 0.5, "y": 2 } }));
    let e = eval(&h, &p, 1.0);
    near(&e["local"]["rotationPivot"]["x"], 1.5, "animated pivot");
    near(&e["local"]["rotationPivot"]["y"], 2.0, "animated pivot");
    near(&e["animated"]["pivot"]["y"], 2.0, "animated pivot");
    // Element: the rotation pivot applies only with *use rotation pivot*; otherwise cr = cs.
    let d = drawing(&h, "Art");
    ok(&h, "anim.transform.set", json!({ "nodeIds": [d], "target": "static", "rotationPivot": { "x": 5, "y": 5 }, "scalePivot": { "x": 1, "y": 2 } }));
    let e = eval(&h, &d, 1.0);
    near(&e["local"]["rotationPivot"]["x"], 1.0, "cr = cs");
    near(&e["local"]["scalePivot"]["y"], 2.0, "cs");
    patch(&h, &d, json!({ "transform": { "rotateAboutPivot": true } }));
    near(&eval(&h, &d, 1.0)["local"]["rotationPivot"]["x"], 5.0, "cr");
    // Use child drawing pivot: only with exactly one drawing child; pivot = child's scale pivot + child's X/Y offset.
    let q = peg(&h, "Holder");
    attach(&h, &d, &q);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [d], "target": "static", "position": { "x": 10, "y": 20, "z": 3 } }));
    patch(&h, &q, json!({ "transform": { "useChildDrawingPivot": true } }));
    let e = eval(&h, &q, 1.0);
    near(&e["local"]["rotationPivot"]["x"], 11.0, "child pivot x");
    near(&e["local"]["rotationPivot"]["y"], 22.0, "child pivot y");
    let other = drawing(&h, "Second");
    attach(&h, &other, &q);
    near(&eval(&h, &q, 1.0)["local"]["rotationPivot"]["x"], 0.0, "two children: no effect");
    // Cameras have no pivots; asking for one is refused, not ignored.
    let e = err(&h, "anim.transform.set", json!({ "nodeIds": [CAMERA], "target": "static", "pivot": { "x": 1, "y": 1 } }));
    assert_eq!(e.code, "anim.channelUnsupported");
    let e = err(&h, "anim.transform.set", json!({ "nodeIds": [CAMERA], "target": "static", "scale": { "x": 2 } }));
    assert_eq!(e.code, "anim.channelUnsupported");
    let e = err(&h, "anim.transform.set", json!({ "nodeIds": [d], "target": "static", "pivot": { "x": 1 } }));
    assert_eq!(e.code, "anim.channelUnsupported", "elements have rotationPivot and scalePivot");
    let e = err(&h, "anim.transform.set", json!({ "nodeIds": [d], "frame": 1, "scalePivot": { "x": 1 } }));
    assert_eq!(e.code, "anim.channelUnsupported", "element pivots are static");
}

#[test]
fn transform_edits_leave_drawings_and_their_noding_alone_and_survive_save_open() {
    let h = fixture();
    let d1 = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
    let drawing_before = ok(&h, "draw.drawing.get", json!({ "drawingId": d1 }));
    let rev_before = get(&h, d1)["rev"].clone();
    ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "rotation": 12, "scalePivot": { "x": 1, "y": 1 } }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [PEG], "frame": 5, "rotation": 30, "scale": { "x": 2, "y": 2 } }));
    assert_eq!(sans_rev(ok(&h, "draw.drawing.get", json!({ "drawingId": d1 }))), sans_rev(drawing_before.clone()));
    assert_eq!(get(&h, d1)["rev"], rev_before);
    let before = eval(&h, BOB, 5.0);
    let dir = tmp();
    let out = dir.join("transforms.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": out }));
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    job_ok(&h, "project.open", json!({ "path": out }));
    assert_eq!(eval(&h, BOB, 5.0), before, "byte-identical after save and open");
    assert_eq!(sans_rev(ok(&h, "draw.drawing.get", json!({ "drawingId": d1 }))), sans_rev(drawing_before));
}

#[test]
fn drawing_bounds_use_1200_du_for_twelve_fields() {
    let h = fixture();
    let d = drawing(&h, "Square");
    let cell = ok(&h, "element.drawing.create", json!({ "elementId": d, "name": "s", "exposeFrom": 1, "exposeTo": 4 }));
    let did = cell["drawingId"].as_str().unwrap().to_owned();
    // A rectangle of 600 × 300 du centred on (100, 0): 6 × 3 fields.
    ok(&h, "draw.rectangle.add", json!({ "drawingId": did, "from": { "x": -200, "y": -150 }, "to": { "x": 400, "y": 150 } }));
    ok(&h, "anim.transform.set", json!({ "nodeIds": [d], "target": "static", "position": { "x": 1, "y": 2, "z": 0 }, "scale": { "x": 2, "y": 1 } }));
    let e = eval(&h, &d, 1.0);
    let b = &e["bounds"];
    // x ∈ (−200, 400) du = (−2, 4) fields, scaled ×2, + 1; y ∈ (−150, 150) du = (−1.5, 1.5), + 2. A stroke width may pad.
    assert!((b["minX"].as_f64().unwrap() - -3.0).abs() < 0.2, "{b}");
    assert!((b["maxX"].as_f64().unwrap() - 9.0).abs() < 0.2, "{b}");
    assert!((b["minY"].as_f64().unwrap() - 0.5).abs() < 0.2, "{b}");
    assert!((b["maxY"].as_f64().unwrap() - 3.5).abs() < 0.2, "{b}");
    near(&b["z"], 0.0, "z");
    // A blank cell has no content at that frame.
    assert!(eval(&h, &d, 40.0).get("bounds").is_none());
    assert_eq!(eval(&h, &d, 40.0)["screen"]["visible"], false);
    // Hidden and annotation: transforms still evaluate, not visible on screen.
    assert_eq!(eval(&h, &d, 1.0)["screen"]["visible"], true);
    ok(&h, "element.visibility.set", json!({ "elementIds": [d], "scope": "scene", "visible": false }));
    let hidden = eval(&h, &d, 1.0);
    assert_eq!(hidden["screen"]["visible"], false);
    assert!(hidden["world"]["matrix"].is_array());
    ok(&h, "element.visibility.set", json!({ "elementIds": [d], "scope": "scene", "visible": true }));
    ok(&h, "element.annotation.set", json!({ "elementId": d, "annotation": true }));
    assert_eq!(eval(&h, &d, 1.0)["screen"]["visible"], false);
    assert_eq!(eval(&h, NOTES, 1.0)["screen"]["visible"], false);
}

// ----- channels -------------------------------------------------------------------------------------------------------

#[test]
fn channels_hold_step_and_report_defaults_in_scene_frames() {
    let h = fixture();
    let p = peg(&h, "Timed");
    // The bar starts at frame 5: frames are scene frames, keys are stored local (05 §19.1.4, §1.6).
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "start": 5, "duration": 20 }));
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [{ "frame": 10, "value": 10 }, { "frame": 20, "value": 30 }] }));
    assert_eq!(full(&h, &p)["channels"]["rotation"]["keys"][0]["frame"], 6, "stored local");
    let f = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "rotation" }));
    assert_eq!(f["keys"][0]["frame"], 10.0, "reported in scene frames");
    near(&eval(&h, &p, 1.0)["animated"]["rotation"], 10.0, "held before the bar");
    near(&eval(&h, &p, 10.0)["animated"]["rotation"], 10.0, "exact at a key");
    near(&eval(&h, &p, 15.0)["animated"]["rotation"], 20.0, "linear");
    near(&eval(&h, &p, 12.5)["animated"]["rotation"], 15.0, "fractional");
    near(&eval(&h, &p, 99.0)["animated"]["rotation"], 30.0, "held after the bar");
    assert_eq!(eval(&h, &p, 12.5)["localFrame"], 8.5);
    // A missing channel is its default; a channel's value is independent of the others.
    near(&eval(&h, &p, 15.0)["animated"]["scale"]["x"], 1.0, "default scale");
    near(&eval(&h, &p, 15.0)["animated"]["skew"], 0.0, "default skew");
    // Keys outside the bar are refused; a fractional key frame has its own code.
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "rotation", "frame": 4 })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "rotation", "frame": 25 })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "rotation", "frame": 7.5 })).code, "anim.frameNotInteger");
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "rotation", "frame": 10 })).code, "anim.keyExists");
    // A constant segment holds, then jumps.
    ok(&h, "anim.segment.set", json!({ "nodeIds": [p], "frame": 10, "constant": true }));
    near(&eval(&h, &p, 19.0)["animated"]["rotation"], 10.0, "constant");
    near(&eval(&h, &p, 20.0)["animated"]["rotation"], 30.0, "jump");
    ok(&h, "anim.segment.set", json!({ "nodeIds": [p], "frame": 10, "channels": ["rotation"], "constant": false }));
    near(&eval(&h, &p, 15.0)["animated"]["rotation"], 20.0, "tweened again");
    // Bounds.
    for (channel, value) in [("skew", 90.0), ("scale.x", 0.0005), ("scale.y", 2e6), ("rotation", 2e9), ("pivot.x", 2e6)] {
        let e = err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": channel, "frame": 8, "value": value }));
        assert_eq!(e.code, "anim.valueOutOfRange", "{channel}");
        assert!(e.details.get("minimum").is_some() && e.details.get("maximum").is_some(), "{e:?}");
    }
    assert_eq!(err(&h, "anim.function.get", json!({ "nodeId": p, "channel": "fov" })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.get", json!({ "nodeId": p, "channel": "position" })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.set", json!({ "nodeId": CAMERA, "channel": "skew", "keys": [] })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.get", json!({ "nodeId": "element:@no", "channel": "rotation" })).code, "anim.notFound");
}

#[test]
fn channel_and_key_ids_are_stable_and_handles_follow_the_spec() {
    let h = fixture();
    let p = peg(&h, "Ids");
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "curveType": "spline", "keys": [{ "frame": 2, "value": 0 }, { "frame": 10, "value": 8 }, { "frame": 18, "value": 0 }] }));
    let c = full(&h, &p)["channels"]["rotation"].clone();
    let (cid, k2) = (c["id"].clone(), c["keys"][1]["id"].clone());
    ok(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "rotation", "frame": 10, "newFrame": 12, "value": 9 }));
    ok(&h, "anim.key.move", json!({ "nodeId": p, "fromFrame": 12, "toFrame": 11 }));
    let c = full(&h, &p)["channels"]["rotation"].clone();
    assert_eq!(c["id"], cid);
    assert_eq!(c["keys"][1]["id"], k2);
    assert_eq!(c["keys"][1]["frame"], 11);
    assert_eq!(c["keys"][1]["value"], 9.0);
    // Crossing a neighbour or landing on one is refused, atomically.
    assert_eq!(err(&h, "anim.key.move", json!({ "nodeId": p, "fromFrame": 11, "toFrame": 19 })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.key.move", json!({ "nodeId": p, "fromFrame": 11, "toFrame": 18 })).code, "anim.keyExists");
    // A right handle turns a smooth left one, keeping its length; broken handles are independent; frame offsets clamp.
    ok(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "rotation", "frame": 11, "left": { "dt": -3, "dv": 0 } }));
    ok(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "rotation", "frame": 11, "right": { "dt": 0, "dv": 2 } }));
    let k = &full(&h, &p)["channels"]["rotation"]["keys"][1];
    assert_eq!(k["handles"]["left"]["dv"], -3.0, "collinear, length kept");
    assert_eq!(k["handles"]["left"]["dt"], 0.0);
    ok(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "rotation", "frame": 11, "handleMode": "broken", "right": { "dt": 100, "dv": 1 } }));
    let k = &full(&h, &p)["channels"]["rotation"]["keys"][1];
    assert_eq!(k["handles"]["right"]["dt"], 7.0, "clamped to the next key");
    assert_eq!(k["handles"]["left"]["dv"], -3.0, "broken: the left handle stays");
    // Copies get new IDs for keys and channels (C3.1's remap) with the same values.
    let copy = ok(&h, "element.duplicate", json!({ "elementIds": [p] }))["elementIds"][0].as_str().unwrap().to_owned();
    let cc = full(&h, &copy)["channels"]["rotation"].clone();
    assert_ne!(cc["id"], cid);
    assert_eq!(cc["keys"][1]["frame"], 11);
    // Presets on a spline set the normalised handles (05 §10.4); on interpolation the segment preset.
    ok(&h, "anim.function.preset.apply", json!({ "nodeId": p, "channel": "rotation", "preset": "easeInOut", "frame": 2 }));
    let keys = full(&h, &p)["channels"]["rotation"]["keys"].clone();
    assert_eq!(keys[0]["handles"]["right"], json!({ "dt": 3.0, "dv": 0.0 }));
    near(&keys[1]["handles"]["left"]["dt"], -3.0, "⅔ of the segment back");
    assert_eq!(keys[1]["handles"]["left"]["dv"], 0.0);
    ok(&h, "anim.function.curveType.set", json!({ "nodeId": p, "channel": "rotation", "curveType": "interpolation" }));
    let r = ok(&h, "anim.function.preset.apply", json!({ "nodeId": p, "channel": "rotation", "preset": "fastInOut", "scope": "all" }));
    assert_eq!(r["segments"], 2);
    assert_eq!(full(&h, &p)["channels"]["rotation"]["keys"][0]["easing"], "fastInOut");
    // `constant` is a preset that sets the flag; velocityBased needs a path.
    ok(&h, "anim.function.preset.apply", json!({ "nodeId": p, "channel": "rotation", "preset": "constant", "frame": 2 }));
    assert_eq!(full(&h, &p)["channels"]["rotation"]["keys"][0]["constantSegment"], true);
    assert_eq!(err(&h, "anim.function.curveType.set", json!({ "nodeId": p, "channel": "rotation", "curveType": "velocityBased" })).code, "anim.curveTypeUnsupported");
    assert_eq!(err(&h, "anim.function.curveType.set", json!({ "nodeId": p, "channel": "pivot.x", "curveType": "velocityBased" })).code, "anim.curveTypeUnsupported");
    // Duplicate and remove.
    ok(&h, "anim.function.key.duplicate", json!({ "nodeId": p, "channel": "rotation", "frame": 2, "toFrame": 5 }));
    let r = ok(&h, "anim.function.key.remove", json!({ "nodeId": p, "channel": "rotation", "frames": [5, 18] }));
    assert_eq!(r["removed"], 2);
    assert_eq!(err(&h, "anim.function.key.remove", json!({ "nodeId": p, "channel": "rotation", "frames": [3] })).code, "anim.frameInvalid");
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [] }));
    assert!(full(&h, &p)["channels"].get("rotation").is_none(), "a channel exists only with keys (10 §4.12)");
}

// ----- paths and velocity ---------------------------------------------------------------------------------------------

/// A peg with the path (−4, 0) @1 → (0, 3) control → (4, 0) @25.
fn walker(h: &ExecutorHandle) -> String {
    let p = peg(h, "Walker");
    ok(
        h,
        "anim.path.set",
        json!({ "nodeId": p, "points": [
            { "kind": "key", "frame": 1, "x": -4, "y": 0, "z": 0 },
            { "kind": "control", "x": 0, "y": 3, "z": 0 },
            { "kind": "key", "frame": 25, "x": 4, "y": 0, "z": 0 }
        ] }),
    );
    p
}

#[test]
fn a_path_reports_arc_lengths_passage_frames_and_exact_keyframes() {
    let h = fixture();
    let p = walker(&h);
    let got = ok(&h, "anim.path.get", json!({ "nodeId": p }));
    let pts = got["points"].as_array().unwrap();
    assert_eq!(pts.len(), 3);
    assert_eq!(pts[0]["kind"], "key");
    assert_eq!(pts[1]["kind"], "control");
    assert_eq!(pts[0]["arcLength"], 0.0);
    // The symmetric path: the control point is half way along it, so linear velocity passes it at frame 13.
    let length = got["length"].as_f64().unwrap();
    near(&pts[1]["arcLength"], length / 2.0, "symmetric arc");
    assert_eq!(pts[1]["frame"], 13.0);
    assert!(length > 10.0 && length < 11.0, "{length}");
    // Keyframes are exact; the middle passes through the control point.
    assert_eq!(eval(&h, &p, 1.0)["animated"]["position"], json!({ "x": -4.0, "y": 0.0, "z": 0.0 }));
    assert_eq!(eval(&h, &p, 25.0)["animated"]["position"], json!({ "x": 4.0, "y": 0.0, "z": 0.0 }));
    let mid = eval(&h, &p, 13.0);
    near(&mid["animated"]["position"]["x"], 0.0, "mid x");
    near(&mid["animated"]["position"]["y"], 3.0, "mid y");
    near(&mid["animated"]["velocity"], length / 2.0, "V");
    // Samples: every frame on the path in parent or world space, at any step.
    let s = ok(&h, "anim.path.sample", json!({ "nodeId": p, "from": 1, "to": 25, "step": 12, "space": "parent" }));
    assert_eq!(s["samples"].as_array().unwrap().len(), 3);
    assert_eq!(s["samples"][2]["x"], 4.0);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [p], "target": "static", "position": { "x": 100, "y": 0, "z": 0 } }));
    let s = ok(&h, "anim.path.sample", json!({ "nodeId": p, "from": 1, "to": 1 }));
    assert_eq!(s["samples"][0]["x"], 96.0, "static + animated");
    assert_eq!(err(&h, "anim.path.sample", json!({ "nodeId": p, "step": 0 })).code, "anim.valueOutOfRange");
    assert_eq!(err(&h, "anim.path.sample", json!({ "nodeId": p, "from": 5, "to": 2 })).code, "anim.frameInvalid");
    // Projections read the keyframes and sample the path.
    let f = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "position.x" }));
    assert_eq!(f["keys"].as_array().unwrap().len(), 2);
    let v = ok(&h, "anim.function.sample", json!({ "nodeId": p, "channel": "position.y", "from": 13, "to": 13 }));
    near(&v["samples"][0]["value"], 3.0, "projection");
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "position.x", "frame": 5 })).code, "anim.channelUnsupported");
}

#[test]
fn motion_points_add_move_lock_and_remove_with_the_velocity_channel() {
    let h = fixture();
    let p = peg(&h, "Line");
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": 21, "x": 20, "y": 0, "z": 0 } ] }));
    // A key on the line, at the position the node occupies.
    let add = ok(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "key", "frame": 6 }));
    let pid = add["pointId"].as_str().unwrap().to_owned();
    assert_eq!(add["frame"], 6.0);
    let pts = ok(&h, "anim.path.get", json!({ "nodeId": p }))["points"].clone();
    near(&pts[1]["x"], 5.0, "inserted where the node is");
    assert_eq!(err(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "key", "frame": 6 })).code, "anim.keyExists");
    assert_eq!(err(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "key", "frame": 6.5 })).code, "anim.frameNotInteger");
    // Before the first or after the last: a new end point.
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 40 }));
    ok(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "key", "frame": 30, "position": { "x": 30, "y": 0, "z": 0 } }));
    let pts = ok(&h, "anim.path.get", json!({ "nodeId": p }))["points"].clone();
    assert_eq!(pts.as_array().unwrap().last().unwrap()["frame"], 30.0);
    // A control point by frame and by curve location; its frame is derived.
    let c = ok(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "control", "frame": 11.5 }));
    assert!((c["frame"].as_f64().unwrap() - 11.5).abs() < 0.01, "{c}");
    let c2 = ok(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "control", "at": { "segment": 0, "s": 0.5 } }));
    let c2id = c2["pointId"].as_str().unwrap().to_owned();
    assert_eq!(err(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "control", "frame": 35 })).code, "anim.frameInvalid");
    // Edits: position, TCB, frame, constant, per-key rotation; bounds refused, nothing changed.
    let rev0 = rev(&h);
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [pid], "tension": 1.5 })).code, "anim.valueOutOfRange");
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [pid], "frame": 2.5 })).code, "anim.frameNotInteger");
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [pid], "frame": 1 })).code, "anim.keyExists");
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [pid], "frame": 25 })).code, "anim.frameInvalid");
    assert_eq!(rev(&h), rev0);
    ok(&h, "anim.path.point.set", json!({ "pointIds": [pid], "y": 2, "tension": 0.5, "frame": 7, "rotation": 45, "scale": { "x": 2 } }));
    let e = eval(&h, &p, 7.0);
    near(&e["animated"]["position"]["y"], 2.0, "moved");
    near(&e["animated"]["rotation"], 45.0, "per-key rotation");
    near(&e["animated"]["scale"]["x"], 2.0, "per-key scale");
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [c2id], "rotation": 45 })).code, "anim.frameInvalid");
    // Lock in time: a control point becomes a keyframe at round(derived); the ends cannot be unlocked.
    let pts = ok(&h, "anim.path.get", json!({ "nodeId": p }))["points"].clone();
    let last = pts.as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_owned();
    assert_eq!(err(&h, "anim.path.point.set", json!({ "pointIds": [last], "lockInTime": false })).code, "anim.pathEndKey");
    ok(&h, "anim.path.point.set", json!({ "pointIds": [c2id], "lockInTime": true }));
    let got = ok(&h, "anim.path.get", json!({ "nodeId": p }));
    let locked = got["points"].as_array().unwrap().iter().find(|q| q["id"] == json!(c2id)).unwrap().clone();
    assert_eq!(locked["kind"], "key");
    assert_eq!(locked["frame"].as_f64().unwrap().fract(), 0.0);
    // Unlock it again: the point stays on the path as a control point; its pin turned into a velocity key.
    ok(&h, "anim.path.point.set", json!({ "pointIds": [c2id], "lockInTime": false }));
    let got = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "velocity" }));
    assert_eq!(got["keys"].as_array().unwrap().len(), 1, "{got}");
    // Removing the last keyframe: the control points before it are converted (a warning, never a block).
    let path = ok(&h, "anim.path.get", json!({ "nodeId": p }));
    let ids: Vec<String> = path["points"].as_array().unwrap().iter().map(|q| q["id"].as_str().unwrap().to_owned()).collect();
    let env = run(&h, "anim.path.point.remove", json!({ "pointIds": [ids.last().unwrap()] }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    // Removing every remaining keyframe removes the animation entirely.
    let path = ok(&h, "anim.path.get", json!({ "nodeId": p }));
    let ids: Vec<String> = path["points"].as_array().unwrap().iter().map(|q| q["id"].as_str().unwrap().to_owned()).collect();
    ok(&h, "anim.path.point.remove", json!({ "pointIds": ids }));
    assert!(full(&h, &p)["channels"].get("position").is_none());
    assert!(full(&h, &p)["channels"].get("velocity").is_none());
    assert_eq!(eval(&h, &p, 1.0)["animated"]["position"], json!({ "x": 0.0, "y": 0.0, "z": 0.0 }));
}

#[test]
fn removing_a_pin_preserves_timing_and_end_control_points_convert_with_a_warning() {
    let h = fixture();
    let p = peg(&h, "Pins");
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 },
        { "kind": "key", "frame": 11, "x": 10, "y": 0, "z": 0 },
        { "kind": "key", "frame": 21, "x": 20, "y": 0, "z": 0 } ] }));
    // A velocity key at frame 6 in fields, half of the first segment; `value` round trips through its normalised form.
    ok(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "velocity", "frame": 6, "value": 2.0 }));
    assert_eq!(full(&h, &p)["channels"]["velocity"]["keys"][0]["value"], 0.2, "stored normalised");
    let f = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "velocity" }));
    near(&f["keys"][0]["value"], 2.0, "reported in fields");
    assert_eq!(f["pins"].as_array().unwrap().len(), 3);
    near(&f["pins"][1]["value"], 10.0, "pin");
    near(&eval(&h, &p, 6.0)["animated"]["position"]["x"], 2.0, "slowed");
    // Pins cannot be changed directly.
    assert_eq!(err(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "velocity", "frame": 11, "value": 3 })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.key.remove", json!({ "nodeId": p, "channel": "velocity", "frames": [11] })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "velocity", "frame": 11 })).code, "anim.keyExists");
    assert_eq!(err(&h, "anim.function.key.set", json!({ "nodeId": p, "channel": "velocity", "frame": 6, "constantSegment": true })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.segment.set", json!({ "nodeIds": [p], "frame": 1, "channels": ["velocity"], "constant": true })).code, "anim.channelUnsupported");
    assert_eq!(err(&h, "anim.function.preset.apply", json!({ "nodeId": p, "channel": "velocity", "preset": "constant", "frame": 1 })).code, "anim.curveTypeUnsupported");
    // Remove the middle keyframe: its pin becomes an ordinary key at its normalised position (05 §10.5), and the node's
    // arc lengths at the kept frames are the same, so the timing is preserved.
    let before: Vec<f64> = [3.0, 6.0, 8.0, 11.0, 16.0].iter().map(|f| eval(&h, &p, *f)["animated"]["velocity"].as_f64().unwrap()).collect();
    ok(&h, "anim.key.remove", json!({ "nodeIds": [p], "frame": 11, "channels": ["position"] }));
    let f = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "velocity" }));
    assert_eq!(f["pins"].as_array().unwrap().len(), 2);
    assert_eq!(f["keys"].as_array().unwrap().len(), 2, "{f}");
    near(&f["keys"][1]["value"], 10.0, "the pin's arc, now a key");
    for (frame, want) in [3.0, 6.0, 8.0, 11.0, 16.0].iter().zip(before) {
        near(&eval(&h, &p, *frame)["animated"]["velocity"], want, "velocity after the merge");
    }
    // Inserting a pin splits the same timing back.
    ok(&h, "anim.path.point.add", json!({ "nodeId": p, "kind": "key", "frame": 11 }));
    near(&eval(&h, &p, 6.0)["animated"]["velocity"], 2.0, "after the split");
    near(&eval(&h, &p, 11.0)["animated"]["position"]["x"], 10.0, "the pin");
    // Uniform: constant speed across the segment.
    ok(&h, "anim.segment.uniform.apply", json!({ "nodeId": p, "frame": 1, "side": "right", "channels": ["velocity"] }));
    near(&eval(&h, &p, 6.0)["animated"]["velocity"], 5.0, "uniform");
    // A path whose end is left beyond the last keyframe converts it, reporting a warning.
    let env = run(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "control", "x": 5, "y": 0, "z": 0 } ] }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert!(env.warnings.iter().any(|w| w.code == "anim.pathEndConverted"), "{env:?}");
    assert_eq!(ok(&h, "anim.path.get", json!({ "nodeId": p }))["points"][1]["kind"], "key");
    // Keyframes out of order, duplicates and frames outside the bar are refused as a whole.
    let path = |frames: [f64; 2]| json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": frames[0], "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": frames[1], "x": 1, "y": 0, "z": 0 } ] });
    let rev0 = rev(&h);
    assert_eq!(err(&h, "anim.path.set", path([5.0, 2.0])).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.path.set", path([5.0, 5.0])).code, "anim.keyExists");
    assert_eq!(err(&h, "anim.path.set", path([1.5, 5.0])).code, "anim.frameNotInteger");
    assert_eq!(err(&h, "anim.path.set", path([1.0, 500_000.0])).code, "anim.frameInvalid");
    assert_eq!(rev(&h), rev0);
}

#[test]
fn constant_segments_constant_z_and_orient_to_path() {
    let h = fixture();
    let p = peg(&h, "Depth");
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 2, "constantSegment": true },
        { "kind": "key", "frame": 5, "x": 4, "y": 0, "z": 2 },
        { "kind": "key", "frame": 9, "x": 4, "y": 4, "z": 10 } ] }));
    assert_eq!(eval(&h, &p, 4.5)["animated"]["position"]["x"], 0.0, "a constant segment holds");
    assert_eq!(eval(&h, &p, 5.0)["animated"]["position"]["x"], 4.0, "then jumps");
    let z = eval(&h, &p, 7.0)["animated"]["position"]["z"].as_f64().unwrap();
    assert!(z > 2.0 && z < 10.0, "tweened Z: {z}");
    ok(&h, "anim.path.lockDepth.set", json!({ "nodeId": p, "frame": 5, "enabled": true }));
    assert_eq!(eval(&h, &p, 7.0)["animated"]["position"]["z"], 2.0, "Z holds from the segment start");
    assert_eq!(eval(&h, &p, 9.0)["animated"]["position"]["z"], 10.0, "and steps at the next key");
    assert_eq!(err(&h, "anim.path.lockDepth.set", json!({ "nodeId": p, "frame": 6, "enabled": true })).code, "anim.frameInvalid");
    // Orient to path: along +Y on the last segment; on the constant one the search finds a valid angle.
    patch(&h, &p, json!({ "transform": { "orientToPath": true } }));
    let e = eval(&h, &p, 7.0);
    let angle = e["animated"]["orientAngle"].as_f64().unwrap();
    // Dout(1) = ½(4, 0) + ½(0, 4) = (2, 2) and Din(2) = (0, 4): the curve bulges past x = 4 at mid-segment, so the
    // tangent there leans just past straight up (x′(½) = −½, y′(½) = 4½ at the parameter mid-point).
    assert!((90.0..100.0).contains(&angle), "{angle}");
    near(&e["local"]["rotation"], angle, "the orient angle adds to rotation");
    let far = peg(&h, "Up");
    ok(&h, "anim.path.set", json!({ "nodeId": far, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": 9, "x": 0, "y": 8, "z": 0 } ] }));
    patch(&h, &far, json!({ "transform": { "orientToPath": true } }));
    near(&eval(&h, &far, 5.0)["animated"]["orientAngle"], 90.0, "straight up");
    near(&eval(&h, &far, 1.0)["animated"]["orientAngle"], 90.0, "at the first keyframe");
    near(&eval(&h, &far, 40.0)["animated"]["orientAngle"], 90.0, "held after the last");
}

#[test]
fn velocity_based_curves_follow_the_movement() {
    let h = fixture();
    let p = peg(&h, "Follows");
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": 11, "x": 10, "y": 0, "z": 0 } ] }));
    ok(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "velocity", "frame": 6, "value": 8.0 }));
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "curveType": "velocityBased", "keys": [{ "frame": 1, "value": 0 }, { "frame": 11, "value": 100 }] }));
    near(&eval(&h, &p, 6.0)["animated"]["rotation"], 80.0, "progress follows V, not time");
}

// ----- SI-089 and loops -----------------------------------------------------------------------------------------------

fn keys_of(h: &ExecutorHandle, node: &str, channel: &str) -> Vec<(u64, f64)> {
    full(h, node)["channels"][channel]["keys"]
        .as_array()
        .map(|a| a.iter().map(|k| (k["frame"].as_u64().unwrap(), k["value"].as_f64().unwrap())).collect())
        .unwrap_or_default()
}

#[test]
fn si089_truncation_holds_the_original_curve_value_at_the_new_end() {
    let h = fixture();
    let p = peg(&h, "Truncated");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 40 }));
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [
        { "frame": 1, "value": 0 }, { "frame": 11, "value": 100 }, { "frame": 31, "value": 300 } ] }));
    // An eased curve: the value at the new end is the original curve's, not the line between kept keys.
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "skew", "keys": [
        { "frame": 1, "value": 0, "preset": "easeInOut" }, { "frame": 21, "value": 40 } ] }));
    // A channel with every key beyond the new end holds its first value there.
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "pivot.x", "keys": [{ "frame": 30, "value": 7 }] }));
    let want_rotation = eval(&h, &p, 16.0)["animated"]["rotation"].as_f64().unwrap();
    assert_eq!(want_rotation, 150.0);
    let want_skew = eval(&h, &p, 6.0)["animated"]["skew"].as_f64().unwrap();
    // easeInOut at a quarter of the time: y = 3u² − 2u³ with u = ¼, of 40.
    near(&json!(want_skew), 40.0 * (3.0 / 16.0 - 2.0 / 64.0), "the original eased value");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 16, "keyframes": "keep" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 0.0), (11, 100.0), (16, 150.0)]);
    near(&eval(&h, &p, 16.0)["animated"]["rotation"], 150.0, "held at the new end");
    near(&eval(&h, &p, 99.0)["animated"]["rotation"], 150.0, "and after it");
    assert_eq!(keys_of(&h, &p, "pivot.x"), vec![(16, 7.0)]);
    let sk = keys_of(&h, &p, "skew");
    assert_eq!(sk.last().unwrap().0, 16);
    let at16 = 40.0 * { let u: f64 = 15.0 / 20.0; 3.0 * u * u - 2.0 * u * u * u };
    near(&json!(sk.last().unwrap().1), at16, "the curve at frame 16");
    // A key exactly at the new end is kept as it is; extending adds nothing; duration 1 keeps one key.
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 11, "keyframes": "keep" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 0.0), (11, 100.0)]);
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 50, "keyframes": "keep" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 0.0), (11, 100.0)]);
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 1, "keyframes": "keep" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 0.0)]);
}

#[test]
fn si089_scale_rounds_collisions_later_wins_and_moving_start_keeps_local_keys() {
    let h = fixture();
    let p = peg(&h, "Scaled");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 11 }));
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [
        { "frame": 1, "value": 1 }, { "frame": 3, "value": 3 }, { "frame": 4, "value": 4 }, { "frame": 11, "value": 11 } ] }));
    // 1 + (k − 1)·(6 − 1)/(11 − 1): 1 → 1, 3 → 2, 4 → 2.5 → 3 (half up), 11 → 6.
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 6, "keyframes": "scale" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 1.0), (2, 3.0), (3, 4.0), (6, 11.0)]);
    // A collision: 1 + (k − 1)·2/5: 2 → 1.4 → 1, 3 → 1.8 → 2; the later key wins at frame 1 (1 → 1 and 2 → 1).
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 3, "keyframes": "scale" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 3.0), (2, 4.0), (3, 11.0)]);
    // Duration 1: everything collides, the last key wins.
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 1, "keyframes": "scale" }));
    assert_eq!(keys_of(&h, &p, "rotation"), vec![(1, 11.0)]);
    // Moving the start: the same local keys, later in the scene.
    let q = peg(&h, "Moved");
    ok(&h, "anim.function.set", json!({ "nodeId": q, "channel": "rotation", "keys": [{ "frame": 2, "value": 5 }, { "frame": 4, "value": 9 }] }));
    let before = full(&h, &q)["channels"].clone();
    ok(&h, "element.timing.set", json!({ "elementIds": [q], "start": 10 }));
    assert_eq!(full(&h, &q)["channels"], before, "local keys stay");
    near(&eval(&h, &q, 11.0)["animated"]["rotation"], 5.0, "the key moved with the bar");
    near(&eval(&h, &q, 13.0)["animated"]["rotation"], 9.0, "the key moved with the bar");
}

#[test]
fn si089_truncating_a_path_keeps_the_position_trims_control_points_and_rescales_velocity() {
    let h = fixture();
    let p = peg(&h, "Trimmed");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 41 }));
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 },
        { "kind": "control", "x": 10, "y": 0, "z": 0 },
        { "kind": "control", "x": 30, "y": 0, "z": 0 },
        { "kind": "key", "frame": 41, "x": 40, "y": 0, "z": 0 } ] }));
    // Linear velocity: at frame 21 the node has passed the first control point, not the second.
    ok(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "velocity", "frame": 11, "value": 5 }));
    let at = eval(&h, &p, 21.0)["animated"]["position"]["x"].as_f64().unwrap();
    let id_first = full(&h, &p)["channels"]["position"]["points"][0]["id"].clone();
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 21, "keyframes": "keep" }));
    let pts = full(&h, &p)["channels"]["position"]["points"].as_array().unwrap().clone();
    assert_eq!(pts.len(), 3, "first key, the passed control point, the new end: {pts:?}");
    assert_eq!(pts[0]["id"], id_first, "kept points keep their IDs");
    assert_eq!(pts[1]["frame"], Value::Null);
    assert_eq!(pts[2]["frame"], 21);
    near(&pts[2]["position"]["x"], at, "the original curve's position at the new end");
    // The intermediate velocity key keeps its frame and its fields value on the shorter path.
    near(&eval(&h, &p, 21.0)["animated"]["position"]["x"], at, "the new end");
    let f = ok(&h, "anim.function.get", json!({ "nodeId": p, "channel": "velocity" }));
    assert_eq!(f["keys"][0]["frame"], 11.0);
    assert_eq!(f["pins"].as_array().unwrap().len(), 2);
    // Everything past the end: a single point at the held position.
    let q = peg(&h, "Late");
    ok(&h, "element.timing.set", json!({ "elementIds": [q], "duration": 40 }));
    ok(&h, "anim.path.set", json!({ "nodeId": q, "points": [
        { "kind": "key", "frame": 20, "x": 1, "y": 2, "z": 3 }, { "kind": "key", "frame": 30, "x": 9, "y": 9, "z": 9 } ] }));
    ok(&h, "element.timing.set", json!({ "elementIds": [q], "duration": 10, "keyframes": "keep" }));
    let pts = full(&h, &q)["channels"]["position"]["points"].as_array().unwrap().clone();
    assert_eq!(pts.len(), 1);
    assert_eq!(pts[0]["frame"], 10);
    assert_eq!(pts[0]["position"]["x"], 1.0);
}

#[test]
fn loops_pin_cycle_length_for_count_and_fill() {
    let h = fixture();
    let p = peg(&h, "Looper");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 10 }));
    ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [{ "frame": 1, "value": 0 }, { "frame": 10, "value": 90 }] }));
    // count: the bar becomes loops × L.
    let r = ok(&h, "anim.loop.set", json!({ "nodeId": p, "loops": 3 }));
    assert_eq!((r["cycleLength"].as_u64(), r["duration"].as_u64()), (Some(10), Some(30)));
    near(&eval(&h, &p, 10.0)["animated"]["rotation"], 90.0, "end of a cycle");
    near(&eval(&h, &p, 11.0)["animated"]["rotation"], 0.0, "restart");
    near(&eval(&h, &p, 25.0)["animated"]["rotation"], 40.0, "third cycle, local frame 5");
    near(&eval(&h, &p, 200.0)["animated"]["rotation"], 90.0, "held at the end of the bar");
    // Keys are refused inside repeated cycles.
    assert_eq!(err(&h, "anim.function.key.add", json!({ "nodeId": p, "channel": "rotation", "frame": 11 })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.transform.set", json!({ "nodeIds": [p], "frame": 15, "rotation": 1 })).code, "anim.frameInvalid");
    // fill: L is kept, the cycles follow from the duration, a partial last cycle plays.
    let r = ok(&h, "anim.loop.set", json!({ "nodeId": p, "loopType": "fill" }));
    assert_eq!((r["cycleLength"].as_u64(), r["duration"].as_u64()), (Some(10), Some(30)));
    assert_eq!(get(&h, &p)["cycleLength"], 10);
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 25 }));
    assert_eq!(get(&h, &p)["cycleLength"], 10, "L kept");
    assert_eq!(get(&h, &p)["loops"], 3, "⌈25 ÷ 10⌉");
    assert_eq!(keys_of(&h, &p, "rotation").len(), 2, "fill does not touch the keys");
    near(&eval(&h, &p, 25.0)["animated"]["rotation"], 40.0, "partial last cycle");
    // Back to count: L carried over; the field goes.
    let r = ok(&h, "anim.loop.set", json!({ "nodeId": p, "loops": 2 }));
    assert_eq!((r["cycleLength"].as_u64(), r["duration"].as_u64()), (Some(10), Some(20)));
    assert!(get(&h, &p).get("cycleLength").is_none());
    // Non-divisible count bars: L = ⌊d ÷ loops⌋ and the remainder plays as a partial cycle.
    let q = peg(&h, "Uneven");
    ok(&h, "element.timing.set", json!({ "elementIds": [q], "duration": 10 }));
    ok(&h, "anim.function.set", json!({ "nodeId": q, "channel": "rotation", "keys": [{ "frame": 1, "value": 0 }, { "frame": 3, "value": 20 }] }));
    ok(&h, "anim.loop.set", json!({ "nodeId": q, "loops": 3 }));
    ok(&h, "element.timing.set", json!({ "elementIds": [q], "duration": 10, "keyframes": "keep" }));
    let e = eval(&h, &q, 10.0);
    assert_eq!(e["localFrame"], 10.0);
    // L = 3, frame 10 is cycle 3, frame 1 of the partial cycle.
    near(&e["animated"]["rotation"], 0.0, "partial cycle");
    // Huge requests are typed refusals with checked arithmetic.
    let r2 = peg(&h, "Huge");
    ok(&h, "element.timing.set", json!({ "elementIds": [r2], "duration": 60_000 }));
    let e = err(&h, "anim.loop.set", json!({ "nodeId": r2, "loops": 3 }));
    assert_eq!(e.code, "anim.valueOutOfRange");
    assert!(e.details["maximum"].as_f64().unwrap() >= 1.0, "{e:?}");
    assert_eq!(err(&h, "anim.loop.set", json!({ "nodeId": r2, "loops": 20_000 })).code, "arg.invalid", "the schema's 1…10000");
    assert_eq!(err(&h, "anim.loop.set", json!({ "nodeId": BOB, "loops": 2 })).code, "element.notLoopable");
}

#[test]
fn position_loops_cycle_or_offset_and_nesting_uses_the_same_scene_frame() {
    let h = fixture();
    let p = peg(&h, "Walk");
    ok(&h, "element.timing.set", json!({ "elementIds": [p], "duration": 5 }));
    ok(&h, "anim.path.set", json!({ "nodeId": p, "points": [
        { "kind": "key", "frame": 1, "x": 0, "y": 0, "z": 0 }, { "kind": "key", "frame": 5, "x": 8, "y": 0, "z": 0 } ] }));
    ok(&h, "anim.loop.set", json!({ "nodeId": p, "loops": 3 }));
    near(&eval(&h, &p, 6.0)["animated"]["position"]["x"], 0.0, "cycle restarts");
    near(&eval(&h, &p, 8.0)["animated"]["position"]["x"], 4.0, "cycle");
    ok(&h, "anim.loop.set", json!({ "nodeId": p, "loops": 3, "positionLoop": "offset" }));
    near(&eval(&h, &p, 6.0)["animated"]["position"]["x"], 8.0, "cycle 1 shifted by the first cycle's displacement");
    near(&eval(&h, &p, 8.0)["animated"]["position"]["x"], 12.0, "cycle 1");
    near(&eval(&h, &p, 15.0)["animated"]["position"]["x"], 24.0, "cycle 2's end");
    // A child with its own timing is evaluated at the same scene frame as the looping parent.
    let c = peg(&h, "Child");
    attach(&h, &c, &p);
    ok(&h, "element.timing.set", json!({ "elementIds": [c], "start": 3, "duration": 2 }));
    near(&eval(&h, &c, 8.0)["world"]["position"]["x"], 12.0, "the parent's cycle at the child's frame");
    // The child's bar ended: its channels hold; the parent keeps moving it.
    near(&eval(&h, &c, 9.0)["world"]["position"]["x"], 14.0, "the parent keeps transforming it");
}

// ----- cameras --------------------------------------------------------------------------------------------------------

#[test]
fn cameras_list_get_set_and_project() {
    let h = fixture();
    let list = ok(&h, "anim.camera.list", json!({}));
    assert_eq!(list["cameras"].as_array().unwrap().len(), 1);
    assert_eq!(list["cameras"][0]["current"], true);
    let current = ok(&h, "anim.camera.current.get", json!({}));
    assert_eq!(current["cameraId"], list["cameras"][0]["id"]);
    let got = ok(&h, "anim.camera.get", json!({ "frame": 1 }));
    assert_eq!(got["fov"], 41.1121);
    assert_eq!(got["effectiveFov"], 41.1121);
    assert_eq!(got["position"]["z"], 16.0);
    let (w, hh) = (got["resolution"]["w"].as_f64().unwrap(), got["resolution"]["h"].as_f64().unwrap());
    // Corners on the table plane: 12 fields wide through the default camera (to 4e-6, SI-102), height by aspect.
    let c = &got["frameCornersAtZ0"];
    assert!((c[0]["x"].as_f64().unwrap() + 6.0).abs() < 1e-5, "{c}");
    assert!((c[1]["x"].as_f64().unwrap() - 6.0).abs() < 1e-5, "{c}");
    assert!((c[0]["y"].as_f64().unwrap() - 6.0 * hh / w).abs() < 1e-5, "{c}");
    // The view matrix takes the camera to the origin.
    let v: Vec<f64> = got["viewMatrix"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(v[11], -16.0);
    // Projection through it (05 §2.6).
    let tan = (41.1121_f64 / 2.0).to_radians().tan();
    let r = ok(&h, "anim.projection.get", json!({ "frame": 1, "points": [{ "x": 3, "y": 1.5, "z": 0 }, { "x": 0, "y": 0, "z": 16 }, { "x": 100, "y": 0, "z": 0 }] }));
    let want_x = (3.0 / (16.0 * tan) + 1.0) / 2.0 * w;
    let want_y = (1.0 - 1.5 / (16.0 * tan * hh / w)) / 2.0 * hh;
    near(&r["points"][0]["x"], want_x, "pixel x");
    near(&r["points"][0]["y"], want_y, "pixel y");
    near(&r["points"][0]["depth"], 16.0, "depth");
    assert_eq!(r["points"][0]["visible"], true);
    assert_eq!(r["points"][1]["x"], Value::Null, "at the camera: not projected");
    assert_eq!(r["points"][1]["visible"], false);
    assert_eq!(r["points"][2]["visible"], false, "outside the frame");
    assert_eq!(err(&h, "anim.projection.get", json!({ "cameraId": BOB })).code, "anim.notFound");
    // Set: FOV bounds; dynamic mode keys the channel; switching initialises it with the static value.
    assert_eq!(err(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 0.5 })).code, "anim.valueOutOfRange");
    assert_eq!(err(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 180 })).code, "anim.valueOutOfRange");
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 60, "roll": 90, "position": { "z": 20 } }));
    let got = ok(&h, "anim.camera.get", json!({ "frame": 1 }));
    assert_eq!(got["fov"], 60.0);
    assert_eq!(got["roll"], 90.0);
    assert_eq!(got["position"]["z"], 20.0);
    near(&got["worldPosition"]["z"], 20.0, "world");
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fovMode": "dynamic", "frame": 1 }));
    assert_eq!(keys_of(&h, CAMERA, "fov"), vec![(1, 60.0)]);
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fov": 90, "frame": 11 }));
    let got = ok(&h, "anim.camera.get", json!({ "frame": 6 }));
    assert_eq!(got["effectiveFov"], 75.0);
    assert_eq!(got["fov"], 60.0, "the static value stays");
    ok(&h, "anim.camera.set", json!({ "cameraId": CAMERA, "fovMode": "static" }));
    assert_eq!(ok(&h, "anim.camera.get", json!({ "frame": 6 }))["effectiveFov"], 60.0);
    // A camera on a scaled, rolled peg is rigid: position and roll only.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [CAMERA], "target": "static", "rotation": 0, "position": { "x": 0, "y": 0, "z": 10 } }));
    let holder = peg(&h, "CameraHolder");
    attach(&h, CAMERA, &holder);
    ok(&h, "anim.transform.set", json!({ "nodeIds": [holder], "target": "static", "position": { "x": 2, "y": 1, "z": 0 }, "rotation": 90, "scale": { "x": 5, "y": 0.5 } }));
    let got = ok(&h, "anim.camera.get", json!({ "frame": 1 }));
    near(&got["worldPosition"]["x"], 2.0, "world x");
    near(&got["worldPosition"]["y"], 1.0, "world y");
    near(&got["worldPosition"]["z"], 10.0, "world z");
    let v: Vec<f64> = got["viewMatrix"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    near(&json!(v[0]), 0.0, "no inherited scale");
    near(&json!(v[1]), 1.0, "unit rotation");
    // A camera behind the table plane has no corners.
    ok(&h, "anim.transform.set", json!({ "nodeIds": [CAMERA], "target": "static", "position": { "z": -1 } }));
    assert_eq!(ok(&h, "anim.camera.get", json!({ "frame": 1 }))["frameCornersAtZ0"], Value::Null);
}

#[test]
fn camera_creation_current_replacement_and_last_camera_refusal() {
    let h = fixture();
    let rx = h.subscribe();
    let wide = ok(&h, "anim.camera.create", json!({ "name": "Wide" }))["cameraId"].as_str().unwrap().to_owned();
    let original = get(&h, CAMERA)["id"].clone();
    assert_eq!(ok(&h, "anim.camera.current.get", json!({}))["cameraId"], original, "not current automatically");
    assert!(!rx.try_iter().any(|e| e.name == "anim.camera.currentChanged"));
    // Defaults.
    let w = get(&h, &wide);
    assert_eq!(w["transform"]["offset"]["z"], 16.0);
    assert_eq!(w["camera"]["fov"], 41.1121);
    let rx = h.subscribe();
    ok(&h, "anim.camera.current.set", json!({ "cameraId": wide }));
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "anim.camera.currentChanged").collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["cameraId"], w["id"]);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(ok(&h, "anim.camera.current.get", json!({}))["cameraId"], original, "undoable");
    ok(&h, "edit.redo", json!({}));
    // Deleting the current camera makes the first remaining in Timeline order current, with its event.
    let rx = h.subscribe();
    let r = ok(&h, "anim.camera.delete", json!({ "cameraId": wide }));
    assert_eq!(r["currentCameraId"], original);
    assert!(rx.try_iter().any(|e| e.name == "anim.camera.currentChanged"));
    // The last camera is refused on every route.
    let rev0 = rev(&h);
    assert_eq!(err(&h, "anim.camera.delete", json!({ "cameraId": CAMERA })).code, "anim.lastCamera");
    assert_eq!(err(&h, "element.delete", json!({ "elementIds": [CAMERA] })).code, "anim.lastCamera");
    assert_eq!(err(&h, "edit.delete", json!({ "context": "staging", "objectIds": [CAMERA] })).code, "anim.lastCamera");
    assert_eq!(err(&h, "edit.cut", json!({ "context": "staging", "objectIds": [CAMERA] })).code, "anim.lastCamera");
    assert_eq!(rev(&h), rev0);
    // Generic delete of a current camera replaces it too.
    let spare = ok(&h, "anim.camera.create", json!({ "name": "Spare", "makeCurrent": true, "fov": 30, "position": { "x": 1, "y": 2, "z": 3 } }))["cameraId"].as_str().unwrap().to_owned();
    let s = get(&h, &spare);
    assert_eq!((s["camera"]["fov"].as_f64(), s["transform"]["offset"]["x"].as_f64()), (Some(30.0), Some(1.0)));
    assert_eq!(ok(&h, "anim.camera.current.get", json!({}))["cameraId"], s["id"]);
    ok(&h, "edit.delete", json!({ "context": "staging", "objectIds": [spare] }));
    assert_eq!(ok(&h, "anim.camera.current.get", json!({}))["cameraId"], original);
    // Not a camera.
    assert_eq!(err(&h, "anim.camera.delete", json!({ "cameraId": BOB })).code, "anim.notFound");
    assert_eq!(err(&h, "anim.camera.current.set", json!({ "cameraId": BOB })).code, "anim.notFound");
    assert_eq!(err(&h, "anim.camera.create", json!({ "fov": 200 })).code, "anim.valueOutOfRange");
}

// ----- anim.evaluate --------------------------------------------------------------------------------------------------

#[test]
fn evaluate_reports_the_shape_and_changes_nothing() {
    let h = fixture();
    ok(&h, "element.select", json!({ "elementIds": [BOB] }));
    let state = ok(&h, "app.state.get", json!({}));
    let before = rev(&h);
    let drawing = ok(&h, "draw.drawing.get", json!({ "drawingId": "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"" }));
    let r = ok(&h, "anim.evaluate", json!({ "nodeIds": [BOB, PEG, CAMERA], "frames": [1, 7.5, 24] }));
    let frames = r["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[1]["frame"], 7.5);
    let n = &frames[0]["nodes"][0];
    for part in ["id", "kind", "localFrame", "static", "animated", "local", "world", "screen"] {
        assert!(n.get(part).is_some(), "{part}: {n}");
    }
    assert_eq!(n["kind"], "drawing");
    assert!(frames[0]["nodes"][1].get("bounds").is_none(), "pegs report only their origin");
    assert!(frames[0]["nodes"][2].get("bounds").is_none(), "cameras too");
    // `include` narrows; `frames` defaults to the current frame.
    let r = ok(&h, "anim.evaluate", json!({ "nodeIds": [BOB], "include": ["world"] }));
    let n = &r["frames"][0]["nodes"][0];
    assert!(n.get("world").is_some() && n.get("animated").is_none() && n.get("screen").is_none(), "{n}");
    // Errors.
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": ["element:@nope"] })).code, "anim.notFound");
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": [BOB], "frames": [0] })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": [BOB], "frames": [100_001] })).code, "anim.frameInvalid");
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": [BOB], "cameraId": BOB })).code, "anim.notFound");
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": [SOUND] })).code, "anim.notTransformable");
    let bone = ok(&h, "doc.object.find", json!({ "type": "element", "where": { "elementType": "bone" }, "limit": 1 }))["items"][0]["id"].clone();
    assert_eq!(err(&h, "anim.evaluate", json!({ "nodeIds": [bone] })).code, "anim.notTransformable");
    // Nothing moved: no revision, selection, zone or saved default.
    assert_eq!(rev(&h), before);
    assert_eq!(ok(&h, "app.state.get", json!({})), state);
    assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"" })), drawing);
}

#[test]
fn the_same_requests_answer_the_same_bytes() {
    let run_once = || {
        let h = fixture();
        let p = walker(&h);
        ok(&h, "anim.function.set", json!({ "nodeId": p, "channel": "rotation", "keys": [{ "frame": 1, "value": 0, "preset": "extremeEaseInOut" }, { "frame": 25, "value": 77.7 }] }));
        let frames: Vec<f64> = (0..200).map(|i| 1.0 + f64::from(i) * 0.125).collect();
        serde_json::to_string(&ok(&h, "anim.evaluate", json!({ "nodeIds": [p], "frames": frames }))).unwrap()
    };
    assert_eq!(run_once(), run_once());
}

// ----- AG-02 ----------------------------------------------------------------------------------------------------------

#[test]
fn generic_patches_of_transform_and_camera_properties_use_the_owning_commands() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [{ "op": "replace", "path": "/transform/rotation", "value": 5 }] }));
    assert_eq!(get(&h, BOB)["transform"]["rotation"], 5.0);
    assert_eq!(rx.try_iter().filter(|e| e.name == "anim.changed").count(), 1, "the owner's event, once");
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "props": { "transform": { "offset": { "x": 1 }, "skew": 10, "scale": { "y": 2 } } } }));
    let t = get(&h, BOB)["transform"].clone();
    assert_eq!((t["offset"]["x"].as_f64(), t["skew"].as_f64(), t["scale"]["y"].as_f64()), (Some(1.0), Some(10.0), Some(2.0)));
    // The owner's refusals, in every form.
    for ops in [
        json!([{ "op": "replace", "path": "/transform/skew", "value": 89.5 }]),
        json!([{ "op": "add", "path": "/meta/s", "value": 89.5 }, { "op": "copy", "from": "/meta/s", "path": "/transform/skew" }]),
        json!([{ "op": "replace", "path": "/transform/scale/x", "value": 0.0001 }]),
        json!([{ "op": "replace", "path": "/transform/offset/z", "value": 2e7 }]),
    ] {
        let r0 = rev(&h);
        assert_eq!(err(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": ops })).code, "anim.valueOutOfRange");
        assert_eq!(rev(&h), r0);
    }
    // A whole transform block is split, field by field, through the owners; a test sees the staged result.
    let mut block = get(&h, BOB)["transform"].clone();
    block["rotation"] = json!(33);
    block["keepProportions"] = json!(true);
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [
        { "op": "replace", "path": "/transform", "value": block },
        { "op": "test", "path": "/transform/rotation", "value": 33.0 } ] }));
    let t = get(&h, BOB)["transform"].clone();
    assert_eq!((t["rotation"].as_f64(), t["keepProportions"].as_bool()), (Some(33.0), Some(true)));
    block["skew"] = json!(120);
    assert_eq!(err(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [{ "op": "replace", "path": "/transform", "value": block }] })).code, "anim.valueOutOfRange");
    // Camera block and the scene's current camera.
    ok(&h, "doc.object.patch", json!({ "targets": [CAMERA], "props": { "camera": { "fov": 50 } } }));
    assert_eq!(get(&h, CAMERA)["camera"]["fov"], 50.0);
    assert_eq!(err(&h, "doc.object.patch", json!({ "targets": [CAMERA], "ops": [{ "op": "replace", "path": "/camera/fov", "value": 500 }] })).code, "anim.valueOutOfRange");
    let wide = ok(&h, "anim.camera.create", json!({ "name": "Wide" }))["cameraId"].clone();
    let rx = h.subscribe();
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "currentCameraId": wide } }));
    assert_eq!(ok(&h, "anim.camera.current.get", json!({}))["cameraId"], wide);
    assert_eq!(rx.try_iter().filter(|e| e.name == "anim.camera.currentChanged").count(), 1);
    // A non-camera is the owner's refusal, not a schema error.
    let bob = get(&h, BOB)["id"].clone();
    assert_eq!(err(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "currentCameraId": bob } })).code, "anim.notFound");
    // One undo step for everything in one patch.
    let before = get(&h, BOB);
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [
        { "op": "replace", "path": "/transform/rotation", "value": 1 },
        { "op": "add", "path": "/meta/x", "value": 1 },
        { "op": "replace", "path": "/transform/skew", "value": 1 } ] }));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(sans_rev(get(&h, BOB)), sans_rev(before));
    // Other commands' properties are still refused with their owners.
    assert_eq!(err(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "visible": false } })).code, "command.notImplemented");
}
