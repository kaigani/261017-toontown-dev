//! `project.*` (01 §3, §12.1): declared with typed parameters and results; implemented by persistence in C1.3. The
//! executor's load path (`ExecutorHandle::open_document`) is what `project.open` will call.

#![allow(
    dead_code,
    reason = "declared commands: parameters are decoded and validated now and read by the handler of the owning checkpoint"
)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tt_registry::{CommandError, command};

use super::common::{NoParams, OnExists, OnUnsaved, event_not_invocable};
use crate::cx::Cx;

const C13: &str = "M1 C1.3";

fn yes() -> bool {
    true
}

/// `project.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreateParams {
    /// Project name.
    #[schemars(length(min = 1, max = 128))]
    name: String,
    /// Save immediately at this absolute path.
    #[serde(default)]
    path: Option<String>,
    /// Format preset id.
    #[serde(default)]
    preset: Option<String>,
    /// Frame rate.
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 120.0))]
    frame_rate: Option<f64>,
    /// Camera width.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_width: Option<u32>,
    /// Camera height.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_height: Option<u32>,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// `project.create` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateResult {
    /// Project id.
    project_id: String,
    /// Saved path.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// First scene.
    scene_id: String,
    /// Its camera.
    camera_id: String,
    /// Its drawing element.
    element_id: String,
}

#[command(
    id = "project.create",
    kind = action,
    label = "New Project",
    scopes = [edit],
    not_implemented = "M1 C1.3",
    example(title = "A 1920×1080, 24 fps project", params = r#"{"name": "Walk cycle", "frameRate": 24, "cameraWidth": 1920, "cameraHeight": 1080}"#),
)]
pub(crate) fn project_create(_cx: &mut Cx, _p: CreateParams) -> Result<CreateResult, CommandError> {
    Err(CommandError::not_implemented("project.create", C13))
}

/// `project.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OpenParams {
    /// Absolute package path.
    path: String,
    /// Open read-only.
    #[serde(default)]
    read_only: bool,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{jobId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobStarted {
    /// The job.
    job_id: String,
}

#[command(
    id = "project.open",
    kind = job,
    label = "Open…",
    scopes = [edit, files_read],
    not_implemented = "M1 C1.3",
    example(title = "Open a package", params = r#"{"path": "/tmp/Short.animpkg"}"#),
)]
pub(crate) fn project_open(_cx: &mut Cx, _p: OpenParams) -> Result<JobStarted, CommandError> {
    Err(CommandError::not_implemented("project.open", C13))
}

/// `{onUnsaved}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UnsavedParams {
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{closed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ClosedResult {
    /// The project was closed.
    closed: bool,
}

#[command(
    id = "project.close",
    kind = action,
    label = "Close Project",
    scopes = [edit],
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "Close, failing if unsaved", params = "{}"),
)]
pub(crate) fn project_close(_cx: &mut Cx, _p: UnsavedParams) -> Result<ClosedResult, CommandError> {
    Err(CommandError::not_implemented("project.close", C13))
}

/// `project.save` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SaveParams {
    /// Also save the global library.
    #[serde(default = "yes")]
    include_global_library: bool,
    /// Allow writing a newer format version.
    #[serde(default)]
    allow_format_upgrade: bool,
}

#[command(
    id = "project.save",
    kind = job,
    label = "Save",
    scopes = [edit, files_write],
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "Save", params = "{}"),
)]
pub(crate) fn project_save(_cx: &mut Cx, _p: SaveParams) -> Result<JobStarted, CommandError> {
    Err(CommandError::not_implemented("project.save", C13))
}

/// `project.saveAs` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SaveAsParams {
    /// Absolute target path.
    path: String,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
    /// Also save the global library.
    #[serde(default = "yes")]
    include_global_library: bool,
    /// Allow writing a newer format version.
    #[serde(default)]
    allow_format_upgrade: bool,
}

