//! The state the document thread owns: the open project (live document, undo history, checkpoints, revision log),
//! the application context, the window/workspace state and pending events. Nothing outside this crate can reach it;
//! command handlers see it only through [`crate::Cx`].

use std::collections::VecDeque;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey};

use crate::app::AppContext;
use crate::history::History;
use crate::jobs::JobTable;
use crate::ui::UiState;

/// A named checkpoint (11 §7.7): a copy-on-write snapshot of the document, independent of the undo stack.
#[derive(Debug, Clone)]
pub(crate) struct Checkpoint {
    pub name: String,
    pub created_at: String,
    pub document_rev: u64,
    pub origin: String,
    pub doc: Document,
}

/// How to go from a revision back to the previous one (for `doc.diff`).
#[derive(Debug, Clone)]
pub(crate) enum Back {
    /// Entry patches, applied in order (modulo `rev`, see [`crate::align`]).
    Patches(Vec<(EntryKey, Vec<PatchOp>)>),
    /// The previous content as a whole snapshot (checkpoint restore and its undo/redo).
    Snapshot(Document),
}

/// One committed transaction in the revision log.
#[derive(Debug, Clone)]
pub(crate) struct RevRecord {
    /// The transaction's `documentRev`.
    pub rev: u64,
    /// How to reach `rev - 1`.
    pub back: Back,
    /// The document at `rev - 1`, kept for the most recent [`SNAPSHOT_WINDOW`] records (copy-on-write: unchanged
    /// entries are shared).
    pub before: Option<Document>,
    /// The revisions the objects the transaction stamped had at `rev - 1` (back-patches restore content, these
    /// restore the revisions; C12-R1-08).
    pub earlier_revs: Vec<(String, u64)>,
    /// The memory the earlier document holds alone, in bytes ([`snapshot_weight`]): what keeping `before`, or a
    /// snapshot in `back` (the same document: checkpoint restores and their undo and redo), costs.
    pub weight: usize,
    /// The memory the record's other owned values hold: the back patches' values and `earlier_revs` (E-77).
    pub extra: usize,
}

impl RevRecord {
    /// Whether the record holds its earlier document (in `before` or as a snapshot in `back`; one document either way).
    fn holds_document(&self) -> bool {
        self.before.is_some() || matches!(self.back, Back::Snapshot(_))
    }

    /// Everything the record holds alone (M5 ruling E-77).
    fn held(&self) -> usize {
        std::mem::size_of::<RevRecord>() + self.extra + if self.holds_document() { self.weight } else { 0 }
    }
}

/// The memory a record's back patches and earlier revisions hold (E-77; [`crate::heap`]).
fn record_extra(back: &Back, earlier_revs: &[(String, u64)], revs_capacity: usize) -> usize {
    let patches = match back {
        Back::Patches(list) => {
            list.capacity() * std::mem::size_of::<(EntryKey, Vec<PatchOp>)>()
                + list.iter().map(|(_, ops)| crate::heap::ops(ops, ops.capacity())).sum::<usize>()
        }
        Back::Snapshot(_) => 0,
    };
    patches
        + revs_capacity * std::mem::size_of::<(String, u64)>()
        + earlier_revs.iter().map(|(k, _)| k.capacity()).sum::<usize>()
}

/// Transactions kept for `doc.diff` (10 AG-09 / D11: at least the last 10 000, and back to the last save).
pub const REV_LOG_LEN: usize = 10_000;
/// Records that keep their previous snapshot for fast diffs.
pub const SNAPSHOT_WINDOW: usize = 256;
/// The most memory the kept snapshots may hold alone, in bytes (M5 ruling E-73 item 3, by the estimate of
/// [`snapshot_weight`]): each commit rebuilds the document's derived data, its object index included, so every kept
/// `before` holds its own index besides the entries the transaction replaced (on a 20,000-stroke drawing about 10 MB per
/// revision: the merge note's known limits, M11 C11.1). The oldest snapshots are dropped first and fall back to their
/// back-patches: `doc.diff` stays exact, only slower; no revision becomes unavailable (the records themselves are kept
/// as 10 AG-09 asks). The newest snapshot is always kept.
pub const SNAPSHOT_MEMORY_BUDGET: usize = 256 << 20;

