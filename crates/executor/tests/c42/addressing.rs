//! The common addressing parameters are honoured, not only declared (02 §13.1 items 4 and 5; C42A-R3-01): for every
//! point-taking `draw.*` and `colour.*` command, the call with `frame` addressing a drawing exposed at another frame
//! (the context stays at frame 1) and `space: "view"` points in a zoomed, panned view equals the same call with an
//! explicit `drawingId` and drawing-space points: the same answer and the same canonical saved bytes.

use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

use crate::common::*;
use crate::equivalence::{BOB, D1, canonical, ready};

struct World {
    frame: u64,
    d2: String,
    stroke: String,
    text: String,
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
    let m: Vec<f64> = ok(&h, "draw.viewMapping.get", json!({ "view": "drawing" }))["viewToDrawing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    assert_eq!(ok(&h, "app.state.get", json!({}))["frame"], 1, "the context stays at frame 1");
    (h, World { frame, d2, stroke, text, m })
}

/// How a case writes its coordinates: view space (the call under test) or drawing space (the reference).
#[derive(Clone, Copy)]
enum Mode {
    View,
    Drawing,
}

struct Coords<'a> {
    mode: Mode,
    m: &'a [f64],
}

impl Coords<'_> {
    /// A point given in view pixels, written in the mode's space.
    fn p(&self, x: f64, y: f64) -> Value {
        match self.mode {
            Mode::View => json!({ "x": x, "y": y }),
            Mode::Drawing => {
                json!({ "x": self.m[0] * x + self.m[2] * y + self.m[4], "y": self.m[1] * x + self.m[3] * y + self.m[5] })
            }
        }
    }

    /// A vector given in view pixels.
    fn v(&self, x: f64, y: f64) -> Value {
        match self.mode {
            Mode::View => json!({ "x": x, "y": y }),
            Mode::Drawing => json!({ "x": self.m[0] * x + self.m[2] * y, "y": self.m[1] * x + self.m[3] * y }),
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
        ("colour.paint.apply", true, |c, _| json!({ "points": [c.p(1000.0, 520.0)] })),
        ("colour.paintUnpainted.apply", true, |c, _| json!({ "points": [c.p(1000.0, 520.0)] })),
        ("colour.paint.remove", true, |c, _| json!({ "points": [c.p(1000.0, 520.0)] })),
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
        let mut pa = build(&Coords { mode: Mode::View, m: &w.m }, &w);
        pa["space"] = json!("view");
        pa["view"] = json!("drawingView");
        let mut pb = build(&Coords { mode: Mode::Drawing, m: &wb.m }, &wb);
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
        if !same_answer || canonical(&a) != canonical(&b) {
            failures.push(format!(
                "{id}: frame/view {pa} -> {:?} {:?}\n  drawingId/drawing {pb} -> {:?} {:?}",
                ea.result, ea.error, eb.result, eb.error
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
