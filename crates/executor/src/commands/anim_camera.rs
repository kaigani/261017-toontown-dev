//! Cameras (05 §2.6, §2.7, §12, §19.3; M3 C3.2): `anim.camera.create`, `.delete`, `.list`, `.get`, `.set`,
//! `.current.get`, `.current.set`, and the events `anim.changed`, `anim.camera.currentChanged`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::hierarchy as h;
use tt_anim::num::tidy;
use tt_model::Id;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::{ElementType, FovMode};
use tt_registry::{CommandError, command};

use super::anim_common::{
    ChannelEdit, P3, Xyz, check_value, data_of, emit_changed, emit_static, eval_error, evaluator, key_frame, node,
    node_id, not_found, out_of_range, scene_frame,
};
use super::anim_function::put_key;
use super::common::NoParams;
use super::timing::{Budget, Changes, err, op, scene_id};
use crate::cx::Cx;

/// A camera reference (`anim.notFound` when it is not a camera).
fn camera_id(cx: &Cx, doc: &Document, r: &str) -> Result<Id, CommandError> {
    let id = node_id(cx, r).map_err(|_| not_found("camera", r))?;
    if *node(doc, &id)?.element_type() != ElementType::Camera {
        return Err(not_found("camera", r));
    }
    Ok(id)
}

/// The camera a `cameraId?` parameter names, else the current scene's current camera.
fn camera_or_current(cx: &Cx, doc: &Document, given: Option<&str>) -> Result<Id, CommandError> {
    if let Some(r) = given {
        return camera_id(cx, doc, r);
    }
    let scene = scene_id(cx, None)?;
    doc.scene(&scene)
        .and_then(|s| s.current_camera_id().clone())
        .ok_or_else(|| not_found("current camera of the scene", scene.as_str()))
}

/// The cameras of a scene in Timeline order.
pub(crate) fn cameras_of(doc: &Document, scene: &Id) -> Vec<Id> {
    let Some(sc) = doc.scene(scene) else { return Vec::new() };
    h::placed(sc.tree())
        .into_iter()
        .filter(|q| doc.element(&q.id).is_some_and(|e| *e.element_type() == ElementType::Camera))
        .map(|q| q.id)
        .collect()
}

/// `anim.lastCamera` when deleting `gone` would leave a scene that has cameras without one (05 §12.1).
pub(crate) fn refuse_last_camera(doc: &Document, scene: &Id, gone: &dyn Fn(&Id) -> bool) -> Result<(), CommandError> {
    let cameras = cameras_of(doc, scene);
    if !cameras.is_empty() && cameras.iter().all(gone) {
        return Err(err(
            "anim.lastCamera",
            "a scene always keeps at least one camera",
            json!({ "sceneId": scene.as_str() }),
        ));
    }
    Ok(())
}

/// The camera that becomes current when the current one goes: the first remaining in Timeline order (05 §12.1).
pub(crate) fn next_current(doc: &Document, scene: &Id, gone: &dyn Fn(&Id) -> bool) -> Option<Id> {
    cameras_of(doc, scene).into_iter().find(|c| !gone(c))
}

// ----- anim.camera.create ------------------------------------------------------------------------------------------

/// `anim.camera.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreateParams {
    /// Name; default: unique `Camera…`.
    #[serde(default)]
    name: Option<String>,
    /// Parent; default: the top level.
    #[serde(default)]
    parent_id: Option<String>,
    /// Static position; default (0, 0, 16).
    #[serde(default)]
    position: Option<Xyz>,
    /// Horizontal field of view, 1…179; default 2·atan(0.375) ≈ 41.1121 (05 §2.7).
    #[serde(default)]
    fov: Option<f64>,
    /// Make it the scene's current camera.
    #[serde(default)]
    make_current: bool,
}

/// `{cameraId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraResult {
    /// The camera.
    camera_id: String,
}

fn emit_current(cx: &mut Cx, scene: &Id, camera: &Id) {
    cx.emit("anim.camera.currentChanged", json!({ "sceneId": scene.as_str(), "cameraId": camera.as_str() }));
}

