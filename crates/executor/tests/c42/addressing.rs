//! The common addressing parameters are honoured, not only declared (02 §13.1 items 4 and 5; C42A-R3-01): for every
//! point-taking `draw.*` and `colour.*` command, the call with `frame` addressing a drawing exposed at another frame
//! (the context stays at frame 1) and `space: "view"` points in a zoomed, panned view equals the same call with an
//! explicit `drawingId` and drawing-space points: the same answer and the same canonical saved bytes.

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Invocation};

use crate::common::*;
use crate::equivalence::{BOB, D1, canonical, ready};

struct World {
    frame: u64,
    d2: String,
    stroke: String,
    text: String,
    /// A second drawing element, made the current one in the `elementId` + `frame` cells, so a command that ignored
    /// `elementId` (and used the current element with `frame`) would fail its cell.
    other: String,
    m: Vec<f64>,
}

/// The fixture at frame 1 with a zoomed, panned (not rotated: rectangles convert) Drawing view, and the drawing exposed
/// at another frame of Bob holding one line and one text.
fn world() -> (ExecutorHandle, World) {
    let h = ready();
    ok(&h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2 }));
    ok(&h, "view.pan.set", json!({ "viewId": "drawingView", "centre": { "x": 40, "y": 30 } }));
    let (frame, d2) = (2..=48u64)
        .find_map(|f| {
            let r = run(&h, "draw.drawing.get", json!({ "elementId": BOB, "frame": f }));
            let v = r.result?;
            let id = v["drawingId"].as_str().map(str::to_owned)?;
            (id != D1).then_some((f, id))
        })
        .expect("Bob exposes another drawing at a later frame");
    // The fixture locks it; unlocked so the edits can run.
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [d2], "locked": false }));
    let line = ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": d2, "from": { "x": -20, "y": -10 }, "to": { "x": 60, "y": 50 }, "layer": "new" }),
    );
    let stroke = format!("{d2}/{}", line["strokeIds"][0].as_str().unwrap());
    let t = ok(&h, "draw.text.add", json!({ "drawingId": d2, "position": { "x": 10, "y": 10 }, "text": "Hi" }));
    let text = t["textId"].as_str().unwrap().to_owned();
    // For the colour commands (view zoom 2 about (40, 30): view (x, y) is drawing ((x - 960) / 2 + 40, (540 - y) / 2 +
    // 30)): an unpainted square around drawing (160, 40) = view (1200, 520), a painted one around (210, 40) = view
    // (1300, 520), and two line ends 5 du apart at (10, 50) and (15, 47.5) = view (900, 500) and (910, 505).
    for (a, b) in [((150, 30), (170, 50)), ((200, 30), (220, 50))] {
        ok(
            &h,
            "draw.rectangle.add",
            json!({ "drawingId": d2, "from": { "x": a.0, "y": a.1 }, "to": { "x": b.0, "y": b.1 }, "layer": "top" }),
        );
    }
    let second = {
        let doc = h.snapshot().unwrap().unwrap();
        doc.palettes().flat_map(|p| p.swatches().iter()).nth(1).unwrap().colour_id().as_str().to_owned()
    };
    ok(&h, "colour.paint.apply", json!({ "drawingId": d2, "points": [{ "x": 210, "y": 40 }], "colourId": second }));
    ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": d2, "from": { "x": -30, "y": 50 }, "to": { "x": 10, "y": 50 }, "layer": "top" }),
    );
    ok(
        &h,
        "draw.line.add",
        json!({ "drawingId": d2, "from": { "x": 15, "y": 47.5 }, "to": { "x": 50, "y": 47.5 }, "layer": "top" }),
    );
    let other =
        ok(&h, "element.create", json!({ "type": "drawing", "name": "Other", "sceneId": "scene:#1" }))["elementIds"][0]
            .as_str()
            .unwrap()
            .to_owned();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    let m: Vec<f64> = ok(&h, "draw.viewMapping.get", json!({ "view": "drawing" }))["viewToDrawing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    assert_eq!(ok(&h, "app.state.get", json!({}))["frame"], 1, "the context stays at frame 1");
    (h, World { frame, d2, stroke, text, other, m })
}

/// How a case writes its coordinates: view space (the call under test) or drawing space (the reference), or the
/// reference's drawing-space point carried forward into another space by `Coords::fwd` (scene space, a Camera view's
/// pixels: C43A-R1-02).
#[derive(Clone, Copy)]
enum Mode {
    View,
    Drawing,
    Mapped,
}

