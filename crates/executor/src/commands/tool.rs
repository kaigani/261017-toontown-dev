//! Drawing tools, tool options, pens and the global drawing settings (02 §5, §6, §7.7, §12.3, §13.2; M4 C4.2,
//! reviews/M4-design-gestures.md §1). Tool selection, options and global settings are the user's drawing settings
//! ([`crate::drawing`]): not undoable, persisted per user. Pens are document content (undoable actions, 02 §6.1); the
//! current pen of each family is project-scope session state (`session.json`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::entities::pen::PenKind;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::draw::op;
use crate::cx::Cx;
use crate::drawing::{self, GROUPS};
use tt_model::document::EntryKey;

fn err(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message, details)
}

/// The current pen of a family: the session's (02 §6.1) while it exists, else the family default (02 §7.7: brush
/// "Pen 3", eraser "Eraser 2"), else the family's first pen.
pub(crate) fn current_pen(cx: &Cx, kind: PenKind) -> Option<Id> {
    let doc = cx.document().ok()?;
    let pens = doc.project().pens();
    let chosen = match kind {
        PenKind::Brush => cx.app().pens.brush.as_ref(),
        PenKind::Eraser => cx.app().pens.eraser.as_ref(),
    };
    if let Some(id) = chosen.filter(|id| pens.iter().any(|p| p.id() == *id && *p.kind() == kind)) {
        return Some(id.clone());
    }
    let default = if kind == PenKind::Brush { "Pen 3" } else { "Eraser 2" };
    pens.iter()
        .find(|p| *p.kind() == kind && p.name() == default)
        .or_else(|| pens.iter().find(|p| *p.kind() == kind))
        .map(|p| p.id().clone())
}

fn kind_str(k: PenKind) -> &'static str {
    match k {
        PenKind::Brush => "brush",
        PenKind::Eraser => "eraser",
    }
}

fn tool_param(tool: &str) -> Result<(), CommandError> {
    if drawing::is_tool(tool) {
        Ok(())
    } else {
        Err(err(codes::ARG_INVALID, format!("`{tool}` is not a tool"), json!({ "pointer": "/toolId" })))
    }
}

// ----- events ---------------------------------------------------------------------------------------------------------

/// `tool.active.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveChanged {
    /// The active tool.
    tool_id: String,
    /// A temporary override.
    temporary: bool,
}

#[command(
    id = "tool.active.changed",
    kind = event,
    label = "Active tool changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn tool_active_changed(_cx: &mut Cx, _p: NoParams) -> Result<ActiveChanged, CommandError> {
    Err(super::common::event_not_invocable("tool.active.changed"))
}

/// `tool.options.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OptionsChanged {
    /// The tool (`null` for a global drawing setting).
    tool_id: Option<String>,
    /// The options or settings now.
    options: Map<String, Value>,
}

#[command(
    id = "tool.options.changed",
    kind = event,
    label = "A tool option or global drawing setting changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn tool_options_changed(_cx: &mut Cx, _p: NoParams) -> Result<OptionsChanged, CommandError> {
    Err(super::common::event_not_invocable("tool.options.changed"))
}

/// A pen change.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PenChange {
    /// Created.
    Created,
    /// Settings changed or renamed.
    Modified,
    /// Deleted.
    Deleted,
    /// Made current.
    Selected,
}

/// `tool.pen.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenChanged {
    /// The pen.
    pen_id: String,
    /// What changed.
    change: PenChange,
}

#[command(
    id = "tool.pen.changed",
    kind = event,
    label = "Pen list or pen settings changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn tool_pen_changed(_cx: &mut Cx, _p: NoParams) -> Result<PenChanged, CommandError> {
    Err(super::common::event_not_invocable("tool.pen.changed"))
}

fn emit_pen(cx: &mut Cx, id: &str, change: PenChange) {
    cx.emit("tool.pen.changed", json!({ "penId": id, "change": change }));
}

/// Announces a change of the global drawing settings (`tool.options.changed` with `toolId: null`).
pub(crate) fn announce_settings(cx: &mut Cx) {
    emit_options(cx, None);
}

fn emit_options(cx: &mut Cx, tool: Option<&str>) {
    let options = match tool {
        Some(t) => cx.ui().drawing().options_of(t),
        None => settings_map(cx),
    };
    cx.emit("tool.options.changed", json!({ "toolId": tool, "options": options }));
}

// ----- tool.list, tool.active.* ---------------------------------------------------------------------------------------

/// One tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolEntry {
    /// Tool id.
    tool_id: String,
    /// Its group.
    group: String,
    /// Current options (empty: the tool has none).
    options: Map<String, Value>,
    /// The options' schema.
    option_schema: Value,
}

/// `tool.list` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolList {
    /// Every tool in palette order.
    tools: Vec<ToolEntry>,
    /// The active tool.
    active_tool_id: String,
}

