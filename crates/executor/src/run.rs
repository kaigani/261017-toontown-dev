//! The transaction pipeline on the document thread (11 §2): validate → enablement → permission → execute as a
//! transaction → record the undo step → emit change events; plus `dryRun`, `expectRev`, `requestId`, batches
//! (11 §7.2–§7.3), the guarded load transaction and the two halves of `job.wait`.
//!
//! **Effects.** Everything a command does outside the staged document — cancelling a job, creating a checkpoint,
//! emitting an event — is deferred (`cx::Effect`) and applied only after its transaction commits; session state
//! (context, window, clipboard, checkpoints) is restored when a call fails or is a dry run. A dry-run sequential batch
//! runs the real batch on a clone of the session, so later steps see the simulated earlier ones, and then discards the
//! clone (C12-R1-02/03).

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use serde_json::{Map, Value, json};
use tt_model::change::ELIDE_BYTES;
use tt_model::document::Document;
use tt_registry::{CommandError, Entry, Kind, Registry, Status, codes, validate_params};

use crate::app::AppContext;
use crate::cx::{Cx, Effect, Txn, TxnKind, no_project};
use crate::history::{Step, StepEdit};
use crate::session::{Back, Checkpoint, Event, OpenProject, Session};
use crate::types::{
    BatchRequest, ChangeRecord, Envelope, Invocation, InvokeOptions, OpenOptions, Origin, Status as EnvStatus, UndoRef,
    UnsavedPolicy,
};
use crate::ui::UiState;
use crate::{CHANGES_CAP, MAX_BATCH_STEPS};

/// Results remembered per session for `requestId` (11 §7.2; ADR-0013 §5): at most this many …
pub(crate) const REQUEST_CACHE_ENTRIES: usize = 100_000;
/// … and at most this many bytes of envelopes. Evicted ids are still known (`request.unknownId`).
pub(crate) const REQUEST_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Cached {
    fingerprint: u64,
    command: String,
    envelope: Envelope,
    bytes: usize,
}

/// One session's remembered requests.
#[derive(Debug, Default, Clone)]
struct SessionRequests {
    order: VecDeque<String>,
    entries: HashMap<String, Cached>,
    bytes: usize,
    /// A hash of every request id the session used (8 bytes each): an evicted id is never executed again.
    seen: HashSet<u64>,
    /// Ids of calls in flight (a `job.wait` waiting off the document thread; C12-R2-06, C12-R3-03).
    pending: HashMap<String, InFlight>,
}

/// The call that owns an id while it is in flight, and the identical retries attached to it.
#[derive(Debug, Clone)]
struct InFlight {
    fingerprint: u64,
    command: String,
    /// Identical retries waiting for the owner's envelope (each receives exactly that envelope).
    attached: Vec<std::sync::mpsc::Sender<Envelope>>,
}

/// `requestId` idempotency, scoped to the session (ADR-0013 §5; C12-R1-06/22).
#[derive(Debug, Default, Clone)]
struct RequestCache {
    sessions: HashMap<String, SessionRequests>,
}

enum Lookup {
    Miss,
    /// The identical call is in flight: attach to it.
    InFlight,
    Hit(Box<Envelope>),
    Refused(CommandError),
}

fn hash64(x: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    x.hash(&mut h);
    h.finish()
}

/// What makes two invocations "the same request": command, parameters (canonical text) and options.
fn fingerprint(inv: &Invocation) -> u64 {
    let params = tt_model::canonical::to_canonical_string(&inv.params).unwrap_or_else(|_| inv.params.to_string());
    let expect: Vec<(&String, &u64)> = inv.options.expect_rev.iter().collect();
    hash64((&inv.command, params, inv.options.dry_run, expect))
}

impl RequestCache {
    fn lookup(&self, session: &str, id: &str, fp: u64) -> Lookup {
        let Some(s) = self.sessions.get(session) else { return Lookup::Miss };
        if let Some(p) = s.pending.get(id) {
            // In flight: the first call owns the id; an identical retry attaches to it, any other call conflicts.
            if p.fingerprint == fp {
                return Lookup::InFlight;
            }
            return Lookup::Refused(
                CommandError::with_details(
                    "request.conflict",
                    format!("requestId `{id}` is in use by a call in progress"),
                    json!({ "requestId": id, "originalCommand": p.command, "inProgress": true }),
                )
                .hint("use a new requestId for a different call"),
            );
        }
        if let Some(c) = s.entries.get(id) {
            if c.fingerprint == fp {
                return Lookup::Hit(Box::new(c.envelope.clone()));
            }
            return Lookup::Refused(
                CommandError::with_details(
                    "request.conflict",
                    format!("requestId `{id}` was used for a different call"),
                    json!({ "requestId": id, "originalCommand": c.command }),
                )
                .hint("use a new requestId for a different call"),
            );
        }
        if s.seen.contains(&hash64(id)) {
            return Lookup::Refused(
                CommandError::with_details(
                    "request.unknownId",
                    format!("the result of requestId `{id}` is no longer retained; the call is not executed again"),
                    json!({ "requestId": id, "reason": "evicted" }),
                )
                .hint("read doc.state.get / doc.diff to learn the outcome, then retry with a new requestId"),
            );
        }
        Lookup::Miss
    }

    /// Reserves an id for the call in flight that owns it.
    fn reserve(&mut self, session: &str, id: &str, fp: u64, command: &str) {
        let s = self.sessions.entry(session.to_owned()).or_default();
        s.seen.insert(hash64(id));
        s.pending
            .insert(id.to_owned(), InFlight { fingerprint: fp, command: command.to_owned(), attached: Vec::new() });
    }

    /// Attaches an identical retry to the call in flight; it receives the owner's envelope.
    fn attach(&mut self, session: &str, id: &str) -> Option<std::sync::mpsc::Receiver<Envelope>> {
        let p = self.sessions.get_mut(session)?.pending.get_mut(id)?;
        let (tx, rx) = std::sync::mpsc::channel();
        p.attached.push(tx);
        Some(rx)
    }

    /// Ends the call in flight: its envelope goes to every attached retry.
    fn release(&mut self, session: &str, id: &str, envelope: &Envelope) {
        if let Some(s) = self.sessions.get_mut(session)
            && let Some(p) = s.pending.remove(id)
        {
            for tx in p.attached {
                let _ = tx.send(envelope.clone());
            }
        }
    }

    /// Replaces the remembered `needs_confirmation` of an id by the call's final outcome (C14-R1-04).
    fn settle(&mut self, session: &str, id: &str, envelope: Envelope) {
        let bytes = serde_json::to_vec(&envelope).map_or(0, |v| v.len());
        if let Some(s) = self.sessions.get_mut(session)
            && let Some(c) = s.entries.get_mut(id)
        {
            s.bytes = s.bytes.saturating_sub(c.bytes).saturating_add(bytes);
            c.bytes = bytes;
            c.envelope = envelope;
        }
    }

    fn insert(&mut self, session: &str, id: &str, fp: u64, command: &str, envelope: Envelope) {
        let bytes = serde_json::to_vec(&envelope).map_or(0, |v| v.len());
        let s = self.sessions.entry(session.to_owned()).or_default();
        s.seen.insert(hash64(id));
        if s.entries.contains_key(id) {
            return; // the first envelope of an id is the one remembered (C12-R3-03)
        }
        if let Some(old) =
            s.entries.insert(id.to_owned(), Cached { fingerprint: fp, command: command.to_owned(), envelope, bytes })
        {
            s.bytes = s.bytes.saturating_sub(old.bytes);
        } else {
            s.order.push_back(id.to_owned());
        }
        s.bytes = s.bytes.saturating_add(bytes);
        while (s.entries.len() > REQUEST_CACHE_ENTRIES || s.bytes > REQUEST_CACHE_BYTES) && s.order.len() > 1 {
            if let Some(old) = s.order.pop_front()
                && let Some(c) = s.entries.remove(&old)
            {
                s.bytes = s.bytes.saturating_sub(c.bytes);
            }
        }
    }
}

/// Session state restored when an invocation fails or is a dry run: everything a handler may change besides the
/// staged document and its deferred effects.
struct Rollback {
    app: AppContext,
    ui: UiState,
    checkpoints: Option<Vec<Checkpoint>>,
    clipboard: Option<Value>,
    edit_options: std::collections::BTreeMap<String, serde_json::Map<String, Value>>,
    events: usize,
}

impl Rollback {
    fn take(s: &Session) -> Self {
        Self {
            app: s.app.clone(),
            ui: s.ui.clone(),
            checkpoints: s.project.as_ref().map(|p| p.checkpoints.clone()),
            clipboard: s.clipboard.clone(),
            edit_options: s.edit_options.clone(),
            events: s.events.len(),
        }
    }