struct Coords<'a> {
    mode: Mode,
    m: &'a [f64],
    /// `Mode::Mapped`: drawing units → the cell's space (an independent mapping of the addressed element).
    fwd: [f64; 6],
}

/// An affine applied to a point (`with_offset`) or a vector.
fn affine(m: &[f64], x: f64, y: f64, with_offset: bool) -> (f64, f64) {
    let (e, f) = if with_offset { (m[4], m[5]) } else { (0.0, 0.0) };
    (m[0] * x + m[2] * y + e, m[1] * x + m[3] * y + f)
}

const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

impl Coords<'_> {
    /// A point given in view pixels, written in the mode's space.
    fn p(&self, x: f64, y: f64) -> Value {
        match self.mode {
            Mode::View => json!({ "x": x, "y": y }),
            Mode::Drawing => {
                json!({ "x": self.m[0] * x + self.m[2] * y + self.m[4], "y": self.m[1] * x + self.m[3] * y + self.m[5] })
            }
            Mode::Mapped => {
                let d = affine(self.m, x, y, true);
                let q = affine(&self.fwd, d.0, d.1, true);
                json!({ "x": q.0, "y": q.1 })
            }
        }
    }

    /// A vector given in view pixels.
    fn v(&self, x: f64, y: f64) -> Value {
        match self.mode {
            Mode::View => json!({ "x": x, "y": y }),
            Mode::Drawing => json!({ "x": self.m[0] * x + self.m[2] * y, "y": self.m[1] * x + self.m[3] * y }),
            Mode::Mapped => {
                let d = affine(self.m, x, y, false);
                let q = affine(&self.fwd, d.0, d.1, false);
                json!({ "x": q.0, "y": q.1 })
            }
        }
    }

    /// An axis-aligned rectangle given in view pixels.
    fn r(&self, x: f64, y: f64, w: f64, h: f64) -> Value {
        match self.mode {
            Mode::View => json!({ "x": x, "y": y, "width": w, "height": h }),
            Mode::Drawing => {
                let (ax, ay) = (self.m[0] * x + self.m[4], self.m[3] * y + self.m[5]);
                let (bx, by) = (self.m[0] * (x + w) + self.m[4], self.m[3] * (y + h) + self.m[5]);
                json!({ "x": ax.min(bx), "y": ay.min(by), "width": (ax - bx).abs(), "height": (ay - by).abs() })
            }
            Mode::Mapped => {
                // Both maps are axis-aligned here (no rotation anywhere on the path), so corners stay corners.
                let (ax, ay) = (self.m[0] * x + self.m[4], self.m[3] * y + self.m[5]);
                let (bx, by) = (self.m[0] * (x + w) + self.m[4], self.m[3] * (y + h) + self.m[5]);
                let (a, b) = (affine(&self.fwd, ax, ay, true), affine(&self.fwd, bx, by, true));
                json!({ "x": a.0.min(b.0), "y": a.1.min(b.1), "width": (a.0 - b.0).abs(), "height": (a.1 - b.1).abs() })
            }
        }
    }
}

/// One case: the command, whether it addresses its drawing (`drawingId` / `frame`), and its parameters.
type Case = (&'static str, bool, fn(&Coords<'_>, &World) -> Value);