fn option_schema(tool: &str) -> Value {
    let props: Map<String, Value> = drawing::default_options(tool)
        .keys()
        .map(|k| {
            let s = match (tool, k.as_str()) {
                ("select", "rectRule") => json!({ "enum": ["inside", "intersect"] }),
                (_, "thicknessSource") => json!({ "enum": ["penMax", "fixed"] }),
                (_, "thickness") => json!({ "type": ["number", "null"], "minimum": 0, "maximum": 100 }),
                (_, "snap" | "bold" | "italic" | "autoKern") => json!({ "type": "boolean" }),
                (_, "snapRadius") => json!({ "type": "number", "minimum": 1, "maximum": 50 }),
                (_, "font") => json!({ "type": ["string", "null"] }),
                (_, "size") => json!({ "type": "number", "minimum": 1, "maximum": 512 }),
                (_, "baseline") => json!({ "enum": ["normal", "subscript", "superscript"] }),
                (_, "kerning") => json!({ "type": "integer", "minimum": -60, "maximum": 60 }),
                (_, "align") => json!({ "enum": ["left", "centre", "right", "justify"] }),
                (_, "leftMargin" | "rightMargin" | "indent" | "lineSpacing") => {
                    json!({ "type": "number", "minimum": -10_000, "maximum": 10_000 })
                }
                (_, "outlineColourId") => json!({ "type": ["string", "null"] }),
                _ => json!({}),
            };
            (k.clone(), s)
        })
        .collect();
    json!({ "type": "object", "properties": props, "additionalProperties": false })
}

#[command(
    id = "tool.list",
    kind = query,
    label = "List tools",
    scopes = [read],
    needs_project,
    example(title = "Every drawing tool with its options", params = "{}"),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_list(cx: &mut Cx, _p: NoParams) -> Result<ToolList, CommandError> {
    let d = cx.ui().drawing();
    let tools = GROUPS
        .iter()
        .flat_map(|(g, tools)| tools.iter().map(move |t| (g, t)))
        .map(|(g, t)| ToolEntry {
            tool_id: (*t).to_owned(),
            group: (*g).to_owned(),
            options: d.options_of(t),
            option_schema: option_schema(t),
        })
        .collect();
    Ok(ToolList { tools, active_tool_id: d.active_tool.clone() })
}

/// `tool.active.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ActiveParams {
    /// The tool.
    tool_id: String,
}

/// `{toolId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolIdResult {
    /// The active tool.
    tool_id: String,
}

/// Makes `tool` active (and its group's shown tool), announcing a change.
pub(crate) fn activate(cx: &mut Cx, tool: &str) {
    let d = cx.ui_mut().drawing_mut();
    let changed = d.active_tool != tool;
    d.active_tool = tool.to_owned();
    if let Some(g) = drawing::group_of(tool) {
        d.last_in_group.insert(g.to_owned(), tool.to_owned());
    }
    if changed {
        cx.emit("tool.active.changed", json!({ "toolId": tool, "temporary": false }));
    }
}

#[command(
    id = "tool.active.set",
    kind = ui,
    label = "Activate a tool",
    scopes = [ui],
    needs_project,
    example(title = "The Brush tool", params = r#"{"toolId": "brush"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_active_set(cx: &mut Cx, p: ActiveParams) -> Result<ToolIdResult, CommandError> {
    tool_param(&p.tool_id)?;
    activate(cx, &p.tool_id);
    Ok(ToolIdResult { tool_id: p.tool_id })
}

/// `tool.active.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveResult {
    /// The active tool.
    tool_id: String,
    /// A temporary override in force (`null`: none; overrides are held by the GUI while a key is held).
    temporary_tool_id: Option<String>,
    /// Its group.
    group: String,
}

#[command(
    id = "tool.active.get",
    kind = query,
    label = "Get the active tool",
    scopes = [read],
    needs_project,
    example(title = "The active tool", params = "{}"),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_active_get(cx: &mut Cx, _p: NoParams) -> Result<ActiveResult, CommandError> {
    let t = cx.ui().drawing().active_tool.clone();
    Ok(ActiveResult {
        group: drawing::group_of(&t).unwrap_or("select").to_owned(),
        tool_id: t,
        temporary_tool_id: None,
    })
}

/// `tool.group.select` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GroupParams {
    /// The group.
    group: Group,
    /// Advance within the group if it is already active (group cycling).
    #[serde(default = "yes")]
    advance: bool,
}

fn yes() -> bool {
    true
}

/// A tool group (02 §5.1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Group {
    /// Select group.
    Select,
    /// Brush group.
    Brush,
    /// Paint group.
    Paint,
    /// Eraser group.
    Eraser,
    /// Navigate group.
    Navigate,
    /// Text group.
    Text,
}

