//! # tt-registry — command registry
//!
//! Declarations of every command and event of the application (11 §2, §8; spec A): id, kind, label, parameter and
//! result JSON Schemas, enablement rule with its reason, undo label, permission scopes, confirmation conditions and
//! examples, plus the registry version reported to agents (11 §4).
//!
//! Two sources make up the registry:
//!
//! - **Typed declarations.** Commands that belong to M1 (`app`, `project`, `edit`, `doc`, `agent`, `inspect`,
//!   `checkpoint`, `job`, and the kernel-level `window`/`workspace` state commands) are declared next to their
//!   implementation with the [`command`] attribute macro: the parameter and result types give the schemas, and the
//!   attribute gives the rest. A declaration whose implementation belongs to a later checkpoint names it
//!   ([`Status::NotImplemented`]) and fails with the typed `command.notImplemented` error after validating its
//!   parameters.
//! - **Generated declarations.** Every other spec A entry ([`declared::DECLARED`], generated from spec A's columns by
//!   `scripts/gen-command-declarations.py`; C12-R1-12): parameter and result schemas, scopes, undo label,
//!   confirmation conditions, `needsProject`, an example, the GUI location or agent-only justification, and the
//!   owning milestone/checkpoint of plan §4 ([`owner_of`]). The executor validates the parameters and answers
//!   `command.notImplemented` naming the owner.
//! - **The inventory.** Every entry of spec A ([`inventory::INVENTORY`], generated from the spec and kept equal to it
//!   by a test); declarations attach to it.
//!
//! The executor (`tt-executor`) assembles both into a [`Registry`] and is the only component that runs handlers.
//!
//! Spec sections implemented: 11 §2 (registry), §7.1–§7.2 (discovery), §8 (kinds, validation), §14.1–§14.2
//! (parity, schemas, examples); spec A. Milestone: M1 (C1.2). Decisions: ADR-0013.

pub mod declared;
pub mod inventory;
mod owners;

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

pub use owners::owner_of;
pub use tt_registry_macros::command;

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] =
    &["11 §2 (registry)", "11 §7.1–§7.2", "11 §8 (kinds)", "11 §14.1–§14.2", "A (command index)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.2)";

/// Command-registry version (semver; minor = additive, major = breaking; 11 §4). Major 0 until the command surface
/// is frozen for version 1 (ADR-0013); every checkpoint that adds commands raises the minor version.
pub const REGISTRY_VERSION: &str = "0.1.0";

/// Commands of one section that perform an operation another section's command owns (ADR-0013 §10). Both ids stay
/// registered with their own parameters: the first defaults its target from the current context (02's drawing
/// commands act on the current drawing) and runs the second, whose semantics — the ones the schema's `x-command`
/// names — apply. Not aliases: their parameter shapes differ.
pub const DELEGATES: &[(&str, &str, &str)] = &[
    (
        "draw.drawing.rename",
        "element.drawing.rename",
        "drawingId defaults to the current drawing; runs element.drawing.rename {drawingId, name}",
    ),
    (
        "draw.lock.set",
        "element.drawingLock.set",
        "drawingId defaults to the current drawing; runs element.drawingLock.set {drawingIds: [drawingId], locked}",
    ),
];

/// Command kind (11 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Undoable document change.
    Action,
    /// Boolean state set explicitly.
    Toggle,
    /// Read-only.
    Query,
    /// Long-running; returns `{jobId}` immediately.
    Job,
    /// UI or view state only; not undoable, does not mark the document dirty.
    Ui,
    /// Documented notification; not callable.
    Event,
}

impl Kind {
    /// The spec spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Kind::Action => "action",
            Kind::Toggle => "toggle",
            Kind::Query => "query",
            Kind::Job => "job",
            Kind::Ui => "ui",
            Kind::Event => "event",
        }
    }
}

