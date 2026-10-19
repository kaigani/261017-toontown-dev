//! `edit.*` (01 §6, §12.1): undo/redo and history (kernel), selection and clipboard state (kernel), and the
//! context-dependent clipboard edits whose semantics belong to the areas (declared here, implemented with their
//! areas and wired to the GUI in M4 C4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::document::Target;
use tt_registry::{CommandError, codes, command};

use super::common::{EditContext, NoParams, event_not_invocable};
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

pub(crate) fn resolve_context(cx: &Cx, c: EditContext) -> EditContext {
    let central = cx.ui().central_panel().to_owned();
    c.resolve(cx.ui().focused(), &central)
}

/// The selection of `context` (each context keeps its own, 01 §6.4).
pub(crate) fn selection_in(cx: &Cx, context: EditContext) -> Vec<String> {
    cx.app().selection_of(context.as_str())
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
    let cells = matches!(context, EditContext::Xsheet | EditContext::Timeline);
    let mut keys = Vec::new();
    for (i, r) in p.ids.iter().enumerate() {
        let wrong = |what: &str, id: &str| {
            CommandError::with_details(
                tt_model::codes::REF_WRONG_TYPE,
                format!("a {what} cannot be selected in the {} context", context.as_str()),
                json!({ "pointer": format!("/ids/{i}"), "id": id, "allowed": allowed.iter().map(|t| t.as_str()).collect::<Vec<_>>() }),
            )
        };
        if r == "@selection" {
            for (ty, key) in cx.resolve_objects(std::slice::from_ref(r))? {
                if !allowed.contains(&ty) {
                    return Err(wrong(ty.as_str(), &key));
                }
                keys.push(key);
            }
            continue;
        }
        match cx.resolve(r)? {
            Target::Object { ty, key } if allowed.contains(&ty) => keys.push(key),
            Target::Object { ty, key } => return Err(wrong(ty.as_str(), &key)),
            // Cells are selected in the Exposure Sheet and the Timeline (01 §6.2, §6.4).
            Target::Cell { element, frame } if cells => keys.push(crate::app::cell_ref(element.as_str(), frame)),
            Target::Cell { element, .. } => return Err(wrong("cell", element.as_str())),
        }
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
    cx.app_mut().set_selection(context.as_str(), cur.clone());
    Ok(SelectionResult { context: context.as_str().to_owned(), ids: cur })
}

/// Every selectable object of a context in the current context (scene, drawing, element).
/// Most cells one `edit.selectAll` selects.
const ALL_CELLS_LIMIT: usize = 100_000;

fn all_of(cx: &Cx, context: EditContext) -> Result<Vec<String>, CommandError> {
    let doc = cx.document()?;
    let app = cx.app();
    Ok(match context {
        EditContext::Staging => app
            .scene
            .as_ref()
            .and_then(|s| doc.scene(s))
            .map(|s| doc.tree_order(s))
            .unwrap_or_default()
            .iter()
            .map(|i| i.as_str().to_owned())
            .collect(),
        // All cells (01 §6.4): every frame of the scene in every cell-based column, Exposure Sheet order for the
        // Exposure Sheet and Timeline order for the Timeline.
        EditContext::Xsheet | EditContext::Timeline => {
            let Some(scene) = app.scene.as_ref().and_then(|s| doc.scene(s)) else { return Ok(Vec::new()) };
            let len = u32::try_from(doc.scene_length(scene)).unwrap_or(u32::MAX);
            let columns: Vec<tt_model::Id> = if context == EditContext::Xsheet {
                scene.column_order().clone()
            } else {
                doc.tree_order(scene)
                    .into_iter()
                    .filter(|e| doc.element(e).is_some_and(|x| x.element_type().is_cell_based()))
                    .collect()
            };
            let total = columns.len().saturating_mul(len as usize);
            if total > ALL_CELLS_LIMIT {
                return Err(CommandError::with_details(
                    codes::LIMIT_EXCEEDED,
                    format!("selecting all {total} cells exceeds {ALL_CELLS_LIMIT}"),
                    json!({ "limit": ALL_CELLS_LIMIT, "actual": total }),
                ));
            }
            columns.iter().flat_map(|e| (1..=len).map(move |f| crate::app::cell_ref(e.as_str(), f))).collect()
        }
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
    cx.app_mut().set_selection(context.as_str(), ids.clone());
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
    cx.app_mut().set_selection(context.as_str(), Vec::new());
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

impl ClipboardKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DrawingObjects => "drawingObjects",
            Self::Elements => "elements",
            Self::ElementProperties => "elementProperties",
            Self::Cells => "cells",
            Self::Keyframes => "keyframes",
            Self::MotionPoint => "motionPoint",
            Self::Templates => "templates",
            Self::FunctionPoints => "functionPoints",
            Self::Image => "image",
        }
    }
}

/// `Clipboard` (01 §12.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Clipboard {
    /// Content kind.
    pub(crate) kind: ClipboardKind,
    /// Context it was copied from.
    #[serde(default)]
    pub(crate) source_context: Option<String>,
    /// Project it was copied from.
    #[serde(default)]
    pub(crate) source_project_id: Option<String>,
    /// Number of items.
    #[schemars(range(min = 0, max = 1_000_000))]
    pub(crate) item_count: u32,
    /// Items in the shape of the kind (present only with `includeData`); see `validate_clipboard`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 1_000_000))]
    pub(crate) items: Option<Vec<serde_json::Map<String, Value>>>,
    /// Cut-special options.
    #[serde(default)]
    pub(crate) options: Option<serde_json::Map<String, Value>>,
}