#[command(
    id = "tool.group.select",
    kind = ui,
    label = "Select a tool group",
    scopes = [ui],
    needs_project,
    example(title = "The Brush group (cycling when it is active)", params = r#"{"group": "brush"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_group_select(cx: &mut Cx, p: GroupParams) -> Result<ToolIdResult, CommandError> {
    let g = serde_json::to_value(p.group).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    let d = cx.ui().drawing();
    let tools = GROUPS.iter().find(|(name, _)| *name == g).map(|(_, t)| *t).unwrap_or(&[]);
    let shown = d.shown_in_group(&g).to_owned();
    let active_here = drawing::group_of(&d.active_tool) == Some(g.as_str());
    let next = if active_here && p.advance {
        let i = tools.iter().position(|t| *t == d.active_tool).unwrap_or(0);
        tools[(i + 1) % tools.len().max(1)].to_owned()
    } else {
        shown
    };
    activate(cx, &next);
    Ok(ToolIdResult { tool_id: next })
}

// ----- options ----------------------------------------------------------------------------------------------------------

/// `toolId`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ToolParams {
    /// The tool.
    tool_id: String,
}

/// `{toolId, options}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolOptions {
    /// The tool.
    tool_id: String,
    /// Its options.
    options: Map<String, Value>,
}

#[command(
    id = "tool.options.get",
    kind = query,
    label = "Get tool options",
    scopes = [read],
    needs_project,
    example(title = "The Select tool's options", params = r#"{"toolId": "select"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_options_get(cx: &mut Cx, p: ToolParams) -> Result<ToolOptions, CommandError> {
    tool_param(&p.tool_id)?;
    Ok(ToolOptions { options: cx.ui().drawing().options_of(&p.tool_id), tool_id: p.tool_id })
}

/// Sets options of a tool after checking every one (all or nothing).
fn set_options(cx: &mut Cx, tool: &str, given: &Map<String, Value>) -> Result<Map<String, Value>, CommandError> {
    tool_param(tool)?;
    for (k, v) in given {
        drawing::check_option(tool, k, v).map_err(|(code, message)| {
            err(code, message, json!({ "pointer": format!("/options/{k}"), "toolId": tool }))
        })?;
    }
    let before = cx.ui().drawing().options_of(tool);
    let stored = cx.ui_mut().drawing_mut().options.entry(tool.to_owned()).or_default();
    for (k, v) in given {
        stored.insert(k.clone(), v.clone());
    }
    let after = cx.ui().drawing().options_of(tool);
    if after != before {
        emit_options(cx, Some(tool));
    }
    Ok(after)
}

/// `tool.options.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OptionsParams {
    /// The tool.
    tool_id: String,
    /// Options to set.
    options: Map<String, Value>,
}

#[command(
    id = "tool.options.set",
    kind = ui,
    label = "Set tool options",
    scopes = [ui],
    needs_project,
    example(title = "Polyline snapping radius", params = r#"{"toolId": "polyline", "options": {"snapRadius": 12}}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_options_set(cx: &mut Cx, p: OptionsParams) -> Result<ToolOptions, CommandError> {
    let options = set_options(cx, &p.tool_id, &p.options)?;
    Ok(ToolOptions { tool_id: p.tool_id, options })
}

/// `{options}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct OptionsResult {
    /// The tool's options now.
    options: Map<String, Value>,
}

/// Select tool options.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectOptions {
    /// `inside` (objects wholly inside) or `intersect`.
    #[serde(default = "inside")]
    rect_rule: RectRule,
}

fn inside() -> RectRule {
    RectRule::Inside
}

/// Marquee and lasso rule.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RectRule {
    /// Wholly inside.
    Inside,
    /// Intersecting.
    Intersect,
}

fn to_map<T: Serialize>(v: &T) -> Map<String, Value> {
    serde_json::to_value(v).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

#[command(
    id = "tool.select.options.set",
    kind = ui,
    label = "Select tool options",
    scopes = [ui],
    needs_project,
    example(title = "Select objects the marquee touches", params = r#"{"rectRule": "intersect"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_select_options_set(cx: &mut Cx, p: SelectOptions) -> Result<OptionsResult, CommandError> {
    Ok(OptionsResult { options: set_options(cx, "select", &to_map(&p))? })
}

/// `penId`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenParams {
    /// The pen.
    pen_id: String,
}

fn find_pen(cx: &Cx, r: &str) -> Result<(Id, PenKind, String), CommandError> {
    let key = cx.resolve_as(r, ObjectType::Pen)?;
    let doc = cx.document()?;
    doc.project()
        .pens()
        .iter()
        .find(|p| p.id().as_str() == key)
        .map(|p| (p.id().clone(), *p.kind(), p.name().clone()))
        .ok_or_else(|| err("pen.notFound", format!("no pen `{r}`"), json!({ "pointer": "/penId", "penId": r })))
}