/// Permission scope (11 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
pub enum Scope {
    /// Queries, resources, renders, inspection.
    #[serde(rename = "read")]
    Read,
    /// Undoable document changes.
    #[serde(rename = "edit")]
    Edit,
    /// UI state: views, panels, workspaces, current frame/element, tool, playback.
    #[serde(rename = "ui")]
    Ui,
    /// Raw input simulation.
    #[serde(rename = "input")]
    Input,
    /// Reading files.
    #[serde(rename = "files.read")]
    FilesRead,
    /// Writing files.
    #[serde(rename = "files.write")]
    FilesWrite,
    /// Preferences and key bindings.
    #[serde(rename = "prefs")]
    Prefs,
    /// The user-wide library.
    #[serde(rename = "library.global")]
    LibraryGlobal,
    /// Outward network actions.
    #[serde(rename = "network")]
    Network,
    /// Physical printing.
    #[serde(rename = "print")]
    Print,
    /// Quit, discard, agent-server settings (never grantable to an agent by itself).
    #[serde(rename = "app")]
    App,
}

impl Scope {
    /// Every scope.
    pub const ALL: [Scope; 11] = [
        Scope::Read,
        Scope::Edit,
        Scope::Ui,
        Scope::Input,
        Scope::FilesRead,
        Scope::FilesWrite,
        Scope::Prefs,
        Scope::LibraryGlobal,
        Scope::Network,
        Scope::Print,
        Scope::App,
    ];

    /// The spec spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Edit => "edit",
            Scope::Ui => "ui",
            Scope::Input => "input",
            Scope::FilesRead => "files.read",
            Scope::FilesWrite => "files.write",
            Scope::Prefs => "prefs",
            Scope::LibraryGlobal => "library.global",
            Scope::Network => "network",
            Scope::Print => "print",
            Scope::App => "app",
        }
    }

    /// Parses the spec spelling.
    #[must_use]
    pub fn parse(s: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|x| x.as_str() == s)
    }
}

/// One entry of spec A, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryEntry {
    /// Command id.
    pub id: &'static str,
    /// Kind.
    pub kind: Kind,
    /// Namespace.
    pub namespace: &'static str,
    /// Owning section (`01` … `11`).
    pub section: &'static str,
    /// Summary column.
    pub summary: &'static str,
    /// Parameters column.
    pub params: &'static str,
    /// Result column.
    pub result: &'static str,
    /// Undo column.
    pub undo: &'static str,
}

/// A typed command error in the `{code, message, details, hint?}` wire form (11 §10).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// Stable dotted code `area.reason`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).
    pub details: Box<Value>,
    /// How to fix the call, where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Error codes used by the registry and the executor (11 §10 generic codes plus the kernel's own).
pub mod codes {
    /// No such command id.
    pub const COMMAND_UNKNOWN: &str = "command.unknown";
    /// Not enabled in the current context.
    pub const COMMAND_DISABLED: &str = "command.disabled";
    /// Registered, implemented by a later milestone (ADR-0013).
    pub const COMMAND_NOT_IMPLEMENTED: &str = "command.notImplemented";
    /// The never-block watchdog's budget was exceeded (ADR-0013).
    pub const COMMAND_TIMEOUT: &str = "command.timeout";
    /// Missing parameter.
    pub const ARG_MISSING: &str = "arg.missing";
    /// Unknown parameter.
    pub const ARG_UNKNOWN: &str = "arg.unknown";
    /// Invalid parameter.
    pub const ARG_INVALID: &str = "arg.invalid";
    /// `expectRev` mismatch.
    pub const REV_CONFLICT: &str = "rev.conflict";
    /// The command needs an open project.
    pub const PROJECT_NONE: &str = "project.none";
    /// Missing scope.
    pub const PERMISSION_DENIED: &str = "permission.denied";
    /// Jobs inside an atomic batch.
    pub const BATCH_JOB_NOT_ALLOWED: &str = "batch.jobNotAllowed";
    /// A batch step failed.
    pub const BATCH_STEP_FAILED: &str = "batch.stepFailed";
    /// `undo ifTop` refused.
    pub const UNDO_NOT_TOP: &str = "undo.notTop";
    /// Nothing to undo (01 §12.1).
    pub const NOTHING_TO_UNDO: &str = "edit.nothingToUndo";
    /// Nothing to redo (01 §12.1).
    pub const NOTHING_TO_REDO: &str = "edit.nothingToRedo";
    /// Session limit.
    pub const LIMIT_EXCEEDED: &str = "limit.exceeded";
    /// Unexpected failure.
    pub const INTERNAL: &str = "internal";
}

