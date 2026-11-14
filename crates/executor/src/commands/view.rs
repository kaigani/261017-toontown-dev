//! View windows (01 §6.5, §12.2; 02 §2.2, §4.3; M4 C4.2, reviews/M4-design-views.md): the four view windows' types and
//! transforms, the one mapping between view pixels and drawing space, and its conversions. View state is session UI
//! state: not undoable (02 §12.1) and not persisted (SI-128). Every change is announced to the host as `shell.view`
//! (the canvas follows it; agents do not receive `shell.*` events).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_registry::{CommandError, codes, command};

use crate::cx::Cx;
use crate::shell::{self, Point, Size, ViewTransform, ViewType, ViewWindow};
use crate::types::Origin;

// ----- the mapping ----------------------------------------------------------------------------------------------------

/// The affine `[a, b, c, d, e, f]` (x' = a x + c y + e, y' = b x + d y + f) of drawing space → view pixels for a view
/// window's current transform and size (the one mapping, design note 6 §2; the coefficients conversions apply).
#[must_use]
pub(crate) fn drawing_to_view(w: &ViewWindow) -> [f64; 6] {
    shell::doc_to_view_matrix(&w.transform(), w.size, shell::pixels_per_unit(w.ty, 1920.0))
}

/// The inverse affine (view pixels → drawing space).
#[must_use]
pub(crate) fn view_to_drawing(w: &ViewWindow) -> [f64; 6] {
    shell::view_to_doc_matrix(&w.transform(), w.size, shell::pixels_per_unit(w.ty, 1920.0))
}

/// A view point → drawing space with a window's mapping (the function itself, not the matrix).
#[must_use]
pub(crate) fn to_drawing(w: &ViewWindow, v: Point) -> Point {
    shell::view_to_doc(&w.transform(), w.size, shell::pixels_per_unit(w.ty, 1920.0), v)
}

/// A drawing point → view pixels with a window's mapping.
#[must_use]
pub(crate) fn to_view(w: &ViewWindow, d: Point) -> Point {
    shell::doc_to_view(&w.transform(), w.size, shell::pixels_per_unit(w.ty, 1920.0), d)
}

/// The Drawing view window view-space points refer to (SI-065, SI-109): `view` names a view id or `drawing`; by
/// default the active view window when it shows the Drawing view, else `drawingView`. View space of the other view
/// types arrives with them (M4 C4.3).
///
/// # Errors
/// `view.notFound`; `command.notImplemented` for a view window that does not show the Drawing view.
pub(crate) fn drawing_view<'a>(cx: &'a Cx, view: Option<&str>) -> Result<&'a ViewWindow, CommandError> {
    let shell = cx.ui().shell();
    let id = match view {
        Some("drawing") | None => {
            let active = cx.ui().active_view();
            if view.is_none() && shell.view(active).is_ok_and(|w| w.ty == ViewType::Drawing) {
                active.to_owned()
            } else {
                "drawingView".to_owned()
            }
        }
        Some(v) => v.to_owned(),
    };
    let w = shell.view(&id)?;
    if w.ty != ViewType::Drawing {
        return Err(CommandError::with_details(
            codes::COMMAND_NOT_IMPLEMENTED,
            format!("view space of the {} view arrives with that view (M4 C4.3)", w.ty.as_str()),
            json!({ "viewId": id, "viewType": w.ty, "milestone": "M4 C4.3" }),
        ));
    }
    Ok(w)
}

/// The Drawing view's zoom in view pixels per drawing unit (tolerances given in view pixels become `px / zoom` du).
#[must_use]
pub(crate) fn pixels_per_du(w: &ViewWindow) -> f64 {
    let m = drawing_to_view(w);
    let s = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    if s.is_finite() && s > 0.0 { s } else { 1.0 }
}

// ----- shared ---------------------------------------------------------------------------------------------------------

/// The view a command addresses: `viewId`, else the active view window (01 §4).
fn view_id(cx: &Cx, given: Option<&str>) -> Result<String, CommandError> {
    let id = given.map_or_else(|| cx.ui().active_view().to_owned(), str::to_owned);
    cx.ui().shell().view(&id).map(|_| id)
}

