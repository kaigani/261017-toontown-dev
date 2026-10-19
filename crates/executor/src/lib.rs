//! # tt-executor — serialized command executor
//!
//! The single document thread and transaction queue through which **every** document mutation flows, from the GUI
//! and from agents alike (11 P2): validate → enablement → permission → execute as a transaction → record the undo
//! step → emit change events. Undo/redo, copy-on-write checkpoints, atomic batches, `dryRun`, `expectRev`,
//! `requestId` idempotency and a never-block watchdog.
//!
//! - [`ExecutorHandle`] is the public face: [`ExecutorHandle::spawn`] starts the document thread; `invoke`,
//!   `batch`, `open_document`, `snapshot`, `subscribe`, `list_commands` and `describe_command` talk to it over a
//!   channel. Every call answers within the watchdog budget (`command.timeout` otherwise) and never waits for user
//!   input (11 P5). `job.wait` waits on the caller's thread, never on the document thread.
//! - [`Cx`] is the exclusive mutation capability: only the executor creates one, handlers receive `&mut Cx`, and
//!   staged changes become visible only when the executor commits them (see `cx.rs`; compile-fail tests in
//!   `tests/ui`).
//! - Commands are declared next to their handlers with `#[tt_registry::command]` (`commands/*`); the registry holds
//!   them together with the stubs of the full spec A inventory.
//!
//! Spec sections: 11 §2 (executor), §5.1 (special references), §7.1–§7.3, §7.7; 10 §2.5–§2.6 (REV-01…06, change
//! sets), AG-01…AG-09; 01 §5 (window, panels, workspaces: kernel state), §6.1 (undo/redo), §12.4. Milestone: M1
//! (C1.2). Decisions: ADR-0013.

#![allow(
    clippy::result_large_err,
    reason = "CommandError is the typed wire error {code, message, details, hint} every command returns (11 §10); boxing it would only move the allocation"
)]

mod align;
mod app;
pub mod commands;
mod cx;
mod history;
mod jobs;
mod rules;
mod run;
mod session;
mod time;
mod types;
mod ui;

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tt_model::document::Document;
use tt_registry::{AssemblyError, CommandError, Kind, Registry, Status as DeclStatus, codes};

pub use app::{AppState, Mode};
pub use cx::Cx;
pub use history::MAX_STEPS as MAX_UNDO_STEPS;
pub use jobs::{JobProgress, JobRecord, JobState, JobTable};
pub use session::Event;
pub use types::{
    BatchRequest, BatchStep, ChangeRecord, Envelope, Invocation, InvokeOptions, Origin, Status, UndoRef, Warning,
};
pub use ui::{Layout, PanelEntry, WorkspaceEntry};

/// Crate name as linked into the engine (the headless self-test lists it to prove the crate is present).
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &[
    "11 §2 (executor)",
    "11 §5.1 (special references)",
    "11 §7.1–§7.3",
    "11 §7.7",
    "10 §2.5–§2.6 (REV-01…06, change sets)",
    "10 §10.1 (AG-01…AG-09)",
    "01 §5 (window/workspace state)",
    "01 §6.1 (undo/redo)",
];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.2)";

/// Maximum change records in an envelope (11 §7.2); the rest is read with `doc.diff` from `changesSince`.
pub const CHANGES_CAP: usize = 500;

/// Maximum steps of one batch (11 §6.5 `maxBatchSteps` default).
pub const MAX_BATCH_STEPS: usize = 1000;

/// Default never-block budget for one call (11 §14.6; ADR-0013 §3).
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(30);

/// The assembled registry: every spec A entry, typed declarations attached.
///
/// # Errors
/// [`AssemblyError`] if a declaration does not match spec A (a build defect, covered by tests).
pub fn registry() -> Result<&'static Registry, AssemblyError> {
    static REGISTRY: OnceLock<Result<Registry, AssemblyError>> = OnceLock::new();
    REGISTRY.get_or_init(|| Registry::assemble(commands::ALL)).as_ref().map_err(Clone::clone)
}

enum Request {
    Invoke(Invocation, Sender<Envelope>),
    Batch(BatchRequest, Origin, Sender<Envelope>),
    Open(Box<Document>, Option<String>, bool, Sender<Envelope>),
    Snapshot(Sender<Option<Document>>),
    List(Origin, Sender<Vec<Value>>),
}

/// Executor configuration.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Never-block budget per call.
    pub budget: Duration,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self { budget: DEFAULT_BUDGET }
    }
}

