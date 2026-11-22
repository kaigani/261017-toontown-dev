//! The Drawing view's selection parts for the live preview (preview note §3, PV3): `canvas_drawing` with `part:
//! Selection` holds exactly the selected objects, `part: Rest` the others, and together they hold what `part: All`
//! holds (the same walk, filtered by object id); with nothing selected, `Selection` is empty and `Rest` is everything.

use serde_json::json;
use tt_executor::canvas::{Part, PayloadOptions};
use tt_render::canvas::{CanvasDrawing, Record, decode};

use crate::camera::BOB;
use crate::common::*;

/// The vertex and index counts of a payload's mesh records, and its bitmap records.
fn counts(d: &CanvasDrawing) -> (usize, usize, usize) {
    let mut out = (0, 0, 0);
    for r in &d.records {
        match r {
            Record::Solid { vertices, indices, .. } | Record::Gradient { vertices, indices, .. } => {
                out.0 += vertices.len();
                out.1 += indices.len();
            }
            Record::Bitmap { .. } => out.2 += 1,
        }
    }
    out
}

fn part(h: &tt_executor::ExecutorHandle, part: Part) -> CanvasDrawing {
    decode(&h.canvas_drawing(None, 1, 0, PayloadOptions { part, ..Default::default() }).unwrap().bytes).unwrap()
}

#[test]
fn pv3_the_selection_and_the_rest_partition_the_drawing() {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    // Three separate shapes of the current drawing, each on its own layer.
    let mut ids = Vec::new();
    for (x, y) in [(-300.0, 200.0), (0.0, 200.0), (300.0, 200.0)] {
        let r = ok(
            &h,
            "draw.rectangle.add",
            json!({ "from": { "x": x - 40.0, "y": y - 30.0 }, "to": { "x": x + 40.0, "y": y + 30.0 }, "layer": "new" }),
        );
        ids.push(r["strokeIds"][0].clone());
    }
    let all = part(&h, Part::All);
    // Nothing selected: the selection part is empty, the rest is everything.
    ok(&h, "draw.selection.set", json!({ "by": "none" }));
    assert_eq!(counts(&part(&h, Part::Selection)), (0, 0, 0), "nothing selected: no selected geometry");
    assert_eq!(counts(&part(&h, Part::Rest)), counts(&all), "nothing selected: the rest is the whole drawing");
    // One shape selected.
    ok(&h, "draw.selection.set", json!({ "by": "ids", "ids": [ids[1]], "mode": "replace" }));
    let (sel, rest) = (part(&h, Part::Selection), part(&h, Part::Rest));
    let (cs, cr, ca) = (counts(&sel), counts(&rest), counts(&all));
    assert!(cs.0 > 0 && cr.0 > 0, "both parts hold geometry: {cs:?} {cr:?}");
    assert_eq!((cs.0 + cr.0, cs.1 + cr.1, cs.2 + cr.2), ca, "the two parts together are the whole drawing");
    // The selected part lies on the selected shape: its bounds are within the rectangle around (0, 200), widened by
    // the line's thickness.
    let b = sel.bounds;
    assert!(b[0] >= -50.0 && b[2] <= 50.0 && b[1] >= 160.0 && b[3] <= 240.0, "selected geometry: {b:?}");
    // The rest does not reach the selected shape's interior.
    let inside = |r: &Record| match r {
        Record::Solid { vertices, .. } | Record::Gradient { vertices, .. } => {
            vertices.iter().any(|v| v.x.abs() < 30.0 && (v.y - 200.0).abs() < 20.0)
        }
        Record::Bitmap { .. } => false,
    };
    assert!(!rest.records.iter().any(inside), "the rest holds none of the selected shape");
}
