//! The invocation context — the **exclusive mutation capability** (plan §3.2; C1.2).
//!
//! A [`Cx`] is the only value through which a document change can be staged. It has private fields, no public
//! constructor, no `Clone` and no `Default`; the executor creates one per invocation (or per atomic batch) on the
//! document thread and lends it to handlers as `&mut Cx`. Enablement checks receive it as `&dyn Any`, which yields
//! only `&Cx` — enough to read, never to stage. Staged changes become visible only when the executor commits the
//! transaction; documents handed out to other threads are immutable snapshots. `tests/ui/*.rs` prove these
//! properties at compile time (trybuild).

use std::collections::BTreeSet;

use serde_json::{Value, json};
use tt_model::ModelError;
use tt_model::change::{self, OwnChange, PatchOp};
use tt_model::document::{Document, EntryCommit, EntryKey, Target};
use tt_model::path::Ref;
use tt_model::{Id, ObjectType, codes as model_codes};
use tt_registry::{CommandError, codes};

use crate::align::prepare;
use crate::app::{AppContext, SpecialContext};
use crate::history::StepEdit;
use crate::session::{Checkpoint, Event, OpenProject, Session};
use crate::types::{ChangeRecord, Origin, Warning};
use crate::ui::UiState;

/// What a committed transaction does to the undo history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TxnKind {
    /// A new undo step.
    Step,
    /// Undo of the step at this index.
    Undo(usize),
    /// Redo of the step at this index.
    Redo(usize),
    /// A system change without an undo step (the `modifiedAt` stamp of a save; C1.3).
    System,
}

/// One transaction ready to commit: the stamped document after it and its change records.
#[derive(Debug, Clone)]
pub(crate) struct Txn {
    pub kind: TxnKind,
    pub rev: u64,
    pub before: Document,
    pub after: Document,
    /// The edit in the forward direction (for a new step) or the step's edit after this replay (undo/redo: the
    /// patches returned by the replay, oriented as the step's undo/redo).
    pub edit: StepEdit,
    pub touched: Vec<String>,
    pub structural: bool,
    pub changes: Vec<ChangeRecord>,
    pub label: String,
}

/// Changes staged by the handlers of the current transaction.
#[derive(Debug, Clone)]
struct Staged {
    doc: Document,
    commits: Vec<EntryCommit>,
    touched: BTreeSet<String>,
    structural: bool,
    /// Change records computed for the current staged state (reused when the transaction commits).
    cached: Option<(Vec<ChangeRecord>, Vec<String>)>,
}

/// An effect a command has outside the document, applied only when its transaction commits (never for a dry run or
/// a failed batch; 11 §7.2–§7.3, C12-R1-02/10).
#[derive(Debug, Clone)]
pub(crate) enum Effect {
    /// Request cancellation of a job.
    CancelJob(String),
    /// Deliver an event.
    Event(Event),
    /// Close the open project, deleting its recovery data (01 §3.4; C1.3).
    Close,
    /// Install a project (a new project, a backup copy) through the guarded load (ADR-0013 §16; C1.3).
    Install(Box<crate::run::Install>),
    /// Merge preference values and write the preferences file (01 §11; C1.3).
    Prefs(serde_json::Map<String, Value>),
    /// A save of the open project (`onUnsaved: "save"`): written after every step succeeded and before the commit,
    /// applied to the project after it (C13-R1-01).
    Save(Box<crate::persistence::PendingSave>),
}

/// The state of a context a step can be rolled back to (a step with its own `dryRun` inside an atomic batch).
pub(crate) struct Savepoint {
    staged: Option<Staged>,
    txns: usize,
    effects: usize,
    ids_drawn: u64,
    app: AppContext,
    ui: UiState,
    clipboard: Option<Value>,
    edit_options: std::collections::BTreeMap<String, serde_json::Map<String, Value>>,
    checkpoints: Option<Vec<Checkpoint>>,
}

/// The invocation context; see the module documentation.
#[derive(Debug)]
pub struct Cx {
    session: Session,
    origin: Origin,
    command: &'static str,
    staged: Option<Staged>,
    txns: Vec<Txn>,
    warnings: Vec<Warning>,
    targets: Option<Value>,
    label: Option<String>,
    in_atomic_batch: bool,
    dry_run: bool,
    effects: Vec<Effect>,
    /// IDs drawn so far from this transaction's ID stream (see [`Cx::new_id`]).
    ids_drawn: u64,
    step: Option<usize>,
    /// The call's confirmation inputs (11 §6.3): token, `confirm: true`, parameter fingerprint.
    confirm_token: Option<String>,
    confirm_flag: bool,
    fingerprint: String,
}

