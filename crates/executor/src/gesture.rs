//! Gestures and the headless `pointer` input path (02 §5.2, §13.3; 11 §7.6; M4 C4.2, reviews/M4-design-gestures.md and
//! reviews/M4-design-pointer.md). One input model: a sample is a view point with pressure, tilt, pointer kind,
//! modifiers and a simulated time. A gesture is press, samples as given, release. [`to_command`] builds the equivalent
//! command of 02 §13.3's table with every current setting explicit; the same function serves the headless `pointer`
//! tool here and the webview (C4.2b, through IPC). The view mapping is captured at the press and used for every
//! sample of the gesture.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::entities::pen::PenKind;
use tt_registry::{CommandError, codes};

use crate::commands::view::to_drawing;
use crate::cx::Cx;
use crate::shell::{Point, ViewType, ViewWindow};
use crate::types::{Envelope, Invocation, InvokeOptions, Origin};

/// At most this many events per `pointer` call (SI-130).
pub const MAX_EVENTS: usize = 10_000;
/// Event coordinates are within this many view pixels of the origin (design note 4 §3).
pub const MAX_COORD: f64 = 10_000_000.0;
/// A press and release no farther apart than this (view pixels) is a click (SI-131).
pub const CLICK_SLOP_PX: f64 = 3.0;
/// The Select tool's drawing-pivot handle radius in view pixels (SI-131).
pub const PIVOT_HANDLE_PX: f64 = 6.0;

// ----- the request ----------------------------------------------------------------------------------------------------

/// The space of `pointer` coordinates (11 §7.6; SI-125).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InputSpace {
    /// View pixels.
    #[default]
    View,
    /// Drawing units (converted to view pixels at the press).
    Drawing,
    /// Scene units (refused in the Drawing view).
    Scene,
    /// Camera units (refused in the Drawing view).
    Camera,
}

/// The pointer kind (02 §13.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PointerKind {
    /// Mouse.
    #[default]
    Mouse,
    /// Pen tip.
    Pen,
    /// The pen's eraser end (a temporary Eraser for the gesture, 02 §5.2).
    PenEraser,
}

/// An event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    /// Press.
    Down,
    /// Move (a sample while pressed).
    Move,
    /// Release.
    Up,
    /// Move without a press.
    Hover,
    /// Wheel.
    Wheel,
    /// Double click (ends a polyline).
    DoubleClick,
}

/// A mouse button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    /// Left.
    #[default]
    Left,
    /// Right.
    Right,
    /// Middle.
    Middle,
}

/// A modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Modifier {
    /// Shift (the constrain modifier).
    Shift,
    /// Alt / Option (the alternate modifier).
    Alt,
    /// Ctrl.
    Ctrl,
    /// Meta / Command.
    Meta,
}

/// One event.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PointerEvent {
    /// Event type.
    #[serde(rename = "type")]
    pub ty: EventType,
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Pressure 0..1 (default 1).
    #[serde(default)]
    pub pressure: Option<f64>,
    /// Tilt X (recorded; no tool reads it, SI-125).
    #[serde(default)]
    pub tilt_x: Option<f64>,
    /// Tilt Y.
    #[serde(default)]
    pub tilt_y: Option<f64>,
    /// Button.
    #[serde(default)]
    pub button: Option<Button>,
    /// Modifiers held.
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    /// Simulated milliseconds since the previous event.
    #[serde(default)]
    pub dt: Option<f64>,
    /// Wheel delta (DOM convention: negative zooms in).
    #[serde(default)]
    pub wheel_delta: Option<f64>,
}

/// The `pointer` request (11 §7.6).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PointerRequest {
    /// View id (or `drawing` for the Drawing view).
    pub view: String,
    /// Coordinate space.
    #[serde(default)]
    pub space: InputSpace,
    /// Pointer kind.
    #[serde(default)]
    pub pointer_type: PointerKind,
    /// Events.
    pub events: Vec<PointerEvent>,
    /// Tool to activate first.
    #[serde(default)]
    pub tool: Option<String>,
}

// ----- gestures -------------------------------------------------------------------------------------------------------

