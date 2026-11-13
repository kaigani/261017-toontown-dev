//! Independent C3.1 round-1 array oracle. Seven properties, 4096 cases each, reviewer seeds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]

use tt_anim::exposure as x;

type Cells = Vec<Option<u8>>;
struct Rng(u64);
impl Rng {
    fn n(&mut self, max: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as usize % max
    }
    fn cells(&mut self) -> Cells {
        let len = self.n(100);
        (0..len)
            .map(|_| match self.n(6) {
                0 | 1 => None,
                k => Some(k as u8),
            })
            .collect()
    }
}
fn trimmed(mut a: Cells) -> Cells {
    while a.last() == Some(&None) {
        a.pop();
    }
    a
}
fn cell(a: &[Option<u8>], f: usize) -> Option<u8> {
    a.get(f - 1).copied().flatten()
}
fn groups(a: &[Option<u8>]) -> Vec<(Option<u8>, usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    while i < a.len() {
        let start = i;
        let value = a[i];
        while i < a.len() && a[i] == value {
            i += 1;
        }
        out.push((value, start + 1, i));
    }
    out
}
fn retime(a: Cells, from: usize, to: usize, n: usize, mode: u8) -> Cells {
    let mut out = vec![];
    for (value, start, end) in groups(&trimmed(a)) {
        let len = end - start + 1;
        let len = if start <= to && end >= from {
            match mode {
                0 => len + n,
                1 => len.saturating_sub(n),
                _ if value.is_some() => n,
                _ => len,
            }
        } else {
            len
        };
        out.extend(std::iter::repeat_n(value, len));
    }
    trimmed(out)
}

#[test]
fn canonical_4096() {
    let mut r = Rng(0x31a001);
    for _ in 0..4096 {
        let a = r.cells();
        let expected: Vec<_> = groups(&trimmed(a.clone())).iter().map(|(v, s, e)| (*v, (e - s + 1) as u32)).collect();
        assert_eq!(x::compress(&a), expected);
        assert_eq!(x::expand(&expected), trimmed(a));
        assert_eq!(x::canonical(&expected), expected);
    }
}
#[test]
fn insert_remove_4096() {
    let mut r = Rng(0x31a002);
    for _ in 0..4096 {
        let a = r.cells();
        let at = 1 + r.n(110);
        let n = 1 + r.n(10);
        let mut expected = a.clone();
        if at <= expected.len() {
            expected.splice(at - 1..at - 1, vec![None; n]);
        }
        let b = x::insert_blank(&a, at, n);
        assert_eq!(b, trimmed(expected));
        assert_eq!(x::remove_blank(&b, at, n), Some(trimmed(a)));
    }
}