/// Millisecond base of the ULID time part of transaction IDs (2025-10-15T00:00:00Z); the time part is a logical
/// clock, not the wall clock, so replays are identical.
const ID_EPOCH_MS: u64 = 1_760_486_400_000;

/// `documentRev` cannot grow further (REV-01; C12-R1-19): a typed error, never an overflow.
pub(crate) fn revisions_exhausted() -> CommandError {
    CommandError::with_details(
        "doc.limitExceeded",
        "the document revision cannot grow beyond 18446744073709551615",
        json!({ "limit": "documentRev", "maximum": u64::MAX }),
    )
}

pub(crate) fn model_error(errors: Vec<ModelError>) -> CommandError {
    let all: Vec<Value> =
        errors.iter().map(|e| json!({ "code": e.code(), "message": e.message(), "details": e.details() })).collect();
    let Some(first) = errors.into_iter().next() else { return CommandError::internal("empty error list") };
    let mut err = CommandError::from(first);
    if all.len() > 1
        && let Some(m) = err.details.as_object_mut()
    {
        m.insert("errors".into(), Value::Array(all));
    }
    err
}

pub(crate) fn no_project() -> CommandError {
    CommandError::with_details(codes::PROJECT_NONE, "no project is open", json!({}))
        .hint("open or create a project first (project.open, project.create)")
}

impl Cx {
    pub(crate) fn new(session: Session, origin: Origin, command: &'static str, dry_run: bool) -> Self {
        Self {
            session,
            origin,
            command,
            staged: None,
            txns: Vec::new(),
            warnings: Vec::new(),
            targets: None,
            label: None,
            in_atomic_batch: false,
            dry_run,
            effects: Vec::new(),
            ids_drawn: 0,
            step: None,
            confirm_token: None,
            confirm_flag: false,
            fingerprint: String::new(),
        }
    }

    /// Sets the call's confirmation inputs.
    pub(crate) fn set_confirmation(&mut self, token: Option<String>, confirm: bool, fingerprint: String) {
        self.confirm_token = token;
        self.confirm_flag = confirm;
        self.fingerprint = fingerprint;
    }

    /// A confirmable condition was reached (11 §6.3). For the user (the GUI asks itself, M4), for dry runs and when
    /// the agent session's policy does not confirm `category`, this passes. `confirm: true` passes for categories the
    /// session pre-authorises. A valid `confirmToken` for this very call passes once. Otherwise the call fails without
    /// side effects with `confirmation.required`, which the executor turns into the `needs_confirmation` result; the
    /// token it carries is registered now: single use, five minutes, bound to the session, the command, the
    /// parameters, the document revision and `binding` (a condition that must still hold when the token is used).
    ///
    /// # Errors
    /// `confirmation.required`, `confirmation.expired`, `confirmation.declined`.
    pub fn confirm(
        &mut self,
        category: crate::agents::Category,
        summary: impl Into<String>,
        consequences: Vec<Value>,
        binding: Option<Value>,
    ) -> Result<(), CommandError> {
        let Origin::Agent { session, .. } = &self.origin else { return Ok(()) };
        let session = session.clone();
        if self.dry_run {
            return Ok(());
        }
        let rev = self.session.project.as_ref().map(|p| p.doc.document_rev());
        let hub = &mut self.session.agents;
        let (policy, pre) = hub
            .sessions
            .get(&session)
            .map_or((hub.limits.policy, false), |s| (s.policy, s.preauthorized.contains(&category)));
        if !policy.confirms(category) || (self.confirm_flag && pre) {
            return Ok(());
        }
        if let Some(token) = self.confirm_token.take() {
            return hub.take(&token, &session, self.command, &self.fingerprint, rev, binding.as_ref());
        }
        hub.prune();
        let summary = summary.into();
        let token = format!("cf_{}", tt_persist::fsx::random_token(12));
        hub.pending.insert(
            token.clone(),
            crate::agents::Pending {
                id: token.clone(),
                session: session.clone(),
                command: self.command.to_owned(),
                fingerprint: self.fingerprint.clone(),
                document_rev: rev,
                created: std::time::Instant::now(),
                summary: summary.clone(),
                consequences: consequences.clone(),
                category,
                decision: None,
                binding,
            },
        );
        Err(CommandError::with_details(
            "confirmation.required",
            summary.clone(),
            json!({
                "confirmToken": token, "summary": summary, "consequences": consequences,
                "category": category.as_str(), "sessionId": session,
            }),
        )
        .hint("re-invoke the same command with the same parameters and confirmToken within five minutes"))
    }