impl CommandError {
    /// An error with empty details.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), details: Box::new(json!({})), hint: None }
    }

    /// An error with details.
    pub fn with_details(code: impl Into<String>, message: impl Into<String>, details: Value) -> Self {
        Self { code: code.into(), message: message.into(), details: Box::new(details), hint: None }
    }

    /// Adds a hint.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// `command.notImplemented` naming the owning milestone.
    pub fn not_implemented(id: &str, owner: &str) -> Self {
        Self::with_details(
            codes::COMMAND_NOT_IMPLEMENTED,
            format!("`{id}` is registered but implemented in {owner}"),
            json!({ "commandId": id, "milestone": owner }),
        )
    }

    /// `internal`.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(codes::INTERNAL, message)
    }
}

impl From<tt_model::ModelError> for CommandError {
    fn from(e: tt_model::ModelError) -> Self {
        Self::with_details(e.code(), e.message().to_owned(), e.details().clone())
    }
}

/// Why a command is disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disabled {
    /// `command.disabled` or `project.none`.
    pub code: &'static str,
    /// The human-readable reason (11 §8).
    pub reason: String,
}

impl Disabled {
    /// `command.disabled` with a reason.
    pub fn because(reason: impl Into<String>) -> Self {
        Self { code: codes::COMMAND_DISABLED, reason: reason.into() }
    }

    /// `project.none` (01 §1.3).
    #[must_use]
    pub fn no_project() -> Self {
        Self { code: codes::PROJECT_NONE, reason: "no project is open".to_owned() }
    }

    /// The error a disabled command fails with.
    #[must_use]
    pub fn to_error(&self, id: &str) -> CommandError {
        CommandError::with_details(
            self.code,
            format!("`{id}` is not enabled: {}", self.reason),
            json!({ "commandId": id, "reason": self.reason }),
        )
    }
}

/// The check of an enablement rule: the executor's context as `&dyn Any` → enabled or why not.
pub type EnablementCheck = fn(&dyn Any) -> Result<(), Disabled>;

/// An enablement rule (11 §2, §8): the text shown to agents and the check. The check receives the executor's
/// context as `&dyn Any` (the registry does not depend on the executor).
#[derive(Clone, Copy)]
pub struct Enablement {
    /// Human-readable rule.
    pub rule: &'static str,
    /// The check; `None` = always enabled.
    pub check: Option<EnablementCheck>,
}

impl std::fmt::Debug for Enablement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enablement").field("rule", &self.rule).finish_non_exhaustive()
    }
}

/// An example invocation (11 §14.2): it validates against the parameter schema and runs on the fixture project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example {
    /// What it shows.
    pub title: &'static str,
    /// Parameters as JSON text.
    pub params: &'static str,
}

/// Implementation status of a declared command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Implemented; the handler runs.
    Implemented,
    /// Declared with schemas and examples; the implementation belongs to the named milestone/checkpoint.
    NotImplemented {
        /// Owning milestone, e.g. `"M1 C1.3"`.
        owner: &'static str,
    },
}

/// A handler: decodes its parameters, runs, encodes its result. The context is the executor's invocation context,
/// passed as `&mut dyn Any`.
pub type Handler = fn(&mut dyn Any, Value) -> Result<Value, CommandError>;

