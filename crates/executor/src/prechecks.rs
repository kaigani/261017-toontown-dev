//! Prechecks (ruling O-23; reviews/M4-design-availability.md): the refusal a command gives with no parameter missing,
//! as a cheap pure function of the session and the raw parameters. The pipeline runs a command's precheck before its
//! handler and availability runs the same function, so the two cannot disagree. This module holds the prechecks that
//! need no command-private helper: parameter-shape rules (a reference that must name one of several parameters) and
//! simple state rules; the others live beside their handlers and reuse the handlers' own target resolution.

use serde_json::{Value, json};
use tt_registry::{CommandError, codes};

use crate::cx::Cx;

fn has(v: &Value, k: &str) -> bool {
    v.get(k).is_some_and(|x| !x.is_null())
}

fn is_true(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool) == Some(true)
}

fn count(v: &Value, keys: &[&str]) -> usize {
    keys.iter().filter(|k| has(v, k)).count()
}

fn missing(message: &str, pointer: &str) -> CommandError {
    CommandError::with_details(codes::ARG_MISSING, message, json!({ "pointer": pointer }))
}

fn invalid(message: &str, pointer: &str) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, message, json!({ "pointer": pointer }))
}

/// Exactly one of `keys` (`arg.invalid` otherwise).
fn exactly_one(v: &Value, keys: &[&str]) -> Result<(), CommandError> {
    if count(v, keys) == 1 {
        Ok(())
    } else {
        Err(invalid(&format!("give exactly one of {}", keys.join(", ")), &format!("/{}", keys[0])))
    }
}

/// `keymap.get`: `commandId` or `keys`.
pub(crate) fn keymap_get(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if count(v, &["commandId", "keys"]) == 0 {
        Err(missing("give `commandId` or `keys`", "/commandId"))
    } else {
        Ok(())
    }
}

/// `keymap.reset`: `commandId` or `all: true`.
pub(crate) fn keymap_reset(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if !has(v, "commandId") && !is_true(v, "all") {
        Err(missing("give `commandId` or `all: true`", "/commandId"))
    } else {
        Ok(())
    }
}

/// `prefs.reset`: `keys` or `prefix`.
pub(crate) fn prefs_reset(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if count(v, &["keys", "prefix"]) == 0 { Err(missing("give `keys` or `prefix`", "/keys")) } else { Ok(()) }
}

/// `ui.help.get`: exactly one target.
pub(crate) fn ui_help_get(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let keys = ["commandId", "panelId", "preferenceKey", "messageId"];
    match count(v, &keys) {
        1 => Ok(()),
        0 => Err(missing("give exactly one of commandId, panelId, preferenceKey, messageId", "/commandId")),
        _ => Err(invalid("give exactly one of commandId, panelId, preferenceKey, messageId", "/commandId")),
    }
}

/// `draw.stroke.add`: `points` or `bezier`.
pub(crate) fn draw_stroke_add(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if count(v, &["points", "bezier"]) == 0 { Err(missing("give points or bezier", "/points")) } else { Ok(()) }
}

/// `cell.label.set`: exactly one of `drawing`, `drawingId`, `step`.
pub(crate) fn cell_label_set(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    exactly_one(v, &["drawing", "drawingId", "step"])
}

/// `colour.recolour.apply`: `fromColourId` and `toColourId`, or `map`; the scope `scene` without `sceneId` needs a
/// current scene.
pub(crate) fn colour_recolour_apply(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if !((has(v, "fromColourId") && has(v, "toColourId")) || has(v, "map")) {
        return Err(invalid("give fromColourId and toColourId, or map", "/fromColourId"));
    }
    if v.get("scope").and_then(Value::as_str) == Some("scene") && !has(v, "sceneId") && cx.app().scene.is_none() {
        return Err(CommandError::with_details(
            "scene.noTarget",
            "there is no current scene",
            json!({ "pointer": "/sceneId" }),
        ));
    }
    Ok(())
}

/// `xsheet.exposures.set`: without columns or `sceneId`, the current scene.
pub(crate) fn xsheet_exposures_set(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if v.get("columns").and_then(Value::as_array).is_none_or(Vec::is_empty) && !has(v, "sceneId") {
        crate::commands::timing::scene_id(cx, None)?;
    }
    Ok(())
}

/// `render.range.run`: the scene (given, else the current or first one).
pub(crate) fn render_range_run(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    crate::commands::render::scene_param(cx, v.get("sceneId").and_then(Value::as_str)).map(|_| ())
}

