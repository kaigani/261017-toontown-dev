//! Exposure algebra (04 §13, §17.2; 10 §4.7): the timing of a cell-based element as a frame array, its canonical
//! run-length form, and every cell operation of 04 §13 as a pure function over frame arrays.
//!
//! Frames are 1-based in every public signature, as in the spec; a frame array holds one entry per frame from frame 1
//! (`None` = blank). Frames after the end of the array are blank. Every function is deterministic and does work
//! proportional to the frames it reads or writes; callers charge their work account with [`cost`] before calling and
//! refuse results longer than [`MAX_FRAMES`] (10 §5) before anything is staged.

use std::cmp::Ordering;

/// The longest scene (10 §5: scene length ≤ 100 000 frames, `doc.limitExceeded`).
pub const MAX_FRAMES: usize = 100_000;

/// Largest frame count or frame number a cell command accepts (04 §13: 1–9999 for counts and target frames).
pub const MAX_COMMAND_FRAMES: u32 = 9_999;

/// One maximal run of equal cells (blank runs included): 1-based first frame, length, value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run<D> {
    /// First frame (1-based).
    pub from: usize,
    /// Number of frames (≥ 1).
    pub len: usize,
    /// The drawing, or `None` for blank cells.
    pub value: Option<D>,
}

impl<D> Run<D> {
    /// Last frame (1-based, inclusive).
    #[must_use]
    pub fn to(&self) -> usize {
        self.from + self.len - 1
    }
}

/// The work units an operation over `frames` frames costs (one unit per frame read or written, at least one).
#[must_use]
pub fn cost(frames: usize) -> u64 {
    u64::try_from(frames).unwrap_or(u64::MAX).max(1)
}

/// Expands canonical runs `[value, count]` into a frame array.
#[must_use]
pub fn expand<D: Clone>(runs: &[(Option<D>, u32)]) -> Vec<Option<D>> {
    let mut out = Vec::new();
    for (v, n) in runs {
        out.extend(std::iter::repeat_n(v.clone(), *n as usize));
    }
    out
}

/// The total frame count of runs (without expanding them).
#[must_use]
pub fn runs_len<D>(runs: &[(Option<D>, u32)]) -> usize {
    runs.iter().map(|(_, n)| *n as usize).sum()
}

/// The canonical runs of a frame array (10 §4.7): adjacent equal values merged, trailing blank run omitted.
#[must_use]
pub fn compress<D: Clone + PartialEq>(frames: &[Option<D>]) -> Vec<(Option<D>, u32)> {
    let mut runs: Vec<(Option<D>, u32)> = Vec::new();
    for f in frames {
        match runs.last_mut() {
            Some((v, n)) if v == f => *n = n.saturating_add(1),
            _ => runs.push((f.clone(), 1)),
        }
    }
    while runs.last().is_some_and(|(v, _)| v.is_none()) {
        runs.pop();
    }
    runs
}

/// Normalises runs read from input to canonical form (merges equal neighbours, drops zero counts and the trailing
/// blank run).
#[must_use]
pub fn canonical<D: Clone + PartialEq>(runs: &[(Option<D>, u32)]) -> Vec<(Option<D>, u32)> {
    let mut out: Vec<(Option<D>, u32)> = Vec::new();
    for (v, n) in runs.iter().filter(|(_, n)| *n > 0) {
        match out.last_mut() {
            Some((w, m)) if w == v => *m = m.saturating_add(*n),
            _ => out.push((v.clone(), *n)),
        }
    }
    while out.last().is_some_and(|(v, _)| v.is_none()) {
        out.pop();
    }
    out
}

/// The last exposed (non-blank) frame, 0 when every cell is blank.
#[must_use]
pub fn last_exposed<D>(frames: &[Option<D>]) -> usize {
    frames.iter().rposition(Option::is_some).map_or(0, |i| i + 1)
}

/// Drops trailing blanks.
pub fn trim<D>(frames: &mut Vec<Option<D>>) {
    let n = last_exposed(frames);
    frames.truncate(n);
}