#[command(
    id = "anim.camera.create",
    kind = action,
    label = "Add camera",
    scopes = [edit],
    undo = "Add Camera",
    needs_project,
    example(title = "A wide camera, made current", params = r#"{"name": "Wide", "fov": 60, "position": {"x": 0, "y": 0, "z": 20}, "makeCurrent": true}"#),
    gui = "Scene views, Element menu: Add Camera (M6 C6.2)",
)]
pub(crate) fn anim_camera_create(cx: &mut Cx, p: CreateParams) -> Result<CameraResult, CommandError> {
    if let Some(f) = p.fov {
        check_value("fov", f)?;
    }
    if let Some(q) = &p.position {
        q.check()?;
    }
    let mut params = json!({ "type": "camera" });
    if let Some(n) = &p.name {
        params["name"] = json!(n);
    }
    if let Some(r) = &p.parent_id {
        params["parentId"] = json!(r);
    }
    let parsed = serde_json::from_value(params).map_err(super::timing::internal)?;
    let made = super::element::element_create(cx, parsed)?;
    let created = made.element_ids.first().cloned().ok_or_else(|| super::timing::internal("no camera created"))?;
    let id = Id::parse(&created)?;
    let doc = cx.document()?.clone();
    let scene = doc.scene_of(&id).cloned().ok_or_else(|| super::timing::internal("scene"))?;
    let mut ops: Vec<tt_model::change::PatchOp> = Vec::new();
    if let Some(q) = &p.position {
        for (axis, v) in ["x", "y", "z"].iter().zip(q.parts()) {
            if let Some(v) = v {
                ops.push(op("replace", &format!("/transform/offset/{axis}"), Some(json!(v))));
            }
        }
    }
    if let Some(f) = p.fov {
        ops.push(op("replace", "/camera/fov", Some(json!(f))));
    }
    let mut changes = Changes::default();
    if !ops.is_empty() {
        changes.push(EntryKey::Element(id.clone()), ops);
    }
    let had_current = doc.scene(&scene).is_some_and(|s| s.current_camera_id().is_some());
    if p.make_current || !had_current {
        changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/currentCameraId", Some(json!(id.as_str())))]);
    }
    changes.stage(cx)?;
    if p.make_current || !had_current {
        emit_current(cx, &scene, &id);
    }
    Ok(CameraResult { camera_id: id.as_str().to_owned() })
}

// ----- anim.camera.delete ------------------------------------------------------------------------------------------

/// A `cameraId` parameter.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CameraParam {
    /// The camera.
    camera_id: String,
}

/// `{currentCameraId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// The scene's current camera after the deletion.
    current_camera_id: Option<String>,
}

#[command(
    id = "anim.camera.delete",
    kind = action,
    label = "Delete camera",
    scopes = [edit],
    undo = "Delete Camera",
    needs_project,
    example(title = "Delete the Wide camera", params = r#"{"cameraId": "scene:\"Intro\"/element:\"Wide\""}"#),
    agent_only = "agents delete cameras by id (05 §12.2); the Timeline deletes them as elements (M5)",
)]
pub(crate) fn anim_camera_delete(cx: &mut Cx, p: CameraParam) -> Result<DeleteResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = camera_id(cx, &doc, &p.camera_id)?;
    let scene = doc.scene_of(&id).cloned().ok_or_else(|| super::timing::internal("scene"))?;
    let params = serde_json::from_value(json!({ "elementIds": [id.as_str()] })).map_err(super::timing::internal)?;
    // `element.delete` refuses the last camera and replaces a deleted current camera (05 §12.1).
    super::element::element_delete(cx, params)?;
    let after = cx.document()?.scene(&scene).and_then(|s| s.current_camera_id().clone());
    Ok(DeleteResult { current_camera_id: after.map(|c| c.as_str().to_owned()) })
}

// ----- anim.camera.list --------------------------------------------------------------------------------------------

/// A `sceneId?` parameter.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SceneParam {
    /// The scene; default: the current scene.
    #[serde(default)]
    scene_id: Option<String>,
}

/// One camera of a scene.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ListedCamera {
    id: String,
    name: String,
    current: bool,
}