fn clipboard_invalid(at: String, message: impl Into<String>) -> CommandError {
    CommandError::with_details("edit.clipboardInvalid", message, json!({ "pointer": at }))
}

fn is_id(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str).is_some_and(|s| !s.is_empty())
}

/// Checks that an item has only the allowed fields.
fn fields_in(item: &serde_json::Map<String, Value>, allowed: &[&str], at: &str) -> Result<(), CommandError> {
    match item.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(clipboard_invalid(
            format!("{at}/{k}"),
            format!("unknown field `{k}` (allowed: {})", allowed.join(", ")),
        )),
        None => Ok(()),
    }
}

fn object_of(ty: ObjectType, v: Option<&Value>, at: String) -> Result<(), CommandError> {
    let Some(v) = v else { return Err(clipboard_invalid(at, format!("missing {ty}"))) };
    match tt_model::schema::validate(ty, v, 1)?.into_iter().next() {
        Some(violation) => Err(clipboard_invalid(
            format!("{at}{}", violation.pointer),
            format!("not a valid {ty}: {}", violation.message),
        )),
        None => Ok(()),
    }
}

/// Validates clipboard content against the schema of its kind (01 §12.1 `edit.clipboard.set`, §12.4; C12-R1-18).
/// Items per kind:
///
/// * `drawingObjects`: `stroke` and `drawingNode` objects (section 10, the drawing read-back of 02);
/// * `elements`: `element` objects, or `{elementId}` references to elements of the project;
/// * `elementProperties`: maps of element property names to values;
/// * `cells`: `{elementId, frameOffset, drawingRef, drawingContent?}`;
/// * `keyframes`, `functionPoints`: `{channel, elementId?, key}` with `key` a `key` object;
/// * `motionPoint`: one `{channel, elementId?, point}` with `point` a `motionPoint` object;
/// * `templates`: `libraryFolder` and `libraryItem` objects;
/// * `image`: `{mediaType, width, height, base64}`.
pub(crate) fn validate_clipboard(c: &Clipboard) -> Result<(), CommandError> {
    let Some(items) = &c.items else { return Ok(()) };
    if items.len() != c.item_count as usize {
        return Err(clipboard_invalid("/content/itemCount".into(), "itemCount must equal the number of items"));
    }
    if c.kind == ClipboardKind::MotionPoint && items.len() > 1 {
        return Err(clipboard_invalid("/content/items".into(), "a motionPoint clipboard holds one point"));
    }
    for (i, item) in items.iter().enumerate() {
        let at = format!("/content/items/{i}");
        let ty_of = || item.get("type").and_then(Value::as_str).and_then(ObjectType::parse);
        match c.kind {
            ClipboardKind::DrawingObjects => match ty_of() {
                Some(t @ (ObjectType::Stroke | ObjectType::DrawingNode)) => {
                    object_of(t, Some(&Value::Object(item.clone())), at)?;
                }
                _ => {
                    return Err(clipboard_invalid(
                        format!("{at}/type"),
                        "drawing objects are strokes and drawing nodes",
                    ));
                }
            },
            ClipboardKind::Elements => {
                if item.contains_key("elementId") {
                    fields_in(item, &["elementId"], &at)?;
                    if !is_id(item.get("elementId")) {
                        return Err(clipboard_invalid(format!("{at}/elementId"), "elementId is an ID"));
                    }
                } else {
                    object_of(ObjectType::Element, Some(&Value::Object(item.clone())), at)?;
                }
            }
            ClipboardKind::ElementProperties => {
                if item.is_empty() {
                    return Err(clipboard_invalid(at, "an element property map is not empty"));
                }
                for k in item.keys() {
                    let pointer = format!("/{}", tt_model::pointer::escape(k));
                    if tt_model::schema::property_schema(ObjectType::Element, &pointer).is_none() {
                        return Err(clipboard_invalid(
                            format!("{at}/{k}"),
                            format!("`{k}` is not an element property"),
                        ));
                    }
                }
            }
            ClipboardKind::Cells => {
                fields_in(item, &["elementId", "frameOffset", "drawingRef", "drawingContent"], &at)?;
                if !is_id(item.get("elementId")) {
                    return Err(clipboard_invalid(format!("{at}/elementId"), "elementId is an ID"));
                }
                if item.get("frameOffset").and_then(Value::as_u64).is_none_or(|n| n >= 100_000) {
                    return Err(clipboard_invalid(format!("{at}/frameOffset"), "frameOffset is an integer 0-99999"));
                }
                let d = item.get("drawingRef");
                if !(d == Some(&Value::Null) || is_id(d)) {
                    return Err(clipboard_invalid(format!("{at}/drawingRef"), "drawingRef is a drawing ID or null"));
                }
                if let Some(d) = item.get("drawingContent") {
                    object_of(ObjectType::Drawing, Some(d), format!("{at}/drawingContent"))?;
                }
            }
            ClipboardKind::Keyframes | ClipboardKind::FunctionPoints => {
                fields_in(item, &["channel", "elementId", "key"], &at)?;
                if !is_id(item.get("channel")) {
                    return Err(clipboard_invalid(format!("{at}/channel"), "channel is a channel name"));
                }
                object_of(ObjectType::Key, item.get("key"), format!("{at}/key"))?;
            }
            ClipboardKind::MotionPoint => {
                fields_in(item, &["channel", "elementId", "point"], &at)?;
                if !is_id(item.get("channel")) {
                    return Err(clipboard_invalid(format!("{at}/channel"), "channel is a channel name"));
                }
                object_of(ObjectType::MotionPoint, item.get("point"), format!("{at}/point"))?;
            }
            ClipboardKind::Templates => match ty_of() {
                Some(t @ (ObjectType::LibraryFolder | ObjectType::LibraryItem)) => {
                    object_of(t, Some(&Value::Object(item.clone())), at)?;
                }
                _ => return Err(clipboard_invalid(format!("{at}/type"), "templates are library folders and items")),
            },
            ClipboardKind::Image => {
                fields_in(item, &["mediaType", "width", "height", "base64"], &at)?;
                let dim = |k: &str| item.get(k).and_then(Value::as_u64).is_some_and(|n| (1..=65_535).contains(&n));
                if !item.get("mediaType").and_then(Value::as_str).is_some_and(|m| m.starts_with("image/"))
                    || !dim("width")
                    || !dim("height")
                    || !item.get("base64").is_some_and(Value::is_string)
                {
                    return Err(clipboard_invalid(at, "an image is {mediaType: image/*, width, height, base64}"));
                }
            }
        }
    }
    Ok(())
}

