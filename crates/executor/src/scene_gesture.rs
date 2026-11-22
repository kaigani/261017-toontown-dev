//! The scene tools' gestures in the Camera view (05 §3.5, §5.0-§5.2; gestures note §7; SI-150; M4 C4.3a): each ends in
//! exactly one command or none. Select (05 §5.1) writes static values and never keys; Transform (05 §5.2) keys the
//! changed channels at the current frame; a pivot drag moves the static pivot (no key).

use serde_json::{Value, json};
use tt_model::Id;
use tt_registry::CommandError;
use tt_vector::geom::P;

use crate::camera_view::{self, Chain, Matrix};
use crate::commands::anim_common::{eval_error, evaluator};
use crate::commands::render_list::{self, Overrides, RenderList, Request, Verdict};
use crate::cx::Cx;
use crate::gesture::{Gesture, HANDLE_PX, InputSpace, ROTATE_PX, Sample};
use crate::shell::Point;

/// The gesture tool name of a scene tool (distinct from the drawing tools': `select` is both, SI-148).
pub(crate) const SCENE_SELECT: &str = "scene:select";
pub(crate) const SCENE_TRANSFORM: &str = "scene:transform";
/// The Transform tool's temporary Skew (05 §5.0 line 394; SI-151): a gesture tool, never a stored scene tool.
pub(crate) const SCENE_SKEW: &str = "scene:skew";

/// The pick tolerance in view pixels (05 §3.5).
pub(crate) const PICK_PX: f64 = 4.0;

/// Skew handles sit this far outside each side's midpoint (SI-150).
pub(crate) const SKEW_OUT_PX: f64 = 12.0;

/// One element of the frame as the view shows it.
struct Shown {
    id: Id,
    /// Its box in view pixels (axis-aligned around the transformed drawing bounds).
    view_box: [f64; 4],
}

/// The furniture handle a drag starts on (SI-150).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Handle {
    /// Corner `i`: 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left (view pixels, y down).
    Corner(usize),
    /// Side `i`: 0 top, 1 right, 2 bottom, 3 left.
    Side(usize),
    /// The skew handle outside side `i`.
    Skew(usize),
    Rotate,
    Pivot,
}

