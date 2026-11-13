//! `keymap.*` (01 §8, §12.2; M4 C4.1): bindings over the bundled default set and the user's overrides
//! ([`crate::keymap`]).
//!
//! Every edit computes the complete new keymap, checks it (registered command, parameters valid for it, canonical
//! and unreserved sequence, no conflict unless `onConflict: "override"`), and stages the new overrides as a settings
//! effect accepted with the call. Later steps of the same call see the staged keymap. Keymap edits are not undoable
//! and do not modify the project (01 §6.1).

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, Kind, codes, command};

use crate::cx::{Cx, Effect, FileWrite, SettingsChange};
use crate::keymap::{self, Binding, Context, Platform, codes as kcodes};

fn platform() -> Platform {
    Platform::current()
}

/// The keymap as this call sees it.
pub(crate) fn current(cx: &Cx) -> Vec<Binding> {
    keymap::effective(&cx.keymap_overrides(), platform())
}

fn stage(cx: &mut Cx, before: &[Binding], after: Vec<Binding>) {
    let ids = keymap::changed_commands(before, &after);
    if ids.is_empty() {
        return;
    }
    let overrides = keymap::overrides_for(&after, platform());
    cx.defer(Effect::Settings(SettingsChange::Keymap(overrides)));
    cx.emit("keymap.changed", json!({ "commandIds": ids }));
}

/// Checks that a command can be bound: registered, not an event, the fixed parameters valid for it.
pub(crate) fn check_reference(command_id: &str, params: Option<&Map<String, Value>>) -> Result<(), CommandError> {
    let registry = crate::registry().map_err(|e| CommandError::internal(e.0))?;
    let entry = registry.get(command_id).filter(|e| e.spec.kind != Kind::Event).ok_or_else(|| {
        CommandError::with_details(
            kcodes::UNKNOWN_COMMAND,
            format!("`{command_id}` is not a command that can be bound"),
            json!({ "commandId": command_id }),
        )
        .hint("list_commands returns the registered commands (events cannot be bound)")
    })?;
    if let Some(p) = params {
        // Fixed parameters are a partial parameter set: required parameters may be supplied at dispatch (a toggle's
        // state) or collected by a dialog, so only the given members are checked against the schema.
        let mut schema = entry.params_schema();
        if let Some(m) = schema.as_object_mut() {
            m.shift_remove("required");
        }
        tt_registry::validate_params(command_id, &schema, &Value::Object(p.clone())).map_err(|e| {
            CommandError::with_details(
                codes::ARG_INVALID,
                format!("the fixed parameters do not fit `{command_id}`: {}", e.message),
                json!({ "pointer": "/params", "error": e }),
            )
        })?;
    }
    Ok(())
}

fn canonical(keys: &str) -> Result<String, CommandError> {
    let seq = keymap::parse(keys, platform())?;
    if let Some(why) = keymap::reserved(&seq, platform()) {
        return Err(CommandError::with_details(
            kcodes::RESERVED_KEY,
            format!("`{keys}` is reserved ({why}) and cannot be bound"),
            json!({ "keys": seq.canonical(), "reason": why }),
        ));
    }
    Ok(seq.canonical())
}

fn conflict_error(candidate: &Binding, with: &[&Binding]) -> CommandError {
    let first = with.first();
    CommandError::with_details(
        kcodes::CONFLICT,
        format!(
            "`{}` is already bound to `{}` in {}",
            candidate.keys,
            first.map_or("", |b| b.command_id.as_str()),
            first.map_or("", |b| b.context.as_str())
        ),
        json!({ "keys": candidate.keys, "context": candidate.context, "conflicts": with }),
    )
    .hint("pass onConflict: \"override\" to move the binding")
}

/// `keymap.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// Only this context.
    #[serde(default)]
    context: Option<Context>,
    /// Only this command.
    #[serde(default)]
    #[schemars(length(max = 256))]
    command_id: Option<String>,
    /// Only commands whose id starts with this prefix.
    #[serde(default)]
    #[schemars(length(max = 256))]
    prefix: Option<String>,
    /// Only bindings the user added.
    #[serde(default)]
    only_modified: bool,
}