/// An agent may move the user's views only with `letAgentMoveView` (11 §6.4; SI-128).
fn agent_may_move(cx: &Cx) -> Result<(), CommandError> {
    if !matches!(cx.origin(), Origin::Agent { .. }) {
        return Ok(());
    }
    let allowed = cx
        .pref("agent.policy.defaults")
        .and_then(|v| v.get("letAgentMoveView").and_then(Value::as_bool))
        .unwrap_or(true);
    if allowed {
        Ok(())
    } else {
        Err(CommandError::with_details(
            codes::PERMISSION_DENIED,
            "the user does not let agents move their views (Let the agent move my view is off)",
            json!({ "preference": "letAgentMoveView" }),
        ))
    }
}

/// Announces a view window's state to the host (`shell.view`).
fn announce(cx: &mut Cx, id: &str) {
    let Ok(w) = cx.ui().shell().view(id) else { return };
    let state = json!({
        "viewId": w.view_id,
        "type": w.ty.as_str(),
        "size": w.size,
        "transform": w.transform(),
        "drawingToView": drawing_to_view(w),
        "viewToDrawing": view_to_drawing(w),
    });
    cx.emit("shell.view", state);
}

/// Changes a view window's transform (after the agent check) and announces it.
fn change(
    cx: &mut Cx,
    id: &str,
    f: impl FnOnce(&ViewWindow, ViewTransform) -> ViewTransform,
) -> Result<ViewTransform, CommandError> {
    agent_may_move(cx)?;
    let w = cx.ui().shell().view(id)?.clone();
    let next = f(&w, w.transform());
    let next = ViewTransform {
        zoom: next.zoom.clamp(shell::ZOOM_MIN, shell::ZOOM_MAX),
        rotation_degrees: shell::normalise_degrees(next.rotation_degrees),
        centre: next.centre,
    };
    if next != w.transform() {
        cx.ui_mut().shell_mut().view_mut(id)?.set_transform(next);
        announce(cx, id);
    }
    Ok(next)
}

/// The centre that keeps the drawing point under view point `anchor` fixed when `t` gets `zoom`.
fn anchored(w: &ViewWindow, t: ViewTransform, zoom: f64, anchor: Point) -> Point {
    let ppu = shell::pixels_per_unit(w.ty, 1920.0);
    let under = shell::view_to_doc(&t, w.size, ppu, anchor);
    let at_origin = ViewTransform { zoom, centre: Point { x: 0.0, y: 0.0 }, ..t };
    let off = shell::view_to_doc(&at_origin, w.size, ppu, anchor);
    Point { x: under.x - off.x, y: under.y - off.y }
}

fn finite(p: Point, pointer: &str) -> Result<Point, CommandError> {
    if p.x.is_finite() && p.y.is_finite() && p.x.abs() <= 1e7 && p.y.abs() <= 1e7 {
        Ok(p)
    } else {
        Err(CommandError::with_details(
            codes::ARG_INVALID,
            "a point needs finite coordinates within ±10 000 000",
            json!({ "pointer": pointer }),
        ))
    }
}

fn exactly_one(given: &[(&str, bool)]) -> Result<(), CommandError> {
    let n = given.iter().filter(|(_, g)| *g).count();
    let names: Vec<&str> = given.iter().map(|(k, _)| *k).collect();
    match n {
        1 => Ok(()),
        0 => Err(CommandError::with_details(
            codes::ARG_MISSING,
            format!("give one of {}", names.join(", ")),
            json!({ "pointer": format!("/{}", names[0]) }),
        )),
        _ => Err(CommandError::with_details(
            codes::ARG_INVALID,
            format!("give exactly one of {}", names.join(", ")),
            json!({ "pointer": format!("/{}", names[0]) }),
        )),
    }
}

/// The one-of groups of the navigation commands, checked by availability as the handler does (a bare reference names
/// none of them, so it is refused with the handler's `arg.missing` rather than reported enabled).
fn one_of(v: &Value, keys: &[&str]) -> Result<(), CommandError> {
    let given: Vec<(&str, bool)> =
        keys.iter().map(|k| (*k, v.get(*k).is_some_and(|x| !x.is_null() && x != &Value::Bool(false)))).collect();
    exactly_one(&given)
}

pub(crate) fn precheck_zoom(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    one_of(v, &["zoom", "steps"])
}

