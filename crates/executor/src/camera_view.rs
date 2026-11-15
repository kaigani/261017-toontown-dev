//! The Camera view's mapping (M4 C4.3a; views note §6, camera-view note §2): one function per link of the chain view
//! pixels ↔ camera space ↔ camera output pixels ↔ a drawing's units, and scene space ↔ output pixels at a depth.
//!
//! The Camera view's document space is camera space (SI-109: fields from the camera centre on the table plane, roll
//! applied, y up); its pixels per field at 100 % are the camera's output pixels per field, so at 100 % one view pixel
//! is one output pixel. A drawing's units map to output pixels through the final renderer's own plane matrix
//! (`render_list::plane_to_pixel`, SI-145), so the view and a render cannot disagree.

use serde_json::json;
use tt_anim::camera::CameraView;
use tt_model::Id;
use tt_registry::CommandError;

use crate::commands::anim_common::{eval_error, evaluator};
use crate::commands::render_list::{DU_PER_FIELD, plane_to_pixel};
use crate::cx::Cx;
use crate::shell::{self, Point, ViewType, ViewWindow};

/// An affine `[a, b, c, d, e, f]`: `(x, y) → (a x + c y + e, b x + d y + f)`.
pub(crate) type Matrix = [f64; 6];

/// `outer ∘ inner`: first `inner`, then `outer`.
#[must_use]
pub(crate) fn compose(outer: &Matrix, inner: &Matrix) -> Matrix {
    let [a1, b1, c1, d1, e1, f1] = *outer;
    let [a2, b2, c2, d2, e2, f2] = *inner;
    [
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    ]
}

/// The inverse of an affine, `None` when it is singular or not finite.
#[must_use]
pub(crate) fn invert(m: &Matrix) -> Option<Matrix> {
    let [a, b, c, d, e, f] = *m;
    let det = a * d - b * c;
    if !det.is_finite() || det == 0.0 {
        return None;
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    Some([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)])
}

/// Applies an affine to a point.
#[must_use]
pub(crate) fn apply(m: &Matrix, p: Point) -> Point {
    shell::apply(m, p)
}

/// The refusal of a scene without a camera to look through (SI-147: the same as a render's).
pub(crate) fn no_camera(scene: &Id) -> CommandError {
    CommandError::with_details(
        "render.noCamera",
        format!("scene `{scene}` has no camera to look through"),
        json!({ "sceneId": scene.as_str() }),
    )
}

/// The refusal for a Top or Side view window, whose views are M6 C6.1's (C4.3 plan answer 1).
pub(crate) fn staging_owner(w: &ViewWindow, what: &str) -> CommandError {
    CommandError::with_details(
        tt_registry::codes::COMMAND_NOT_IMPLEMENTED,
        format!("{what} of the {} view arrive with that view (M6 C6.1)", w.ty.as_str()),
        json!({ "viewId": w.view_id, "viewType": w.ty, "milestone": "M6 C6.1" }),
    )
}

/// The Camera view's chain at the current frame, at the camera's output size (the project's camera size).
#[derive(Debug, Clone)]
pub(crate) struct Chain {
    /// The scene shown.
    pub scene: Id,
    /// The scene frame shown.
    pub frame: u32,
    /// The current camera.
    pub camera: Id,
    /// The camera at the frame, at the output size.
    pub view: CameraView,
    /// Output size in pixels.
    pub width: u32,
    pub height: u32,
    /// Output pixels → camera space (fields; `render::image_to_camera`).
    pub image_to_camera: Matrix,
    /// Output pixels per field (the Camera view's pixels per document unit at 100 %).
    pub pixels_per_field: f64,
    /// Output pixels → view pixels.
    pub output_to_view: Matrix,
}

/// The scene the session shows: the current scene, else the project's first.
///
/// # Errors
/// `no.project`; `render.noScene`.
pub(crate) fn current_scene(cx: &Cx) -> Result<Id, CommandError> {
    let doc = cx.document()?;
    cx.app()
        .scene
        .clone()
        .filter(|s| doc.scene(s).is_some())
        .or_else(|| doc.scenes().next().map(|s| s.id().clone()))
        .ok_or_else(|| CommandError::with_details("render.noScene", "the project has no scene", json!({})))
}

/// Output pixels → camera space for a camera at depth `depth` (its height above the table plane) and horizontal field
/// of view `fov`, at output size `w × h`; and the output pixels per field (`1 / s`). A camera at or below the table
/// plane falls back to the default camera's scale (12 fields across the frame).
#[must_use]
pub(crate) fn image_to_camera(view: &CameraView, w: u32, h: u32) -> (Matrix, f64) {
    let depth = view.position[2];
    let half = tt_anim::num::tan_deg(view.fov / 2.0);
    let (wf, hf) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let s = if depth > 0.0 && half.is_finite() && half > 0.0 { 2.0 * depth * half / wf } else { 12.0 / wf };
    ([s, 0.0, 0.0, -s, -wf / 2.0 * s, hf / 2.0 * s], 1.0 / s)
}

