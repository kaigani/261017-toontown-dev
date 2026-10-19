//! The undo history (01 §6.1, §10; 11 §7.7): one stack per open project, steps labelled and attributed, a clean
//! marker for the saved position (01 §3.5), redo discarded by a new change, at least 200 steps.

use tt_model::document::{Document, EntryCommit};

use crate::types::Origin;

/// How a step's document change is recorded.
#[derive(Debug, Clone)]
pub(crate) enum StepEdit {
    /// Entry-level canonical undo/redo patches from `Document::with_patched_entries`, in commit order.
    Patches(Vec<EntryCommit>),
    /// Whole-document snapshots (checkpoint restore): undo restores `before`, redo restores `after`. Unchanged
    /// entries are shared with the live document (copy-on-write).
    Snapshot { before: Box<Document>, after: Box<Document> },
}

/// One undo step.
#[derive(Debug, Clone)]
pub(crate) struct Step {
    /// `u-<n>`, unique within the session.
    pub id: String,
    /// Undo label.
    pub label: String,
    /// Who made it.
    pub origin: Origin,
    /// Commit time (ISO 8601 UTC).
    pub time: String,
    /// The change.
    pub edit: StepEdit,
    /// Index keys of the objects whose content the step changed (stamped again by undo and redo, REV-03).
    pub touched: Vec<String>,
    /// The step creates, deletes or moves objects (its change records come from entry diffs).
    pub structural: bool,
}

/// Maximum number of steps kept; the oldest are discarded first (01 §6.1: at least 200).
pub const MAX_STEPS: usize = 1000;

/// The history of the open project.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    steps: Vec<Step>,
    /// Number of applied steps (the position after the last applied step).
    position: usize,
    /// Position at the last save; `None` when it was discarded (the project cannot become clean by undo/redo).
    clean: Option<usize>,
    next_id: u64,
}

impl History {
    /// A fresh history whose current position is clean (a project just opened or created).
    pub fn clean() -> Self {
        Self { clean: Some(0), ..Self::default() }
    }

    /// The id the next step will get.
    pub fn next_step_id(&mut self) -> String {
        self.next_id += 1;
        format!("u-{}", self.next_id)
    }

    /// Records a new step: discards redo steps, enforces the depth bound.
    pub fn push(&mut self, step: Step) {
        self.steps.truncate(self.position);
        if self.clean.is_some_and(|c| c > self.position) {
            self.clean = None;
        }
        self.steps.push(step);
        self.position = self.steps.len();
        while self.steps.len() > MAX_STEPS {
            self.steps.remove(0);
            self.position -= 1;
            self.clean = match self.clean {
                Some(0) | None => None,
                Some(c) => Some(c - 1),
            };
        }
    }

    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    pub fn can_redo(&self) -> bool {
        self.position < self.steps.len()
    }

    /// The step undo would reverse.
    pub fn top(&self) -> Option<&Step> {
        self.position.checked_sub(1).and_then(|i| self.steps.get(i))
    }

    pub fn moved_back(&mut self) {
        self.position = self.position.saturating_sub(1);
    }

    pub fn moved_forward(&mut self) {
        self.position = (self.position + 1).min(self.steps.len());
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn clean_position(&self) -> Option<usize> {
        self.clean
    }

    /// The project is modified when the position differs from the clean marker (01 §3.5).
    pub fn dirty(&self) -> bool {
        self.clean != Some(self.position)
    }

    /// Marks the current position clean (a save, C1.3).
    #[allow(dead_code, reason = "used by project.save in C1.3")]
    pub fn mark_clean(&mut self) {
        self.clean = Some(self.position);
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Replaces a step's edit with the patches its last replay returned (C11-R6-01: the redo returned by undo).
    pub fn replace_edit(&mut self, index: usize, edit: StepEdit) {
        if let Some(s) = self.steps.get_mut(index) {
            s.edit = edit;
        }
    }
}
