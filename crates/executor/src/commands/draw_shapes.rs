//! Shape tools (02 §7.2.3–§7.2.6, §13.5; M2 C2.3): `draw.rectangle.add`, `draw.ellipse.add`, `draw.line.add` and
//! `draw.polyline.add`. Each builds a pencil path and adds it through `draw.stroke.add`'s handler (no fitting), so
//! shapes share its layer rules, symmetry copies, persistent noding, zones and settle pass.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_registry::{CommandError, command};
use tt_vector::geom::P;

use super::draw::{Point, err};
use super::draw_stroke::{StrokeAddParams, draw_stroke_add};
use crate::cx::Cx;

fn auto() -> String {
    "auto".into()
}

fn yes() -> bool {
    true
}

/// The result of a shape tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShapeResult {
    /// The drawing.
    drawing_id: String,
    /// New strokes (the pieces noding made of them, symmetry copies included).
    stroke_ids: Vec<String>,
    /// Zones the shape created.
    zone_ids: Vec<String>,
    /// The layer holding them.
    layer_id: String,
    /// Created objects.
    created_ids: Vec<String>,
    /// Modified objects.
    modified_ids: Vec<String>,
    /// Removed objects (strokes split by noding, zones that ceased to exist).
    removed_ids: Vec<String>,
    /// The drawing revision after the change.
    rev: u64,
}

/// Common shape options.
struct Common<'a> {
    drawing_id: Option<&'a str>,
    element_id: Option<&'a str>,
    frame: Option<u32>,
    create_drawing: bool,
    colour_id: Option<&'a str>,
    thickness: Option<f64>,
    pen_id: Option<&'a str>,
    layer: &'a str,
    symmetry: bool,
}

/// Adds a pencil path through `draw.stroke.add`: corner points, or a Bézier list.
fn add(cx: &mut Cx, c: &Common<'_>, geometry: Value, closed: bool) -> Result<ShapeResult, CommandError> {
    let mut v = json!({ "kind": "pencil", "fit": false, "closed": closed, "layer": c.layer, "symmetry": c.symmetry });
    if let Some(o) = v.as_object_mut() {
        if let Some(m) = geometry.as_object() {
            o.extend(m.clone());
        }
        if let Some(f) = c.frame {
            o.insert("frame".into(), json!(f));
        }
        // Forwarded either way: an explicit false refuses an empty cell (C23-R2-02).
        o.insert("createDrawing".into(), json!(c.create_drawing));
        for (k, val) in
            [("drawingId", c.drawing_id), ("elementId", c.element_id), ("colourId", c.colour_id), ("penId", c.pen_id)]
        {
            if let Some(x) = val {
                o.insert(k.into(), json!(x));
            }
        }
        if let Some(t) = c.thickness {
            o.insert("thickness".into(), json!(t));
        }
    }
    let p: StrokeAddParams = serde_json::from_value(v)
        .map_err(|e| err("arg.invalid", format!("shape parameters: {e}"), json!({ "pointer": "" })))?;
    let r = serde_json::to_value(draw_stroke_add(cx, p)?).map_err(|e| CommandError::internal(e.to_string()))?;
    let ids = |k: &str| -> Vec<String> {
        r.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
    };
    Ok(ShapeResult {
        drawing_id: r.get("drawingId").and_then(Value::as_str).unwrap_or_default().to_owned(),
        stroke_ids: ids("strokeIds"),
        zone_ids: ids("zoneIds"),
        layer_id: r.get("layerId").and_then(Value::as_str).unwrap_or_default().to_owned(),
        created_ids: ids("createdIds"),
        modified_ids: ids("modifiedIds"),
        removed_ids: ids("removedIds"),
        rev: r.get("rev").and_then(Value::as_u64).unwrap_or_default(),
    })
}

fn points_json(pts: &[P]) -> Value {
    json!({ "points": pts.iter().map(|p| json!({ "x": p.x, "y": p.y })).collect::<Vec<_>>() })
}