    fn apply(self, s: &mut Session) {
        s.app = self.app;
        s.ui = self.ui;
        s.clipboard = self.clipboard;
        s.edit_options = self.edit_options;
        s.events.truncate(self.events);
        if let (Some(p), Some(c)) = (s.project.as_mut(), self.checkpoints) {
            p.checkpoints = c;
        }
    }
}

/// What observers see of the session, to emit `app.state.changed`, `edit.selection.changed`, `workspace.changed`,
/// `edit.history.changed` and `project.dirty.changed` only when something changed.
struct Observed {
    state: Value,
    history: Option<(usize, usize)>,
    dirty: Option<bool>,
    /// The persisted form of the workspaces (written to the settings when it changes; 01 §11).
    workspaces: Value,
}

fn observe(s: &Session) -> Observed {
    Observed {
        state: serde_json::to_value(crate::commands::app::app_state(s)).unwrap_or(Value::Null),
        history: s.project.as_ref().map(|p| (p.history.position(), p.history.len())),
        dirty: s.project.as_ref().map(OpenProject::dirty),
        workspaces: s.ui.persisted(),
    }
}

/// Proof that the load guard passed (ADR-0013 §16; C12-R1-01). Only [`Core::load`] creates one, and only
/// [`Core::install`] — the one place that replaces the open project — accepts it; neither is reachable from other
/// crates, which can only ask for the guarded load through `ExecutorHandle::open_document`.
pub struct LoadGrant {
    _private: (),
}

/// The outcome of the first half of `job.wait`.
pub(crate) enum WaitStart {
    /// Answered on the document thread (final job, `timeoutMs: 0`, an error, or a remembered request).
    Done(Box<Envelope>),
    /// Wait on the caller's thread, then finish on the document thread.
    Wait {
        /// The job.
        job_id: String,
        /// How long.
        timeout: Duration,
    },
    /// An identical retry of a call in flight: receive that call's envelope (C12-R3-03).
    Attached {
        /// Delivers the owner's envelope.
        envelope: std::sync::mpsc::Receiver<Envelope>,
        /// The owner's wait (the retry has the same parameters).
        timeout: Duration,
    },
}

/// The core on the document thread.
#[derive(Debug)]
pub(crate) struct Core {
    session: Option<Session>,
    requests: RequestCache,
    registry: &'static Registry,
    /// Running a dry-run batch on a clone of the session: no effect leaves it, no request is remembered.
    simulating: bool,
    /// The watchdog budget of one call (bounds waits on the document thread).
    budget: Duration,
    /// The last `invoke` was answered from the `requestId` cache (read and cleared by `take_served_from_cache`).
    served_from_cache: bool,
}

fn unknown(id: &str, registry: &Registry) -> CommandError {
    let ns = id.split('.').next().unwrap_or_default();
    let suggestions: Vec<&str> = registry.entries().map(Entry::id).filter(|e| e.starts_with(ns)).take(10).collect();
    CommandError::with_details(
        codes::COMMAND_UNKNOWN,
        format!("no command `{id}`"),
        json!({ "commandId": id, "suggestions": suggestions }),
    )
    .hint("list_commands shows every registered command")
}

/// The scopes an origin lacks for an entry.
fn missing_scopes(entry: &Entry, origin: &Origin) -> Vec<&'static str> {
    entry.scopes().into_iter().filter(|s| !origin.has_scope(*s)).map(|s| s.as_str()).collect()
}

/// Whether a command declares its own `dryRun` parameter and the call sets it (10 AG-02).
fn param_dry_run(entry: &Entry, params: &Value) -> bool {
    params.get("dryRun") == Some(&Value::Bool(true)) && entry.params_schema().pointer("/properties/dryRun").is_some()
}

/// The validation and execution of one command inside a context (11 §8 order: existence, kind, permission,
/// parameters, project, enablement, implementation status, `expectRev`, handler).
pub(crate) fn run_step(cx: &mut Cx, entry: &Entry, params: Value, opts: &InvokeOptions) -> Result<Value, CommandError> {
    let id = entry.id();
    cx.set_command(id);
    if entry.spec.kind == Kind::Event {
        return Err(CommandError::with_details(
            codes::COMMAND_DISABLED,
            format!("`{id}` is an event; events are delivered to subscribers and cannot be invoked"),
            json!({ "commandId": id, "reason": "event" }),
        ));
    }
    let missing = missing_scopes(entry, cx.origin());
    if !missing.is_empty() {
        return Err(CommandError::with_details(
            codes::PERMISSION_DENIED,
            format!("`{id}` needs the scopes {missing:?}"),
            json!({ "commandId": id, "missingScopes": missing }),
        ));
    }
    let params = match params {
        Value::Null => Value::Object(Map::new()),
        v @ Value::Object(_) => v,
        _ => {
            return Err(CommandError::with_details(
                codes::ARG_INVALID,
                format!("`{id}`: parameters must be a JSON object"),
                json!({ "pointer": "" }),
            ));
        }
    };
    validate_params(id, &entry.params_schema(), &params)?;
    if let Some(c) = entry.command {
        if c.decl.needs_project && !cx.has_project() {
            return Err(no_project());
        }
        if let Some(check) = c.decl.enablement.check {
            check(&*cx).map_err(|d| d.to_error(id))?;
        }
    }
    if let Status::NotImplemented { owner } = entry.status() {
        return Err(CommandError::not_implemented(id, owner));
    }
    if !opts.expect_rev.is_empty() {
        let pairs: Vec<(String, u64)> = opts.expect_rev.iter().map(|(k, v)| (k.clone(), *v)).collect();
        check_expect_rev(cx, &pairs)?;
    }
    let Some(command) = entry.command else { return Err(CommandError::internal("implemented entry without handler")) };
    (command.handler)(cx, params)
}

/// REV-05 (C12-R1-07): every listed object must still have the expected `rev`; a mismatch is `rev.conflict` with
/// `details.currentRev` and, when its JSON is smaller than 64 KiB, the current object.
pub(crate) fn check_expect_rev(cx: &Cx, expect: &[(String, u64)]) -> Result<(), CommandError> {
    let doc = cx.document()?;
    for (r, expected) in expect {
        let key = match cx.resolve(r)? {
            tt_model::document::Target::Object { key, .. } => key,
            tt_model::document::Target::Cell { element, .. } => element.as_str().to_owned(),
        };
        let current = doc.locate(&key).map_or(0, |l| l.rev);
        if current != *expected {
            let mut details = json!({
                "ref": r,
                "id": key,
                "expectedRev": expected,
                "currentRev": current,
                "documentRev": doc.document_rev(),
            });
            if let Ok((_, object)) = doc.object_raw_json(&key)
                && serde_json::to_vec(&object).is_ok_and(|b| b.len() < ELIDE_BYTES)
                && let Some(m) = details.as_object_mut()
            {
                m.insert("current".into(), object);
            }
            return Err(CommandError::with_details(
                codes::REV_CONFLICT,
                format!("`{r}` changed: rev {current}, expected {expected}"),
                details,
            )
            .hint("re-read the object and retry"));
        }
    }
    Ok(())
}

fn cap(mut changes: Vec<ChangeRecord>, before_rev: u64) -> (Vec<ChangeRecord>, bool, Option<u64>) {
    if changes.len() > CHANGES_CAP {
        changes.truncate(CHANGES_CAP);
        (changes, true, Some(before_rev))
    } else {
        (changes, false, None)
    }
}

/// Elides values over 64 KiB in event payloads (10 §2.6).
fn elide(changes: &[ChangeRecord]) -> Vec<ChangeRecord> {
    changes
        .iter()
        .map(|c| {
            let mut c = c.clone();
            if let Some(p) = c.patch.as_mut() {
                for op in p.iter_mut() {
                    if let Some(m) = op.as_object_mut() {
                        let big = |v: Option<&Value>| v.is_some_and(|v| v.to_string().len() > ELIDE_BYTES);
                        if big(m.get("value")) || big(m.get("oldValue")) {
                            m.shift_remove("value");
                            m.shift_remove("oldValue");
                            m.insert("elided".into(), json!(true));
                        }
                    }
                }
            }
            c
        })
        .collect()
}

fn back_of(t: &Txn) -> Back {
    match (&t.edit, t.kind) {
        (StepEdit::Snapshot { .. }, _) => Back::Snapshot(t.before.clone()),
        (StepEdit::Patches(commits), TxnKind::Step | TxnKind::Redo(_) | TxnKind::System) => {
            Back::Patches(commits.iter().rev().map(|c| (c.key.clone(), c.undo.clone())).collect())
        }
        (StepEdit::Patches(commits), TxnKind::Undo(_)) => {
            Back::Patches(commits.iter().map(|c| (c.key.clone(), c.redo.clone())).collect())
        }
    }
}