/// One sample, in view pixels and in drawing units through the mapping captured at the press.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// View point.
    pub v: Point,
    /// Drawing point.
    pub d: Point,
    /// Pressure 0..1.
    pub pressure: f64,
    /// Simulated time (ms).
    pub t_ms: f64,
}

/// Modifiers at the press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    /// Constrain (Shift).
    pub constrain: bool,
    /// Alternate (Alt / Option).
    pub alternate: bool,
    /// Temporary Select (Ctrl or Meta, 02 §5.2).
    pub temp_select: bool,
}

impl Mods {
    fn of(m: &[Modifier]) -> Self {
        Self {
            constrain: m.contains(&Modifier::Shift),
            alternate: m.contains(&Modifier::Alt),
            temp_select: m.contains(&Modifier::Ctrl) || m.contains(&Modifier::Meta),
        }
    }
}

/// One press-to-release stroke of input.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Samples from the press to the release, as given.
    pub samples: Vec<Sample>,
    /// Modifiers at the press.
    pub mods: Mods,
}

impl Stroke {
    fn first(&self) -> Option<&Sample> {
        self.samples.first()
    }

    fn last(&self) -> Option<&Sample> {
        self.samples.last()
    }

    /// Whether every sample stays within [`CLICK_SLOP_PX`] of the press.
    #[must_use]
    pub fn is_click(&self) -> bool {
        let Some(a) = self.first() else { return true };
        self.samples.iter().all(|s| (s.v.x - a.v.x).hypot(s.v.y - a.v.y) <= CLICK_SLOP_PX)
    }
}

/// A gesture: one stroke for most tools, several clicks or click-drags for the Polyline (02 §13.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Gesture {
    /// The tool the gesture is made with (after temporary overrides).
    pub tool: String,
    /// The view.
    pub view_id: String,
    /// Strokes (Polyline: one per click).
    pub strokes: Vec<Stroke>,
    /// Ended by a double click (Polyline).
    pub double_click: bool,
}

/// The tools whose gestures C4.2 builds (design note 3 §2); the others answer `command.notImplemented` (SI-129).
pub const COVERED: &[&str] = &[
    "select",
    "brush",
    "pencil",
    "rectangle",
    "ellipse",
    "polyline",
    "line",
    "paint",
    "unpaint",
    "paintUnpainted",
    "dropper",
    "closeGap",
    "stroke",
    "eraser",
    "cutter",
    "scissor",
    "zoom",
    "grabber",
];

/// The tool a gesture uses after the temporary overrides of 02 §5.2 held at the press.
#[must_use]
pub fn effective_tool(active: &str, kind: PointerKind, mods: Mods) -> String {
    const TEMP_SELECT: &[&str] = &[
        "zoom",
        "grabber",
        "polyline",
        "line",
        "rectangle",
        "ellipse",
        "brush",
        "pencil",
        "eraser",
        "stroke",
        "closeGap",
        "paint",
        "paintUnpainted",
        "unpaint",
        "dropper",
        "editTexture",
    ];
    if kind == PointerKind::PenEraser {
        return "eraser".into();
    }
    if mods.temp_select && TEMP_SELECT.contains(&active) {
        return "select".into();
    }
    if mods.alternate && matches!(active, "paint" | "brush" | "pencil") {
        return "dropper".into();
    }
    active.to_owned()
}

// ----- the equivalent command -----------------------------------------------------------------------------------------

fn pt(p: Point) -> Value {
    json!({ "x": p.x, "y": p.y })
}

fn not_built(tool: &str) -> CommandError {
    CommandError::with_details(
        codes::COMMAND_NOT_IMPLEMENTED,
        format!("gestures of the {tool} tool are not built yet (owner M4 C4.3, SI-129); use its structured command"),
        json!({ "toolId": tool, "owner": "M4 C4.3" }),
    )
}

fn empty() -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, "the gesture has no samples", json!({ "pointer": "/events" }))
}

/// The current settings made explicit (02 §13.3 "current settings are made explicit").
struct Settings {
    brush_pen: Option<String>,
    brush_max: Option<f64>,
    eraser_pen: Option<String>,
    colour: Option<String>,
    layer: &'static str,
    gap: Value,
}