/// `anim.camera.list` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct CameraListResult {
    cameras: Vec<ListedCamera>,
}

#[command(
    id = "anim.camera.list",
    kind = query,
    label = "List cameras",
    scopes = [read],
    needs_project,
    example(title = "Cameras of the current scene", params = "{}"),
    gui = "Scene views: camera list (M6 C6.2)",
)]
pub(crate) fn anim_camera_list(cx: &mut Cx, p: SceneParam) -> Result<CameraListResult, CommandError> {
    let doc = cx.document()?.clone();
    let scene = scene_id(cx, p.scene_id.as_deref())?;
    let current = doc.scene(&scene).and_then(|s| s.current_camera_id().clone());
    let cameras = cameras_of(&doc, &scene)
        .into_iter()
        .filter_map(|c| {
            doc.element(&c).map(|e| ListedCamera {
                id: c.as_str().to_owned(),
                name: e.name().clone(),
                current: current.as_ref() == Some(&c),
            })
        })
        .collect();
    Ok(CameraListResult { cameras })
}

// ----- anim.camera.get ---------------------------------------------------------------------------------------------

/// `anim.camera.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GetParams {
    /// The camera; default: the current scene's current camera.
    #[serde(default)]
    camera_id: Option<String>,
    /// Scene frame; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
}

/// Output size.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Resolution {
    w: u32,
    h: u32,
}

/// `anim.camera.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraGetResult {
    id: String,
    /// Static position.
    position: P3,
    /// Static roll.
    roll: f64,
    fov_mode: FovMode,
    /// Static field of view.
    fov: f64,
    /// Field of view at the frame.
    effective_fov: f64,
    /// World position at the frame.
    world_position: P3,
    /// 16 numbers row-major.
    view_matrix: Vec<f64>,
    /// The frame's corners on Z = 0, top-left first, clockwise; `null` when the plane is not in front of the camera.
    frame_corners_at_z0: Option<Vec<P3>>,
    resolution: Resolution,
}

#[command(
    id = "anim.camera.get",
    kind = query,
    label = "Get camera",
    scopes = [read],
    needs_project,
    example(title = "The current camera at frame 1", params = r#"{"frame": 1}"#),
    agent_only = "agents read the evaluated camera (05 §19.1.5); the Properties panel shows it (M6 C6.2)",
)]
pub(crate) fn anim_camera_get(cx: &mut Cx, p: GetParams) -> Result<CameraGetResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = camera_or_current(cx, &doc, p.camera_id.as_deref())?;
    let frame = scene_frame(cx, p.frame.map(|f| f as f64))?;
    let e = node(&doc, &id)?;
    let block = e.camera().as_ref().ok_or_else(|| not_found("camera", id.as_str()))?;
    let mut ev = evaluator(&doc, "reading a camera");
    let view = ev.camera(&id, frame).map_err(eval_error)?;
    let t = e.transform().as_ref().ok_or_else(|| not_found("camera", id.as_str()))?;
    Ok(CameraGetResult {
        id: id.as_str().to_owned(),
        position: [t.offset.x, t.offset.y, t.offset.z].into(),
        roll: tidy(t.rotation),
        fov_mode: block.fov_mode,
        fov: tidy(block.fov),
        effective_fov: tidy(view.fov),
        world_position: view.position.into(),
        view_matrix: view.view.0.iter().map(|v| tidy(*v)).collect(),
        frame_corners_at_z0: view.frame_corners_at_z0().map(|c| c.iter().map(|q| P3::from(*q)).collect()),
        resolution: Resolution { w: *doc.project().camera_width(), h: *doc.project().camera_height() },
    })
}

// ----- anim.camera.set ---------------------------------------------------------------------------------------------

/// `anim.camera.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetParams {
    /// The camera.
    camera_id: String,
    /// Static position; omitted axes keep their value.
    #[serde(default)]
    position: Option<Xyz>,
    /// Static roll, degrees.
    #[serde(default)]
    roll: Option<f64>,
    /// `static` or `dynamic`.
    #[serde(default)]
    fov_mode: Option<FovMode>,
    /// Field of view, 1…179: static, or keyed at `frame` when dynamic.
    #[serde(default)]
    fov: Option<f64>,
    /// Scene frame of the `fov` key; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
}