/// The revisions the objects a transaction stamped had before it (so `doc.diff` can rebuild them; C12-R1-08).
fn earlier_revs(t: &Txn) -> Vec<(String, u64)> {
    t.after
        .locations()
        .filter(|l| l.rev == t.rev)
        .filter_map(|l| t.before.locate(&l.key).filter(|b| b.key == l.key).map(|b| (l.key.clone(), b.rev)))
        .collect()
}

/// VER-02: objects holding enum values of a newer minor version are read-only. A transaction that changes the entry
/// of such an object (the object, or anything stored in its entry) fails with `doc.unknownValue`; the entry is written
/// back as read on save. The `modifiedAt` stamp of a save (a system transaction) is exempt: a frozen `project.json` is
/// written as read anyway.
fn frozen_guard(s: &Session, txns: &[Txn]) -> Result<(), CommandError> {
    let Some(p) = s.project.as_ref() else { return Ok(()) };
    if p.store.preserved.frozen.is_empty() {
        return Ok(());
    }
    for t in txns.iter().filter(|t| t.kind != TxnKind::System) {
        for (key, f) in &p.store.preserved.frozen {
            let same = match (t.before.entry_canonical(key), t.after.entry_canonical(key)) {
                (Ok(a), Ok(b)) => a == b,
                (Err(_), Err(_)) => true,
                _ => false,
            };
            if !same {
                let entry = tt_persist::layout::entry_path(&t.before, key);
                let values: Vec<Value> =
                    f.substitutes.iter().map(|(ptr, v, _)| json!({ "pointer": ptr, "value": v })).collect();
                return Err(CommandError::with_details(
                    tt_persist::codes::UNKNOWN_VALUE,
                    format!(
                        "`{}` holds values of a newer format version and is read-only",
                        entry.as_deref().unwrap_or("an entry")
                    ),
                    json!({ "entry": entry, "unknownValues": values }),
                )
                .hint("open the project with a newer version of the application to edit it"));
            }
        }
    }
    Ok(())
}

/// The envelope of a failed call. `confirmation.required` (11 §6.3) is not a failure: the call had no effect and
/// returns `needs_confirmation` with the token, the summary and the consequences; `agent.confirmation.requested` tells
/// the GUI (attached mode).
fn needs_confirmation(session: &mut Session, e: CommandError, rev: Option<u64>) -> Envelope {
    if e.code != "confirmation.required" {
        return Envelope::error(e, rev);
    }
    let d = &e.details;
    let result = json!({
        "status": "needs_confirmation",
        "confirmToken": d.get("confirmToken"),
        "summary": d.get("summary"),
        "consequences": d.get("consequences"),
        "category": d.get("category"),
    });
    session.emit(
        "agent.confirmation.requested",
        json!({
            "confirmationId": d.get("confirmToken"), "sessionId": d.get("sessionId"),
            "summary": d.get("summary"), "consequences": d.get("consequences"),
        }),
    );
    let mut env = Envelope::ok(result, rev);
    env.status = crate::types::Status::NeedsConfirmation;
    env
}

/// Commits transactions: installs each stamped snapshot, records undo steps or moves the history position, logs
/// the revision and emits `doc.changed`.
fn commit(s: &mut Session, txns: Vec<Txn>, origin: &Origin) -> Result<Option<UndoRef>, CommandError> {
    if txns.is_empty() {
        return Ok(None);
    }
    frozen_guard(s, &txns)?;
    let mut undo_ref = None;
    let mut events = Vec::new();
    {
        let p = s.project.as_mut().ok_or_else(no_project)?;
        for t in txns {
            p.log_commit(t.rev, back_of(&t), t.before.clone(), earlier_revs(&t));
            let step_id = match t.kind {
                TxnKind::Step => {
                    let id = p.history.next_step_id();
                    p.history.push(Step {
                        id: id.clone(),
                        label: t.label.clone(),
                        origin: origin.clone(),
                        time: crate::time::now_iso(),
                        edit: t.edit,
                        touched: t.touched,
                        structural: t.structural,
                    });
                    undo_ref = Some(UndoRef { step_id: id.clone(), label: t.label.clone() });
                    id
                }
                TxnKind::Undo(i) => {
                    p.history.replace_edit(i, t.edit);
                    p.history.moved_back();
                    p.history.steps().get(i).map(|st| st.id.clone()).unwrap_or_default()
                }
                TxnKind::Redo(i) => {
                    p.history.replace_edit(i, t.edit);
                    p.history.moved_forward();
                    p.history.steps().get(i).map(|st| st.id.clone()).unwrap_or_default()
                }
                TxnKind::System => String::new(),
            };
            p.doc = t.after;
            let step_value = if t.kind == TxnKind::System { Value::Null } else { Value::String(step_id) };
            let origin_label = if t.kind == TxnKind::System { "system".to_owned() } else { origin.label() };
            events.push(json!({
                "undoStepId": step_value,
                "documentRev": t.rev,
                "label": t.label,
                "origin": origin_label,
                "changes": elide(&t.changes),
            }));
        }
    }
    for e in events {
        s.emit("doc.changed", e);
    }
    Ok(undo_ref)
}

/// Stages the file writes of a call about to commit: each is written to a temporary sibling of its target (C14-R1-03).
/// A failure removes what was staged and fails the call.
fn stage_files(effects: &mut [Effect]) -> Result<(), CommandError> {
    let mut failure = None;
    for e in effects.iter_mut() {
        if let Effect::WriteFile(w) = e
            && w.staged.is_none()
            && failure.is_none()
        {
            let tmp = tt_persist::fsx::temp_sibling(&w.path);
            if let Some(dir) = w.path.parent()
                && !dir.as_os_str().is_empty()
                && let Err(err) = std::fs::create_dir_all(dir)
            {
                failure = Some(CommandError::from(tt_persist::fsx::fs_error(&err, dir, "cannot create the folder")));
                continue;
            }
            let written = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .and_then(|mut f| std::io::Write::write_all(&mut f, &w.bytes).and_then(|()| f.sync_all()));
            match written {
                Ok(()) => w.staged = Some(tmp),
                Err(err) => {
                    let _ = std::fs::remove_file(&tmp);
                    failure = Some(CommandError::from(tt_persist::fsx::fs_error(&err, &w.path, "cannot write")));
                }
            }
        }
    }
    match failure {
        Some(err) => {
            discard_files(effects);
            Err(err)
        }
        None => Ok(()),
    }
}

/// Removes the staged file writes of a call that did not commit.
fn discard_files(effects: &[Effect]) {
    for e in effects {
        if let Effect::WriteFile(w) = e
            && let Some(tmp) = &w.staged
        {
            let _ = std::fs::remove_file(tmp);
        }
    }
}