fn select_pen(cx: &mut Cx, r: &str, want: Option<PenKind>) -> Result<(Id, PenKind), CommandError> {
    let (id, kind, name) = find_pen(cx, r)?;
    if want.is_some_and(|w| w != kind) {
        return Err(err(
            "tool.notApplicable",
            format!("pen `{name}` is a {} pen", kind_str(kind)),
            json!({ "pointer": "/penId", "penId": id.as_str() }),
        ));
    }
    let slot = match kind {
        PenKind::Brush => &mut cx.app_mut().pens.brush,
        PenKind::Eraser => &mut cx.app_mut().pens.eraser,
    };
    let changed = slot.as_ref() != Some(&id);
    *slot = Some(id.clone());
    if changed {
        emit_pen(cx, id.as_str(), PenChange::Selected);
    }
    Ok((id, kind))
}

fn pen_options(cx: &Cx, kind: PenKind) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("penId".into(), json!(current_pen(cx, kind).map(|p| p.as_str().to_owned())));
    m
}

#[command(
    id = "tool.brush.options.set",
    kind = ui,
    label = "Brush options",
    scopes = [ui],
    needs_project,
    example(title = "Make Pen 1 the current brush pen", params = r#"{"penId": "pen:\"Pen 1\""}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_brush_options_set(cx: &mut Cx, p: PenParams) -> Result<OptionsResult, CommandError> {
    select_pen(cx, &p.pen_id, Some(PenKind::Brush))?;
    Ok(OptionsResult { options: pen_options(cx, PenKind::Brush) })
}

#[command(
    id = "tool.eraser.options.set",
    kind = ui,
    label = "Eraser options",
    scopes = [ui],
    needs_project,
    example(title = "Make Eraser 1 the current eraser pen", params = r#"{"penId": "pen:\"Eraser 1\""}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_eraser_options_set(cx: &mut Cx, p: PenParams) -> Result<OptionsResult, CommandError> {
    select_pen(cx, &p.pen_id, Some(PenKind::Eraser))?;
    Ok(OptionsResult { options: pen_options(cx, PenKind::Eraser) })
}

/// Pencil and shape-tool line thickness.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PencilOptions {
    /// `penMax` (the current brush pen's maximum size) or `fixed`.
    #[serde(default = "pen_max")]
    thickness_source: ThicknessSource,
    /// The fixed thickness (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    thickness: Option<f64>,
}

fn pen_max() -> ThicknessSource {
    ThicknessSource::PenMax
}

/// Where the line thickness comes from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThicknessSource {
    /// The current brush pen's maximum size.
    PenMax,
    /// A fixed thickness.
    Fixed,
}

#[command(
    id = "tool.pencil.options.set",
    kind = ui,
    label = "Pencil options",
    scopes = [ui],
    needs_project,
    example(title = "A fixed line thickness of 3 du", params = r#"{"thicknessSource": "fixed", "thickness": 3}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pencil_options_set(cx: &mut Cx, p: PencilOptions) -> Result<OptionsResult, CommandError> {
    let m = to_map(&p);
    // "Pencil and shape-tool line thickness source" (02 §13.2): one setting shared by the Pencil and the shape tools.
    for t in ["rectangle", "ellipse", "line", "polyline"] {
        set_options(cx, t, &m)?;
    }
    Ok(OptionsResult { options: set_options(cx, "pencil", &m)? })
}

/// Polyline snapping.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PolylineOptions {
    /// Snap to existing anchors.
    #[serde(default = "yes")]
    snap: bool,
    /// Snap radius in view pixels.
    #[serde(default = "eight")]
    #[schemars(range(min = 1.0, max = 50.0))]
    snap_radius: f64,
}

fn eight() -> f64 {
    8.0
}

#[command(
    id = "tool.polyline.options.set",
    kind = ui,
    label = "Polyline options",
    scopes = [ui],
    needs_project,
    example(title = "Snap within 12 view pixels", params = r#"{"snap": true, "snapRadius": 12}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_polyline_options_set(cx: &mut Cx, p: PolylineOptions) -> Result<OptionsResult, CommandError> {
    Ok(OptionsResult { options: set_options(cx, "polyline", &to_map(&p))? })
}

/// Text defaults for new text (02 §7.6.1).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextOptions {
    /// Font family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<String>,
    /// Size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1.0, max = 512.0))]
    size: Option<f64>,
    /// Bold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bold: Option<bool>,
    /// Italic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    /// Baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baseline: Option<Baseline>,
    /// Kerning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -60, max = 60))]
    kerning: Option<i64>,
    /// Automatic kerning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auto_kern: Option<bool>,
    /// Alignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    align: Option<Align>,
    /// Left margin (du).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left_margin: Option<f64>,
    /// Right margin (du).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    right_margin: Option<f64>,
    /// First-line indent (du).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    indent: Option<f64>,
    /// Extra line spacing (du).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line_spacing: Option<f64>,
    /// Outline colour of new text (`null`: none). Absent leaves it; `null` clears it (ruling D-7e).
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    outline_colour_id: Option<Option<String>>,
}

/// Distinguishes a member given as `null` (`Some(None)`) from an absent one (`None`, by `#[serde(default)]`).
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// Baseline.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Baseline {
    /// Normal.
    Normal,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
}

