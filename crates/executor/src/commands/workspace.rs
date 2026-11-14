//! `workspace.*` (01 §5.5, §12.1): named window arrangements, kept by the kernel so agents can list, read, select
//! and edit them (readiness bullet 4). Persistence per user is part of the settings work (C1.3); file import and
//! export arrived with the shell (M4 C4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    /// Save the current arrangement into the current workspace first (default: the preference
    /// `general.workspace.autoSave`, whose default is on, 01 §7; preferences arrive in M4).
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
    let auto = cx.pref("general.workspace.autoSave").and_then(|v| v.as_bool()).unwrap_or(true);
    cx.ui_mut().select(&p.name, p.save_current.unwrap_or(auto))?;
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
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExportParams {
    /// Workspace name (empty, unknown or longer than any name: `workspace.notFound`; C41-R1-13, C41-R2-21 audits).
    name: String,
    /// Absolute target path.
    #[schemars(length(max = 4096))]
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

/// The format of a workspace file.
const FORMAT: &str = "toontown-workspace";

#[command(
    id = "workspace.export",
    kind = ui,
    label = "Export workspace",
    scopes = [ui, files_write],
    confirm = ["overwrite"],
    example(title = "Export the Drawing workspace", params = r#"{"name": "Drawing", "path": "/tmp/drawing.ttworkspace", "onExists": "overwrite"}"#),
    gui = "Window › Workspace › Export… (native dialog; M4 C4.1)",
)]
pub(crate) fn workspace_export(cx: &mut Cx, p: ExportParams) -> Result<PathResult, CommandError> {
    let layout = cx.ui().workspace_layout(Some(&p.name))?;
    let path = crate::persistence::path_param(cx, &p.path, "/path")?;
    if path.exists() {
        if matches!(p.on_exists, OnExists::Fail) {
            return Err(CommandError::with_details(
                "fs.exists",
                format!("`{}` exists", p.path),
                json!({ "path": p.path }),
            )
            .hint("pass onExists: \"overwrite\""));
        }
        crate::persistence::confirm_overwrite(cx, &path)?;
    }
    let file = json!({ "format": FORMAT, "formatVersion": "1.0", "name": p.name, "layout": layout });
    let text = tt_model::canonical::to_canonical_string(&file).map_err(|e| CommandError::internal(e.to_string()))?;
    cx.defer(crate::cx::Effect::WriteFile(Box::new(crate::cx::FileWrite::new(path.clone(), text.into_bytes()))));
    Ok(PathResult { path: path.to_string_lossy().into_owned() })
}

/// Import conflict policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
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
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ImportParams {
    /// Absolute source path.
    #[schemars(length(max = 4096))]
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
    example(title = "Import a workspace file", params = r#"{"path": "/tmp/drawing.ttworkspace", "onExists": "rename"}"#),
    gui = "Window › Workspace › Import… (native dialog; M4 C4.1)",
)]
pub(crate) fn workspace_import(cx: &mut Cx, p: ImportParams) -> Result<NameResult, CommandError> {
    let path = crate::persistence::path_param(cx, &p.path, "/path")?;
    let bad = |why: String| {
        CommandError::with_details(
            "workspace.invalidFile",
            format!("`{}` is not a workspace file: {why}", p.path),
            json!({ "path": p.path, "reason": why }),
        )
    };
    let bytes = tt_persist::fsx::read_limited(&path, 4 << 20).map_err(|_| {
        if path.exists() {
            bad("unreadable or larger than 4 MiB".into())
        } else {
            CommandError::with_details("fs.notFound", format!("`{}` does not exist", p.path), json!({ "path": p.path }))
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|_| bad("not UTF-8".into()))?;
    let v: serde_json::Value = tt_model::json_text::parse(&text).map_err(|e| bad(e.to_string()))?;
    if v.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT) {
        return Err(bad(format!("format is not {FORMAT}")));
    }
    let name = v.get("name").and_then(serde_json::Value::as_str).ok_or_else(|| bad("no name".into()))?.to_owned();
    let layout: Layout = v
        .get("layout")
        .cloned()
        .ok_or_else(|| bad("no layout".into()))
        .and_then(|l| serde_json::from_value(l).map_err(|e| bad(e.to_string())))?;
    let name = cx.ui_mut().import(
        &name,
        layout,
        p.on_exists == ImportOnExists::Overwrite,
        p.on_exists == ImportOnExists::Rename,
    )?;
    Ok(NameResult { name })
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
