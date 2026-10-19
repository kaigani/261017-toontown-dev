//! `workspace.*` (01 §5.5, §12.1): named window arrangements, kept by the kernel so agents can list, read, select
//! and edit them (readiness bullet 4). Persistence per user is part of the settings work (C1.3); file import and
//! export follow with the shell (M4 C4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tt_registry::{CommandError, command};

use super::common::{NoParams, OnExists, event_not_invocable};
use crate::cx::Cx;
use crate::ui::{Layout, WorkspaceEntry};

#[command(
    id = "workspace.list",
    kind = query,
    label = "List workspaces",
    scopes = [read],
    example(title = "Built-in and user workspaces in menu order", params = "{}"),
    gui = "Workspace drop-down of the main toolbar (M4)",
)]
pub(crate) fn workspace_list(cx: &mut Cx, _p: NoParams) -> Result<Vec<WorkspaceEntry>, CommandError> {
    Ok(cx.ui().workspaces())
}

/// `{name?}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OptionalName {
    /// Workspace name (default: the current arrangement).
    #[serde(default)]
    name: Option<String>,
}

#[command(
    id = "workspace.get",
    kind = query,
    label = "Get workspace layout",
    scopes = [read],
    example(title = "The current arrangement", params = "{}"),
    example(title = "The stored Drawing workspace", params = r#"{"name": "Drawing"}"#),
    agent_only = "the GUI shows the arrangement itself",
)]
pub(crate) fn workspace_get(cx: &mut Cx, p: OptionalName) -> Result<Layout, CommandError> {
    cx.ui().workspace_layout(p.name.as_deref())
}

/// `{name}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct NameResult {
    /// Workspace name.
    name: String,
}

/// `workspace.select` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectParams {
    /// Workspace name.
    name: String,
    /// Save the current arrangement into the current workspace first (default: `general.workspace.autoSave`,
    /// which is off until preferences arrive in M4).
    #[serde(default)]
    save_current: Option<bool>,
}

#[command(
    id = "workspace.select",
    kind = ui,
    label = "Select workspace",
    scopes = [ui],
    example(title = "Switch to the Drawing workspace", params = r#"{"name": "Drawing"}"#),
    gui = "Windows › Workspace menu and the workspace drop-down (M4)",
)]
pub(crate) fn workspace_select(cx: &mut Cx, p: SelectParams) -> Result<NameResult, CommandError> {
    cx.ui_mut().select(&p.name, p.save_current.unwrap_or(false))?;
    Ok(NameResult { name: p.name })
}

#[command(
    id = "workspace.save",
    kind = ui,
    label = "Save workspace",
    scopes = [ui],
    example(title = "Save the arrangement into the current workspace", params = "{}"),
    gui = "Windows › Workspace › Save Workspace (M4)",
)]
pub(crate) fn workspace_save(cx: &mut Cx, p: OptionalName) -> Result<NameResult, CommandError> {
    let name = cx.ui_mut().save(p.name.as_deref())?;
    Ok(NameResult { name })
}

/// `workspace.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateParams {
    /// New workspace name.
    name: String,
    /// 1-based position in the list (default: last).
    #[serde(default)]
    #[schemars(range(min = 1, max = 1000))]
    position: Option<usize>,
    /// Layout (default: the current arrangement).
    #[serde(default)]
    layout: Option<Layout>,
}

/// `{name, position}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct CreateResult {
    /// Name.
    name: String,
    /// Position.
    position: usize,
}

#[command(
    id = "workspace.create",
    kind = ui,
    label = "New workspace",
    scopes = [ui],
    example(title = "Save the current arrangement as a new workspace", params = r#"{"name": "Compositing"}"#),
    gui = "Windows › Workspace › Workspace Manager (M4)",
)]
pub(crate) fn workspace_create(cx: &mut Cx, p: CreateParams) -> Result<CreateResult, CommandError> {
    let (name, position) = cx.ui_mut().create(&p.name, p.position, p.layout)?;
    Ok(CreateResult { name, position })
}

/// `workspace.rename` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RenameParams {
    /// Current name.
    name: String,
    /// New name.
    new_name: String,
}

#[command(
    id = "workspace.rename",
    kind = ui,
    label = "Rename workspace",
    scopes = [ui],
    example(title = "Rename the Staging workspace", params = r#"{"name": "Staging", "newName": "Layout"}"#),
    gui = "Workspace Manager (M4)",
)]
pub(crate) fn workspace_rename(cx: &mut Cx, p: RenameParams) -> Result<NameResult, CommandError> {
    let name = cx.ui_mut().rename(&p.name, &p.new_name)?;
    Ok(NameResult { name })
}

/// `{name}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NameParams {
    /// Workspace name.
    name: String,
}