#[command(
    id = "keymap.list",
    kind = query,
    label = "List key bindings",
    scopes = [read],
    example(title = "Bindings of the Save command", params = r#"{"commandId": "project.save"}"#),
    gui = "Preferences › Keyboard (shortcut editor; M4 C4.1)",
)]
pub(crate) fn keymap_list(cx: &mut Cx, p: ListParams) -> Result<Vec<Binding>, CommandError> {
    Ok(current(cx)
        .into_iter()
        .filter(|b| p.context.is_none_or(|c| b.context == c))
        .filter(|b| p.command_id.as_ref().is_none_or(|c| &b.command_id == c))
        .filter(|b| p.prefix.as_ref().is_none_or(|x| b.command_id.starts_with(x.as_str())))
        .filter(|b| !p.only_modified || !b.is_default)
        .collect())
}

/// `keymap.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GetParams {
    /// The command whose bindings to read.
    #[serde(default)]
    #[schemars(length(max = 256))]
    command_id: Option<String>,
    /// The key sequence whose command to read.
    #[serde(default)]
    #[schemars(length(max = 64))]
    keys: Option<String>,
    /// Context of `keys` (a mode context falls back to `global`).
    #[serde(default = "global")]
    context: Context,
}

fn global() -> Context {
    Context::Global
}

/// A command reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandRef {
    /// Command id.
    command_id: String,
    /// Fixed parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Map<String, Value>>,
}

/// `keymap.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GetResult {
    /// Bindings of the command, or the binding of the sequence.
    bindings: Vec<Binding>,
    /// The command the sequence runs in the context.
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_command: Option<CommandRef>,
}

#[command(
    id = "keymap.get",
    kind = query,
    label = "Get key bindings",
    scopes = [read],
    example(title = "What Primary+S runs", params = r#"{"keys": "Primary+S"}"#),
    example(title = "Bindings of Save", params = r#"{"commandId": "project.save"}"#),
    gui = "Preferences › Keyboard: details of the selected command (M4 C4.1)",
)]
pub(crate) fn keymap_get(cx: &mut Cx, p: GetParams) -> Result<GetResult, CommandError> {
    let all = current(cx);
    match (&p.command_id, &p.keys) {
        (None, None) => Err(CommandError::with_details(
            codes::ARG_MISSING,
            "give `commandId` or `keys`",
            json!({ "pointer": "/commandId" }),
        )),
        (Some(c), None) => {
            check_reference(c, None)?;
            Ok(GetResult {
                bindings: all.into_iter().filter(|b| &b.command_id == c).collect(),
                effective_command: None,
            })
        }
        (c, Some(k)) => {
            let keys = keymap::parse(k, platform())?.canonical();
            let hit = keymap::resolve(&all, &keys, Some(p.context)).cloned();
            let bindings = all
                .iter()
                .filter(|b| b.keys == keys && c.as_ref().is_none_or(|c| &b.command_id == c))
                .cloned()
                .collect();
            Ok(GetResult {
                bindings,
                effective_command: hit.map(|b| CommandRef { command_id: b.command_id, params: b.params }),
            })
        }
    }
}

/// Conflict policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnConflict {
    /// Refuse with `keymap.conflict`.
    #[default]
    Fail,
    /// Move the sequence to this command.
    Override,
}

/// `keymap.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetParams {
    /// Command to bind (an empty or unknown id is `keymap.unknownCommand`, not a schema error: C41-R1-13).
    #[schemars(length(max = 256))]
    command_id: String,
    /// Fixed parameters of the command reference.
    #[serde(default)]
    params: Option<Map<String, Value>>,
    /// Key sequence (01 §12.1 rule 17); an empty one is `keymap.invalidSequence` (C41-R1-13).
    #[schemars(length(max = 64))]
    keys: String,
    /// Context.
    #[serde(default = "global")]
    context: Context,
    /// What to do when the sequence is taken.
    #[serde(default)]
    on_conflict: OnConflict,
    /// Remove the command reference's other bindings in this context first.
    #[serde(default)]
    replace_existing: bool,
}

/// `keymap.set` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SetResult {
    /// The new binding.
    binding: Binding,
    /// Bindings removed (moved away from another command, or replaced).
    removed: Vec<Binding>,
}

