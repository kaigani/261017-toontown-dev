//! The transaction pipeline on the document thread (11 §2): validate → enablement → permission → execute as a
//! transaction → record the undo step → emit change events; plus `dryRun`, `expectRev`, `requestId` and batches
//! (11 §7.2–§7.3).

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Map, Value, json};
use tt_model::change::ELIDE_BYTES;
use tt_model::document::Document;
use tt_registry::{CommandError, Entry, Kind, Registry, Status, codes, validate_params};

use crate::app::AppContext;
use crate::cx::{Cx, Txn, TxnKind, no_project};
use crate::history::{Step, StepEdit};
use crate::session::{Back, Checkpoint, OpenProject, Session};
use crate::types::{
    BatchRequest, ChangeRecord, Envelope, Invocation, InvokeOptions, Origin, Status as EnvStatus, UndoRef,
};
use crate::ui::UiState;
use crate::{CHANGES_CAP, MAX_BATCH_STEPS};

/// Remembered `requestId` results per session (11 §7.2).
const REQUEST_CACHE_LEN: usize = 10_000;

#[derive(Debug, Default)]
struct RequestCache {
    order: VecDeque<(String, String)>,
    results: BTreeMap<(String, String), Envelope>,
}

impl RequestCache {
    fn get(&self, key: &(String, String)) -> Option<&Envelope> {
        self.results.get(key)
    }

    fn insert(&mut self, key: (String, String), env: Envelope) {
        if self.results.insert(key.clone(), env).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > REQUEST_CACHE_LEN {
            if let Some(old) = self.order.pop_front() {
                self.results.remove(&old);
            }
        }
    }
}

/// Session state restored when an invocation fails or is a dry run: everything a handler may change besides the
/// staged document.
struct Rollback {
    app: AppContext,
    ui: UiState,
    checkpoints: Option<Vec<Checkpoint>>,
    clipboard: Option<Value>,
}

impl Rollback {
    fn take(s: &Session) -> Self {
        Self {
            app: s.app.clone(),
            ui: s.ui.clone(),
            checkpoints: s.project.as_ref().map(|p| p.checkpoints.clone()),
            clipboard: s.clipboard.clone(),
        }
    }

    fn apply(self, s: &mut Session) {
        s.app = self.app;
        s.ui = self.ui;
        s.clipboard = self.clipboard;
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
}

fn observe(s: &Session) -> Observed {
    Observed {
        state: serde_json::to_value(crate::commands::app::app_state(s)).unwrap_or(Value::Null),
        history: s.project.as_ref().map(|p| (p.history.position(), p.history.len())),
        dirty: s.project.as_ref().map(OpenProject::dirty),
    }
}

/// The core on the document thread.
#[derive(Debug)]
pub(crate) struct Core {
    session: Option<Session>,
    requests: RequestCache,
    registry: &'static Registry,
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
    let missing: Vec<&str> =
        entry.scopes().into_iter().filter(|s| !cx.origin().has_scope(*s)).map(|s| s.as_str()).collect();
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
        check_expect_rev(cx, &opts.expect_rev)?;
    }
    let Some(command) = entry.command else { return Err(CommandError::internal("implemented entry without handler")) };
    (command.handler)(cx, params)
}