fn cases() -> Vec<Case> {
    vec![
        ("draw.pivot.set", true, |c, _| json!({ "point": c.p(980.0, 520.0) })),
        ("draw.hitTest.get", true, |c, _| json!({ "point": c.p(1000.0, 520.0) })),
        (
            "draw.cut.apply",
            true,
            |c, _| json!({ "polygon": [c.p(900.0, 450.0), c.p(1100.0, 450.0), c.p(1100.0, 650.0), c.p(900.0, 650.0)] }),
        ),
        ("draw.erase.apply", true, |c, _| json!({ "path": [c.p(900.0, 520.0), c.p(1150.0, 560.0)] })),
        ("draw.selection.set", true, |c, _| json!({ "by": "rect", "rect": c.r(880.0, 400.0, 300.0, 300.0) })),
        ("draw.pointSelection.set", true, |c, _| json!({ "rect": c.r(880.0, 400.0, 300.0, 300.0) })),
        ("draw.rectangle.add", true, |c, _| json!({ "from": c.p(900.0, 500.0), "to": c.p(1000.0, 600.0) })),
        ("draw.ellipse.add", true, |c, _| json!({ "from": c.p(900.0, 500.0), "to": c.p(1000.0, 600.0) })),
        ("draw.line.add", true, |c, _| json!({ "from": c.p(900.0, 500.0), "to": c.p(1000.0, 600.0) })),
        ("draw.polyline.add", true, |c, _| json!({ "points": [c.p(900.0, 500.0), c.p(1000.0, 600.0)] })),
        (
            "draw.stroke.add",
            true,
            |c, _| json!({ "kind": "pencil", "points": [c.p(900.0, 500.0), c.p(1000.0, 600.0)] }),
        ),
        (
            "draw.stroke.set",
            true,
            |c, w| json!({ "strokeId": w.stroke, "bezier": [c.p(900.0, 500.0), c.p(930.0, 520.0), c.p(970.0, 560.0), c.p(1000.0, 600.0)] }),
        ),
        (
            "draw.content.add",
            true,
            |c, _| json!({ "content": { "nodes": [{ "type": "layer", "id": "l_1", "layerKind": "pencil", "strokes": [{ "id": "s_1", "kind": "pencil", "closed": false, "bezier": [{"x": 0, "y": 0}, {"x": 10, "y": 5}, {"x": 20, "y": 5}, {"x": 30, "y": 0}], "colourId": null, "lineStyle": { "thickness": 2, "lineType": "solid" } }] }] }, "offset": c.v(20.0, -10.0) }),
        ),
        ("draw.text.add", true, |c, _| json!({ "position": c.p(950.0, 550.0), "text": "Yo" })),
        ("draw.text.set", true, |c, w| json!({ "textId": w.text, "position": c.p(950.0, 560.0) })),
        (
            "draw.transform.apply",
            true,
            |c, w| json!({ "ids": [w.stroke.rsplit('/').next().unwrap()], "translate": c.v(30.0, 10.0), "pivot": c.p(960.0, 540.0) }),
        ),
        (
            "draw.envelope.apply",
            true,
            |c, w| json!({ "ids": [w.stroke.rsplit('/').next().unwrap()], "corners": [c.p(900.0, 450.0), c.p(1100.0, 460.0), c.p(1090.0, 650.0), c.p(910.0, 640.0)] }),
        ),
        ("draw.repositionAll.apply", false, |c, _| json!({ "elementId": BOB, "translate": c.v(30.0, 10.0) })),
        ("draw.point.add", false, |c, w| json!({ "strokeId": w.stroke, "at": c.p(1000.0, 500.0) })),
        (
            "draw.point.move",
            true,
            |c, w| json!({ "points": [{ "strokeId": w.stroke, "anchor": 0 }], "delta": c.v(6.0, -4.0) }),
        ),
        ("draw.point.set", false, |c, w| json!({ "strokeId": w.stroke, "anchor": 0, "position": c.p(930.0, 560.0) })),
        (
            "draw.segment.move",
            false,
            |c, w| json!({ "strokeId": w.stroke, "segment": 0, "t": 0.5, "delta": c.v(5.0, 5.0) }),
        ),
        ("colour.paint.apply", true, |c, _| json!({ "points": [c.p(1200.0, 520.0)] })),
        ("colour.paintUnpainted.apply", true, |c, _| json!({ "points": [c.p(1200.0, 520.0)] })),
        ("colour.paint.remove", true, |c, _| json!({ "points": [c.p(1300.0, 520.0)] })),
        ("colour.pick", true, |c, _| json!({ "point": c.p(1000.0, 520.0) })),
        ("colour.gap.close", true, |c, _| json!({ "from": c.p(900.0, 500.0), "to": c.p(910.0, 505.0) })),
    ]
}