/// The cell at a 1-based frame (blank beyond the end).
#[must_use]
pub fn at<D: Clone>(frames: &[Option<D>], frame: usize) -> Option<D> {
    frame.checked_sub(1).and_then(|i| frames.get(i)).cloned().flatten()
}

/// The maximal runs of equal cells of a frame array (blank runs included; trailing blanks excluded).
#[must_use]
pub fn runs<D: Clone + PartialEq>(frames: &[Option<D>]) -> Vec<Run<D>> {
    let mut out: Vec<Run<D>> = Vec::new();
    for (i, f) in frames[..last_exposed(frames)].iter().enumerate() {
        match out.last_mut() {
            Some(r) if r.value == *f => r.len += 1,
            _ => out.push(Run { from: i + 1, len: 1, value: f.clone() }),
        }
    }
    out
}

/// The run containing a frame; frames past the last exposure belong to an unbounded trailing blank run (returned with
/// `len` reaching `frame`).
#[must_use]
pub fn run_at<D: Clone + PartialEq>(frames: &[Option<D>], frame: usize) -> Run<D> {
    let end = last_exposed(frames);
    if frame > end || frame == 0 {
        return Run { from: end + 1, len: frame.saturating_sub(end).max(1), value: None };
    }
    let i = frame - 1;
    let v = frames[i].clone();
    let mut a = i;
    while a > 0 && frames[a - 1] == v {
        a -= 1;
    }
    let mut b = i;
    while b + 1 < end && frames[b + 1] == v {
        b += 1;
    }
    Run { from: a + 1, len: b - a + 1, value: v }
}

/// The runs intersecting `from..=to`, clipped to the exposed part: each distinct exposure (or blank run) once.
fn intersecting<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize) -> Vec<Run<D>> {
    runs(frames).into_iter().filter(|r| r.from <= to && r.to() >= from).collect()
}

/// A changed frame window `(from, to)` (1-based, inclusive).
pub type Window = (usize, usize);

/// The window that differs between two frame arrays (`None` when equal).
#[must_use]
pub fn diff<D: PartialEq>(before: &[Option<D>], after: &[Option<D>]) -> Option<Window> {
    let len = before.len().max(after.len());
    let differs = |i: usize| before.get(i).and_then(Option::as_ref) != after.get(i).and_then(Option::as_ref);
    let first = (0..len).find(|&i| differs(i))?;
    let last = (0..len).rev().find(|&i| differs(i))?;
    Some((first + 1, last + 1))
}

/// 13.1 add exposure: each distinct exposure (or blank run) intersecting `from..=to` is held `n` frames longer; the
/// following cells shift later. Frames past the last exposure are blank and inserting blanks there changes nothing.
#[must_use]
pub fn add_exposure<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize, n: usize) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    // Insert from the back so earlier positions stay valid.
    for r in intersecting(frames, from, to).into_iter().rev() {
        let at = r.to();
        out.splice(at..at, std::iter::repeat_n(r.value.clone(), n));
    }
    trim(&mut out);
    out
}

/// 13.2 remove exposure: each distinct exposure (or blank run) intersecting `from..=to` is shortened by `n` frames
/// (removed when it is at most `n` long); the following cells shift earlier. The drawing stays in the bank.
#[must_use]
pub fn remove_exposure<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize, n: usize) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    for r in intersecting(frames, from, to).into_iter().rev() {
        let k = n.min(r.len);
        let end = r.to();
        out.drain(end - k..end);
    }
    trim(&mut out);
    out
}

/// 13.3/13.4 set exposure: every distinct exposure intersecting `from..=to` is re-timed to `len` frames; blank runs
/// keep their length (04 §18 #11); the rest of the column shifts so the sequence of drawings is preserved.
#[must_use]
pub fn set_exposure<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize, len: usize) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    for r in intersecting(frames, from, to).into_iter().rev() {
        if r.value.is_none() || r.len == len {
            continue;
        }
        let start = r.from - 1;
        out.splice(start..start + r.len, std::iter::repeat_n(r.value.clone(), len));
    }
    trim(&mut out);
    out
}