fn settings(cx: &Cx) -> Settings {
    let doc = cx.document().ok();
    let brush = crate::commands::tool::current_pen(cx, PenKind::Brush);
    let brush_max = brush
        .as_ref()
        .and_then(|id| doc.and_then(|d| d.project().pens().iter().find(|p| p.id() == id).map(|p| *p.max_size())));
    let d = cx.ui().drawing();
    // Draw on Top Layer takes precedence over Draw Behind when both are on (SI-131).
    let layer = if d.draw_top_layer {
        "top"
    } else if d.place_behind {
        "behind"
    } else {
        "auto"
    };
    let gap = json!(crate::commands::colour::GapMode::of(cx.pref(crate::commands::colour::GAP_PREF).as_ref()));
    Settings {
        brush_pen: brush.map(|i| i.as_str().to_owned()),
        brush_max,
        eraser_pen: crate::commands::tool::current_pen(cx, PenKind::Eraser).map(|i| i.as_str().to_owned()),
        colour: doc.and_then(crate::commands::draw_stroke::current_colour),
        layer,
        gap,
    }
}

/// The line thickness of the Pencil and the shape tools (02 §13.2): the current brush pen's maximum size, or the
/// tool's fixed thickness.
fn thickness(cx: &Cx, tool: &str, s: &Settings) -> Option<f64> {
    let o = cx.ui().drawing().options_of(tool);
    if o.get("thicknessSource").and_then(Value::as_str) == Some("fixed")
        && let Some(t) = o.get("thickness").and_then(Value::as_f64)
    {
        return Some(t);
    }
    s.brush_max
}