/// `{deleted}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct DeleteResult {
    /// Deleted workspace.
    deleted: String,
}

#[command(
    id = "workspace.delete",
    kind = ui,
    label = "Delete workspace",
    scopes = [ui],
    example(title = "Delete a user workspace (built-in ones cannot be deleted)", params = r#"{"name": "Inking"}"#),
    gui = "Workspace Manager (M4)",
)]
pub(crate) fn workspace_delete(cx: &mut Cx, p: NameParams) -> Result<DeleteResult, CommandError> {
    let deleted = cx.ui_mut().delete(&p.name)?;
    Ok(DeleteResult { deleted })
}

/// `workspace.reset` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetParams {
    /// Built-in workspace (default: current).
    #[serde(default)]
    name: Option<String>,
    /// Every built-in workspace.
    #[serde(default)]
    all: bool,
    /// Select it afterwards.
    #[serde(default)]
    select: bool,
}

/// `{reset}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ResetResult {
    /// Reset workspaces.
    reset: Vec<String>,
}

#[command(
    id = "workspace.reset",
    kind = ui,
    label = "Reset workspace",
    scopes = [ui],
    example(title = "Restore and select the Default workspace", params = r#"{"name": "Default", "select": true}"#),
)]
pub(crate) fn workspace_reset(cx: &mut Cx, p: ResetParams) -> Result<ResetResult, CommandError> {
    let reset = cx.ui_mut().reset(p.name.as_deref(), p.all, p.select)?;
    Ok(ResetResult { reset })
}

/// `{layout}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutParams {
    /// The layout to apply (not saved).
    layout: Layout,
}

#[command(
    id = "workspace.layout.set",
    kind = ui,
    label = "Apply layout",
    scopes = [ui],
    example(
        title = "Camera view with the Timeline below",
        params = r#"{"layout": {"centralPanel": "cameraView", "corners": "sides", "docks": {"bottom": [{"tabs": ["timeline"], "current": "timeline", "size": 200}]}, "floating": [], "toolPalettes": [], "mainToolbar": {"visible": true, "items": []}, "fullScreen": {"centralPanel": "cameraView", "corners": "sides", "docks": {}, "floating": [], "toolPalettes": [], "mainToolbar": {"visible": false, "items": []}}}}"#
    ),
    agent_only = "agents apply prepared layouts; users arrange panels by dragging",
)]
pub(crate) fn workspace_layout_set(cx: &mut Cx, p: LayoutParams) -> Result<Layout, CommandError> {
    cx.ui_mut().apply_layout(p.layout)
}

/// `workspace.export` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExportParams {
    /// Workspace name.
    name: String,
    /// Absolute target path.
    path: String,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// `{path}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PathResult {
    /// The file written.
    path: String,
}

#[command(
    id = "workspace.export",
    kind = ui,
    label = "Export workspace",
    scopes = [ui, files_write],
    not_implemented = "M4 C4.1",
    example(title = "Export the Drawing workspace", params = r#"{"name": "Drawing", "path": "/tmp/drawing.ttworkspace"}"#),
    gui = "Workspace Manager (M4)",
)]
pub(crate) fn workspace_export(_cx: &mut Cx, _p: ExportParams) -> Result<PathResult, CommandError> {
    Err(CommandError::not_implemented("workspace.export", "M4 C4.1"))
}

/// Import conflict policy.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ImportOnExists {
    /// Fail.
    #[default]
    Fail,
    /// Overwrite.
    Overwrite,
    /// Rename the imported workspace.
    Rename,
}

/// `workspace.import` parameters.
#[allow(dead_code, reason = "decoded and validated now; read by the handler of the owning checkpoint")]
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ImportParams {
    /// Absolute source path.
    path: String,
    /// Name-conflict policy.
    #[serde(default)]
    on_exists: ImportOnExists,
}

#[command(
    id = "workspace.import",
    kind = ui,
    label = "Import workspace",
    scopes = [ui, files_read],
    not_implemented = "M4 C4.1",
    example(title = "Import a workspace file", params = r#"{"path": "/tmp/drawing.ttworkspace", "onExists": "rename"}"#),
    gui = "Workspace Manager (M4)",
)]
pub(crate) fn workspace_import(_cx: &mut Cx, _p: ImportParams) -> Result<NameResult, CommandError> {
    Err(CommandError::not_implemented("workspace.import", "M4 C4.1"))
}

/// `workspace.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Changed {
    /// Current workspace.
    name: String,
    /// The arrangement differs from the stored one.
    modified: bool,
}

#[command(
    id = "workspace.changed",
    kind = event,
    label = "Workspace changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn workspace_changed(_cx: &mut Cx, _p: NoParams) -> Result<Changed, CommandError> {
    Err(event_not_invocable("workspace.changed"))
}