/// Applies the deferred effects of a committed call. A simulation applies none. A file that cannot be published
/// (moved from its stage onto its target) after the commit is the call's error (C14-R2-01, ADR-0015 §12): every
/// other effect still applies, the target keeps what it had, and the stage is removed.
fn apply_effects(s: &mut Session, effects: Vec<Effect>, simulating: bool, origin: &Origin) -> Result<(), CommandError> {
    if simulating {
        return Ok(());
    }
    let mut failure: Option<CommandError> = None;
    for e in effects {
        match e {
            Effect::CancelJob(id) => {
                let _ = s.jobs.cancel(&id);
            }
            Effect::Event(ev) => s.events.push(ev),
            Effect::Close => crate::persistence::close(s),
            Effect::Install(i) => {
                // The handler checked the candidate; the gate runs again here (ADR-0013 §16).
                if gate(&i.project.doc).is_ok() {
                    install_into(s, LoadGrant { _private: () }, *i, origin);
                }
            }
            Effect::Save(p) => crate::persistence::apply_pending(s, &p),
            Effect::WriteFile(w) => {
                // The content is complete beside its target; a rename that fails after the commit leaves the target
                // as it was, removes the staged copy and fails the call.
                if let Some(tmp) = &w.staged {
                    match std::fs::rename(tmp, &w.path) {
                        Ok(()) => {
                            if w.private {
                                restrict_to_user(&w.path);
                            }
                        }
                        Err(err) => {
                            let _ = std::fs::remove_file(tmp);
                            failure.get_or_insert_with(|| {
                                CommandError::with_details(
                                    tt_persist::codes::FS_IO,
                                    format!("`{}` could not be published: {err}", w.path.display()),
                                    json!({ "path": w.path.to_string_lossy(), "reason": "publish" }),
                                )
                            });
                        }
                    }
                }
            }
            Effect::Prefs(values) => {
                // Only values that differ from the defaults are stored (01 §11; C13-R1-15).
                for (k, v) in values {
                    if crate::store::pref_default(&k).is_some_and(|d| same_value(&d, &v)) {
                        s.env.settings.prefs.shift_remove(&k);
                    } else {
                        s.env.settings.prefs.insert(k, v);
                    }
                }
                let _ = s.env.write("preferences");
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

/// Makes a file readable by its owner only (POSIX); on Windows the per-user settings folder is already private.
fn restrict_to_user(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// The envelope of a call whose transaction committed but whose file could not be published (C14-R2-01): an error
/// that keeps the commit's `changes`, `undo` and `documentRev`, and says in `details.committed` whether the document
/// changed (the change stays; undo reverts it).
fn publish_failed(mut env: Envelope, mut e: CommandError) -> Envelope {
    let committed = !env.changes.is_empty();
    if let Some(m) = e.details.as_object_mut() {
        m.insert("committed".into(), json!(committed));
        m.insert("documentRev".into(), json!(env.document_rev));
    }
    env.status = crate::types::Status::Error;
    env.result = None;
    env.error = Some(e);
    env
}

/// Two preference values are the same (numbers compare by value: `24` and `24.0` are one frame rate).
fn same_value(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < f64::EPSILON,
        _ => a == b,
    }
}

/// A project to install (C1.3): the open project with its store, the `doc.changed` of its load repairs (REP-01) and
/// its last-session UI state (10 §6.12).
#[derive(Debug, Clone)]
pub(crate) struct Install {
    pub project: OpenProject,
    pub repairs: Option<Value>,
    pub restore: Option<Value>,
}

/// The integrity gate (C12-R2-01): a candidate is installed only when the model reports no error (schemas,
/// references, REV-01/02, §5 limits, names, trees, exposures).
fn gate(doc: &Document) -> Result<(), CommandError> {
    let errors: Vec<tt_model::document::Issue> =
        doc.issues().into_iter().filter(|i| i.severity == tt_model::document::Severity::Error).collect();
    let Some(first) = errors.first() else { return Ok(()) };
    let listed: Vec<Value> = errors
        .iter()
        .take(50)
        .map(|i| json!({ "code": i.code, "objectId": i.object_id, "pointer": i.pointer, "message": i.message }))
        .collect();
    Err(CommandError::with_details(
        first.code,
        format!("the document is not installed: {} integrity error(s); first: {}", errors.len(), first.message),
        json!({ "count": errors.len(), "issues": listed }),
    ))
}

/// Replaces the open project — only with a [`LoadGrant`]. The replacement is recorded: `project.closed` for the
/// previous project, `project.opened` for the new one, and the `doc.changed` of its load repairs (origin `load`).
fn install_into(s: &mut Session, _grant: LoadGrant, inst: Install, origin: &Origin) -> Value {
    if let Some(old) = s.project.take() {
        crate::persistence::clear_cache(&s.env, &old);
        s.emit("project.closed", json!({ "projectId": old.doc.project().id().as_str() }));
    }
    let Install { project, repairs, restore } = inst;
    s.app = AppContext::for_document(&project.doc);
    if let Some(v) = &restore {
        crate::persistence::restore_session(&mut s.app, &project.doc, v);
    }
    let rev = project.doc.document_rev();
    let id = project.doc.project().id().as_str().to_owned();
    let path = project.path.clone();
    let read_only = project.read_only;
    let base = project.history.steps().first().map(|st| st.id.clone());
    s.project = Some(project);
    s.emit("project.opened", json!({ "projectId": id, "path": path, "readOnly": read_only, "origin": origin.label() }));
    if let Some(changes) = repairs {
        s.emit(
            "doc.changed",
            json!({ "undoStepId": base, "documentRev": rev, "label": "Repair on Open", "origin": "load", "changes": changes }),
        );
    }
    json!({ "projectId": id, "readOnly": read_only, "documentRev": rev })
}

fn changed_fields(a: &Value, b: &Value) -> Vec<String> {
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else { return Vec::new() };
    b.iter().filter(|(k, v)| a.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect()
}

/// `project.unsavedChanges` (01 §11.2 rule 3) for the open project of a session.
pub(crate) fn unsaved_changes(p: &OpenProject) -> CommandError {
    CommandError::with_details(
        "project.unsavedChanges",
        "the project has unsaved changes",
        json!({
            "projectId": p.doc.project().id().as_str(),
            "name": p.doc.project().name(),
            "path": p.path,
            "projectDirty": true,
            "globalLibraryDirty": false,
        }),
    )
    .hint("pass onUnsaved: \"save\" or \"discard\"")
}

impl Core {
    pub(crate) fn new(registry: &'static Registry, session: Session, budget: Duration) -> Self {
        Self {
            session: Some(session),
            requests: RequestCache::default(),
            registry,
            simulating: false,
            budget,
            served_from_cache: false,
        }
    }

    /// Whether the last `invoke` was answered from the `requestId` cache (C14-R2-03); clears the flag.
    pub(crate) fn take_served_from_cache(&mut self) -> bool {
        std::mem::take(&mut self.served_from_cache)
    }

    /// The origin a call runs with: an agent session registered in the hub gets its current name and grant.
    fn effective_origin(&self, origin: &Origin) -> Origin {
        if let Origin::Agent { session, .. } = origin
            && let Some(s) = self.session.as_ref().and_then(|x| x.agents.sessions.get(session))
        {
            return Origin::Agent { session: session.clone(), name: s.client.clone(), scopes: s.scopes.clone() };
        }
        origin.clone()
    }

    /// Registers an agent session (`agent.session.connected`).
    pub(crate) fn connect_agent(&mut self, c: crate::agents::AgentConnect) -> crate::agents::AgentSession {
        let roots = c.roots.clone();
        let Some(s) = self.session.as_mut() else {
            return crate::agents::AgentHub::default().connect(c, crate::time::now_iso());
        };
        let a = s.agents.connect(c, crate::time::now_iso());
        if !roots.is_empty() {
            s.env.roots.insert(a.session_id.clone(), roots);
        }
        s.emit(
            "agent.session.connected",
            json!({ "sessionId": a.session_id, "client": a.client, "policy": a.policy.as_str() }),
        );
        a
    }

    /// Removes an agent session (`agent.session.disconnected`).
    pub(crate) fn disconnect_agent(&mut self, session: &str, reason: &str) {
        if let Some(s) = self.session.as_mut()
            && s.agents.sessions.remove(session).is_some()
        {
            s.agents.pending.retain(|_, p| p.session != session);
            s.env.roots.remove(session);
            s.emit("agent.session.disconnected", json!({ "sessionId": session, "reason": reason }));
        }
    }

    /// The state an MCP server polls.
    pub(crate) fn agent_status(&self, session: &str) -> crate::agents::AgentStatus {
        self.session
            .as_ref()
            .map_or_else(|| crate::agents::AgentHub::default().status(session), |s| s.agents.status(session))
    }

    /// Appends one activity entry for an agent call (11 §11), and beside the project when persisting is on.
    pub(crate) fn audit(&mut self, entry: crate::agents::ActivityEntry) -> crate::agents::ActivityEntry {
        let Some(s) = self.session.as_mut() else { return entry };
        let e = s.agents.record(entry);
        if s.agents.persist
            && let Some(dir) =
                s.project.as_ref().and_then(|p| p.path.as_ref()).and_then(|p| crate::commands::agent::activity_file(p))
            && let Ok(line) = serde_json::to_string(&e)
        {
            let _ = crate::commands::agent::append_line(&dir, &line);
        }
        e
    }

    fn take_session(&mut self) -> Session {
        self.session.take().unwrap_or_default()
    }

    pub(crate) fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub(crate) fn session_mut(&mut self) -> Option<&mut Session> {
        self.session.as_mut()
    }

    pub(crate) fn registry(&self) -> &'static Registry {
        self.registry
    }

    fn rev(&self) -> Option<u64> {
        self.session.as_ref().and_then(|s| s.project.as_ref()).map(|p| p.doc.document_rev())
    }

    /// Repairs the context after a change and emits the state events.
    fn settle(s: &mut Session, before: &Observed) {
        if let Some(p) = &s.project {
            let doc = p.doc.clone();
            s.app.repair(&doc);
        }
        let after = observe(s);
        let changed = changed_fields(&before.state, &after.state);
        if !changed.is_empty() {
            if changed.iter().any(|c| c == "selection")
                && let Some(sel) = after.state.get("selection")
            {
                s.emit("edit.selection.changed", json!({ "context": sel.get("context"), "ids": sel.get("ids") }));
            }
            if changed.iter().any(|c| c == "workspace")
                && let Some(w) = after.state.get("workspace")
            {
                s.emit("workspace.changed", w.clone());
            }
            s.emit("app.state.changed", json!({ "changed": changed, "state": after.state }));
        }
        if after.history != before.history
            && let Some(p) = &s.project
        {
            let top = p.history.top();
            s.emit(
                "edit.history.changed",
                json!({
                    "position": p.history.position(),
                    "entryCount": p.history.len(),
                    "lastLabel": top.map(|t| t.label.clone()),
                    "source": top.map(|t| t.origin.source()),
                }),
            );
        }
        if after.dirty != before.dirty
            && let Some(d) = after.dirty
        {
            s.emit("project.dirty.changed", json!({ "projectDirty": d, "globalLibraryDirty": false }));
        }
        if after.workspaces != before.workspaces {
            let _ = tt_persist::settings::write(&s.env.dirs.settings, "workspaces", &after.workspaces);
        }
    }

    /// One invocation, with `requestId` idempotency (C12-R1-06/22).
    pub(crate) fn invoke(&mut self, inv: &Invocation) -> Envelope {
        self.served_from_cache = false;
        let Some(id) = inv.options.request_id.clone().filter(|_| !self.simulating) else { return self.run(inv) };
        let session = inv.origin.session_key();
        let fp = fingerprint(inv);
        match self.requests.lookup(&session, &id, fp) {
            // The remembered outcome is a confirmation and this retry carries its token (by the agent, or by the
            // server after an accepted or declined elicitation): the logical call is settled now, and its final
            // outcome replaces the remembered one, so a later retry of the requestId returns it (C14-R1-04).
            Lookup::Hit(env)
                if env.status == crate::types::Status::NeedsConfirmation
                    && inv.options.confirm_token.is_some()
                    && env.result.as_ref().and_then(|r| r.get("confirmToken")).and_then(Value::as_str)
                        == inv.options.confirm_token.as_deref() =>
            {
                let settled = self.run(inv);
                self.requests.settle(&session, &id, settled.clone());
                settled
            }
            Lookup::Hit(env) => {
                self.served_from_cache = true;
                *env
            }
            Lookup::Refused(e) => Envelope::error(e, self.rev()),
            // Only `job.wait` is in flight across calls, and it is answered by `wait_begin`; an identical call that
            // reaches this route while its twin is pending is told so rather than run twice.
            Lookup::InFlight => Envelope::error(
                CommandError::with_details(
                    "request.conflict",
                    format!("requestId `{id}` is in use by a call in progress"),
                    json!({ "requestId": id, "originalCommand": inv.command, "inProgress": true }),
                ),
                self.rev(),
            ),
            Lookup::Miss => {
                let env = self.run(inv);
                self.requests.insert(&session, &id, fp, &inv.command, env.clone());
                env
            }
        }
    }

    /// The session limits of agent submissions (11 §6.5), checked once here for every entry point: protocol
    /// wrappers, typed tools, batches and batch scripts (C14-R1-11). `steps`: the size of a batch being admitted;
    /// `job`: a job command being admitted.
    fn admit(&self, origin: &Origin, steps: Option<usize>, job: Option<&str>) -> Result<(), CommandError> {
        let Origin::Agent { .. } = origin else { return Ok(()) };
        let Some(s) = self.session.as_ref() else { return Ok(()) };
        let limits = &s.agents.limits;
        if let Some(n) = steps
            && n > limits.max_batch_steps as usize
        {
            return Err(CommandError::with_details(
                codes::LIMIT_EXCEEDED,
                format!("a batch of this session has at most {} steps", limits.max_batch_steps),
                json!({ "limit": "maxBatchSteps", "value": limits.max_batch_steps, "actual": n }),
            ));
        }
        if let Some(command) = job {
            let running = s.jobs.list(None, Some(&origin.session_key())).iter().filter(|j| !j.state.is_final()).count();
            if running >= limits.max_jobs as usize {
                return Err(CommandError::with_details(
                    codes::LIMIT_EXCEEDED,
                    format!("this session already runs {running} job(s); the limit is {}", limits.max_jobs),
                    json!({ "limit": "maxJobs", "value": limits.max_jobs, "commandId": command }),
                )
                .hint("wait for a job with job_wait, or cancel one"));
            }
        }
        Ok(())
    }

    fn run(&mut self, inv: &Invocation) -> Envelope {
        // An agent session's grant is the hub's (11 §6.1; `agent.session.grant` applies to the next call).
        let resolved = Invocation { origin: self.effective_origin(&inv.origin), ..inv.clone() };
        let inv = &resolved;
        let Some(entry) = self.registry.get(&inv.command).copied() else {
            return Envelope::error(unknown(&inv.command, self.registry), self.rev());
        };
        if !self.simulating
            && !inv.options.dry_run
            && let Err(e) =
                self.admit(&inv.origin, None, (entry.spec.kind == Kind::Job).then_some(inv.command.as_str()))
        {
            return Envelope::error(e, self.rev());
        }
        let session = self.take_session();
        let before = observe(&session);
        let rollback = Rollback::take(&session);
        // A command that declares its own `dryRun` parameter (doc.object.patch, doc.import.json, doc.repair) is a dry
        // run when it is set, exactly as with the invocation option.
        let dry = inv.options.dry_run || param_dry_run(&entry, &inv.params);
        let mut cx = Cx::new(session, inv.origin.clone(), entry.id(), dry);
        cx.set_confirmation(
            inv.options.confirm_token.clone(),
            inv.options.confirm,
            crate::agents::fingerprint(entry.id(), &inv.params),
        );
        let outcome = run_step(&mut cx, &entry, inv.params.clone(), &inv.options).and_then(|result| {
            let label = cx
                .label()
                .map(str::to_owned)
                .or_else(|| entry.command.and_then(|c| c.decl.undo).map(str::to_owned))
                .unwrap_or_else(|| entry.spec.summary.to_owned());
            let mut txns = cx.take_txns();
            if let Some(t) = cx.finish_staged(label)? {
                txns.push(t);
            }
            Ok((result, txns))
        });
        let warnings = cx.take_warnings();
        let targets = cx.take_targets();
        let mut effects = cx.take_effects();
        let mut session = cx.into_session();
        let rev_before = session.project.as_ref().map(|p| p.doc.document_rev());
        let env = match outcome {
            Err(e) => {
                rollback.apply(&mut session);
                let mut env = needs_confirmation(&mut session, e, rev_before);
                env.warnings = warnings;
                env
            }
            Ok((result, txns)) => {
                let changes: Vec<ChangeRecord> = txns.iter().flat_map(|t| t.changes.iter().cloned()).collect();
                let last_rev = txns.last().map(|t| t.rev);
                let (changes, truncated, since) = cap(changes, rev_before.unwrap_or(0));
                let mut env = Envelope::ok(result, rev_before);
                env.targets = targets;
                env.warnings = warnings;
                env.changes = changes;
                env.changes_truncated = truncated;
                env.changes_since = since;
                if dry {
                    let refused = frozen_guard(&session, &txns);
                    rollback.apply(&mut session);
                    env.dry_run = true;
                    if let Err(e) = refused {
                        env = Envelope::error(e, rev_before);
                    }
                } else {
                    // Pending saves are written only now that every step succeeded, and before anything commits; a
                    // failed write rolls the invocation back (C13-R1-01).
                    let pre = frozen_guard(&session, &txns)
                        .and_then(|()| self.write_pending(&session, txns.last().map(|t| &t.after), &mut effects));
                    match pre.and_then(|()| commit(&mut session, txns, &inv.origin)) {
                        Ok(u) => {
                            env.undo = u;
                            env.document_rev = last_rev.or(rev_before);
                            if let Err(e) = apply_effects(&mut session, effects, self.simulating, &inv.origin) {
                                env = publish_failed(env, e);
                            }
                        }
                        Err(e) => {
                            discard_files(&effects);
                            rollback.apply(&mut session);
                            env = Envelope::error(e, rev_before);
                        }
                    }
                }
                if env.document_rev.is_none() {
                    env.document_rev = session.project.as_ref().map(|p| p.doc.document_rev());
                }
                env
            }
        };
        if !dry {
            Self::settle(&mut session, &before);
        }
        self.session = Some(session);
        env
    }

    /// Runs `f` on a clone of the session and discards everything it did (a dry-run sequential batch, C12-R1-03).
    fn simulate<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = self.session.clone();
        let was = self.simulating;
        self.simulating = true;
        let out = f(self);
        self.simulating = was;
        self.session = saved;
        out
    }

    /// A batch (11 §7.3).
    pub(crate) fn batch(&mut self, req: &BatchRequest, origin: &Origin) -> Envelope {
        let resolved = self.effective_origin(origin);
        let origin = &resolved;
        if req.steps.is_empty() || req.steps.len() > MAX_BATCH_STEPS {
            return Envelope::error(
                CommandError::with_details(
                    if req.steps.is_empty() { codes::ARG_INVALID } else { codes::LIMIT_EXCEEDED },
                    format!("a batch has 1–{MAX_BATCH_STEPS} steps"),
                    json!({ "pointer": "/steps", "limit": MAX_BATCH_STEPS, "actual": req.steps.len() }),
                ),
                self.rev(),
            );
        }
        if let Err(e) = self.admit(origin, Some(req.steps.len()), None) {
            return Envelope::error(e, self.rev());
        }
        if req.atomic
            && let Some((i, s)) = req
                .steps
                .iter()
                .enumerate()
                .find(|(_, s)| self.registry.get(&s.id).is_some_and(|e| e.spec.kind == Kind::Job))
        {
            return Envelope::error(
                CommandError::with_details(
                    codes::BATCH_JOB_NOT_ALLOWED,
                    format!("step {} (`{}`) is a job; jobs cannot run inside an atomic batch", i + 1, s.id),
                    json!({ "stepIndex": i + 1, "commandId": s.id }),
                ),
                self.rev(),
            );
        }
        if req.atomic { self.batch_atomic(req, origin) } else { self.batch_sequential(req, origin) }
    }

    fn batch_sequential(&mut self, req: &BatchRequest, origin: &Origin) -> Envelope {
        if req.dry_run && !self.simulating {
            // Run the real sequence on a clone of the session: every step sees the simulated earlier steps, and
            // nothing leaves the clone (C12-R1-03).
            let mut real = req.clone();
            real.dry_run = false;
            let rev = self.rev();
            let mut env = self.simulate(|core| core.batch_sequential(&real, origin));
            env.dry_run = true;
            env.undo = None;
            env.document_rev = rev;
            for s in env.steps.iter_mut().flatten() {
                s.dry_run = true;
                s.undo = None;
            }
            return env;
        }
        let started = std::time::Instant::now();
        let mut steps: Vec<Envelope> = Vec::new();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        let mut failed: Option<(usize, CommandError)> = None;
        for (i, step) in req.steps.iter().enumerate() {
            let env = match resolve_refs(&step.params, &steps, &names) {
                Ok(params) => {
                    let inv = Invocation {
                        command: step.id.clone(),
                        params,
                        options: InvokeOptions::default(),
                        origin: origin.clone(),
                    };
                    let env = self.invoke(&inv);
                    self.finish_batched_wait(&inv, env, started)
                }
                Err(e) => Envelope::error(e, self.rev()),
            };
            if let Some(n) = &step.name {
                names.insert(n.clone(), i);
            }
            let ok = env.is_ok();
            if !ok && failed.is_none() {
                failed = env.error.clone().map(|e| (i, e));
            }
            steps.push(env);
            if !ok && req.stop_on_error {
                break;
            }
        }
        let mut env = match failed {
            Some((i, e)) => Envelope::error(step_failed(i, &req.steps[i].id, e), self.rev()),
            None => Envelope::ok(json!({ "stepCount": steps.len() }), self.rev()),
        };
        env.changes = steps.iter().flat_map(|s| s.changes.iter().cloned()).take(CHANGES_CAP).collect();
        env.steps = Some(steps);
        env
    }

    /// A `job.wait` step of a sequential batch waits like a direct call (C12-R2-07): on the job table's condition
    /// variable for its `timeoutMs`, bounded by what remains of the watchdog budget of the batch call (minus a margin
    /// for the rest of the batch), so the caller still gets an answer. Ownership and parameters were checked by the
    /// invocation that returned the pending record.
    fn finish_batched_wait(&mut self, inv: &Invocation, env: Envelope, started: std::time::Instant) -> Envelope {
        let pending = env.result.as_ref().is_some_and(|r| r.get("timedOut") == Some(&Value::Bool(true)));
        if inv.command != "job.wait" || !env.is_ok() || !pending {
            return env;
        }
        let Ok(p) = tt_registry::decode_params::<crate::commands::job::WaitParams>(inv.params.clone()) else {
            return env;
        };
        let margin = self.budget / 10;
        let left = self.budget.saturating_sub(started.elapsed()).saturating_sub(margin);
        let wait = Duration::from_millis(p.timeout_ms).min(left);
        if wait.is_zero() {
            return env;
        }
        let Some(jobs) = self.session.as_ref().map(|s| s.jobs.clone()) else { return env };
        let outcome = jobs
            .wait(&p.job_id, wait)
            .and_then(|(record, timed_out)| crate::commands::job::wait_result(record, timed_out));
        match outcome {
            Ok(result) => Envelope::ok(result, self.rev()),
            Err(e) => Envelope::error(e, self.rev()),
        }
    }

    fn batch_atomic(&mut self, req: &BatchRequest, origin: &Origin) -> Envelope {
        let session = self.take_session();
        let before = observe(&session);
        let rollback = Rollback::take(&session);
        let rev_before = session.project.as_ref().map(|p| p.doc.document_rev());
        let mut cx = Cx::new(session, origin.clone(), "batch", req.dry_run);
        cx.set_atomic_batch(true);
        let mut steps: Vec<Envelope> = Vec::new();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        let mut failed: Option<(usize, CommandError)> = None;
        let opts = InvokeOptions { dry_run: req.dry_run, ..InvokeOptions::default() };
        for (i, step) in req.steps.iter().enumerate() {
            cx.set_step(Some(i));
            let mut preview = false;
            let mut savepoint = None;
            let outcome =
                resolve_refs(&step.params, &steps, &names).and_then(|params| match self.registry.get(&step.id) {
                    Some(entry) => {
                        // A step that sets its own `dryRun` previews inside the batch and leaves no trace in it
                        // (10 AG-02; C12-R1-03).
                        preview = param_dry_run(entry, &params);
                        if preview {
                            savepoint = Some(cx.savepoint());
                        }
                        run_step(&mut cx, entry, params, &opts)
                    }
                    None => Err(unknown(&step.id, self.registry)),
                });
            if let Some(n) = &step.name {
                names.insert(n.clone(), i);
            }
            let rev_now = cx.predicted_rev().ok().or(rev_before);
            if let Some(sp) = savepoint {
                cx.rollback_to(sp);
            }
            match outcome {
                Ok(result) => {
                    let mut env = Envelope::ok(result, rev_now);
                    env.warnings = cx.take_warnings();
                    env.targets = cx.take_targets();
                    env.dry_run = preview;
                    steps.push(env);
                }
                Err(e) => {
                    let mut env = Envelope::error(e.clone(), rev_before);
                    env.warnings = cx.take_warnings();
                    steps.push(env);
                    failed = Some((i, e));
                    break;
                }
            }
        }
        cx.set_step(None);
        // A save inside the batch stamps `modifiedAt` into the batch's own changes: one transaction, one undo step,
        // and the written file is the revision the batch commits (C13-R2-01).
        if failed.is_none()
            && !req.dry_run
            && let Some(now) = cx.pending_batch_save()
            && let Err(e) = crate::persistence::stamp_staged(&mut cx, &now)
        {
            failed = Some((req.steps.len().saturating_sub(1), e));
        }
        let committed = match failed {
            Some((i, e)) => Err(step_failed(i, &req.steps[i].id, e)),
            None => {
                // Anything sealed during the batch belongs to its single commit.
                let sealed = cx.take_txns();
                if sealed.is_empty() {
                    cx.finish_staged(req.label.clone())
                } else {
                    Err(CommandError::new(codes::INTERNAL, "an atomic batch sealed a transaction"))
                }
            }
        };
        let mut effects = cx.take_effects();
        let mut session = cx.into_session();
        let env = match committed {
            Err(e) => {
                rollback.apply(&mut session);
                let mut env = Envelope::error(e, rev_before);
                env.steps = Some(steps);
                env.dry_run = req.dry_run;
                env
            }
            Ok(txn) => {
                let changes = txn.as_ref().map(|t| t.changes.clone()).unwrap_or_default();
                let last_rev = txn.as_ref().map(|t| t.rev);
                let (changes, truncated, since) = cap(changes, rev_before.unwrap_or(0));
                let mut env = Envelope::ok(json!({ "stepCount": steps.len() }), rev_before);
                env.changes = changes;
                env.changes_truncated = truncated;
                env.changes_since = since;
                if req.dry_run {
                    let refused = txn.as_ref().map_or(Ok(()), |t| frozen_guard(&session, std::slice::from_ref(t)));
                    rollback.apply(&mut session);
                    env.dry_run = true;
                    if let Err(e) = refused {
                        env = Envelope::error(e, rev_before);
                    }
                } else {
                    let pre = txn
                        .as_ref()
                        .map_or(Ok(()), |t| frozen_guard(&session, std::slice::from_ref(t)))
                        .and_then(|()| self.write_pending(&session, txn.as_ref().map(|t| &t.after), &mut effects));
                    let committed = pre.and_then(|()| match txn {
                        Some(t) => commit(&mut session, vec![t], origin),
                        None => Ok(None),
                    });
                    match committed {
                        Ok(u) => {
                            env.undo = u;
                            env.document_rev = last_rev.or(rev_before);
                            if let Err(e) = apply_effects(&mut session, effects, self.simulating, origin) {
                                env = publish_failed(env, e);
                            }
                        }
                        Err(e) => {
                            discard_files(&effects);
                            rollback.apply(&mut session);
                            env = Envelope::error(e, rev_before);
                        }
                    }
                }
                env.steps = Some(steps);
                env
            }
        };
        if !req.dry_run {
            Self::settle(&mut session, &before);
        }
        self.session = Some(session);
        env
    }

    /// The guarded load transaction (ADR-0013 §16; C12-R1-01): the guards of the registered `project.open` — the
    /// caller's scopes and the unsaved-changes rule of 01 §11.2 — then installation with a [`LoadGrant`].
    pub(crate) fn load(&mut self, doc: Document, opts: &OpenOptions, origin: &Origin) -> Envelope {
        let project = OpenProject::new(doc, opts.path.clone(), opts.read_only);
        self.install_guarded(Install { project, repairs: None, restore: None }, opts.on_unsaved, origin, None)
    }

    /// Installs a candidate project after the guards: the scopes of `project.open`, the unsaved-changes policy of the
    /// open project (`save` saves it now), the integrity gate, and — for an open job — the job's commit point, so a
    /// cancelled open installs nothing (LOAD-02).
    pub(crate) fn install_guarded(
        &mut self,
        inst: Install,
        on_unsaved: UnsavedPolicy,
        origin: &Origin,
        commit: Option<&crate::jobs::JobProgress>,
    ) -> Envelope {
        let rev = self.rev();
        let Some(entry) = self.registry.get("project.open").copied() else {
            return Envelope::error(CommandError::internal("project.open is not registered"), rev);
        };
        let missing = missing_scopes(&entry, origin);
        if !missing.is_empty() {
            return Envelope::error(
                CommandError::with_details(
                    codes::PERMISSION_DENIED,
                    format!("opening a project needs the scopes {missing:?}"),
                    json!({ "commandId": "project.open", "missingScopes": missing }),
                ),
                rev,
            );
        }
        if let Some(p) = self.session.as_ref().and_then(|s| s.project.as_ref())
            && p.dirty()
        {
            match on_unsaved {
                UnsavedPolicy::Fail => return Envelope::error(unsaved_changes(p), rev),
                UnsavedPolicy::Save => {
                    if let Err(e) = self.save_open_project(origin) {
                        return Envelope::error(e, self.rev());
                    }
                }
                // Discarding unsaved work is the `app` scope's (11 §6.1), which is never an agent's by itself.
                UnsavedPolicy::Discard => {
                    if let Origin::Agent { scopes, .. } = origin
                        && !scopes.contains(&tt_registry::Scope::App)
                    {
                        return Envelope::error(
                            CommandError::with_details(
                                codes::PERMISSION_DENIED,
                                "discarding unsaved changes needs the scope app",
                                json!({ "commandId": "project.open", "missingScopes": ["app"] }),
                            ),
                            rev,
                        );
                    }
                }
            }
        }
        if let Err(e) = gate(&inst.project.doc) {
            return Envelope::error(e, rev);
        }
        if let Some(c) = commit
            && !c.commit()
        {
            return Envelope::error(
                CommandError::with_details("job.cancelled", "the open was cancelled", json!({})),
                rev,
            );
        }
        let mut session = self.take_session();
        let before = observe(&session);
        // The project being replaced is discarded (its unsaved changes were dealt with above).
        if let Some(old) = session.project.as_ref() {
            let _ = tt_persist::recovery::delete(&session.env.dirs.recovery, &old.store.recovery_id);
        }
        let result = install_into(&mut session, LoadGrant { _private: () }, inst, origin);
        Self::settle(&mut session, &before);
        let rev = session.project.as_ref().map(|p| p.doc.document_rev());
        self.session = Some(session);
        Envelope::ok(result, rev)
    }

    /// Writes the pending saves of an invocation about to commit `final_doc` (`None`: nothing to commit, the live
    /// document is saved); none while simulating.
    fn write_pending(
        &self,
        s: &Session,
        final_doc: Option<&Document>,
        effects: &mut [Effect],
    ) -> Result<(), CommandError> {
        if self.simulating {
            return Ok(());
        }
        stage_files(effects)?;
        if !effects.iter().any(|e| matches!(e, Effect::Save(_))) {
            return Ok(());
        }
        let live = s.project.as_ref().map(|p| p.doc.clone());
        match final_doc.cloned().or(live) {
            Some(doc) => crate::persistence::write_pending(s, &doc, effects),
            None => Err(crate::cx::no_project()),
        }
    }

    /// Saves the open project to its location (the `onUnsaved: "save"` of the load path), as an internal invocation
    /// of `project.save`: its `modifiedAt` stamp commits like any transaction.
    fn save_open_project(&mut self, origin: &Origin) -> Result<(), CommandError> {
        let session = self.take_session();
        let mut cx = Cx::new(session, origin.clone(), "project.save", false);
        // The load path's internal save has no call of its own to confirm: an external change refuses it
        // (`project.changedOnDisk`); the commands that offer `onUnsaved: "save"` confirm before reaching it.
        let outcome = crate::persistence::save_now(&mut cx, false, false);
        let txns = cx.take_txns();
        let mut effects = cx.take_effects();
        let mut session = cx.into_session();
        let committed = outcome
            .and_then(|()| self.write_pending(&session, txns.last().map(|t| &t.after), &mut effects))
            .and_then(|()| commit(&mut session, txns, origin).map(|_| ()));
        let committed = match committed {
            Ok(()) => apply_effects(&mut session, effects, self.simulating, origin),
            Err(e) => {
                discard_files(&effects);
                Err(e)
            }
        };
        self.session = Some(session);
        committed
    }

    /// The first half of `job.wait` (C12-R1-04): the whole invocation contract on the document thread — remembered
    /// requests, scopes, parameters, `expectRev`, ownership — answering at once when the job is final or the timeout
    /// is zero.
    pub(crate) fn wait_begin(&mut self, inv: &Invocation) -> WaitStart {
        if let Some(id) = &inv.options.request_id {
            match self.requests.lookup(&inv.origin.session_key(), id, fingerprint(inv)) {
                Lookup::Hit(env) => return WaitStart::Done(env),
                Lookup::Refused(e) => return WaitStart::Done(Box::new(Envelope::error(e, self.rev()))),
                Lookup::InFlight => {
                    let timeout = tt_registry::decode_params::<crate::commands::job::WaitParams>(inv.params.clone())
                        .map_or(Duration::ZERO, |p| Duration::from_millis(p.timeout_ms));
                    if let Some(envelope) = self.requests.attach(&inv.origin.session_key(), id) {
                        return WaitStart::Attached { envelope, timeout };
                    }
                }
                Lookup::Miss => {}
            }
        }
        let env = self.run(inv);
        let running = env.result.as_ref().is_some_and(|r| r.get("timedOut") == Some(&Value::Bool(true)));
        if env.is_ok()
            && running
            && let Ok(p) = tt_registry::decode_params::<crate::commands::job::WaitParams>(inv.params.clone())
            && p.timeout_ms > 0
        {
            if let Some(id) = &inv.options.request_id
                && !self.simulating
            {
                self.requests.reserve(&inv.origin.session_key(), id, fingerprint(inv), &inv.command);
            }
            return WaitStart::Wait { job_id: p.job_id, timeout: Duration::from_millis(p.timeout_ms) };
        }
        self.remember(inv, &env);
        WaitStart::Done(Box::new(env))
    }

    /// The second half of `job.wait`: the envelope for the record the caller waited for.
    pub(crate) fn wait_finish(&mut self, inv: &Invocation, outcome: Result<Value, CommandError>) -> Envelope {
        let env = match outcome {
            Ok(result) => Envelope::ok(result, self.rev()),
            Err(e) => Envelope::error(e, self.rev()),
        };
        self.remember(inv, &env);
        if let Some(id) = &inv.options.request_id
            && !self.simulating
        {
            self.requests.release(&inv.origin.session_key(), id, &env);
        }
        env
    }

    fn remember(&mut self, inv: &Invocation, env: &Envelope) {
        if let Some(id) = &inv.options.request_id
            && !self.simulating
        {
            self.requests.insert(&inv.origin.session_key(), id, fingerprint(inv), &inv.command, env.clone());
        }
    }

    /// Drains pending events.
    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        self.session.as_mut().map(|s| std::mem::take(&mut s.events)).unwrap_or_default()
    }
}

fn step_failed(i: usize, id: &str, e: CommandError) -> CommandError {
    CommandError::with_details(
        codes::BATCH_STEP_FAILED,
        format!("step {} (`{id}`) failed: {}", i + 1, e.message),
        json!({ "stepIndex": i + 1, "commandId": id, "error": e }),
    )
}

/// Replaces batch references in parameters: a string that is exactly `$<n>.<path>` (step `n`, 1-based) or
/// `$<name>.<path>` (a step named with `as`) becomes the value at that dotted path of the step's envelope
/// (11 §7.3; ADR-0013 §6).
pub(crate) fn resolve_refs(
    params: &Value,
    done: &[Envelope],
    names: &BTreeMap<String, usize>,
) -> Result<Value, CommandError> {
    fn walk(v: &Value, done: &[Envelope], names: &BTreeMap<String, usize>, at: &str) -> Result<Value, CommandError> {
        match v {
            Value::String(s) if s.starts_with('$') && s.contains('.') => {
                let Some((head, path)) = s[1..].split_once('.') else { return Ok(v.clone()) };
                let index = match head.parse::<usize>() {
                    Ok(n) if n >= 1 => Some(n - 1),
                    Ok(_) => None,
                    Err(_) => names.get(head).copied(),
                };
                let bad = |why: &str| {
                    CommandError::with_details(
                        codes::ARG_INVALID,
                        format!("batch reference `{s}` {why}"),
                        json!({ "pointer": at, "reference": s }),
                    )
                };
                let Some(env) = index.and_then(|i| done.get(i)) else {
                    return Err(bad("names no earlier step"));
                };
                let mut cur = serde_json::to_value(env).map_err(|e| CommandError::internal(e.to_string()))?;
                for seg in path.split('.') {
                    cur = match (&cur, seg.parse::<usize>()) {
                        (Value::Array(a), Ok(i)) => a.get(i).cloned(),
                        (Value::Object(m), _) => m.get(seg).cloned(),
                        _ => None,
                    }
                    .ok_or_else(|| bad("names no value"))?;
                }
                Ok(cur)
            }
            Value::Array(a) => a
                .iter()
                .enumerate()
                .map(|(i, x)| walk(x, done, names, &format!("{at}/{i}")))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Value::Object(m) => {
                let mut out = Map::new();
                for (k, x) in m {
                    out.insert(k.clone(), walk(x, done, names, &format!("{at}/{}", tt_model::pointer::escape(k)))?);
                }
                Ok(Value::Object(out))
            }
            other => Ok(other.clone()),
        }
    }
    walk(params, done, names, "")
}

impl Envelope {
    /// `true` when the envelope reports a failure.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.status == EnvStatus::Error
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test")]

    use serde_json::json;
    use tt_model::change::PatchOp;
    use tt_model::document::EntryKey;

    use super::*;
    use crate::types::Invocation;

    fn core() -> Core {
        let doc = tt_model::fixture::sample_document(1).unwrap();
        let session = Session { project: Some(OpenProject::new(doc, None, false)), ..Session::default() };
        Core::new(crate::registry().unwrap(), session, Duration::from_secs(30))
    }

    fn doc(core: &Core) -> Document {
        core.session().unwrap().project.as_ref().unwrap().doc.clone()
    }

    /// Stages raw entry patches as one new step (what an M2 area command will do through `Cx::patch_entries`).
    fn structural(core: &mut Core, changes: Vec<(EntryKey, Vec<PatchOp>)>) -> Vec<ChangeRecord> {
        let session = core.take_session();
        let mut cx = Cx::new(session, Origin::User, "test.structural", false);
        cx.patch_entries(changes).unwrap();
        let txn = cx.finish_staged("Add Stroke".into()).unwrap().unwrap();
        let changes = txn.changes.clone();
        let mut session = cx.into_session();
        commit(&mut session, vec![txn], &Origin::User).unwrap();
        core.session = Some(session);
        changes
    }

    fn d0_pointer(d: &Document, key: &str) -> String {
        d.object_pointer(key).unwrap().2
    }

    fn op(op: &'static str, path: &str, value: Option<Value>) -> PatchOp {
        PatchOp { op, from: None, path: path.into(), range: None, value, old_value: None, elided: false }
    }

    #[test]
    fn created_objects_survive_undo_redo_cycles_with_rising_revisions() {
        let mut core = core();
        let d0 = doc(&core);
        let mut strokes: Vec<String> = d0
            .locations()
            .filter(|l| l.ty == tt_model::ObjectType::Stroke)
            .map(|l| l.key.clone())
            .filter(|k| d0.object_raw_json(k).unwrap().1["kind"] == "pencil")
            .collect();
        strokes.sort();
        let stroke_key = strokes[0].clone();
        let (entry, _, at) = d0.object_pointer(&stroke_key).unwrap();
        let EntryKey::Drawing(drawing) = entry.clone() else { panic!("stroke outside a drawing") };
        let (_, mut stroke) = d0.object_raw_json(&stroke_key).unwrap();
        stroke["id"] = json!("s_c12_new");
        stroke["rev"] = json!(1);
        let strokes = at.rsplit_once('/').unwrap().0.to_owned();
        let changes =
            structural(&mut core, vec![(entry.clone(), vec![op("add", &format!("{strokes}/-"), Some(stroke))])]);
        let new_key = format!("{drawing}/s_c12_new");
        assert!(changes.iter().any(|c| c.id == new_key && c.op == "created"), "{changes:?}");
        let r1 = doc(&core).document_rev();
        assert_eq!(doc(&core).locate(&new_key).unwrap().rev, r1, "a created object carries its transaction's rev");
        // Edit it through the property path, then undo both and redo both, twice.
        let mut swatches: Vec<String> =
            d0.locations().filter(|l| l.ty == tt_model::ObjectType::Swatch).map(|l| l.key.clone()).collect();
        swatches.sort();
        let swatch = swatches[0].clone();
        let env = core.invoke(&Invocation::user(
            "doc.object.patch",
            json!({ "targets": [new_key], "props": { "colourId": swatch } }),
        ));
        assert!(env.is_ok(), "{:?}", env.error);
        // The creation step's undo is one operation, not a copy of the layer (ID-keyed list diff).
        let p = core.session().unwrap().project.as_ref().unwrap();
        let StepEdit::Patches(commits) = &p.history.steps()[0].edit else { panic!("patch step") };
        assert_eq!(commits[0].undo.len(), 1, "{:?}", commits[0].undo);
        assert_eq!(commits[0].undo[0].op, "remove");
        // A step that replaces the whole stroke object (as a tool may) records the stroke's pre-stamp `rev` in its
        // patches; once stamped, the stored undo names a revision the live stroke no longer has. Replays align it
        // modulo rev (ADR-0013 §4; `align::tests` shows a checked precondition), and undo/redo work.
        let new_at = d0_pointer(&doc(&core), &new_key);
        let (_, mut replaced) = doc(&core).object_raw_json(&new_key).unwrap();
        replaced["rev"] = json!(1);
        replaced["meta"] = json!({ "replaced": true });
        structural(&mut core, vec![(entry.clone(), vec![op("replace", &new_at, Some(replaced))])]);
        let p = core.session().unwrap().project.as_ref().unwrap();
        let StepEdit::Patches(commits) = &p.history.steps().last().unwrap().edit else { panic!("patch step") };
        let undo = commits[0].undo.clone();
        assert!(undo.iter().any(|o| o.path.ends_with("/rev")), "{undo:?}");
        let aligned = crate::align::prepare(&doc(&core), &entry, &undo).unwrap();
        assert!(doc(&core).with_patched_entries(&[(entry.clone(), aligned)]).is_ok(), "aligned replay applies");
        let env = core.invoke(&Invocation::user("edit.undo", json!({})));
        assert!(env.is_ok(), "{:?}", env.error);
        assert!(doc(&core).object_raw_json(&new_key).unwrap().1.get("meta").is_none());
        assert!(core.invoke(&Invocation::user("edit.redo", json!({}))).is_ok());
        assert_eq!(doc(&core).object_raw_json(&new_key).unwrap().1["meta"]["replaced"], true);
        assert!(core.invoke(&Invocation::user("edit.undo", json!({}))).is_ok());
        let mut last = doc(&core).document_rev();
        for _ in 0..2 {
            for _ in 0..2 {
                let env = core.invoke(&Invocation::user("edit.undo", json!({})));
                assert!(env.is_ok(), "undo: {:?}", env.error);
                assert_eq!(env.document_rev, Some(last + 1));
                last += 1;
            }
            assert!(doc(&core).locate(&new_key).is_none(), "the created stroke is gone again");
            for _ in 0..2 {
                let env = core.invoke(&Invocation::user("edit.redo", json!({})));
                assert!(env.is_ok(), "redo: {:?}", env.error);
                last += 1;
            }
            let now = doc(&core);
            let loc = now.locate(&new_key).unwrap();
            assert_eq!(loc.rev, last, "redo re-creates and stamps it");
            assert_eq!(now.object_raw_json(&new_key).unwrap().1["colourId"], json!(swatch));
            assert_eq!(now.locate(drawing.as_str()).unwrap().rev, last, "and the drawing with it");
        }
        // doc.diff reconstructs earlier revisions through the same replay.
        let env = core.invoke(&Invocation::user("doc.diff", json!({ "sinceRev": r1 - 1 })));
        assert!(env.is_ok(), "{:?}", env.error);
    }
}