/// A typed command declaration (from the [`command`] attribute).
#[derive(Debug, Clone, Copy)]
pub struct Decl {
    /// Command id.
    pub id: &'static str,
    /// Kind.
    pub kind: Kind,
    /// Human label (menu text / undo label base).
    pub label: &'static str,
    /// Scopes a session needs.
    pub scopes: &'static [Scope],
    /// Undo label of an undoable command.
    pub undo: Option<&'static str>,
    /// Conditions under which the command asks for confirmation (11 §6.3).
    pub confirm: &'static [&'static str],
    /// Enablement rule.
    pub enablement: Enablement,
    /// Whether the command needs an open project (01 §1.3: `project.none` otherwise).
    pub needs_project: bool,
    /// Examples.
    pub examples: &'static [Example],
    /// Parameter schema.
    pub params_schema: fn() -> Value,
    /// Result schema.
    pub result_schema: fn() -> Value,
    /// Implementation status.
    pub status: Status,
    /// `Some(justification)` when no GUI location invokes the command (11 §14.1).
    pub agent_only: Option<&'static str>,
    /// GUI locations (panels, context menus, toolbars) that invoke the command besides the menu manifest.
    pub gui: Option<&'static str>,
}

/// A declared command with its handler.
#[derive(Debug, Clone, Copy)]
pub struct Command {
    /// The declaration.
    pub decl: Decl,
    /// The handler.
    pub handler: Handler,
}

/// A registry entry: the spec entry and, for typed commands, the declaration.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Spec A entry.
    pub spec: &'static InventoryEntry,
    /// Typed declaration, if any.
    pub command: Option<&'static Command>,
}

impl Entry {
    /// The command id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        self.spec.id
    }

    /// The implementation status: typed declarations carry theirs; inventory stubs belong to [`owner_of`].
    #[must_use]
    pub fn status(&self) -> Status {
        match self.command {
            Some(c) => c.decl.status,
            None => Status::NotImplemented { owner: owner_of(self.spec.id) },
        }
    }

    /// The scopes needed: declared, or derived from the kind for stubs (`query` → `read`, `ui`/`toggle` → `ui`,
    /// `action`/`job` → `edit`).
    #[must_use]
    pub fn scopes(&self) -> Vec<Scope> {
        match self.command {
            Some(c) => c.decl.scopes.to_vec(),
            None => match self.spec.kind {
                Kind::Query | Kind::Event => vec![Scope::Read],
                Kind::Ui | Kind::Toggle => vec![Scope::Ui],
                Kind::Action | Kind::Job => vec![Scope::Edit],
            },
        }
    }

    /// The parameter schema: declared, or "any object" for stubs (their parameters are specified in the owning
    /// section and typed when implemented).
    #[must_use]
    pub fn params_schema(&self) -> Value {
        match self.command {
            Some(c) => (c.decl.params_schema)(),
            None => json!({ "type": "object", "description": format!("Parameters per spec A: {}", self.spec.params) }),
        }
    }

    /// The result schema.
    #[must_use]
    pub fn result_schema(&self) -> Value {
        match self.command {
            Some(c) => (c.decl.result_schema)(),
            None => json!({ "description": format!("Result per spec A: {}", self.spec.result) }),
        }
    }

    /// The full declaration as returned by `describe_command` (11 §7.1).
    #[must_use]
    pub fn describe(&self) -> Value {
        let (status, owner) = match self.status() {
            Status::Implemented => ("implemented", None),
            Status::NotImplemented { owner } => ("notImplemented", Some(owner)),
        };
        let mut v = json!({
            "id": self.spec.id,
            "kind": self.spec.kind.as_str(),
            "label": self.command.map_or(self.spec.summary, |c| c.decl.label),
            "summary": self.spec.summary,
            "section": self.spec.section,
            "scopes": self.scopes().iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "undoable": self.command.is_some_and(|c| c.decl.undo.is_some()),
            "undoLabel": self.command.and_then(|c| c.decl.undo),
            "confirmation": self.command.map_or(&[][..], |c| c.decl.confirm),
            "enablement": self.command.map_or("see the owning section", |c| c.decl.enablement.rule),
            "needsProject": self.command.is_some_and(|c| c.decl.needs_project),
            "paramsSchema": self.params_schema(),
            "resultSchema": self.result_schema(),
            "examples": self.command.map_or(Vec::new(), |c| {
                c.decl
                    .examples
                    .iter()
                    .map(|e| {
                        let mut ex = json!({ "title": e.title, "params": parse_example(e.params) });
                        if let (Some(r), Some(m)) = (example_result(self.spec.id, e.title), ex.as_object_mut()) {
                            for (from, to) in [("result", "result"), ("resultDigest", "resultDigest"), ("varies", "resultVaries")] {
                                if let Some(v) = r.get(from) {
                                    m.insert(to.into(), v.clone());
                                }
                            }
                        }
                        ex
                    })
                    .collect()
            }),
            "status": status,
            "agentOnly": self.command.and_then(|c| c.decl.agent_only),
            "guiLocations": self.command.and_then(|c| c.decl.gui),
            "spec": { "params": self.spec.params, "result": self.spec.result, "undo": self.spec.undo },
        });
        if let (Some(owner), Some(m)) = (owner, v.as_object_mut()) {
            m.insert("milestone".into(), json!(owner));
        }
        if let (Some((_, to, how)), Some(m)) = (DELEGATES.iter().find(|d| d.0 == self.spec.id), v.as_object_mut()) {
            m.insert("delegatesTo".into(), json!({ "commandId": to, "mapping": how }));
        }
        v
    }
}