fn dist(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn rect_of(m: &Matrix, r: &tt_vector::geom::Rect) -> [f64; 4] {
    let c = [(r.min_x, r.min_y), (r.max_x, r.min_y), (r.max_x, r.max_y), (r.min_x, r.max_y)];
    let pts: Vec<Point> = c.iter().map(|&(x, y)| camera_view::apply(m, Point { x, y })).collect();
    let (mut b0, mut b1) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for q in pts {
        b0 = [b0[0].min(q.x), b0[1].min(q.y)];
        b1 = [b1[0].max(q.x), b1[1].max(q.y)];
    }
    [b0[0], b0[1], b1[0], b1[1]]
}

/// The frame's render list through the view's camera: built once per gesture and shared by the furniture and the
/// pick (each node evaluated once).
fn render_list(cx: &Cx, chain: &Chain) -> Result<RenderList, CommandError> {
    let doc = cx.document()?;
    let overrides = Overrides::default();
    let req = Request {
        scene: &chain.scene,
        frame: chain.frame,
        camera: None,
        width: chain.width,
        height: chain.height,
        only: None,
        overrides: &overrides,
    };
    let mut ev = evaluator(doc, "picking in the Camera view");
    render_list::build(doc, &req, &mut ev)
}

/// The elements the view shows (drawn or around the frame), front-most last, with their boxes in view pixels.
fn shown(chain: &Chain, list: &RenderList) -> Vec<Shown> {
    list.entries
        .iter()
        .filter(|en| matches!(en.verdict, Verdict::Drawn | Verdict::OutOfFrame))
        .filter_map(|en| {
            let (m, b) = (en.to_pixel?, en.bounds?);
            Some(Shown { id: en.id.clone(), view_box: rect_of(&camera_view::compose(&chain.output_to_view, &m.0), &b) })
        })
        .collect()
}

fn selected(cx: &Cx) -> Vec<Id> {
    cx.app().selection_list("staging").iter().filter_map(|k| Id::parse(k).ok()).collect()
}

fn locked(cx: &Cx, id: &Id) -> bool {
    cx.document().ok().and_then(|d| d.element(id)).is_some_and(|e| *e.locked())
}

/// The front-most unlocked element under view point `v` within [`PICK_PX`] (05 §3.5; SI-150), from one render list
/// (each node evaluated once per gesture).
fn pick(cx: &Cx, chain: &Chain, list: &RenderList, v: Point) -> Result<Option<Id>, CommandError> {
    let inv = camera_view::invert(&chain.output_to_view)
        .ok_or_else(|| crate::commands::common::invalid("/view", "the Camera view's transform is singular"))?;
    let doc = cx.document()?;
    let ring = (0..8).map(|k| {
        let a = f64::from(k) * std::f64::consts::FRAC_PI_4;
        Point { x: v.x + PICK_PX * a.cos(), y: v.y + PICK_PX * a.sin() }
    });
    for q in std::iter::once(v).chain(ring) {
        let o = camera_view::apply(&inv, q);
        for h in crate::commands::inspect::hits_in(cx, doc, list, P::new(o.x, o.y), true)? {
            if let Ok(id) = Id::parse(h.element_id())
                && !locked(cx, &id)
            {
                return Ok(Some(id));
            }
        }
    }
    Ok(None)
}

/// The first selected element's rotation pivot in view pixels (the Transform tool's pivot marker, SI-150).
fn pivot_marker(cx: &Cx, chain: &Chain, id: &Id) -> Result<Option<Point>, CommandError> {
    let doc = cx.document()?;
    let p = doc
        .element(id)
        .and_then(|e| e.transform().as_ref().and_then(|t| t.rotation_pivot.as_ref()).map(|q| (q.x, q.y)))
        .unwrap_or((0.0, 0.0));
    let mut ev = evaluator(doc, "picking in the Camera view");
    let world = ev.world(id, f64::from(chain.frame)).map_err(eval_error)?;
    Ok(camera_view::scene_to_output(chain, world.apply([p.0, p.1, 0.0]))
        .map(|o| camera_view::apply(&chain.output_to_view, o)))
}

fn corners(b: &[f64; 4]) -> [Point; 4] {
    [Point { x: b[0], y: b[1] }, Point { x: b[2], y: b[1] }, Point { x: b[2], y: b[3] }, Point { x: b[0], y: b[3] }]
}

fn midpoints(b: &[f64; 4]) -> [Point; 4] {
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    [Point { x: cx, y: b[1] }, Point { x: b[2], y: cy }, Point { x: cx, y: b[3] }, Point { x: b[0], y: cy }]
}

fn skew_handles(b: &[f64; 4]) -> [Point; 4] {
    let m = midpoints(b);
    [
        Point { x: m[0].x, y: m[0].y - SKEW_OUT_PX },
        Point { x: m[1].x + SKEW_OUT_PX, y: m[1].y },
        Point { x: m[2].x, y: m[2].y + SKEW_OUT_PX },
        Point { x: m[3].x - SKEW_OUT_PX, y: m[3].y },
    ]
}

/// The handle under `p` (SI-150), tested in order: the pivot marker, corners, sides, skew handles, the rotation zone.
fn handle_at(b: &[f64; 4], pivot: Option<Point>, p: Point) -> Option<Handle> {
    if pivot.is_some_and(|q| dist(q, p) <= HANDLE_PX) {
        return Some(Handle::Pivot);
    }
    let c = corners(b);
    if let Some(i) = (0..4).find(|&i| dist(c[i], p) <= HANDLE_PX) {
        return Some(Handle::Corner(i));
    }
    let m = midpoints(b);
    if let Some(i) = (0..4).find(|&i| dist(m[i], p) <= HANDLE_PX) {
        return Some(Handle::Side(i));
    }
    let k = skew_handles(b);
    if let Some(i) = (0..4).find(|&i| dist(k[i], p) <= HANDLE_PX) {
        return Some(Handle::Skew(i));
    }
    let inside = p.x >= b[0] && p.x <= b[2] && p.y >= b[1] && p.y <= b[3];
    (!inside && c.iter().any(|&q| dist(q, p) <= ROTATE_PX)).then_some(Handle::Rotate)
}

fn union(boxes: impl Iterator<Item = [f64; 4]>) -> Option<[f64; 4]> {
    boxes.reduce(|u, b| [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])])
}