/// The outcome of [`fill`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillError {
    /// The cell at `frame` is blank: there is no drawing to extend.
    Blank,
    /// `to_frame` is before `frame` (the minimum).
    BeforeSelection {
        /// The minimum target frame.
        minimum: usize,
    },
}

/// 13.5 extend exposure: exposes the drawing at `frame` through `to_frame`, overwriting the cells between the end of
/// its exposure and the target; later cells do not shift.
///
/// # Errors
/// [`FillError::Blank`] on a blank cell, [`FillError::BeforeSelection`] when `to_frame < frame`.
pub fn fill<D: Clone + PartialEq>(
    frames: &[Option<D>],
    frame: usize,
    to_frame: usize,
) -> Result<Vec<Option<D>>, FillError> {
    if to_frame < frame {
        return Err(FillError::BeforeSelection { minimum: frame });
    }
    let r = run_at(frames, frame);
    let Some(v) = r.value.clone() else { return Err(FillError::Blank) };
    let mut out = frames.to_vec();
    if out.len() < to_frame {
        out.resize(to_frame, None);
    }
    for c in out.iter_mut().take(to_frame).skip(r.to()) {
        *c = Some(v.clone());
    }
    trim(&mut out);
    Ok(out)
}

/// Inserts `values` before the 1-based frame `at` (frames past the end are blank-padded first when `values` holds a
/// drawing); the following cells shift later.
#[must_use]
pub fn insert<D: Clone + PartialEq>(frames: &[Option<D>], at: usize, values: &[Option<D>]) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    let i = at.saturating_sub(1);
    if i > out.len() {
        if values.iter().all(Option::is_none) {
            return out;
        }
        out.resize(i, None);
    }
    out.splice(i..i, values.iter().cloned());
    trim(&mut out);
    out
}

/// 13.7 insert blank cells at a frame (the current and following cells shift later).
#[must_use]
pub fn insert_blank<D: Clone + PartialEq>(frames: &[Option<D>], frame: usize, count: usize) -> Vec<Option<D>> {
    insert(frames, frame, &vec![None; count])
}

/// 13.8 delete blank cells `frame..frame+count-1`; `None` when one of them is not blank (`cell.notBlank`).
#[must_use]
pub fn remove_blank<D: Clone + PartialEq>(frames: &[Option<D>], frame: usize, count: usize) -> Option<Vec<Option<D>>> {
    let a = frame.saturating_sub(1);
    let b = a.saturating_add(count);
    if frames.iter().take(b).skip(a).any(Option::is_some) {
        return None;
    }
    let mut out = frames.to_vec();
    if a < out.len() {
        out.drain(a..b.min(out.len()));
    }
    trim(&mut out);
    Some(out)
}

/// 13.17 delete cells `from..=to`: removed and following cells shifted earlier (`shift`), or cleared to blanks.
#[must_use]
pub fn delete_range<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize, shift: bool) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    let a = from.saturating_sub(1).min(out.len());
    let b = to.min(out.len());
    if a < b {
        if shift {
            out.drain(a..b);
        } else {
            for c in &mut out[a..b] {
                *c = None;
            }
        }
    }
    trim(&mut out);
    out
}

/// The cells `from..=to` (blank-padded past the end).
#[must_use]
pub fn slice<D: Clone>(frames: &[Option<D>], from: usize, to: usize) -> Vec<Option<D>> {
    (from..=to).map(|f| at(frames, f)).collect()
}

/// Overwrites the cells from `at` with `values` (no shift).
#[must_use]
pub fn overwrite<D: Clone + PartialEq>(frames: &[Option<D>], at: usize, values: &[Option<D>]) -> Vec<Option<D>> {
    let mut out = frames.to_vec();
    let i = at.saturating_sub(1);
    let end = i + values.len();
    if out.len() < end {
        out.resize(end, None);
    }
    out[i..end].clone_from_slice(values);
    trim(&mut out);
    out
}