/// The executed result of an implemented command's example, retained in `example-results.json` (M2 C2.1 obligation 1;
/// 11 §13): `{title, result | resultDigest, varies}` (a digest `{bytes, sha256}` for results over 32 KiB), where `varies` lists the JSON Pointers whose values differ between executions
/// (generated ids, times, tokens). `crates/executor/tests/c21_example_results.rs` records the file and checks on every
/// OS that each example still reproduces it.
#[must_use]
pub fn example_result(id: &str, title: &str) -> Option<&'static Value> {
    static ALL: OnceLock<Value> = OnceLock::new();
    let all = ALL.get_or_init(|| serde_json::from_str(include_str!("example-results.json")).unwrap_or(Value::Null));
    all.get(id)?.as_array()?.iter().find(|r| r.get("title").and_then(Value::as_str) == Some(title))
}

/// The entries of `declared.json`, parsed once.
fn declared_entries() -> &'static [Value] {
    static ALL: OnceLock<Vec<Value>> = OnceLock::new();
    ALL.get_or_init(|| match serde_json::from_str::<Value>(include_str!("declared.json")) {
        Ok(Value::Array(a)) => a,
        _ => Vec::new(),
    })
}

/// The parameter (`result = false`) or result schema of the `index`-th generated declaration (`declared.json`).
#[must_use]
pub fn declared_schema(index: usize, result: bool) -> Value {
    declared_entries()
        .get(index)
        .and_then(|e| e.get(if result { "result" } else { "params" }))
        .cloned()
        .unwrap_or(Value::Null)
}

/// The stored sample of the `index`-th generated declaration (`part`: `params`, `result` or `jobResult`): an instance
/// that uses every property of the schema, for the negative tests (C12-R2-02). `{"x-unsampleable": reason}` where a
/// required model object cannot be sampled; `Null` when absent.
#[must_use]
pub fn declared_sample(index: usize, part: &str) -> Value {
    declared_entries()
        .get(index)
        .and_then(|e| e.get("samples"))
        .and_then(|s| s.get(part))
        .cloned()
        .unwrap_or(Value::Null)
}

