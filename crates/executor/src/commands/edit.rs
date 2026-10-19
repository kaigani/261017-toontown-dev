//! `edit.*` (01 §6, §12.1): undo/redo and history (kernel), selection and clipboard state (kernel), and the
//! context-dependent clipboard edits whose semantics belong to the areas (declared here, implemented with their
//! areas and wired to the GUI in M4 C4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_registry::{CommandError, codes, command};

use super::common::{EditContext, NoParams, event_not_invocable, invalid};
use crate::cx::Cx;
use crate::rules;

fn one() -> u32 {
    1
}

/// `edit.undo` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UndoParams {
    /// Steps to undo (each is its own transaction).
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 1000))]
    steps: u32,
    /// Undo only if this step id is still the most recent one (`undo.notTop` otherwise; 11 §7.7).
    #[serde(default)]
    if_top: Option<String>,
}

/// `edit.undo` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct UndoResult {
    /// Labels of the undone steps, most recent first.
    undone: Vec<String>,
    /// History position after the call.
    position: usize,
}

/// Replays `steps` steps of the history as consecutive transactions.
fn replay(cx: &mut Cx, steps: u32, undo: bool) -> Result<(Vec<String>, usize), CommandError> {
    let mut labels = Vec::new();
    let project = cx.project()?;
    let mut position = project.history.position();
    let len = project.history.len();
    for _ in 0..steps {
        let index = if undo {
            match position.checked_sub(1) {
                Some(i) => i,
                None => break,
            }
        } else if position < len {
            position
        } else {
            break;
        };
        let step =
            cx.project()?.history.steps().get(index).cloned().ok_or_else(|| CommandError::internal("history step"))?;
        cx.replay(index, undo, &step.edit, &step.touched, step.structural, &step.label)?;
        labels.push(step.label);
        position = if undo { index } else { index + 1 };
    }
    Ok((labels, position))
}

#[command(
    id = "edit.undo",
    kind = action,
    label = "Undo",
    scopes = [edit],
    needs_project,
    enablement = rules::CAN_UNDO,
    example(title = "Undo the last step", params = r#"{"steps": 1}"#),
)]
pub(crate) fn edit_undo(cx: &mut Cx, p: UndoParams) -> Result<UndoResult, CommandError> {
    if let Some(want) = &p.if_top {
        let top = cx.project()?.history.top().map(|s| (s.id.clone(), s.label.clone(), s.origin.label()));
        if top.as_ref().map(|t| &t.0) != Some(want) {
            return Err(CommandError::with_details(
                codes::UNDO_NOT_TOP,
                format!("step `{want}` is not the most recent step"),
                json!({ "ifTop": want, "top": top.map(|(id, label, origin)| json!({ "stepId": id, "label": label, "origin": origin })) }),
            ));
        }
    }
    let (undone, position) = replay(cx, p.steps, true)?;
    Ok(UndoResult { undone, position })
}

/// `edit.redo` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RedoParams {
    /// Steps to redo.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 1000))]
    steps: u32,
}

/// `edit.redo` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RedoResult {
    /// Labels of the redone steps, in order.
    redone: Vec<String>,
    /// History position after the call.
    position: usize,
}

#[command(
    id = "edit.redo",
    kind = action,
    label = "Redo",
    scopes = [edit],
    needs_project,
    enablement = rules::CAN_REDO,
    example(title = "Redo one step", params = r#"{"steps": 1}"#),
)]
pub(crate) fn edit_redo(cx: &mut Cx, p: RedoParams) -> Result<RedoResult, CommandError> {
    let (redone, position) = replay(cx, p.steps, false)?;
    Ok(RedoResult { redone, position })
}

fn hundred() -> u32 {
    100
}

/// `edit.history.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryGetParams {
    /// Entries to skip (from the oldest).
    #[serde(default)]
    #[schemars(range(min = 0, max = 100_000))]
    offset: u32,
    /// Entries to return.
    #[serde(default = "hundred")]
    #[schemars(range(min = 1, max = 1000))]
    limit: u32,
}

