//! M3 C3.1: the exposure algebra of 04 §13 — the section's worked examples, edge cases (13.21) and seeded properties.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use proptest::prelude::*;
use tt_anim::exposure::{self as x, Continues, FillError, LoopMode};

/// Frames from a label string: `1 1 2 . 3` (`.` = blank).
fn f(s: &str) -> Vec<Option<String>> {
    s.split_whitespace().map(|t| (t != ".").then(|| t.to_owned())).collect()
}

fn s(frames: &[Option<String>]) -> String {
    frames.iter().map(|c| c.clone().unwrap_or_else(|| ".".to_owned())).collect::<Vec<_>>().join(" ")
}

#[test]
fn runs_round_trip_and_canonical_form() {
    let frames = f("1 1 . . 2 2 2 1 . .");
    let runs = x::compress(&frames);
    assert_eq!(runs.len(), 4, "trailing blanks dropped: {runs:?}");
    assert_eq!(s(&x::expand(&runs)), "1 1 . . 2 2 2 1");
    assert_eq!(x::canonical(&[(Some(1), 2), (Some(1), 3), (None, 0), (None, 4)]), vec![(Some(1), 5)]);
    assert_eq!(x::last_exposed(&frames), 8);
}

#[test]
fn set_exposure_on_ones_example_13_3() {
    // `1 1 2 2 3 3` set to 1 → `1 2 3` (the rest moves up 3 frames).
    let out = x::set_exposure(&f("1 1 2 2 3 3 4"), 1, 6, 1);
    assert_eq!(s(&out), "1 2 3 4");
    // Blank runs inside the selection keep their length (04 §18 #11).
    assert_eq!(s(&x::set_exposure(&f("1 . . 2"), 1, 4, 3)), "1 1 1 . . 2 2 2");
    // Only exposures intersecting the range change.
    assert_eq!(s(&x::set_exposure(&f("1 1 2 2 3 3"), 3, 3, 4)), "1 1 2 2 2 2 3 3");
}

#[test]
fn add_and_remove_exposure_13_1_13_2() {
    assert_eq!(s(&x::add_exposure(&f("1 2 2 3"), 2, 2, 1)), "1 2 2 2 3");
    // Multi-cell: one frame added to each distinct exposure that intersects the selection.
    assert_eq!(s(&x::add_exposure(&f("1 2 2 3"), 1, 4, 1)), "1 1 2 2 2 3 3");
    // On a blank cell: inserts one blank frame.
    assert_eq!(s(&x::add_exposure(&f("1 . 2"), 2, 2, 1)), "1 . . 2");
    // Past the end: nothing.
    assert_eq!(s(&x::add_exposure(&f("1"), 5, 5, 1)), "1");
    assert_eq!(s(&x::remove_exposure(&f("1 2 2 3"), 2, 2, 1)), "1 2 3");
    // A one-frame exposure is removed (the drawing stays in the bank).
    assert_eq!(s(&x::remove_exposure(&f("1 2 3"), 2, 2, 1)), "1 3");
    assert_eq!(s(&x::remove_exposure(&f("1 2 2 2 3"), 2, 2, 9)), "1 3");
}

#[test]
fn fill_overwrites_without_shift_13_5() {
    assert_eq!(s(&x::fill(&f("1 1 2 3 4"), 1, 4).unwrap()), "1 1 1 1 4");
    assert_eq!(s(&x::fill(&f("1"), 1, 4).unwrap()), "1 1 1 1");
    assert_eq!(x::fill(&f("1 . 2"), 2, 3), Err(FillError::Blank));
    assert_eq!(x::fill(&f("1 1 2"), 3, 2), Err(FillError::BeforeSelection { minimum: 3 }));
    // A target inside the current exposure changes nothing.
    assert_eq!(s(&x::fill(&f("1 1 1 2"), 1, 2).unwrap()), "1 1 1 2");
}

#[test]
fn blanks_insert_delete_13_6_to_13_8() {
    assert_eq!(s(&x::insert_blank(&f("1 2 3"), 2, 2)), "1 . . 2 3");
    assert_eq!(x::remove_blank(&f("1 2 3"), 2, 1), None, "not blank");
    assert_eq!(s(&x::remove_blank(&f("1 . . 2"), 2, 2).unwrap()), "1 2");
    assert_eq!(s(&x::remove_blank(&f("1"), 4, 3).unwrap()), "1", "past the end is blank");
    assert_eq!(s(&x::insert(&f("1 2"), 2, &f("5 6"))), "1 5 6 2");
    assert_eq!(s(&x::insert(&f("1"), 4, &f("5"))), "1 . . 5");
    assert_eq!(s(&x::delete_range(&f("1 2 3 4"), 2, 3, true)), "1 4");
    assert_eq!(s(&x::delete_range(&f("1 2 3 4"), 2, 3, false)), "1 . . 4");
    assert_eq!(s(&x::delete_range(&f("1 2 3 4"), 3, 9, false)), "1 2");
}