#[command(
    id = "anim.camera.set",
    kind = action,
    label = "Set camera",
    scopes = [edit],
    undo = "Change Camera",
    needs_project,
    example(title = "Widen and roll the Intro camera", params = r#"{"cameraId": "scene:\"Intro\"/element:\"Camera\"", "fov": 60, "roll": 5}"#),
    agent_only = "agents set camera values (05 §19.1.1 FOV wedge equivalent); the Properties panel does the same (M6 C6.2)",
)]
pub(crate) fn anim_camera_set(cx: &mut Cx, p: SetParams) -> Result<CameraResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = camera_id(cx, &doc, &p.camera_id)?;
    let e = node(&doc, &id)?;
    let block = e.camera().as_ref().ok_or_else(|| not_found("camera", id.as_str()))?;
    if let Some(q) = &p.position {
        q.check()?;
    }
    if let Some(r) = p.roll
        && r.abs() > super::anim_common::MAX_VALUE
    {
        return Err(out_of_range("roll", r, -super::anim_common::MAX_VALUE, super::anim_common::MAX_VALUE));
    }
    if let Some(f) = p.fov {
        check_value("fov", f)?;
    }
    let mode = p.fov_mode.unwrap_or(block.fov_mode);
    let mut ops: Vec<tt_model::change::PatchOp> = Vec::new();
    if let Some(q) = &p.position {
        for (axis, v) in ["x", "y", "z"].iter().zip(q.parts()) {
            if let Some(v) = v {
                ops.push(op("replace", &format!("/transform/offset/{axis}"), Some(json!(v))));
            }
        }
    }
    if let Some(r) = p.roll {
        ops.push(op("replace", "/transform/rotation", Some(json!(r))));
    }
    if p.fov_mode.is_some() {
        ops.push(op("replace", "/camera/fovMode", Some(json!(mode))));
    }
    let mut budget = Budget::new("setting a camera");
    let data = data_of(&doc, &id, &mut budget)?;
    let mut edit = ChannelEdit::read(&doc, &id)?;
    let frame = scene_frame(cx, p.frame.map(|f| f as f64))?;
    if mode == FovMode::Dynamic {
        let empty = edit.keys("fov").is_empty();
        let value = match (p.fov, empty) {
            (Some(v), _) => Some(v),
            // Switching to dynamic initialises the channel with the static value (05 §12.3).
            (None, true) => Some(block.fov),
            (None, false) => None,
        };
        if let Some(v) = value {
            let local = key_frame(&data, frame)?;
            put_key(cx, &mut edit, e, "fov", local, v)?;
        }
    } else if let Some(v) = p.fov {
        ops.push(op("replace", "/camera/fov", Some(json!(v))));
    }
    let mut changes = Changes::default();
    if !ops.is_empty() {
        changes.push(EntryKey::Element(id.clone()), ops);
    }
    edit.stage(&mut changes);
    changes.stage(cx)?;
    emit_changed(cx, &data, &edit);
    emit_static(cx, &id, &["camera"]);
    Ok(CameraResult { camera_id: id.as_str().to_owned() })
}

// ----- anim.camera.current.get / .set ------------------------------------------------------------------------------

/// `anim.camera.current.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentResult {
    /// The current camera (`null` in a scene without cameras).
    camera_id: Option<String>,
}

#[command(
    id = "anim.camera.current.get",
    kind = query,
    label = "Current camera",
    scopes = [read],
    needs_project,
    example(title = "The current camera of the current scene", params = "{}"),
    gui = "Scene views: camera list (M6 C6.2)",
)]
pub(crate) fn anim_camera_current_get(cx: &mut Cx, p: SceneParam) -> Result<CurrentResult, CommandError> {
    let doc = cx.document()?.clone();
    let scene = scene_id(cx, p.scene_id.as_deref())?;
    let camera = doc.scene(&scene).and_then(|s| s.current_camera_id().clone());
    Ok(CurrentResult { camera_id: camera.map(|c| c.as_str().to_owned()) })
}