pub(crate) fn precheck_rotation(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    one_of(v, &["degrees", "steps"])
}

pub(crate) fn precheck_pan(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    one_of(v, &["centre", "by", "atPointer"])
}

// ----- view.list ------------------------------------------------------------------------------------------------------

/// A view window entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewEntry {
    /// View id (= its panel id, SI-112).
    view_id: String,
    /// Panel id.
    panel_id: String,
    /// Current type.
    #[serde(rename = "type")]
    ty: ViewType,
    /// The active view window.
    focused: bool,
    /// Canvas size in logical pixels.
    size: Size,
    /// The current type's transform.
    transform: ViewTransform,
}

#[command(
    id = "view.list",
    kind = query,
    label = "List views",
    scopes = [read],
    needs_project,
    example(title = "The four view windows", params = "{}"),
    gui = "the View menu and view toolbars (01); wired in M4 C4.2",
)]
pub(crate) fn view_list(cx: &mut Cx, _p: super::common::NoParams) -> Result<Vec<ViewEntry>, CommandError> {
    let active = cx.ui().active_view().to_owned();
    Ok(cx
        .ui()
        .shell()
        .views
        .iter()
        .map(|w| ViewEntry {
            view_id: w.view_id.clone(),
            panel_id: w.view_id.clone(),
            ty: w.ty,
            focused: w.view_id == active,
            size: w.size,
            transform: w.transform(),
        })
        .collect())
}

// ----- view.active.set ------------------------------------------------------------------------------------------------

/// `view.active.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ActiveParams {
    /// The view window to activate.
    view_id: String,
}

/// `{viewId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewIdResult {
    /// View id.
    view_id: String,
}

#[command(
    id = "view.active.set",
    kind = ui,
    label = "Activate view",
    scopes = [ui],
    needs_project,
    example(title = "Activate the Drawing view", params = r#"{"viewId": "drawingView"}"#),
    gui = "Click into a view window; Window menu (M4 C4.2)",
)]
pub(crate) fn view_active_set(cx: &mut Cx, p: ActiveParams) -> Result<ViewIdResult, CommandError> {
    let id = view_id(cx, Some(&p.view_id))?;
    agent_may_move(cx)?;
    if !cx.ui().panel_visible(&id) {
        cx.ui_mut().set_panel(&id, Some(true), None, true, None)?;
    }
    // An event only for an effective change (the C4.1 change-event rule).
    if cx.ui().active_view() != id {
        cx.ui_mut().shell_mut().active_view = Some(id.clone());
        announce(cx, &id);
    } else {
        cx.ui_mut().shell_mut().active_view = Some(id.clone());
    }
    Ok(ViewIdResult { view_id: id })
}

// ----- view.type.set --------------------------------------------------------------------------------------------------

/// A view type, or `previous` (switch active view, 01 §4).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TypeParam {
    /// Drawing view.
    Drawing,
    /// Camera view.
    Camera,
    /// Top view.
    Top,
    /// Side view.
    Side,
    /// The type before the last switch.
    Previous,
}

/// `view.type.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TypeParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
    /// The type.
    #[serde(rename = "type")]
    ty: TypeParam,
}

/// `{viewId, type}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeResult {
    /// View id.
    view_id: String,
    /// Its type now.
    #[serde(rename = "type")]
    ty: ViewType,
}

#[command(
    id = "view.type.set",
    kind = ui,
    label = "Set view type",
    scopes = [ui],
    needs_project,
    example(title = "Show the Drawing view in the active window", params = r#"{"type": "drawing"}"#),
    example(title = "Switch active view", params = r#"{"type": "previous"}"#),
    gui = "View window type switch; View > Switch Active View (M4 C4.2)",
)]
pub(crate) fn view_type_set(cx: &mut Cx, p: TypeParams) -> Result<TypeResult, CommandError> {
    let id = view_id(cx, p.view_id.as_deref())?;
    agent_may_move(cx)?;
    let w = cx.ui_mut().shell_mut().view_mut(&id)?;
    let next = match p.ty {
        TypeParam::Drawing => ViewType::Drawing,
        TypeParam::Camera => ViewType::Camera,
        TypeParam::Top => ViewType::Top,
        TypeParam::Side => ViewType::Side,
        TypeParam::Previous => w.previous.unwrap_or(w.ty),
    };
    if next != w.ty {
        w.previous = Some(w.ty);
        w.ty = next;
        announce(cx, &id);
    }
    Ok(TypeResult { view_id: id, ty: next })
}