fn styled(m: &mut Map<String, Value>, cx: &Cx, tool: &str, s: &Settings) {
    if let Some(c) = &s.colour {
        m.insert("colourId".into(), json!(c));
    }
    if let Some(t) = thickness(cx, tool, s) {
        m.insert("thickness".into(), json!(t));
    }
    if let Some(p) = &s.brush_pen {
        m.insert("penId".into(), json!(p));
    }
    m.insert("layer".into(), json!(s.layer));
    m.insert("symmetry".into(), json!(true));
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn rect(a: Point, b: Point) -> Value {
    json!({ "x": a.x.min(b.x), "y": a.y.min(b.y), "width": (a.x - b.x).abs(), "height": (a.y - b.y).abs() })
}

/// The equivalent command of a gesture (02 §13.3), with every current setting explicit. `None`: the gesture does
/// nothing (a Polyline of one click).
///
/// # Errors
/// `command.notImplemented` for the tools of SI-129; `arg.invalid` for a gesture without samples.
pub fn to_command(cx: &Cx, g: &Gesture) -> Result<Option<(String, Value)>, CommandError> {
    let tool = g.tool.as_str();
    if !COVERED.contains(&tool) {
        return Err(not_built(tool));
    }
    if tool == "polyline" {
        return Ok(polyline(cx, g));
    }
    let Some(st) = g.strokes.first() else { return Err(empty()) };
    let (Some(p0), Some(pn)) = (st.first().copied(), st.last().copied()) else { return Err(empty()) };
    let s = settings(cx);
    let mapped: Vec<Value> = st.samples.iter().map(|x| pt(x.d)).collect();
    let pressured: Vec<Value> =
        st.samples.iter().map(|x| json!({ "x": x.d.x, "y": x.d.y, "pressure": x.pressure })).collect();
    let space = |m: &mut Map<String, Value>| {
        m.insert("space".into(), json!("drawing"));
        m.insert("view".into(), json!(g.view_id));
    };
    let out = match tool {
        "brush" | "pencil" | "stroke" => {
            let kind = if tool == "stroke" { "invisible" } else { tool };
            let mut m = obj(json!({ "kind": kind, "points": pressured, "fit": true }));
            if let Some(p) = &s.brush_pen {
                m.insert("penId".into(), json!(p));
            }
            if let Some(c) = &s.colour {
                m.insert("colourId".into(), json!(c));
            }
            if tool == "pencil"
                && let Some(t) = thickness(cx, "pencil", &s)
            {
                m.insert("thickness".into(), json!(t));
            }
            m.insert("layer".into(), json!(s.layer));
            m.insert("symmetry".into(), json!(true));
            ("draw.stroke.add", m)
        }
        "rectangle" | "ellipse" => {
            let constrain = if tool == "rectangle" { "square" } else { "circle" };
            let mut m = obj(json!({ "from": pt(p0.d), "to": pt(pn.d), constrain: st.mods.constrain,
                                   "fromCentre": st.mods.alternate }));
            styled(&mut m, cx, tool, &s);
            (if tool == "rectangle" { "draw.rectangle.add" } else { "draw.ellipse.add" }, m)
        }
        "line" => {
            let mut m = obj(json!({ "to": pt(pn.d), "constrainAngle": st.mods.constrain,
                                   "continuePrevious": st.mods.alternate }));
            if !st.mods.alternate {
                m.insert("from".into(), pt(p0.d));
            }
            styled(&mut m, cx, tool, &s);
            ("draw.line.add", m)
        }
        "eraser" => {
            let mut m = obj(json!({ "path": pressured }));
            if let Some(p) = &s.eraser_pen {
                m.insert("penId".into(), json!(p));
            }
            ("draw.erase.apply", m)
        }
        "cutter" => ("draw.cut.apply", obj(json!({ "polygon": mapped }))),
        "scissor" if st.mods.alternate => ("draw.cut.apply", obj(json!({ "polygon": mapped }))),
        "scissor" => ("draw.cut.apply", obj(json!({ "rect": rect(p0.d, pn.d) }))),
        "paint" | "unpaint" | "paintUnpainted" => {
            let click = st.is_click();
            let mut m = Map::new();
            match (tool, click) {
                (_, true) => {
                    m.insert("points".into(), json!([pt(p0.d)]));
                }
                ("paint", false) => {
                    m.insert("path".into(), json!(mapped));
                }
                ("paintUnpainted", false) => {
                    m.insert("lasso".into(), json!(mapped));
                }
                _ => {
                    m.insert("points".into(), json!([pt(p0.d)]));
                }
            }
            space(&mut m);
            if tool != "unpaint"
                && let Some(c) = &s.colour
            {
                m.insert("colourId".into(), json!(c));
            }
            m.insert("gapClosing".into(), s.gap.clone());
            let id = match tool {
                "paint" => "colour.paint.apply",
                "unpaint" => "colour.paint.remove",
                _ => "colour.paintUnpainted.apply",
            };
            (id, m)
        }
        "dropper" => {
            let mut m = obj(json!({ "point": pt(p0.d) }));
            space(&mut m);
            ("colour.pick", m)
        }
        "closeGap" => {
            let mut m = obj(json!({ "from": pt(p0.d), "to": pt(pn.d) }));
            space(&mut m);
            ("colour.gap.close", m)
        }
        "zoom" => {
            let steps = if st.mods.alternate { -1 } else { 1 };
            ("view.zoom.set", obj(json!({ "viewId": g.view_id, "steps": steps, "anchor": pt(p0.v) })))
        }
        "grabber" => {
            let by = Point { x: pn.v.x - p0.v.x, y: pn.v.y - p0.v.y };
            ("view.pan.set", obj(json!({ "viewId": g.view_id, "by": pt(by) })))
        }
        "select" => return select(cx, g, st, p0, pn),
        _ => return Err(not_built(tool)),
    };
    Ok(Some((out.0.to_owned(), Value::Object(out.1))))
}

fn select(cx: &Cx, g: &Gesture, st: &Stroke, p0: Sample, pn: Sample) -> Result<Option<(String, Value)>, CommandError> {
    let w = cx.ui().shell().view(&g.view_id)?;
    // The drawing pivot handle (02 §7.1 "drag drawing pivot").
    if let Some(d) = current_drawing(cx) {
        let pv = crate::commands::view::to_view(w, Point { x: d.1.x, y: d.1.y });
        if !st.is_click() && (pv.x - p0.v.x).hypot(pv.y - p0.v.y) <= PIVOT_HANDLE_PX {
            return Ok(Some(("draw.pivot.set".into(), json!({ "point": pt(pn.d) }))));
        }
        // A drag that starts on the selection moves it (alternate: a copy).
        if !st.is_click() && inside_selection(cx, &d.0, p0.d) {
            let by = Point { x: pn.d.x - p0.d.x, y: pn.d.y - p0.d.y };
            return Ok(Some((
                "draw.transform.apply".into(),
                json!({ "translate": pt(by), "copy": st.mods.alternate }),
            )));
        }
    }
    let mode = if st.mods.constrain { "add" } else { "replace" };
    let rule = cx.ui().drawing().options_of("select").get("rectRule").cloned().unwrap_or(json!("inside"));
    let v = if st.is_click() {
        json!({ "by": "point", "point": pt(p0.d), "mode": mode })
    } else if st.mods.alternate {
        let poly: Vec<Value> = st.samples.iter().map(|x| pt(x.d)).collect();
        json!({ "by": "lasso", "polygon": poly, "rectRule": rule, "mode": mode })
    } else {
        json!({ "by": "rect", "rect": rect(p0.d, pn.d), "rectRule": rule, "mode": mode })
    };
    Ok(Some(("draw.selection.set".into(), v)))
}

/// The current drawing and its pivot.
fn current_drawing(cx: &Cx) -> Option<(tt_model::entities::drawing::Drawing, Point)> {
    let doc = cx.document().ok()?;
    let id = cx.app().current_drawing(doc)?;
    let d = doc.drawing(&id)?.clone();
    let p = Point { x: d.pivot().x, y: d.pivot().y };
    Some((d, p))
}

fn inside_selection(cx: &Cx, d: &tt_model::entities::drawing::Drawing, p: Point) -> bool {
    let ids = crate::commands::draw::selection_ids(cx, d);
    if ids.is_empty() {
        return false;
    }
    crate::commands::draw_select::selection_bounds(cx, d, &ids)
        .ok()
        .flatten()
        .is_some_and(|b| p.x >= b.min_x && p.x <= b.max_x && p.y >= b.min_y && p.y <= b.max_y)
}

/// The Polyline (02 §7.3.5): one anchor per click; a click-drag gives the anchor an out-handle (and the mirrored
/// in-handle unless the alternate modifier breaks the symmetry); closed by clicking within the snap radius of the first
/// anchor, ended by a double click.
fn polyline(cx: &Cx, g: &Gesture) -> Option<(String, Value)> {
    let s = settings(cx);
    let o = cx.ui().drawing().options_of("polyline");
    let snap = o.get("snap").and_then(Value::as_bool).unwrap_or(true);
    let radius = o.get("snapRadius").and_then(Value::as_f64).unwrap_or(8.0);
    let mut points: Vec<Value> = Vec::new();
    let mut first_v: Option<Point> = None;
    let mut closed = false;
    for (i, st) in g.strokes.iter().enumerate() {
        let (Some(a), Some(b)) = (st.first(), st.last()) else { continue };
        if i > 0
            && let Some(f) = first_v
            && (a.v.x - f.x).hypot(a.v.y - f.y) <= radius
            && points.len() >= 2
        {
            closed = true;
            break;
        }
        if i == 0 {
            first_v = Some(a.v);
        }
        let mut p = obj(json!({ "x": a.d.x, "y": a.d.y }));
        if !st.is_click() {
            let out = Point { x: b.d.x - a.d.x, y: b.d.y - a.d.y };
            p.insert("outHandle".into(), pt(out));
            if !st.mods.alternate {
                p.insert("inHandle".into(), pt(Point { x: -out.x, y: -out.y }));
            }
        }
        points.push(Value::Object(p));
    }
    if points.len() < 2 {
        return None;
    }
    let mut m = obj(json!({ "points": points, "closed": closed, "snap": snap, "snapRadius": radius }));
    styled(&mut m, cx, "polyline", &s);
    Some(("draw.polyline.add".into(), Value::Object(m)))
}

// ----- the headless runner ----------------------------------------------------------------------------------------------

fn invalid(pointer: &str, message: impl Into<String>) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, message, json!({ "pointer": pointer }))
}