/// The box of a drag: `from`–`to`, or centred on `from`; squared to the larger side when asked.
fn drag_box(from: Point, to: Point, square: bool, from_centre: bool) -> (P, P) {
    let (mut dx, mut dy) = (to.x - from.x, to.y - from.y);
    if square {
        let s = dx.abs().max(dy.abs());
        dx = s.copysign(if dx == 0.0 { 1.0 } else { dx });
        dy = s.copysign(if dy == 0.0 { 1.0 } else { dy });
    }
    if from_centre {
        (P::new(from.x - dx, from.y - dy), P::new(from.x + dx, from.y + dy))
    } else {
        (P::new(from.x, from.y), P::new(from.x + dx, from.y + dy))
    }
}

fn degenerate(a: P, b: P, what: &str) -> Option<CommandError> {
    ((a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9)
        .then(|| err("drawing.invalidGeometry", format!("the {what} has no area"), json!({ "pointer": "/to" })))
}

/// `draw.rectangle.add` / `draw.ellipse.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BoxShapeParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Or the cell of this element (02 §13.1 item 4; ADR-0016 common addressing).
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    frame: Option<u32>,
    /// Create the cell's drawing when it has none (default true, as `draw.stroke.add`).
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Press point.
    from: Point,
    /// Release point.
    to: Point,
    /// Square constraint.
    #[serde(default)]
    square: bool,
    /// `from` is the centre.
    #[serde(default)]
    from_centre: bool,
    /// Line colour (default: current colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// Line thickness (default: the pen's).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// Pen (default: current pencil pen).
    #[serde(default)]
    pen_id: Option<String>,
    /// Layer rule (as `draw.stroke.add`).
    #[serde(default = "auto")]
    layer: String,
    /// Symmetry copies (02 §6.2).
    #[serde(default = "yes")]
    symmetry: bool,
}

impl BoxShapeParams {
    fn common(&self) -> Common<'_> {
        Common {
            drawing_id: self.drawing_id.as_deref(),
            element_id: self.element_id.as_deref(),
            frame: self.frame,
            create_drawing: self.create_drawing,
            colour_id: self.colour_id.as_deref(),
            thickness: self.thickness,
            pen_id: self.pen_id.as_deref(),
            layer: &self.layer,
            symmetry: self.symmetry,
        }
    }
}

#[command(
    id = "draw.rectangle.add",
    kind = action,
    label = "Rectangle",
    scopes = [edit],
    undo = "Rectangle Tool",
    needs_project,
    example(title = "A 100 × 60 rectangle", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 360}}"#),
    gui = "Drawing view: Rectangle tool (M4 C4.2)",
)]
pub(crate) fn draw_rectangle_add(cx: &mut Cx, p: BoxShapeParams) -> Result<ShapeResult, CommandError> {
    let (a, b) = drag_box(p.from, p.to, p.square, p.from_centre);
    if let Some(e) = degenerate(a, b, "rectangle") {
        return Err(e);
    }
    // Counter-clockwise from the lower-left corner (y up).
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    let pts = [P::new(x0, y0), P::new(x1, y0), P::new(x1, y1), P::new(x0, y1)];
    add(cx, &p.common(), points_json(&pts), true)
}

/// The Bézier constant of a quarter circle.
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// `draw.ellipse.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct EllipseParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Or the cell of this element (02 §13.1 item 4; ADR-0016 common addressing).
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    frame: Option<u32>,
    /// Create the cell's drawing when it has none (default true, as `draw.stroke.add`).
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Press point.
    from: Point,
    /// Release point.
    to: Point,
    /// Circle constraint.
    #[serde(default)]
    circle: bool,
    /// `from` is the centre.
    #[serde(default)]
    from_centre: bool,
    /// Line colour (default: current colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// Line thickness (default: the pen's).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// Pen (default: current pencil pen).
    #[serde(default)]
    pen_id: Option<String>,
    /// Layer rule (as `draw.stroke.add`).
    #[serde(default = "auto")]
    layer: String,
    /// Symmetry copies (02 §6.2).
    #[serde(default = "yes")]
    symmetry: bool,
}

