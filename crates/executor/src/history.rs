//! The undo history (01 §6.1, §10; 11 §7.7): one stack per open project, steps labelled and attributed, a clean
//! marker for the saved position (01 §3.5), redo discarded by a new change, at least 200 steps.
//!
//! The memory bound (01 §6.1: "at least 200 steps; beyond that, unlimited subject to a memory bound, the oldest steps
//! discarded first"; M5 ruling E-51, E-60): each step's size is measured from its patch when it is recorded; while the
//! total passes [`MEMORY_BOUND`] the oldest steps are discarded, never below [`MIN_STEPS`]; the [`MAX_STEPS`] limit
//! stays. When the last 200 steps alone pass the bound they are kept and stderr says so once per history. 01 §7.3 gives
//! no preference for the bound: a fixed 1 GiB, SI-217.

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

/// The steps the memory bound never discards (01 §6.1: at least 200).
pub const MIN_STEPS: usize = 200;

/// The history's memory bound in bytes (M5 ruling E-51, E-60; SI-217: no 01 §7.3 preference names it).
pub const MEMORY_BOUND: usize = 1 << 30;

/// The size of a step's change in bytes, measured once when it is recorded: for patches, every operation's pointers and
/// the serialised length of its values (the new and the previous one); for a snapshot step, the serialised length of
/// the entries `before` and `after` do not share (the shared ones are the live document's, copy-on-write).
pub(crate) fn edit_size(edit: &StepEdit) -> usize {
    match edit {
        StepEdit::Patches(commits) => {
            commits.iter().map(|c| c.undo.iter().chain(&c.redo).map(op_size).sum::<usize>() + KEY_SIZE).sum()
        }
        StepEdit::Snapshot { before, after } => {
            let mut keys = before.entry_keys();
            for k in after.entry_keys() {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
            keys.iter()
                .filter(|k| !before.same_entry(after, k))
                .map(|k| entry_size(before, k) + entry_size(after, k) + KEY_SIZE)
                .sum()
        }
    }
}

/// What one entry key costs in a step besides its operations (its key and the record around it).
const KEY_SIZE: usize = 64;

fn op_size(op: &tt_model::change::PatchOp) -> usize {
    op.path.len()
        + op.from.as_ref().map_or(0, String::len)
        + json_len(op.value.as_ref())
        + json_len(op.old_value.as_ref())
}

fn entry_size(doc: &Document, key: &tt_model::document::EntryKey) -> usize {
    doc.entry_raw_json(key).map_or(0, |v| json_len(Some(&v)))
}

/// The serialised length of a value, counted without building the text.
fn json_len(v: Option<&serde_json::Value>) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let Some(v) = v else { return 0 };
    let mut c = Count(0);
    match serde_json::to_writer(&mut c, v) {
        Ok(()) => c.0,
        Err(_) => 0,
    }
}

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
    /// Each step's size in bytes ([`edit_size`]), index for index with `steps`.
    sizes: Vec<usize>,
    /// The sum of `sizes`.
    bytes: usize,
    /// The bound in bytes ([`MEMORY_BOUND`]; a test sets a smaller one, [`History::with_bound`]).
    bound: Option<usize>,
    /// stderr has said that the last [`MIN_STEPS`] steps alone pass the bound (once per history).
    over_bound_said: bool,
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
        self.sizes.truncate(self.position);
        self.bytes = self.sizes.iter().sum();
        if self.clean.is_some_and(|c| c > self.position) {
            self.clean = None;
        }
        let size = edit_size(&step.edit);
        self.steps.push(step);
        self.sizes.push(size);
        self.bytes += size;
        self.position = self.steps.len();
        while self.steps.len() > MAX_STEPS {
            self.discard_oldest();
        }
        self.keep_bound();
    }

    /// Discards the oldest steps while the total passes the bound, never below [`MIN_STEPS`] (E-51); when the last
    /// [`MIN_STEPS`] alone pass it, they are kept and stderr says so once for this history.
    fn keep_bound(&mut self) {
        let bound = self.bound();
        while self.bytes > bound && self.steps.len() > MIN_STEPS {
            self.discard_oldest();
        }
        if self.bytes > bound && !self.over_bound_said {
            self.over_bound_said = true;
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "toontown: the undo history holds {} MB in its last {} steps, over its {} MB bound; they are kept \
                 (01 §6.1: at least {MIN_STEPS} steps)",
                self.bytes >> 20,
                self.steps.len(),
                bound >> 20
            );
        }
    }

    pub fn can_undo(&self) -> bool {
        self.position > self.floor
    }

    /// A history starting with one step that cannot be undone and is not clean (10 REP-01: repairs on open).
    pub fn with_base_step(step: Step) -> Self {
        let size = edit_size(&step.edit);
        Self {
            steps: vec![step],
            position: 1,
            clean: None,
            next_id: 1,
            floor: 1,
            run: None,
            sizes: vec![size],
            bytes: size,
            bound: None,
            over_bound_said: false,
        }
    }

    /// This history with the memory bound `bytes` instead of [`MEMORY_BOUND`] (tests of the bound).
    #[cfg(test)]
    pub fn with_bound(mut self, bytes: usize) -> Self {
        self.bound = Some(bytes);
        self
    }

    /// The memory bound in force: this history's, the dev-only hook's (`test-hooks`), or [`MEMORY_BOUND`].
    fn bound(&self) -> usize {
        #[cfg(feature = "test-hooks")]
        if let Some(b) = crate::hooks::history_bound() {
            return self.bound.unwrap_or(b);
        }
        self.bound.unwrap_or(MEMORY_BOUND)
    }

    /// The total size of the kept steps in bytes.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Discards the oldest step, keeping the position, the clean marker and the floor on the same steps.
    fn discard_oldest(&mut self) {
        self.steps.remove(0);
        let size = if self.sizes.is_empty() { 0 } else { self.sizes.remove(0) };
        self.bytes = self.bytes.saturating_sub(size);
        self.position = self.position.saturating_sub(1);
        self.clean = match self.clean {
            Some(0) | None => None,
            Some(c) => Some(c - 1),
        };
        self.floor = self.floor.saturating_sub(1);
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
        // The extended step's size, measured again; the bound applies to it as to a new step.
        let index = self.position - 1;
        let size = self.steps.get(index).map_or(0, |t| edit_size(&t.edit));
        if let Some(old) = self.sizes.get_mut(index) {
            self.bytes = self.bytes.saturating_sub(*old) + size;
            *old = size;
        }
        self.keep_bound();
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
        let size = edit_size(&edit);
        if let Some(s) = self.steps.get_mut(index) {
            s.edit = edit;
        }
        if let Some(old) = self.sizes.get_mut(index) {
            self.bytes = self.bytes.saturating_sub(*old) + size;
            *old = size;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tt_model::change::PatchOp;
    use tt_model::document::EntryKey;

    use super::*;

    /// A step whose patch carries `n` characters in each of its values (its size is about `2 n` bytes).
    fn step(i: usize, n: usize) -> Step {
        let op = |v: String, old: String| PatchOp {
            op: "replace",
            from: None,
            path: "/note".into(),
            range: None,
            value: Some(json!(v)),
            old_value: Some(json!(old)),
            elided: false,
        };
        let text = |c: char| std::iter::repeat_n(c, n).collect::<String>();
        Step {
            id: format!("u-{i}"),
            label: format!("Step {i}"),
            origin: Origin::User,
            time: String::new(),
            edit: StepEdit::Patches(vec![EntryCommit {
                key: EntryKey::Project,
                undo: vec![op(text('a'), text('b'))],
                redo: vec![op(text('b'), text('a'))],
            }]),
            touched: Vec::new(),
            structural: false,
            anim: None,
        }
    }

    #[test]
    fn a_step_is_measured_from_its_patch() {
        let small = edit_size(&step(1, 10).edit);
        let large = edit_size(&step(2, 10_000).edit);
        // Four values of n characters each, with their quotes, the pointers and the key.
        assert!((40_000..40_200).contains(&large), "{large}");
        assert!(large - small >= 4 * (10_000 - 10), "{small} {large}");
    }

    #[test]
    fn the_bound_holds_on_large_steps_and_the_oldest_go_first() {
        let size = edit_size(&step(0, 1_000).edit);
        let bound = 250 * size + size / 2;
        let mut h = History::clean().with_bound(bound);
        for i in 0..400 {
            h.push(step(i, 1_000));
            assert!(h.bytes() <= bound || h.len() <= MIN_STEPS, "step {i}: {} bytes in {} steps", h.bytes(), h.len());
        }
        assert_eq!(h.len(), 250);
        assert_eq!(h.bytes(), 250 * size);
        assert_eq!(h.steps()[0].id, "u-150", "the oldest are discarded first");
        assert_eq!(h.position(), 250);
        assert_eq!(h.clean_position(), None, "the clean position was discarded with its step");
    }

    #[test]
    fn the_floor_of_two_hundred_steps_is_never_crossed() {
        let size = edit_size(&step(0, 1_000).edit);
        let mut h = History::clean().with_bound(10 * size);
        for i in 0..300 {
            h.push(step(i, 1_000));
        }
        assert_eq!(h.len(), MIN_STEPS, "200 steps over the bound are kept");
        assert!(h.bytes() > 10 * size);
        assert!(h.over_bound_said, "stderr said so");
        // Said once: the flag stays set; nothing else changes it.
        h.push(step(300, 1_000));
        assert_eq!(h.len(), MIN_STEPS);
    }

    #[test]
    fn the_step_count_limit_stays() {
        let mut h = History::clean().with_bound(usize::MAX);
        for i in 0..(MAX_STEPS + 5) {
            h.push(step(i, 1));
        }
        assert_eq!(h.len(), MAX_STEPS);
    }

    #[test]
    fn undo_position_and_redo_survive_a_discard() {
        let size = edit_size(&step(0, 1_000).edit);
        let mut h = History::clean().with_bound(220 * size);
        for i in 0..220 {
            h.push(step(i, 1_000));
        }
        // Undo five, then a new larger step: redo is discarded, then the bound discards the oldest.
        for _ in 0..5 {
            h.moved_back();
        }
        assert!(h.can_redo());
        // About ten ordinary steps' worth: the oldest five go so the total fits the bound again.
        h.push(step(500, 10_000));
        assert!(!h.can_redo());
        assert!(h.bytes() <= 220 * size);
        assert_eq!(h.steps()[0].id, "u-5", "the oldest five were discarded");
        assert_eq!(h.position(), h.len());
        assert_eq!(h.top().map(|t| t.id.as_str()), Some("u-500"));
        // Every kept step can be undone down to the oldest kept one, and no further.
        let mut undone = 0;
        while h.can_undo() {
            h.moved_back();
            undone += 1;
        }
        assert_eq!(undone, h.len());
    }

    #[test]
    fn the_floor_follows_a_discard() {
        // A repaired open's base step (10 REP-01) cannot be undone; once discarded, nothing below the kept steps is.
        let size = edit_size(&step(0, 1_000).edit);
        let mut h = History::with_base_step(step(0, 1_000)).with_bound(MIN_STEPS * size);
        for i in 1..=(MIN_STEPS + 3) {
            h.push(step(i, 1_000));
        }
        assert_eq!(h.len(), MIN_STEPS);
        let mut undone = 0;
        while h.can_undo() {
            h.moved_back();
            undone += 1;
        }
        assert_eq!(undone, MIN_STEPS);
    }
}
