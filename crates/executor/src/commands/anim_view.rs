//! `anim.view.coords.get` (05 §19.3 line 1234; M4 C4.3a, C4.3 plan answer 2): scene points and a Camera view's pixels,
//! for pointer simulation. `anim.view.type.set`, `anim.view.slot.select` and `anim.view.get` stay with M6 (SI-143).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::{CommandError, codes, command};

use crate::cx::Cx;
use crate::shell::{Point, ViewType};

/// The space of `points`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum From {
    /// Scene points (fields).
    Scene,
    /// View pixels.
    View,
}

/// A point of `anim.view.coords.get`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pt {
    /// X.
    x: f64,
    /// Y.
    y: f64,
    /// Scene Z (scene points; a view point's answer carries it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    z: Option<f64>,
}

/// `anim.view.coords.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CoordsParams {
    /// A Camera view window.
    view_id: String,
    /// The points.
    #[schemars(length(min = 1, max = 100_000))]
    points: Vec<Pt>,
    /// Their space.
    from: From,
    /// The scene Z of the plane view points are unprojected onto, and of scene points given without `z` (SI-149).
    #[serde(default)]
    depth: f64,
}

/// `{points}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct CoordsResult {
    /// The converted points.
    points: Vec<Pt>,
}

fn behind(i: usize) -> CommandError {
    CommandError::with_details(
        "inspect.behindCamera",
        "the point is at or behind the camera's near plane",
        json!({ "pointer": format!("/points/{i}") }),
    )
}

/// `anim.view.coords.get`: the view window's type (O(1); the precheck and the handler give the same refusal).
pub(crate) fn precheck_coords(cx: &Cx, v: &serde_json::Value) -> Result<(), CommandError> {
    let Some(id) = v.get("viewId").and_then(serde_json::Value::as_str) else { return Ok(()) };
    let Ok(w) = cx.ui().shell().view(id) else { return Ok(()) };
    view_type_refusal(w)
}

fn view_type_refusal(w: &crate::shell::ViewWindow) -> Result<(), CommandError> {
    match w.ty {
        ViewType::Camera => Ok(()),
        ViewType::Drawing => Err(CommandError::with_details(
            codes::ARG_INVALID,
            "anim.view.coords.get maps scene points in a scene view; the Drawing view converts with view.coordinates.convert",
            json!({ "pointer": "/viewId", "viewId": w.view_id, "viewType": w.ty }),
        )),
        _ => Err(crate::camera_view::staging_owner(w, "scene coordinates")),
    }
}

#[command(
    id = "anim.view.coords.get",
    precheck = precheck_coords,
    kind = query,
    label = "Map scene and view coordinates",
    scopes = [read],
    needs_project,
    example(title = "The table-plane origin in the Camera view", params = r#"{"viewId": "cameraView", "points": [{"x": 0, "y": 0}], "from": "scene"}"#),
    agent_only = "agents map scene points to view pixels for pointer simulation (05 §19.3)",
)]
pub(crate) fn anim_view_coords_get(cx: &mut Cx, p: CoordsParams) -> Result<CoordsResult, CommandError> {
    let w = cx.ui().shell().view(&p.view_id)?.clone();
    view_type_refusal(&w)?;
    if !p.depth.is_finite() {
        return Err(super::common::invalid("/depth", "depth must be finite"));
    }
    let chain = crate::camera_view::chain(cx, &w)?;
    let view_from_output = chain.output_to_view;
    let output_from_view = crate::camera_view::invert(&view_from_output)
        .ok_or_else(|| super::common::invalid("/viewId", "the Camera view's transform is singular"))?;
    let mut out = Vec::with_capacity(p.points.len());
    for (i, q) in p.points.iter().enumerate() {
        if !(q.x.is_finite() && q.y.is_finite() && q.z.is_none_or(f64::is_finite)) {
            return Err(super::common::invalid(&format!("/points/{i}"), "points must be finite"));
        }
        match p.from {
            From::Scene => {
                let z = q.z.unwrap_or(p.depth);
                let o = crate::camera_view::scene_to_output(&chain, [q.x, q.y, z]).ok_or_else(|| behind(i))?;
                let v = crate::camera_view::apply(&view_from_output, o);
                out.push(Pt { x: v.x, y: v.y, z: None });
            }
            From::View => {
                // The camera is rigid and looks along −Z (05 line 236): the plane Z = depth is at distance
                // (camera Z − depth) in front of it.
                let distance = chain.view.position[2] - p.depth;
                if distance <= tt_anim::camera::NEAR_PLANE {
                    return Err(behind(i));
                }
                let o = crate::camera_view::apply(&output_from_view, Point { x: q.x, y: q.y });
                let s = crate::camera_view::output_to_scene(&chain, o, distance).ok_or_else(|| behind(i))?;
                out.push(Pt { x: s[0], y: s[1], z: Some(s[2]) });
            }
        }
    }
    Ok(CoordsResult { points: out })
}