/// The executor always supplies canonical columns. Keep the failing arbitrary-tail probe above separately.
#[test]
fn insert_remove_canonical_4096() {
    let mut r = Rng(0x31b002);
    for _ in 0..4096 {
        let a = trimmed(r.cells());
        let at = 1 + r.n(110);
        let n = 1 + r.n(10);
        let mut expected = a.clone();
        if at <= expected.len() {
            expected.splice(at - 1..at - 1, vec![None; n]);
        }
        let b = x::insert_blank(&a, at, n);
        assert_eq!(b, trimmed(expected));
        assert_eq!(x::remove_blank(&b, at, n), Some(a));
    }
}
#[test]
fn set_length_4096() {
    let mut r = Rng(0x31a003);
    for _ in 0..4096 {
        let a = r.cells();
        let from = 1 + r.n(110);
        let to = from + r.n(20);
        let n = 1 + r.n(9);
        assert_eq!(x::set_exposure(&a, from, to, n), retime(a, from, to, n, 2));
    }
}
#[test]
fn fill_4096() {
    let mut r = Rng(0x31a004);
    for _ in 0..4096 {
        let a = r.cells();
        let from = 1 + r.n(110);
        let to = from + r.n(25);
        if let Some(value) = cell(&a, from) {
            let mut expected = a.clone();
            expected.resize(expected.len().max(to), None);
            for f in from..=to {
                expected[f - 1] = Some(value);
            }
            assert_eq!(x::fill(&a, from, to).unwrap(), trimmed(expected));
        } else {
            assert_eq!(x::fill(&a, from, to), Err(x::FillError::Blank));
        }
    }
}
#[test]
fn cycle_4096() {
    let mut r = Rng(0x31a005);
    for _ in 0..4096 {
        let a = r.cells();
        let from = 1 + r.n(110);
        let to = from + r.n(15);
        let k = 1 + r.n(8);
        let mut expected = a.clone();
        expected.resize(expected.len().max(to), None);
        let pattern = expected[from - 1..to].to_vec();
        let mut out = expected[..to].to_vec();
        for _ in 1..k {
            out.extend_from_slice(&pattern);
        }
        out.extend_from_slice(&expected[to..]);
        assert_eq!(x::cycle(&a, from, to, k), trimmed(out));
    }
}
#[test]
fn add_remove_4096() {
    let mut r = Rng(0x31a006);
    for _ in 0..4096 {
        let a = r.cells();
        let from = 1 + r.n(110);
        let to = from + r.n(20);
        let n = 1 + r.n(9);
        assert_eq!(x::add_exposure(&a, from, to, n), retime(a.clone(), from, to, n, 0));
        assert_eq!(x::remove_exposure(&a, from, to, n), retime(a, from, to, n, 1));
    }
}
#[test]
fn diff_4096() {
    let mut r = Rng(0x31a007);
    for _ in 0..4096 {
        let a = r.cells();
        let b = r.cells();
        let changed: Vec<_> = (1..=a.len().max(b.len())).filter(|f| cell(&a, *f) != cell(&b, *f)).collect();
        assert_eq!(x::diff(&a, &b), changed.first().zip(changed.last()).map(|(a, b)| (*a, *b)));
    }
}
#[test]
fn worked_examples_and_edges() {
    let a = vec![Some(1), Some(1), Some(2), Some(2), Some(3), Some(3)];
    assert_eq!(x::set_exposure(&a, 1, 6, 1), vec![Some(1), Some(2), Some(3)]);
    assert_eq!(x::cycle(&a, 1, 6, 3), a.repeat(3));
    assert!(x::insert_blank::<u8>(&[], 1, 1).is_empty());
    assert!(x::remove_exposure(&[Some(1)], 1, 1, 1).is_empty());
    let long = vec![Some(2); 100_000];
    assert_eq!(x::window(&long, 2, 99_999)[0].continues, Some(x::Continues::Both));
    assert_eq!(x::delete_range(&long, 100_000, 100_000, true).len(), 99_999);
    assert_eq!(x::insert_blank(&long, 1, 1).len(), 100_001);
    let mut names = vec!["é10", "é2", "a01", "a1", "a10", "a2", "界2", "界10"];
    names.sort_by(|a, b| x::natural_cmp(a, b));
    assert_eq!(names, ["a1", "a01", "a2", "a10", "é2", "é10", "界2", "界10"]);
    assert_eq!(
        x::unique_names("Name", &|n| ["Name", "Name_1", "Name_3"].contains(&n), 3, false),
        ["Name_2", "Name_4", "Name_5"]
    );
}

#[test]
fn independently_enumerated_columnate_formulas() {
    for mode in [x::LoopMode::None, x::LoopMode::Continuous, x::LoopMode::ForwardBackward] {
        for n in 1usize..=8 {
            for each in 1usize..=5 {
                for last in 1usize..=5 {
                    for loops in 1usize..=8 {
                        let k = if mode == x::LoopMode::None { 1 } else { loops };
                        let width = if mode == x::LoopMode::ForwardBackward && n > 1 { 2 * n - 2 } else { n };
                        let mut expected = vec![];
                        for visit in 0..width * k {
                            let p = visit % width;
                            let drawing = if p < n { p } else { 2 * n - 2 - p };
                            let hold = if visit + 1 == width * k { last } else { each };
                            expected.extend(std::iter::repeat_n(Some(drawing), hold));
                        }
                        let c = x::columnate(&(0..n).collect::<Vec<_>>(), mode, each, last, loops, None);
                        assert_eq!(x::sequence_frames(&c.sequence), expected);
                        assert_eq!(c.total_frames, (width * k - 1) * each + last);
                    }
                }
            }
        }
    }
}

/// C31-R1-01: requested total must be filled even when exposeLast is shorter than exposeEach.
#[test]
fn columnate_total_frames_short_last_reaches_requested_length() {
    let c = x::columnate(&[1, 2, 3], x::LoopMode::Continuous, 4, 1, 1, Some(12));
    assert_eq!(c.total_frames, 12, "04 §13.10: truncate the last loop to the requested frame count; {c:?}");
}