#[command(
    id = "draw.ellipse.add",
    kind = action,
    label = "Ellipse",
    scopes = [edit],
    undo = "Ellipse Tool",
    needs_project,
    example(title = "A circle of radius 50", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "from": {"x": 500, "y": 300}, "to": {"x": 550, "y": 350}, "circle": true, "fromCentre": true}"#),
    gui = "Drawing view: Ellipse tool (M4 C4.2)",
)]
pub(crate) fn draw_ellipse_add(cx: &mut Cx, p: EllipseParams) -> Result<ShapeResult, CommandError> {
    let p = BoxShapeParams {
        drawing_id: p.drawing_id,
        element_id: p.element_id,
        frame: p.frame,
        create_drawing: p.create_drawing,
        from: p.from,
        to: p.to,
        square: p.circle,
        from_centre: p.from_centre,
        colour_id: p.colour_id,
        thickness: p.thickness,
        pen_id: p.pen_id,
        layer: p.layer,
        symmetry: p.symmetry,
    };
    let (a, b) = drag_box(p.from, p.to, p.square, p.from_centre);
    if let Some(e) = degenerate(a, b, "ellipse") {
        return Err(e);
    }
    let (cx0, cy0) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let (rx, ry) = ((b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0);
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    // Four counter-clockwise quarter arcs from the rightmost point: a closed Bézier list of 12 points.
    let list = [
        P::new(cx0 + rx, cy0),
        P::new(cx0 + rx, cy0 + ky),
        P::new(cx0 + kx, cy0 + ry),
        P::new(cx0, cy0 + ry),
        P::new(cx0 - kx, cy0 + ry),
        P::new(cx0 - rx, cy0 + ky),
        P::new(cx0 - rx, cy0),
        P::new(cx0 - rx, cy0 - ky),
        P::new(cx0 - kx, cy0 - ry),
        P::new(cx0, cy0 - ry),
        P::new(cx0 + kx, cy0 - ry),
        P::new(cx0 + rx, cy0 - ky),
    ];
    let bezier = json!({ "bezier": list.iter().map(|q| json!({ "x": q.x, "y": q.y })).collect::<Vec<_>>() });
    add(cx, &p.common(), bezier, true)
}

/// `draw.line.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LineParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Or the cell of this element (02 §13.1 item 4; ADR-0016 common addressing).
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    frame: Option<u32>,
    /// Create the cell's drawing when it has none (default true, as `draw.stroke.add`).
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Start (required unless `continuePrevious`).
    #[serde(default)]
    from: Option<Point>,
    /// End.
    to: Point,
    /// Start at the end of the line this tool added last in the drawing.
    #[serde(default)]
    continue_previous: bool,
    /// Constrain the angle to multiples of 45°.
    #[serde(default)]
    constrain_angle: bool,
    /// Line colour.
    #[serde(default)]
    colour_id: Option<String>,
    /// Thickness.
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// Pen.
    #[serde(default)]
    pen_id: Option<String>,
    /// Layer rule.
    #[serde(default = "auto")]
    layer: String,
    /// Symmetry copies.
    #[serde(default = "yes")]
    symmetry: bool,
}

