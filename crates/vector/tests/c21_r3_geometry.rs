//! Independent final-round pressure, Unicode and complete-work variants.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_vector::fit::{self, Sample};
use tt_vector::geom::P;
use unicode_normalization::UnicodeNormalization;

fn s(x: f64, y: f64, pressure: f64) -> Sample {
    Sample { p: P::new(x, y), pressure }
}

#[test]
fn exact_stationary_samples_keep_pressure_extrema_after_command_cleaning() {
    let raw = [s(0.0, 0.0, 0.0), s(0.0, 0.0, 1.0), s(0.0, 0.0, 0.0)];
    let cleaned = fit::clean(&raw, "/points").unwrap();
    let f = fit::fit(&cleaned, 0.0, false);
    assert_eq!(f.pressure.iter().map(|k| k[1]).fold(0.0, f64::max), 1.0,
        "02 3.3/3.4: a stationary pressure maximum must reach the brush: {:?}", f.pressure);
}

#[test]
fn closing_near_duplicate_pressure_stays_at_the_end_of_the_loop() {
    let raw = [s(0.0, 0.0, 1.0), s(100.0, 0.0, 1.0), s(100.0, 100.0, 1.0),
        s(0.0, 100.0, 1.0), s(0.001, 0.0, 0.0)];
    let f = fit::fit(&raw, 0.0, true);
    eprintln!("closed seam pressure {:?}", f.pressure);
    assert!(fit::pressure_at(&f.pressure, 0.125) > 0.999,
        "last sample's low pressure migrated to the first edge: {:?}", f.pressure);
}

#[test]
fn merged_pressure_moving_average_matches_input_window_at_every_smoothness() {
    for smooth in 0..=3 {
        for closed in [false, true] {
            for at in [0, 1, 31, 63, 64, 65, 128] {
                let raw: Vec<_> = (0..129).map(|i| s(f64::from(i) * 0.000001, 0.0,
                    if i == at { 1.0 } else { 0.0 })).collect();
                let f = fit::fit(&raw, f64::from(smooth), closed);
                let half = smooth as usize;
                let expected = (0usize..129).map(|i| {
                    let lo = i.saturating_sub(half);
                    let hi = (i + half).min(128);
                    if (lo..=hi).contains(&(at as usize)) { 1.0 / (hi - lo + 1) as f64 } else { 0.0 }
                }).fold(0.0, f64::max);
                let actual = f.pressure.iter().map(|k| k[1]).fold(0.0, f64::max);
                assert!((actual - expected).abs() < 0.0011, "smooth={smooth} closed={closed} at={at}: {actual} / {expected}");
            }
        }
    }
}

#[test]
fn nfc_reference_and_origin_coverage_corpus() {
    for input in ["\u{995}\u{9c7}\u{9be}", "\u{915}\u{93c}", "\u{ac00}\u{11a8}",
        "a\u{301}\u{323}\u{327}", "\u{5e9}\u{5c1}\u{5b8}",
        "\u{1f469}\u{200d}\u{1f3a8}\u{fe0f}", "\u{fb1d}", "\u{344}"] {
        for separator in ["", "\n"] {
            let text = format!("L{input}{separator}Z");
            let chars: Vec<_> = text.chars().collect();
            let mapped = tt_vector::text::nfc_with_origins(&chars);
            assert_eq!(mapped.iter().map(|(c, _)| *c).collect::<String>(), text.nfc().collect::<String>());
            assert!(mapped.iter().all(|(_, origin)| *origin < chars.len()));
            assert_eq!(mapped.first().unwrap().1, 0);
            assert_eq!(mapped.last().unwrap().1, chars.len() - 1);
            // Every decomposed input scalar survives in the canonical decomposition of the output.
            let in_nfd: String = text.nfd().collect();
            let out_nfd: String = mapped.iter().map(|(c, _)| *c).collect::<String>().nfd().collect();
            assert_eq!(in_nfd, out_nfd);
        }
    }
}