/// One history entry (01 §12.1).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryEntry {
    /// 1-based position of the step (position `n` = steps 1…n applied).
    index: usize,
    /// Step id (`u-<n>`), as in envelopes' `undo.stepId`.
    step_id: String,
    /// Label.
    label: String,
    /// `user` or `agent`.
    source: String,
    /// Agent session id.
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_session_id: Option<String>,
    /// Agent display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_name: Option<String>,
    /// Commit time (ISO 8601 UTC).
    time: String,
    /// Changed objects.
    object_count: usize,
}

/// `edit.history.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryGetResult {
    /// Entries, oldest first.
    entries: Vec<HistoryEntry>,
    /// Steps applied.
    position: usize,
    /// Position of the last save (`null` if unreachable).
    clean_position: Option<usize>,
    /// Total entries.
    total: usize,
}

#[command(
    id = "edit.history.get",
    kind = query,
    label = "History",
    scopes = [read],
    needs_project,
    example(title = "The last 20 entries", params = r#"{"limit": 20}"#),
    gui = "History panel (M4)",
)]
pub(crate) fn edit_history_get(cx: &mut Cx, p: HistoryGetParams) -> Result<HistoryGetResult, CommandError> {
    let h = &cx.project()?.history;
    let entries = h
        .steps()
        .iter()
        .enumerate()
        .skip(p.offset as usize)
        .take(p.limit as usize)
        .map(|(i, s)| {
            let (sid, name) = match &s.origin {
                crate::types::Origin::Agent { session, name, .. } => (Some(session.clone()), Some(name.clone())),
                _ => (None, None),
            };
            HistoryEntry {
                index: i + 1,
                step_id: s.id.clone(),
                label: s.label.clone(),
                source: s.origin.source().to_owned(),
                agent_session_id: sid,
                agent_name: name,
                time: s.time.clone(),
                object_count: s.touched.len(),
            }
        })
        .collect();
    Ok(HistoryGetResult { entries, position: h.position(), clean_position: h.clean_position(), total: h.len() })
}

/// `edit.history.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryMoveParams {
    /// Target position (0 = before the first step, entry count = after the last).
    #[schemars(range(min = 0, max = 1000))]
    position: u32,
}

/// `{position}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PositionResult {
    /// History position after the call.
    position: usize,
}

#[command(
    id = "edit.history.move",
    kind = action,
    label = "Go to history state",
    scopes = [edit],
    needs_project,
    enablement = rules::OUTSIDE_ATOMIC_BATCH,
    example(title = "Back to the opened state", params = r#"{"position": 0}"#),
    gui = "History panel: click an entry (M4)",
)]
pub(crate) fn edit_history_move(cx: &mut Cx, p: HistoryMoveParams) -> Result<PositionResult, CommandError> {
    let h = &cx.project()?.history;
    let (cur, len) = (h.position(), h.len());
    let target = p.position as usize;
    if target > len {
        return Err(CommandError::with_details(
            codes::ARG_INVALID,
            format!("position {target} is beyond the {len} history entries"),
            json!({ "pointer": "/position", "allowed": { "minimum": 0, "maximum": len } }),
        ));
    }
    let position = if target < cur {
        replay(cx, u32::try_from(cur - target).unwrap_or(u32::MAX), true)?.1
    } else if target > cur {
        replay(cx, u32::try_from(target - cur).unwrap_or(u32::MAX), false)?.1
    } else {
        cur
    };
    Ok(PositionResult { position })
}

/// `edit.history.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryChanged {
    /// Position.
    position: usize,
    /// Entries.
    entry_count: usize,
    /// Label of the top step.
    last_label: Option<String>,
    /// `user` or `agent`.
    source: Option<String>,
}

#[command(
    id = "edit.history.changed",
    kind = event,
    label = "History changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn edit_history_changed(_cx: &mut Cx, _p: NoParams) -> Result<HistoryChanged, CommandError> {
    Err(event_not_invocable("edit.history.changed"))
}

// ----- selection --------------------------------------------------------------------------------------------------

/// `{context}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextParams {
    /// Selection context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
}

/// `edit.selection.get` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SelectionGetResult {
    /// The resolved context.
    context: String,
    /// Selected ids.
    ids: Vec<String>,
    /// Object types present.
    kinds: Vec<String>,
}

fn resolve_context(cx: &Cx, c: EditContext) -> EditContext {
    let central = cx.ui().central_panel().to_owned();
    c.resolve(cx.ui().focused(), &central)
}