/// Alignment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Align {
    /// Left.
    Left,
    /// Centre.
    Centre,
    /// Right.
    Right,
    /// Justified.
    Justify,
}

#[command(
    id = "tool.text.options.set",
    kind = ui,
    label = "Text options",
    scopes = [ui],
    needs_project,
    example(title = "New text 48 du bold", params = r#"{"size": 48, "bold": true}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_text_options_set(cx: &mut Cx, p: TextOptions) -> Result<OptionsResult, CommandError> {
    Ok(OptionsResult { options: set_options(cx, "text", &to_map(&p))? })
}

// ----- pens -----------------------------------------------------------------------------------------------------------

/// `tool.pen.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenListParams {
    /// Only pens of this family.
    #[serde(default, rename = "type")]
    ty: Option<Family>,
}

/// A pen family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Family {
    /// Brush pens.
    Brush,
    /// Eraser pens.
    Eraser,
}

impl Family {
    fn kind(self) -> PenKind {
        match self {
            Family::Brush => PenKind::Brush,
            Family::Eraser => PenKind::Eraser,
        }
    }
}

/// One pen.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenEntry {
    /// Pen id.
    pen_id: String,
    /// Name.
    name: String,
    /// Family.
    #[serde(rename = "type")]
    ty: Family,
    /// Tip shape.
    tip_shape: String,
    /// Minimum size (du).
    min_size: f64,
    /// Maximum size (du).
    max_size: f64,
    /// Smoothness 0-10.
    smoothness: u8,
    /// Tip angle (degrees).
    tip_angle: f64,
}

/// `tool.pen.list` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenList {
    /// Pens in project order.
    pens: Vec<PenEntry>,
    /// The current brush pen.
    current_brush_pen_id: Option<String>,
    /// The current eraser pen.
    current_eraser_pen_id: Option<String>,
}

#[command(
    id = "tool.pen.list",
    kind = query,
    label = "List pens",
    scopes = [read],
    needs_project,
    example(title = "Every pen of the project", params = "{}"),
    example(title = "The eraser pens", params = r#"{"type": "eraser"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_list(cx: &mut Cx, p: PenListParams) -> Result<PenList, CommandError> {
    let doc = cx.document()?;
    let pens = doc
        .project()
        .pens()
        .iter()
        .filter(|x| p.ty.is_none_or(|f| f.kind() == *x.kind()))
        .map(|x| PenEntry {
            pen_id: x.id().as_str().to_owned(),
            name: x.name().clone(),
            ty: if *x.kind() == PenKind::Brush { Family::Brush } else { Family::Eraser },
            tip_shape: serde_json::to_value(x.tip())
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            min_size: *x.min_size(),
            max_size: *x.max_size(),
            smoothness: *x.smoothness(),
            tip_angle: *x.tip_angle(),
        })
        .collect();
    Ok(PenList {
        pens,
        current_brush_pen_id: current_pen(cx, PenKind::Brush).map(|i| i.as_str().to_owned()),
        current_eraser_pen_id: current_pen(cx, PenKind::Eraser).map(|i| i.as_str().to_owned()),
    })
}

/// The pens array of the project entry and one pen's index in it.
fn pen_index(cx: &Cx, id: &Id) -> Result<(Value, usize), CommandError> {
    let pj = cx.document()?.entry_raw_json(&EntryKey::Project)?;
    let i = pj["pens"]
        .as_array()
        .and_then(|a| a.iter().position(|p| p["id"].as_str() == Some(id.as_str())))
        .ok_or_else(|| CommandError::internal("pen not in the project entry"))?;
    Ok((pj, i))
}

fn name_taken(cx: &Cx, name: &str, except: Option<&Id>) -> Result<bool, CommandError> {
    Ok(cx.document()?.project().pens().iter().any(|p| p.name() == name && Some(p.id()) != except))
}

fn check_name(cx: &Cx, name: &str, except: Option<&Id>) -> Result<(), CommandError> {
    if name.trim().is_empty() || name.chars().count() > 128 {
        return Err(err(codes::ARG_INVALID, "a pen name is 1-128 characters", json!({ "pointer": "/name" })));
    }
    if name_taken(cx, name, except)? {
        return Err(err(
            "pen.nameExists",
            format!("a pen named `{name}` exists"),
            json!({ "pointer": "/name", "name": name }),
        ));
    }
    Ok(())
}

/// `tool.pen.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenCreateParams {
    /// Family.
    #[serde(default = "brush", rename = "type")]
    ty: Family,
    /// The pen to copy (default: the family's current pen).
    #[serde(default)]
    from_pen_id: Option<String>,
    /// Name (default: the next free "Pen n" or "Eraser n").
    #[serde(default)]
    name: Option<String>,
    /// Settings over the copied ones (`tipShape`, `minSize`, `maxSize`, `smoothness`, `tipAngle`).
    #[serde(default)]
    settings: Option<Map<String, Value>>,
}