/// `render.frame.run`: the scene (given, else the current or first one), the frame (given, else the current one) and,
/// without `cameraId`, the scene's current camera, for the camera composition (the refusals of `scene_param`,
/// `frame_param` and the render list).
pub(crate) fn render_frame_run(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let composition = v.get("composition").and_then(Value::as_str).unwrap_or("camera");
    if matches!(composition, "xsheet" | "activeView") {
        return Ok(());
    }
    let scene = crate::commands::render::scene_param(cx, v.get("sceneId").and_then(Value::as_str))?;
    let frame = v.get("frame").and_then(Value::as_u64).and_then(|f| u32::try_from(f).ok());
    crate::commands::render::frame_param(cx, frame)?;
    let drawing_space = composition == "element" && has(v, "drawingId");
    if !drawing_space && !has(v, "cameraId") {
        let doc = cx.document()?;
        if let Some(sc) = doc.scene(&scene)
            && sc.current_camera_id().is_none()
        {
            return Err(CommandError::with_details(
                "render.noCamera",
                format!("scene `{}` has no camera to render through", sc.name()),
                json!({ "sceneId": scene.as_str() }),
            ));
        }
    }
    Ok(())
}

/// `doc.asset.embed`: `assetIds` or `all: true`.
pub(crate) fn doc_asset_embed(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if has(v, "assetIds") || is_true(v, "all") {
        Ok(())
    } else {
        Err(invalid("give `assetIds` or `all: true`", "/assetIds"))
    }
}

/// `doc.asset.relink`: `assetId` with `path`, or `searchFolder`.
pub(crate) fn doc_asset_relink(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if (has(v, "assetId") && has(v, "path")) || has(v, "searchFolder") {
        Ok(())
    } else {
        Err(invalid("give `assetId` with `path`, or `searchFolder`", "/assetId"))
    }
}

/// `doc.asset.remove`: `assetIds` or `unused: true`.
pub(crate) fn doc_asset_remove(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if has(v, "assetIds") || is_true(v, "unused") {
        Ok(())
    } else {
        Err(invalid("give `assetIds` or `unused: true`", "/assetIds"))
    }
}

/// `draw.cut.apply`: exactly one of `polygon`, `rect`, `segment`.
pub(crate) fn draw_cut_apply(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    exactly_one(v, &["polygon", "rect", "segment"])
}

/// `draw.erase.apply`: exactly one of `path`, `region`.
pub(crate) fn draw_erase_apply(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    exactly_one(v, &["path", "region"])
}

/// `draw.vectorize.preview`: exactly one of `file`, `imageElementId`.
pub(crate) fn draw_vectorize_preview(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    exactly_one(v, &["file", "imageElementId"])
}

/// `draw.vectorize.run`: exactly one of `files`, `imageElementId`.
pub(crate) fn draw_vectorize_run(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    exactly_one(v, &["files", "imageElementId"])
}

/// `element.parent.set`: exactly one of `parentId` and `nearest: true`.
pub(crate) fn element_parent_set(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if usize::from(has(v, "parentId")) + usize::from(is_true(v, "nearest")) == 1 {
        Ok(())
    } else {
        Err(invalid("give exactly one of `parentId` and `nearest: true`", "/parentId"))
    }
}

/// `project.recent.remove`: `path` or `all: true`.
pub(crate) fn project_recent_remove(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if has(v, "path") || is_true(v, "all") { Ok(()) } else { Err(invalid("give `path` or `all: true`", "/path")) }
}

/// `project.recovery.delete`: `recoveryId` or `all: true`.
pub(crate) fn project_recovery_delete(_cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if has(v, "recoveryId") || is_true(v, "all") {
        Ok(())
    } else {
        Err(invalid("give `recoveryId` or `all: true`", "/recoveryId"))
    }
}

/// A command whose handler needs an open project without declaring `needs_project` (`project.backup.list`).
pub(crate) fn project_open(cx: &Cx, _v: &Value) -> Result<(), CommandError> {
    if cx.session().project.is_some() { Ok(()) } else { Err(crate::cx::no_project()) }
}

/// `window.panel.set`: the central view cannot be hidden or floated (`active` names the focused panel).
pub(crate) fn window_panel_set(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let Some(given) = v.get("panelId").and_then(Value::as_str) else { return Ok(()) };
    let ui = cx.ui();
    let panel = if given == "active" { ui.focused().unwrap_or_default().to_owned() } else { given.to_owned() };
    let central = ui.central_panel() == panel;
    let hide = v.get("visible").and_then(Value::as_bool) == Some(false);
    let float = v.get("floating").and_then(Value::as_bool) == Some(true);
    if central && (hide || float) {
        return Err(invalid(
            "the central view cannot be hidden or floated; move another view to the centre first",
            "/panelId",
        ));
    }
    Ok(())
}