/// The selection of `context` (the kernel keeps one selection; another context's selection is empty).
fn selection_in(cx: &Cx, context: EditContext) -> Vec<String> {
    let sel = &cx.app().selection;
    if sel.context == context.as_str() { sel.ids.clone() } else { Vec::new() }
}

#[command(
    id = "edit.selection.get",
    kind = query,
    label = "Get selection",
    scopes = [read],
    needs_project,
    example(title = "Selection of the focused context", params = "{}"),
    agent_only = "the GUI shows the selection directly",
)]
pub(crate) fn edit_selection_get(cx: &mut Cx, p: ContextParams) -> Result<SelectionGetResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let ids = selection_in(cx, context);
    let doc = cx.document()?;
    let mut kinds: Vec<String> = Vec::new();
    for k in &ids {
        if let Some(l) = doc.locate(k)
            && !kinds.iter().any(|x| x == l.ty.as_str())
        {
            kinds.push(l.ty.as_str().to_owned());
        }
    }
    Ok(SelectionGetResult { context: context.as_str().to_owned(), ids, kinds })
}

/// Selection change mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SelectMode {
    /// Replace the selection.
    #[default]
    Replace,
    /// Add to it.
    Add,
    /// Remove from it.
    Remove,
    /// Toggle membership.
    Toggle,
}

/// `edit.selection.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectionSetParams {
    /// Selection context.
    #[serde(default)]
    context: EditContext,
    /// Objects (ids or paths).
    #[schemars(length(max = 100_000))]
    ids: Vec<String>,
    /// How to combine with the current selection.
    #[serde(default)]
    mode: SelectMode,
}

/// `{context, ids}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SelectionResult {
    /// The resolved context.
    context: String,
    /// The selection.
    ids: Vec<String>,
}

/// Object types a selection context holds (01 §6.2).
fn context_types(c: EditContext) -> &'static [ObjectType] {
    match c {
        EditContext::Drawing => &[ObjectType::DrawingNode, ObjectType::Stroke],
        EditContext::Staging | EditContext::Timeline | EditContext::Xsheet => &[ObjectType::Element],
        EditContext::Library => &[ObjectType::LibraryFolder, ObjectType::LibraryItem],
        EditContext::FunctionEditor => &[ObjectType::Key, ObjectType::MotionPoint, ObjectType::Channel],
        EditContext::Auto => &[],
    }
}

#[command(
    id = "edit.selection.set",
    kind = ui,
    label = "Set selection",
    scopes = [ui],
    needs_project,
    example(title = "Select the first element of the first scene", params = r#"{"context": "staging", "ids": ["scene:#1/element:#1"]}"#),
    gui = "Selection in every view and panel (M4)",
)]
pub(crate) fn edit_selection_set(cx: &mut Cx, p: SelectionSetParams) -> Result<SelectionResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let allowed = context_types(context);
    let mut keys = Vec::new();
    for (i, (ty, key)) in cx.resolve_objects(&p.ids)?.into_iter().enumerate() {
        if !allowed.contains(&ty) {
            return Err(CommandError::with_details(
                tt_model::codes::REF_WRONG_TYPE,
                format!("a {ty} cannot be selected in the {} context", context.as_str()),
                json!({ "pointer": format!("/ids/{i}"), "id": key, "allowed": allowed.iter().map(|t| t.as_str()).collect::<Vec<_>>() }),
            ));
        }
        keys.push(key);
    }
    let mut cur = selection_in(cx, context);
    match p.mode {
        SelectMode::Replace => cur = keys,
        SelectMode::Add => keys.into_iter().for_each(|k| {
            if !cur.contains(&k) {
                cur.push(k)
            }
        }),
        SelectMode::Remove => cur.retain(|k| !keys.contains(k)),
        SelectMode::Toggle => {
            for k in keys {
                if let Some(i) = cur.iter().position(|x| *x == k) {
                    cur.remove(i);
                } else {
                    cur.push(k);
                }
            }
        }
    }
    let sel = &mut cx.app_mut().selection;
    context.as_str().clone_into(&mut sel.context);
    sel.ids.clone_from(&cur);
    Ok(SelectionResult { context: context.as_str().to_owned(), ids: cur })
}