/// The chain of Camera view window `w` (camera-view note §2, views note §6).
///
/// # Errors
/// `no.project`, `render.noScene`; `render.noCamera` without a current camera (SI-147); the evaluator's refusals.
pub(crate) fn chain(cx: &Cx, w: &ViewWindow) -> Result<Chain, CommandError> {
    let doc = cx.document()?;
    let scene = current_scene(cx)?;
    let camera = doc.scene(&scene).and_then(|s| s.current_camera_id().clone()).ok_or_else(|| no_camera(&scene))?;
    let frame = cx.app().frame.max(1);
    let (width, height) = crate::commands::render::output_size(doc, None, None, 1.0)?;
    let mut ev = evaluator(doc, "mapping the Camera view");
    let mut view = ev.camera(&camera, f64::from(frame)).map_err(eval_error)?;
    view.width = f64::from(width);
    view.height = f64::from(height);
    let (image_to_camera, pixels_per_field) = image_to_camera(&view, width, height);
    let camera_to_view = shell::doc_to_view_matrix(&w.transform(), w.size, pixels_per_field);
    let output_to_view = compose(&camera_to_view, &image_to_camera);
    Ok(Chain { scene, frame, camera, view, width, height, image_to_camera, pixels_per_field, output_to_view })
}

/// The Camera view's pixels per document unit for window `w`: the chain's output pixels per field, or the default
/// camera's (width / 12) when the chain cannot be built (no camera yet).
#[must_use]
pub(crate) fn pixels_per_unit(cx: &Cx, w: &ViewWindow) -> f64 {
    match w.ty {
        ViewType::Drawing => 1.0,
        ViewType::Camera => chain(cx, w).map_or_else(
            |_| {
                let width = cx.document().map_or(1920.0, |d| f64::from(*d.project().camera_width()));
                shell::pixels_per_unit(ViewType::Camera, width)
            },
            |c| c.pixels_per_field,
        ),
        _ => shell::pixels_per_unit(w.ty, 1920.0),
    }
}

/// An element's plane: drawing units → output pixels at the chain's frame (`None` when the plane is at or behind the
/// near plane), the final renderer's matrix.
///
/// # Errors
/// The evaluator's refusals.
pub(crate) fn element_to_output(cx: &Cx, chain: &Chain, element: &Id) -> Result<Option<Matrix>, CommandError> {
    let doc = cx.document()?;
    let mut ev = evaluator(doc, "mapping the Camera view");
    let world = ev.world(element, f64::from(chain.frame)).map_err(eval_error)?;
    Ok(plane_to_pixel(&chain.view, &world).map(|a| a.0))
}

/// An element's plane: drawing units → view pixels of the Camera view (`None` behind the near plane).
///
/// # Errors
/// The evaluator's refusals.
pub(crate) fn element_to_view(cx: &Cx, chain: &Chain, element: &Id) -> Result<Option<Matrix>, CommandError> {
    Ok(element_to_output(cx, chain, element)?.map(|m| compose(&chain.output_to_view, &m)))
}

/// A scene-space point (fields) addressed to an element's drawing → drawing units: the point of the element's plane
/// with those scene X and Y at `frame` (SI-146).
///
/// # Errors
/// `arg.invalid` when the plane is seen edge-on in X/Y (singular); the evaluator's refusals.
pub(crate) fn scene_to_drawing(cx: &Cx, element: &Id, frame: u32, p: Point) -> Result<Point, CommandError> {
    let m = plane_xy(cx, element, frame)?;
    let inv = invert(&m).ok_or_else(|| {
        CommandError::with_details(
            tt_registry::codes::ARG_INVALID,
            "the element's plane is edge-on in scene X and Y: a scene point does not name one of its points",
            json!({ "pointer": "/space", "elementId": element.as_str() }),
        )
    })?;
    Ok(apply(&inv, p))
}

/// Drawing units of an element's drawing → the scene X and Y of that point of its plane at `frame` (SI-146).
///
/// # Errors
/// The evaluator's refusals.
pub(crate) fn drawing_to_scene(cx: &Cx, element: &Id, frame: u32, p: Point) -> Result<Point, CommandError> {
    Ok(apply(&plane_xy(cx, element, frame)?, p))
}

/// Drawing units → scene X and Y (fields) of an element's plane at `frame`.
fn plane_xy(cx: &Cx, element: &Id, frame: u32) -> Result<Matrix, CommandError> {
    let doc = cx.document()?;
    let mut ev = evaluator(doc, "converting scene coordinates");
    let w = ev.world(element, f64::from(frame.max(1))).map_err(eval_error)?;
    let o = w.apply([0.0, 0.0, 0.0]);
    let u = w.apply([1.0 / DU_PER_FIELD, 0.0, 0.0]);
    let v = w.apply([0.0, 1.0 / DU_PER_FIELD, 0.0]);
    Ok([u[0] - o[0], u[1] - o[1], v[0] - o[0], v[1] - o[1], o[0], o[1]])
}