#[command(
    id = "keymap.set",
    kind = ui,
    label = "Set a key binding",
    scopes = [prefs],
    example(title = "Bind F2 to Save", params = r#"{"commandId": "project.save", "keys": "F2"}"#),
    example(title = "Zoom in with a fixed parameter", params = r#"{"commandId": "view.zoom.set", "params": {"steps": 1}, "keys": "Primary+Shift+=", "onConflict": "override"}"#),
    gui = "Preferences › Keyboard: shortcut capture (M4 C4.1)",
)]
pub(crate) fn keymap_set(cx: &mut Cx, p: SetParams) -> Result<SetResult, CommandError> {
    check_reference(&p.command_id, p.params.as_ref())?;
    let keys = canonical(&p.keys)?;
    let before = current(cx);
    let candidate = Binding {
        command_id: p.command_id.clone(),
        params: p.params.clone().filter(|m| !m.is_empty()),
        keys,
        context: p.context,
        is_default: false,
    };
    let clashes: Vec<Binding> = keymap::conflicts(&before, &candidate).into_iter().cloned().collect();
    if !clashes.is_empty() && p.on_conflict == OnConflict::Fail {
        return Err(conflict_error(&candidate, &clashes.iter().collect::<Vec<_>>()));
    }
    let mut removed = clashes;
    if p.replace_existing {
        removed.extend(
            before
                .iter()
                .filter(|b| {
                    b.command_id == candidate.command_id
                        && b.params == candidate.params
                        && b.context == candidate.context
                        && b.keys != candidate.keys
                })
                .cloned(),
        );
    }
    let mut after: Vec<Binding> = before.iter().filter(|b| !removed.contains(b)).cloned().collect();
    let existing = after.iter().find(|b| {
        b.command_id == candidate.command_id
            && b.params == candidate.params
            && b.keys == candidate.keys
            && b.context == candidate.context
    });
    let binding = match existing {
        Some(b) => b.clone(),
        None => {
            after.push(candidate.clone());
            candidate
        }
    };
    stage(cx, &before, after);
    Ok(SetResult { binding, removed })
}

/// `keymap.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RemoveParams {
    /// Command whose bindings to remove (empty or unknown: `keymap.unknownCommand`).
    #[schemars(length(max = 256))]
    command_id: String,
    /// Only this sequence.
    #[serde(default)]
    #[schemars(length(max = 64))]
    keys: Option<String>,
    /// Only this context.
    #[serde(default)]
    context: Option<Context>,
}

/// `keymap.remove` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RemoveResult {
    /// Bindings removed.
    removed: Vec<Binding>,
}

#[command(
    id = "keymap.remove",
    kind = ui,
    label = "Remove key bindings",
    scopes = [prefs],
    example(title = "Remove Primary+P from Print", params = r#"{"commandId": "print.run", "keys": "Primary+P"}"#),
    gui = "Preferences › Keyboard: Remove (M4 C4.1)",
)]
pub(crate) fn keymap_remove(cx: &mut Cx, p: RemoveParams) -> Result<RemoveResult, CommandError> {
    check_reference(&p.command_id, None)?;
    let keys = match &p.keys {
        Some(k) => Some(keymap::parse(k, platform())?.canonical()),
        None => None,
    };
    let before = current(cx);
    let (removed, after): (Vec<Binding>, Vec<Binding>) = before.iter().cloned().partition(|b| {
        b.command_id == p.command_id
            && keys.as_ref().is_none_or(|k| &b.keys == k)
            && p.context.is_none_or(|c| b.context == c)
    });
    stage(cx, &before, after);
    Ok(RemoveResult { removed })
}

/// `keymap.reset` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ResetParams {
    /// The command to reset to its default bindings.
    #[serde(default)]
    #[schemars(length(max = 256))]
    command_id: Option<String>,
    /// Restore the whole default keymap.
    #[serde(default)]
    all: bool,
}

/// `keymap.reset` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ResetResult {
    /// Commands whose bindings changed.
    reset: usize,
}