/// Checks a request before anything runs (limits, finite coordinates, the view and space).
fn check(req: &PointerRequest) -> Result<(), CommandError> {
    if req.events.len() > MAX_EVENTS {
        return Err(CommandError::with_details(
            codes::LIMIT_EXCEEDED,
            format!("a pointer call has at most {MAX_EVENTS} events"),
            json!({ "limit": "pointerEvents", "max": MAX_EVENTS, "actual": req.events.len() }),
        ));
    }
    for (i, e) in req.events.iter().enumerate() {
        for (k, v) in
            [("x", Some(e.x)), ("y", Some(e.y)), ("pressure", e.pressure), ("dt", e.dt), ("wheelDelta", e.wheel_delta)]
        {
            if let Some(v) = v
                && !(v.is_finite() && v.abs() <= MAX_COORD)
            {
                return Err(invalid(&format!("/events/{i}/{k}"), format!("`{k}` must be a finite number")));
            }
        }
        if e.pressure.is_some_and(|p| !(0.0..=1.0).contains(&p)) {
            return Err(invalid(&format!("/events/{i}/pressure"), "pressure is 0-1"));
        }
        if e.dt.is_some_and(|d| d < 0.0) {
            return Err(invalid(&format!("/events/{i}/dt"), "dt is not negative"));
        }
    }
    if matches!(req.space, InputSpace::Scene | InputSpace::Camera) {
        return Err(CommandError::with_details(
            "inspect.spaceUnavailable",
            "`pointer` in the Drawing view takes view or drawing coordinates",
            json!({ "pointer": "/space", "space": req.space }),
        ));
    }
    Ok(())
}