/// A scene point (fields, three coordinates) → output pixels; `None` at or behind the near plane.
#[must_use]
pub(crate) fn scene_to_output(chain: &Chain, p: [f64; 3]) -> Option<Point> {
    chain.view.project(p).pixel.map(|q| Point { x: q[0], y: q[1] })
}

/// Output pixels → the scene point at `depth` in front of the camera (05 line 1234's `depth`).
#[must_use]
pub(crate) fn output_to_scene(chain: &Chain, p: Point, depth: f64) -> Option<[f64; 3]> {
    chain.view.unproject([p.x, p.y], depth)
}

/// The view window a call's view-space points refer to: `view` names a view id or `drawing`; by default the active view
/// window when it shows the Drawing view, else `drawingView` (the C4.2 default, unchanged: a Camera view is used when
/// a call names it).
///
/// # Errors
/// `view.notFound`.
pub(crate) fn resolve_view<'a>(cx: &'a Cx, view: Option<&str>) -> Result<&'a ViewWindow, CommandError> {
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
    shell.view(&id)
}

/// The element whose drawing a Camera-view or scene-space point addresses: the current element.
///
/// # Errors
/// `arg.invalid` without a current element.
pub(crate) fn current_element(cx: &Cx) -> Result<Id, CommandError> {
    cx.app().element.clone().ok_or_else(|| {
        CommandError::with_details(
            tt_registry::codes::ARG_INVALID,
            "scene and Camera-view points need a current element (its drawing is the one addressed)",
            json!({ "pointer": "/space" }),
        )
    })
}

/// The scale of an affine (its linear part's area factor, square-rooted), 1 when degenerate.
#[must_use]
pub(crate) fn scale_of(m: &Matrix) -> f64 {
    let s = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    if s.is_finite() && s > 0.0 { s } else { 1.0 }
}

/// How a call's points reach drawing units (02 §13.1 item 5; views note §6): the matrix from the given space to the
/// current drawing's units (`None` for drawing space) and the view pixels per drawing unit its view-pixel tolerances
/// are divided by (1 without a view). `drawing`: as given; `view`: the Drawing view's mapping, or in a Camera view the
/// current element's plane through the camera and the view transform; `scene`: the current element's plane at the
/// frame (SI-146). Top and Side views refuse (M6 C6.1).
///
/// # Errors
/// `arg.invalid` for an unknown space or without a current element; `view.notFound`; `command.notImplemented` for a
/// Top or Side view; `render.noCamera`; the evaluator's refusals.
pub(crate) fn addressing(
    cx: &Cx,
    space: &str,
    view: Option<&str>,
    frame: Option<u32>,
) -> Result<(Option<Matrix>, f64), CommandError> {
    let window_scale = |w: &ViewWindow| -> Result<(Option<Matrix>, f64), CommandError> {
        match w.ty {
            ViewType::Drawing => {
                let m = crate::commands::view::view_to_drawing(w);
                Ok((Some(m), crate::commands::view::pixels_per_du(w)))
            }
            ViewType::Camera => {
                let c = chain(cx, w)?;
                let e = current_element(cx)?;
                let to_view = element_to_view(cx, &c, &e)?.ok_or_else(|| behind(&e))?;
                let inv = invert(&to_view).ok_or_else(|| behind(&e))?;
                Ok((Some(inv), scale_of(&to_view)))
            }
            _ => Err(staging_owner(w, "view-space points")),
        }
    };
    match space {
        "drawing" => match view {
            None => Ok((None, 1.0)),
            Some(v) => Ok((None, window_scale(resolve_view(cx, Some(v))?)?.1)),
        },
        "view" => window_scale(resolve_view(cx, view)?),
        "scene" => {
            let e = current_element(cx)?;
            let f = frame.unwrap_or(cx.app().frame);
            let m = plane_xy(cx, &e, f)?;
            let inv = invert(&m).ok_or_else(|| {
                CommandError::with_details(
                    tt_registry::codes::ARG_INVALID,
                    "the element's plane is edge-on in scene X and Y: a scene point does not name one of its points",
                    json!({ "pointer": "/space", "elementId": e.as_str() }),
                )
            })?;
            let scale = match view {
                Some(v) => window_scale(resolve_view(cx, Some(v))?)?.1,
                None => 1.0,
            };
            Ok((Some(inv), scale))
        }
        other => Err(CommandError::with_details(
            tt_registry::codes::ARG_INVALID,
            format!("unknown space `{other}`"),
            json!({ "pointer": "/space" }),
        )),
    }
}

/// The refusal for an element whose plane is at or behind the camera's near plane.
pub(crate) fn behind(e: &Id) -> CommandError {
    CommandError::with_details(
        "inspect.behindCamera",
        "the element's plane is at or behind the camera's near plane: Camera-view points do not reach its drawing",
        json!({ "elementId": e.as_str() }),
    )
}