    /// The next ID of this transaction's ID stream (C12-R2-04). The stream is seeded from the project's ID and the
    /// live `documentRev` and advanced per ID, so the same document and the same calls produce the same IDs (11 P7,
    /// 10 AG-06), a dry run predicts exactly the IDs the real call creates, and a rolled-back step returns its IDs
    /// to the stream. Seeds differ between projects and between revisions, so committed IDs are never reused.
    ///
    /// # Errors
    /// `project.none`; `internal` if the ID cannot be formed.
    pub fn new_id(&mut self, ty: ObjectType) -> Result<Id, CommandError> {
        let p = self.project()?;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in p.doc.project().id().as_str().bytes().chain(p.doc.document_rev().to_le_bytes()) {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        let n = self.ids_drawn;
        self.ids_drawn = n.saturating_add(1);
        // One seeded generator per ID: the n-th ID depends only on the seed and n.
        let seed = h ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let id = tt_model::IdGenerator::seeded(seed, ID_EPOCH_MS.saturating_add(n)).next(ty)?;
        Ok(id)
    }

    pub(crate) fn set_step(&mut self, step: Option<usize>) {
        self.step = step;
    }

    /// Defers an effect to the commit of the transaction.
    pub(crate) fn defer(&mut self, effect: Effect) {
        self.effects.push(effect);
    }

    /// Emits an event when the transaction commits (never for a dry run or a failed batch).
    pub(crate) fn emit(&mut self, name: &str, payload: Value) {
        self.effects.push(Effect::Event(Event { name: name.to_owned(), payload }));
    }

    /// The save time of a pending save of this atomic batch (see `persistence::batch_save_time`).
    pub(crate) fn pending_batch_save(&self) -> Option<String> {
        crate::persistence::batch_save_time(&self.effects)
    }

    pub(crate) fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub(crate) fn savepoint(&self) -> Savepoint {
        Savepoint {
            staged: self.staged.clone(),
            txns: self.txns.len(),
            effects: self.effects.len(),
            ids_drawn: self.ids_drawn,
            app: self.session.app.clone(),
            ui: self.session.ui.clone(),
            clipboard: self.session.clipboard.clone(),
            edit_options: self.session.edit_options.clone(),
            checkpoints: self.session.project.as_ref().map(|p| p.checkpoints.clone()),
        }
    }

    pub(crate) fn rollback_to(&mut self, sp: Savepoint) {
        self.staged = sp.staged;
        self.txns.truncate(sp.txns);
        self.effects.truncate(sp.effects);
        self.ids_drawn = sp.ids_drawn;
        self.session.app = sp.app;
        self.session.ui = sp.ui;
        self.session.clipboard = sp.clipboard;
        self.session.edit_options = sp.edit_options;
        if let (Some(p), Some(c)) = (self.session.project.as_mut(), sp.checkpoints) {
            p.checkpoints = c;
        }
    }

    /// The revision the document will have when this context's transactions commit (the live revision when nothing
    /// is staged).
    ///
    /// # Errors
    /// `project.none`; `doc.limitExceeded` when revisions are exhausted.
    pub fn predicted_rev(&mut self) -> Result<u64, CommandError> {
        if self.staged.is_some() {
            let (changes, rev) = self.preview_changes()?;
            if !changes.is_empty() {
                return Ok(rev);
            }
        }
        Ok(match self.txns.last() {
            Some(t) => t.rev,
            None => self.project()?.doc.document_rev(),
        })
    }

    pub(crate) fn into_session(self) -> Session {
        self.session
    }

    pub(crate) fn set_command(&mut self, command: &'static str) {
        self.command = command;
    }

    pub(crate) fn set_atomic_batch(&mut self, on: bool) {
        self.in_atomic_batch = on;
    }

    /// Whether the current invocation runs inside an atomic batch (history commands are disabled there).
    #[must_use]
    pub fn in_atomic_batch(&self) -> bool {
        self.in_atomic_batch
    }

    /// Whether the invocation is a dry run.
    #[must_use]
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// The command being run.
    #[must_use]
    pub fn command(&self) -> &'static str {
        self.command
    }