fn brush() -> Family {
    Family::Brush
}

/// `{penId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenIdName {
    /// Pen id.
    pen_id: String,
    /// Name.
    name: String,
}

/// Applies pen settings to a pen's JSON with 02 §6.1's rules (ranges; raising the minimum above the maximum pushes the
/// maximum up, lowering the maximum below the minimum pushes the minimum down).
fn apply_settings(pen: &mut Value, given: &Map<String, Value>, pointer: &str) -> Result<(), CommandError> {
    let range = |k: &str, lo: f64, hi: f64| -> Result<Option<f64>, CommandError> {
        match given.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => match v.as_f64().filter(|x| x.is_finite() && (lo..=hi).contains(x)) {
                Some(x) => Ok(Some(x)),
                None => Err(err(
                    "tool.valueOutOfRange",
                    format!("`{k}` is {lo}-{hi}"),
                    json!({ "pointer": format!("{pointer}/{k}") }),
                )),
            },
        }
    };
    for k in given.keys() {
        if !["tipShape", "minSize", "maxSize", "smoothness", "tipAngle"].contains(&k.as_str()) {
            return Err(err(
                codes::ARG_INVALID,
                format!("pens have no setting `{k}`"),
                json!({ "pointer": format!("{pointer}/{k}") }),
            ));
        }
    }
    if let Some(t) = given.get("tipShape").filter(|v| !v.is_null()) {
        if !matches!(t.as_str(), Some("round" | "oval" | "square" | "flat")) {
            return Err(err(
                codes::ARG_INVALID,
                "tipShape is round, oval, square or flat",
                json!({ "pointer": format!("{pointer}/tipShape") }),
            ));
        }
        pen["tip"] = t.clone();
    }
    let min = range("minSize", 0.0, 100.0)?;
    let max = range("maxSize", 0.0, 100.0)?;
    if let Some(s) = range("smoothness", 0.0, 10.0)? {
        pen["smoothness"] = json!(s.round());
    }
    if let Some(a) = range("tipAngle", 0.0, 360.0)? {
        pen["tipAngle"] = json!(a);
    }
    let mut lo = pen["minSize"].as_f64().unwrap_or(0.0);
    let mut hi = pen["maxSize"].as_f64().unwrap_or(0.0);
    if let Some(m) = min {
        lo = m;
        hi = hi.max(m);
    }
    if let Some(m) = max {
        hi = m;
        lo = lo.min(m);
    }
    pen["minSize"] = json!(lo);
    pen["maxSize"] = json!(hi);
    Ok(())
}

#[command(
    id = "tool.pen.create",
    kind = action,
    label = "New pen",
    scopes = [edit],
    undo = "New Pen",
    needs_project,
    example(title = "A new brush pen from the current one", params = "{}"),
    example(title = "A wide eraser", params = r#"{"type": "eraser", "name": "Wide", "settings": {"maxSize": 80, "minSize": 80}}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_create(cx: &mut Cx, p: PenCreateParams) -> Result<PenIdName, CommandError> {
    let kind = p.ty.kind();
    let from = match &p.from_pen_id {
        Some(r) => {
            let (id, k, name) = find_pen(cx, r)?;
            if k != kind {
                return Err(err(
                    "tool.notApplicable",
                    format!("pen `{name}` is not a {} pen", kind_str(kind)),
                    json!({ "pointer": "/fromPenId" }),
                ));
            }
            Some(id)
        }
        None => current_pen(cx, kind),
    };
    let base = if kind == PenKind::Brush { "Pen" } else { "Eraser" };
    let name = match p.name {
        Some(n) => {
            check_name(cx, &n, None)?;
            n
        }
        None => {
            let mut n = 1;
            while name_taken(cx, &format!("{base} {n}"), None)? {
                n += 1;
            }
            format!("{base} {n}")
        }
    };
    let id = cx.new_id(ObjectType::Pen)?;
    let mut pen = match &from {
        Some(f) => {
            let (pj, i) = pen_index(cx, f)?;
            pj["pens"][i].clone()
        }
        None => {
            json!({ "type": "pen", "rev": 0, "kind": kind_str(kind), "minSize": 2.0, "maxSize": 8.0, "smoothness": 3,
                        "tip": "round", "tipAngle": 0, "colourId": null })
        }
    };
    pen["id"] = json!(id.as_str());
    pen["name"] = json!(name);
    pen["kind"] = json!(kind_str(kind));
    if let Some(s) = &p.settings {
        apply_settings(&mut pen, s, "/settings")?;
    }
    cx.patch_entries(vec![(EntryKey::Project, vec![op("add", "/pens/-", Some(pen))])])?;
    emit_pen(cx, id.as_str(), PenChange::Created);
    // A new pen becomes current for its family (02 §6.1).
    match kind {
        PenKind::Brush => cx.app_mut().pens.brush = Some(id.clone()),
        PenKind::Eraser => cx.app_mut().pens.eraser = Some(id.clone()),
    }
    emit_pen(cx, id.as_str(), PenChange::Selected);
    Ok(PenIdName { pen_id: id.as_str().to_owned(), name })
}