/// The factor along one axis about `pivot`: 1 when the press is on the pivot's line.
fn factor(press: f64, release: f64, pivot: f64) -> f64 {
    let den = press - pivot;
    if den.abs() < 1e-9 { 1.0 } else { (release - pivot) / den }
}

/// The command `anim.transform.set` with the tool's target (Select: static, 05 §5.1; Transform: keyed at the current
/// frame, 05 §5.2) over `nodes`, plus `fields`.
fn transform(transform_tool: bool, frame: u32, nodes: &[Id], fields: Value) -> (String, Value) {
    let mut v = json!({
        "nodeIds": nodes.iter().map(|n| n.as_str()).collect::<Vec<_>>(),
        "target": if transform_tool { "key" } else { "static" },
    });
    if transform_tool {
        v["frame"] = json!(frame);
    }
    if let (Some(o), Some(f)) = (v.as_object_mut(), fields.as_object()) {
        o.extend(f.clone());
    }
    ("anim.transform.set".to_owned(), v)
}

/// A view-pixel delta at a node's depth → a parent-space delta (fields) of that node (05 §3.5; SI-150).
fn parent_delta(cx: &Cx, chain: &Chain, node: &Id, a: Point, b: Point) -> Result<(f64, f64), CommandError> {
    let doc = cx.document()?;
    let mut ev = evaluator(doc, "moving in the Camera view");
    let f = f64::from(chain.frame);
    let world = ev.world(node, f).map_err(eval_error)?;
    let depth = chain.view.project(world.translation()).depth;
    if depth <= tt_anim::camera::NEAR_PLANE {
        return Err(camera_view::behind(node));
    }
    let inv = camera_view::invert(&chain.output_to_view)
        .ok_or_else(|| crate::commands::common::invalid("/view", "the Camera view's transform is singular"))?;
    // The delta is a vector: it goes through the linear parts only (view -> output pixels -> camera space at the
    // node's depth, 05 §2.6 -> world -> parent), never as the difference of two converted points, so a round drag
    // gives a round delta (ruling D-28's note).
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let out = (inv[0] * dx + inv[2] * dy, inv[1] * dx + inv[3] * dy);
    let k = 2.0 * depth * tt_anim::num::tan_deg(chain.view.fov / 2.0) / chain.view.width;
    let camera_delta = [out.0 * k, -out.1 * k, 0.0];
    let to_world = chain.view.view.inverse().ok_or_else(|| camera_view::behind(node))?;
    let parent = ev.parent_world(node, f).map_err(eval_error)?.inverse().ok_or_else(|| {
        CommandError::with_details(
            "anim.valueOutOfRange",
            "the parent's world matrix is singular",
            json!({ "nodeId": node.as_str() }),
        )
    })?;
    let p = linear(&parent, linear(&to_world, camera_delta));
    Ok((p[0], p[1]))
}

/// A 4 x 4 matrix's linear part applied to a vector (row-major, as `Mat4::apply` without the translation).
fn linear(m: &tt_anim::matrix::Mat4, v: [f64; 3]) -> [f64; 3] {
    let m = &m.0;
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        m[4] * v[0] + m[5] * v[1] + m[6] * v[2],
        m[8] * v[0] + m[9] * v[1] + m[10] * v[2],
    ]
}

