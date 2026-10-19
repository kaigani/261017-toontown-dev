//! `agent.*` (11 §15.1): agent-server and session controls, declared with typed parameters; implemented with the MCP
//! server in C1.4. Commands with the `app` scope cannot be granted to an agent session (11 §6.1).

#![allow(
    dead_code,
    reason = "declared commands: parameters are decoded and validated now and read by the handler of the owning checkpoint"
)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tt_registry::{CommandError, command};

use super::common::{Empty, NoParams, event_not_invocable};
use crate::cx::Cx;

const C14: &str = "M1 C1.4";

/// Policy presets (11 §6.1).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Policy {
    /// Read only.
    Observe,
    /// Asks before consequential actions.
    Assist,
    /// Acts within its scopes.
    Autonomous,
}

/// `agent.server.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ServerSetParams {
    /// Enable the server.
    enabled: bool,
    /// Streamable HTTP transport.
    #[serde(default)]
    http: Option<bool>,
    /// Port.
    #[serde(default)]
    #[schemars(range(min = 1024, max = 65535))]
    port: Option<u16>,
    /// Accept non-loopback connections.
    #[serde(default)]
    allow_remote: Option<bool>,
}

/// `agent.server.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSetResult {
    /// Enabled.
    enabled: bool,
    /// Endpoint URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    /// Discovery file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_file: Option<String>,
}

#[command(
    id = "agent.server.set",
    kind = ui,
    label = "Agent server",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Enable the local agent server", params = r#"{"enabled": true}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_server_set(_cx: &mut Cx, _p: ServerSetParams) -> Result<ServerSetResult, CommandError> {
    Err(CommandError::not_implemented("agent.server.set", C14))
}

#[command(
    id = "agent.server.status",
    kind = query,
    label = "Agent server status",
    scopes = [read],
    not_implemented = "M1 C1.4",
    example(title = "Endpoint and connected sessions", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_server_status(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("agent.server.status", C14))
}

/// `agent.session.grant` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GrantParams {
    /// Session.
    session_id: String,
    /// Policy preset.
    #[serde(default)]
    policy: Option<Policy>,
    /// Scopes.
    #[serde(default)]
    scopes: Option<Vec<String>>,
    /// File-system roots.
    #[serde(default)]
    roots: Option<Vec<String>>,
}

#[command(
    id = "agent.session.grant",
    kind = ui,
    label = "Grant agent access",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Let a session edit", params = r#"{"sessionId": "a1", "policy": "assist", "scopes": ["read", "edit"]}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_session_grant(_cx: &mut Cx, _p: GrantParams) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("agent.session.grant", C14))
}

/// `{sessionId}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SessionParams {
    /// Session.
    session_id: String,
}

#[command(
    id = "agent.session.disconnect",
    kind = ui,
    label = "Disconnect agent",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Disconnect a session", params = r#"{"sessionId": "a1"}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_session_disconnect(_cx: &mut Cx, _p: SessionParams) -> Result<Empty, CommandError> {
    Err(CommandError::not_implemented("agent.session.disconnect", C14))
}

/// `{sessionId?}` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OptionalSession {
    /// Session (default: all).
    #[serde(default)]
    session_id: Option<String>,
}

/// `{paused}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Paused {
    /// Paused sessions.
    paused: Vec<String>,
}

#[command(
    id = "agent.session.pause",
    kind = ui,
    label = "Pause agents",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Pause every session", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_session_pause(_cx: &mut Cx, _p: OptionalSession) -> Result<Paused, CommandError> {
    Err(CommandError::not_implemented("agent.session.pause", C14))
}

/// `{resumed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Resumed {
    /// Resumed sessions.
    resumed: Vec<String>,
}

#[command(
    id = "agent.session.resume",
    kind = ui,
    label = "Resume agents",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Resume every session", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_session_resume(_cx: &mut Cx, _p: OptionalSession) -> Result<Resumed, CommandError> {
    Err(CommandError::not_implemented("agent.session.resume", C14))
}

/// `agent.policy.defaults.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PolicyDefaultsParams {
    /// Default policy.
    #[serde(default)]
    policy: Option<Policy>,
    /// Concurrent jobs per session.
    #[serde(default)]
    #[schemars(range(min = 1, max = 8))]
    max_jobs: Option<u32>,
    /// Steps per batch.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    max_batch_steps: Option<u32>,
    /// Largest inline image side.
    #[serde(default)]
    #[schemars(range(min = 256, max = 8192))]
    max_inline_image_px: Option<u32>,
    /// Largest inline result.
    #[serde(default)]
    #[schemars(range(min = 1, max = 1_073_741_824))]
    max_inline_bytes: Option<u64>,
    /// Ask in the app.
    #[serde(default)]
    ask_in_app: Option<bool>,
    /// Let agents move the view.
    #[serde(default)]
    let_agent_move_view: Option<bool>,
    /// Highlight agent changes.
    #[serde(default)]
    highlight_agent_changes: Option<bool>,
}

#[command(
    id = "agent.policy.defaults.set",
    kind = ui,
    label = "Agent policy defaults",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Assist by default", params = r#"{"policy": "assist", "maxJobs": 2}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_policy_defaults_set(
    _cx: &mut Cx,
    _p: PolicyDefaultsParams,
) -> Result<Map<String, Value>, CommandError> {
    Err(CommandError::not_implemented("agent.policy.defaults.set", C14))
}