    /// Who invoked the command.
    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    pub(crate) fn project(&self) -> Result<&OpenProject, CommandError> {
        self.session.project.as_ref().ok_or_else(no_project)
    }

    pub(crate) fn project_mut(&mut self) -> Result<&mut OpenProject, CommandError> {
        self.session.project.as_mut().ok_or_else(no_project)
    }

    /// Whether a project is open.
    #[must_use]
    pub fn has_project(&self) -> bool {
        self.session.project.is_some()
    }

    /// The document as this transaction sees it: the staged state when the transaction (or an earlier step of its
    /// atomic batch) changed it, else the live document.
    ///
    /// # Errors
    /// `project.none`.
    pub fn document(&self) -> Result<&Document, CommandError> {
        if let Some(t) = self.txns.last() {
            return Ok(&t.after);
        }
        match &self.staged {
            Some(s) => Ok(&s.doc),
            None => self.project().map(|p| &p.doc),
        }
    }

    pub(crate) fn app(&self) -> &AppContext {
        &self.session.app
    }

    pub(crate) fn app_mut(&mut self) -> &mut AppContext {
        &mut self.session.app
    }

    pub(crate) fn ui(&self) -> &UiState {
        &self.session.ui
    }

    pub(crate) fn ui_mut(&mut self) -> &mut UiState {
        &mut self.session.ui
    }

    /// Adds a warning to the envelope.
    pub fn warn(&mut self, code: &str, message: impl Into<String>, object_ids: Vec<String>) {
        self.warnings.push(Warning { code: code.to_owned(), message: message.into(), object_ids });
    }