/// Every selectable object of a context in the current context (scene, drawing, element).
fn all_of(cx: &Cx, context: EditContext) -> Result<Vec<String>, CommandError> {
    let doc = cx.document()?;
    let app = cx.app();
    Ok(match context {
        EditContext::Staging | EditContext::Timeline => app
            .scene
            .as_ref()
            .and_then(|s| doc.scene(s))
            .map(|s| doc.tree_order(s))
            .unwrap_or_default()
            .iter()
            .map(|i| i.as_str().to_owned())
            .collect(),
        EditContext::Xsheet => app
            .scene
            .as_ref()
            .and_then(|s| doc.scene(s))
            .map(|s| s.column_order().iter().map(|i| i.as_str().to_owned()).collect())
            .unwrap_or_default(),
        EditContext::Drawing => match app.current_drawing(doc).and_then(|d| doc.drawing(&d)) {
            Some(d) => {
                let prefix = format!("{}/", d.id());
                let mut out = Vec::new();
                for l in doc.locations() {
                    if l.ty == ObjectType::Stroke && l.key.starts_with(&prefix) {
                        out.push(l.key.clone());
                    }
                }
                out.sort();
                out
            }
            None => Vec::new(),
        },
        EditContext::Library => {
            let mut out: Vec<String> = doc
                .locations()
                .filter(|l| matches!(l.ty, ObjectType::LibraryFolder | ObjectType::LibraryItem))
                .map(|l| l.key.clone())
                .collect();
            out.sort();
            out
        }
        EditContext::FunctionEditor => {
            let el = app.element.as_ref().map(|e| e.as_str().to_owned()).unwrap_or_default();
            let mut out: Vec<String> = doc
                .locations()
                .filter(|l| matches!(l.ty, ObjectType::Key | ObjectType::MotionPoint))
                .filter(|l| {
                    l.parent.as_deref().and_then(|c| doc.locate(c)).and_then(|c| c.parent.clone()).as_deref()
                        == Some(el.as_str())
                })
                .map(|l| l.key.clone())
                .collect();
            out.sort();
            out
        }
        EditContext::Auto => Vec::new(),
    })
}

/// `edit.selectAll` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SelectAllResult {
    /// The resolved context.
    context: String,
    /// The selection.
    ids: Vec<String>,
}

#[command(
    id = "edit.selectAll",
    kind = ui,
    label = "Select All",
    scopes = [ui],
    needs_project,
    example(title = "Select every element of the current scene", params = r#"{"context": "staging"}"#),
)]
pub(crate) fn edit_select_all(cx: &mut Cx, p: ContextParams) -> Result<SelectAllResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let ids = all_of(cx, context)?;
    let sel = &mut cx.app_mut().selection;
    context.as_str().clone_into(&mut sel.context);
    sel.ids.clone_from(&ids);
    Ok(SelectAllResult { context: context.as_str().to_owned(), ids })
}

/// `{context}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ContextResult {
    /// The resolved context.
    context: String,
}

#[command(
    id = "edit.deselectAll",
    kind = ui,
    label = "Deselect All",
    scopes = [ui],
    needs_project,
    example(title = "Clear the selection of the focused context", params = "{}"),
    gui = "Edit menu and Escape in views (M4)",
)]
pub(crate) fn edit_deselect_all(cx: &mut Cx, p: ContextParams) -> Result<ContextResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let sel = &mut cx.app_mut().selection;
    if sel.context == context.as_str() {
        sel.ids.clear();
    }
    Ok(ContextResult { context: context.as_str().to_owned() })
}

#[command(
    id = "edit.selection.changed",
    kind = event,
    label = "Selection changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn edit_selection_changed(_cx: &mut Cx, _p: NoParams) -> Result<SelectionResult, CommandError> {
    Err(event_not_invocable("edit.selection.changed"))
}

// ----- clipboard --------------------------------------------------------------------------------------------------

/// Clipboard content kinds (01 §12.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ClipboardKind {
    /// Drawing objects.
    DrawingObjects,
    /// Elements.
    Elements,
    /// Element properties.
    ElementProperties,
    /// Cells.
    Cells,
    /// Keyframes.
    Keyframes,
    /// A motion point.
    MotionPoint,
    /// Library templates.
    Templates,
    /// Function points.
    FunctionPoints,
    /// An image from the system clipboard.
    Image,
}

