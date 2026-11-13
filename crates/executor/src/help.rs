//! Help texts (01 §9.3; M4 C4.1): every command, panel, preference and message has a help content, available to the
//! GUI's context help and tooltips and to agents (`ui.help.get`). Texts are built from the registry, the panel
//! catalogue, the preference catalogue and the message catalogue below, so they cannot drift from what they describe.

use serde_json::{Value, json};
use tt_registry::{CommandError, Status};

use crate::keymap::Binding;

/// The main window's title without a project (01 §5.1).
pub(crate) const APP_TITLE: &str = "Toontown";

/// Messages of 01 §12.1 rule 7 and the generic codes it names, with what each conveys (01 §9.5: the message id is the
/// agent error code). Suppressible tips are listed in `commands::ui_shell::TIPS` and included.
pub(crate) const MESSAGES: &[(&str, &str)] = &[
    ("arg.missing", "A required parameter was not given."),
    ("fs.invalidPath", "A file path is relative or malformed; paths must be absolute."),
    ("fs.outsideRoots", "A path lies outside the folders the agent session may use."),
    ("fs.exists", "The file exists; pass onExists to overwrite it."),
    ("ui.busy", "The user is in a dialog or a gesture; the agent's change waited and gave up."),
    ("command.disabled", "The command is not available in the current state."),
    ("app.outOfMemory", "The operation ran out of memory and was rolled back."),
    ("project.none", "No project is open."),
    ("project.notFound", "The project file does not exist."),
    ("project.invalidFormat", "The file is not a project package or is damaged."),
    ("project.unsupportedVersion", "The project was written by a newer version of the application."),
    ("project.permissionDenied", "The location cannot be read or written."),
    ("project.readOnly", "The project was opened read-only; use Save As."),
    ("project.exists", "A project already exists at that location."),
    ("project.pathRequired", "The project has no location yet; choose one with Save As."),
    ("project.pathInsideProject", "A project cannot be saved inside its own package."),
    ("project.unsavedChanges", "The project has unsaved changes."),
    ("project.formatUpgrade", "Saving upgrades the package; older versions will not open it."),
    ("project.saveFailed", "The project could not be saved."),
    ("library.saveFailed", "The global library could not be saved."),
    ("name.empty", "A name cannot be empty."),
    ("name.tooLong", "A name can have at most 100 characters."),
    ("name.invalidCharacter", "A name cannot contain / \\ : * ? \" < > | or control characters."),
    ("edit.nothingToUndo", "There is nothing to undo."),
    ("edit.nothingToRedo", "There is nothing to redo."),
    ("edit.clipboardEmpty", "The clipboard is empty."),
    ("edit.clipboardInvalid", "The clipboard content does not match its kind."),
    ("edit.pasteNotAllowed", "The clipboard content cannot be pasted here."),
    ("edit.wrongElementType", "The drawing cannot be pasted into an element of that type."),
    ("edit.busy", "Paste is not possible while bone deformation is being computed."),
    ("edit.noPreviousPasteSpecial", "Paste Special Again needs an earlier Paste Special."),
    ("view.notFound", "There is no such view window."),
    ("window.panelNotFound", "There is no such panel."),
    ("window.toolbarItemUnknown", "The toolbar item is not a command or a known group."),
    ("workspace.notFound", "There is no workspace with that name."),
    ("workspace.exists", "A workspace with that name exists."),
    ("workspace.nameInvalid", "The workspace name is empty or invalid."),
    ("workspace.builtIn", "Built-in workspaces cannot be deleted."),
    ("workspace.last", "The last workspace cannot be deleted."),
    ("prefs.unknownKey", "There is no preference with that key."),
    ("prefs.invalidValue", "The value has the wrong type or is not one of the allowed values."),
    ("prefs.outOfRange", "The value is outside the preference's range."),
    ("agent.sessionNotFound", "There is no agent session with that id; it may have disconnected."),
    ("anim.valueOutOfRange", "An animation value is outside its allowed range."),
    ("arg.invalid", "A parameter has the wrong type or value; details.pointer names it."),
    ("arg.unknown", "A parameter is not one the command accepts; details.pointer names it."),
    ("asset.inUse", "The asset is still used by the project and cannot be removed."),
    ("asset.relinkFailed", "The linked media could not be relinked to the file given."),
    ("batch.jobNotAllowed", "A batch cannot start a job; run the job on its own."),
    ("batch.nestedGroup", "A batch step cannot open a group inside another group."),
    ("batch.stepFailed", "A step of the batch failed; details.stepIndex names it and nothing was changed."),
    ("cell.clipboardEmpty", "There are no cells on the clipboard."),
    ("cell.typeMismatch", "The cells do not fit the column type they are pasted into."),
    ("checkpoint.exists", "A checkpoint with that name exists."),
    ("checkpoint.notFound", "There is no checkpoint with that name."),
    ("command.notImplemented", "The command is registered but arrives with a later milestone; details name it."),
    ("command.unknown", "There is no command with that id."),
    ("confirmation.declined", "The user declined the confirmation; nothing was changed."),
    ("confirmation.expired", "The confirmation token expired; ask again."),
    ("confirmation.required", "The action needs the user's confirmation (pass confirm, or answer the request)."),
    ("doc.blobTooLarge", "The embedded data is larger than the project format allows."),
    ("doc.diffUnavailable", "The changes since that revision are no longer kept; read the objects again."),
    ("doc.exportTooLarge", "The exported content is larger than the limit."),
    ("doc.importConflict", "The imported content conflicts with existing objects."),
    ("doc.limitExceeded", "A document limit would be exceeded."),
    ("doc.nameRepaired", "A name was repaired to a valid, unique one (a warning)."),
    ("doc.patchTestFailed", "A test operation of the patch did not match; nothing was changed."),
    ("doc.readOnlyProperty", "The property cannot be changed."),
    ("doc.schemaViolation", "The change would make the object invalid."),
    ("doc.structuralChange", "The property can only be changed with its dedicated command."),
    ("doc.unknownValue", "The value is not one the property accepts."),
    ("doc.valueClamped", "A value was clamped to its allowed range (a warning)."),
    ("drawing.contentMissing", "The drawing's content is missing from the project."),
    ("drawing.locked", "The drawing is locked."),
    ("element.locked", "The element is locked."),
    ("exposure.invalid", "The exposure is not valid for the cells given."),
    ("fs.ioError", "The file could not be read or written; details give the reason."),
    ("fs.notFound", "The file or folder does not exist."),
    ("history.diverged", "The history changed since it was read; read it again."),
    ("id.duplicate", "An id is used twice."),
    ("inspect.behindCamera", "The point is behind the camera and has no view position."),
    ("inspect.singularTransform", "The transform cannot be inverted."),
    ("inspect.spaceUnavailable", "That coordinate space is not available here."),
    ("integrity.hashMismatch", "A file's content does not match its recorded hash."),
    ("internal", "An unexpected failure; report its message and correlation id. It changed nothing unless said."),
    ("job.cancelled", "The job was cancelled."),
    ("job.failed", "The job failed; details give the reason."),
    ("job.notFound", "There is no job with that id."),
    ("job.waitTimeout", "The job did not finish within the wait; it keeps running."),
    ("keymap.invalidFile", "The file is not a keymap file or is damaged."),
    ("limit.exceeded", "A session limit was exceeded; details give the limit and the value."),
    ("permission.denied", "The agent session lacks a scope the command needs."),
    ("project.backupNotFound", "There is no such backup."),
    ("project.changedOnDisk", "The project file changed on disk since it was opened."),
    ("project.lockedByOther", "Another session has the project open; it opens read-only."),
    ("project.newerMinorVersion", "The project was written by a newer version; unknown content is kept (a warning)."),
    ("project.recoveryInUse", "The recovery data belongs to a running session."),
    ("project.recoveryNotFound", "There is no such recovery data."),
    ("project.recoveryOlderThanFile", "The recovery copy is older than the saved project file."),
    ("project.saveInterrupted", "A save was interrupted; the previous file is intact."),
    ("ref.ambiguous", "The reference matches several objects; details list candidates."),
    ("ref.dangling", "The reference points to an object that no longer exists."),
    ("ref.notFound", "The reference matches no object."),
    ("ref.wrongType", "The reference names an object of another type."),
    ("render.failed", "The render failed; details give the reason."),
    ("render.inlineTooLarge", "The image is too large to return inline; write it to a file."),
    ("request.conflict", "A request with that id is already running or answered differently."),
    ("request.unknownId", "There is no request with that id."),
    ("rev.conflict", "The object changed since the revision given (expectRev); read it again."),
    ("storyline.transitionInvalid", "The transition does not fit between those scenes."),
    ("tree.invalid", "The change would make the hierarchy invalid."),
    ("ui.modalOpen", "A user dialog is open; the agent's focus change was refused."),
    ("undo.notTop", "Only the most recent step can be undone this way."),
    ("workspace.invalidFile", "The file is not a workspace file or is damaged."),
    ("prefs.corrupt", "A settings file was damaged; it was kept aside and its defaults are used."),
    (
        "prefs.saveFailed",
        "A settings file could not be saved (SI-119, interim id): the change applies until the application quits. The message names the file and the reason.",
    ),
    ("agent.serverFailed", "The agent server (attached mode) did not start; the message says why."),
    ("keymap.unknownCommand", "There is no command with that id to bind."),
    ("keymap.invalidSequence", "The key sequence is not valid."),
    ("keymap.reservedKey", "The key sequence is reserved by the system or the application."),
    ("keymap.conflict", "The key sequence is already bound to another command."),
    ("ui.unknownMessage", "There is no message with that id."),
    ("command.timeout", "The command did not answer in time and was cancelled unless it had already committed."),
    ("project.mediaMissing", "Some media referenced by the project are missing; placeholders are shown."),
];