/// REV-05: every listed object must still have the expected `rev`.
fn check_expect_rev(cx: &Cx, expect: &BTreeMap<String, u64>) -> Result<(), CommandError> {
    let doc = cx.document()?;
    for (r, expected) in expect {
        let key = match cx.resolve(r)? {
            tt_model::document::Target::Object { key, .. } => key,
            tt_model::document::Target::Cell { element, .. } => element.as_str().to_owned(),
        };
        let actual = doc.locate(&key).map_or(0, |l| l.rev);
        if actual != *expected {
            return Err(CommandError::with_details(
                codes::REV_CONFLICT,
                format!("`{r}` changed: rev {actual}, expected {expected}"),
                json!({ "ref": r, "id": key, "expected": expected, "actual": actual, "documentRev": doc.document_rev() }),
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
        (StepEdit::Patches(commits), TxnKind::Step | TxnKind::Redo(_)) => {
            Back::Patches(commits.iter().rev().map(|c| (c.key.clone(), c.undo.clone())).collect())
        }
        (StepEdit::Patches(commits), TxnKind::Undo(_)) => {
            Back::Patches(commits.iter().map(|c| (c.key.clone(), c.redo.clone())).collect())
        }
    }
}

/// Commits transactions: installs each stamped snapshot, records undo steps or moves the history position, logs
/// the revision and emits `doc.changed`.
fn commit(s: &mut Session, txns: Vec<Txn>, origin: &Origin) -> Result<Option<UndoRef>, CommandError> {
    if txns.is_empty() {
        return Ok(None);
    }
    let mut undo_ref = None;
    let mut events = Vec::new();
    {
        let p = s.project.as_mut().ok_or_else(no_project)?;
        for t in txns {
            p.log_commit(t.rev, back_of(&t), t.before.clone());
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
            };
            p.doc = t.after;
            events.push(json!({
                "undoStepId": step_id,
                "documentRev": t.rev,
                "label": t.label,
                "origin": origin.label(),
                "changes": elide(&t.changes),
            }));
        }
    }
    for e in events {
        s.emit("doc.changed", e);
    }
    Ok(undo_ref)
}

fn changed_fields(a: &Value, b: &Value) -> Vec<String> {
    let (Some(a), Some(b)) = (a.as_object(), b.as_object()) else { return Vec::new() };
    b.iter().filter(|(k, v)| a.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect()
}

impl Core {
    pub(crate) fn new(registry: &'static Registry, session: Session) -> Self {
        Self { session: Some(session), requests: RequestCache::default(), registry }
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
    }

    /// One invocation, with `requestId` idempotency.
    pub(crate) fn invoke(&mut self, inv: &Invocation) -> Envelope {
        let key = inv.options.request_id.as_ref().map(|r| (inv.origin.session_key(), r.clone()));
        if let Some(k) = &key
            && let Some(env) = self.requests.get(k)
        {
            return env.clone();
        }
        let env = self.run(inv);
        if let Some(k) = key {
            self.requests.insert(k, env.clone());
        }
        env
    }

    fn run(&mut self, inv: &Invocation) -> Envelope {
        let Some(entry) = self.registry.get(&inv.command).copied() else {
            return Envelope::error(unknown(&inv.command, self.registry), self.rev());
        };
        let session = self.take_session();
        let before = observe(&session);
        let rollback = Rollback::take(&session);
        // A command that declares its own `dryRun` parameter (doc.object.patch, doc.import.json, doc.repair) is a dry
        // run when it is set, exactly as with the invocation option.
        let dry = inv.options.dry_run
            || (inv.params.get("dryRun") == Some(&Value::Bool(true))
                && entry.params_schema().pointer("/properties/dryRun").is_some());
        let mut cx = Cx::new(session, inv.origin.clone(), entry.id(), dry);
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
        let mut session = cx.into_session();
        let rev_before = session.project.as_ref().map(|p| p.doc.document_rev());
        let env = match outcome {
            Err(e) => {
                rollback.apply(&mut session);
                let mut env = Envelope::error(e, rev_before);
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
                    rollback.apply(&mut session);
                    env.dry_run = true;
                } else {
                    match commit(&mut session, txns, &inv.origin) {
                        Ok(u) => {
                            env.undo = u;
                            env.document_rev = last_rev.or(rev_before).or(env.document_rev);
                            if env.document_rev.is_none() {
                                env.document_rev = session.project.as_ref().map(|p| p.doc.document_rev());
                            }
                        }
                        Err(e) => env = Envelope::error(e, rev_before),
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

    /// A batch (11 §7.3).
    pub(crate) fn batch(&mut self, req: &BatchRequest, origin: &Origin) -> Envelope {
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
        let mut steps: Vec<Envelope> = Vec::new();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        let mut failed: Option<(usize, CommandError)> = None;
        for (i, step) in req.steps.iter().enumerate() {
            let env = match resolve_refs(&step.params, &steps, &names) {
                Ok(params) => self.invoke(&Invocation {
                    command: step.id.clone(),
                    params,
                    options: InvokeOptions { dry_run: req.dry_run, ..InvokeOptions::default() },
                    origin: origin.clone(),
                }),
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
        env.dry_run = req.dry_run;
        env.steps = Some(steps);
        env
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
            let outcome =
                resolve_refs(&step.params, &steps, &names).and_then(|params| match self.registry.get(&step.id) {
                    Some(entry) => run_step(&mut cx, entry, params, &opts),
                    None => Err(unknown(&step.id, self.registry)),
                });
            if let Some(n) = &step.name {
                names.insert(n.clone(), i);
            }
            let rev_now = cx.document().ok().map(Document::document_rev).or(rev_before);
            match outcome {
                Ok(result) => {
                    let mut env = Envelope::ok(result, rev_now);
                    env.warnings = cx.take_warnings();
                    env.targets = cx.take_targets();
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
        let committed = match failed {
            Some((i, e)) => Err(step_failed(i, &req.steps[i].id, e)),
            None => cx.finish_staged(req.label.clone()),
        };
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
                env.steps = Some(steps);
                if req.dry_run {
                    rollback.apply(&mut session);
                    env.dry_run = true;
                } else if let Some(t) = txn {
                    match commit(&mut session, vec![t], origin) {
                        Ok(u) => {
                            env.undo = u;
                            env.document_rev = last_rev;
                        }
                        Err(e) => env.error = Some(e),
                    }
                }
                env
            }
        };
        if !req.dry_run {
            Self::settle(&mut session, &before);
        }
        self.session = Some(session);
        env
    }

    /// Opens a document as the current project (the load path: C1.3's `project.open` reads packages into it).
    pub(crate) fn open(&mut self, doc: Document, path: Option<String>, read_only: bool) -> Envelope {
        let mut session = self.take_session();
        let before = observe(&session);
        session.app = AppContext::for_document(&doc);
        let rev = doc.document_rev();
        let id = doc.project().id().as_str().to_owned();
        session.project = Some(OpenProject::new(doc, path.clone(), read_only));
        session.emit("project.opened", json!({ "projectId": id, "path": path, "readOnly": read_only }));
        Self::settle(&mut session, &before);
        self.session = Some(session);
        Envelope::ok(json!({ "projectId": id, "readOnly": read_only }), Some(rev))
    }

    /// Drains pending events.
    pub(crate) fn take_events(&mut self) -> Vec<crate::session::Event> {
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
/// (11 §7.3; ADR-0013 §5).
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