#[test]
fn c42a_r3_01_frame_and_view_space_are_honoured_by_every_point_taking_command() {
    let all = cases();
    assert_eq!(all.len(), 27, "the 27 point-taking commands of scripts/check-addressing-params.py");
    let mut failures = Vec::new();
    let mut succeeded = 0;
    for (id, addresses, build) in all {
        let (a, w) = world();
        let (b, wb) = world();
        assert_eq!((w.frame, &w.d2), (wb.frame, &wb.d2));
        let mut pa = build(&Coords { mode: Mode::View, m: &w.m, fwd: IDENTITY }, &w);
        pa["space"] = json!("view");
        pa["view"] = json!("drawingView");
        let mut pb = build(&Coords { mode: Mode::Drawing, m: &wb.m, fwd: IDENTITY }, &wb);
        if addresses {
            pa["frame"] = json!(w.frame);
            pb["drawingId"] = json!(wb.d2);
        }
        let (ea, eb) = (run(&a, id, pa.clone()), run(&b, id, pb.clone()));
        if ea.is_ok() {
            succeeded += 1;
            if let Some(d) = ea.result.as_ref().and_then(|r| r.get("drawingId")).and_then(Value::as_str)
                && addresses
                && d != w.d2
            {
                failures
                    .push(format!("{id}: `frame` {} addressed {d}, not the drawing at that frame {}", w.frame, w.d2));
            }
        }
        let same_answer = ea.status == eb.status
            && ea.error.as_ref().map(|e| e.code.clone()) == eb.error.as_ref().map(|e| e.code.clone())
            && ea.result == eb.result;
        if !same_answer || content(&a) != content(&b) {
            failures.push(format!(
                "{id}: frame/view {pa} -> {:?} {:?}\n  drawingId/drawing {pb} -> {:?} {:?}",
                ea.result, ea.error, eb.result, eb.error
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(succeeded >= 20, "most cases run (not only equal refusals): {succeeded} of 27");
}

/// The canonical saved content without the creation time of the second element (`World::other`), which the clock
/// stamps when each world is built; every other byte, that element's included, is compared.
fn content(h: &ExecutorHandle) -> Vec<(String, String)> {
    canonical(h)
        .into_iter()
        .map(|(k, v)| {
            let mut j: Value = serde_json::from_str(&v).unwrap();
            if j.get("name").and_then(Value::as_str) == Some("Other")
                && let Some(m) = j.as_object_mut()
            {
                m.remove("createdAt");
            }
            (k, j.to_string())
        })
        .collect()
}

/// How a cell names its target (02 §13.1 item 4).
#[derive(Clone, Copy, Debug)]
enum Naming {
    /// `drawingId`.
    DrawingId,
    /// `elementId` + `frame`, with the context moved to another element first (Bob is not the current element).
    ElementFrame,
    /// `frame` alone: the current element's cell.
    FrameAlone,
    /// Nothing: the current drawing (the context moved to that cell first).
    Current,
    /// `drawingId` with an `elementId` + `frame` naming another drawing: `drawingId` takes precedence ("if omitted, it
    /// uses `elementId?` + `frame?`", 02 §13.1 item 4).
    DrawingIdWins,
}

const NAMINGS: [Naming; 5] =
    [Naming::DrawingId, Naming::ElementFrame, Naming::FrameAlone, Naming::Current, Naming::DrawingIdWins];

/// Stroke ids as the drawing's local ids, so the naming decides the drawing (a qualified id names it itself).
fn localise(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "strokeId"
                    && let Some(local) = x.as_str().and_then(|s| s.split_once('/')).map(|(_, l)| l.to_owned())
                {
                    *x = json!(local);
                } else {
                    localise(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(localise),
        _ => {}
    }
}

#[test]
fn c42a_r4_01_every_point_taking_command_by_every_naming_in_both_spaces() {
    let mut failures = Vec::new();
    let mut cells = 0;
    for (id, addresses, build) in cases() {
        // The reference: `drawingId` (the element for the element-wide command) and drawing-space points.
        let (b, wb) = world();
        let mut pb = build(&Coords { mode: Mode::Drawing, m: &wb.m, fwd: IDENTITY }, &wb);
        localise(&mut pb);
        if id == "draw.repositionAll.apply" {
            pb["elementId"] = json!(BOB);
        } else {
            pb["drawingId"] = json!(wb.d2);
        }
        let eb = run(&b, id, pb.clone());
        let reference = content(&b);
        // `draw.repositionAll.apply` edits every drawing of an element (02 §13.2 line 1336: `elementId?: id = current`):
        // only the element namings apply to it.
        let namings: Vec<Naming> = if addresses || id != "draw.repositionAll.apply" {
            NAMINGS.to_vec()
        } else {
            vec![Naming::ElementFrame, Naming::Current]
        };
        for space in [Mode::Drawing, Mode::View] {
            for naming in &namings {
                cells += 1;
                let (a, w) = world();
                let mut pa = build(&Coords { mode: space, m: &w.m, fwd: IDENTITY }, &w);
                localise(&mut pa);
                if matches!(space, Mode::View) {
                    pa["space"] = json!("view");
                    pa["view"] = json!("drawingView");
                }
                if matches!(naming, Naming::ElementFrame) {
                    ok(&a, "app.context.set", json!({ "elementId": w.other, "frame": 1 }));
                    assert_eq!(
                        ok(&a, "app.state.get", json!({}))["element"]["id"],
                        json!(w.other),
                        "Bob is not current"
                    );
                }
                if id == "draw.repositionAll.apply" {
                    if matches!(naming, Naming::Current) {
                        pa.as_object_mut().unwrap().remove("elementId");
                        ok(&a, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
                    }
                } else {
                    match naming {
                        Naming::DrawingId => pa["drawingId"] = json!(w.d2),
                        Naming::ElementFrame => {
                            pa["elementId"] = json!(BOB);
                            pa["frame"] = json!(w.frame);
                        }
                        Naming::FrameAlone => pa["frame"] = json!(w.frame),
                        Naming::Current => {
                            ok(&a, "app.context.set", json!({ "elementId": BOB, "frame": w.frame }));
                        }
                        Naming::DrawingIdWins => {
                            pa["drawingId"] = json!(w.d2);
                            pa["elementId"] = json!(BOB);
                            pa["frame"] = json!(1);
                        }
                    }
                }
                let ea = run(&a, id, pa.clone());
                let space_name = if matches!(space, Mode::View) { "view" } else { "drawing" };
                if !ea.is_ok() {
                    failures.push(format!(
                        "{id} {naming:?} {space_name}: refused {:?}",
                        ea.error.as_ref().map(|e| (&e.code, &e.message))
                    ));
                } else if ea.result != eb.result || content(&a) != reference {
                    failures.push(format!(
                        "{id} {naming:?} {space_name}: differs from the drawingId reference\n  {pa}\n  {:?}\n  {:?}",
                        ea.result, eb.result
                    ));
                }
            }
        }
    }
    eprintln!("{cells} cells");
    assert!(failures.is_empty(), "{} of {cells} cells:\n{}", failures.len(), failures.join("\n"));
}

/// The world with distinct transforms: Bob (the addressed element) and `Other` (made current in most cells) are placed,
/// scaled and (Other only) rotated differently, so a conversion through the wrong element or frame lands elsewhere; the
/// Camera view is zoomed so that a view pixel is half a drawing unit on Bob's plane (the tolerances of the view-space
/// cells are then those of the C4.2a cells).
fn staged() -> (ExecutorHandle, World) {
    let (h, w) = world();
    // The keyed motion of Bob and its parent peg (the fixture animates both; the peg's skew changes over the scene) is
    // removed so Bob's plane is axis-aligned at every frame: a rectangle stays a rectangle in scene space and in the
    // Camera view (the refusal of a rotated rectangle is its own rule, C42A-R2-01).
    ok(&h, "anim.key.remove", json!({ "nodeIds": [BOB, "scene:#1/element:\"Bob-P\""], "all": true }));
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [BOB], "target": "static", "position": { "x": 0.8, "y": -0.4, "z": -2.0 },
                "scale": { "x": 1.25, "y": 0.8 } }),
    );
    ok(
        &h,
        "anim.transform.set",
        json!({ "nodeIds": [w.other], "target": "static", "position": { "x": -3.0, "y": 1.5, "z": 1.0 },
                "scale": { "x": 0.5, "y": 0.5 }, "rotation": 30.0 }),
    );
    let m = mapping(&h, "camera", BOB, 1);
    let ppu = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    let zoom = ok(&h, "view.transform.get", json!({ "viewId": "cameraView" }))["zoom"].as_f64().unwrap();
    ok(&h, "view.zoom.set", json!({ "viewId": "cameraView", "zoom": zoom * 2.0 / ppu }));
    (h, w)
}

/// Drawing units of `element`'s drawing -> `space` (`scene`: scene X and Y of its plane; `camera`: the Camera view's
/// pixels) at `frame`, through `inspect.coordinates.convert` and the scene answer's `cameraToView`: a path apart from
/// the addressing parameters' conversion.
fn mapping(h: &ExecutorHandle, space: &str, element: &str, frame: u64) -> [f64; 6] {
    let to = if space == "scene" { "scene" } else { "image" };
    let r = ok(
        h,
        "inspect.coordinates.convert",
        json!({ "from": "drawing", "to": to, "elementId": element, "frame": frame,
                "points": [{ "x": 0, "y": 0 }, { "x": 1, "y": 0 }, { "x": 0, "y": 1 }] }),
    );
    let p = |i: usize| (r["points"][i]["x"].as_f64().unwrap(), r["points"][i]["y"].as_f64().unwrap());
    let (o, a, b) = (p(0), p(1), p(2));
    let m = [a.0 - o.0, a.1 - o.1, b.0 - o.0, b.1 - o.1, o.0, o.1];
    if space == "scene" {
        return m;
    }
    let scene = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    let v: Vec<f64> = scene["camera"]["cameraToView"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    [
        v[0] * m[0] + v[2] * m[1],
        v[1] * m[0] + v[3] * m[1],
        v[0] * m[2] + v[2] * m[3],
        v[1] * m[2] + v[3] * m[3],
        v[0] * m[4] + v[2] * m[5] + v[4],
        v[1] * m[4] + v[3] * m[5] + v[5],
    ]
}

/// Two answers or documents equal up to the rounding of a conversion and its inverse (1e-6, relative above 1).
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1.0)
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close(p, q)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, p)| y.get(k).is_some_and(|q| close(p, q)))
        }
        _ => a == b,
    }
}