/// `edit.clipboard.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ClipboardGetParams {
    /// Include the items.
    #[serde(default)]
    include_data: bool,
}

/// Returns the `Clipboard` itself (01 §12.1; C12-R1-17), `null` when it is empty.
#[command(
    id = "edit.clipboard.get",
    kind = query,
    label = "Get clipboard",
    scopes = [read],
    needs_project,
    example(title = "Kind and item count", params = r#"{"includeData": false}"#),
    agent_only = "the GUI reads the clipboard implicitly when pasting",
)]
pub(crate) fn edit_clipboard_get(cx: &mut Cx, p: ClipboardGetParams) -> Result<Option<Clipboard>, CommandError> {
    let clip: Option<Clipboard> = match &cx.session().clipboard {
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| CommandError::internal(e.to_string()))?),
        None => None,
    };
    Ok(clip.map(|mut c| {
        if !p.include_data {
            c.items = None;
        }
        c
    }))
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
    pub(crate) kind: ClipboardKind,
    /// Items.
    pub(crate) item_count: u32,
}

/// Puts content on the clipboard and announces it (the event is delivered when the transaction commits).
pub(crate) fn store_clipboard(cx: &mut Cx, clip: &Clipboard) -> Result<ClipboardSummary, CommandError> {
    validate_clipboard(clip)?;
    let summary = ClipboardSummary { kind: clip.kind, item_count: clip.item_count };
    cx.session_mut().clipboard = Some(serde_json::to_value(clip).map_err(|e| CommandError::internal(e.to_string()))?);
    cx.emit("edit.clipboard.changed", json!({ "kind": summary.kind, "itemCount": summary.item_count }));
    Ok(summary)
}

#[command(
    id = "edit.clipboard.set",
    kind = ui,
    label = "Set clipboard",
    scopes = [ui],
    needs_project,
    example(
        title = "Put an element reference on the clipboard",
        params = r#"{"content": {"kind": "elements", "sourceContext": "staging", "itemCount": 1, "items": [{"elementId": "scene:#1/element:#1"}], "options": null}}"#
    ),
    agent_only = "agents place prepared content on the clipboard; the GUI fills it by cutting and copying",
)]
pub(crate) fn edit_clipboard_set(cx: &mut Cx, p: ClipboardSetParams) -> Result<ClipboardSummary, CommandError> {
    store_clipboard(cx, &p.content)
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