/// `tool.pen.delete` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenDeleted {
    /// The removed pen.
    removed_pen_id: String,
    /// The family's current pen now.
    current_pen_id: Option<String>,
}

#[command(
    id = "tool.pen.delete",
    kind = action,
    label = "Delete pen",
    scopes = [edit],
    undo = "Delete Pen",
    needs_project,
    example(title = "Delete Pen 2", params = r#"{"penId": "pen:\"Pen 2\""}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_delete(cx: &mut Cx, p: PenParams) -> Result<PenDeleted, CommandError> {
    let (id, kind, name) = find_pen(cx, &p.pen_id)?;
    let same_kind = cx.document()?.project().pens().iter().filter(|x| *x.kind() == kind).count();
    if same_kind <= 1 {
        return Err(err(
            "pen.lastPen",
            format!("`{name}` is the last {} pen", kind_str(kind)),
            json!({ "penId": id.as_str() }),
        ));
    }
    let (_, i) = pen_index(cx, &id)?;
    cx.patch_entries(vec![(EntryKey::Project, vec![op("remove", &format!("/pens/{i}"), None)])])?;
    let slot = match kind {
        PenKind::Brush => &mut cx.app_mut().pens.brush,
        PenKind::Eraser => &mut cx.app_mut().pens.eraser,
    };
    if slot.as_ref() == Some(&id) {
        *slot = None;
    }
    emit_pen(cx, id.as_str(), PenChange::Deleted);
    Ok(PenDeleted {
        removed_pen_id: id.as_str().to_owned(),
        current_pen_id: current_pen(cx, kind).map(|i| i.as_str().to_owned()),
    })
}

/// `tool.pen.rename` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenRenameParams {
    /// The pen.
    pen_id: String,
    /// The new name.
    name: String,
}

#[command(
    id = "tool.pen.rename",
    kind = action,
    label = "Rename pen",
    scopes = [edit],
    undo = "Rename Pen",
    needs_project,
    example(title = "Rename Pen 1", params = r#"{"penId": "pen:\"Pen 1\"", "name": "Fine"}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_rename(cx: &mut Cx, p: PenRenameParams) -> Result<PenIdName, CommandError> {
    let (id, _, _) = find_pen(cx, &p.pen_id)?;
    check_name(cx, &p.name, Some(&id))?;
    let (_, i) = pen_index(cx, &id)?;
    cx.patch_entries(vec![(EntryKey::Project, vec![op("replace", &format!("/pens/{i}/name"), Some(json!(p.name)))])])?;
    emit_pen(cx, id.as_str(), PenChange::Modified);
    Ok(PenIdName { pen_id: id.as_str().to_owned(), name: p.name })
}

/// `tool.pen.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PenSetParams {
    /// The pen.
    pen_id: String,
    /// Tip shape.
    #[serde(default)]
    tip_shape: Option<String>,
    /// Minimum size (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    min_size: Option<f64>,
    /// Maximum size (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 100.0))]
    max_size: Option<f64>,
    /// Smoothness.
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 10.0))]
    smoothness: Option<f64>,
    /// Tip angle (degrees).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 360.0))]
    tip_angle: Option<f64>,
}

/// `{pen}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PenResult {
    /// The pen now.
    pen: Value,
}

#[command(
    id = "tool.pen.set",
    kind = action,
    label = "Set pen style",
    scopes = [edit],
    undo = "Set Pen Style",
    needs_project,
    example(title = "A wider Pen 2", params = r#"{"penId": "pen:\"Pen 2\"", "maxSize": 6}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_set(cx: &mut Cx, p: PenSetParams) -> Result<PenResult, CommandError> {
    let (id, _, _) = find_pen(cx, &p.pen_id)?;
    let (pj, i) = pen_index(cx, &id)?;
    let mut pen = pj["pens"][i].clone();
    let mut given = Map::new();
    if let Some(t) = p.tip_shape {
        given.insert("tipShape".into(), json!(t));
    }
    for (k, v) in
        [("minSize", p.min_size), ("maxSize", p.max_size), ("smoothness", p.smoothness), ("tipAngle", p.tip_angle)]
    {
        if let Some(x) = v {
            given.insert(k.into(), json!(x));
        }
    }
    apply_settings(&mut pen, &given, "")?;
    if pen != pj["pens"][i] {
        cx.patch_entries(vec![(EntryKey::Project, vec![op("replace", &format!("/pens/{i}"), Some(pen.clone()))])])?;
        emit_pen(cx, id.as_str(), PenChange::Modified);
    }
    Ok(PenResult { pen })
}

/// `{penId, type}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PenSelected {
    /// The pen.
    pen_id: String,
    /// Its family.
    #[serde(rename = "type")]
    ty: Family,
}

