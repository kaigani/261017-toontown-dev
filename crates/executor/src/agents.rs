//! Agent sessions, policies, confirmations and the activity log (11 §6, §11, §15.1; C1.4; ADR-0015).
//!
//! The hub lives on the document thread beside the document, so that `agent.*` commands are ordinary registry
//! commands (the GUI of M4 uses the same ones) and so that the policy, the confirmation tokens and the audit record of a
//! call are decided in the same serialized turn as the call itself. It is **not** part of the rollback state: a call
//! that fails or needs confirmation still leaves its pending confirmation and its activity entry.
//!
//! - **Sessions.** An MCP server registers each client session (`connect`), with a policy preset and scopes. The hub's
//!   grant is authoritative: the executor replaces the scopes an `Origin::Agent` carries with the session's current
//!   grant, so `agent.session.grant` takes effect on the next call. `app` is never granted to an agent (11 §6.1).
//! - **Confirmations** (11 §6.3). A handler that reaches a confirmable condition calls `Cx::confirm`. Under the
//!   session's policy that either passes, or fails the call without side effects with `confirmation.required`; the
//!   executor turns that into the `needs_confirmation` result and records a pending confirmation here: a single-use
//!   token, valid for five minutes, bound to the session, the command, the canonical parameters and the document
//!   revision. Re-invoking with the token passes the check once.
//! - **Activity** (11 §11). One entry per agent call (tool, command, parameters with inline media replaced by a hash,
//!   status, changed-object count, undo step, duration), kept per process and optionally appended beside the project
//!   (`agent-activity.jsonl`).

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, Scope};

/// How long a confirmation token is valid (11 §6.3).
pub const CONFIRMATION_TTL: Duration = Duration::from_secs(300);

/// Policy presets (11 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Policy {
    /// Read only.
    Observe,
    /// All scopes except `network`, `print`, `prefs`, `app`; confirms destructive and overwriting actions.
    #[default]
    Assist,
    /// All scopes except `app`; confirms only `network`/`print` actions and discarding unsaved work.
    Autonomous,
}

impl Policy {
    /// The preset's scopes (11 §6.1 table). `network` and `print` are optional under Autonomous: granted only by
    /// name.
    #[must_use]
    pub fn scopes(self) -> BTreeSet<Scope> {
        use Scope::{Edit, FilesRead, FilesWrite, Input, LibraryGlobal, Prefs, Read, Ui};
        match self {
            Policy::Observe => [Read].into_iter().collect(),
            Policy::Assist => [Read, Edit, Ui, Input, FilesRead, FilesWrite, LibraryGlobal].into_iter().collect(),
            Policy::Autonomous => {
                [Read, Edit, Ui, Input, FilesRead, FilesWrite, Prefs, LibraryGlobal].into_iter().collect()
            }
        }
    }

    /// The policy name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Policy::Observe => "observe",
            Policy::Assist => "assist",
            Policy::Autonomous => "autonomous",
        }
    }

    /// Parses a policy name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "observe" => Some(Policy::Observe),
            "assist" => Some(Policy::Assist),
            "autonomous" => Some(Policy::Autonomous),
            _ => None,
        }
    }

    /// Whether this policy asks before an action of `category` (11 §6.1 presets).
    #[must_use]
    pub fn confirms(self, category: Category) -> bool {
        match category {
            Category::Overwrite | Category::Destructive => self != Policy::Autonomous,
            Category::Discard | Category::Network | Category::Print | Category::ExternalChange => true,
        }
    }
}

/// Confirmable conditions (11 §6.3; 10 RO-02 / SI-045).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Overwriting a file.
    Overwrite,
    /// Deleting content (a scene, used media, unused drawings).
    Destructive,
    /// Discarding unsaved work.
    Discard,
    /// Outward-facing actions (publishing, uploading, fetching).
    Network,
    /// Printing to a physical printer.
    Print,
    /// Saving over a project file changed by another program since it was opened (10 RO-02).
    ExternalChange,
}

impl Category {
    /// The category name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Overwrite => "overwrite",
            Category::Destructive => "destructive",
            Category::Discard => "discard",
            Category::Network => "network",
            Category::Print => "print",
            Category::ExternalChange => "externalChange",
        }
    }

    /// Parses a category name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "overwrite" => Some(Category::Overwrite),
            "destructive" => Some(Category::Destructive),
            "discard" => Some(Category::Discard),
            "network" => Some(Category::Network),
            "print" => Some(Category::Print),
            "externalChange" => Some(Category::ExternalChange),
            _ => None,
        }
    }
}

/// Session limits (11 §6.5) and attached-mode preferences (`agent.policy.defaults.set`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    /// Default policy of new sessions.
    pub policy: Policy,
    /// Concurrent jobs per session.
    pub max_jobs: u32,
    /// Steps per batch.
    pub max_batch_steps: u32,
    /// Long side of an inline image.
    pub max_inline_image_px: u32,
    /// Inline payload accepted / returned.
    pub max_inline_bytes: u64,
    /// Attached mode: ask the user in the app.
    pub ask_in_app: bool,
    /// Attached mode: agent `ui` commands move the user's views.
    pub let_agent_move_view: bool,
    /// Attached mode: highlight agent changes.
    pub highlight_agent_changes: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            policy: Policy::Assist,
            max_jobs: 2,
            max_batch_steps: 1000,
            max_inline_image_px: 2048,
            max_inline_bytes: 50 * 1024 * 1024,
            ask_in_app: true,
            let_agent_move_view: true,
            highlight_agent_changes: true,
        }
    }
}

