//! Requests and the command result envelope (11 §7.2–§7.3).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tt_registry::{CommandError, Scope};

/// Who invoked a command (10 §2.6 `origin`; 11 §11 attribution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The GUI user. Holds every scope.
    User,
    /// An agent session (11 §6.1): its id, display name and granted scopes.
    Agent {
        /// Session id.
        session: String,
        /// Client display name (undo labels: "— agent: <name>").
        name: String,
        /// Granted scopes.
        scopes: BTreeSet<Scope>,
    },
    /// Automatic actions (migration, background repair).
    System,
    /// The load path (repairs on open, 10 REP-01).
    Load,
}

impl Origin {
    /// The `origin` string of change sets and events: `user`, `agent:<name>`, `system`, `load`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Origin::User => "user".to_owned(),
            Origin::Agent { name, .. } => format!("agent:{name}"),
            Origin::System => "system".to_owned(),
            Origin::Load => "load".to_owned(),
        }
    }

    /// The key that scopes `requestId` idempotency (11 §7.2: "within the session").
    #[must_use]
    pub fn session_key(&self) -> String {
        match self {
            Origin::Agent { session, .. } => format!("agent:{session}"),
            other => other.label(),
        }
    }

    /// Whether the origin holds a scope.
    #[must_use]
    pub fn has_scope(&self, scope: Scope) -> bool {
        match self {
            Origin::Agent { scopes, .. } => scopes.contains(&scope),
            _ => true,
        }
    }

    /// `"user"` or `"agent"` (the `source` of 01 §12.2 history entries).
    #[must_use]
    pub fn source(&self) -> &'static str {
        match self {
            Origin::Agent { .. } => "agent",
            _ => "user",
        }
    }
}

/// Options of one invocation (11 §7.2 `run_command`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvokeOptions {
    /// Validate, check enablement and permissions, report the changes that would be made, change nothing.
    pub dry_run: bool,
    /// Optimistic concurrency: object ref (id or path) → expected `rev` (REV-05).
    pub expect_rev: BTreeMap<String, u64>,
    /// Idempotency key: a repeated id within the session returns the original result.
    pub request_id: Option<String>,
}

/// One command invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct Invocation {
    /// Command id.
    pub command: String,
    /// Parameters (an object; `null` means `{}`).
    pub params: Value,
    /// Options.
    pub options: InvokeOptions,
    /// Who invokes it.
    pub origin: Origin,
}

impl Invocation {
    /// A user invocation with default options.
    #[must_use]
    pub fn user(command: &str, params: Value) -> Self {
        Self { command: command.to_owned(), params, options: InvokeOptions::default(), origin: Origin::User }
    }
}

/// One step of a batch (11 §7.3).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchStep {
    /// Command id.
    pub id: String,
    /// Parameters; string values `$<n>.<path>` / `$<name>.<path>` refer to earlier step envelopes.
    #[serde(default)]
    pub params: Value,
    /// Name for later references (`$<name>.…`).
    #[serde(default, rename = "as")]
    pub name: Option<String>,
}

/// A batch request (11 §7.3).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BatchRequest {
    /// Undo label of the batch (one step when atomic).
    pub label: String,
    /// The steps.
    pub steps: Vec<BatchStep>,
    /// One transaction and one undo step; any failure rolls everything back.
    #[serde(default = "yes")]
    pub atomic: bool,
    /// Stop at the first failing step (non-atomic batches).
    #[serde(default = "yes")]
    pub stop_on_error: bool,
    /// Report what would change; change nothing.
    #[serde(default)]
    pub dry_run: bool,
}

fn yes() -> bool {
    true
}

/// `status` of an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The command ran.
    Ok,
    /// Confirmation is needed (11 §6.3; the protocol arrives with the MCP server, C1.4).
    NeedsConfirmation,
    /// The command failed; `error` says why.
    Error,
}

/// One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRecord {
    /// Object id (compound for container-local ids).
    pub id: String,
    /// Object type.
    #[serde(rename = "type")]
    pub ty: String,
    /// `created`, `modified` or `deleted`.
    pub op: String,
    /// The object's `rev` after the transaction (the `documentRev` it would get for a dry run).
    pub rev: u64,
    /// Parent (created and deleted objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Position in the parent's collection (created objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Own-view RFC 6902 patch with `oldValue` (modified objects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Vec<Value>>,
}

/// The undo step a transaction created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UndoRef {
    /// Step id (`u-<n>`).
    pub step_id: String,
    /// Label.
    pub label: String,
}

/// A warning (11 §6.3, §10): same code space as errors, never aborts.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// Code.
    pub code: String,
    /// Message.
    pub message: String,
    /// Affected objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_ids: Vec<String>,
}

/// The command result envelope (11 §7.2), also the shape of each batch step.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// `ok`, `needs_confirmation` or `error`.
    pub status: Status,
    /// Command-specific result (per its result schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// What was actually targeted (P4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Value>,
    /// Changed objects (capped at [`crate::CHANGES_CAP`]).
    pub changes: Vec<ChangeRecord>,
    /// `true` when `changes` was capped; read the rest with `doc.diff` from `changesSince`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub changes_truncated: bool,
    /// Revision to pass to `doc.diff` when `changes` was capped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes_since: Option<u64>,
    /// The undo step created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<UndoRef>,
    /// Warnings.
    pub warnings: Vec<Warning>,
    /// `documentRev` after the call (`null` with no project).
    pub document_rev: Option<u64>,
    /// The error of a failed call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
    /// `true` for a dry run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
    /// Batch step envelopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Envelope>>,
}

impl Envelope {
    /// A failed call.
    #[must_use]
    pub fn error(error: CommandError, document_rev: Option<u64>) -> Self {
        Self {
            status: Status::Error,
            result: None,
            targets: None,
            changes: Vec::new(),
            changes_truncated: false,
            changes_since: None,
            undo: None,
            warnings: Vec::new(),
            document_rev,
            error: Some(error),
            dry_run: false,
            steps: None,
        }
    }

    /// A successful call without changes.
    #[must_use]
    pub fn ok(result: Value, document_rev: Option<u64>) -> Self {
        Self {
            status: Status::Ok,
            result: Some(result),
            error: None,
            ..Self::error(CommandError::new("", ""), document_rev)
        }
    }

    /// Whether the call succeeded.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status == Status::Ok
    }
}