#[command(
    id = "keymap.reset",
    kind = ui,
    label = "Restore default key bindings",
    scopes = [prefs],
    example(title = "Restore Save's defaults", params = r#"{"commandId": "project.save"}"#),
    example(title = "Restore the whole default keymap", params = r#"{"all": true}"#),
    gui = "Preferences › Keyboard: Reset / Reset all (M4 C4.1)",
)]
pub(crate) fn keymap_reset(cx: &mut Cx, p: ResetParams) -> Result<ResetResult, CommandError> {
    let before = current(cx);
    let defaults = keymap::defaults(platform());
    let after: Vec<Binding> = match (&p.command_id, p.all) {
        (_, true) => defaults,
        (Some(c), false) => {
            check_reference(c, None)?;
            // The command's defaults return; a user binding of another command on one of those sequences gives way
            // (reset means reset; keymap.changed names both commands).
            let mine: Vec<Binding> = defaults.into_iter().filter(|d| &d.command_id == c).collect();
            let mut after: Vec<Binding> = before
                .iter()
                .filter(|b| &b.command_id != c)
                .filter(|b| !mine.iter().any(|d| d.keys == b.keys && keymap::contexts_conflict(d.context, b.context)))
                .cloned()
                .collect();
            after.extend(mine);
            after
        }
        (None, false) => {
            return Err(CommandError::with_details(
                codes::ARG_MISSING,
                "give `commandId` or `all: true`",
                json!({ "pointer": "/commandId" }),
            ));
        }
    };
    let reset = keymap::changed_commands(&before, &after).len();
    stage(cx, &before, after);
    Ok(ResetResult { reset })
}

/// Overwrite policy of `keymap.export` (01 §12.1 rule 4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnExists {
    /// Refuse with `fs.exists`.
    #[default]
    Fail,
    /// Replace the file.
    Overwrite,
}

/// `keymap.export` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExportParams {
    /// Absolute file path.
    #[schemars(length(max = 4096))]
    path: String,
    /// When the file exists.
    #[serde(default)]
    on_exists: OnExists,
}

/// `{path}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PathResult {
    /// The file written.
    path: String,
}

/// The keymap exchange file (01 §8.3 item 8).
pub(crate) fn exchange_file(bindings: &[Binding]) -> Value {
    json!({ "format": "toontown-keymap", "formatVersion": "1.0", "baseSet": keymap::default_set_name(), "bindings": bindings })
}

#[command(
    id = "keymap.export",
    kind = ui,
    label = "Export key bindings",
    scopes = [prefs, files_write],
    confirm = ["overwrite"],
    example(title = "Write the keymap to a file", params = r#"{"path": "/tmp/keymap.json", "onExists": "overwrite"}"#),
    gui = "Preferences › Keyboard: Export… (native dialog; M4 C4.1)",
)]
pub(crate) fn keymap_export(cx: &mut Cx, p: ExportParams) -> Result<PathResult, CommandError> {
    let path = crate::persistence::path_param(cx, &p.path, "/path")?;
    if path.exists() {
        if p.on_exists == OnExists::Fail {
            return Err(CommandError::with_details(
                "fs.exists",
                format!("`{}` exists", p.path),
                json!({ "path": p.path }),
            )
            .hint("pass onExists: \"overwrite\""));
        }
        crate::persistence::confirm_overwrite(cx, &path)?;
    }
    let text = tt_model::canonical::to_canonical_string(&exchange_file(&current(cx)))
        .map_err(|e| CommandError::internal(e.to_string()))?;
    cx.defer(Effect::WriteFile(Box::new(FileWrite::new(path.clone(), text.into_bytes()))));
    Ok(PathResult { path: path.to_string_lossy().into_owned() })
}

/// Import mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ImportMode {
    /// Add the file's bindings to the current keymap.
    #[default]
    Merge,
    /// The file's bindings become the whole keymap.
    Replace,
}

/// `keymap.import` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ImportParams {
    /// Absolute file path.
    #[schemars(length(max = 4096))]
    path: String,
    /// Merge or replace.
    #[serde(default)]
    mode: ImportMode,
    /// Conflicts with the current keymap (merge): refuse the whole import, or let the file win.
    #[serde(default)]
    on_conflict: OnConflict,
}

/// `keymap.import` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ImportResult {
    /// Bindings imported.
    imported: usize,
    /// Bindings the file displaced (`onConflict: "override"`).
    conflicts: Vec<Binding>,
}