/// 13.9 create cycle: the range `from..=to` appears `cycles` times in total; copies are inserted after it.
#[must_use]
pub fn cycle<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize, cycles: usize) -> Vec<Option<D>> {
    let pattern = slice(frames, from, to);
    let copies: Vec<Option<D>> =
        std::iter::repeat_n(pattern.iter().cloned(), cycles.saturating_sub(1)).flatten().collect();
    // The copies go after `to` even when the range ends in blanks past the last exposure.
    let mut out = frames.to_vec();
    if out.len() < to {
        out.resize(to, None);
    }
    out.splice(to..to, copies);
    trim(&mut out);
    out
}

/// Columnate loop modes (13.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// One pass.
    None,
    /// The run restarts from the first drawing each loop.
    Continuous,
    /// Each loop goes forward then backward without repeating the end drawings.
    ForwardBackward,
}

/// A computed columnate cycle (13.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Columnate<D> {
    /// The drawings in order with their exposure lengths.
    pub sequence: Vec<(D, usize)>,
    /// Total frames.
    pub total_frames: usize,
    /// Loops.
    pub loops: usize,
}

/// The drawings of one loop: `D` (none, continuous) or `D` followed by reverse(`D`) without its first and last
/// elements (forward-backward; as continuous when `n ≤ 2` leaves nothing to reverse, and when `n = 1`).
fn one_loop<D: Clone>(d: &[D], mode: LoopMode) -> Vec<D> {
    let mut out = d.to_vec();
    if mode == LoopMode::ForwardBackward && d.len() > 2 {
        out.extend(d[1..d.len() - 1].iter().rev().cloned());
    }
    out
}

/// 13.10 columnate. `drawings` is D (first to last in bank order, already reversed when last precedes first);
/// `each` = expose-each, `last` = expose-last; `loops` (continuous / forward-backward; 1 for none) or `total_frames`
/// (when given, loops = ceil(frames ÷ frames-per-loop) and the last loop is truncated to the requested count).
#[must_use]
pub fn columnate<D: Clone>(
    drawings: &[D],
    mode: LoopMode,
    each: usize,
    last: usize,
    loops: usize,
    total_frames: Option<usize>,
) -> Columnate<D> {
    if drawings.is_empty() {
        return Columnate { sequence: Vec::new(), total_frames: 0, loops: 0 };
    }
    let unit = one_loop(drawings, mode);
    let loops = if mode == LoopMode::None { 1 } else { loops.max(1) };
    // The full (untruncated) sequence for `k` loops: every drawing `each` frames except the final one, `last`.
    let full = |k: usize| -> Vec<(D, usize)> {
        let mut s: Vec<(D, usize)> =
            std::iter::repeat_n(unit.iter().cloned(), k).flatten().map(|d| (d, each)).collect();
        if let Some(l) = s.last_mut() {
            l.1 = last;
        }
        s
    };
    match total_frames {
        None => {
            let sequence = full(loops);
            let total_frames = sequence.iter().map(|(_, n)| n).sum();
            Columnate { sequence, total_frames, loops }
        }
        Some(t) => {
            let per_loop = (unit.len() * each).max(1);
            let k = if mode == LoopMode::None { 1 } else { t.div_ceil(per_loop).max(1) };
            let mut sequence = full(k);
            // Truncate (or, for a single pass, never extend) to the requested frame count.
            let mut left = t;
            let mut out = Vec::new();
            for (d, n) in sequence.drain(..) {
                if left == 0 {
                    break;
                }
                let take = n.min(left);
                out.push((d, take));
                left -= take;
            }
            let total_frames = out.iter().map(|(_, n)| n).sum();
            Columnate { sequence: out, total_frames, loops: k }
        }
    }
}

/// The frames of a columnate sequence.
#[must_use]
pub fn sequence_frames<D: Clone>(sequence: &[(D, usize)]) -> Vec<Option<D>> {
    sequence.iter().flat_map(|(d, n)| std::iter::repeat_n(Some(d.clone()), *n)).collect()
}

/// Whether a window run continues outside the window (04 §17.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continues {
    /// Starts before the window.
    Before,
    /// Ends after the window.
    After,
    /// Both.
    Both,
}