// ----- view.transform.get ---------------------------------------------------------------------------------------------

/// `viewId?`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ViewParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
}

#[command(
    id = "view.transform.get",
    kind = query,
    label = "Get view transform",
    scopes = [read],
    needs_project,
    example(title = "The active view's zoom, rotation and centre", params = "{}"),
    gui = "the view's zoom and rotation display (01 §5.3); wired in M4 C4.2",
)]
pub(crate) fn view_transform_get(cx: &mut Cx, p: ViewParams) -> Result<ViewTransform, CommandError> {
    let id = view_id(cx, p.view_id.as_deref())?;
    Ok(cx.ui().shell().view(&id)?.transform())
}

// ----- view.zoom.set --------------------------------------------------------------------------------------------------

/// `view.zoom.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ZoomParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
    /// Absolute zoom factor.
    #[serde(default)]
    #[schemars(range(min = 0.0625, max = 64.0))]
    zoom: Option<f64>,
    /// Table steps (zoom in = +1, zoom out = −1).
    #[serde(default)]
    #[schemars(range(min = -64, max = 64))]
    steps: Option<i64>,
    /// The view point that stays fixed (view pixels; default the view centre).
    #[serde(default)]
    anchor: Option<Point>,
}

#[command(
    id = "view.zoom.set",
    precheck = precheck_zoom,
    kind = ui,
    label = "Zoom",
    scopes = [ui],
    needs_project,
    example(title = "Zoom in one step", params = r#"{"steps": 1}"#),
    example(title = "200 % in the Drawing view", params = r#"{"viewId": "drawingView", "zoom": 2}"#),
    gui = "View > Zoom In / Zoom Out / Reset Zoom; wheel; Zoom tool (M4 C4.2)",
)]
pub(crate) fn view_zoom_set(cx: &mut Cx, p: ZoomParams) -> Result<ViewTransform, CommandError> {
    exactly_one(&[("zoom", p.zoom.is_some()), ("steps", p.steps.is_some())])?;
    let anchor = p.anchor.map(|a| finite(a, "/anchor")).transpose()?;
    let id = view_id(cx, p.view_id.as_deref())?;
    change(cx, &id, |w, t| {
        let zoom = match (p.zoom, p.steps) {
            (Some(z), _) => z,
            (None, Some(n)) => shell::zoom_step(t.zoom, n),
            (None, None) => t.zoom,
        }
        .clamp(shell::ZOOM_MIN, shell::ZOOM_MAX);
        let centre = match anchor {
            Some(a) => anchored(w, t, zoom, a),
            None => t.centre,
        };
        ViewTransform { zoom, centre, ..t }
    })
}

// ----- view.rotation.set ----------------------------------------------------------------------------------------------

/// `view.rotation.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RotationParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
    /// Absolute rotation in degrees (clockwise positive in the view).
    #[serde(default)]
    #[schemars(range(min = -360.0, max = 360.0))]
    degrees: Option<f64>,
    /// 30° steps (clockwise = +1).
    #[serde(default)]
    #[schemars(range(min = -12, max = 12))]
    steps: Option<i64>,
}

#[command(
    id = "view.rotation.set",
    precheck = precheck_rotation,
    kind = ui,
    label = "Rotate view",
    scopes = [ui],
    needs_project,
    example(title = "Rotate the view 30° clockwise", params = r#"{"steps": 1}"#),
    example(title = "Rotate the Drawing view by 45°", params = r#"{"viewId": "drawingView", "degrees": 45}"#),
    gui = "View > Rotate View Clockwise / Counter-clockwise / Reset Rotation (M4 C4.2)",
)]
pub(crate) fn view_rotation_set(cx: &mut Cx, p: RotationParams) -> Result<ViewTransform, CommandError> {
    exactly_one(&[("degrees", p.degrees.is_some()), ("steps", p.steps.is_some())])?;
    let id = view_id(cx, p.view_id.as_deref())?;
    change(cx, &id, |_, t| {
        let rotation_degrees = match (p.degrees, p.steps) {
            (Some(d), _) => d,
            (None, Some(n)) => t.rotation_degrees + f64::from(i32::try_from(n).unwrap_or(0)) * shell::ROTATION_STEP,
            (None, None) => t.rotation_degrees,
        };
        ViewTransform { rotation_degrees, ..t }
    })
}