    pub(crate) fn take_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }

    /// Records what the command actually targeted (11 P4).
    pub fn set_targets(&mut self, targets: Value) {
        self.targets = Some(targets);
    }

    pub(crate) fn take_targets(&mut self) -> Option<Value> {
        self.targets.take()
    }

    /// Overrides the undo label of the transaction (`doc.object.patch {label}`).
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub(crate) fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    // ----- references ------------------------------------------------------------------------------------------

    /// The special-reference context of this invocation.
    pub(crate) fn special_cx(&self) -> Result<SpecialContext<'_>, CommandError> {
        Ok(SpecialContext { app: &self.session.app, ui: &self.session.ui, doc: self.document()? })
    }

    /// Resolves a reference (ID, compound ID, path, special reference; 10 AG-03) against the transaction's view.
    ///
    /// # Errors
    /// `ref.notFound`, `ref.ambiguous`, `ref.wrongType`, `arg.invalid`; `project.none`.
    pub fn resolve(&self, text: &str) -> Result<Target, CommandError> {
        let sp = self.special_cx()?;
        Ok(sp.doc.resolve(&Ref::parse(text)?, text, &sp)?)
    }

    /// Resolves a reference that must be an object of the given type; a bare `@current` names the current object
    /// of that type (ADR-0013 §6).
    ///
    /// # Errors
    /// As [`Cx::resolve`], plus `ref.wrongType`.
    pub fn resolve_as(&self, text: &str, expected: ObjectType) -> Result<String, CommandError> {
        let sp = self.special_cx()?;
        if text == "@current" {
            return match sp.current_of(expected) {
                Some(Target::Object { key, .. }) => Ok(key),
                _ => Err(CommandError::with_details(
                    model_codes::REF_NOT_FOUND,
                    format!("there is no current {expected}"),
                    json!({ "ref": text, "reason": "noCurrent", "expected": expected.as_str() }),
                )),
            };
        }
        Ok(sp.doc.resolve_as(text, expected, &sp)?)
    }

    /// Resolves a list of references to object keys; `@selection` expands to every selected object.
    ///
    /// # Errors
    /// As [`Cx::resolve`]; a reference to a cell is `ref.wrongType`.
    pub fn resolve_objects(&self, refs: &[String]) -> Result<Vec<(ObjectType, String)>, CommandError> {
        let mut out: Vec<(ObjectType, String)> = Vec::new();
        for r in refs {
            if r == "@selection" {
                let doc = self.document()?;
                for k in &self.session.app.selection.ids {
                    if let Some(l) = doc.locate(k)
                        && !out.iter().any(|(_, x)| x == &l.key)
                    {
                        out.push((l.ty, l.key.clone()));
                    }
                }
                continue;
            }
            match self.resolve(r)? {
                Target::Object { ty, key } => {
                    if !out.iter().any(|(_, x)| x == &key) {
                        out.push((ty, key));
                    }
                }
                Target::Cell { .. } => {
                    return Err(CommandError::with_details(
                        model_codes::REF_WRONG_TYPE,
                        format!("`{r}` is a cell, not an object"),
                        json!({ "ref": r }),
                    ));
                }
            }
        }
        Ok(out)
    }

    // ----- staging ---------------------------------------------------------------------------------------------

    /// The committed-or-replayed state new staging starts from.
    fn base_doc(&self) -> Result<Document, CommandError> {
        Ok(match self.txns.last() {
            Some(t) => t.after.clone(),
            None => self.project()?.doc.clone(),
        })
    }

    fn staging_doc(&self) -> Result<Document, CommandError> {
        match &self.staged {
            Some(s) => Ok(s.doc.clone()),
            None => self.base_doc(),
        }
    }

    fn stage(
        &mut self,
        changes: &[(EntryKey, Vec<PatchOp>)],
        touched: impl IntoIterator<Item = String>,
        structural: bool,
    ) -> Result<(), CommandError> {
        let doc = self.staging_doc()?;
        // The redo of a new step is the verified patch its first undo returns (`replay`).
        let (next, commits) = doc.with_patched_entries_undo_only(changes).map_err(model_error)?;
        let staged = self.staged.get_or_insert_with(|| Staged {
            doc: doc.clone(),
            commits: Vec::new(),
            touched: BTreeSet::new(),
            structural: false,
            cached: None,
        });
        staged.cached = None;
        staged.doc = next;
        staged.commits.extend(commits);
        staged.touched.extend(touched);
        staged.structural |= structural;
        Ok(())
    }

    /// Stages property patches on objects: each operation's path is relative to its object (`/name`,
    /// `/lineStyle/thickness`). The objects' own views are what the transaction reports as modified. All patches of
    /// one call commit atomically (one staged commit through `Document::with_patched_entries`).
    ///
    /// # Errors
    /// `ref.notFound`; the model's patch, schema and integrity errors (nothing is staged then).
    pub fn patch_objects(&mut self, edits: Vec<(String, Vec<PatchOp>)>) -> Result<(), CommandError> {
        let doc = self.staging_doc()?;
        let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
        let mut keys = Vec::new();
        for (key, ops) in edits {
            let (entry, _ty, at) = doc.object_pointer(&key)?;
            let ops: Vec<PatchOp> = ops
                .into_iter()
                .map(|mut op| {
                    op.path = format!("{at}{}", op.path);
                    op.from = op.from.map(|f| format!("{at}{f}"));
                    op
                })
                .collect();
            match changes.iter_mut().find(|(e, _)| *e == entry) {
                Some((_, v)) => v.extend(ops),
                None => changes.push((entry, ops)),
            }
            keys.push(doc.locate(&key).map_or(key, |l| l.key.clone()));
        }
        self.stage(&changes, keys, false)
    }

    /// Stages raw entry patches that may create, delete or move objects (area commands from M2 on). Change records
    /// are computed by diffing the changed entries.
    ///
    /// # Errors
    /// The model's patch, schema and integrity errors (nothing is staged then).
    pub fn patch_entries(&mut self, changes: Vec<(EntryKey, Vec<PatchOp>)>) -> Result<(), CommandError> {
        self.stage(&changes, Vec::new(), true)
    }

    /// Whether the transaction has staged document changes.
    #[must_use]
    pub fn has_staged(&self) -> bool {
        self.staged.is_some() || !self.txns.is_empty()
    }

    /// The revision the next transaction of this invocation gets.
    fn next_rev(&self) -> Result<u64, CommandError> {
        let current = match self.txns.last() {
            Some(t) => t.rev,
            None => self.project()?.doc.document_rev(),
        };
        current.checked_add(1).ok_or_else(revisions_exhausted)
    }

    /// Turns the staged changes into a committed-ready transaction (stamped with the next revision); `None` when the
    /// staged changes change nothing.
    /// Seals the staged changes as a system transaction: committed with its own revision and `doc.changed`, without an
    /// undo step (the `modifiedAt` stamp of a save, 10 §4.1).
    ///
    /// # Errors
    /// As [`Cx::finish_staged`].
    pub(crate) fn seal_system(&mut self, label: &str) -> Result<(), CommandError> {
        if let Some(mut t) = self.finish_staged(label.to_owned())? {
            t.kind = TxnKind::System;
            self.txns.push(t);
        }
        Ok(())
    }

    pub(crate) fn finish_staged(&mut self, label: String) -> Result<Option<Txn>, CommandError> {
        let Some(staged) = self.staged.take() else { return Ok(None) };
        let before = self.base_doc()?;
        let rev = self.next_rev()?;
        let touched: Vec<String> = staged.touched.into_iter().collect();
        let (changes, keys) = match staged.cached {
            Some(c) => c,
            None => changes_between(&before, &staged.doc, &touched, staged.structural, rev)?,
        };
        if changes.is_empty() {
            return Ok(None);
        }
        let stamps: Vec<String> = keys.iter().filter(|k| staged.doc.locate(k).is_some()).cloned().collect();
        let after = staged.doc.with_revisions(rev, &stamps)?;
        Ok(Some(Txn {
            kind: TxnKind::Step,
            rev,
            before,
            after,
            edit: StepEdit::Patches(staged.commits),
            touched: keys,
            structural: staged.structural,
            changes,
            label,
        }))
    }

    /// The change records the staged changes will commit with, and the revision they get.
    ///
    /// # Errors
    /// `project.none`; `internal` if serialisation fails.
    pub fn preview_changes(&mut self) -> Result<(Vec<ChangeRecord>, u64), CommandError> {
        let rev = self.next_rev()?;
        let Some(staged) = &self.staged else { return Ok((Vec::new(), rev)) };
        if let Some((c, _)) = &staged.cached {
            return Ok((c.clone(), rev));
        }
        let before = self.base_doc()?;
        let touched: Vec<String> = staged.touched.iter().cloned().collect();
        let (changes, keys) = changes_between(&before, &staged.doc, &touched, staged.structural, rev)?;
        if let Some(s) = self.staged.as_mut() {
            s.cached = Some((changes.clone(), keys));
        }
        Ok((changes, rev))
    }

    /// The document as the staged changes will commit it: content staged so far, `documentRev` and the touched objects
    /// stamped with the revision they will get (C12-R3-02). Without staged changes, the current document.
    ///
    /// # Errors
    /// `project.none`; `doc.limitExceeded` when revisions are exhausted.
    pub(crate) fn stamped_document(&mut self) -> Result<Document, CommandError> {
        if self.staged.is_none() {
            return Ok(self.document()?.clone());
        }
        let (changes, rev) = self.preview_changes()?;
        if let Some(staged) = &self.staged
            && !changes.is_empty()
            && let Some((_, keys)) = &staged.cached
        {
            let stamps: Vec<String> = keys.iter().filter(|k| staged.doc.locate(k).is_some()).cloned().collect();
            return Ok(staged.doc.with_revisions(rev, &stamps)?);
        }
        Ok(self.document()?.clone())
    }

    pub(crate) fn take_txns(&mut self) -> Vec<Txn> {
        std::mem::take(&mut self.txns)
    }

    /// Replays a history step (undo when `undo`, else redo) on the transaction's view as a new transaction (REV-03).
    pub(crate) fn replay(
        &mut self,
        index: usize,
        undo: bool,
        edit: &StepEdit,
        touched: &[String],
        structural: bool,
        label: &str,
    ) -> Result<(), CommandError> {
        if self.staged.is_some() {
            return Err(CommandError::internal("history replay after staged changes"));
        }
        let before = self.document()?.clone();
        let rev = self.next_rev()?;
        let kind = if undo { TxnKind::Undo(index) } else { TxnKind::Redo(index) };
        let (after, new_edit, changes, keys) = match edit {
            StepEdit::Patches(commits) => {
                // The step's patches in replay order, merged per entry in order of first appearance (an atomic batch
                // may have changed one entry several times); each merged patch is checked against the live entry.
                let ordered: Vec<&EntryCommit> =
                    if undo { commits.iter().rev().collect() } else { commits.iter().collect() };
                let mut merged: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
                for c in ordered {
                    let p = if undo { &c.undo } else { &c.redo };
                    match merged.iter_mut().find(|(k, _)| *k == c.key) {
                        Some((_, ops)) => ops.extend(p.iter().cloned()),
                        None => merged.push((c.key.clone(), p.clone())),
                    }
                }
                let mut patches = Vec::with_capacity(merged.len());
                for (key, ops) in merged {
                    let ready = prepare(&before, &key, &ops)?;
                    patches.push((key, ready));
                }
                let (next, applied) = before.with_patched_entries_undo_only(&patches).map_err(model_error)?;
                // The replay returns the verified patch back to the state before it (C1.1 ticket: use the redo
                // returned by undo); the patch it applied — checked against the live entry by `prepare` — stays the
                // step's patch for this direction. One commit per entry, in the step's commit order.
                let mut replayed: Vec<EntryCommit> = applied
                    .into_iter()
                    .zip(patches)
                    .map(|(a, (key, used))| {
                        if undo {
                            EntryCommit { key, undo: used, redo: a.undo }
                        } else {
                            EntryCommit { key, undo: a.undo, redo: used }
                        }
                    })
                    .collect();
                if undo {
                    replayed.reverse();
                }
                let (changes, keys) = changes_between(&before, &next, touched, structural, rev)?;
                let stamps: Vec<String> = keys.iter().filter(|k| next.locate(k).is_some()).cloned().collect();
                let after = next.with_revisions(rev, &stamps)?;
                (after, StepEdit::Patches(replayed), changes, keys)
            }
            StepEdit::Snapshot { before: b, after: a } => {
                let target = if undo { b } else { a };
                let (after, changes, keys) = restore_snapshot(&before, target, rev)?;
                (after, edit.clone(), changes, keys)
            }
        };
        self.txns.push(Txn {
            kind,
            rev,
            before,
            after,
            edit: new_edit,
            touched: keys,
            structural,
            changes,
            label: label.to_owned(),
        });
        Ok(())
    }

    /// Stages a whole-document restore (checkpoint restore) as one new step.
    pub(crate) fn restore(&mut self, target: &Document, label: &str) -> Result<(), CommandError> {
        if self.staged.is_some() || !self.txns.is_empty() {
            return Err(CommandError::internal("restore after staged changes"));
        }
        let before = self.document()?.clone();
        let rev = self.next_rev()?;
        let (after, changes, keys) = restore_snapshot(&before, target, rev)?;
        if changes.is_empty() {
            return Ok(());
        }
        self.txns.push(Txn {
            kind: TxnKind::Step,
            rev,
            edit: StepEdit::Snapshot { before: Box::new(before.clone()), after: Box::new(after.clone()) },
            before,
            after,
            touched: keys,
            structural: true,
            changes,
            label: label.to_owned(),
        });
        Ok(())
    }
}

