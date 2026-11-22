//! The common addressing parameters of the point-taking `draw.*` commands (02 §13.1 item 5; C42A-R2-01): `space:
//! "drawing" | "scene" | "view" = "drawing"`, `view?`, `frame?`. Points given in view space are converted to drawing
//! space with the view's one mapping (design note 6) before the handler runs, so every handler works in drawing space;
//! `view` stays in the parameters for the view-dependent tolerances (polyline and contour snap radii), which use the
//! given view's zoom, or 100 % without a view. Scene-space points and view-space points of a Camera view reach the
//! current drawing through its element's plane (views note §6, SI-146; M4 C4.3a).

use serde_json::{Value, json};
use tt_registry::{CommandError, codes};

use tt_model::{Id, ObjectType};

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
    ("draw.hitTest.get", &[("/point", Kind::Point)]),
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
    ("draw.pointSelection.set", &[("/rect", Kind::Rect)]),
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

/// The point-taking `colour.*` commands: they convert their points themselves (`commands/colour.rs`), through the same
/// addressed element.
const COLOUR: &[&str] =
    &["colour.paint.apply", "colour.paintUnpainted.apply", "colour.paint.remove", "colour.pick", "colour.gap.close"];

/// The element and frame a point-taking command's scene points, Camera-view points and view-pixel tolerances address
/// (02 §13.1 item 4; SI-146), resolved from the call's own target before any conversion: with `drawingId`, the element
/// showing that drawing (the call's `elementId` if it shows it, else the current element if it does, else the first
/// element of the drawing's bank); else with a qualified object id (`<drawingId>/<objectId>`), the same for that
/// drawing; else `elementId`; else the current element. The frame is the call's `frame`, else the current frame.
/// `None` for other commands, for drawing-space calls without a view, and without any element.
///
/// # Errors
/// The reference errors of `drawingId` and `elementId` (the handler gives the same).
pub(crate) fn target(cx: &Cx, id: &str, params: &Value) -> Result<Option<(Id, u32)>, CommandError> {
    if !FIELDS.iter().any(|(c, _)| *c == id) && !COLOUR.contains(&id) {
        return Ok(None);
    }
    let space = params.get("space").and_then(Value::as_str).unwrap_or("drawing");
    if space == "drawing" && params.get("view").is_none_or(Value::is_null) {
        return Ok(None);
    }
    let frame = params
        .get("frame")
        .and_then(Value::as_u64)
        .and_then(|f| u32::try_from(f).ok())
        .unwrap_or(cx.app().frame)
        .max(1);
    let named = params.get("elementId").and_then(Value::as_str);
    let element = match named {
        Some(r) => Some(Id::parse(&cx.resolve_as(r, ObjectType::Element)?)?),
        None => None,
    };
    let drawing = match params.get("drawingId").and_then(Value::as_str) {
        Some(d) => Some(Id::parse(&cx.resolve_as(d, ObjectType::Drawing)?)?),
        None => qualified_drawing(params),
    };
    let current = cx.app().element.clone();
    let chosen = match drawing {
        Some(d) => {
            let doc = cx.document()?;
            let users = doc.bank_of(&d).map_or(&[][..], |b| doc.bank_users(b));
            element
                .filter(|e| users.contains(e))
                .or_else(|| current.filter(|e| users.contains(e)))
                .or_else(|| users.first().cloned())
        }
        None => element.or(current),
    };
    Ok(chosen.map(|e| (e, frame)))
}

/// The common addressing check (C43A-R1-03): whether a point-taking command's scene points, Camera-view points or
/// view-pixel tolerances reach the addressed drawing: the conversion the call itself makes first
/// ([`normalise`], the colour commands' frames), evaluated without running the call. Availability asks it of every
/// reference of these commands (`host::unavailable`), so a Camera view whose addressed element's plane is at or behind
/// the near plane makes them unavailable with the call's own refusal (`inspect.behindCamera`).
///
/// # Errors
/// As [`target`] and [`crate::camera_view::addressing`].
pub(crate) fn check(cx: &Cx, id: &str, params: &Value) -> Result<(), CommandError> {
    if !FIELDS.iter().any(|(c, _)| *c == id) && !COLOUR.contains(&id) {
        return Ok(());
    }
    let space = params.get("space").and_then(Value::as_str).unwrap_or("drawing");
    let view = params.get("view").and_then(Value::as_str);
    if space == "drawing" && view.is_none() {
        return Ok(());
    }
    let t = target(cx, id, params)?;
    crate::camera_view::addressing_for(cx, space, view, t.as_ref().map(|t| t.1), t.as_ref()).map(|_| ())
}

/// The drawing named by a qualified object id of the call (`strokeId`, `textId`, or the first of `points/*/strokeId`).
fn qualified_drawing(params: &Value) -> Option<Id> {
    let first = params.get("points").and_then(Value::as_array).and_then(|a| a.first()).and_then(|p| p.get("strokeId"));
    ["strokeId", "textId"]
        .iter()
        .filter_map(|k| params.get(*k))
        .chain(first)
        .filter_map(Value::as_str)
        .find_map(|s| s.rsplit_once('/').and_then(|(d, _)| Id::parse(d).ok()))
}

fn invalid(pointer: &str, message: impl Into<String>) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, message, json!({ "pointer": pointer }))
}

/// Converts the view-space points of a point-taking `draw.*` command to drawing space (no change for any other command
/// or for drawing space).
///
/// # Errors
/// As [`crate::camera_view::addressing`]; `arg.invalid` for a rectangle through a rotating or non-axis-aligned map.
pub(crate) fn normalise(cx: &Cx, id: &str, mut params: Value) -> Result<Value, CommandError> {
    let Some((_, fields)) = FIELDS.iter().find(|(c, _)| *c == id) else { return Ok(params) };
    let space = params.get("space").and_then(Value::as_str).unwrap_or("drawing").to_owned();
    if space == "drawing" {
        return Ok(params);
    }
    let view = params.get("view").and_then(Value::as_str).map(str::to_owned);
    let frame = params.get("frame").and_then(Value::as_u64).and_then(|f| u32::try_from(f).ok());
    let (m, _) = crate::camera_view::addressing(cx, &space, view.as_deref(), frame)?;
    let Some(m) = m else { return Ok(params) };
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

/// A tolerance given in view pixels, in drawing units: through the given view's zoom (a Camera view's through the
/// current element's plane), or 100 % without a view (02 §13.1 item 5).
///
/// # Errors
/// As [`crate::camera_view::addressing`] for a named view.
pub(crate) fn tolerance_du(cx: &Cx, view: Option<&str>, px: f64) -> Result<f64, CommandError> {
    match view {
        None => Ok(px),
        Some(v) => Ok(px / crate::camera_view::addressing(cx, "drawing", Some(v), None)?.1),
    }
}