/// The view window a request names (11 §7.6 `viewName | viewId`): a view id, or a view type name (`drawing`,
/// `camera`, `top`, `side`) naming the active view window when it has that type, else the first window of that type.
fn view_of(ui: &crate::ui::UiState, name: &str) -> String {
    let views = &ui.shell().views;
    if views.iter().any(|v| v.view_id == name) {
        return name.to_owned();
    }
    let active = ui.active_view();
    views
        .iter()
        .filter(|v| v.ty.as_str() == name)
        .min_by_key(|v| v.view_id != active)
        .map_or_else(|| name.to_owned(), |v| v.view_id.clone())
}

/// A view point of an event (drawing-space events are converted with the mapping in force).
fn view_point(w: &ViewWindow, space: InputSpace, e: &PointerEvent) -> Point {
    let p = Point { x: e.x, y: e.y };
    if space == InputSpace::Drawing { crate::commands::view::to_view(w, p) } else { p }
}

/// One answered command of a `pointer` call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answered {
    /// The command the gesture produced.
    pub command_id: String,
    /// Its parameters as invoked.
    pub params: Value,
    /// Its envelope.
    pub envelope: Envelope,
}

/// The outcome of a `pointer` call: the commands the gestures produced, in order, and the final hover information.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerOutcome {
    /// The commands invoked, in order (a refused one is the last).
    pub commands: Vec<Answered>,
    /// The final pointer position: view and drawing point.
    pub hover: Option<Value>,
}

impl PointerOutcome {
    /// The envelope the `pointer` tool answers: the first refusal's envelope (its details name the gesture), else
    /// `{commands: [{commandId, params, result, documentRev}], hover}`.
    #[must_use]
    pub fn envelope(&self, rev: Option<u64>) -> Envelope {
        if let Some((i, a)) = self.commands.iter().enumerate().find(|(_, a)| !a.envelope.is_ok()) {
            let mut env = a.envelope.clone();
            if let Some(e) = env.error.as_mut() {
                let mut details = (*e.details).clone();
                if let Value::Object(m) = &mut details {
                    m.insert("gesture".into(), json!(i));
                    m.insert("gestureCommandId".into(), json!(a.command_id));
                }
                e.details = details.into();
            }
            return env;
        }
        let commands: Vec<Value> = self
            .commands
            .iter()
            .map(|a| {
                json!({ "commandId": a.command_id, "params": a.params, "result": a.envelope.result,
                        "documentRev": a.envelope.document_rev })
            })
            .collect();
        Envelope::ok(json!({ "commands": commands, "hover": self.hover }), rev)
    }
}