/// The snapshot budget in force: [`SNAPSHOT_MEMORY_BUDGET`], or the dev-only hook's (`test-hooks`).
fn snapshot_budget() -> usize {
    #[cfg(feature = "test-hooks")]
    if let Some(b) = crate::hooks::snapshot_budget() {
        return b;
    }
    SNAPSHOT_MEMORY_BUDGET
}

/// The kept earlier documents of a log (in `before` or as a snapshot in `back`): how many, and everything the log's
/// records hold alone by the estimate (E-77).
#[cfg(feature = "test-hooks")]
pub(crate) fn kept_snapshots(log: &VecDeque<RevRecord>) -> (usize, usize) {
    (log.iter().filter(|r| r.holds_document()).count(), log.iter().map(RevRecord::held).sum())
}

/// The memory one indexed object costs in a document's derived data (the index's map entry with its location, the
/// copies of its key, the local-ID map), estimated from the measurement of `crates/executor/tests/c52a_history_size.rs`
/// (about 500 bytes per object on a crowded drawing).
pub const INDEX_BYTES_PER_OBJECT: usize = 512;

/// What keeping `before` costs beside `after`, in bytes: the entries `before` holds that `after` does not share
/// (copy-on-write; [`crate::heap::entry`]), and its own derived data, rebuilt by every commit and so never shared
/// ([`INDEX_BYTES_PER_OBJECT`] per indexed object).
pub(crate) fn snapshot_weight(before: &Document, after: &Document) -> usize {
    let replaced: usize = before
        .entry_keys()
        .iter()
        .filter(|k| !before.same_entry(after, k))
        .map(|k| crate::heap::entry(before, k))
        .sum();
    std::mem::size_of::<Document>() + replaced + before.object_count() * INDEX_BYTES_PER_OBJECT
}

/// The open project.
#[derive(Debug, Clone)]
pub(crate) struct OpenProject {
    pub doc: Document,
    pub history: History,
    pub checkpoints: Vec<Checkpoint>,
    pub log: VecDeque<RevRecord>,
    /// `documentRev` at the last save (`None`: never saved in this session).
    pub saved_rev: Option<u64>,
    pub path: Option<String>,
    pub form: Option<String>,
    pub read_only: bool,
    /// Changes that cannot be undone but must be saved (01 §3.5): repairs on open, a recovered copy, an untitled
    /// document opened without a location. The project is modified until the next save.
    pub permanent_dirty: bool,
    /// Persistence state (C1.3).
    pub store: crate::store::Store,
    /// This opening's number, new for every project opened, created or reverted (never reused in the process): what
    /// the drawing thumbnails' cache belongs to (M5 rulings E-70, E-71).
    pub generation: u64,
}

