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
//! - Persistence (C1.3): `project.*` and the file, media and validation commands of `doc.*` read and write through
//!   `tt-persist` on job threads; a loaded candidate is installed only by the executor, after its integrity gate.
//!   Saves serialise copy-on-write snapshots; background maintenance autosaves, refreshes locks and watches the
//!   project file (`maintenance.rs`). User folders and settings come from [`PersistOptions`].
//!
//! Spec sections: 11 §2 (executor), §5.1 (special references), §7.1–§7.3, §7.7; 10 §2.5–§2.6 (REV-01…06, change
//! sets), AG-01…AG-09, §6–§8 and §10.2–§10.3 through the executor; 01 §3 (projects), §5 (window, panels, workspaces:
//! kernel state), §6.1 (undo/redo), §11 (settings), §12.4. Milestone: M1 (C1.2, C1.3). Decisions: ADR-0013, ADR-0014.

pub mod agents;
mod align;
mod app;
pub mod commands;
mod cx;
mod history;
mod jobs;
mod load_repair;
mod maintenance;
mod persistence;
mod planar;
mod rules;
mod run;
mod session;
mod store;
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
pub use jobs::{JobNotifier, JobProgress, JobRecord, JobState, JobTable};
pub use maintenance::Timings as MaintenanceTimings;
pub use run::LoadGrant;
pub use session::Event;
pub use store::{Maintenance, PersistOptions};
pub use tt_persist::settings::UserDirs;
pub use types::{
    BatchRequest, BatchStep, ChangeRecord, Envelope, Invocation, InvokeOptions, OpenOptions, Origin, Status, UndoRef,
    UnsavedPolicy, Warning,
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
    "10 §6–§8, §10.2–§10.3 (projects and doc.* through the executor)",
    "01 §3 (projects), §11 (settings)",
];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 (C1.2, C1.3)";

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
    REGISTRY
        .get_or_init(|| {
            // Hand-written declarations and the generated ones of every other spec A entry (C12-R1-12).
            let mut all: Vec<&'static tt_registry::Command> = commands::ALL.to_vec();
            all.extend(tt_registry::declared::DECLARED.iter());
            Registry::assemble(&all)
        })
        .as_ref()
        .map_err(Clone::clone)
}

enum Request {
    Invoke(Invocation, Sender<(Envelope, bool)>),
    Batch(BatchRequest, Origin, Sender<Envelope>),
    Open(Box<Document>, OpenOptions, Origin, Sender<Envelope>),
    WaitBegin(Invocation, Sender<run::WaitStart>),
    WaitFinish(Invocation, Result<Value, CommandError>, Sender<Envelope>),
    Snapshot(Sender<Option<Document>>),
    List(Origin, Sender<Vec<Value>>),
    /// Work posted by a job thread (installing a loaded project, finishing a save; C1.3).
    Run(Box<dyn FnOnce(&mut run::Core) + Send>),
    /// A caller's request with its cancellation token, allocated before enqueueing (C23-R2-01), and the link to a caller
    /// that can cancel it (the worker asks it directly at its boundaries; C34-R2-01).
    Tagged(u64, Option<CallerLink>, Box<Request>),
    /// Background maintenance: autosave, heartbeats, external-change detection.
    Tick,
    /// Maintenance now (forcing autosave or not), answered after its events are delivered.
    Maintenance(bool, Sender<maintenance::Timings>),
    /// The last handle was dropped.
    Shutdown,
}

/// Posts work to the document thread (job threads use it to install projects and finish saves).
#[derive(Clone)]
pub(crate) struct Poster(Sender<Request>);

impl std::fmt::Debug for Poster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Poster")
    }
}

impl Poster {
    /// Runs `f` on the document thread and returns its value, waiting at most `timeout`.
    pub(crate) fn run<T: Send + 'static>(
        &self,
        timeout: Duration,
        f: impl FnOnce(&mut run::Core) -> T + Send + 'static,
    ) -> Result<T, CommandError> {
        let (tx, rx) = mpsc::channel();
        self.0
            .send(Request::Run(Box::new(move |core| {
                let _ = tx.send(f(core));
            })))
            .map_err(|_| stopped())?;
        rx.recv_timeout(timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => crate::timeout("document thread", timeout),
            RecvTimeoutError::Disconnected => stopped(),
        })
    }
}

/// Stops the document thread when the last handle is dropped.
struct ShutdownGuard(Sender<Request>, Mutex<Option<Receiver<()>>>);

/// How long dropping the last handle waits for the document thread to release the project (its lock and files).
const SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let _ = self.0.send(Request::Shutdown);
        // Deterministic shutdown: once the last handle is gone, the project's lock and files are released (a caller
        // may remove the folder next). Bounded, so a stuck worker never blocks the dropper for long.
        if let Ok(mut done) = self.1.lock()
            && let Some(rx) = done.take()
        {
            let _ = rx.recv_timeout(SHUTDOWN_WAIT);
        }
    }
}

/// Executor options: the never-block budget and persistence (C1.3).
#[derive(Debug, Clone, Default)]
pub struct ExecutorOptions {
    /// Never-block budget.
    pub config: ExecutorConfig,
    /// Persistence: user folders, maintenance schedule, migrations.
    pub persist: PersistOptions,
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

/// One logical call to record in the activity log ([`ExecutorHandle::audit_logical_call`]).
#[derive(Debug, Clone, Copy)]
pub struct AuditCall<'a> {
    /// The MCP tool the client called.
    pub tool: &'a str,
    /// The command it ran (the tool's name for a server-side tool; `batch` for a batch).
    pub command: &'a str,
    /// The call's parameters (redacted before they are stored).
    pub params: &'a Value,
    /// The final answer.
    pub env: &'a Envelope,
    /// The whole call's duration.
    pub took: Duration,
    /// The answer came from the `requestId` cache: nothing executed (C14-R2-03).
    pub cached: bool,
}