#[command(
    id = "draw.line.add",
    kind = action,
    label = "Line",
    scopes = [edit],
    undo = "Line Tool",
    needs_project,
    example(title = "A straight line", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "from": {"x": 300, "y": 500}, "to": {"x": 420, "y": 540}}"#),
    gui = "Drawing view: Line tool (M4 C4.2)",
)]
pub(crate) fn draw_line_add(cx: &mut Cx, p: LineParams) -> Result<ShapeResult, CommandError> {
    let from = match (p.from, p.continue_previous) {
        (Some(f), false) => P::new(f.x, f.y),
        (_, true) => {
            previous_line_end(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame)?.ok_or_else(|| {
                err(
                    "drawing.nothingToDo",
                    "there is no previous line to continue",
                    json!({ "pointer": "/continuePrevious" }),
                )
            })?
        }
        (None, false) => {
            return Err(err(
                "arg.invalid",
                "`from` is required unless `continuePrevious`",
                json!({ "pointer": "/from" }),
            ));
        }
    };
    let mut to = P::new(p.to.x, p.to.y);
    if p.constrain_angle {
        // The nearest of the eight 45° directions (exact unit vectors, no platform trigonometry), same length.
        let d = to - from;
        let r = std::f64::consts::FRAC_1_SQRT_2;
        let dirs = [(1.0, 0.0), (r, r), (0.0, 1.0), (-r, r), (-1.0, 0.0), (-r, -r), (0.0, -1.0), (r, -r)];
        let best = dirs
            .iter()
            .copied()
            .max_by(|a, b| (a.0 * d.x + a.1 * d.y).total_cmp(&(b.0 * d.x + b.1 * d.y)))
            .unwrap_or((1.0, 0.0));
        let len = d.len();
        to = P::new(from.x + len * best.0, from.y + len * best.1);
    }
    if from.dist(to) < 1e-9 {
        return Err(err("drawing.invalidGeometry", "the line has no length", json!({ "pointer": "/to" })));
    }
    let c = Common {
        drawing_id: p.drawing_id.as_deref(),
        element_id: p.element_id.as_deref(),
        frame: p.frame,
        create_drawing: p.create_drawing,
        colour_id: p.colour_id.as_deref(),
        thickness: p.thickness,
        pen_id: p.pen_id.as_deref(),
        layer: &p.layer,
        symmetry: p.symmetry,
    };
    let r = add(cx, &c, points_json(&[from, to]), false)?;
    remember_line_end(cx, &r.drawing_id, to);
    Ok(r)
}

/// The Line tool's last end per drawing, kept in the session (tool memory, not document content).
fn remember_line_end(cx: &mut Cx, drawing: &str, to: P) {
    cx.app_mut().tool_memory.insert(format!("line.end:{drawing}"), format!("{},{}", to.x, to.y));
}

fn previous_line_end(
    cx: &mut Cx,
    drawing: Option<&str>,
    element: Option<&str>,
    frame: Option<u32>,
) -> Result<Option<P>, CommandError> {
    let mut none = Vec::new();
    let t = super::draw::target(cx, drawing, element, frame, false, &mut none)?;
    let v = cx.app().tool_memory.get(&format!("line.end:{}", t.id.as_str())).cloned();
    Ok(v.as_deref().and_then(|s| {
        let (x, y) = s.split_once(',')?;
        Some(P::new(x.parse().ok()?, y.parse().ok()?))
    }))
}

/// A polyline anchor: position and optional handle offsets.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PolyPoint {
    /// x (du).
    x: f64,
    /// y (du).
    y: f64,
    /// Incoming handle, as an offset from the anchor (10 §4.9 convention; SI-069).
    #[serde(default)]
    in_handle: Option<Point>,
    /// Outgoing handle, as an offset from the anchor.
    #[serde(default)]
    out_handle: Option<Point>,
}