/// A handle to the document thread. Cloning shares the thread; the thread ends when the last handle is dropped.
#[derive(Debug, Clone)]
pub struct ExecutorHandle {
    tx: Sender<Request>,
    jobs: Arc<JobTable>,
    subscribers: Arc<Mutex<Vec<Sender<Event>>>>,
    config: ExecutorConfig,
}

fn timeout(what: &str, budget: Duration) -> CommandError {
    CommandError::with_details(
        codes::COMMAND_TIMEOUT,
        format!("`{what}` did not answer within {} ms", budget.as_millis()),
        json!({ "commandId": what, "budgetMs": u64::try_from(budget.as_millis()).unwrap_or(u64::MAX) }),
    )
    .hint("the document thread is busy; the call may still complete — read doc.state.get before retrying")
}

fn stopped() -> CommandError {
    CommandError::internal("the document thread has stopped")
}

impl ExecutorHandle {
    /// Starts the document thread.
    ///
    /// # Errors
    /// `internal` if the registry does not assemble or the thread cannot start.
    pub fn spawn(config: ExecutorConfig) -> Result<Self, CommandError> {
        let registry = registry().map_err(|e| CommandError::internal(format!("registry assembly failed: {}", e.0)))?;
        let (tx, rx) = mpsc::channel::<Request>();
        let jobs = Arc::new(JobTable::default());
        let subscribers: Arc<Mutex<Vec<Sender<Event>>>> = Arc::new(Mutex::new(Vec::new()));
        let session = session::Session { jobs: jobs.clone(), ..session::Session::default() };
        let subs = subscribers.clone();
        std::thread::Builder::new()
            .name("tt-document".to_owned())
            .spawn(move || document_thread(run::Core::new(registry, session), &rx, &subs))
            .map_err(|e| CommandError::internal(format!("cannot start the document thread: {e}")))?;
        Ok(Self { tx, jobs, subscribers, config })
    }

    fn call<T>(&self, what: &str, make: impl FnOnce(Sender<T>) -> Request) -> Result<T, CommandError> {
        let (reply, rx) = mpsc::channel();
        self.tx.send(make(reply)).map_err(|_| stopped())?;
        match rx.recv_timeout(self.config.budget) {
            Ok(v) => Ok(v),
            Err(RecvTimeoutError::Timeout) => Err(timeout(what, self.config.budget)),
            Err(RecvTimeoutError::Disconnected) => Err(stopped()),
        }
    }

    /// Runs one command (11 §7.2 `run_command`).
    #[must_use]
    pub fn invoke(&self, inv: Invocation) -> Envelope {
        if inv.command == "job.wait" {
            return self.job_wait(&inv);
        }
        let what = inv.command.clone();
        self.call(&what, |r| Request::Invoke(inv, r)).unwrap_or_else(|e| Envelope::error(e, None))
    }

    /// Runs a batch (11 §7.3).
    #[must_use]
    pub fn batch(&self, req: BatchRequest, origin: Origin) -> Envelope {
        self.call("batch", |r| Request::Batch(req, origin, r)).unwrap_or_else(|e| Envelope::error(e, None))
    }

    /// Makes a document the open project (the load path; C1.3's `project.open` reads packages into it).
    #[must_use]
    pub fn open_document(&self, doc: Document, path: Option<String>, read_only: bool) -> Envelope {
        self.call("project.open", |r| Request::Open(Box::new(doc), path, read_only, r))
            .unwrap_or_else(|e| Envelope::error(e, None))
    }

    /// An immutable snapshot of the live document (copy-on-write: cheap).
    ///
    /// # Errors
    /// `command.timeout`, `internal`.
    pub fn snapshot(&self) -> Result<Option<Document>, CommandError> {
        self.call("snapshot", Request::Snapshot)
    }