/// Runs a `pointer` request on the document thread as one request (design note 4 §1-2: gestures of different calls
/// never interleave). Gestures run in order; the first refused gesture ends the call.
pub(crate) fn run(
    core: &mut crate::run::Core,
    origin: &Origin,
    req: &PointerRequest,
) -> Result<PointerOutcome, CommandError> {
    check(req)?;
    let view_id = {
        let s = core.session().ok_or_else(|| CommandError::internal("the session is unavailable"))?;
        let id = view_of(&s.ui, &req.view);
        let w = s.ui.shell().view(&id)?;
        if w.ty != ViewType::Drawing {
            return Err(CommandError::with_details(
                codes::COMMAND_NOT_IMPLEMENTED,
                format!("`pointer` gestures in the {} view arrive with that view (M4 C4.3)", w.ty.as_str()),
                json!({ "viewId": id, "viewType": w.ty, "milestone": "M4 C4.3" }),
            ));
        }
        id
    };
    let invoke = |core: &mut crate::run::Core, command: &str, params: Value| -> Envelope {
        core.invoke(&Invocation {
            command: command.to_owned(),
            params,
            options: InvokeOptions::default(),
            origin: origin.clone(),
        })
    };
    let mut out = PointerOutcome { commands: Vec::new(), hover: None };
    if let Some(t) = &req.tool {
        let params = json!({ "toolId": t });
        let env = invoke(core, "tool.active.set", params.clone());
        let ok = env.is_ok();
        out.commands.push(Answered { command_id: "tool.active.set".into(), params, envelope: env });
        if !ok {
            return Ok(out);
        }
    }
    let mut clock = 0.0f64;
    let mut pending: Option<(Gesture, Option<Stroke>, Option<[f64; 6]>)> = None;
    for (i, e) in req.events.iter().enumerate() {
        clock += e.dt.unwrap_or(0.0);
        let w = core
            .session()
            .and_then(|s| s.ui.shell().view(&view_id).ok().cloned())
            .ok_or_else(|| CommandError::internal("the view is gone"))?;
        let vp = view_point(&w, req.space, e);
        let in_stroke = pending.as_ref().is_some_and(|p| p.1.is_some());
        match e.ty {
            EventType::Hover => {
                if let Some(s) = core.session_mut()
                    && let Ok(v) = s.ui.shell_mut().view_mut(&view_id)
                {
                    v.pointer = Some(vp);
                }
            }
            EventType::Wheel => {
                if in_stroke {
                    return Err(invalid(&format!("/events/{i}"), "view navigation is not allowed during a gesture"));
                }
                let delta = e.wheel_delta.unwrap_or(0.0);
                if delta != 0.0 {
                    let steps = if delta < 0.0 { 1 } else { -1 };
                    let params = json!({ "viewId": view_id, "steps": steps, "anchor": pt(vp) });
                    let env = invoke(core, "view.zoom.set", params.clone());
                    let ok = env.is_ok();
                    out.commands.push(Answered { command_id: "view.zoom.set".into(), params, envelope: env });
                    if !ok {
                        return Ok(out);
                    }
                }
            }
            EventType::Down => {
                if in_stroke {
                    return Err(invalid(&format!("/events/{i}"), "a press while pressed (release first)"));
                }
                let mods = Mods::of(&e.modifiers);
                // The mapping is taken at the press (02 §13.3) and kept for the gesture.
                let matrix = crate::commands::view::view_to_drawing(&w);
                let sample = Sample { v: vp, d: to_drawing(&w, vp), pressure: e.pressure.unwrap_or(1.0), t_ms: clock };
                let stroke = Stroke { samples: vec![sample], mods };
                match pending.as_mut() {
                    // A further click of a Polyline in progress.
                    Some(p) if p.0.tool == "polyline" => {
                        p.1 = Some(stroke);
                        p.2 = Some(matrix);
                    }
                    _ => {
                        let active = core.session().map(|s| s.ui.drawing().active_tool.clone()).unwrap_or_default();
                        let tool = effective_tool(&active, req.pointer_type, mods);
                        let g = Gesture { tool, view_id: view_id.clone(), strokes: Vec::new(), double_click: false };
                        pending = Some((g, Some(stroke), Some(matrix)));
                    }
                }
            }
            EventType::Move | EventType::Up => {
                let Some(p) = pending.as_mut() else {
                    if e.ty == EventType::Move {
                        if let Some(s) = core.session_mut()
                            && let Ok(v) = s.ui.shell_mut().view_mut(&view_id)
                        {
                            v.pointer = Some(vp);
                        }
                        continue;
                    }
                    return Err(invalid(&format!("/events/{i}"), "a release without a press"));
                };
                let Some(stroke) = p.1.as_mut() else {
                    if e.ty == EventType::Up {
                        return Err(invalid(&format!("/events/{i}"), "a release without a press"));
                    }
                    continue;
                };
                let m = p.2.unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                let d = Point { x: m[0] * vp.x + m[2] * vp.y + m[4], y: m[1] * vp.x + m[3] * vp.y + m[5] };
                stroke.samples.push(Sample { v: vp, d, pressure: e.pressure.unwrap_or(1.0), t_ms: clock });
                if e.ty == EventType::Up {
                    if let Some(s) = p.1.take() {
                        p.0.strokes.push(s);
                    }
                    if p.0.tool != "polyline"
                        && let Some((g, _, _)) = pending.take()
                        && !finish(core, origin, &g, &mut out)
                    {
                        return Ok(out);
                    }
                }
            }
            EventType::DoubleClick => {
                if let Some(p) = pending.as_mut()
                    && p.0.tool == "polyline"
                    && p.1.is_none()
                {
                    p.0.double_click = true;
                    if let Some((g, _, _)) = pending.take()
                        && !finish(core, origin, &g, &mut out)
                    {
                        return Ok(out);
                    }
                }
            }
        }
        // A Polyline closes when a click lands on its first anchor.
        if let Some(p) = pending.as_ref()
            && p.0.tool == "polyline"
            && p.1.is_none()
            && polyline_closed(core, &p.0)
            && let Some((g, _, _)) = pending.take()
            && !finish(core, origin, &g, &mut out)
        {
            return Ok(out);
        }
        out.hover = Some(json!({ "viewId": view_id, "view": pt(vp), "drawing": pt(to_drawing(&w, vp)) }));
    }
    // A gesture still pressed at the end of the events is cancelled: nothing is committed (D-7c). An open Polyline
    // stays open across calls only in the GUI; headless, the call's clicks are its gesture and are committed.
    if let Some((g, None, _)) = pending.take()
        && g.tool == "polyline"
    {
        finish(core, origin, &g, &mut out);
    }
    Ok(out)
}