/// `Clipboard` (01 §12.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Clipboard {
    /// Content kind.
    kind: ClipboardKind,
    /// Context it was copied from.
    #[serde(default)]
    source_context: Option<String>,
    /// Project it was copied from.
    #[serde(default)]
    source_project_id: Option<String>,
    /// Number of items.
    #[schemars(range(min = 0, max = 1_000_000))]
    item_count: u32,
    /// Items in the section-10 shape of the kind (present only with `includeData`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 1_000_000))]
    items: Option<Vec<serde_json::Map<String, Value>>>,
    /// Cut-special options.
    #[serde(default)]
    options: Option<serde_json::Map<String, Value>>,
}

/// `edit.clipboard.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ClipboardGetParams {
    /// Include the items.
    #[serde(default)]
    include_data: bool,
}

/// `edit.clipboard.get` result: the clipboard, or `null` when it is empty.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ClipboardGetResult {
    /// Content.
    clipboard: Option<Clipboard>,
}

#[command(
    id = "edit.clipboard.get",
    kind = query,
    label = "Get clipboard",
    scopes = [read],
    example(title = "Kind and item count", params = r#"{"includeData": false}"#),
    agent_only = "the GUI reads the clipboard implicitly when pasting",
)]
pub(crate) fn edit_clipboard_get(cx: &mut Cx, p: ClipboardGetParams) -> Result<ClipboardGetResult, CommandError> {
    let clip: Option<Clipboard> = match &cx.session().clipboard {
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| CommandError::internal(e.to_string()))?),
        None => None,
    };
    Ok(ClipboardGetResult {
        clipboard: clip.map(|mut c| {
            if !p.include_data {
                c.items = None;
            }
            c
        }),
    })
}

/// `edit.clipboard.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClipboardSetParams {
    /// The content.
    content: Clipboard,
}

/// `{kind, itemCount}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardSummary {
    /// Kind.
    kind: ClipboardKind,
    /// Items.
    item_count: u32,
}

#[command(
    id = "edit.clipboard.set",
    kind = ui,
    label = "Set clipboard",
    scopes = [ui],
    example(
        title = "Put two element references on the clipboard",
        params = r#"{"content": {"kind": "elements", "sourceContext": "staging", "itemCount": 2, "items": [{"elementId": "e1"}, {"elementId": "e2"}], "options": null}}"#
    ),
    agent_only = "agents place prepared content on the clipboard; the GUI fills it by cutting and copying",
)]
pub(crate) fn edit_clipboard_set(cx: &mut Cx, p: ClipboardSetParams) -> Result<ClipboardSummary, CommandError> {
    if let Some(items) = &p.content.items
        && items.len() != p.content.item_count as usize
    {
        return Err(invalid("/content/itemCount", "itemCount must equal the number of items"));
    }
    let summary = ClipboardSummary { kind: p.content.kind, item_count: p.content.item_count };
    let v = serde_json::to_value(&p.content).map_err(|e| CommandError::internal(e.to_string()))?;
    cx.session_mut().clipboard = Some(v);
    cx.session_mut().emit("edit.clipboard.changed", json!({ "kind": summary.kind, "itemCount": summary.item_count }));
    Ok(summary)
}

#[command(
    id = "edit.clipboard.changed",
    kind = event,
    label = "Clipboard changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn edit_clipboard_changed(_cx: &mut Cx, _p: NoParams) -> Result<ClipboardSummary, CommandError> {
    Err(event_not_invocable("edit.clipboard.changed"))
}

/// `context` + `objectIds` parameters of cut, copy and delete.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ObjectsParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Objects (default: the selection).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    object_ids: Option<Vec<String>>,
}

/// `{clipboard: {kind, itemCount}}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ClipResult {
    /// What the clipboard holds now.
    clipboard: ClipboardSummary,
}

const AREA_OWNER: &str = "M4 C4.1";

#[command(
    id = "edit.cut",
    kind = action,
    label = "Cut",
    scopes = [edit],
    undo = "Cut",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Cut the selection of the focused context", params = "{}"),
)]
pub(crate) fn edit_cut(_cx: &mut Cx, _p: ObjectsParams) -> Result<ClipResult, CommandError> {
    Err(CommandError::not_implemented("edit.cut", AREA_OWNER))
}