    /// Subscribes to events (11 §9). Events are delivered after each call that produced them.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.lock().unwrap_or_else(PoisonError::into_inner).push(tx);
        rx
    }

    /// `list_commands` (11 §7.1): every command with `enabled` evaluated for the given origin.
    ///
    /// # Errors
    /// `command.timeout`, `internal`.
    pub fn list_commands(&self, origin: Origin) -> Result<Vec<Value>, CommandError> {
        self.call("list_commands", |r| Request::List(origin, r))
    }

    /// `describe_command` (11 §7.1).
    #[must_use]
    pub fn describe_command(&self, id: &str) -> Option<Value> {
        registry().ok().and_then(|r| r.get(id)).map(tt_registry::Entry::describe)
    }

    /// The job table (job commands submit work to it).
    #[must_use]
    pub fn jobs(&self) -> &Arc<JobTable> {
        &self.jobs
    }

    /// `job.wait` on the caller's thread (11 P5: the document thread never waits).
    fn job_wait(&self, inv: &Invocation) -> Envelope {
        let check = || -> Result<Value, CommandError> {
            let entry = registry()
                .map_err(|e| CommandError::internal(e.0))?
                .get("job.wait")
                .ok_or_else(|| CommandError::internal("job.wait missing"))?;
            let missing: Vec<&str> =
                entry.scopes().into_iter().filter(|s| !inv.origin.has_scope(*s)).map(|s| s.as_str()).collect();
            if !missing.is_empty() {
                return Err(CommandError::with_details(
                    codes::PERMISSION_DENIED,
                    "`job.wait` needs the read scope",
                    json!({ "commandId": "job.wait", "missingScopes": missing }),
                ));
            }
            let params = if inv.params.is_null() { json!({}) } else { inv.params.clone() };
            tt_registry::validate_params("job.wait", &entry.params_schema(), &params)?;
            let p: commands::job::WaitParams = tt_registry::decode_params(params)?;
            let (record, timed_out) = self.jobs.wait(&p.job_id, Duration::from_millis(p.timeout_ms))?;
            commands::job::wait_result(record, timed_out)
        };
        match check() {
            Ok(v) => Envelope::ok(v, None),
            Err(e) => Envelope::error(e, None),
        }
    }
}

fn document_thread(mut core: run::Core, rx: &Receiver<Request>, subscribers: &Mutex<Vec<Sender<Event>>>) {
    while let Ok(req) = rx.recv() {
        match req {
            Request::Invoke(inv, reply) => {
                let env = core.invoke(&inv);
                let _ = reply.send(env);
            }
            Request::Batch(b, origin, reply) => {
                let env = core.batch(&b, &origin);
                let _ = reply.send(env);
            }
            Request::Open(doc, path, ro, reply) => {
                let env = core.open(*doc, path, ro);
                let _ = reply.send(env);
            }
            Request::Snapshot(reply) => {
                let doc = core.session().and_then(|s| s.project.as_ref()).map(|p| p.doc.clone());
                let _ = reply.send(doc);
            }
            Request::List(origin, reply) => {
                let _ = reply.send(list(&mut core, origin));
            }
        }
        let events = core.take_events();
        if !events.is_empty() {
            let mut subs = subscribers.lock().unwrap_or_else(PoisonError::into_inner);
            subs.retain(|s| events.iter().all(|e| s.send(e.clone()).is_ok()));
        }
    }
}

/// `list_commands` on the document thread: enablement is evaluated against the current context.
fn list(core: &mut run::Core, origin: Origin) -> Vec<Value> {
    let registry = core.registry();
    let Some(session) = core.session_mut().map(std::mem::take) else { return Vec::new() };
    let cx = Cx::new(session, origin.clone(), "list_commands", true);
    let mut out = Vec::with_capacity(registry.len());
    for e in registry.entries() {
        let missing: Vec<&str> = e.scopes().into_iter().filter(|s| !origin.has_scope(*s)).map(|s| s.as_str()).collect();
        let disabled: Option<String> = if e.spec.kind == Kind::Event {
            Some("events are delivered to subscribers, not invoked".to_owned())
        } else if let DeclStatus::NotImplemented { owner } = e.status() {
            Some(format!("implemented in {owner}"))
        } else if !missing.is_empty() {
            Some(format!("needs the scopes {missing:?}"))
        } else if let Some(c) = e.command {
            if c.decl.needs_project && !cx.has_project() {
                Some("no project is open".to_owned())
            } else {
                c.decl.enablement.check.and_then(|check| check(&cx).err()).map(|d| d.reason)
            }
        } else {
            None
        };
        let mut v = json!({
            "id": e.id(),
            "kind": e.spec.kind.as_str(),
            "label": e.command.map_or(e.spec.summary, |c| c.decl.label),
            "summary": e.spec.summary,
            "enabled": disabled.is_none(),
            "scopes": e.scopes().iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "undoable": e.command.is_some_and(|c| c.decl.undo.is_some()),
        });
        if let (Some(r), Some(m)) = (disabled, v.as_object_mut()) {
            m.insert("disabledReason".into(), json!(r));
        }
        out.push(v);
    }
    if let Some(s) = core.session_mut() {
        *s = cx.into_session();
    }
    out
}