// ----- view.pan.set ---------------------------------------------------------------------------------------------------

/// `view.pan.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PanParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
    /// The document point to centre on.
    #[serde(default)]
    centre: Option<Point>,
    /// A pan by view pixels (the content moves by this offset).
    #[serde(default)]
    by: Option<Point>,
    /// Centre on the pointer.
    #[serde(default)]
    at_pointer: bool,
}

#[command(
    id = "view.pan.set",
    precheck = precheck_pan,
    kind = ui,
    label = "Pan view",
    scopes = [ui],
    needs_project,
    example(title = "Centre the view on a drawing point", params = r#"{"viewId": "drawingView", "centre": {"x": 100, "y": 50}}"#),
    example(title = "Pan by 40 pixels to the right", params = r#"{"by": {"x": 40, "y": 0}}"#),
    gui = "Grabber tool; Space-drag; View > Centre on Pointer / Recentre (M4 C4.2)",
)]
pub(crate) fn view_pan_set(cx: &mut Cx, p: PanParams) -> Result<ViewTransform, CommandError> {
    exactly_one(&[("centre", p.centre.is_some()), ("by", p.by.is_some()), ("atPointer", p.at_pointer)])?;
    let centre = p.centre.map(|c| finite(c, "/centre")).transpose()?;
    let by = p.by.map(|b| finite(b, "/by")).transpose()?;
    let id = view_id(cx, p.view_id.as_deref())?;
    let pointer = cx.ui().shell().view(&id)?.pointer;
    if p.at_pointer && pointer.is_none() {
        return Err(CommandError::with_details(
            codes::ARG_INVALID,
            "the pointer is not over this view",
            json!({ "pointer": "/atPointer", "viewId": id }),
        ));
    }
    change(cx, &id, |w, t| {
        let c = match (centre, by, pointer) {
            (Some(c), _, _) => c,
            (None, Some(b), _) => {
                // The content moves by `b`: the new centre is the drawing point now at (centre − b).
                let mid = Point { x: w.size.width / 2.0 - b.x, y: w.size.height / 2.0 - b.y };
                to_drawing(w, mid)
            }
            (None, None, Some(at)) => to_drawing(w, at),
            (None, None, None) => t.centre,
        };
        ViewTransform { centre: c, ..t }
    })
}

// ----- view.transform.reset -------------------------------------------------------------------------------------------

/// A part of a view transform.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Part {
    /// Zoom back to 100 %.
    Zoom,
    /// Rotation back to 0°.
    Rotation,
    /// Recentre on the drawing centre.
    Pan,
}

fn all_parts() -> Vec<Part> {
    vec![Part::Zoom, Part::Rotation, Part::Pan]
}

/// `view.transform.reset` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ResetParams {
    /// View window (default: the active one).
    #[serde(default)]
    view_id: Option<String>,
    /// Parts to reset.
    #[serde(default = "all_parts")]
    parts: Vec<Part>,
}

#[command(
    id = "view.transform.reset",
    kind = ui,
    label = "Reset view",
    scopes = [ui],
    needs_project,
    example(title = "Reset zoom, rotation and pan", params = "{}"),
    example(title = "Reset the rotation only", params = r#"{"parts": ["rotation"]}"#),
    gui = "View > Reset View / Reset Zoom / Reset Rotation / Recentre (M4 C4.2)",
)]
pub(crate) fn view_transform_reset(cx: &mut Cx, p: ResetParams) -> Result<ViewTransform, CommandError> {
    let id = view_id(cx, p.view_id.as_deref())?;
    change(cx, &id, |_, t| ViewTransform {
        zoom: if p.parts.contains(&Part::Zoom) { 1.0 } else { t.zoom },
        rotation_degrees: if p.parts.contains(&Part::Rotation) { 0.0 } else { t.rotation_degrees },
        centre: if p.parts.contains(&Part::Pan) { Point { x: 0.0, y: 0.0 } } else { t.centre },
    })
}

