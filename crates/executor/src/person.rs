//! The interactive-target rule for a PERSON's contextual edits whatever the input (ruling D-83 item 3; finding
//! C44B-R2S-03): a key, a Tool options or Properties control, a menu or context-menu item. The gesture path applies the
//! same rule to pointer gestures (`gesture::to_commands`); this module is the window's and the key dispatch's entry
//! point to it. Not a gesture driver: the commands it names are the ones it refuses or leaves alone.

use serde_json::{Value, json};
use tt_registry::CommandError;

use crate::cx::Cx;

/// The interactive-target rule for a PERSON's contextual command (ruling D-83 item 3; findings C44B-R1-02, -03,
/// C44B-R2S-03), whatever the input: a key (the host's key resolution), a Tool options or Properties control, a menu or
/// context-menu item (the window's actions, [`crate::InvokeOptions::person`]). A command is contextual when it edits
/// or picks the current drawing: a drawing command (`draw.*`) that is an undoable edit, other than the drawing lock
/// itself, or an `edit.*` edit in the drawing context; or a pick (`draw.selection.set`, `draw.pointSelection.set`); and
/// that names no other drawing. The view is the active one, the tool the active one (Reposition All keeps SI-189's
/// precheck). Agents' direct calls never pass here (05 §6.6: "Commands addressing it by id still act on it").
///
/// # Errors
/// `drawing.locked`, `element.locked`, `element.hidden`, naming the refused command (`refusedCommandId`).
pub(crate) fn person_rule(cx: &Cx, command: &str, params: &Value) -> Result<(), CommandError> {
    let edit = if crate::gesture::picks(command) {
        false
    } else if crate::gesture::edits(command)
        && ((command.starts_with("draw.") && command != "draw.lock.set")
            || (command.starts_with("edit.") && drawing_context(cx, params)))
    {
        true
    } else {
        return Ok(());
    };
    if !names_current(cx, params) {
        return Ok(());
    }
    let view = cx.ui().shell().active_view.clone().unwrap_or_default();
    let tool = cx.ui().drawing().active_tool.clone();
    crate::gesture::interactive_target(cx, &view, &tool, edit).map_err(|mut e| {
        if let Some(m) = e.details.as_object_mut() {
            m.insert("refusedCommandId".into(), json!(command));
        }
        e
    })
}

/// Whether an `edit.*` command acts in the drawing context: named, or `auto` (or absent) resolving to it (the Drawing
/// view focused, or the Camera view with the drawing tools; ruling D-87).
fn drawing_context(cx: &Cx, params: &Value) -> bool {
    match params.get("context").and_then(Value::as_str) {
        Some("drawing") => true,
        None | Some("auto") => {
            crate::commands::edit::resolve_edit_context(cx, crate::commands::common::EditContext::Auto)
                == crate::commands::common::EditContext::Drawing
        }
        Some(_) => false,
    }
}

/// Whether a command's parameters address the current drawing: no drawing named (the command acts on the current
/// one), or the current one named (`drawingId`, or a qualified `strokeId` / `textId` `"<drawingId>/<local id>"`).
fn names_current(cx: &Cx, params: &Value) -> bool {
    let Ok(doc) = cx.document() else { return false };
    let Some(current) = cx.app().current_drawing(doc) else { return false };
    let current = current.as_str();
    if let Some(d) = params.get("drawingId").and_then(Value::as_str)
        && d != current
    {
        return false;
    }
    ["strokeId", "textId"].iter().all(|k| {
        params
            .get(*k)
            .and_then(Value::as_str)
            .and_then(|s| s.split_once('/'))
            .is_none_or(|(drawing, _)| drawing == current)
    })
}
