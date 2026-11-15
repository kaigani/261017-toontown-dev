//! The Camera view's composition (camera-view note §2-§4: CV1-CV4; M3 merge note §8 item 3, camera half; scene
//! answer form 1.1, `docs/canvas-format.md` §11).

use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

use crate::common::*;

pub const BOB: &str = "scene:#1/element:\"Bob\"";

/// The Camera view's scene answer.
pub fn scene(h: &ExecutorHandle) -> Value {
    serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap()
}

/// A drawing element `name` with one 100 × 100 du square drawing exposed at frame 1, placed statically at `pos`
/// (fields) with `rotation` degrees; its id.
pub fn square(h: &ExecutorHandle, name: &str, pos: (f64, f64, f64), rotation: f64) -> String {
    let id = ok(h, "element.create", json!({ "type": "drawing", "name": name }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(h, "element.drawing.create", json!({ "elementId": id, "name": "1", "exposeFrom": 1, "exposeTo": 1 }));
    ok(h, "app.context.set", json!({ "elementId": id, "frame": 1 }));
    ok(h, "draw.rectangle.add", json!({ "from": { "x": -50, "y": -50 }, "to": { "x": 50, "y": 50 } }));
    ok(
        h,
        "anim.transform.set",
        json!({ "nodeIds": [id], "target": "static", "position": { "x": pos.0, "y": pos.1, "z": pos.2 }, "rotation": rotation }),
    );
    id
}

/// An element's id by reference.
pub fn id_of(h: &ExecutorHandle, r: &str) -> String {
    ok(h, "doc.object.resolve", json!({ "refs": [r] }))["items"][0]["id"].as_str().unwrap().to_owned()
}

/// Drawing units → output pixels of `element` at `frame`, through `inspect.coordinates.convert` (the evaluator and
/// the camera projection, a code path apart from the render list's `plane_to_pixel`).
pub fn plane(h: &ExecutorHandle, element: &str, frame: u32) -> [f64; 6] {
    let r = ok(
        h,
        "inspect.coordinates.convert",
        json!({ "from": "drawing", "to": "image", "elementId": element, "frame": frame,
                "points": [{ "x": 0, "y": 0 }, { "x": 1, "y": 0 }, { "x": 0, "y": 1 }] }),
    );
    let p = |i: usize| (r["points"][i]["x"].as_f64().unwrap(), r["points"][i]["y"].as_f64().unwrap());
    let (o, a, b) = (p(0), p(1), p(2));
    [a.0 - o.0, a.1 - o.1, b.0 - o.0, b.1 - o.1, o.0, o.1]
}

pub fn matrix(v: &Value) -> [f64; 6] {
    let m: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    [m[0], m[1], m[2], m[3], m[4], m[5]]
}

pub fn close(a: &[f64; 6], b: &[f64; 6], what: &str) {
    for i in 0..6 {
        let tol = 1e-9 * (1.0 + a[i].abs().max(b[i].abs()));
        assert!((a[i] - b[i]).abs() <= tol, "{what}: {a:?} vs {b:?}");
    }
}

pub fn apply(m: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// CV1: the Camera view shows exactly the elements the final renderer's camera walk draws or places around the frame,
/// each through the renderer's plane matrix, back to front; hidden elements, elements under a hidden ancestor and
/// elements behind the camera are absent (camera-view note §2, §4).
#[test]
fn cv1_the_occurrences_are_the_renderers_elements_and_plane_matrices() {
    let h = fixture();
    let near = square(&h, "Near", (-3.0, 0.0, 0.0), 0.0);
    let far = square(&h, "Far", (2.0, 1.0, -5.0), 30.0);
    let hidden = square(&h, "Hidden", (0.0, 2.0, 0.0), 0.0);
    let behind = square(&h, "Behind", (0.0, 0.0, 20.0), 0.0);
    let aside = square(&h, "Aside", (40.0, 0.0, 0.0), 0.0);
    let peg = ok(&h, "element.create", json!({ "type": "peg", "name": "HiddenPeg" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let child = square(&h, "UnderHiddenPeg", (1.0, -2.0, 0.0), 0.0);
    ok(&h, "element.parent.set", json!({ "elementIds": [child], "parentId": peg }));
    ok(&h, "element.visibility.set", json!({ "elementIds": [hidden, peg], "scope": "scene", "visible": false }));
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    let s = scene(&h);
    assert_eq!(s["sceneForm"], "1.1");
    assert_eq!(s["viewType"], "camera");
    assert_eq!(s["frame"], 1);
    let ids: Vec<&str> = s["entries"].as_array().unwrap().iter().map(|e| e["elementId"].as_str().unwrap()).collect();
    for shown in [near.as_str(), far.as_str(), aside.as_str()] {
        assert!(ids.contains(&shown), "{shown} shown: {ids:?}");
    }
    for absent in [hidden.as_str(), behind.as_str(), child.as_str()] {
        assert!(!ids.contains(&absent), "{absent} absent: {ids:?}");
    }
    // Back to front (the renderer's order: role tier, then depth, SI-106): the farther square (Z = -5) before the
    // nearer one (Z = 0).
    let at = |id: &str| ids.iter().position(|x| *x == id).unwrap();
    assert!(at(&far) < at(&near), "{ids:?}");
    // Every occurrence's matrix is its element's plane through the camera (an independent path), to 1e-9.
    for e in s["entries"].as_array().unwrap() {
        let id = e["elementId"].as_str().unwrap();
        close(&matrix(&e["matrix"]), &plane(&h, id, 1), id);
        assert_eq!(e["role"], if e["elementId"] == json!(id_of(&h, BOB)) { "current" } else { "staticUnder" });
        let d = ok(&h, "draw.drawing.get", json!({ "drawingId": e["drawingId"] }));
        assert_eq!(e["rev"], d["rev"], "the occurrence names the drawing's revision");
    }
}

/// CV2: a drawing is resident once whatever its occurrence count. In this model a drawing belongs to one element and an
/// element is one occurrence, so the entries never name a drawing twice; the canvas's load set is their drawings.
#[test]
fn cv2_each_drawing_appears_once_among_the_loads() {
    let h = fixture();
    for (i, x) in [-4.0, -2.0, 0.0, 2.0, 4.0].iter().enumerate() {
        square(&h, &format!("S{i}"), (*x, 0.0, 0.0), 0.0);
    }
    let s = scene(&h);
    let mut drawings: Vec<&str> =
        s["entries"].as_array().unwrap().iter().map(|e| e["drawingId"].as_str().unwrap()).collect();
    let n = drawings.len();
    drawings.sort_unstable();
    drawings.dedup();
    assert_eq!(drawings.len(), n);
    assert!(n >= 6, "Bob and the five squares: {n}");
}

/// CV4: overlays never reach a render: the camera render is byte-identical with and without a selection, while the
/// Camera view's overlays show it.
#[test]
fn cv4_the_selection_overlay_never_reaches_a_render() {
    let h = fixture();
    let render = |h: &ExecutorHandle| {
        ok(h, "render.frame.run", json!({ "composition": "camera", "scale": 0.25 }))["sha256"].clone()
    };
    let before = render(&h);
    assert_eq!(scene(&h)["overlays"]["selectionBoxes"], json!([]));
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let s = scene(&h);
    assert_eq!(s["overlays"]["selectionBoxes"].as_array().unwrap().len(), 1, "{s}");
    assert_eq!(s["overlays"]["pivots"].as_array().unwrap().len(), 1, "{s}");
    assert!(s["overlays"]["selectionUnion"].is_array());
    assert_eq!(render(&h), before);
}

/// M3 merge note §8 item 3, camera half (C4.3 plan answer 7): the Camera view's mapping agrees with the final renderer
/// for (a) a rolled camera, (b) the default field of view, (c) an element under a hidden ancestor: the centre of each
/// shown element, mapped through its occurrence matrix, is where the renderer's hit test finds that element, and a
/// hidden element is neither shown nor hit.
#[test]
fn m3_8_3_the_camera_view_agrees_with_the_renderer_on_roll_fov_and_hidden_ancestors() {
    let h = fixture();
    let a = square(&h, "Agree", (1.5, 0.5, 0.0), 0.0);
    let cam = ok(&h, "anim.camera.current.get", json!({}))["cameraId"].clone();
    let check = |h: &ExecutorHandle, what: &str| {
        let s = scene(h);
        let e = s["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == json!(a)).cloned();
        let e = e.unwrap_or_else(|| panic!("{what}: Agree shown"));
        let (x, y) = apply(&matrix(&e["matrix"]), 0.0, 0.0);
        let hits = ok(h, "inspect.hitTest", json!({ "space": "image", "point": { "x": x, "y": y } }));
        assert!(hits["hits"].as_array().unwrap().iter().any(|h| h["elementId"] == json!(a)), "{what}: {hits}");
        close(&matrix(&e["matrix"]), &plane(h, &a, 1), what);
    };
    // (b) the default field of view.
    let c = ok(&h, "anim.camera.get", json!({ "cameraId": cam, "frame": 1 }));
    assert_eq!(c["fov"], json!(tt_model::defaults::DEFAULT_FOV));
    check(&h, "default fov");
    // (a) a rolled camera.
    ok(&h, "anim.camera.set", json!({ "cameraId": cam, "roll": 17.0 }));
    check(&h, "rolled");
    // (c) under a hidden ancestor: neither shown nor hit.
    let peg = ok(&h, "element.create", json!({ "type": "peg", "name": "Hide" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let before = scene(&h);
    let e = before["entries"].as_array().unwrap().iter().find(|e| e["elementId"] == json!(a)).cloned().unwrap();
    let (x, y) = apply(&matrix(&e["matrix"]), 0.0, 0.0);
    ok(&h, "element.parent.set", json!({ "elementIds": [a], "parentId": peg }));
    ok(&h, "element.visibility.set", json!({ "elementIds": [peg], "scope": "scene", "visible": false }));
    let s = scene(&h);
    assert!(!s["entries"].as_array().unwrap().iter().any(|e| e["elementId"] == json!(a)));
    let hits = ok(&h, "inspect.hitTest", json!({ "space": "image", "point": { "x": x, "y": y } }));
    assert!(!hits["hits"].as_array().unwrap().iter().any(|h| h["elementId"] == json!(a)), "{hits}");
}

/// The members of a JSON object, sorted.
fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    k.sort();
    k
}

/// `docs/canvas-format.md` §11.7: the kernel's answers carry exactly the members of the corpus's valid answers (the
/// window's decoder is tested against those files in Vitest), in the scene, an entry, the camera and the overlays.
#[test]
fn the_kernels_scene_answers_have_the_corpus_members() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/canvas/testdata/scene/valid");
    let read = |n: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(root.join(n)).unwrap()).unwrap() };
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let cam = scene(&h);
    let want = read("camera.json")["scene"].clone();
    assert_eq!(keys(&cam), keys(&want), "the Camera view's scene members");
    assert_eq!(keys(&cam["entries"][0]), keys(&want["entries"][0]), "an occurrence's members");
    assert_eq!(keys(&cam["camera"]), keys(&want["camera"]), "the camera's members");
    assert_eq!(keys(&cam["overlays"]), keys(&want["overlays"]), "the overlays' members");
    assert_eq!(keys(&cam["overlays"]["selectionDisplay"]), keys(&want["overlays"]["selectionDisplay"]));
    let drw = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    let want = read("drawing-1.1.json")["scene"].clone();
    assert_eq!(keys(&drw), keys(&want), "the Drawing view's scene members");
    assert_eq!(keys(&drw["entries"][0]), keys(&want["entries"][0]), "a Drawing view entry's members");
}