/// The document with `target`'s content at revision `rev`: objects whose own view changes (or that are created) get
/// `rev`; objects whose content is unchanged keep the larger of their current and target revisions, so no revision
/// decreases (REV-03).
fn restore_snapshot(
    current: &Document,
    target: &Document,
    rev: u64,
) -> Result<(Document, Vec<ChangeRecord>, Vec<String>), CommandError> {
    let diff = change::diff(current, target)?;
    let mut changed: BTreeSet<String> = BTreeSet::new();
    let mut records = Vec::with_capacity(diff.len());
    let mut keys: Vec<String> = Vec::new();
    for (c, o) in &diff {
        records.push(record(c, o.key.clone(), o.ty, o.parent.clone(), o.index, rev)?);
        keys.push(o.key.clone());
    }
    add_drawing_records(target, &mut records, &mut keys, rev);
    changed.extend(keys);
    let mut stamps: Vec<(String, u64)> = Vec::new();
    for loc in target.locations() {
        if changed.contains(&loc.key) {
            stamps.push((loc.key.clone(), rev));
        } else if let Some(cur) = current.locate(&loc.key)
            && cur.key == loc.key
            && cur.rev > loc.rev
        {
            stamps.push((loc.key.clone(), cur.rev));
        }
    }
    let after = target.with_object_revisions(rev, &stamps)?;
    Ok((after, records, changed.into_iter().collect()))
}

