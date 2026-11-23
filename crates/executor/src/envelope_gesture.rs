//! M4 C4.4b, the Perspective and Reposition All Drawings tools' gestures (02 §7.1.3 lines 465-473, §7.1.4 lines
//! 475-482, §13.3 lines 1227-1228; design note 4 §2-§4; SI-188, SI-189): a gesture becomes the command an agent sends
//! for the same edit. Both tools take their handles from one box the kernel computes ([`perspective_box`],
//! [`reposition_box`]), which the command maps and the scene answer draws (`toolBox`), so the handles drawn, hit and
//! mapped cannot differ.

use serde_json::{Value, json};
use tt_model::entities::drawing::Drawing;
use tt_registry::CommandError;
use tt_vector::geom::Rect;

use crate::cx::Cx;
use crate::gesture::{Gesture, Handle, Sample, Stroke};
use crate::shell::Point;

/// The Perspective tool's id.
pub const PERSPECTIVE: &str = "perspective";
/// The Reposition All Drawings tool's id.
pub const REPOSITION: &str = "repositionAll";

fn pt(p: Point) -> Value {
    json!({ "x": p.x, "y": p.y })
}

/// The current drawing.
fn current(cx: &Cx) -> Option<Drawing> {
    let doc = cx.document().ok()?;
    let id = cx.app().current_drawing(doc)?;
    doc.drawing(&id).cloned()
}

/// Perspective's box: the bounds of what `draw.envelope.apply` maps (the selection's objects, a zone standing for its
/// strokes, a group counting its invisible strokes), `None` with nothing selected.
#[must_use]
pub(crate) fn perspective_box(cx: &Cx, d: &Drawing) -> Option<Rect> {
    crate::commands::draw_transform::envelope_box(cx, d)
}

/// Reposition All's box: the current drawing's bounds, invisible strokes included (SI-189); `None` when it is empty. A
/// drawing with text waits for the font list within the bound (ruling E-43).
pub(crate) fn reposition_box(d: &Drawing) -> Result<Option<Rect>, CommandError> {
    crate::fonts_gate::drawing_bounds(d, true)
}

/// The scene answer's `toolBox` (design note 4 §2): the active tool's box and whether its command is refused now
/// (Perspective: a locked drawing; Reposition All: nothing it can move, its precheck).
pub(crate) fn scene_tool_box(cx: &Cx) -> Option<crate::canvas::SceneToolBox> {
    let tool = cx.ui().drawing().active_tool.clone();
    let d = current(cx)?;
    let (b, locked) = match tool.as_str() {
        PERSPECTIVE => (perspective_box(cx, &d)?, *d.locked()),
        REPOSITION => {
            let refused = crate::commands::draw_transform::precheck_reposition_all(cx, &json!({})).is_err();
            // The scene answer never waits for the font list (ruling E-43): no box until it is read.
            (crate::fonts_gate::drawing_bounds_now(&d, true).flatten()?, refused)
        }
        _ => return None,
    };
    let mapping = (tool == PERSPECTIVE).then(|| {
        cx.ui().drawing().options_of(PERSPECTIVE).get("mapping").and_then(Value::as_str).unwrap_or("auto").to_owned()
    });
    Some(crate::canvas::SceneToolBox { tool, bbox: [b.min_x, b.min_y, b.max_x, b.max_y], locked, mapping })
}

/// Corners of `b`, bottom-left first, counter-clockwise (drawing space), as `draw.envelope.apply` takes them.
fn corners(b: &Rect) -> [Point; 4] {
    [
        Point { x: b.min_x, y: b.min_y },
        Point { x: b.max_x, y: b.min_y },
        Point { x: b.max_x, y: b.max_y },
        Point { x: b.min_x, y: b.max_y },
    ]
}

/// The envelope corners of a handle drag by `d` (SI-188): a corner follows the drag; a side's two corners follow it;
/// with `symmetric`, a side's corners spread or close about its midpoint (the corner with the larger coordinate along
/// the side follows the drag's part along the side, the other mirrors it) and both take the perpendicular part.
fn envelope_corners(h: Handle, b: &Rect, d: Point, symmetric: bool) -> Option<Vec<Value>> {
    let mut c = corners(b);
    let add = |p: Point, x: f64, y: f64| Point { x: p.x + x, y: p.y + y };
    match h {
        Handle::Corner(i) => c[i] = add(c[i], d.x, d.y),
        Handle::Side(i) if !symmetric => {
            let j = (i + 1) % 4;
            c[i] = add(c[i], d.x, d.y);
            c[j] = add(c[j], d.x, d.y);
        }
        Handle::Side(i) => {
            let j = (i + 1) % 4;
            // Sides 0 (bottom) and 2 (top) run along x; 1 (right) and 3 (left) along y. Outward normals: -y, +x, +y, -x.
            let along_x = i % 2 == 0;
            let (s, t) = match i {
                0 => (d.x, -d.y),
                1 => (d.y, d.x),
                2 => (d.x, d.y),
                _ => (d.y, -d.x),
            };
            let (ux, uy) = if along_x { (1.0, 0.0) } else { (0.0, 1.0) };
            let (nx, ny) = match i {
                0 => (0.0, -1.0),
                1 => (1.0, 0.0),
                2 => (0.0, 1.0),
                _ => (-1.0, 0.0),
            };
            let coord = |p: Point| if along_x { p.x } else { p.y };
            let (hi, lo) = if coord(c[i]) >= coord(c[j]) { (i, j) } else { (j, i) };
            c[hi] = add(c[hi], s * ux + t * nx, s * uy + t * ny);
            c[lo] = add(c[lo], -s * ux + t * nx, -s * uy + t * ny);
        }
        Handle::Rotate => return None,
    }
    Some(c.iter().map(|p| pt(*p)).collect())
}

