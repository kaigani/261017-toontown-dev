//! The common addressing parameters of the point-taking `draw.*` commands (02 §13.1 item 5; C42A-R2-01): `space:
//! "drawing" | "scene" | "view" = "drawing"`, `view?`, `frame?`. Points given in view space are converted to drawing
//! space with the view's one mapping (design note 6) before the handler runs, so every handler works in drawing space;
//! `view` stays in the parameters for the view-dependent tolerances (polyline and contour snap radii), which use the
//! given view's zoom, or 100 % without a view. Scene space waits for the scene transforms of the Camera view, as it
//! does in section 03's commands.

use serde_json::{Value, json};
use tt_registry::{CommandError, codes};

use crate::cx::Cx;

/// How a field converts: a point (the whole affine), a vector or relative handle (its linear part), or an axis-aligned
/// rectangle `{x, y, width, height}` (only without rotation).
#[derive(Debug, Clone, Copy)]
enum Kind {
    Point,
    Vector,
    Rect,
}

/// The point fields of each point-taking `draw.*` command; `*` stands for every element of an array.
const FIELDS: &[(&str, &[(&str, Kind)])] = &[
    ("draw.pivot.set", &[("/point", Kind::Point)]),
    (
        "draw.cut.apply",
        &[
            ("/polygon/*", Kind::Point),
            ("/rect", Kind::Rect),
            ("/segment/from", Kind::Point),
            ("/segment/to", Kind::Point),
        ],
    ),
    ("draw.erase.apply", &[("/path/*", Kind::Point), ("/region/*", Kind::Point)]),
    ("draw.selection.set", &[("/point", Kind::Point), ("/rect", Kind::Rect), ("/polygon/*", Kind::Point)]),
    ("draw.rectangle.add", &[("/from", Kind::Point), ("/to", Kind::Point)]),
    ("draw.ellipse.add", &[("/from", Kind::Point), ("/to", Kind::Point)]),
    ("draw.line.add", &[("/from", Kind::Point), ("/to", Kind::Point)]),
    (
        "draw.polyline.add",
        &[("/points/*", Kind::Point), ("/points/*/inHandle", Kind::Vector), ("/points/*/outHandle", Kind::Vector)],
    ),
    ("draw.stroke.add", &[("/points/*", Kind::Point), ("/bezier/*", Kind::Point)]),
    ("draw.stroke.set", &[("/bezier/*", Kind::Point)]),
    ("draw.content.add", &[("/offset", Kind::Vector)]),
    ("draw.text.add", &[("/position", Kind::Point)]),
    ("draw.text.set", &[("/position", Kind::Point)]),
    ("draw.transform.apply", &[("/translate", Kind::Vector), ("/pivot", Kind::Point)]),
    ("draw.envelope.apply", &[("/corners/*", Kind::Point)]),
    ("draw.repositionAll.apply", &[("/translate", Kind::Vector), ("/pivot", Kind::Point), ("/corners/*", Kind::Point)]),
    ("draw.point.add", &[("/at", Kind::Point)]),
    ("draw.point.move", &[("/delta", Kind::Vector)]),
    ("draw.point.set", &[("/position", Kind::Point), ("/inHandle", Kind::Vector), ("/outHandle", Kind::Vector)]),
    ("draw.segment.move", &[("/delta", Kind::Vector)]),
];

fn invalid(pointer: &str, message: impl Into<String>) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, message, json!({ "pointer": pointer }))
}

/// Converts the view-space points of a point-taking `draw.*` command to drawing space (no change for any other command
/// or for drawing space).
///
/// # Errors
/// `arg.invalid` for scene space (the Camera view's, M4 C4.3) and for a rectangle in a rotated view;
/// `view.notFound` / `command.notImplemented` for a view that is not a Drawing view window.
pub(crate) fn normalise(cx: &Cx, id: &str, mut params: Value) -> Result<Value, CommandError> {
    let Some((_, fields)) = FIELDS.iter().find(|(c, _)| *c == id) else { return Ok(params) };
    let space = params.get("space").and_then(Value::as_str).unwrap_or("drawing").to_owned();
    match space.as_str() {
        "drawing" => return Ok(params),
        "view" => {}
        _ => {
            return Err(invalid("/space", "scene-space points need the scene transforms of the Camera view (M4 C4.3)")
                .hint("give points in drawing or view space"));
        }
    }
    let view = params.get("view").and_then(Value::as_str).map(str::to_owned);
    let w = crate::commands::view::drawing_view(cx, view.as_deref())?;
    let m = crate::commands::view::view_to_drawing(w);
    for (path, kind) in *fields {
        convert(&mut params, &path[1..], *kind, &m, path)?;
    }
    if let Some(obj) = params.as_object_mut() {
        obj.insert("space".into(), json!("drawing"));
    }
    Ok(params)
}

fn convert(v: &mut Value, rest: &str, kind: Kind, m: &[f64; 6], path: &str) -> Result<(), CommandError> {
    let (head, tail) = rest.split_once('/').map_or((rest, None), |(h, t)| (h, Some(t)));
    if head == "*" {
        if let Some(items) = v.as_array_mut() {
            for item in items {
                match tail {
                    Some(t) => convert(item, t, kind, m, path)?,
                    None => leaf(item, kind, m, path)?,
                }
            }
        }
        return Ok(());
    }
    let Some(next) = v.get_mut(head) else { return Ok(()) };
    match tail {
        Some(t) => convert(next, t, kind, m, path),
        None => leaf(next, kind, m, path),
    }
}

fn num(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

fn leaf(v: &mut Value, kind: Kind, m: &[f64; 6], path: &str) -> Result<(), CommandError> {
    if v.is_null() {
        return Ok(());
    }
    match kind {
        Kind::Point | Kind::Vector => {
            let (Some(x), Some(y)) = (num(v, "x"), num(v, "y")) else { return Ok(()) };
            let (e, f) = if matches!(kind, Kind::Point) { (m[4], m[5]) } else { (0.0, 0.0) };
            v["x"] = json!(m[0] * x + m[2] * y + e);
            v["y"] = json!(m[1] * x + m[3] * y + f);
        }
        Kind::Rect => {
            let (Some(x), Some(y), Some(w), Some(h)) = (num(v, "x"), num(v, "y"), num(v, "width"), num(v, "height"))
            else {
                return Ok(());
            };
            if m[1].abs() > 1e-12 || m[2].abs() > 1e-12 {
                return Err(invalid(path, "a rectangle cannot be given in a rotated view; give a polygon"));
            }
            let (ax, ay) = (m[0] * x + m[4], m[3] * y + m[5]);
            let (bx, by) = (m[0] * (x + w) + m[4], m[3] * (y + h) + m[5]);
            *v = json!({ "x": ax.min(bx), "y": ay.min(by), "width": (ax - bx).abs(), "height": (ay - by).abs() });
        }
    }
    Ok(())
}

/// A tolerance given in view pixels, in drawing units: through the given view's zoom, or 100 % without a view
/// (02 §13.1 item 5).
///
/// # Errors
/// As [`crate::commands::view::drawing_view`] for a named view.
pub(crate) fn tolerance_du(cx: &Cx, view: Option<&str>, px: f64) -> Result<f64, CommandError> {
    match view {
        None => Ok(px),
        Some(v) => Ok(px / crate::commands::view::pixels_per_du(crate::commands::view::drawing_view(cx, Some(v))?)),
    }
}
