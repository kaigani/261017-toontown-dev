//! `agent.*` (11 §15.1): the agent server, sessions, policies, confirmations and the activity log (C1.4; ADR-0015).
//! The state is the executor's agent hub (`crate::agents`); the MCP server registers its sessions there and these
//! commands are how the user (GUI, M4) and agents read and control it. Commands with the `app` scope cannot be granted
//! to an agent session (11 §6.1), so an agent can neither widen its own grant nor answer its own confirmations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_persist::fsx;
use tt_registry::{CommandError, Kind, command};

use super::common::{Empty, NoParams, event_not_invocable, invalid};
use crate::agents::{Category, Policy, parse_scope, scope_name};
use crate::cx::{Cx, Effect};
use crate::persistence;

/// The file of a persisted activity log: `agent-activity.jsonl` beside the project (11 §11).
pub(crate) fn activity_file(project_path: &str) -> Option<PathBuf> {
    Path::new(project_path).parent().map(|d| d.join("agent-activity.jsonl"))
}

/// Appends one line to a file.
pub(crate) fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")
}

/// `name-2.ext`, `name-3.ext`, … : the first name not taken (`onExists: "rename"`).
fn free_name(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem}-{n}{ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

fn session_not_found(id: &str) -> CommandError {
    CommandError::with_details("agent.sessionNotFound", format!("no agent session `{id}`"), json!({ "sessionId": id }))
        .hint("agent.server.status lists the sessions")
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
    example(title = "Enable the local agent server", params = r#"{"enabled": true}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_server_set(cx: &mut Cx, p: ServerSetParams) -> Result<ServerSetResult, CommandError> {
    cx.session_mut().agents.server_enabled = p.enabled;
    let mut prefs = Map::new();
    prefs.insert("agent.server.enabled".into(), json!(p.enabled));
    if let Some(port) = p.port {
        prefs.insert("agent.server.port".into(), json!(port));
    }
    if let Some(remote) = p.allow_remote {
        // Remote binding needs the MCP authorization flow (11 §4); it is stored as asked and applies with HTTP (M4).
        prefs.insert("agent.server.allowRemote".into(), json!(remote));
    }
    cx.defer(Effect::Prefs(prefs));
    if p.http.unwrap_or(true) && p.enabled {
        // Streamable HTTP is the attached-mode transport of the desktop host (11 §4; ADR-0015 §2): M4.
        cx.warn(
            "agent.httpUnavailable",
            "the Streamable HTTP transport comes with the desktop host (attached mode, M4); headless sessions use stdio",
            Vec::new(),
        );
    }
    Ok(ServerSetResult { enabled: p.enabled, endpoint: None, discovery_file: None })
}

#[command(
    id = "agent.server.status",
    kind = query,
    label = "Agent server status",
    scopes = [read],
    example(title = "Endpoint and connected sessions", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_server_status(cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    let hub = &cx.session().agents;
    let sessions = hub.summary();
    let stdio = hub.sessions.values().any(|s| s.transport == "stdio");
    let mut m = Map::new();
    m.insert("enabled".into(), json!(hub.server_enabled || !sessions.is_empty()));
    m.insert("endpoint".into(), if stdio { json!("stdio") } else { Value::Null });
    m.insert("sessions".into(), Value::Array(sessions));
    m.insert("pendingConfirmations".into(), Value::Array(hub.pending_summary()));
    Ok(m)
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
    example(title = "Let a session edit", params = r#"{"sessionId": "a1", "policy": "assist", "scopes": ["read", "edit"]}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_session_grant(cx: &mut Cx, p: GrantParams) -> Result<Map<String, Value>, CommandError> {
    let scopes = match &p.scopes {
        None => None,
        Some(list) => {
            let mut out = std::collections::BTreeSet::new();
            for (i, name) in list.iter().enumerate() {
                match parse_scope(name) {
                    Some(tt_registry::Scope::App) => {
                        return Err(invalid(&format!("/scopes/{i}"), "the scope `app` is never granted to an agent"));
                    }
                    Some(sc) => {
                        out.insert(sc);
                    }
                    None => return Err(invalid(&format!("/scopes/{i}"), format!("`{name}` is not a scope"))),
                }
            }
            Some(out)
        }
    };
    let mut roots = Vec::new();
    for (i, r) in p.roots.iter().flatten().enumerate() {
        roots.push(fsx::absolute(r, &format!("/roots/{i}"))?);
    }
    let s = cx.session_mut();
    let a = s.agents.sessions.get_mut(&p.session_id).ok_or_else(|| session_not_found(&p.session_id))?;
    if let Some(policy) = p.policy {
        a.policy = policy;
        if scopes.is_none() {
            a.scopes = policy.scopes();
        }
    }
    if let Some(sc) = scopes {
        a.scopes = sc;
    }
    if p.roots.is_some() {
        a.roots = roots.iter().map(|r| r.to_string_lossy().into_owned()).collect();
        s.env.roots.insert(p.session_id.clone(), roots);
    }
    let a = s.agents.sessions.get(&p.session_id).cloned().ok_or_else(|| session_not_found(&p.session_id))?;
    let mut m = Map::new();
    m.insert("sessionId".into(), json!(a.session_id));
    m.insert("policy".into(), json!(a.policy.as_str()));
    m.insert("scopes".into(), json!(a.scopes.iter().map(|x| scope_name(*x)).collect::<Vec<_>>()));
    m.insert("roots".into(), json!(a.roots));
    Ok(m)
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
    example(title = "Disconnect a session", params = r#"{"sessionId": "a1"}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_session_disconnect(cx: &mut Cx, p: SessionParams) -> Result<Empty, CommandError> {
    let s = cx.session_mut();
    if s.agents.sessions.remove(&p.session_id).is_none() {
        return Err(session_not_found(&p.session_id));
    }
    s.agents.pending.retain(|_, x| x.session != p.session_id);
    s.env.roots.remove(&p.session_id);
    cx.emit("agent.session.disconnected", json!({ "sessionId": p.session_id, "reason": "user" }));
    Ok(Empty {})
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
    example(title = "Pause every session", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_session_pause(cx: &mut Cx, p: OptionalSession) -> Result<Paused, CommandError> {
    let ids = set_paused(cx, p.session_id.as_deref(), true)?;
    for id in &ids {
        cx.emit("agent.paused", json!({ "by": "user", "sessionId": id }));
    }
    Ok(Paused { paused: ids })
}

/// Pauses or resumes one session or all; returns the sessions changed.
fn set_paused(cx: &mut Cx, session: Option<&str>, paused: bool) -> Result<Vec<String>, CommandError> {
    let hub = &mut cx.session_mut().agents;
    if let Some(id) = session
        && !hub.sessions.contains_key(id)
    {
        return Err(session_not_found(id));
    }
    let mut out = Vec::new();
    for (id, s) in &mut hub.sessions {
        if session.is_none_or(|x| x == id) && s.paused != paused {
            s.paused = paused;
            out.push(id.clone());
        }
    }
    Ok(out)
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
    example(title = "Resume every session", params = "{}"),
    gui = "Agent indicator (M4)",
)]
pub(crate) fn agent_session_resume(cx: &mut Cx, p: OptionalSession) -> Result<Resumed, CommandError> {
    let ids = set_paused(cx, p.session_id.as_deref(), false)?;
    for id in &ids {
        cx.emit("agent.resumed", json!({ "by": "user", "sessionId": id }));
    }
    Ok(Resumed { resumed: ids })
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
    example(title = "Assist by default", params = r#"{"policy": "assist", "maxJobs": 2}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_policy_defaults_set(
    cx: &mut Cx,
    p: PolicyDefaultsParams,
) -> Result<Map<String, Value>, CommandError> {
    let l = &mut cx.session_mut().agents.limits;
    if let Some(v) = p.policy {
        l.policy = v;
    }
    if let Some(v) = p.max_jobs {
        l.max_jobs = v;
    }
    if let Some(v) = p.max_batch_steps {
        l.max_batch_steps = v;
    }
    if let Some(v) = p.max_inline_image_px {
        l.max_inline_image_px = v;
    }
    if let Some(v) = p.max_inline_bytes {
        l.max_inline_bytes = v;
    }
    if let Some(v) = p.ask_in_app {
        l.ask_in_app = v;
    }
    if let Some(v) = p.let_agent_move_view {
        l.let_agent_move_view = v;
    }
    if let Some(v) = p.highlight_agent_changes {
        l.highlight_agent_changes = v;
    }
    let v = serde_json::to_value(&*l).map_err(|e| CommandError::internal(e.to_string()))?;
    let mut prefs = Map::new();
    prefs.insert("agent.policy.defaults".into(), v.clone());
    cx.defer(Effect::Prefs(prefs));
    Ok(v.as_object().cloned().unwrap_or_default())
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
    example(title = "Invalidate the bearer token", params = "{}"),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_token_regenerate(cx: &mut Cx, _p: NoParams) -> Result<Discovery, CommandError> {
    // The bearer token of the Streamable HTTP endpoint (11 §4), kept in the discovery file in the per-user settings
    // folder. HTTP sessions arrive with attached mode (M4); stdio sessions are not affected.
    let dir = cx.session().env.dirs.settings.clone();
    std::fs::create_dir_all(&dir).map_err(|e| CommandError::from(fsx::fs_error(&e, &dir, "cannot create")))?;
    let path = dir.join("agent-discovery.json");
    let text = serde_json::to_vec_pretty(&json!({
        "format": "toontown-agent-discovery", "formatVersion": "1.0",
        "endpoint": Value::Null, "token": fsx::random_token(24), "processId": std::process::id(),
    }))
    .map_err(|e| CommandError::internal(e.to_string()))?;
    if !cx.dry_run() {
        fsx::write_atomic(&path, &text, fsx::fs_error)?;
        restrict_to_user(&path);
    }
    Ok(Discovery { discovery_file: path.to_string_lossy().into_owned() })
}

/// Makes a file readable by its owner only (POSIX); on Windows the per-user settings folder is already private.
fn restrict_to_user(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
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
    example(title = "Expose doc and edit commands as tools", params = r#"{"enabled": true, "namespaces": ["doc", "edit"]}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_typed_tools_set(cx: &mut Cx, p: TypedToolsParams) -> Result<ToolCount, CommandError> {
    let namespaces = p.namespaces.unwrap_or_default();
    let count = typed_tool_count(p.enabled, &namespaces);
    let hub = &mut cx.session_mut().agents;
    hub.typed_tools = p.enabled.then_some(namespaces);
    hub.typed_tools_version += 1;
    Ok(ToolCount { tool_count: count })
}

/// The number of typed tools a setting exposes: every invocable command (not events) of the namespaces (all when
/// empty).
pub(crate) fn typed_tool_count(enabled: bool, namespaces: &[String]) -> u32 {
    if !enabled {
        return 0;
    }
    let Ok(reg) = crate::registry() else { return 0 };
    let n = reg
        .entries()
        .filter(|e| e.spec.kind != tt_registry::Kind::Event)
        .filter(|e| namespaces.is_empty() || namespaces.iter().any(|ns| e.id().split('.').next() == Some(ns.as_str())))
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
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
    example(title = "Accept a request", params = r#"{"confirmationId": "cf_1", "accept": true}"#),
    gui = "Agent confirmation sheet (M4)",
)]
pub(crate) fn agent_confirmation_respond(cx: &mut Cx, p: ConfirmParams) -> Result<Confirmed, CommandError> {
    let hub = &mut cx.session_mut().agents;
    hub.prune();
    let pending = hub.pending.get_mut(&p.confirmation_id).ok_or_else(|| {
        CommandError::with_details(
            "confirmation.expired",
            format!("no pending confirmation `{}`", p.confirmation_id),
            json!({ "confirmationId": p.confirmation_id }),
        )
    })?;
    pending.decision = Some(p.accept);
    Ok(Confirmed { confirmation_id: p.confirmation_id, accepted: p.accept })
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
    example(title = "The last 20 actions", params = r#"{"limit": 20}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_activity_list(cx: &mut Cx, p: ActivityListParams) -> Result<Vec<Map<String, Value>>, CommandError> {
    let limit = usize::try_from(p.limit.unwrap_or(100)).unwrap_or(100);
    let hub = &cx.session().agents;
    let picked: Vec<&crate::agents::ActivityEntry> = hub
        .activity
        .iter()
        .filter(|e| p.session_id.as_ref().is_none_or(|s| *s == e.session_id))
        .filter(|e| p.since.as_ref().is_none_or(|t| e.time.as_str() >= t.as_str()))
        .collect();
    let skip = picked.len().saturating_sub(limit);
    Ok(picked
        .into_iter()
        .skip(skip)
        .filter_map(|e| serde_json::to_value(e).ok().and_then(|v| v.as_object().cloned()))
        .collect())
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

/// `{path, entries, omitted?}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Exported {
    /// File written.
    path: String,
    /// Entries.
    entries: u32,
    /// `format: "batch"`: the entries left out of the replayable batch, with the reason (C14-R1-15).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    omitted: Vec<Omitted>,
}

/// An activity entry a replayable batch leaves out.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Omitted {
    /// The entry's sequence number.
    seq: u64,
    /// Its tool.
    tool: String,
    /// Its command.
    command_id: String,
    /// `notCompleted` (an error or a confirmation that was not given), `dryRun` (it changed nothing), `readOnly` (a
    /// query), `notACommand` (a discovery tool such as `list_commands`), `redacted` (inline data replaced by its hash).
    reason: &'static str,
}

/// Whether a value holds a redacted (`sha256:`) string.
fn redacted(v: &Value) -> bool {
    match v {
        Value::String(s) => s.starts_with("sha256:"),
        Value::Array(a) => a.iter().any(redacted),
        Value::Object(m) => m.values().any(redacted),
        _ => false,
    }
}

/// Renumbers `$<n>.` references of a batch's steps placed at `offset` and renames its step names that are taken.
fn relocate(v: &mut Value, offset: usize, names: &BTreeMap<String, String>) {
    match v {
        Value::String(s) if s.starts_with('$') => {
            let rest = &s[1..];
            let (head, tail) = rest.split_once('.').map_or((rest, ""), |(h, t)| (h, t));
            let dot = if rest.contains('.') { "." } else { "" };
            if let Ok(n) = head.parse::<usize>() {
                *s = format!("${}{dot}{tail}", n + offset);
            } else if let Some(new) = names.get(head) {
                *s = format!("${new}{dot}{tail}");
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| relocate(x, offset, names)),
        Value::Object(m) => m.values_mut().for_each(|x| relocate(x, offset, names)),
        _ => {}
    }
}

/// The replayable batch of an activity log (11 §11, §3): committed edits only, in order. A committed batch contributes
/// its steps (references renumbered); dry runs, queries, discovery tools, incomplete and redacted calls are left out
/// and reported (C14-R1-15).
fn replay_batch(entries: &[crate::agents::ActivityEntry]) -> (Vec<Value>, Vec<Omitted>) {
    let reg = crate::registry().ok();
    let mut steps: Vec<Value> = Vec::new();
    let mut omitted = Vec::new();
    let mut used: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for e in entries {
        let reason = if e.status != "ok" {
            Some("notCompleted")
        } else if e.dry_run {
            Some("dryRun")
        } else if redacted(&e.params) {
            Some("redacted")
        } else if e.command_id == "batch" {
            None
        } else {
            match reg.and_then(|r| r.get(&e.command_id)) {
                None => Some("notACommand"),
                Some(entry) if matches!(entry.spec.kind, Kind::Query | Kind::Event) => Some("readOnly"),
                Some(_) => None,
            }
        };
        if let Some(reason) = reason {
            omitted.push(Omitted { seq: e.seq, tool: e.tool.clone(), command_id: e.command_id.clone(), reason });
            continue;
        }
        if e.command_id != "batch" {
            steps.push(json!({ "id": e.command_id, "params": e.params }));
            continue;
        }
        let batch_steps = e.params.get("steps").and_then(Value::as_array).cloned().unwrap_or_default();
        if e.params.get("dryRun").and_then(Value::as_bool) == Some(true) {
            omitted.push(Omitted {
                seq: e.seq,
                tool: e.tool.clone(),
                command_id: e.command_id.clone(),
                reason: "dryRun",
            });
            continue;
        }
        let offset = steps.len();
        let mut names = BTreeMap::new();
        for s in &batch_steps {
            if let Some(n) = s.get("as").and_then(Value::as_str) {
                let mut new = n.to_owned();
                let mut k = 2;
                while used.contains(&new) {
                    new = format!("{n}_{k}");
                    k += 1;
                }
                used.insert(new.clone());
                names.insert(n.to_owned(), new);
            }
        }
        for mut s in batch_steps {
            if let Some(p) = s.get_mut("params") {
                relocate(p, offset, &names);
            }
            if let Some(m) = s.as_object_mut() {
                let renamed: Option<String> =
                    m.get("as").and_then(Value::as_str).and_then(|n| names.get(n)).map(String::clone);
                if let Some(n) = renamed {
                    m.insert("as".into(), json!(n));
                }
                if m.get("as").is_some_and(Value::is_null) {
                    m.remove("as");
                }
            }
            steps.push(s);
        }
    }
    (steps, omitted)
}

#[command(
    id = "agent.activity.export",
    kind = action,
    label = "Export agent activity",
    scopes = [read, files_write],
    confirm = ["overwrite"],
    example(title = "Export as a replayable batch", params = r#"{"path": "/tmp/activity.json", "format": "batch"}"#),
    gui = "Agent activity panel (M4)",
)]
pub(crate) fn agent_activity_export(cx: &mut Cx, p: ActivityExportParams) -> Result<Exported, CommandError> {
    let mut path = persistence::path_param(cx, &p.path, "/path")?;
    if path.exists() {
        match p.on_exists {
            ExportOnExists::Fail => {
                return Err(CommandError::with_details(
                    tt_persist::codes::FS_EXISTS,
                    format!("`{}` exists", path.display()),
                    json!({ "path": path.to_string_lossy() }),
                )
                .hint("pass onExists: \"overwrite\" or \"rename\""));
            }
            ExportOnExists::Overwrite => cx.confirm(
                Category::Overwrite,
                format!("Overwrite `{}` with the agent activity log", path.display()),
                vec![json!({ "kind": "overwrite", "path": path.to_string_lossy() })],
                None,
            )?,
            ExportOnExists::Rename => path = free_name(&path),
        }
    }
    let entries = cx.session().agents.activity.clone();
    let mut omitted = Vec::new();
    let text = match p.format {
        ActivityFormat::Jsonl => {
            let mut t = String::new();
            for e in &entries {
                t.push_str(&serde_json::to_string(e).map_err(|e| CommandError::internal(e.to_string()))?);
                t.push('\n');
            }
            t
        }
        ActivityFormat::Batch => {
            // A replayable batch (11 §11): the committed edits, in order (sequential, so each step stands alone).
            let (steps, left_out) = replay_batch(&entries);
            omitted = left_out;
            serde_json::to_string_pretty(
                &json!({ "label": "Agent activity", "atomic": false, "stopOnError": true, "steps": steps }),
            )
            .map_err(|e| CommandError::internal(e.to_string()))?
        }
    };
    // Written only when the call commits (C14-R1-03): a later failing step of the same batch leaves no file.
    cx.defer(Effect::WriteFile(Box::new(crate::cx::FileWrite {
        path: path.clone(),
        bytes: text.into_bytes(),
        staged: None,
    })));
    Ok(Exported {
        path: path.to_string_lossy().into_owned(),
        entries: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        omitted,
    })
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
    example(title = "Persist the activity log", params = r#"{"enabled": true}"#),
    gui = "Preferences › Agent access (M4)",
)]
pub(crate) fn agent_activity_persist_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    cx.session_mut().agents.persist = p.enabled;
    Ok(Enabled { enabled: p.enabled })
}

#[command(
    id = "agent.session.connected",
    kind = event,
    label = "Agent connected",
    scopes = [read],
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
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn agent_confirmation_requested(_cx: &mut Cx, _p: NoParams) -> Result<Map<String, Value>, CommandError> {
    Err(event_not_invocable("agent.confirmation.requested"))
}