fn not_found(code: &str, what: &str, id: &str, pointer: &str) -> CommandError {
    CommandError::with_details(code, format!("no {what} `{id}`"), json!({ pointer: id }))
}

/// Help for a command: label, summary, kind, availability, shortcuts and owner.
///
/// # Errors
/// `command.unknown`.
pub(crate) fn command(id: &str, bindings: &[Binding]) -> Result<(String, String), CommandError> {
    let registry = crate::registry().map_err(|e| CommandError::internal(e.0))?;
    let e =
        registry.get(id).ok_or_else(|| not_found(tt_registry::codes::COMMAND_UNKNOWN, "command", id, "commandId"))?;
    let d = e.describe();
    let label = d["label"].as_str().unwrap_or(id).to_owned();
    let mut text = format!("{}\n\n", d["summary"].as_str().unwrap_or_default());
    text.push_str(&format!("- Command: `{id}` ({})\n", d["kind"].as_str().unwrap_or_default()));
    if d["undoable"] == Value::Bool(true) {
        text.push_str(&format!("- Undoable: \"{}\"\n", d["undoLabel"].as_str().unwrap_or(&label)));
    }
    if d["needsProject"] == Value::Bool(true) {
        text.push_str("- Needs an open project.\n");
    }
    // Platform-neutral notation (01 §12.1 rule 17): agents read this text on any OS; the GUI shows native notation.
    let keys: Vec<String> = bindings
        .iter()
        .filter(|b| b.command_id == id)
        .map(|b| format!("{} ({})", b.keys, b.context.as_str()))
        .collect();
    text.push_str(&format!("- Shortcut: {}\n", if keys.is_empty() { "none".to_owned() } else { keys.join(", ") }));
    if let Status::NotImplemented { owner } = e.status() {
        text.push_str(&format!("- Not available yet: arrives with {owner}.\n"));
    }
    Ok((label, text))
}