/// `{discoveryFile}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Discovery {
    /// Discovery file path.
    discovery_file: String,
}

#[command(
    id = "agent.token.regenerate",
    kind = ui,
    label = "Regenerate agent token",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Invalidate the bearer token", params = "{}"),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_token_regenerate(_cx: &mut Cx, _p: NoParams) -> Result<Discovery, CommandError> {
    Err(CommandError::not_implemented("agent.token.regenerate", C14))
}

/// `agent.typedTools.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypedToolsParams {
    /// Expose one MCP tool per command.
    enabled: bool,
    /// Limit to namespaces.
    #[serde(default)]
    namespaces: Option<Vec<String>>,
}

/// `{toolCount}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCount {
    /// Tools exposed.
    tool_count: u32,
}

#[command(
    id = "agent.typedTools.set",
    kind = ui,
    label = "Typed agent tools",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Expose doc and edit commands as tools", params = r#"{"enabled": true, "namespaces": ["doc", "edit"]}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_typed_tools_set(_cx: &mut Cx, _p: TypedToolsParams) -> Result<ToolCount, CommandError> {
    Err(CommandError::not_implemented("agent.typedTools.set", C14))
}

/// `agent.confirmation.respond` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ConfirmParams {
    /// Confirmation.
    confirmation_id: String,
    /// Accept or decline.
    accept: bool,
}

/// `{confirmationId, accepted}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Confirmed {
    /// Confirmation.
    confirmation_id: String,
    /// Accepted.
    accepted: bool,
}

#[command(
    id = "agent.confirmation.respond",
    kind = ui,
    label = "Answer agent request",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Accept a request", params = r#"{"confirmationId": "cf_1", "accept": true}"#),
    gui = "Agent confirmation sheet (M4)",
)]
pub(crate) fn agent_confirmation_respond(_cx: &mut Cx, _p: ConfirmParams) -> Result<Confirmed, CommandError> {
    Err(CommandError::not_implemented("agent.confirmation.respond", C14))
}

/// `agent.activity.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ActivityListParams {
    /// Session.
    #[serde(default)]
    session_id: Option<String>,
    /// Since (ISO 8601).
    #[serde(default)]
    since: Option<String>,
    /// Maximum entries.
    #[serde(default)]
    #[schemars(range(min = 1, max = 10_000))]
    limit: Option<u32>,
}

#[command(
    id = "agent.activity.list",
    kind = query,
    label = "Agent activity",
    scopes = [read],
    not_implemented = "M1 C1.4",
    example(title = "The last 20 actions", params = r#"{"limit": 20}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_activity_list(
    _cx: &mut Cx,
    _p: ActivityListParams,
) -> Result<Vec<Map<String, Value>>, CommandError> {
    Err(CommandError::not_implemented("agent.activity.list", C14))
}

/// Export format.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ActivityFormat {
    /// JSON lines.
    #[default]
    Jsonl,
    /// A replayable batch.
    Batch,
}

/// Export conflict policy.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExportOnExists {
    /// Fail.
    #[default]
    Fail,
    /// Overwrite.
    Overwrite,
    /// Pick a new name.
    Rename,
}

/// `agent.activity.export` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ActivityExportParams {
    /// Target path.
    path: String,
    /// Format.
    #[serde(default)]
    format: ActivityFormat,
    /// Conflict policy.
    #[serde(default)]
    on_exists: ExportOnExists,
}

/// `{path, entries}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Exported {
    /// File written.
    path: String,
    /// Entries.
    entries: u32,
}

#[command(
    id = "agent.activity.export",
    kind = action,
    label = "Export agent activity",
    scopes = [read, files_write],
    not_implemented = "M1 C1.4",
    example(title = "Export as a replayable batch", params = r#"{"path": "/tmp/activity.json", "format": "batch"}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_activity_export(_cx: &mut Cx, _p: ActivityExportParams) -> Result<Exported, CommandError> {
    Err(CommandError::not_implemented("agent.activity.export", C14))
}

/// `{enabled}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// Persist the activity log.
    enabled: bool,
}

#[command(
    id = "agent.activity.persist.set",
    kind = toggle,
    label = "Keep agent activity log",
    scopes = [app],
    not_implemented = "M1 C1.4",
    example(title = "Persist the activity log", params = r#"{"enabled": true}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_activity_persist_set(_cx: &mut Cx, _p: Enabled) -> Result<Enabled, CommandError> {
    Err(CommandError::not_implemented("agent.activity.persist.set", C14))
}

#[command(
    id = "agent.session.connected",
    kind = event,
    label = "Agent connected",
    scopes = [read],
    not_implemented = "M1 C1.4",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn agent_session_connected(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("agent.session.connected"))
}

#[command(
    id = "agent.session.disconnected",
    kind = event,
    label = "Agent disconnected",
    scopes = [read],
    not_implemented = "M1 C1.4",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn agent_session_disconnected(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("agent.session.disconnected"))
}

#[command(
    id = "agent.confirmation.requested",
    kind = event,
    label = "Agent confirmation requested",
    scopes = [read],
    not_implemented = "M1 C1.4",
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn agent_confirmation_requested(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("agent.confirmation.requested"))
}