#[test]
fn cycle_example_13_9() {
    let out = x::cycle(&f("1 1 2 2 3 3 9"), 1, 6, 3);
    assert_eq!(s(&out), "1 1 2 2 3 3 1 1 2 2 3 3 1 1 2 2 3 3 9");
    // A selection ending in blank cells keeps them in the pattern (13.21).
    assert_eq!(s(&x::cycle(&f("1 ."), 1, 2, 3)), "1 . 1 . 1");
    assert_eq!(s(&x::cycle(&f("1 2"), 1, 2, 1)), "1 2", "1 = no change");
}

#[test]
fn columnate_formulas_13_10() {
    let d = ["1", "2", "3"];
    let none = x::columnate(&d, LoopMode::None, 2, 4, 5, None);
    assert_eq!(none.total_frames, 2 * 2 + 4, "(n - 1)e + l");
    assert_eq!(none.loops, 1);
    let cont = x::columnate(&d, LoopMode::Continuous, 1, 1, 2, None);
    assert_eq!(cont.sequence.iter().map(|(d, _)| *d).collect::<Vec<_>>(), ["1", "2", "3", "1", "2", "3"]);
    assert_eq!(cont.total_frames, 2 * 3 - 1 + 1, "(kn - 1)e + l");
    // 2 loops of 1-2-3 forward-backward → 1-2-3-2-1-2-3-2.
    let fb = x::columnate(&d, LoopMode::ForwardBackward, 1, 1, 2, None);
    assert_eq!(fb.sequence.iter().map(|(d, _)| *d).collect::<Vec<_>>(), ["1", "2", "3", "2", "1", "2", "3", "2"]);
    assert_eq!(fb.total_frames, 2 * (2 * 3 - 2), "(k(2n - 2) - 1)e + l");
    // first = last: single-drawing cycle; forward-backward behaves as continuous.
    let one = x::columnate(&["7"], LoopMode::ForwardBackward, 2, 3, 3, None);
    assert_eq!(one.sequence, vec![("7", 2), ("7", 2), ("7", 3)]);
    // Editing total frames: loops = ceil(frames / frames-per-loop), last loop truncated.
    let t = x::columnate(&d, LoopMode::Continuous, 2, 2, 1, Some(10));
    assert_eq!(t.loops, 2);
    assert_eq!(t.total_frames, 10);
    let frames: Vec<Option<String>> =
        x::sequence_frames(&t.sequence).into_iter().map(|c| c.map(str::to_owned)).collect();
    assert_eq!(s(&frames), "1 1 2 2 3 3 1 1 2 2");
}

#[test]
fn window_reads_flag_continuation_17_2() {
    let w = x::window(&f("1 1 1 2 2 . . 3 3 3"), 2, 9);
    assert_eq!(w.len(), 3);
    assert_eq!((w[0].from, w[0].to, w[0].continues), (2, 3, Some(Continues::Before)));
    assert_eq!((w[1].from, w[1].to, w[1].continues), (4, 5, None));
    assert_eq!((w[2].from, w[2].to, w[2].continues), (8, 9, Some(Continues::After)));
    assert_eq!(x::window(&f("1 1 1"), 2, 2)[0].continues, Some(Continues::Both));
}

#[test]
fn names_natural_order_and_defaults_14_2() {
    let mut names = vec!["10", "2", "A", "1", "01", "b2", "b10"];
    names.sort_by(|a, b| x::natural_cmp(a, b));
    assert_eq!(names, ["1", "01", "2", "10", "A", "b2", "b10"]);
    assert_eq!(x::next_drawing_name(["1", "3", "A7"]), "4", "next after the highest numeric name, not a gap");
    assert_eq!(x::next_drawing_name(["A"]), "1");
    assert_eq!(x::next_drawing_name(["0099"]), "100");
    assert_eq!(x::next_drawing_name(["99999999999999999999999999"]), "100000000000000000000000000");
    assert_eq!(x::next_drawing_names(["5"], 3), ["6", "7", "8"]);
    let taken = |n: &str| ["Peg", "Peg_1", "Peg_3"].contains(&n);
    assert_eq!(x::unique_names("Peg", &taken, 3, false), ["Peg_2", "Peg_4", "Peg_5"]);
    assert_eq!(x::unique_names("Bob", &|_| false, 2, false), ["Bob", "Bob_1"]);
    assert_eq!(x::unique_names("Bob", &|_| false, 1, true), ["Bob_1"], "clones always take a suffix");
}