/// The command of a scene-tool gesture in a Camera view (gestures note §7's table), or none.
///
/// # Errors
/// The chain's and the evaluator's refusals; `inspect.behindCamera`.
pub(crate) fn commands(cx: &Cx, g: &Gesture) -> Result<Option<(String, Value)>, CommandError> {
    let skew_tool = g.tool == SCENE_SKEW;
    // The Skew tool keys as the Transform tool does (05 §5.0 line 397: a keyframing tool).
    let transform_tool = g.tool == SCENE_TRANSFORM || skew_tool;
    let Some(st) = g.strokes.first() else { return Ok(None) };
    let (Some(p0), Some(pn)) = (st.samples.first().copied(), st.samples.last().copied()) else { return Ok(None) };
    let w = cx.ui().shell().view(&g.view_id)?.clone();
    let chain = camera_view::chain(cx, &w)?;
    let sel = selected(cx);
    let list = render_list(cx, &chain)?;
    let all = shown(&chain, &list);
    let union = union(all.iter().filter(|s| sel.contains(&s.id)).map(|s| s.view_box));
    let is_click = st.is_click();
    let extend = st.mods.constrain;
    // The Skew tool (SI-151): a drag from a side (its midpoint or the skew handle outside it) shears; any other drag
    // commits nothing; a click selects as the Transform tool's click does.
    if skew_tool && !is_click {
        let Some(b) = union else { return Ok(None) };
        let d = Drag { cx, chain: &chain, transform_tool, sel: &sel, b, p0, pn, constrain: st.mods.constrain };
        return match handle_at(&b, None, p0.v) {
            Some(Handle::Side(i) | Handle::Skew(i)) if i % 2 == 0 => Ok(Some(handle_drag(&d, Handle::Skew(i))?)),
            Some(Handle::Side(i) | Handle::Skew(i)) => vertical_shear(&d, i),
            _ => Ok(None),
        };
    }
    if !is_click && let Some(b) = union {
        let pivot = if transform_tool { pivot_marker(cx, &chain, &sel[0])? } else { None };
        if let Some(h) = handle_at(&b, pivot, p0.v) {
            let d = Drag { cx, chain: &chain, transform_tool, sel: &sel, b, p0, pn, constrain: st.mods.constrain };
            return Ok(Some(handle_drag(&d, h)?));
        }
        if p0.v.x >= b[0] && p0.v.x <= b[2] && p0.v.y >= b[1] && p0.v.y <= b[3] {
            let (dx, dy) = parent_delta(cx, &chain, &sel[0], p0.v, pn.v)?;
            return Ok(Some(transform(
                transform_tool,
                chain.frame,
                &sel,
                json!({ "position": { "x": dx, "y": dy }, "relative": true }),
            )));
        }
    }
    let ids: Vec<String> = if is_click {
        pick(cx, &chain, &list, p0.v)?.map(|id| vec![id.as_str().to_owned()]).unwrap_or_default()
    } else {
        // A rubber band: every unlocked element whose box touches the rectangle (05 §5.0; SI-150).
        let r = [p0.v.x.min(pn.v.x), p0.v.y.min(pn.v.y), p0.v.x.max(pn.v.x), p0.v.y.max(pn.v.y)];
        all.iter()
            .filter(|s| {
                s.view_box[0] <= r[2] && s.view_box[2] >= r[0] && s.view_box[1] <= r[3] && s.view_box[3] >= r[1]
            })
            .filter(|s| !locked(cx, &s.id))
            .map(|s| s.id.as_str().to_owned())
            .collect()
    };
    let mode = if extend { if is_click { "toggle" } else { "add" } } else { "replace" };
    if extend && ids.is_empty() {
        return Ok(None);
    }
    Ok(Some(("edit.selection.set".to_owned(), json!({ "context": "staging", "ids": ids, "mode": mode }))))
}

/// One handle drag's inputs.
struct Drag<'a> {
    cx: &'a Cx,
    chain: &'a Chain,
    transform_tool: bool,
    sel: &'a [Id],
    b: [f64; 4],
    p0: Sample,
    pn: Sample,
    constrain: bool,
}

