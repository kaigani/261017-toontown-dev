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
    /// The step's `anim.changed` payload (the union its call emitted, 05 §19.4), emitted again by each undo and redo
    /// of the step (C32-R2-02); `None` when the step changed no animation.
    pub anim: Option<serde_json::Value>,
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
    /// Steps below this position cannot be undone (the "Repair on Open" base step, 10 REP-01).
    floor: usize,
    /// The open typing run (M4 C4.4b, design note 5 §3; SI-191): the step the Text tool's input path extends while its
    /// keystrokes go to one text object from one origin.
    run: Option<TypingRun>,
}

/// An open typing run: the step it extends, the text object and the origin typing into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypingRun {
    pub step_id: String,
    pub text_id: String,
    pub origin: String,
}

impl History {
    /// A fresh history whose current position is clean (a project just opened or created).
    pub fn clean() -> Self {
        Self { clean: Some(0), ..Self::default() }
    }

    /// The id the next step will get.
    pub fn next_step_id(&mut self) -> String {
        self.next_id = self.next_id.saturating_add(1);
        format!("u-{}", self.next_id)
    }

    /// Records a new step: discards redo steps, enforces the depth bound. It ends any typing run (a keystroke that opens
    /// a run opens it after the push, [`History::open_run`]).
    pub fn push(&mut self, step: Step) {
        self.run = None;
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
            self.floor = self.floor.saturating_sub(1);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.position > self.floor
    }

    /// A history starting with one step that cannot be undone and is not clean (10 REP-01: repairs on open).
    pub fn with_base_step(step: Step) -> Self {
        Self { steps: vec![step], position: 1, clean: None, next_id: 1, floor: 1, run: None }
    }

    /// The ID of the step just before `position` (the step undo would reverse there).
    pub fn step_id_before(&self, position: usize) -> Option<&str> {
        position.checked_sub(1).and_then(|i| self.steps.get(i)).map(|s| s.id.as_str())
    }

    /// Marks `position` clean if the history still leads there through the step `top` (SAVE-04: the clean marker is
    /// the saved snapshot's undo position); otherwise no position is clean.
    pub fn mark_clean_at(&mut self, position: usize, top: Option<&str>) {
        // A save ends the typing run (design note 5 §3).
        self.run = None;
        let same = position <= self.steps.len() && self.step_id_before(position) == top;
        self.clean = if same { Some(position) } else { None };
    }

    pub fn can_redo(&self) -> bool {
        self.position < self.steps.len()
    }

    /// The step undo would reverse.
    pub fn top(&self) -> Option<&Step> {
        self.position.checked_sub(1).and_then(|i| self.steps.get(i))
    }

    pub fn moved_back(&mut self) {
        self.run = None;
        self.position = self.position.saturating_sub(1);
    }

    pub fn moved_forward(&mut self) {
        self.run = None;
        self.position = (self.position + 1).min(self.steps.len());
    }

    /// Whether a keystroke of `origin` into `text_id` extends the open run (design note 5 §3): the run is open, its
    /// step is the top one at the end of the history (nothing undone), same object and origin, and the position is not
    /// the clean (saved) one.
    pub fn extends_run(&self, text_id: &str, origin: &str) -> bool {
        self.run.as_ref().is_some_and(|r| {
            r.text_id == text_id
                && r.origin == origin
                && self.position == self.steps.len()
                && self.top().is_some_and(|t| t.id == r.step_id)
                && self.clean != Some(self.position)
        })
    }

    /// Opens a run on the top step.
    pub fn open_run(&mut self, text_id: &str, origin: &str) {
        self.run = self.top().map(|t| TypingRun {
            step_id: t.id.clone(),
            text_id: text_id.to_owned(),
            origin: origin.to_owned(),
        });
    }

    /// Ends the open run (a caret move, a highlight, a format change, leaving the session: design note 5 §3).
    pub fn end_run(&mut self) {
        self.run = None;
    }

    /// Extends the top step with a keystroke's commits (design note 5 §3): appended in commit order; when the step's
    /// last commit and the new one are each one `replace` at the same pointer of the same entry, they fold into one
    /// (the first undo, the new redo), so a run of any length undoes with one patch per entry. The step keeps its id
    /// and label; `touched` is unioned; the time is the keystroke's.
    pub fn extend_top(&mut self, edit: StepEdit, touched: Vec<String>, structural: bool, time: String) {
        let Some(top) = self.position.checked_sub(1).and_then(|i| self.steps.get_mut(i)) else { return };
        let (StepEdit::Patches(have), StepEdit::Patches(new)) = (&mut top.edit, edit) else { return };
        for c in new {
            let fold = have.last().is_some_and(|last| {
                last.key == c.key
                    && last.redo.len() == 1
                    && c.redo.len() == 1
                    && last.undo.len() == 1
                    && c.undo.len() == 1
                    && last.redo[0].op == "replace"
                    && c.redo[0].op == "replace"
                    && last.redo[0].path == c.redo[0].path
                    && last.undo[0].path == c.undo[0].path
            });
            if fold && let Some(last) = have.last_mut() {
                last.redo = c.redo;
            } else {
                have.push(c);
            }
        }
        for t in touched {
            if !top.touched.contains(&t) {
                top.touched.push(t);
            }
        }
        top.structural |= structural;
        top.time = time;
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