/// One agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    /// Session id.
    pub session_id: String,
    /// Client display name.
    pub client: String,
    /// Policy preset.
    pub policy: Policy,
    /// Granted scopes.
    pub scopes: BTreeSet<Scope>,
    /// File-system roots granted to the session (besides the project folder).
    pub roots: Vec<String>,
    /// Confirmation categories pre-authorised for `confirm: true` (11 §6.3 rule 3).
    pub preauthorized: BTreeSet<Category>,
    /// Transport (`stdio`, `batch`).
    pub transport: String,
    /// When it connected.
    pub connected_at: String,
    /// Last call.
    pub last_activity: Option<String>,
    /// Paused by the user (11 §6.4).
    pub paused: bool,
}

/// What an MCP server registers.
#[derive(Debug, Clone, Default)]
pub struct AgentConnect {
    /// Client display name.
    pub client: String,
    /// Policy preset (`None`: the default policy).
    pub policy: Option<Policy>,
    /// Scopes (`None`: the preset's). `app` is always removed.
    pub scopes: Option<BTreeSet<Scope>>,
    /// Extra file-system roots.
    pub roots: Vec<std::path::PathBuf>,
    /// Categories pre-authorised for `confirm: true`.
    pub preauthorized: BTreeSet<Category>,
    /// Transport name.
    pub transport: String,
}

/// The state an MCP server polls between calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    /// The session is registered (not disconnected).
    pub connected: bool,
    /// Paused by the user.
    pub paused: bool,
    /// Current grant.
    pub scopes: BTreeSet<Scope>,
    /// Policy preset.
    pub policy: Policy,
    /// Limits.
    pub limits: Limits,
    /// Typed tools: enabled and the namespaces (`[]`: all).
    pub typed_tools: Option<Vec<String>>,
    /// Changes whenever the typed-tool setting changes (the server then sends tools/list_changed).
    pub typed_tools_version: u64,
}

/// A pending confirmation.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub id: String,
    pub session: String,
    pub command: String,
    pub fingerprint: String,
    pub document_rev: Option<u64>,
    pub created: Instant,
    pub summary: String,
    pub consequences: Vec<Value>,
    pub category: Category,
    /// The user's answer in the app (`agent.confirmation.respond`).
    pub decision: Option<bool>,
    /// Extra binding checked again when the token is used (e.g. the project file's identity, SI-045).
    pub binding: Option<Value>,
}

/// One activity-log entry (11 §11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    /// Sequence number (per process).
    pub seq: u64,
    /// When the call finished.
    pub time: String,
    /// Session id.
    pub session_id: String,
    /// Client name.
    pub client: String,
    /// MCP tool (or `batch-script`).
    pub tool: String,
    /// Command id (`batch` for batches).
    pub command_id: String,
    /// Parameters, with inline media replaced by a hash.
    pub params: Value,
    /// `ok`, `needs_confirmation` or `error`.
    pub status: String,
    /// Error code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Undo step.
    pub undo_step_id: Option<String>,
    /// Changed objects.
    pub changed_count: usize,
    /// Document revision after the call.
    pub document_rev: Option<u64>,
    /// Duration.
    pub duration_ms: u64,
}

/// The hub.
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentHub {
    pub sessions: BTreeMap<String, AgentSession>,
    pub limits: Limits,
    pub pending: BTreeMap<String, Pending>,
    pub activity: Vec<ActivityEntry>,
    pub next_seq: u64,
    pub next_session: u64,
    pub persist: bool,
    pub typed_tools: Option<Vec<String>>,
    pub typed_tools_version: u64,
    pub ttl: Option<Duration>,
    pub server_enabled: bool,
}

/// The canonical form of a call's parameters, which a confirmation token is bound to.
#[must_use]
pub fn fingerprint(command: &str, params: &Value) -> String {
    let p = tt_model::canonical::to_canonical_string(params).unwrap_or_else(|_| params.to_string());
    tt_persist::fsx::sha256(format!("{command}\n{p}").as_bytes())
}

/// Replaces inline media in logged parameters by their hash (11 §11): `base64` and `data` members, and any string
/// longer than 4 KiB.
#[must_use]
pub fn redact(v: &Value) -> Value {
    fn walk(v: &Value, key: Option<&str>, depth: usize) -> Value {
        if depth > 64 {
            return Value::String("…".into());
        }
        match v {
            Value::String(s) if matches!(key, Some("base64" | "data")) || s.len() > 4096 => {
                json!(format!("sha256:{}", tt_persist::fsx::sha256(s.as_bytes())))
            }
            Value::Object(m) => {
                Value::Object(m.iter().map(|(k, x)| (k.clone(), walk(x, Some(k), depth + 1))).collect::<Map<_, _>>())
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| walk(x, key, depth + 1)).collect()),
            other => other.clone(),
        }
    }
    walk(v, None, 0)
}