fn frames_strategy() -> impl Strategy<Value = Vec<Option<u8>>> {
    proptest::collection::vec(proptest::option::weighted(0.7, 0u8..4), 0..40)
}

fn cases() -> u32 {
    std::env::var("TT_C31_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(512)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: cases(),
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x0C31),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn compress_expand_is_canonical(frames in frames_strategy()) {
        let runs = x::compress(&frames);
        let mut trimmed = frames.clone();
        x::trim(&mut trimmed);
        prop_assert_eq!(x::expand(&runs), trimmed);
        prop_assert!(runs.windows(2).all(|w| w[0].0 != w[1].0));
        prop_assert!(runs.last().is_none_or(|r| r.0.is_some()));
        prop_assert_eq!(x::canonical(&runs), runs);
    }

    #[test]
    fn insert_then_remove_blank_is_identity(frames in frames_strategy(), at in 1usize..45, n in 1usize..5) {
        let mut trimmed = frames.clone();
        x::trim(&mut trimmed);
        let ins = x::insert_blank(&frames, at, n);
        prop_assert_eq!(x::remove_blank(&ins, at, n), Some(trimmed));
    }

    #[test]
    fn set_exposure_preserves_the_drawing_sequence(
        frames in frames_strategy(), a in 1usize..45, w in 0usize..10, len in 1usize..5,
    ) {
        let out = x::set_exposure(&frames, a, a + w, len);
        let seq = |v: &[Option<u8>]| x::runs(v).into_iter().map(|r| r.value).collect::<Vec<_>>();
        prop_assert_eq!(seq(&out), seq(&frames));
        let blanks = |v: &[Option<u8>]| {
            x::runs(v).into_iter().filter(|r| r.value.is_none()).map(|r| r.len).collect::<Vec<_>>()
        };
        prop_assert_eq!(blanks(&out), blanks(&frames));
    }

    #[test]
    fn fill_never_shifts_later_cells(frames in frames_strategy(), a in 1usize..45, extra in 0usize..10) {
        if let Ok(out) = x::fill(&frames, a, a + extra) {
            for fr in (a + extra + 1)..=frames.len() {
                prop_assert_eq!(x::at(&out, fr), x::at(&frames, fr));
            }
        }
    }

    #[test]
    fn cycle_inserts_k_minus_one_copies(frames in frames_strategy(), a in 1usize..30, w in 0usize..8, k in 1usize..5) {
        let out = x::cycle(&frames, a, a + w, k);
        let pattern = x::slice(&frames, a, a + w);
        for c in 0..k {
            prop_assert_eq!(x::slice(&out, a + c * (w + 1), a + w + c * (w + 1)), pattern.clone());
        }
        for fr in (a + w + 1)..=frames.len() {
            prop_assert_eq!(x::at(&out, fr + (k - 1) * (w + 1)), x::at(&frames, fr));
        }
    }

    #[test]
    fn add_then_remove_exposure_on_one_frame_is_identity(frames in frames_strategy(), a in 1usize..45) {
        let mut trimmed = frames.clone();
        x::trim(&mut trimmed);
        if a <= x::last_exposed(&frames) {
            let added = x::add_exposure(&frames, a, a, 2);
            prop_assert_eq!(x::remove_exposure(&added, a, a, 2), trimmed);
        }
    }

    #[test]
    fn diff_window_covers_every_change(a in frames_strategy(), b in frames_strategy()) {
        match x::diff(&a, &b) {
            None => {
                let (mut ta, mut tb) = (a.clone(), b.clone());
                x::trim(&mut ta);
                x::trim(&mut tb);
                prop_assert_eq!(ta, tb);
            }
            Some((from, to)) => {
                prop_assert!(x::at(&a, from) != x::at(&b, from) && x::at(&a, to) != x::at(&b, to));
                for fr in (1..from).chain(to + 1..=a.len().max(b.len())) {
                    prop_assert_eq!(x::at(&a, fr), x::at(&b, fr));
                }
            }
        }
    }
}