#[command(
    id = "project.saveAs",
    kind = job,
    label = "Save As…",
    scopes = [edit, files_write],
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "Save a copy", params = r#"{"path": "/tmp/Copy.animpkg"}"#),
)]
pub(crate) fn project_save_as(_cx: &mut Cx, _p: SaveAsParams) -> Result<JobStarted, CommandError> {
    Err(CommandError::not_implemented("project.saveAs", C13))
}

#[command(
    id = "project.info.get",
    kind = query,
    label = "Project information",
    scopes = [read],
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "ProjectInfo (01 §12.4)", params = "{}"),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_info_get(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("project.info.get", C13))
}

/// Project properties.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Properties {
    /// Frame rate.
    frame_rate: f64,
    /// Camera width.
    camera_width: u32,
    /// Camera height.
    camera_height: u32,
}

#[command(
    id = "project.properties.get",
    kind = query,
    label = "Project properties",
    scopes = [read],
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "Frame rate and camera size", params = "{}"),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_properties_get(_cx: &mut Cx, _p: NoParams) -> Result<Properties, CommandError> {
    Err(CommandError::not_implemented("project.properties.get", C13))
}

/// `project.properties.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PropertiesSetParams {
    /// Frame rate.
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 120.0))]
    frame_rate: Option<f64>,
    /// Camera width.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_width: Option<u32>,
    /// Camera height.
    #[serde(default)]
    #[schemars(range(min = 16, max = 8192))]
    camera_height: Option<u32>,
    /// Make these the defaults for new projects.
    #[serde(default)]
    set_as_default: bool,
}

#[command(
    id = "project.properties.set",
    kind = action,
    label = "Set project properties",
    scopes = [edit],
    undo = "Change Project Properties",
    needs_project,
    not_implemented = "M1 C1.3",
    example(title = "25 fps", params = r#"{"frameRate": 25}"#),
    gui = "Project › Properties (M4)",
)]
pub(crate) fn project_properties_set(_cx: &mut Cx, _p: PropertiesSetParams) -> Result<Properties, CommandError> {
    Err(CommandError::not_implemented("project.properties.set", C13))
}

#[command(
    id = "project.presets.list",
    kind = query,
    label = "Format presets",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Presets for new projects", params = "{}"),
    gui = "Start screen new-project form (M4)",
)]
pub(crate) fn project_presets_list(_cx: &mut Cx, _p: NoParams) -> Result<Vec<Map<String, Value>>, CommandError> {
    Err(CommandError::not_implemented("project.presets.list", C13))
}

#[command(
    id = "project.recent.list",
    kind = query,
    label = "Recent projects",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Recent projects", params = "{}"),
    gui = "File › Open Recent and the start screen (M4)",
)]
pub(crate) fn project_recent_list(_cx: &mut Cx, _p: NoParams) -> Result<Vec<Map<String, Value>>, CommandError> {
    Err(CommandError::not_implemented("project.recent.list", C13))
}

/// `project.recent.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecentRemoveParams {
    /// One entry.
    #[serde(default)]
    path: Option<String>,
    /// Every entry.
    #[serde(default)]
    all: bool,
}

/// `{remaining}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Remaining {
    /// Entries left.
    remaining: u32,
}

#[command(
    id = "project.recent.remove",
    kind = ui,
    label = "Remove from recent",
    scopes = [ui],
    not_implemented = "M1 C1.3",
    example(title = "Clear the list", params = r#"{"all": true}"#),
    gui = "File › Open Recent › Clear Menu (M4)",
)]
pub(crate) fn project_recent_remove(_cx: &mut Cx, _p: RecentRemoveParams) -> Result<Remaining, CommandError> {
    Err(CommandError::not_implemented("project.recent.remove", C13))
}

#[command(
    id = "project.recovery.list",
    kind = query,
    label = "Recovery data",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Recoverable sessions", params = "{}"),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_list(_cx: &mut Cx, _p: NoParams) -> Result<Vec<Map<String, Value>>, CommandError> {
    Err(CommandError::not_implemented("project.recovery.list", C13))
}

/// `project.recovery.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryOpenParams {
    /// Recovery id.
    recovery_id: String,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