#[command(
    id = "anim.camera.current.set",
    kind = action,
    label = "Set current camera",
    scopes = [edit],
    undo = "Set Current Camera",
    needs_project,
    example(title = "Make the Intro camera current", params = r#"{"cameraId": "scene:\"Intro\"/element:\"Camera\""}"#),
    gui = "Scene views: camera list (M6 C6.2)",
)]
pub(crate) fn anim_camera_current_set(cx: &mut Cx, p: CameraParam) -> Result<CameraResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = camera_id(cx, &doc, &p.camera_id)?;
    let scene = doc.scene_of(&id).cloned().ok_or_else(|| super::timing::internal("scene"))?;
    let current = doc.scene(&scene).and_then(|s| s.current_camera_id().clone());
    if current.as_ref() != Some(&id) {
        let mut changes = Changes::default();
        changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/currentCameraId", Some(json!(id.as_str())))]);
        changes.stage(cx)?;
        emit_current(cx, &scene, &id);
    }
    Ok(CameraResult { camera_id: id.as_str().to_owned() })
}

// ----- events ------------------------------------------------------------------------------------------------------

/// `anim.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimChanged {
    node_ids: Vec<String>,
    channels: Vec<String>,
    /// Scene frames touched.
    #[serde(skip_serializing_if = "Option::is_none")]
    frames: Option<FrameSpan>,
}

/// A frame span.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameSpan {
    from: f64,
    to: f64,
}

#[command(
    id = "anim.changed",
    kind = event,
    label = "Animation changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn anim_changed(_cx: &mut Cx, _p: NoParams) -> Result<AnimChanged, CommandError> {
    Ok(AnimChanged { node_ids: Vec::new(), channels: Vec::new(), frames: None })
}

/// `anim.camera.currentChanged` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentChanged {
    scene_id: String,
    camera_id: String,
}

#[command(
    id = "anim.camera.currentChanged",
    kind = event,
    label = "Current camera changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn anim_camera_current_changed(_cx: &mut Cx, _p: NoParams) -> Result<CurrentChanged, CommandError> {
    Ok(CurrentChanged { scene_id: String::new(), camera_id: String::new() })
}

/// Merges the `anim.changed` events of one accepted call into the first (05 §19.4: once per undo step).
pub(crate) fn coalesce_changed(effects: &mut Vec<crate::cx::Effect>) {
    let mut merged: Option<(usize, Value)> = None;
    let mut remove = Vec::new();
    for (i, e) in effects.iter().enumerate() {
        let crate::cx::Effect::Event(ev) = e else { continue };
        if ev.name != "anim.changed" {
            continue;
        }
        match &mut merged {
            None => merged = Some((i, ev.payload.clone())),
            Some((_, m)) => {
                for list in ["nodeIds", "channels"] {
                    for v in ev.payload[list].as_array().cloned().unwrap_or_default() {
                        if let Some(a) = m[list].as_array_mut()
                            && !a.contains(&v)
                        {
                            a.push(v);
                        }
                    }
                }
                // An event without `frames` covers every frame, and absorbs any range (C32-R1-05); two ranges merge
                // into the one spanning both.
                match (m.get("frames").cloned(), ev.payload.get("frames")) {
                    (Some(o), Some(f)) => {
                        let lo = o["from"].as_f64().unwrap_or(0.0).min(f["from"].as_f64().unwrap_or(0.0));
                        let hi = o["to"].as_f64().unwrap_or(0.0).max(f["to"].as_f64().unwrap_or(0.0));
                        m["frames"] = json!({ "from": lo, "to": hi });
                    }
                    (Some(_), None) => {
                        if let Some(o) = m.as_object_mut() {
                            o.remove("frames");
                        }
                    }
                    (None, _) => {}
                }
                remove.push(i);
            }
        }
    }
    if let Some((i, payload)) = merged
        && let Some(crate::cx::Effect::Event(ev)) = effects.get_mut(i)
    {
        ev.payload = payload;
    }
    for i in remove.into_iter().rev() {
        effects.remove(i);
    }
}