/// `draw.polyline.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PolylineParams {
    /// The drawing (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Or the cell of this element (02 §13.1 item 4; ADR-0016 common addressing).
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: current frame).
    #[serde(default)]
    frame: Option<u32>,
    /// Create the cell's drawing when it has none (default true, as `draw.stroke.add`).
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Anchors (at least 2).
    #[schemars(length(min = 2, max = 10000))]
    points: Vec<PolyPoint>,
    /// Closed.
    #[serde(default)]
    closed: bool,
    /// Snap anchors to existing anchors within `snapRadius`.
    #[serde(default = "yes")]
    snap: bool,
    /// Snap radius (view px; 1 px = 1 du at 100 %, SI-063).
    #[serde(default = "eight")]
    #[schemars(range(min = 0.0, max = 1000.0))]
    snap_radius: f64,
    /// Line colour.
    #[serde(default)]
    colour_id: Option<String>,
    /// Thickness.
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
    /// Pen.
    #[serde(default)]
    pen_id: Option<String>,
    /// Layer rule.
    #[serde(default = "auto")]
    layer: String,
    /// Symmetry copies.
    #[serde(default = "yes")]
    symmetry: bool,
}

fn eight() -> f64 {
    8.0
}

#[command(
    id = "draw.polyline.add",
    kind = action,
    label = "Polyline",
    scopes = [edit],
    undo = "Polyline Tool",
    needs_project,
    example(title = "An open polyline with a curved segment", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "points": [{"x": 300, "y": 600}, {"x": 360, "y": 640, "inHandle": {"x": -20, "y": 0}, "outHandle": {"x": 20, "y": 0}}, {"x": 420, "y": 600}], "snap": false}"#),
    gui = "Drawing view: Polyline tool (M4 C4.2)",
)]
pub(crate) fn draw_polyline_add(cx: &mut Cx, p: PolylineParams) -> Result<ShapeResult, CommandError> {
    let mut anchors: Vec<(P, P, P)> = p
        .points
        .iter()
        .map(|q| {
            let h = |o: Option<Point>| o.map_or(P::default(), |v| P::new(v.x, v.y));
            (P::new(q.x, q.y), h(q.in_handle), h(q.out_handle))
        })
        .collect();
    if p.snap && p.snap_radius > 0.0 {
        let mut none = Vec::new();
        let t = super::draw::target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut none);
        // A cell without a drawing yet (createDrawing) has nothing to snap to.
        let doc = cx.document()?.clone();
        if let Ok(t) = t
            && let Ok(d) = t.drawing(&doc)
        {
            let existing: Vec<P> = tt_vector::drawing::objects(&d)
                .iter()
                .filter_map(|o| tt_vector::drawing::stroke_at(&d, &o.pointer))
                .flat_map(|s| s.points().iter().map(|a| P::new(a[0], a[1])).collect::<Vec<_>>())
                .collect();
            for a in &mut anchors {
                if let Some(best) = existing
                    .iter()
                    .filter(|e| e.dist(a.0) <= p.snap_radius)
                    .min_by(|x, y| x.dist(a.0).total_cmp(&y.dist(a.0)))
                {
                    a.0 = *best;
                }
            }
        }
    }
    // The Bézier list: anchor, out control, in control, next anchor, … (closed: back to the first).
    let n = anchors.len();
    let mut list: Vec<P> = Vec::new();
    let segs = if p.closed { n } else { n - 1 };
    for k in 0..segs {
        let (a, b) = (anchors[k], anchors[(k + 1) % n]);
        if k == 0 {
            list.push(a.0);
        }
        list.push(a.0 + a.2);
        list.push(b.0 + b.1);
        if !(p.closed && k + 1 == segs) {
            list.push(b.0);
        }
    }
    let bezier = json!({ "bezier": list.iter().map(|q| json!({ "x": q.x, "y": q.y })).collect::<Vec<_>>() });
    let c = Common {
        drawing_id: p.drawing_id.as_deref(),
        element_id: p.element_id.as_deref(),
        frame: p.frame,
        create_drawing: p.create_drawing,
        colour_id: p.colour_id.as_deref(),
        thickness: p.thickness,
        pen_id: p.pen_id.as_deref(),
        layer: &p.layer,
        symmetry: p.symmetry,
    };
    add(cx, &c, bezier, p.closed)
}