/// The press mapping drawing units -> view pixels, as the Select tool's handles (SI-156).
fn view_of<'a>(cx: &'a Cx, g: &Gesture) -> Result<impl Fn(Point) -> Point + 'a, CommandError> {
    let w = cx.ui().shell().view(&g.view_id)?;
    let camera = w.ty == crate::shell::ViewType::Camera;
    let inv = crate::camera_view::invert(&g.press);
    Ok(move |p: Point| match inv {
        Some(m) if camera => crate::shell::apply(&m, p),
        _ => crate::commands::view::to_view(w, p),
    })
}

/// Perspective (design note 4 §3): a handle drag is `draw.envelope.apply` with the tool's `mapping` explicit; anything
/// else selects as the Select tool does.
///
/// # Errors
/// The view's refusal.
pub(crate) fn perspective(
    cx: &Cx,
    g: &Gesture,
    st: &Stroke,
    p0: Sample,
    pn: Sample,
) -> Result<Option<(String, Value)>, CommandError> {
    if !st.is_click()
        && let Some(d) = current(cx)
        && let Some(b) = perspective_box(cx, &d)
    {
        let view = view_of(cx, g)?;
        if let Some(h) = crate::gesture::handle_at(&view, &b, p0, false) {
            let delta = Point { x: pn.d.x - p0.d.x, y: pn.d.y - p0.d.y };
            let mapping = cx.ui().drawing().options_of(PERSPECTIVE).get("mapping").cloned().unwrap_or(json!("auto"));
            if let Some(corners) = envelope_corners(h, &b, delta, st.mods.alternate) {
                return Ok(Some(("draw.envelope.apply".into(), json!({ "corners": corners, "mapping": mapping }))));
            }
        }
    }
    crate::gesture::select_by_area(cx, g, st, p0, pn)
}

/// Reposition All Drawings (design note 4 §4): one `draw.repositionAll.apply` per drag for every drawing of the
/// current element; a click does nothing.
///
/// # Errors
/// The view's refusal; a handle drag scaling below the minimum (`arg.invalid`, as Select's).
pub(crate) fn reposition_all(
    cx: &Cx,
    g: &Gesture,
    st: &Stroke,
    p0: Sample,
    pn: Sample,
) -> Result<Option<(String, Value)>, CommandError> {
    if st.is_click() {
        return Ok(None);
    }
    let delta = Point { x: pn.d.x - p0.d.x, y: pn.d.y - p0.d.y };
    let offset = || Ok(Some(("draw.repositionAll.apply".into(), json!({ "translate": pt(delta) }))));
    let b = match current(cx) {
        Some(d) => reposition_box(&d)?,
        None => None,
    };
    let Some(b) = b else { return offset() };
    let view = view_of(cx, g)?;
    let Some(h) = crate::gesture::handle_at(&view, &b, p0, true) else { return offset() };
    if st.mods.alternate {
        // Envelope handles (SI-189): the corner, a plain side, or with constrain the symmetric side.
        let symmetric = st.mods.constrain && matches!(h, Handle::Side(_));
        return match envelope_corners(h, &b, delta, symmetric) {
            Some(corners) => Ok(Some(("draw.repositionAll.apply".into(), json!({ "corners": corners })))),
            None => offset(),
        };
    }
    let (_, v) = crate::gesture::handle_drag(h, &b, p0, pn, st.mods.constrain)?;
    Ok(Some(("draw.repositionAll.apply".into(), v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect { min_x: 0.0, min_y: 0.0, max_x: 100.0, max_y: 50.0 }
    }

    fn xy(v: &Value) -> (f64, f64) {
        (v["x"].as_f64().unwrap_or(f64::NAN), v["y"].as_f64().unwrap_or(f64::NAN))
    }

    /// SI-188: a symmetric drag on the top side spreads its corners about the midpoint and lifts both.
    #[test]
    fn the_symmetric_side_drag_mirrors_along_the_side() {
        let c = envelope_corners(Handle::Side(2), &rect(), Point { x: 10.0, y: 4.0 }, true).unwrap_or_default();
        let got: Vec<(f64, f64)> = c.iter().map(xy).collect();
        assert_eq!(got, vec![(0.0, 0.0), (100.0, 0.0), (110.0, 54.0), (-10.0, 54.0)]);
        // The left side (3): along y, outward -x.
        let c = envelope_corners(Handle::Side(3), &rect(), Point { x: -3.0, y: 5.0 }, true).unwrap_or_default();
        let got: Vec<(f64, f64)> = c.iter().map(xy).collect();
        assert_eq!(got, vec![(-3.0, -5.0), (100.0, 0.0), (100.0, 50.0), (-3.0, 55.0)]);
    }

    #[test]
    fn corner_and_plain_side_drags() {
        let c = envelope_corners(Handle::Corner(1), &rect(), Point { x: 7.0, y: -2.0 }, false).unwrap_or_default();
        assert_eq!(xy(&c[1]), (107.0, -2.0));
        let c = envelope_corners(Handle::Side(1), &rect(), Point { x: 7.0, y: -2.0 }, false).unwrap_or_default();
        assert_eq!((xy(&c[1]), xy(&c[2])), ((107.0, -2.0), (107.0, 48.0)));
        assert!(envelope_corners(Handle::Rotate, &rect(), Point { x: 1.0, y: 1.0 }, false).is_none());
    }
}