#[command(
    id = "tool.pen.select",
    kind = ui,
    label = "Select pen",
    scopes = [ui],
    needs_project,
    example(title = "Make Pen 2 current", params = r#"{"penId": "pen:\"Pen 2\""}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_pen_select(cx: &mut Cx, p: PenParams) -> Result<PenSelected, CommandError> {
    let (id, kind) = select_pen(cx, &p.pen_id, None)?;
    Ok(PenSelected {
        pen_id: id.as_str().to_owned(),
        ty: if kind == PenKind::Brush { Family::Brush } else { Family::Eraser },
    })
}

// ----- global settings ------------------------------------------------------------------------------------------------

/// `{enabled}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// On.
    enabled: bool,
}

fn set_flag(cx: &mut Cx, f: impl FnOnce(&mut crate::drawing::DrawingSettings) -> &mut bool, v: bool) -> Enabled {
    let slot = f(cx.ui_mut().drawing_mut());
    let changed = *slot != v;
    *slot = v;
    if changed {
        emit_options(cx, None);
    }
    Enabled { enabled: v }
}

#[command(
    id = "tool.placeBehind.set",
    kind = toggle,
    label = "Draw Behind",
    scopes = [ui],
    needs_project,
    example(title = "Draw behind the existing art", params = r#"{"enabled": true}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_place_behind_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    Ok(set_flag(cx, |d| &mut d.place_behind, p.enabled))
}

#[command(
    id = "tool.drawTopLayer.set",
    kind = toggle,
    label = "Draw on Top Layer",
    scopes = [ui],
    needs_project,
    example(title = "Draw on the top layer", params = r#"{"enabled": true}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_draw_top_layer_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    Ok(set_flag(cx, |d| &mut d.draw_top_layer, p.enabled))
}

#[command(
    id = "tool.snapToContour.set",
    kind = toggle,
    label = "Snap to Contour",
    scopes = [ui],
    needs_project,
    example(title = "Snap the Contour Editor to contours", params = r#"{"enabled": true}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_snap_to_contour_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    Ok(set_flag(cx, |d| &mut d.snap_to_contour, p.enabled))
}

/// `tool.tablet.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TabletParams {
    /// The tablet acts as a mouse.
    #[serde(default)]
    ignore_tablet: Option<bool>,
    /// Portrait-mode correction.
    #[serde(default)]
    portrait_correction: Option<bool>,
    /// Compatibility handling.
    #[serde(default)]
    compatibility_mode: Option<bool>,
    /// The input interface.
    #[serde(default)]
    input_interface: Option<String>,
}

/// `{settings, restartRequired}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabletResult {
    /// The tablet settings now.
    settings: crate::drawing::TabletSetting,
    /// A change applies after a restart (the input interface).
    restart_required: bool,
}

#[command(
    id = "tool.tablet.set",
    kind = ui,
    label = "Tablet input settings",
    scopes = [ui],
    needs_project,
    example(title = "Ignore tablet input", params = r#"{"ignoreTablet": true}"#),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_tablet_set(cx: &mut Cx, p: TabletParams) -> Result<TabletResult, CommandError> {
    if p.input_interface.as_ref().is_some_and(|s| s.is_empty() || s.len() > 64) {
        return Err(err(
            codes::ARG_INVALID,
            "an input interface name is 1-64 characters",
            json!({ "pointer": "/inputInterface" }),
        ));
    }
    let before = cx.ui().drawing().tablet.clone();
    let t = &mut cx.ui_mut().drawing_mut().tablet;
    if let Some(v) = p.ignore_tablet {
        t.ignore_tablet = v;
    }
    if let Some(v) = p.portrait_correction {
        t.portrait_correction = v;
    }
    if let Some(v) = p.compatibility_mode {
        t.compatibility_mode = v;
    }
    if p.input_interface.is_some() {
        t.input_interface = p.input_interface;
    }
    let after = t.clone();
    let restart_required = after.input_interface != before.input_interface;
    if after != before {
        emit_options(cx, None);
    }
    Ok(TabletResult { settings: after, restart_required })
}

fn settings_map(cx: &Cx) -> Map<String, Value> {
    let d = cx.ui().drawing();
    let gap = json!(super::colour::GapMode::of(cx.pref(super::colour::GAP_PREF).as_ref()));
    let v = json!({
        "placeBehind": d.place_behind,
        "drawTopLayer": d.draw_top_layer,
        "snapToContour": d.snap_to_contour,
        "symmetry": d.symmetry,
        "tablet": d.tablet,
        "gapClosing": gap,
    });
    v.as_object().cloned().unwrap_or_default()
}

#[command(
    id = "tool.settings.get",
    kind = query,
    label = "Get drawing-tool settings",
    scopes = [read],
    needs_project,
    example(title = "Every global drawing-tool setting", params = "{}"),
    gui = "the Tools toolbar and tool properties (02, 05); wired in M4 C4.2",
)]
pub(crate) fn tool_settings_get(cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Ok(settings_map(cx))
}