/// The owner of a panel whose content arrives later (shown in the placeholder and the help).
#[must_use]
pub fn panel_owner(id: &str) -> Option<&'static str> {
    Some(match id {
        "cameraView" | "drawingView" => "M4 C4.2 (canvas), M4 C4.3 (Camera view tools)",
        "topView" | "sideView" | "functionEditor" | "motionRecording" => "M6",
        "timeline" => "M5 C5.1",
        "xsheet" | "cells" => "M5 C5.2",
        "properties" | "pen" | "text" => "M4 C4.2",
        "colour" => "M7 C7.1",
        "library" | "storyline" | "sceneManager" => "M9",
        "effectPreview" => "M7",
        "lipSync" | "soundEditor" => "M8",
        "capture" => "M10 C10.3",
        "drawingTools" => "M4 C4.2",
        "animationTools" => "M6 C6.1",
        "boneTools" => "M6 C6.3",
        "effectTools" => "M7",
        "soundTools" => "M8",
        _ => return None,
    })
}

/// Help for a panel or tool palette.
///
/// # Errors
/// `window.panelNotFound`.
pub(crate) fn panel(id: &str) -> Result<(String, String), CommandError> {
    let title = crate::ui::PANELS
        .iter()
        .find(|(p, _)| *p == id)
        .map(|(_, t)| (*t).to_owned())
        .or_else(|| crate::ui::TOOL_PALETTES.contains(&id).then(|| id.to_owned()))
        .ok_or_else(|| not_found("window.panelNotFound", "panel", id, "panelId"))?;
    let mut text = format!("Panel `{id}` (01 §5.2). Show or hide it from the Window menu (`window.panel.set`).\n");
    if let Some(owner) = panel_owner(id) {
        text.push_str(&format!("\nIts content arrives with {owner}.\n"));
    }
    Ok((title, text))
}