#[command(
    id = "edit.copy",
    kind = ui,
    label = "Copy",
    scopes = [ui],
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Copy the selection of the focused context", params = "{}"),
)]
pub(crate) fn edit_copy(_cx: &mut Cx, _p: ObjectsParams) -> Result<ClipResult, CommandError> {
    Err(CommandError::not_implemented("edit.copy", AREA_OWNER))
}

/// `{deletedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// Deleted objects.
    deleted_ids: Vec<String>,
}

#[command(
    id = "edit.delete",
    kind = action,
    label = "Delete",
    scopes = [edit],
    undo = "Delete",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Delete the selection of the focused context", params = "{}"),
)]
pub(crate) fn edit_delete(_cx: &mut Cx, _p: ObjectsParams) -> Result<DeleteResult, CommandError> {
    Err(CommandError::not_implemented("edit.delete", AREA_OWNER))
}

/// `edit.paste` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasteParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Where to paste: `{drawingId}`, `{elementId, frame}`, `{catalogId}`, `{functionId, frame}` or `{objectIds}`.
    #[serde(default)]
    destination: Option<serde_json::Map<String, Value>>,
}

/// Paste result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasteResult {
    /// Created objects.
    created_ids: Vec<String>,
    /// Modified objects.
    modified_ids: Vec<String>,
    /// Warnings.
    warnings: Vec<crate::types::Warning>,
}

#[command(
    id = "edit.paste",
    kind = action,
    label = "Paste",
    scopes = [edit],
    undo = "Paste",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Paste into the focused context", params = "{}"),
)]
pub(crate) fn edit_paste(_cx: &mut Cx, _p: PasteParams) -> Result<PasteResult, CommandError> {
    Err(CommandError::not_implemented("edit.paste", AREA_OWNER))
}

fn yes() -> bool {
    true
}

/// `edit.cutSpecial` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CutSpecialParams {
    /// Objects (default: the selection).
    #[serde(default)]
    object_ids: Option<Vec<String>>,
    /// `TimelineEditOptions` (01 §12.4); omitted fields take the remembered values.
    #[serde(default)]
    options: Option<serde_json::Map<String, Value>>,
    /// Remember the options.
    #[serde(default = "yes")]
    remember: bool,
}

#[command(
    id = "edit.cutSpecial",
    kind = action,
    label = "Cut Special",
    scopes = [edit],
    undo = "Cut Special",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Cut the selected cells, keeping exposure", params = r#"{"options": {"createDeleteExposure": false}}"#),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_cut_special(_cx: &mut Cx, _p: CutSpecialParams) -> Result<ClipResult, CommandError> {
    Err(CommandError::not_implemented("edit.cutSpecial", AREA_OWNER))
}

/// `edit.pasteSpecial` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PasteSpecialParams {
    /// Where to paste.
    #[serde(default)]
    destination: Option<serde_json::Map<String, Value>>,
    /// `TimelineEditOptions` or `MotionPointPasteOptions` (01 §12.4).
    #[serde(default)]
    options: Option<serde_json::Map<String, Value>>,
    /// Use the last options.
    #[serde(default)]
    use_last: bool,
    /// Remember the options.
    #[serde(default = "yes")]
    remember: bool,
}

#[command(
    id = "edit.pasteSpecial",
    kind = action,
    label = "Paste Special",
    scopes = [edit],
    undo = "Paste Special",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Paste with the last options", params = r#"{"useLast": true}"#),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_paste_special(_cx: &mut Cx, _p: PasteSpecialParams) -> Result<PasteResult, CommandError> {
    Err(CommandError::not_implemented("edit.pasteSpecial", AREA_OWNER))
}

/// `edit.pasteAsNewElement` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PasteNewParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame (default current).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
}

/// `{elementId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewElementResult {
    /// The new element.
    element_id: String,
}

#[command(
    id = "edit.pasteAsNewElement",
    kind = action,
    label = "Paste as New Element",
    scopes = [edit],
    undo = "Paste as New Element",
    needs_project,
    not_implemented = "M4 C4.1",
    example(title = "Paste drawing objects as a new element at the current frame", params = "{}"),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_paste_as_new_element(_cx: &mut Cx, _p: PasteNewParams) -> Result<NewElementResult, CommandError> {
    Err(CommandError::not_implemented("edit.pasteAsNewElement", AREA_OWNER))
}