fn record(
    c: &OwnChange,
    key: String,
    ty: ObjectType,
    parent: Option<String>,
    index: usize,
    rev: u64,
) -> Result<ChangeRecord, CommandError> {
    let (op, parent_id, index, patch) = match c {
        OwnChange::Created => ("created", parent, Some(index), None),
        OwnChange::Deleted => ("deleted", parent, None, None),
        OwnChange::Modified(p) => {
            let mut ops = Vec::with_capacity(p.len());
            for op in p {
                ops.push(serde_json::to_value(op).map_err(|e| CommandError::internal(e.to_string()))?);
            }
            ("modified", None, None, Some(ops))
        }
    };
    Ok(ChangeRecord { id: key, ty: ty.as_str().to_owned(), op: op.to_owned(), rev, parent_id, index, patch })
}

/// The change records between two snapshots: by touched-object own views (property edits), or by diffing the
/// changed entries (structural edits). Returns the records and the keys of the changed objects.
pub(crate) fn changes_between(
    before: &Document,
    after: &Document,
    touched: &[String],
    structural: bool,
    rev: u64,
) -> Result<(Vec<ChangeRecord>, Vec<String>), CommandError> {
    let mut records = Vec::new();
    let mut keys = Vec::new();
    if structural {
        for (c, o) in change::diff(before, after)? {
            keys.push(o.key.clone());
            records.push(record(&c, o.key.clone(), o.ty, o.parent.clone(), o.index, rev)?);
        }
        add_drawing_records(after, &mut records, &mut keys, rev);
        return Ok((records, keys));
    }
    for key in touched {
        let old = match change::own_view(before, key) {
            Ok(v) => Some(v),
            Err(e) if e.code() == model_codes::REF_NOT_FOUND => None,
            Err(e) => return Err(e.into()),
        };
        let new = match change::own_view(after, key) {
            Ok(v) => Some(v),
            Err(e) if e.code() == model_codes::REF_NOT_FOUND => None,
            Err(e) => return Err(e.into()),
        };
        let parent = |d: &Document| d.locate(key).and_then(|l| l.parent.clone());
        let c = match (&old, &new) {
            (Some((_, a)), Some((ty, b))) if a != b => {
                Some((OwnChange::Modified(change::diff_json(a, b, "", *ty)), *ty))
            }
            (Some(_), Some(_)) | (None, None) => None,
            (None, Some((ty, _))) => Some((OwnChange::Created, *ty)),
            (Some((ty, _)), None) => Some((OwnChange::Deleted, *ty)),
        };
        if let Some((c, ty)) = c {
            let par = if matches!(c, OwnChange::Deleted) { parent(before) } else { parent(after) };
            keys.push(key.clone());
            records.push(record(&c, key.clone(), ty, par, 0, rev)?);
        }
    }
    add_drawing_records(after, &mut records, &mut keys, rev);
    Ok((records, keys))
}

/// REV-02 / 11 P6 (C12-R1-11): a change to the content of a drawing (a node or stroke) changes the revision of the
/// drawing, so the drawing is reported too: `modified` with an empty own-view patch, because its own view (children
/// as stubs) did not change.
fn add_drawing_records(after: &Document, records: &mut Vec<ChangeRecord>, keys: &mut Vec<String>, rev: u64) {
    let mut drawings: Vec<String> = Vec::new();
    for k in keys.iter() {
        if let Some((container, _)) = k.split_once('/')
            && after.locate(container).is_some_and(|l| l.ty == ObjectType::Drawing)
            && !drawings.iter().any(|d| d == container)
            && !keys.iter().any(|x| x == container)
        {
            drawings.push(container.to_owned());
        }
    }
    for d in drawings {
        records.push(ChangeRecord {
            id: d.clone(),
            ty: ObjectType::Drawing.as_str().to_owned(),
            op: "modified".to_owned(),
            rev,
            parent_id: None,
            index: None,
            patch: Some(Vec::new()),
        });
        keys.push(d);
    }
}