#[command(
    id = "project.recovery.open",
    kind = job,
    label = "Recover",
    scopes = [edit],
    not_implemented = "M1 C1.3",
    example(title = "Recover a session", params = r#"{"recoveryId": "rc_1"}"#),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_open(_cx: &mut Cx, _p: RecoveryOpenParams) -> Result<JobStarted, CommandError> {
    Err(CommandError::not_implemented("project.recovery.open", C13))
}

/// `project.recovery.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryDeleteParams {
    /// One entry.
    #[serde(default)]
    recovery_id: Option<String>,
    /// Every entry.
    #[serde(default)]
    all: bool,
}

/// `{deleted}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Deleted {
    /// Entries deleted.
    deleted: u32,
}

#[command(
    id = "project.recovery.delete",
    kind = ui,
    label = "Discard recovery data",
    scopes = [ui],
    not_implemented = "M1 C1.3",
    example(title = "Discard everything", params = r#"{"all": true}"#),
    gui = "Recovery dialog at launch (M4)",
)]
pub(crate) fn project_recovery_delete(_cx: &mut Cx, _p: RecoveryDeleteParams) -> Result<Deleted, CommandError> {
    Err(CommandError::not_implemented("project.recovery.delete", C13))
}

/// `project.backup.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupListParams {
    /// Project path (default: the current project).
    #[serde(default)]
    path: Option<String>,
}

#[command(
    id = "project.backup.list",
    kind = query,
    label = "Backups",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Backups of the current project", params = "{}"),
    gui = "File › Open Backup (M4)",
)]
pub(crate) fn project_backup_list(_cx: &mut Cx, _p: BackupListParams) -> Result<Vec<Map<String, Value>>, CommandError> {
    Err(CommandError::not_implemented("project.backup.list", C13))
}

/// `project.backup.open` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BackupOpenParams {
    /// Backup id.
    backup_id: String,
    /// Unsaved-changes policy.
    #[serde(default)]
    on_unsaved: OnUnsaved,
}

/// `{projectId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectId {
    /// Project id.
    project_id: String,
}

#[command(
    id = "project.backup.open",
    kind = action,
    label = "Open backup",
    scopes = [edit],
    not_implemented = "M1 C1.3",
    example(title = "Open a backup as an untitled copy", params = r#"{"backupId": "bk_1"}"#),
    gui = "File › Open Backup (M4)",
)]
pub(crate) fn project_backup_open(_cx: &mut Cx, _p: BackupOpenParams) -> Result<ProjectId, CommandError> {
    Err(CommandError::not_implemented("project.backup.open", C13))
}

#[command(
    id = "project.autosaved",
    kind = event,
    label = "Autosaved",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_autosaved(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("project.autosaved"))
}

#[command(
    id = "project.closed",
    kind = event,
    label = "Project closed",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_closed(_cx: &mut Cx, _p: NoParams) -> Result<ProjectId, CommandError> {
    Err(event_not_invocable("project.closed"))
}

/// `project.dirty.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirtyChanged {
    /// Project modified.
    project_dirty: bool,
    /// Global library modified.
    global_library_dirty: bool,
}

#[command(
    id = "project.dirty.changed",
    kind = event,
    label = "Modified state changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_dirty_changed(_cx: &mut Cx, _p: NoParams) -> Result<DirtyChanged, CommandError> {
    Err(event_not_invocable("project.dirty.changed"))
}

/// `project.opened` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Opened {
    /// Project id.
    project_id: String,
    /// Path.
    path: Option<String>,
    /// Read-only.
    read_only: bool,
}

#[command(
    id = "project.opened",
    kind = event,
    label = "Project opened",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_opened(_cx: &mut Cx, _p: NoParams) -> Result<Opened, CommandError> {
    Err(event_not_invocable("project.opened"))
}

#[command(
    id = "project.saved",
    kind = event,
    label = "Project saved",
    scopes = [read],
    not_implemented = "M1 C1.3",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn project_saved(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("project.saved"))
}