fn handle_drag(d: &Drag<'_>, h: Handle) -> Result<(String, Value), CommandError> {
    let Drag { cx, chain, transform_tool, sel, p0, pn, constrain, .. } = *d;
    let b = &d.b;
    let centre = Point { x: (b[0] + b[2]) / 2.0, y: (b[1] + b[3]) / 2.0 };
    match h {
        Handle::Pivot => {
            // The static pivot moves with the pointer, in the node's own fields (05 line 418: no key).
            let doc = cx.document()?;
            let e = &sel[0];
            let old = doc
                .element(e)
                .and_then(|el| el.transform().as_ref().and_then(|t| t.rotation_pivot.as_ref()).map(|q| (q.x, q.y)))
                .unwrap_or((0.0, 0.0));
            let to_view = camera_view::element_to_view(cx, chain, e)?.ok_or_else(|| camera_view::behind(e))?;
            let from_view = camera_view::invert(&to_view).ok_or_else(|| camera_view::behind(e))?;
            let (a, z) = (camera_view::apply(&from_view, p0.v), camera_view::apply(&from_view, pn.v));
            let du = render_list::DU_PER_FIELD;
            let p = json!({ "x": old.0 + (z.x - a.x) / du, "y": old.1 + (z.y - a.y) / du });
            Ok(transform(false, chain.frame, &sel[..1], json!({ "rotationPivot": p, "scalePivot": p })))
        }
        Handle::Rotate => {
            // View y points down: an anticlockwise turn on screen is a positive angle (11 line 148).
            let a0 = (p0.v.y - centre.y).atan2(p0.v.x - centre.x);
            let a1 = (pn.v.y - centre.y).atan2(pn.v.x - centre.x);
            let mut deg = -(a1 - a0).to_degrees();
            if deg > 180.0 {
                deg -= 360.0;
            } else if deg <= -180.0 {
                deg += 360.0;
            }
            if constrain {
                deg = (deg / 15.0).round() * 15.0;
            }
            Ok(transform(transform_tool, chain.frame, sel, json!({ "rotation": deg, "relative": true })))
        }
        Handle::Corner(i) => {
            let o = corners(b)[(i + 2) % 4];
            let (mut sx, mut sy) = (factor(p0.v.x, pn.v.x, o.x), factor(p0.v.y, pn.v.y, o.y));
            if constrain {
                let s = if sx.abs() >= sy.abs() { sx } else { sy };
                (sx, sy) = (s, s);
            }
            Ok(transform(transform_tool, chain.frame, sel, json!({ "scale": { "x": sx, "y": sy }, "relative": true })))
        }
        Handle::Side(i) => {
            // A side changes one axis, and only that axis is sent: a keyframing drag keys only the channels it changes
            // (05 §5.0 line 398, `scale.x`/`scale.y`; C43A-R1-22). `anim.transform.set` keys every channel it is
            // given (A: "key animated values"), so the gesture omits the other axis.
            let scale = match i {
                0 => json!({ "y": factor(p0.v.y, pn.v.y, b[3]) }),
                2 => json!({ "y": factor(p0.v.y, pn.v.y, b[1]) }),
                1 => json!({ "x": factor(p0.v.x, pn.v.x, b[0]) }),
                _ => json!({ "x": factor(p0.v.x, pn.v.x, b[2]) }),
            };
            Ok(transform(transform_tool, chain.frame, sel, json!({ "scale": scale, "relative": true })))
        }
        Handle::Skew(i) => {
            let (half, d) = if i % 2 == 0 {
                ((b[3] - b[1]) / 2.0, if i == 0 { pn.v.x - p0.v.x } else { p0.v.x - pn.v.x })
            } else {
                ((b[2] - b[0]) / 2.0, if i == 1 { p0.v.y - pn.v.y } else { pn.v.y - p0.v.y })
            };
            let deg = if half > 1e-9 { (d / half).atan().to_degrees().clamp(-89.0, 89.0) } else { 0.0 };
            Ok(transform(transform_tool, chain.frame, sel, json!({ "skew": deg, "relative": true })))
        }
    }
}

/// The Skew tool's vertical shear from the left (`i` = 3) or right (`i` = 1) side (05 §5.4; SI-151): y' = y + k·x in
/// the parent's axes, k the vertical drag over half the box's width (up on the right side, down on the left, is
/// positive). The first selected node's R(θ)·K(κ)·S(sx, sy) premultiplied by the shear is decomposed by 05 §2.5 and the
/// change applied relatively to the selection, keying only the channels whose value changes; `None` when nothing
/// changes.
fn vertical_shear(d: &Drag<'_>, i: usize) -> Result<Option<(String, Value)>, CommandError> {
    let Drag { cx, chain, transform_tool, sel, p0, pn, .. } = *d;
    let b = &d.b;
    let half = (b[2] - b[0]) / 2.0;
    let dy = if i == 1 { p0.v.y - pn.v.y } else { pn.v.y - p0.v.y };
    let k = if half > 1e-9 { dy / half } else { 0.0 };
    let doc = cx.document()?;
    let mut ev = evaluator(doc, "skewing in the Camera view");
    let l = ev.local(&sel[0], f64::from(chain.frame)).map_err(eval_error)?;
    let (cos, sin) = (l.rotation.to_radians().cos(), l.rotation.to_radians().sin());
    let tan = l.skew.to_radians().tan();
    let (sx, sy) = (l.scale[0], l.scale[1]);
    // A = R(θ)·K(κ)·S(sx, sy) = [[a, c], [b, d]].
    let (a, bb) = (cos * sx, sin * sx);
    let (c, dd) = (cos * tan * sy - sin * sy, sin * tan * sy + cos * sy);
    let before = decompose(a, bb, c, dd);
    let after = decompose(a, bb + k * a, c, dd + k * c);
    let mut turn = after.0 - before.0;
    if turn > 180.0 {
        turn -= 360.0;
    } else if turn <= -180.0 {
        turn += 360.0;
    }
    let mut fields = serde_json::Map::new();
    if turn.abs() > 1e-9 {
        fields.insert("rotation".into(), json!(turn));
    }
    if (after.1 - before.1).abs() > 1e-9 {
        fields.insert("skew".into(), json!(after.1 - before.1));
    }
    let mut scale = serde_json::Map::new();
    for (axis, ratio) in [("x", after.2 / before.2), ("y", after.3 / before.3)] {
        if ratio.is_finite() && (ratio - 1.0).abs() > 1e-12 {
            scale.insert(axis.into(), json!(ratio));
        }
    }
    if !scale.is_empty() {
        fields.insert("scale".into(), Value::Object(scale));
    }
    if fields.is_empty() {
        return Ok(None);
    }
    fields.insert("relative".into(), json!(true));
    Ok(Some(transform(transform_tool, chain.frame, sel, Value::Object(fields))))
}