impl Continues {
    /// The JSON value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
            Self::Both => "both",
        }
    }
}

/// A run read through a frame window: clipped to the window, flagged when it continues outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRun<D> {
    /// The drawing.
    pub value: D,
    /// First frame inside the window.
    pub from: usize,
    /// Last frame inside the window.
    pub to: usize,
    /// Continuation outside the window.
    pub continues: Option<Continues>,
}

/// 04 §17.2 read: the exposed runs intersecting `from..=to`, sorted, blanks omitted, clipped and flagged.
#[must_use]
pub fn window<D: Clone + PartialEq>(frames: &[Option<D>], from: usize, to: usize) -> Vec<WindowRun<D>> {
    let mut out = Vec::new();
    for r in intersecting(frames, from, to) {
        let Some(v) = r.value.clone() else { continue };
        let before = r.from < from;
        let after = r.to() > to;
        let continues = match (before, after) {
            (true, true) => Some(Continues::Both),
            (true, false) => Some(Continues::Before),
            (false, true) => Some(Continues::After),
            (false, false) => None,
        };
        out.push(WindowRun { value: v, from: r.from.max(from), to: r.to().min(to), continues });
    }
    out
}

/// Natural order of drawing names (10 §4.8 "displayed in natural name order"): digit runs compare numerically
/// (ignoring leading zeros, then by their length so `01` follows `1`), other characters by code point.
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut p = String::new();
                while let Some(c) = x.next_if(char::is_ascii_digit) {
                    p.push(c);
                }
                let mut q = String::new();
                while let Some(d) = y.next_if(char::is_ascii_digit) {
                    q.push(d);
                }
                let o = numeric_cmp(&p, &q).then(p.len().cmp(&q.len()));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(&d);
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Compares two decimal digit strings by value.
fn numeric_cmp(p: &str, q: &str) -> Ordering {
    let p = p.trim_start_matches('0');
    let q = q.trim_start_matches('0');
    p.len().cmp(&q.len()).then_with(|| p.cmp(q))
}

/// 04 §14.2 default new drawing name: the next integer after the highest purely numeric name (`1` when there is
/// none); non-numeric names do not count. Decimal strings of any length are incremented exactly.
#[must_use]
pub fn next_drawing_name<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let highest = names
        .into_iter()
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .map(|n| n.trim_start_matches('0'))
        .max_by(|a, b| numeric_cmp(a, b));
    match highest {
        None => "1".to_owned(),
        Some(h) => increment(h),
    }
}

/// The next `count` default drawing names after `names` (13.6 named cells: `6`, `7`, … after `5`).
#[must_use]
pub fn next_drawing_names<'a>(names: impl IntoIterator<Item = &'a str>, count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    let mut cur = next_drawing_name(names);
    for _ in 0..count {
        let next = increment(&cur);
        out.push(std::mem::replace(&mut cur, next));
    }
    out
}

/// Adds one to a decimal digit string (`""` and `"0"` give `"1"`).
fn increment(n: &str) -> String {
    let mut digits: Vec<u8> = n.bytes().collect();
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        if digits[i] == b'9' {
            digits[i] = b'0';
        } else {
            digits[i] += 1;
            break;
        }
    }
    let s = String::from_utf8(digits).unwrap_or_default();
    let t = s.trim_start_matches('0');
    if t.is_empty() { "1".to_owned() } else { t.to_owned() }
}

/// 04 §3.2 / §8.1 unique element names: `base` when free (unless `skip_base`), else `base_1`, `base_2`, … skipping
/// used names; `count` names, each distinct.
#[must_use]
pub fn unique_names(base: &str, taken: &dyn Fn(&str) -> bool, count: usize, skip_base: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(count);
    let used = |n: &str, out: &[String]| taken(n) || out.iter().any(|o| o == n);
    if !skip_base && !used(base, &out) && count > 0 {
        out.push(base.to_owned());
    }
    let mut k: u64 = 1;
    while out.len() < count {
        let n = format!("{base}_{k}");
        if !used(&n, &out) {
            out.push(n);
        }
        k += 1;
    }
    out
}
