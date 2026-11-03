//! Round 2 independent variants: SI-095 lengths, reverse units, trimmed insertion and suffix collisions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
use tt_anim::exposure as x;

#[test]
fn r01_total_frames_boundary_grid_reverse_and_single_drawing() {
    for n in 1..=7 {
        let d: Vec<usize> = (1..=n).rev().collect();
        for mode in [x::LoopMode::Continuous, x::LoopMode::ForwardBackward] {
            let mut unit = d.clone();
            if mode == x::LoopMode::ForwardBackward && n > 2 {
                unit.extend(d[1..n-1].iter().rev());
            }
            for each in [1, 2, 5, 9] {
                for last in [1, 3, 8, 13] {
                    for k in 1..=5 {
                        let boundary = unit.len()*each*k;
                        for t in [boundary.saturating_sub(1).max(1), boundary, boundary+1] {
                            let r = x::columnate(&d, mode, each, last, 99, Some(t));
                            assert_eq!(r.total_frames, t, "n={n} mode={mode:?} e={each} l={last} t={t}");
                            assert_eq!(r.loops, t.div_ceil(unit.len()*each));
                            let f = x::sequence_frames(&r.sequence);
                            assert_eq!(f.len(), t);
                            // Before the potentially changed final hold, the independently built unit repeats.
                            let safe = ((r.loops*unit.len()).saturating_sub(1)*each).min(t);
                            for i in 0..safe { assert_eq!(f[i], Some(unit[(i/each)%unit.len()])); }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn r11_insert_beyond_arbitrary_blank_tail_always_trims() {
    for prefix in 0..31 {
        for tail in 1..17 {
            let mut f = vec![Some(73u8); prefix];
            f.extend(vec![None; tail]);
            let actual = x::insert(&f, prefix+tail+11, &[None, None, None]);
            assert_eq!(actual, vec![Some(73u8); prefix]);
        }
    }
}

#[test]
fn r08_unicode_suffix_crosses_one_to_two_digits() {
    let base = "界".repeat(64);
    let mut taken = Vec::new();
    for i in 1..=9 { taken.push(format!("{}_{}", "界".repeat(62), i)); }
    taken.push(format!("{}_10", "界".repeat(61)));
    let out = x::unique_names(&base, &|n| taken.iter().any(|s| s == n), 3, true);
    assert_eq!(out, (11..=13).map(|n| format!("{}_{}", "界".repeat(61), n)).collect::<Vec<_>>());
    assert!(out.iter().all(|n| n.chars().count() == 64));
}