impl AgentHub {
    /// Registers a session.
    pub fn connect(&mut self, c: AgentConnect, now: String) -> AgentSession {
        self.next_session += 1;
        let policy = c.policy.unwrap_or(self.limits.policy);
        let mut scopes = c.scopes.unwrap_or_else(|| policy.scopes());
        scopes.remove(&Scope::App);
        let s = AgentSession {
            session_id: format!("s{}-{}", self.next_session, tt_persist::fsx::random_token(3)),
            client: if c.client.trim().is_empty() { "agent".to_owned() } else { c.client },
            policy,
            scopes,
            roots: c.roots.iter().map(|r| r.to_string_lossy().into_owned()).collect(),
            preauthorized: c.preauthorized,
            transport: c.transport,
            connected_at: now,
            last_activity: None,
            paused: false,
        };
        self.sessions.insert(s.session_id.clone(), s.clone());
        s
    }

    /// The status an MCP server polls.
    pub fn status(&self, session: &str) -> AgentStatus {
        let s = self.sessions.get(session);
        AgentStatus {
            connected: s.is_some(),
            paused: s.is_some_and(|s| s.paused),
            scopes: s.map(|s| s.scopes.clone()).unwrap_or_default(),
            policy: s.map_or(self.limits.policy, |s| s.policy),
            limits: self.limits.clone(),
            typed_tools: self.typed_tools.clone(),
            typed_tools_version: self.typed_tools_version,
        }
    }

    /// The session summary of `agent.server.status`.
    pub fn summary(&self) -> Vec<Value> {
        self.sessions
            .values()
            .map(|s| {
                json!({
                    "sessionId": s.session_id, "client": s.client, "policy": s.policy.as_str(),
                    "scopes": s.scopes.iter().map(|x| scope_name(*x)).collect::<Vec<_>>(),
                    "connectedAt": s.connected_at, "lastActivity": s.last_activity, "paused": s.paused,
                    "transport": s.transport,
                })
            })
            .collect()
    }

    /// The pending confirmations (attached mode shows them; `agent.server.status`).
    pub fn pending_summary(&self) -> Vec<Value> {
        self.pending
            .values()
            .map(|p| {
                json!({
                    "confirmationId": p.id, "sessionId": p.session, "commandId": p.command, "summary": p.summary,
                    "consequences": p.consequences, "category": p.category.as_str(), "decision": p.decision,
                })
            })
            .collect()
    }

    /// Appends an activity entry.
    pub fn record(&mut self, mut e: ActivityEntry) -> ActivityEntry {
        self.next_seq += 1;
        e.seq = self.next_seq;
        if let Some(s) = self.sessions.get_mut(&e.session_id) {
            s.last_activity = Some(e.time.clone());
        }
        self.activity.push(e.clone());
        e
    }

    /// Takes a confirmation token for a call (single use). `Ok(())` lets the call proceed.
    ///
    /// # Errors
    /// `confirmation.expired` (unknown, used, expired, or bound to other parameters, another session or another
    /// document revision), `confirmation.declined` (the user declined it in the app).
    pub fn take(
        &mut self,
        token: &str,
        session: &str,
        command: &str,
        fingerprint: &str,
        document_rev: Option<u64>,
        binding: Option<&Value>,
    ) -> Result<(), CommandError> {
        let ttl = self.ttl.unwrap_or(CONFIRMATION_TTL);
        let expired = |why: &str| {
            CommandError::with_details(
                "confirmation.expired",
                format!("the confirmation token cannot be used: {why}"),
                json!({ "confirmToken": token, "reason": why }),
            )
            .hint("invoke the command again without confirmToken to get a new confirmation")
        };
        let Some(p) = self.pending.remove(token) else { return Err(expired("unknown or already used")) };
        if p.created.elapsed() > ttl {
            return Err(expired("expired"));
        }
        if p.session != session || p.command != command || p.fingerprint != fingerprint {
            return Err(expired("bound to another call"));
        }
        if p.document_rev != document_rev {
            return Err(expired("the document changed since the confirmation was requested"));
        }
        if p.binding.as_ref() != binding {
            return Err(expired("the condition changed since the confirmation was requested"));
        }
        if p.decision == Some(false) {
            return Err(CommandError::with_details(
                "confirmation.declined",
                "the user declined the action",
                json!({ "confirmToken": token }),
            ));
        }
        Ok(())
    }

    /// Drops expired confirmations.
    pub fn prune(&mut self) {
        let ttl = self.ttl.unwrap_or(CONFIRMATION_TTL);
        self.pending.retain(|_, p| p.created.elapsed() <= ttl);
    }
}

/// A scope's name.
#[must_use]
pub fn scope_name(s: Scope) -> String {
    s.as_str().to_owned()
}

/// Parses a scope name.
#[must_use]
pub fn parse_scope(s: &str) -> Option<Scope> {
    Scope::parse(s)
}