// ----- view.coordinates.convert ---------------------------------------------------------------------------------------

/// A space a view converts between.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Space {
    /// View pixels (y down).
    View,
    /// Drawing space of the view's drawing (du, y up).
    Drawing,
    /// Scene space (fields) under the current element's transform at the current frame.
    Scene,
}

/// `view.coordinates.convert` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ConvertParams {
    /// View window.
    view_id: String,
    /// The point.
    point: Point,
    /// Its space.
    from: Space,
    /// The space wanted.
    to: Space,
}

/// `{point}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PointResult {
    /// The converted point.
    point: Point,
}

/// Drawing → scene (or back) under the current element's world transform at the current frame.
fn scene_of(cx: &Cx, p: Point, into_scene: bool) -> Result<Point, CommandError> {
    let doc = cx.document()?;
    let element = cx.app().element.clone().ok_or_else(|| {
        CommandError::with_details(
            codes::ARG_INVALID,
            "scene space needs a current element (the view's drawing belongs to it)",
            json!({ "pointer": "/to" }),
        )
    })?;
    let mut ev = super::anim_common::evaluator(doc, "converting view coordinates");
    let w = ev.world(&element, f64::from(cx.app().frame)).map_err(super::anim_common::eval_error)?;
    const DU_PER_FIELD: f64 = 100.0;
    if into_scene {
        let s = w.apply([p.x / DU_PER_FIELD, p.y / DU_PER_FIELD, 0.0]);
        Ok(Point { x: s[0], y: s[1] })
    } else {
        let inv = w.inverse().ok_or_else(|| {
            CommandError::with_details(
                codes::ARG_INVALID,
                "the element's transform is singular",
                json!({ "pointer": "/from" }),
            )
        })?;
        let l = inv.apply([p.x, p.y, 0.0]);
        Ok(Point { x: l[0] * DU_PER_FIELD, y: l[1] * DU_PER_FIELD })
    }
}

/// Conversions in a view window showing a staging view arrive with the Camera view (M4 C4.3); the precheck and the
/// handler give the same refusal (ruling O-24; C42A-R1-08).
fn staging_refusal(w: &ViewWindow, converts: bool) -> Result<(), CommandError> {
    if w.ty != ViewType::Drawing && converts {
        return Err(CommandError::with_details(
            codes::COMMAND_NOT_IMPLEMENTED,
            format!("conversions in the {} view arrive with that view (M4 C4.3)", w.ty.as_str()),
            json!({ "pointer": "/viewId", "viewId": w.view_id, "viewType": w.ty, "milestone": "M4 C4.3" }),
        ));
    }
    Ok(())
}

/// `view.coordinates.convert`: the view window's type (O(1)).
pub(crate) fn precheck_convert(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let Some(id) = v.get("viewId").and_then(Value::as_str) else { return Ok(()) };
    let Ok(w) = cx.ui().shell().view(id) else { return Ok(()) };
    staging_refusal(w, v.get("from") != v.get("to"))
}

#[command(
    id = "view.coordinates.convert",
    precheck = precheck_convert,
    kind = query,
    label = "Convert view coordinates",
    scopes = [read],
    needs_project,
    example(title = "A Drawing-view pixel in drawing space", params = r#"{"viewId": "drawingView", "point": {"x": 960, "y": 540}, "from": "view", "to": "drawing"}"#),
    gui = "the status bar's pointer position (02 §2.2); wired in M4 C4.2",
)]
pub(crate) fn view_coordinates_convert(cx: &mut Cx, p: ConvertParams) -> Result<PointResult, CommandError> {
    let point = finite(p.point, "/point")?;
    let w = cx.ui().shell().view(&p.view_id)?.clone();
    staging_refusal(&w, p.from != p.to)?;
    // Into drawing space, then out of it.
    let d = match p.from {
        Space::View => to_drawing(&w, point),
        Space::Drawing => point,
        Space::Scene => scene_of(cx, point, false)?,
    };
    let out = match p.to {
        Space::View => to_view(&w, d),
        Space::Drawing => d,
        Space::Scene => scene_of(cx, d, true)?,
    };
    Ok(PointResult { point: out })
}