fn polyline_closed(core: &mut crate::run::Core, g: &Gesture) -> bool {
    let n = g.strokes.len();
    if n < 3 {
        return false;
    }
    let radius = core
        .session()
        .and_then(|s| s.ui.drawing().options_of("polyline").get("snapRadius").and_then(Value::as_f64))
        .unwrap_or(8.0);
    let (Some(f), Some(l)) = (g.strokes[0].first(), g.strokes[n - 1].first()) else { return false };
    (f.v.x - l.v.x).hypot(f.v.y - l.v.y) <= radius
}

/// Builds and invokes a finished gesture's command; `false` when it was refused (the call ends).
fn finish(core: &mut crate::run::Core, origin: &Origin, g: &Gesture, out: &mut PointerOutcome) -> bool {
    let built =
        core.with_lent_session(|cx| to_command(cx, g)).unwrap_or_else(|| Err(CommandError::internal("no session")));
    match built {
        Ok(None) => true,
        Ok(Some((command, params))) => {
            let env = core.invoke(&Invocation {
                command: command.clone(),
                params: params.clone(),
                options: InvokeOptions::default(),
                origin: origin.clone(),
            });
            let ok = env.is_ok();
            out.commands.push(Answered { command_id: command, params, envelope: env });
            ok
        }
        Err(e) => {
            let rev = core.session().and_then(|s| s.project.as_ref()).map(|p| p.doc.document_rev());
            out.commands.push(Answered {
                command_id: format!("tool:{}", g.tool),
                params: Value::Null,
                envelope: Envelope::error(e, rev),
            });
            false
        }
    }
}