/// The owning sections' examples the `index`-th generated declaration must accept (C12-R3-01): each
/// `{part, type, section, literal, value}`, `type` naming the `$defs` entry the example is of (`null`: the whole part).
#[must_use]
pub fn declared_spec_examples(index: usize) -> Vec<Value> {
    declared_entries()
        .get(index)
        .and_then(|e| e.get("specExamples"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The placeholders of the `index`-th generated declaration: positions whose type or shape the owning section does
/// not define (each family is a spec issue).
#[must_use]
pub fn declared_placeholders(index: usize) -> Vec<Value> {
    declared_entries()
        .get(index)
        .and_then(|e| e.get("placeholders"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The handler of a generated declaration. Never reached: the executor answers `command.notImplemented` for every
/// entry whose status is [`Status::NotImplemented`] after validating its parameters.
///
/// # Errors
/// Always `internal`.
pub fn not_implemented_handler(_cx: &mut dyn Any, _params: Value) -> Result<Value, CommandError> {
    Err(CommandError::internal("a not-implemented command reached its handler"))
}

fn parse_example(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// The assembled registry: every spec A entry, typed declarations attached.
#[derive(Debug)]
pub struct Registry {
    entries: BTreeMap<&'static str, Entry>,
}

/// Problems assembling a registry (a build defect; covered by tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyError(pub String);

impl Registry {
    /// Assembles the registry from the inventory and the typed declarations. Every typed declaration must name a
    /// spec A entry of the same kind, at most once.
    ///
    /// # Errors
    /// [`AssemblyError`] for a declaration that is not in spec A, has another kind, or is declared twice.
    pub fn assemble(commands: &[&'static Command]) -> Result<Registry, AssemblyError> {
        let mut entries: BTreeMap<&'static str, Entry> =
            inventory::INVENTORY.iter().map(|e| (e.id, Entry { spec: e, command: None })).collect();
        if entries.len() != inventory::INVENTORY.len() {
            return Err(AssemblyError("spec A lists an id twice".to_owned()));
        }
        for c in commands {
            let Some(entry) = entries.get_mut(c.decl.id) else {
                return Err(AssemblyError(format!("`{}` is declared but not in spec A", c.decl.id)));
            };
            if entry.spec.kind != c.decl.kind {
                return Err(AssemblyError(format!(
                    "`{}` is declared as {} but spec A says {}",
                    c.decl.id,
                    c.decl.kind.as_str(),
                    entry.spec.kind.as_str()
                )));
            }
            if entry.command.is_some() {
                return Err(AssemblyError(format!("`{}` is declared twice", c.decl.id)));
            }
            entry.command = Some(c);
        }
        Ok(Registry { entries })
    }

    /// An entry by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// Every entry, ordered by id.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty (never, once assembled).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The JSON Schema of a parameter or result type: draft 2020-12, definitions inlined under `$defs`, integer bounds
/// as the model applies them.
#[must_use]
pub fn schema_of<T: JsonSchema>() -> Value {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let schema = generator.into_root_schema_for::<T>();
    let mut v = serde_json::to_value(schema).unwrap_or(Value::Null);
    tt_model::schema::apply_integer_bounds(&mut v);
    v
}

/// Decodes validated parameters into their type.
///
/// # Errors
/// `arg.invalid` when serde rejects a value the schema accepted (a schema/type mismatch, covered by tests).
pub fn decode_params<T: DeserializeOwned>(v: Value) -> Result<T, CommandError> {
    serde_json::from_value(v)
        .map_err(|e| CommandError::with_details(codes::ARG_INVALID, e.to_string(), json!({ "pointer": "" })))
}

/// Encodes a result; NaN and infinities are errors, never `null` (10 §1.1).
///
/// # Errors
/// `internal` if the result cannot be encoded.
pub fn encode_result<T: Serialize>(r: &T) -> Result<Value, CommandError> {
    tt_model::canonical::to_value(r).map_err(|e| CommandError::internal(format!("result encoding failed: {e}")))
}

/// Downcasts the handler context (the executor passes its invocation context).
///
/// # Errors
/// `internal` if a handler is run with a foreign context.
pub fn context<T: 'static>(cx: &mut dyn Any) -> Result<&mut T, CommandError> {
    cx.downcast_mut::<T>().ok_or_else(|| CommandError::internal("handler run with a foreign context"))
}

/// Validates parameters against a command's parameter schema (11 §8): unknown parameters are `arg.unknown`,
/// missing required ones `arg.missing`, everything else `arg.invalid`, each with `details.pointer` and, where the
/// schema states it, `details.allowed`.
///
/// # Errors
/// The first violation, with every violation in `details.violations`.
pub fn validate_params(id: &str, schema: &Value, params: &Value) -> Result<(), CommandError> {
    static CACHE: OnceLock<std::sync::Mutex<BTreeMap<String, std::sync::Arc<jsonschema::Validator>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    let validator = {
        let mut map = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(v) = map.get(id) {
            v.clone()
        } else {
            let v = jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .should_validate_formats(false)
                .build(schema)
                .map_err(|e| CommandError::internal(format!("parameter schema of `{id}` does not compile: {e}")))?;
            let v = std::sync::Arc::new(v);
            map.insert(id.to_owned(), v.clone());
            v
        }
    };
    let mut violations: Vec<(String, String, Value)> = Vec::new();
    for e in validator.iter_errors(params).take(20) {
        let pointer = e.instance_path().to_string();
        let message = e.to_string();
        let (code, pointer, allowed) = classify(&e, pointer, schema);
        violations.push((
            code.to_owned(),
            pointer.clone(),
            json!({ "pointer": pointer, "message": message, "allowed": allowed }),
        ));
    }
    let Some((code, pointer, first)) = violations.first().cloned() else { return Ok(()) };
    let mut details = first.as_object().cloned().unwrap_or_default();
    details.insert(
        "violations".into(),
        Value::Array(
            violations
                .iter()
                .map(|(c, _, v)| {
                    let mut m = v.as_object().cloned().unwrap_or_default();
                    m.insert("code".into(), json!(c));
                    Value::Object(m)
                })
                .collect(),
        ),
    );
    let message = details.get("message").and_then(Value::as_str).unwrap_or("invalid parameters").to_owned();
    let mut err = CommandError::with_details(code, format!("`{id}`: {message}"), Value::Object(details));
    if pointer.is_empty() && err.code == codes::ARG_INVALID {
        err = err.hint("pass the parameters as a JSON object; see describe_command");
    }
    Err(err)
}

/// Validates a value against a schema (draft 2020-12): at most `limit` violations as `{pointer, message}`.
///
/// # Errors
/// `internal` if the schema does not compile.
pub fn schema_violations(schema: &Value, value: &Value, limit: usize) -> Result<Vec<Value>, CommandError> {
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(false)
        .build(schema)
        .map_err(|e| CommandError::internal(format!("schema does not compile: {e}")))?;
    Ok(validator
        .iter_errors(value)
        .take(limit)
        .map(|e| json!({ "pointer": e.instance_path().to_string(), "message": e.to_string() }))
        .collect())
}

fn classify(e: &jsonschema::ValidationError<'_>, pointer: String, schema: &Value) -> (&'static str, String, Value) {
    use jsonschema::error::ValidationErrorKind as K;
    match e.kind() {
        K::Required { property } => {
            let name = property.as_str().unwrap_or_default();
            (codes::ARG_MISSING, format!("{pointer}/{}", tt_model::pointer::escape(name)), Value::Null)
        }
        K::AdditionalProperties { unexpected } => {
            let first = unexpected.first().cloned().unwrap_or_default();
            (codes::ARG_UNKNOWN, format!("{pointer}/{}", tt_model::pointer::escape(&first)), Value::Null)
        }
        _ => {
            let allowed = allowed_at(schema, e.schema_path().as_str());
            (codes::ARG_INVALID, pointer, allowed)
        }
    }
}

/// The constraint keywords of the failing subschema (minimum, maximum, enum, const, type, pattern, lengths).
fn allowed_at(schema: &Value, schema_path: &str) -> Value {
    let Some((parent, _keyword)) = schema_path.rsplit_once('/') else { return Value::Null };
    let Some(sub) = schema.pointer(parent) else { return Value::Null };
    let mut out = Map::new();
    for k in [
        "type",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "enum",
        "const",
        "pattern",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
    ] {
        if let Some(v) = sub.get(k) {
            out.insert(k.to_owned(), v.clone());
        }
    }
    if out.is_empty() { Value::Null } else { Value::Object(out) }
}
