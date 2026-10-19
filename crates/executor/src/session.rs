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
}

/// Transactions kept for `doc.diff` (10 AG-09: at least the last 10 000).
pub const REV_LOG_LEN: usize = 10_000;
/// Records that keep their previous snapshot for fast diffs.
pub const SNAPSHOT_WINDOW: usize = 256;

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
}

impl OpenProject {
    pub fn new(doc: Document, path: Option<String>, read_only: bool) -> Self {
        Self {
            saved_rev: path.as_ref().map(|_| doc.document_rev()),
            form: path.as_ref().map(|_| "package".to_owned()),
            doc,
            history: History::clean(),
            checkpoints: Vec::new(),
            log: VecDeque::new(),
            path,
            read_only,
        }
    }

    /// The oldest revision `doc.diff` can start from.
    pub fn history_from_rev(&self) -> u64 {
        self.log.front().map_or(self.doc.document_rev(), |r| r.rev - 1)
    }

    /// Records a committed transaction.
    pub fn log_commit(&mut self, rev: u64, back: Back, before: Document) {
        self.log.push_back(RevRecord { rev, back, before: Some(before) });
        if self.log.len() > SNAPSHOT_WINDOW
            && let Some(old) = self.log.get_mut(self.log.len() - SNAPSHOT_WINDOW - 1)
        {
            old.before = None;
        }
        while self.log.len() > REV_LOG_LEN {
            self.log.pop_front();
        }
    }

    /// Whether the project differs from its saved state (01 §3.5): a new project is modified until saved.
    pub fn dirty(&self) -> bool {
        self.path.is_none() || self.history.dirty()
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

/// Everything the document thread owns.
#[derive(Debug, Default)]
pub(crate) struct Session {
    pub project: Option<OpenProject>,
    pub app: AppContext,
    pub ui: UiState,
    pub jobs: Arc<JobTable>,
    pub events: Vec<Event>,
    /// The kernel clipboard (01 §12.4 `Clipboard`), `null` when empty.
    pub clipboard: Option<Value>,
}

impl Session {
    pub fn emit(&mut self, name: &str, payload: Value) {
        self.events.push(Event { name: name.to_owned(), payload });
    }
}