impl OpenProject {
    pub fn new(doc: Document, path: Option<String>, read_only: bool) -> Self {
        static GENERATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            generation: GENERATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            saved_rev: path.as_ref().map(|_| doc.document_rev()),
            form: path.as_ref().map(|_| "package".to_owned()),
            doc,
            history: History::clean(),
            checkpoints: Vec::new(),
            log: VecDeque::new(),
            permanent_dirty: path.is_none(),
            path,
            read_only,
            store: crate::store::Store::default(),
        }
    }

    /// The form (`package` or `folder`) the project is saved in.
    pub fn form_kind(&self) -> tt_persist::Form {
        if self.form.as_deref() == Some("folder") { tt_persist::Form::Folder } else { tt_persist::Form::Package }
    }

    /// The oldest revision `doc.diff` can start from.
    pub fn history_from_rev(&self) -> u64 {
        self.log.front().map_or(self.doc.document_rev(), |r| r.rev.saturating_sub(1))
    }

    /// Records a committed transaction. The log keeps at least the last [`REV_LOG_LEN`] transactions and never drops
    /// a record the last save still needs (D11, AG-09; C12-R1-08).
    pub fn log_commit(
        &mut self,
        rev: u64,
        back: Back,
        before: Document,
        after: &Document,
        earlier_revs: Vec<(String, u64)>,
    ) {
        let weight = snapshot_weight(&before, after);
        let extra = record_extra(&back, &earlier_revs, earlier_revs.capacity());
        self.log.push_back(RevRecord { rev, back, before: Some(before), earlier_revs, weight, extra });
        if self.log.len() > SNAPSHOT_WINDOW
            && let Some(old) = self.log.get_mut(self.log.len() - SNAPSHOT_WINDOW - 1)
        {
            old.before = None;
        }
        // Bounded memory (E-73 item 3, E-77). What can only go with its record counts first: every record's owned values
        // and a snapshot in `back` (its earlier document). The kept `before` documents beside back patches then fit in
        // what remains, newest first (releasing one costs only speed: `doc.diff` rebuilds that revision from the
        // patches). The newest record is always kept whole.
        let budget = snapshot_budget();
        let mut total: usize = self
            .log
            .iter()
            .map(|r| {
                std::mem::size_of::<RevRecord>()
                    + r.extra
                    + if matches!(r.back, Back::Snapshot(_)) { r.weight } else { 0 }
            })
            .sum();
        let mut first = true;
        for r in self.log.iter_mut().rev() {
            if r.before.is_some() && !matches!(r.back, Back::Snapshot(_)) {
                if !first && total.saturating_add(r.weight) > budget {
                    r.before = None;
                } else {
                    total = total.saturating_add(r.weight);
                }
            }
            first = false;
        }
        // Still over: the oldest records go whole, their snapshots in `back` included, never the newest. A `doc.diff`
        // from a base older than the oldest kept record then answers `doc.diffUnavailable` with `oldestRev` (10 line 1060,
        // AG-09 line 1117), as for any base outside the kept records.
        while total > budget && self.log.len() > 1 {
            match self.log.pop_front() {
                Some(r) => total = total.saturating_sub(r.held()),
                None => break,
            }
        }
        while self.log.len() > REV_LOG_LEN
            && self.log.front().is_some_and(|r| self.saved_rev.is_none_or(|saved| r.rev.saturating_sub(1) < saved))
        {
            self.log.pop_front();
        }
    }

    /// Whether the project differs from its saved state (01 §3.5): a new project is modified until saved.
    pub fn dirty(&self) -> bool {
        self.permanent_dirty || self.history.dirty()
    }
}

/// An event (11 §9; spec A `event` commands): name and payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    /// Event id (`doc.changed`, `app.state.changed`, …).
    pub name: String,
    /// Payload per the event's schema.
    pub payload: Value,
}

/// Everything the document thread owns (cloned to simulate a dry-run batch; the job table is shared, and simulations
/// never apply effects to it).
#[derive(Debug, Default, Clone)]
pub(crate) struct Session {
    pub project: Option<OpenProject>,
    pub app: AppContext,
    pub ui: UiState,
    pub jobs: Arc<JobTable>,
    pub events: Vec<Event>,
    /// The kernel clipboard (01 §12.4 `Clipboard`), `null` when empty. Shared and replaced whole, never edited in
    /// place, so prechecks can read facts about it once per clipboard (`memo`, ruling O-24).
    pub clipboard: Option<std::sync::Arc<Value>>,
    /// The remembered options of cut special, paste special and motion-point paste special (01 §6.3).
    pub edit_options: std::collections::BTreeMap<String, serde_json::Map<String, Value>>,
    /// The persistence environment (C1.3).
    pub env: crate::store::Env,
    /// Agent sessions, confirmations and the activity log (C1.4). A refused call restores its sessions and policy
    /// (C23-R3-01); its confirmation requests and activity entries are kept.
    pub agents: crate::agents::AgentHub,
    /// The derived planar-map cache (M2 C2.2), shared by the session's invocations.
    pub planar: crate::planar::SharedCache,
    /// The drawing thumbnails' cache, shared with the window's background thumbnails (M5 ruling E-56); `None` in a
    /// session made without an executor (unit tests), where the derived thumbnail is rendered and not kept.
    pub thumbs: Option<std::sync::Arc<crate::thumbnails::Service>>,
    /// Timeline and Exposure Sheet view state that is not document content (M3 C3.1): solo mode, category colours,
    /// the static light tables (SI-085).
    pub timeline: crate::commands::timing::TimelineState,
    /// The project, document revision and selection lists the last context repair checked: while all three are the
    /// same, the lists need no new check (a large cell selection is not rescanned on every call; C31-R1-14).
    pub selection_checked: Option<(String, u64, Vec<crate::app::Ids>)>,
}

impl Session {
    pub fn emit(&mut self, name: &str, payload: Value) {
        self.events.push(Event { name: name.to_owned(), payload });
    }
}