/// Help for a preference.
///
/// # Errors
/// `prefs.unknownKey`.
pub(crate) fn preference(key: &str, current: Option<&Value>) -> Result<(String, String), CommandError> {
    let d = crate::prefs::catalogue().get(key).ok_or_else(|| crate::prefs::unknown(key))?;
    let category = crate::prefs::catalogue()
        .categories
        .iter()
        .find(|c| c.id == d.category)
        .map_or(d.category.as_str(), |c| c.label.as_str());
    let mut text = format!(
        "{}\n\n- Category: {category}\n- Key: `{key}` ({})\n- Default: `{}`\n",
        d.description,
        d.ty.as_str(),
        d.default
    );
    if let Some(v) = current {
        text.push_str(&format!("- Current: `{v}`\n"));
    }
    if let (Some(a), Some(b)) = (d.min, d.max) {
        text.push_str(&format!("- Range: {a} to {b}\n"));
    }
    if let Some(values) = &d.values {
        text.push_str(&format!("- Values: {}\n", values.join(", ")));
    }
    text.push_str(match d.applies {
        crate::prefs::Applies::Live => "- Takes effect at once.\n",
        crate::prefs::Applies::NextLaunch => "- Takes effect at the next launch.\n",
        crate::prefs::Applies::Restart => "- Takes effect after a restart.\n",
    });
    Ok((d.key.clone(), text))
}

/// Help for a message (`None`: unknown id).
#[must_use]
pub(crate) fn message(id: &str) -> Option<(String, String)> {
    MESSAGES
        .iter()
        .chain(crate::commands::ui_shell::TIPS.iter())
        .find(|(m, _)| *m == id)
        .map(|(m, d)| ((*m).to_owned(), format!("{d}\n\nMessage id: `{m}` (report this id with any problem).\n")))
}