/// Reads and checks a keymap exchange file: every binding canonical, unreserved, bindable, and the file itself free of
/// conflicts.
fn read_exchange(path: &std::path::Path, shown: &str) -> Result<Vec<Binding>, CommandError> {
    let bad = |why: String| {
        CommandError::with_details(
            "keymap.invalidFile",
            format!("`{shown}` is not a keymap file: {why}"),
            json!({ "path": shown, "reason": why }),
        )
    };
    let bytes = tt_persist::fsx::read_limited(path, 4 << 20).map_err(|_| {
        if path.exists() {
            bad("unreadable or larger than 4 MiB".into())
        } else {
            CommandError::with_details("fs.notFound", format!("`{shown}` does not exist"), json!({ "path": shown }))
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|_| bad("not UTF-8".into()))?;
    let v: Value = tt_model::json_text::parse(&text).map_err(|e| bad(e.to_string()))?;
    if v.get("format").and_then(Value::as_str) != Some("toontown-keymap") {
        return Err(bad("format is not toontown-keymap".into()));
    }
    let list = v.get("bindings").and_then(Value::as_array).ok_or_else(|| bad("no bindings".into()))?;
    let mut out: Vec<Binding> = Vec::new();
    // A binding's own domain error keeps its code, with the binding's index (C41-R1-12); only the file's structure is
    // `keymap.invalidFile`.
    let at = |i: usize, mut e: CommandError| {
        if let Some(m) = e.details.as_object_mut() {
            m.insert("index".into(), json!(i));
            m.insert("path".into(), json!(shown));
        }
        e.message = format!("binding {i} of `{shown}`: {}", e.message);
        e
    };
    for (i, b) in list.iter().enumerate() {
        let mut b = b.clone();
        if let Some(m) = b.as_object_mut() {
            m.insert("isDefault".into(), json!(false));
        }
        let mut binding: Binding = serde_json::from_value(b).map_err(|e| bad(format!("binding {i}: {e}")))?;
        binding.keys = canonical(&binding.keys).map_err(|e| at(i, e))?;
        check_reference(&binding.command_id, binding.params.as_ref()).map_err(|e| at(i, e))?;
        if let Some(c) = keymap::conflicts(&out, &binding).first() {
            return Err(at(
                i,
                CommandError::with_details(
                    kcodes::CONFLICT,
                    format!("it conflicts with {} on {}", c.command_id, c.keys),
                    json!({ "conflicts": [{ "commandId": c.command_id, "keys": c.keys }] }),
                ),
            ));
        }
        if !out.iter().any(|o| o == &binding) {
            out.push(binding);
        }
    }
    Ok(out)
}

#[command(
    id = "keymap.import",
    kind = ui,
    label = "Import key bindings",
    scopes = [prefs, files_read],
    example(title = "Merge a keymap file", params = r#"{"path": "/tmp/keymap.json", "mode": "merge", "onConflict": "override"}"#),
    gui = "Preferences › Keyboard: Import… (native dialog; M4 C4.1)",
)]
pub(crate) fn keymap_import(cx: &mut Cx, p: ImportParams) -> Result<ImportResult, CommandError> {
    let path: PathBuf = crate::persistence::path_param(cx, &p.path, "/path")?;
    let incoming = read_exchange(&path, &p.path)?;
    let before = current(cx);
    let imported = incoming.len();
    let (after, conflicts) = match p.mode {
        ImportMode::Replace => (incoming, Vec::new()),
        ImportMode::Merge => {
            let mut displaced: Vec<Binding> = Vec::new();
            for b in &incoming {
                for c in keymap::conflicts(&before, b) {
                    if !displaced.contains(c) {
                        displaced.push(c.clone());
                    }
                }
            }
            if !displaced.is_empty() && p.on_conflict == OnConflict::Fail {
                return Err(CommandError::with_details(
                    kcodes::CONFLICT,
                    format!("{} binding(s) of the file conflict with the current keymap", displaced.len()),
                    json!({ "conflicts": displaced }),
                )
                .hint("pass onConflict: \"override\" to let the file win, or mode: \"replace\""));
            }
            let mut after: Vec<Binding> = before.iter().filter(|b| !displaced.contains(b)).cloned().collect();
            for b in incoming {
                if !after.iter().any(|a| {
                    a.command_id == b.command_id && a.params == b.params && a.keys == b.keys && a.context == b.context
                }) {
                    after.push(b);
                }
            }
            (after, displaced)
        }
    };
    stage(cx, &before, after);
    Ok(ImportResult { imported, conflicts })
}

/// `keymap.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedPayload {
    /// Commands whose bindings changed.
    command_ids: Vec<String>,
}

#[command(
    id = "keymap.changed",
    kind = event,
    label = "Key bindings changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn keymap_changed(_cx: &mut Cx, _p: super::common::NoParams) -> Result<ChangedPayload, CommandError> {
    Err(super::common::event_not_invocable("keymap.changed"))
}