/// A handle to the document thread. Cloning shares the thread; the thread ends when the last handle is dropped.
#[derive(Clone)]
pub struct ExecutorHandle {
    tx: Sender<Request>,
    registry: &'static Registry,
    jobs: Arc<JobTable>,
    subscribers: Arc<Mutex<Vec<Sender<Event>>>>,
    config: ExecutorConfig,
    /// The document thread's watchdog cancellation state (C23-R1-07).
    cancel: Arc<tt_vector::work::CancelState>,
    _guard: Arc<ShutdownGuard>,
}

impl std::fmt::Debug for ExecutorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorHandle").field("config", &self.config).finish_non_exhaustive()
    }
}

fn timeout(what: &str, budget: Duration) -> CommandError {
    CommandError::with_details(
        codes::COMMAND_TIMEOUT,
        format!("`{what}` did not answer within {} ms", budget.as_millis()),
        json!({ "commandId": what, "budgetMs": u64::try_from(budget.as_millis()).unwrap_or(u64::MAX) }),
    )
    .hint("the call was cancelled: unless it had already committed, it changed nothing; read doc.state.get before retrying")
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
        Self::spawn_with_registry(config, registry)
    }

    /// Starts a document thread over another registry (embedding hosts and the never-block tests, which declare a
    /// handler that does not return). The executor's guarantees are the same: handlers still stage only through
    /// their `Cx`, and the load transaction is still guarded.
    ///
    /// # Errors
    /// `internal` if the thread cannot start.
    pub fn spawn_with_registry(config: ExecutorConfig, registry: &'static Registry) -> Result<Self, CommandError> {
        Self::spawn_full(ExecutorOptions { config, persist: PersistOptions::default() }, registry)
    }

    /// Starts the document thread with persistence options (user folders, maintenance; C1.3). Hosts pass the
    /// platform's user folders ([`UserDirs::platform`]); without them the executor uses an isolated temporary folder.
    ///
    /// # Errors
    /// `internal` if the registry does not assemble or the threads cannot start.
    pub fn spawn_with_options(options: ExecutorOptions) -> Result<Self, CommandError> {
        let registry = registry().map_err(|e| CommandError::internal(format!("registry assembly failed: {}", e.0)))?;
        Self::spawn_full(options, registry)
    }

    fn spawn_full(options: ExecutorOptions, registry: &'static Registry) -> Result<Self, CommandError> {
        let ExecutorOptions { config, persist } = options;
        // The system font list (text layout, missingFonts lint) is read once, off the document thread.
        tt_vector::fonts::warm();
        let (tx, rx) = mpsc::channel::<Request>();
        let subscribers: Arc<Mutex<Vec<Sender<Event>>>> = Arc::new(Mutex::new(Vec::new()));
        let job_subs = subscribers.clone();
        let jobs = Arc::new(JobTable::with_notifier(Box::new(move |e| broadcast(&job_subs, std::slice::from_ref(&e)))));
        let mut env = store::Env::new(&persist);
        env.poster = Some(Poster(tx.clone()));
        let mut session = session::Session { jobs: jobs.clone(), env, ..session::Session::default() };
        // The saved agent policy defaults and limits (`agent.policy.defaults.set`; C14-R1-06).
        if let Some(saved) = session.env.pref("agent.policy.defaults")
            && let Ok(limits) = serde_json::from_value::<agents::Limits>(saved)
        {
            session.agents.limits = limits;
        }
        maintenance::start(&mut session);
        let subs = subscribers.clone();
        let budget = config.budget;
        let cancel = Arc::new(tt_vector::work::CancelState::default());
        let worker_cancel = cancel.clone();
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let tick = persist.maintenance.tick;
        std::thread::Builder::new()
            .name("tt-document".to_owned())
            .spawn(move || {
                document_thread(run::Core::new(registry, session, budget), &rx, &subs, &worker_cancel, budget);
                let _ = done_tx.send(());
            })
            .map_err(|e| CommandError::internal(format!("cannot start the document thread: {e}")))?;
        if !tick.is_zero() {
            let ticker = tx.clone();
            std::thread::Builder::new()
                .name("tt-maintenance".to_owned())
                .spawn(move || {
                    loop {
                        std::thread::sleep(tick);
                        if ticker.send(Request::Tick).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|e| CommandError::internal(format!("cannot start the maintenance thread: {e}")))?;
        }
        let guard = Arc::new(ShutdownGuard(tx.clone(), Mutex::new(Some(done_rx))));
        Ok(Self { tx, registry, jobs, subscribers, config, cancel, _guard: guard })
    }

    /// Grants an agent session file-system roots (11 §6.2): paths given by that session must lie inside one of them
    /// or inside the open project's folder (`fs.outsideRoots` otherwise). The MCP host (C1.4) grants the project
    /// folder, `--allow-path` folders and the client's roots.
    ///
    /// # Errors
    /// `command.timeout`, `internal`.
    pub fn grant_roots(&self, session: &str, roots: Vec<std::path::PathBuf>) -> Result<(), CommandError> {
        let session = session.to_owned();
        self.call("grant_roots", |r| {
            Request::Run(Box::new(move |core| {
                if let Some(s) = core.session_mut() {
                    s.env.roots.insert(session, roots);
                }
                let _ = r.send(());
            }))
        })
    }

    /// Runs the background maintenance now (autosave, heartbeats, external-change detection), regardless of the
    /// schedule. Hosts never need it; tests use it to make autosave deterministic.
    ///
    /// # Errors
    /// `command.timeout`, `internal`.
    pub fn run_maintenance(&self, force_autosave: bool) -> Result<(), CommandError> {
        self.run_maintenance_timed(force_autosave).map(|_| ())
    }

    /// [`ExecutorHandle::run_maintenance`], reporting how long each part held the document thread (C1.4 obligations 4
    /// and 5: the folder fingerprint and the autosave snapshot are measured where callers wait for them).
    ///
    /// # Errors
    /// `command.timeout`, `internal`.
    pub fn run_maintenance_timed(&self, force_autosave: bool) -> Result<MaintenanceTimings, CommandError> {
        self.call("maintenance", |r| Request::Maintenance(force_autosave, r))
    }

    fn call<T>(&self, what: &str, make: impl FnOnce(Sender<T>) -> Request) -> Result<T, CommandError> {
        self.call_with(what, make, &not_cancelled, Link::None)
    }

    /// [`ExecutorHandle::call`] for a caller that can cancel a request that may change something (an action, a job, a
    /// batch; ADR-0020 decision 8, C34-R2-01):
    /// - a request already cancelled when the call is made never begins (no token, nothing enqueued, nothing
    ///   remembered under its `requestId`);
    /// - the worker asks the caller directly, through a link, at dequeue, before each invocation or batch step, and at
    ///   each acceptance and commit check, so once the cancellation is observable no later boundary is passed;
    /// - the caller also marks the call's watchdog token when it sees the cancellation (polled every [`CANCEL_POLL`]),
    ///   which stops work-account checks and wakes waits;
    /// - each question is bounded ([`ask_bound`]); a caller that does not answer in time is taken as not cancelled at
    ///   that boundary and is not asked again for this request (C34-R3-01).
    fn call_until<T>(
        &self,
        what: &str,
        make: impl FnOnce(Sender<T>) -> Request,
        cancelled: Cancelled<'_>,
    ) -> Result<T, CommandError> {
        if cancelled() {
            return Err(request_cancelled_error(what));
        }
        self.call_with(what, make, cancelled, Link::Direct)
    }

    /// The caller's side of a call: enqueue with a token (and a link when `link` is direct), then wait for the reply,
    /// answering the worker's questions, polling `cancelled` and enforcing the watchdog budget.
    fn call_with<T>(
        &self,
        what: &str,
        make: impl FnOnce(Sender<T>) -> Request,
        cancelled: Cancelled<'_>,
        link: Link,
    ) -> Result<T, CommandError> {
        let (reply, rx) = mpsc::channel();
        let (asks, questions) = mpsc::channel::<CallerMsg>();
        let linked = !matches!(link, Link::None);
        let link = linked.then_some(CallerLink { tx: asks, ask: matches!(link, Link::Direct) });
        // The token travels with the request: a timeout cancels exactly this request, queued or running (C23-R2-01).
        let token = self.cancel.token();
        self.tx.send(Request::Tagged(token, link, Box::new(make(reply)))).map_err(|_| stopped())?;
        let deadline = std::time::Instant::now() + self.config.budget;
        let mut marked = false;
        // While the worker holds the link, the caller waits on it (questions, a discard notice, or its release right
        // after the reply); otherwise on the reply itself.
        let mut link_open = linked;
        loop {
            // The watchdog first: a caller held past its budget (inside its own probe, say) answers the timeout even if
            // the reply has arrived meanwhile; a commit that happened stands and is remembered under its `requestId`.
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                // Queued: the worker drops it at dequeue. Running: it stops at its next work-account check and before
                // any commit. Either way it never begins or commits after this answer (ADR-0015 history).
                self.cancel.mark(token);
                return Err(timeout(what, self.config.budget));
            }
            match rx.try_recv() {
                Ok(v) => return Ok(v),
                Err(mpsc::TryRecvError::Disconnected) => {
                    // The worker dropped the request without answering. Only its positive notice that it discarded a
                    // cancelled request makes that a cancellation; a marked token alone proves nothing, and anything
                    // else is a worker that stopped (C34-R2-04, C34-R3-03).
                    let discarded = questions.try_iter().any(|m| matches!(m, CallerMsg::Discarded));
                    return Err(if discarded { request_cancelled_error(what) } else { stopped() });
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            let wait = left.min(CANCEL_POLL);
            if link_open {
                match questions.recv_timeout(wait) {
                    Ok(CallerMsg::Ask(answer)) => {
                        let c = cancelled();
                        if c && !marked {
                            self.cancel.mark_cancelled(token);
                            marked = true;
                        }
                        let _ = answer.send(c);
                        continue;
                    }
                    Ok(CallerMsg::Discarded) => return Err(request_cancelled_error(what)),
                    // The worker released the link: the reply was sent first.
                    Err(RecvTimeoutError::Disconnected) => {
                        link_open = false;
                        continue;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
            } else {
                match rx.recv_timeout(wait) {
                    Ok(v) => return Ok(v),
                    Err(RecvTimeoutError::Disconnected) => {
                        let discarded = questions.try_iter().any(|m| matches!(m, CallerMsg::Discarded));
                        return Err(if discarded { request_cancelled_error(what) } else { stopped() });
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
            if !marked && cancelled() {
                self.cancel.mark_cancelled(token);
                marked = true;
            }
        }
    }

    /// Runs one command (11 §7.2 `run_command`).
    #[must_use]
    pub fn invoke(&self, inv: Invocation) -> Envelope {
        if inv.command == "job.wait" {
            return self.job_wait(inv);
        }
        self.invoke_served(inv).0
    }

    /// `wait: true` of a job command (09 §11.1 item 4; C33-R1-02): the call is answered on the document thread with
    /// `{jobId}` as usual, then the caller's thread (never the document thread) waits for the job, bounded by the call's
    /// watchdog budget, and the envelope carries the job's final result (its manifest, with `jobId`) or its failure. A
    /// job still running at the timeout answers `job.waitTimeout` with the `jobId`, which stays addressable
    /// (`job.wait`, `job.cancel`).
    fn waited(&self, params: &Value, env: Envelope, cancelled: Cancelled<'_>) -> Envelope {
        if params.get("wait") != Some(&Value::Bool(true)) || !env.is_ok() {
            return env;
        }
        let Some(job) = env
            .result
            .as_ref()
            .and_then(|r| r.get("jobId"))
            .and_then(Value::as_str)
            .filter(|j| *j != "dryRun")
            .map(str::to_owned)
        else {
            return env;
        };
        wait_envelope(&self.jobs, &job, self.config.budget, env.document_rev, cancelled)
    }

    /// [`ExecutorHandle::invoke`], also saying whether the answer came from the `requestId` cache: a retry that
    /// executed nothing (C14-R2-03; the envelope is the remembered one, identical to the first answer).
    #[must_use]
    pub fn invoke_served(&self, inv: Invocation) -> (Envelope, bool) {
        self.serve(inv, None)
    }

    /// [`ExecutorHandle::invoke_served`] for a caller that can cancel its request (an MCP request the client cancelled,
    /// 11 §4): `cancelled` is polled while the command runs and while a `wait: true` waits. A cancelled wait answers
    /// `command.timeout` with `reason: "cancelled"` and the `jobId`; the job is not the request's and continues
    /// (`job.list`, `job.wait`, `job.cancel`; ADR-0020 decision 8).
    #[must_use]
    pub fn invoke_served_until(&self, inv: Invocation, cancelled: Cancelled<'_>) -> (Envelope, bool) {
        self.serve(inv, Some(cancelled))
    }

    /// One command, with the caller's cancellation probe or none. Without a probe the call carries no link and costs no
    /// question (C34-R3-01).
    fn serve(&self, inv: Invocation, probe: Option<Cancelled<'_>>) -> (Envelope, bool) {
        let cancelled = probe.unwrap_or(&not_cancelled);
        if inv.command == "job.wait" {
            return (self.job_wait_until(inv, cancelled), false);
        }
        let what = inv.command.clone();
        let params = inv.params.clone();
        // A query changes nothing: it is not refused before it begins and the worker never asks its caller; a
        // cancellation marks its token, which stops it at its next work-account check, and a link carries only the
        // notice that it was discarded (ADR-0020 decision 8).
        let query = self.registry.get(&what).is_some_and(|e| e.spec.kind == tt_registry::Kind::Query);
        let called = match probe {
            None => self.call(&what, |r| Request::Invoke(inv, r)),
            Some(_) if query => self.call_with(&what, |r| Request::Invoke(inv, r), cancelled, Link::Notify),
            Some(c) => self.call_until(&what, |r| Request::Invoke(inv, r), c),
        };
        let (env, cached) = called.unwrap_or_else(|e| (Envelope::error(e, None), false));
        (self.waited(&params, env, cancelled), cached)
    }

    /// Runs a batch (11 §7.3).
    #[must_use]
    pub fn batch(&self, req: BatchRequest, origin: Origin) -> Envelope {
        self.call("batch", |r| Request::Batch(req, origin, r)).unwrap_or_else(|e| Envelope::error(e, None))
    }

    /// [`ExecutorHandle::batch`] for a caller that can cancel its request: once `cancelled` holds, the step running
    /// stops at its next safe point (a waited job step within [`CANCEL_POLL`]) and every later step is refused at its
    /// acceptance boundary, so nothing after the cancellation changes the document. Jobs the batch started continue
    /// (ADR-0020 decision 8).
    #[must_use]
    pub fn batch_until(&self, req: BatchRequest, origin: Origin, cancelled: Cancelled<'_>) -> Envelope {
        self.call_until("batch", |r| Request::Batch(req, origin, r), cancelled)
            .unwrap_or_else(|e| Envelope::error(e, None))
    }

    /// Registers an agent session with the hub (11 §6.1; `agent.session.connected`). Its grant is authoritative for
    /// every later call of `Origin::Agent { session, .. }`.
    ///
    /// # Errors
    /// `command.timeout`, `internal` (executor stopped).
    pub fn connect_agent(&self, c: agents::AgentConnect) -> Result<agents::AgentSession, CommandError> {
        self.call("agent.connect", |r| {
            Request::Run(Box::new(move |core| {
                let _ = r.send(core.connect_agent(c));
            }))
        })
    }

    /// Removes an agent session (`agent.session.disconnected`); its pending confirmations and roots go with it.
    ///
    /// # Errors
    /// `command.timeout`, `internal` (executor stopped).
    pub fn disconnect_agent(&self, session: &str, reason: &str) -> Result<(), CommandError> {
        let (session, reason) = (session.to_owned(), reason.to_owned());
        self.call("agent.disconnect", |r| {
            Request::Run(Box::new(move |core| {
                core.disconnect_agent(&session, &reason);
                let _ = r.send(());
            }))
        })
    }

    /// The session state an MCP server polls between calls (connected, paused, grant, limits, typed tools).
    ///
    /// # Errors
    /// `command.timeout`, `internal` (executor stopped).
    pub fn agent_status(&self, session: &str) -> Result<agents::AgentStatus, CommandError> {
        let session = session.to_owned();
        self.call("agent.status", |r| {
            Request::Run(Box::new(move |core| {
                let _ = r.send(core.agent_status(&session));
            }))
        })
    }

    /// Sets how long confirmation tokens are valid (11 §6.3: five minutes; tests shorten it).
    ///
    /// # Errors
    /// `command.timeout`, `internal` (executor stopped).
    pub fn set_confirmation_ttl(&self, ttl: Duration) -> Result<(), CommandError> {
        self.call("agent.ttl", |r| {
            Request::Run(Box::new(move |core| {
                if let Some(s) = core.session_mut() {
                    s.agents.ttl = Some(ttl);
                }
                let _ = r.send(());
            }))
        })
    }

    /// Runs one agent call and appends its activity entry (11 §11): exactly one entry per call, whatever its outcome,
    /// with the MCP tool that made it.
    #[must_use]
    pub fn invoke_audited(&self, inv: Invocation, tool: &str) -> Envelope {
        let started = std::time::Instant::now();
        let (command, params, origin) = (inv.command.clone(), inv.params.clone(), inv.origin.clone());
        let env = self.invoke(inv);
        self.audit_call(&origin, tool, &command, &params, &env, started.elapsed());
        env
    }

    /// Runs one agent batch and appends its activity entry (one for the batch; 11 §11 groups by batch).
    #[must_use]
    pub fn batch_audited(&self, req: BatchRequest, origin: Origin, tool: &str) -> Envelope {
        let started = std::time::Instant::now();
        let params = serde_json::to_value(&req).unwrap_or(Value::Null);
        let env = self.batch(req, origin.clone());
        self.audit_call(&origin, tool, "batch", &params, &env, started.elapsed());
        env
    }

    /// [`ExecutorHandle::batch_audited`] for a caller that can cancel its request ([`ExecutorHandle::batch_until`]).
    #[must_use]
    pub fn batch_audited_until(
        &self,
        req: BatchRequest,
        origin: Origin,
        tool: &str,
        cancelled: Cancelled<'_>,
    ) -> Envelope {
        let started = std::time::Instant::now();
        let params = serde_json::to_value(&req).unwrap_or(Value::Null);
        let env = self.batch_until(req, origin.clone(), cancelled);
        self.audit_call(&origin, tool, "batch", &params, &env, started.elapsed());
        env
    }

    /// Appends one activity entry for a call the MCP server answered itself (a server-side tool such as
    /// `describe_command`, or a tool whose capability arrives with a later milestone). No-op for non-agent origins.
    pub fn audit_call(
        &self,
        origin: &Origin,
        tool: &str,
        command: &str,
        params: &Value,
        env: &Envelope,
        took: Duration,
    ) {
        self.audit_logical_call(origin, &AuditCall { tool, command, params, env, took, cached: false });
    }

    /// [`ExecutorHandle::audit_call`] for a call whose answer may have come from the `requestId` cache (`cached`):
    /// the entry is marked, and a replay leaves it out (C14-R2-03). A batch records the steps it committed
    /// (C14-R2-02).
    pub fn audit_logical_call(&self, origin: &Origin, call: &AuditCall<'_>) {
        let Origin::Agent { session, name, .. } = origin else { return };
        let AuditCall { tool, command, params, env, took, cached } = *call;
        // Steps that committed: an atomic batch commits all of them or none; a sequential batch commits each
        // successful step on its own (11 §7.3). A dry run commits nothing.
        // Commit facts come from the commits themselves, not from the answer's status (C14-R3-01): an atomic batch
        // whose file could not be published still committed all its steps.
        let committed_steps = (command == "batch").then(|| {
            let atomic = params.get("atomic").and_then(Value::as_bool).unwrap_or(true);
            let steps = env.steps.as_deref().unwrap_or_default();
            if env.dry_run || params.get("dryRun").and_then(Value::as_bool) == Some(true) {
                Vec::new()
            } else if atomic {
                if env.committed { (0..steps.len()).collect() } else { Vec::new() }
            } else {
                steps.iter().enumerate().filter(|(_, s)| s.committed && !s.dry_run).map(|(i, _)| i).collect()
            }
        });
        let entry = agents::ActivityEntry {
            seq: 0,
            time: time::now_iso(),
            session_id: session.clone(),
            client: name.clone(),
            tool: tool.to_owned(),
            command_id: command.to_owned(),
            params: agents::redact(params),
            status: match env.status {
                Status::Ok => "ok",
                Status::NeedsConfirmation => "needs_confirmation",
                Status::Error => "error",
            }
            .to_owned(),
            error_code: env.error.as_ref().map(|e| e.code.clone()),
            undo_step_id: env.undo.as_ref().map(|u| u.step_id.clone()),
            changed_count: env.changes.len(),
            dry_run: env.dry_run,
            cached_retry: cached,
            committed_steps,
            document_rev: env.document_rev,
            duration_ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
        };
        // Queued behind the call it records, without waiting: the reply never blocks on a busy worker (C23-R1-07).
        let _ = self.tx.send(Request::Run(Box::new(move |core| {
            let _ = core.audit(entry);
        })));
    }

    /// Opens a document as the project, as the user, refusing to discard unsaved changes of an open project
    /// ([`ExecutorHandle::open_document_with`] with [`UnsavedPolicy::Fail`]).
    #[must_use]
    pub fn open_document(&self, doc: Document, path: Option<String>, read_only: bool) -> Envelope {
        self.open_document_with(doc, OpenOptions { path, read_only, on_unsaved: UnsavedPolicy::Fail }, Origin::User)
    }

    /// The guarded load transaction (ADR-0013 §16; C12-R1-01): the origin needs the scopes of `project.open`, and a
    /// modified open project is only replaced as `on_unsaved` says (`project.unsavedChanges` by default). The
    /// replacement is recorded (`project.closed`, `project.opened`). `project.open` installs packages the same way.
    #[must_use]
    pub fn open_document_with(&self, doc: Document, options: OpenOptions, origin: Origin) -> Envelope {
        self.call("project.open", |r| Request::Open(Box::new(doc), options, origin, r))
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

    /// `job.wait` (C12-R1-04): the invocation contract runs on the document thread (remembered requests, scopes,
    /// parameters, `expectRev`, ownership); the wait itself happens on the caller's thread (11 P5), bounded by the
    /// command's own `timeoutMs` (11 §15.2), and the envelope is completed and remembered on the document thread.
    fn job_wait(&self, inv: Invocation) -> Envelope {
        self.job_wait_until(inv, &not_cancelled)
    }

    /// `job.wait` for a caller that can cancel its request (C34-R2-02): the wait (and the attached retry's wait) ends
    /// on cancellation with `command.timeout`, `reason: "cancelled"` and the `jobId`; the job is untouched. The two
    /// bookkeeping halves on the document thread run as usual, so the call in flight is always finished.
    fn job_wait_until(&self, inv: Invocation, cancelled: Cancelled<'_>) -> Envelope {
        let start = match self.call("job.wait", |r| Request::WaitBegin(inv.clone(), r)) {
            Ok(s) => s,
            Err(e) => return Envelope::error(e, None),
        };
        let (job_id, timeout) = match start {
            run::WaitStart::Done(env) => return *env,
            run::WaitStart::Wait { job_id, timeout } => (job_id, timeout),
            // An identical retry gets the envelope of the call in flight (C12-R3-03), at most the owner's wait plus
            // one watchdog budget later.
            run::WaitStart::Attached { envelope, timeout: owner_wait } => {
                let job = inv.params.get("jobId").and_then(Value::as_str).unwrap_or_default().to_owned();
                let deadline = std::time::Instant::now() + owner_wait.saturating_add(self.config.budget);
                loop {
                    let left = deadline.saturating_duration_since(std::time::Instant::now());
                    match envelope.recv_timeout(left.min(CANCEL_POLL)) {
                        Ok(env) => return env,
                        Err(RecvTimeoutError::Timeout) if cancelled() => {
                            return Envelope::error(wait_cancelled_error(&job), None);
                        }
                        Err(RecvTimeoutError::Timeout) if !left.is_zero() => {}
                        Err(_) => return Envelope::error(timeout("job.wait", self.config.budget), None),
                    }
                }
            }
        };
        let outcome = match wait_sliced(&self.jobs, &job_id, timeout, cancelled) {
            Some(waited) => waited.and_then(|(record, timed_out)| commands::job::wait_result(record, timed_out)),
            None => Err(wait_cancelled_error(&job_id)),
        };
        self.call("job.wait", |r| Request::WaitFinish(inv, outcome, r)).unwrap_or_else(|e| Envelope::error(e, None))
    }
}

fn document_thread(
    mut core: run::Core,
    rx: &Receiver<Request>,
    subscribers: &Mutex<Vec<Sender<Event>>>,
    cancel: &Arc<tt_vector::work::CancelState>,
    budget: Duration,
) {
    let bound = ask_bound(budget);
    // A call's events are broadcast before its reply is sent, so a subscriber has them when the caller has the result.
    let flush = |core: &mut run::Core| broadcast(subscribers, &core.take_events());
    tt_vector::work::set_cancel_state(Some(cancel.clone()));
    while let Ok(req) = rx.recv() {
        let (token, link, req) = match req {
            Request::Tagged(t, link, r) => (t, link, *r),
            r => (0, None, r),
        };
        // A request whose caller already timed out never begins (C23-R2-01), nor one whose caller says at dequeue that
        // it cancelled (C34-R2-01); that caller is told it was discarded, not that the worker stopped (C34-R2-04).
        caller::set(link.clone(), bound);
        // A probed query (a notice-only link) is not discarded for its caller's cancellation: it changes nothing and
        // stops at its next work-account check; a timed-out caller's request is discarded whatever it is.
        let query = link.as_ref().is_some_and(|l| !l.ask);
        let marked = match cancel.cause(token) {
            Some(tt_vector::work::Cause::Cancelled) => !query,
            Some(tt_vector::work::Cause::Timeout) => true,
            None => false,
        };
        if token != 0 && (marked || caller::cancelled()) {
            if let Some(l) = &link {
                let _ = l.tx.send(CallerMsg::Discarded);
            }
            caller::set(None, bound);
            cancel.forget(token);
            continue;
        }
        cancel.current.store(token, std::sync::atomic::Ordering::Release);
        let shutdown = matches!(req, Request::Shutdown);
        match req {
            Request::Invoke(inv, reply) => {
                let env = core.invoke(&inv);
                let cached = core.take_served_from_cache();
                flush(&mut core);
                let _ = reply.send((env, cached));
            }
            Request::Batch(b, origin, reply) => {
                let env = core.batch(&b, &origin);
                flush(&mut core);
                let _ = reply.send(env);
            }
            Request::Open(doc, options, origin, reply) => {
                let env = core.load(*doc, &options, &origin);
                flush(&mut core);
                let _ = reply.send(env);
            }
            Request::WaitBegin(inv, reply) => {
                let start = core.wait_begin(&inv);
                flush(&mut core);
                let _ = reply.send(start);
            }
            Request::WaitFinish(inv, outcome, reply) => {
                let env = core.wait_finish(&inv, outcome);
                flush(&mut core);
                let _ = reply.send(env);
            }
            Request::Snapshot(reply) => {
                let doc = core.session().and_then(|s| s.project.as_ref()).map(|p| p.doc.clone());
                let _ = reply.send(doc);
            }
            Request::List(origin, reply) => {
                let listed = list(&mut core, origin);
                flush(&mut core);
                let _ = reply.send(listed);
            }
            Request::Run(f) => f(&mut core),
            Request::Tick => {
                let _ = maintenance::tick(&mut core, false);
            }
            Request::Maintenance(force, reply) => {
                let timings = maintenance::tick(&mut core, force);
                flush(&mut core);
                let _ = reply.send(timings);
            }
            Request::Shutdown | Request::Tagged(..) => {}
        }
        // The link is released only after the reply was sent, so a caller woken by its release finds the reply.
        caller::set(None, bound);
        drop(link);
        cancel.current.store(0, std::sync::atomic::Ordering::Release);
        if token != 0 {
            cancel.forget(token);
        }
        if shutdown {
            break;
        }
        flush(&mut core);
    }
    maintenance::stop(&mut core);
    // The project (lock, recovery files) is released before the dropper is told the thread is done.
    drop(core);
}

/// Delivers events to every live subscriber (dropping closed ones).
fn broadcast(subscribers: &Mutex<Vec<Sender<Event>>>, events: &[Event]) {
    if events.is_empty() {
        return;
    }
    let mut subs = subscribers.lock().unwrap_or_else(PoisonError::into_inner);
    subs.retain(|s| events.iter().all(|e| s.send(e.clone()).is_ok()));
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

/// Measurement hooks (dev-only feature `test-hooks`; never in a product build): lift Flatten's work budget so the
/// C22-T2 measurement can time a 100 000-stroke merge to completion (M2 C2.4).
#[cfg(feature = "test-hooks")]
pub mod hooks {
    use std::sync::atomic::{AtomicU64, Ordering};

    static MERGE_BUDGET: AtomicU64 = AtomicU64::new(0);

    /// Flatten's work budget from now on (0: the product's).
    pub fn set_merge_work_budget(units: u64) {
        MERGE_BUDGET.store(units, Ordering::Release);
    }

    pub(crate) fn merge_work_budget() -> Option<u64> {
        let b = MERGE_BUDGET.load(Ordering::Acquire);
        (b > 0).then_some(b)
    }
}

/// The answer of a waited job (09 §11.1 item 4; C33-R1-02): the job's final result with its `jobId` when it
/// succeeded, its error with the `jobId` when it failed or was cancelled, `job.waitTimeout` with the `jobId` when it is
/// still running after `wait` (it continues). Used by `wait: true` on a direct call and in a sequential batch.
pub(crate) fn wait_envelope(
    jobs: &JobTable,
    job: &str,
    wait: std::time::Duration,
    document_rev: Option<u64>,
    cancelled: Cancelled<'_>,
) -> Envelope {
    let outcome = match wait_sliced(jobs, job, wait, cancelled) {
        Some(o) => o,
        None => return Envelope::error(wait_cancelled_error(job), document_rev),
    };
    match outcome {
        Ok((record, false)) if record.state == JobState::Succeeded => {
            let mut result = record.result.unwrap_or(Value::Null);
            if let Some(m) = result.as_object_mut() {
                m.insert("jobId".into(), Value::String(job.to_owned()));
            }
            Envelope::ok(result, document_rev)
        }
        Ok((record, false)) => {
            let mut e = record
                .error
                .unwrap_or_else(|| CommandError::new("job.failed", format!("job `{job}` ended {:?}", record.state)));
            if let Some(m) = e.details.as_object_mut() {
                m.insert("jobId".into(), Value::String(job.to_owned()));
            } else {
                *e.details = serde_json::json!({ "jobId": job });
            }
            Envelope::error(e, document_rev)
        }
        Ok((_, true)) => Envelope::error(
            CommandError::with_details(
                "job.waitTimeout",
                format!("job `{job}` is still running after {} ms; it continues", wait.as_millis()),
                serde_json::json!({ "jobId": job, "timeoutMs": u64::try_from(wait.as_millis()).unwrap_or(u64::MAX) }),
            )
            .hint("wait for it with job.wait, or cancel it with job.cancel"),
            document_rev,
        ),
        Err(e) => Envelope::error(e, document_rev),
    }
}

/// A caller's cancellation probe: `true` once the caller cancelled its request (an MCP `notifications/cancelled`, 11 §4).
pub type Cancelled<'a> = &'a (dyn Fn() -> bool + Sync);

/// How often a call and a wait look at their caller's cancellation probe.
pub const CANCEL_POLL: Duration = Duration::from_millis(20);

fn not_cancelled() -> bool {
    false
}

/// Waits for a job up to `wait`, looking at `cancelled` every [`CANCEL_POLL`]: `None` when the caller cancelled first,
/// else the job table's answer (the record and whether the wait timed out).
pub(crate) fn wait_sliced(
    jobs: &JobTable,
    job: &str,
    wait: Duration,
    cancelled: Cancelled<'_>,
) -> Option<Result<(JobRecord, bool), CommandError>> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let slice = left.min(CANCEL_POLL);
        match jobs.wait(job, slice) {
            Ok((_, true)) if cancelled() => return None,
            Ok((record, true)) if slice >= left => return Some(Ok((record, true))),
            Ok((_, true)) => {}
            other => return Some(other),
        }
    }
}

/// Whether a call carries a link to its caller: none (no probe), a notice-only link (a probed query: the worker never
/// asks, but tells the caller when it discards the request), or a link the worker asks (C34-R2-01, C34-R3-01).
#[derive(Clone, Copy)]
enum Link {
    None,
    Notify,
    Direct,
}

/// The link a request carries to its caller.
#[derive(Clone)]
struct CallerLink {
    tx: Sender<CallerMsg>,
    /// Whether the worker asks this caller at its boundaries.
    ask: bool,
}

/// What the worker sends a cancellable caller (C34-R2-01, C34-R2-04).
enum CallerMsg {
    /// "Is the request cancelled?": the caller answers its probe at once.
    Ask(Sender<bool>),
    /// The request was discarded at dequeue because it was cancelled.
    Discarded,
}

/// The longest the document thread waits for one answer of a caller's probe (C34-R3-01).
pub const ASK_BOUND_MAX: Duration = Duration::from_millis(250);

/// One question's bound: [`ASK_BOUND_MAX`] or a quarter of the watchdog budget, whichever is smaller.
#[must_use]
pub fn ask_bound(budget: Duration) -> Duration {
    ASK_BOUND_MAX.min(budget / 4)
}

/// The link to the caller of the request the document thread is running.
mod caller {
    use std::cell::{Cell, RefCell};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    use super::{CallerLink, CallerMsg};

    thread_local! {
        static LINK: RefCell<Option<CallerLink>> = const { RefCell::new(None) };
        static BOUND: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        /// Latched when the caller did not answer in time: it is asked nothing more for this request.
        static UNRESPONSIVE: Cell<bool> = const { Cell::new(false) };
    }

    /// The running request's link (or none) and the bound of one question.
    pub(super) fn set(link: Option<CallerLink>, bound: Duration) {
        LINK.with(|l| *l.borrow_mut() = link);
        BOUND.with(|b| b.set(bound));
        UNRESPONSIVE.with(|u| u.set(false));
    }

    /// Whether the running request carries a notice-only link (a probed standalone query).
    pub(super) fn notice_only() -> bool {
        LINK.with(|l| l.borrow().as_ref().is_some_and(|link| !link.ask))
    }

    /// Asks the caller of the running request whether it cancelled. No link, a notice-only link or a caller that once
    /// failed to answer in time: `false` without asking. An answer within the bound: that answer. Silence: `false`
    /// (the request proceeds as not cancelled; a cancellation seen late is like a commit that got there first) and the
    /// link is latched. A caller that is gone: `true` (C34-R3-01).
    pub(super) fn cancelled() -> bool {
        let Some(link) = LINK.with(|l| l.borrow().clone()) else { return false };
        if !link.ask || UNRESPONSIVE.with(Cell::get) {
            return false;
        }
        tt_vector::work::note_ask();
        let (tx, rx) = mpsc::channel();
        if link.tx.send(CallerMsg::Ask(tx)).is_err() {
            return true;
        }
        match rx.recv_timeout(BOUND.with(Cell::get)) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => {
                UNRESPONSIVE.with(|u| u.set(true));
                false
            }
            Err(RecvTimeoutError::Disconnected) => true,
        }
    }
}

/// Why the request the document thread is running is cancelled, if it is: its watchdog token's cause (a timed-out
/// caller, or a cancelling caller's mark), else its caller's answer when asked (ADR-0020 decision 8; C34-R3-02).
pub(crate) fn cancellation() -> Option<tt_vector::work::Cause> {
    tt_vector::work::cancel_cause().or_else(|| caller::cancelled().then_some(tt_vector::work::Cause::Cancelled))
}

/// Whether the running request is a probed standalone query (its link carries notices only).
pub(crate) fn notice_only_link() -> bool {
    caller::notice_only()
}

/// Whether the request the document thread is running is cancelled ([`cancellation`]). Consulted at every
/// invocation's and batch step's start, at acceptance and before commit, and by waits on the document thread.
pub(crate) fn request_cancelled() -> bool {
    cancellation().is_some()
}

/// Labels a refusal of the running request whose cause is its caller's cancellation, wherever it was refused (a
/// work-account check included): `details.reason: "cancelled"` (SI-111). A watchdog timeout is not relabelled.
pub(crate) fn label_cancellation(env: &mut Envelope) {
    if tt_vector::work::cancel_cause() != Some(tt_vector::work::Cause::Cancelled) {
        return;
    }
    let label = |e: &mut CommandError| {
        if e.code == tt_registry::codes::COMMAND_TIMEOUT {
            if let Some(m) = e.details.as_object_mut() {
                m.entry("reason").or_insert_with(|| Value::String("cancelled".to_owned()));
            } else {
                *e.details = serde_json::json!({ "reason": "cancelled" });
            }
        }
    };
    if let Some(e) = env.error.as_mut() {
        label(e);
    }
    for step in env.steps.iter_mut().flatten() {
        if let Some(e) = step.error.as_mut() {
            label(e);
        }
    }
}

/// The refusal of a request its caller cancelled (SI-111: `command.timeout`, `details.reason: "cancelled"`).
fn request_cancelled_error(what: &str) -> CommandError {
    CommandError::with_details(
        tt_registry::codes::COMMAND_TIMEOUT,
        format!("`{what}` was not run: its caller cancelled the request"),
        serde_json::json!({ "commandId": what, "reason": "cancelled" }),
    )
}

/// A wait its caller cancelled; the job continues (SI-111).
fn wait_cancelled_error(job: &str) -> CommandError {
    CommandError::with_details(
        tt_registry::codes::COMMAND_TIMEOUT,
        format!("the request was cancelled by its caller; job `{job}` continues"),
        serde_json::json!({ "jobId": job, "reason": "cancelled" }),
    )
    .hint("wait for it with job.wait, or cancel it with job.cancel")
}