fn same_content(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((ka, va), (kb, vb))| {
            ka == kb && close(&serde_json::from_str(va).unwrap(), &serde_json::from_str(vb).unwrap())
        })
}

/// C43A-R1-02 (02 line 61; section 13.1 items 4 and 5; SI-146): the C4.2a matrix with a new axis. Every point-taking
/// command, by every way of naming its target, with its points in scene space and in the Camera view's pixels,
/// addressed to Bob while another element with another transform is current (in the `drawingId`, `drawingId`-wins and
/// `elementId` + `frame` cells; the `frame`-alone and current cells name the current element by definition), equals the
/// call with `drawingId` and drawing-space points: the same answer and the same saved content, to the rounding of a
/// conversion. Each cell's points are the reference's drawing-space points carried into its space by an independent
/// mapping of the element and frame the naming addresses (`drawingId` without `frame`: the current frame).
#[test]
fn c43a_r1_02_every_point_taking_command_by_every_naming_in_scene_and_camera_view_space() {
    let mut failures = Vec::new();
    let mut cells = 0;
    for (id, addresses, build) in cases() {
        let (b, wb) = staged();
        let mut pb = build(&Coords { mode: Mode::Drawing, m: &wb.m, fwd: IDENTITY }, &wb);
        localise(&mut pb);
        if id == "draw.repositionAll.apply" {
            pb["elementId"] = json!(BOB);
        } else {
            pb["drawingId"] = json!(wb.d2);
        }
        let eb = run(&b, id, pb.clone());
        let reference = content(&b);
        let namings: Vec<Naming> = if addresses || id != "draw.repositionAll.apply" {
            NAMINGS.to_vec()
        } else {
            vec![Naming::ElementFrame, Naming::Current]
        };
        for space in ["scene", "camera"] {
            for naming in &namings {
                cells += 1;
                let (a, w) = staged();
                // The frame the naming addresses Bob at.
                let frame = match naming {
                    Naming::ElementFrame | Naming::FrameAlone | Naming::Current if id != "draw.repositionAll.apply" => {
                        w.frame
                    }
                    _ => 1,
                };
                let fwd = mapping(&a, space, BOB, frame);
                let mut pa = build(&Coords { mode: Mode::Mapped, m: &w.m, fwd }, &w);
                localise(&mut pa);
                if space == "scene" {
                    pa["space"] = json!("scene");
                } else {
                    pa["space"] = json!("view");
                    pa["view"] = json!("cameraView");
                }
                if matches!(naming, Naming::ElementFrame | Naming::DrawingId | Naming::DrawingIdWins) {
                    ok(&a, "app.context.set", json!({ "elementId": w.other, "frame": 1 }));
                }
                if id == "draw.repositionAll.apply" {
                    if matches!(naming, Naming::Current) {
                        pa.as_object_mut().unwrap().remove("elementId");
                        ok(&a, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
                    }
                } else {
                    match naming {
                        Naming::DrawingId => pa["drawingId"] = json!(w.d2),
                        Naming::ElementFrame => {
                            pa["elementId"] = json!(BOB);
                            pa["frame"] = json!(w.frame);
                        }
                        Naming::FrameAlone => pa["frame"] = json!(w.frame),
                        Naming::Current => {
                            ok(&a, "app.context.set", json!({ "elementId": BOB, "frame": w.frame }));
                        }
                        Naming::DrawingIdWins => {
                            pa["drawingId"] = json!(w.d2);
                            pa["elementId"] = json!(BOB);
                            pa["frame"] = json!(1);
                        }
                    }
                }
                let ea = run(&a, id, pa.clone());
                let same = match (&ea.result, &eb.result) {
                    (Some(x), Some(y)) => close(x, y),
                    (x, y) => x == y,
                };
                if !ea.is_ok() {
                    failures.push(format!(
                        "{id} {naming:?} {space}: refused {:?}",
                        ea.error.as_ref().map(|e| (&e.code, &e.message))
                    ));
                } else if !same || !same_content(&content(&a), &reference) {
                    failures.push(format!(
                        "{id} {naming:?} {space}: differs from the drawingId reference\n  {pa}\n  {:?}\n  {:?}",
                        ea.result, eb.result
                    ));
                }
            }
        }
    }
    eprintln!("{cells} cells");
    assert!(failures.is_empty(), "{} of {cells} cells:\n{}", failures.len(), failures.join("\n"));
}

/// C43A-R1-03 (02 lines 61, 101-103; SI-145): with its points in the Camera view's pixels, every point-taking command
/// is unavailable, with the call's own refusal (`inspect.behindCamera`), while the addressed element's plane is behind
/// the camera's near plane or on the camera's plane, and its dry run refuses the same and creates nothing; with the
/// plane in front, neither availability nor the dry run gives that refusal. Both states of every command, availability
/// against a dry run (the sweep's rule, ruling O-24).
#[test]
fn c43a_r1_03_camera_points_on_an_unprojectable_plane_are_unavailable_and_refused() {
    let mut failures = Vec::new();
    let mut passes = 0;
    for (id, addresses, build) in cases() {
        for (state, z) in
            [("in front", None), ("behind the near plane", Some(15.95)), ("on the camera's plane", Some(16.0))]
        {
            let (h, w) = world();
            // The points are made while the plane is in front; then the plane moves (Bob current at frame 1).
            let fwd = mapping(&h, "camera", BOB, 1);
            let mut p = build(&Coords { mode: Mode::Mapped, m: &w.m, fwd }, &w);
            p["space"] = json!("view");
            p["view"] = json!("cameraView");
            // The case's drawing (Bob's later drawing, holding its strokes and text), shown by Bob, the current element.
            if addresses && id != "draw.repositionAll.apply" {
                p["drawingId"] = json!(w.d2);
            }
            if let Some(z) = z {
                ok(&h, "anim.transform.set", json!({ "nodeIds": [BOB], "target": "static", "position": { "z": z } }));
            }
            let before = content(&h);
            let a = h.availability(vec![(id.to_owned(), p.as_object().cloned().unwrap())]).unwrap().remove(0);
            let mut inv = Invocation::user(id, a.params.clone());
            inv.options.dry_run = true;
            let e = h.invoke(inv);
            let code = e.error.as_ref().map(|x| x.code.clone());
            let cell = format!("{id} [{state}]");
            if a.dialog {
                failures.push(format!("{cell}: the reference is not complete"));
                continue;
            }
            if z.is_some() {
                let message = e.error.as_ref().map(|x| x.message.clone());
                if a.enabled || code.as_deref() != Some("inspect.behindCamera") || a.reason != message {
                    failures.push(format!("{cell}: enabled {} ({:?}); dry run {:?}", a.enabled, a.reason, e.error));
                }
                // Nothing is created: the real call refuses the same.
                let r = run(&h, id, p.clone());
                if r.error.as_ref().map(|x| x.code.as_str()) != Some("inspect.behindCamera") || content(&h) != before {
                    failures.push(format!("{cell}: the call {:?} or the document changed", r.error));
                }
            } else {
                if code.as_deref() == Some("inspect.behindCamera")
                    || a.reason.as_deref().is_some_and(|r| r.contains("near plane"))
                {
                    failures.push(format!("{cell}: refused in front: {:?} / {:?}", a.reason, e.error));
                }
                if a.enabled && e.is_ok() {
                    passes += 1;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{passes} of 27 run in front");
    assert!(passes >= 25, "the commands run in front (not only equal refusals): {passes} of 27");
}