/// 05 §2.5: [[a, c], [b, d]] = R(θ)·K(κ)·S(sx, sy) → (θ°, κ°, sx, sy).
fn decompose(a: f64, b: f64, c: f64, d: f64) -> (f64, f64, f64, f64) {
    let sx = a.hypot(b);
    let theta = b.atan2(a);
    let (cos, sin) = (theta.cos(), theta.sin());
    let (c2, d2) = (cos * c + sin * d, -sin * c + cos * d);
    let kappa = if d2 == 0.0 { 0.0 } else { (c2 / d2).atan().to_degrees() };
    (theta.to_degrees(), kappa, sx, d2)
}

/// A `pointer` event's point in a Camera view, in view pixels (11 line 354; SI-125): `view` as given; `drawing` through
/// the current element's plane; `camera` through the camera's output pixels; `scene` the table-plane point (Z = 0)
/// through the camera.
///
/// # Errors
/// The chain's refusals; `arg.invalid` without a current element for drawing points; `inspect.behindCamera`.
pub(crate) fn view_point(
    cx: &Cx,
    w: &crate::shell::ViewWindow,
    space: InputSpace,
    p: Point,
) -> Result<Point, CommandError> {
    if space == InputSpace::View {
        return Ok(p);
    }
    let chain = camera_view::chain(cx, w)?;
    match space {
        InputSpace::Drawing => {
            let e = camera_view::current_element(cx)?;
            let m = camera_view::element_to_view(cx, &chain, &e)?.ok_or_else(|| camera_view::behind(&e))?;
            Ok(camera_view::apply(&m, p))
        }
        InputSpace::Camera => {
            let inv = camera_view::invert(&chain.image_to_camera)
                .ok_or_else(|| crate::commands::common::invalid("/space", "the camera's scale is degenerate"))?;
            Ok(camera_view::apply(&chain.output_to_view, camera_view::apply(&inv, p)))
        }
        _ => {
            let o = camera_view::scene_to_output(&chain, [p.x, p.y, 0.0]).ok_or_else(|| {
                CommandError::with_details(
                    "inspect.behindCamera",
                    "the scene point is at or behind the camera's near plane",
                    json!({ "pointer": "/events" }),
                )
            })?;
            Ok(camera_view::apply(&chain.output_to_view, o))
        }
    }
}

/// A Camera view's pixels → the current drawing's units at the press (02 line 61: through the current element's full
/// scene transform); the identity without a current element in front of the camera (scene tools read view pixels).
pub(crate) fn press_matrix(cx: &Cx, w: &crate::shell::ViewWindow) -> Result<Matrix, CommandError> {
    let chain = camera_view::chain(cx, w)?;
    let e = camera_view::current_element(cx)?;
    let to_view = camera_view::element_to_view(cx, &chain, &e)?.ok_or_else(|| camera_view::behind(&e))?;
    camera_view::invert(&to_view).ok_or_else(|| camera_view::behind(&e))
}
