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
    assert_eq!(
        f.pressure.iter().map(|k| k[1]).fold(0.0, f64::max),
        1.0,
        "02 3.3/3.4: a stationary pressure maximum must reach the brush: {:?}",
        f.pressure
    );
}

#[test]
fn closing_near_duplicate_pressure_stays_at_the_end_of_the_loop() {
    let raw = [s(0.0, 0.0, 1.0), s(100.0, 0.0, 1.0), s(100.0, 100.0, 1.0), s(0.0, 100.0, 1.0), s(0.001, 0.0, 0.0)];
    let f = fit::fit(&raw, 0.0, true);
    eprintln!("closed seam pressure {:?}", f.pressure);
    assert!(
        fit::pressure_at(&f.pressure, 0.125) > 0.999,
        "last sample's low pressure migrated to the first edge: {:?}",
        f.pressure
    );
}

#[test]
fn merged_pressure_moving_average_matches_input_window_at_every_smoothness() {
    for smooth in 0..=3 {
        for closed in [false, true] {
            for at in [0, 1, 31, 63, 64, 65, 128] {
                let raw: Vec<_> =
                    (0..129).map(|i| s(f64::from(i) * 0.000001, 0.0, if i == at { 1.0 } else { 0.0 })).collect();
                let f = fit::fit(&raw, f64::from(smooth), closed);
                let half = smooth as usize;
                let expected = (0usize..129)
                    .map(|i| {
                        let lo = i.saturating_sub(half);
                        let hi = (i + half).min(128);
                        if (lo..=hi).contains(&(at as usize)) { 1.0 / (hi - lo + 1) as f64 } else { 0.0 }
                    })
                    .fold(0.0, f64::max);
                let actual = f.pressure.iter().map(|k| k[1]).fold(0.0, f64::max);
                assert!(
                    (actual - expected).abs() < 0.0011,
                    "smooth={smooth} closed={closed} at={at}: {actual} / {expected}"
                );
            }
        }
    }
}

#[test]
fn nfc_reference_and_origin_coverage_corpus() {
    for input in [
        "\u{995}\u{9c7}\u{9be}",
        "\u{915}\u{93c}",
        "\u{ac00}\u{11a8}",
        "a\u{301}\u{323}\u{327}",
        "\u{5e9}\u{5c1}\u{5b8}",
        "\u{1f469}\u{200d}\u{1f3a8}\u{fe0f}",
        "\u{fb1d}",
        "\u{344}",
    ] {
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

#[test]
fn canonical_reordering_preserves_each_marks_own_format() {
    use tt_vector::text::{Content, default_para, default_run};
    let mut runs = Vec::new();
    for i in 0..3 {
        let mut r = default_run(None);
        r.start = i;
        r.length = 1;
        r.size = 10.0 * f64::from(i + 1);
        runs.push(r);
    }
    // SHIN, SHIN DOT (ccc24, size20), QAMATS (ccc18, size30).
    // NFC reorders the two marks, without composing either.
    let c = Content::from_parts("\u{5e9}\u{5c1}\u{5b8}", runs, vec![default_para()]);
    assert_eq!(c.text(), "\u{5e9}\u{5b8}\u{5c1}");
    let sizes: Vec<f64> = c.runs.iter().flat_map(|r| std::iter::repeat_n(r.size, r.length as usize)).collect();
    assert_eq!(sizes, vec![10.0, 30.0, 20.0], "reordered marks must keep their source runs, not the maximum run index");
}

#[test]
#[ignore = "bounded subprocess worker"]
fn maximum_spine_and_pressure_worker() {
    use tt_model::entities::pen::Tip;
    use tt_vector::{
        brush::{BrushPen, outline_measured},
        path::{Anchor, Path},
    };
    let spine =
        Path { anchors: (0..99_998).map(|i| Anchor::corner(P::new(f64::from(i), 0.0))).collect(), closed: false };
    let pressure: Vec<_> = (0..100_000).map(|i| [f64::from(i) / 99_999.0, f64::from(i % 2)]).collect();
    let started = std::time::Instant::now();
    let result = outline_measured(
        &spine,
        &pressure,
        &BrushPen { tip: Tip::Round, min_size: 0.0, max_size: 100.0, tip_angle: 0.0 },
    );
    eprintln!(
        "maximum spine + pressure complete outline {:?}: {:?}",
        started.elapsed(),
        result.map(|(p, w)| (p.anchors.len(), w))
    );
}

#[test]
fn maximum_spine_and_pressure_return_within_watchdog() {
    use std::time::{Duration, Instant};
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "maximum_spine_and_pressure_worker", "--ignored", "--nocapture"])
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            assert!(
                started.elapsed() <= Duration::from_secs(30),
                "placement pressure lookups must return/refuse within the complete work watchdog"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "release-only bridge and symmetry work observations"]
fn bridge_threshold_closed_spine_and_symmetry_work_variants() {
    use tt_model::entities::pen::Tip;
    use tt_vector::{
        brush::{BrushPen, outline_measured},
        path::{Anchor, Path},
        symmetry::{Symmetry, maps},
    };
    let pen = BrushPen { tip: Tip::Round, min_size: 0.0, max_size: 1.0, tip_angle: 0.0 };
    for loops in [500, 800, 1000, 1200] {
        let path = Path {
            anchors: (0..loops * 24)
                .map(|i| {
                    let a = std::f64::consts::TAU * f64::from(i) / 24.0;
                    Anchor::corner(P::new(f64::from(i) / 24.0 * 4.0 + 5.0 * a.cos(), 5.0 * a.sin()))
                })
                .collect(),
            closed: false,
        };
        let started = std::time::Instant::now();
        let result = outline_measured(&path, &[[0.0, 1.0], [1.0, 1.0]], &pen);
        eprintln!("coil {loops}: {:?}, {:?}", started.elapsed(), result.as_ref().map(|(p, w)| (p.anchors.len(), w)));
        if let Err(e) = result {
            assert_eq!(e.code, "limit.exceeded");
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
    }
    let path = Path {
        anchors: vec![
            Anchor::corner(P::new(0.0, 0.0)),
            Anchor::corner(P::new(100.0, 0.0)),
            Anchor::corner(P::new(50.0, 100.0)),
        ],
        closed: true,
    };
    let pressure: Vec<_> = (0..99_000).map(|i| [f64::from(i) / 98_999.0, f64::from(i % 2)]).collect();
    let wide = BrushPen { max_size: 100.0, ..pen };
    let started = std::time::Instant::now();
    let result = outline_measured(&path, &pressure, &wide);
    eprintln!(
        "closed 99000 step knots: {:?}, {:?}",
        started.elapsed(),
        result.as_ref().map(|(p, w)| (p.anchors.len(), w))
    );
    if let Err(e) = result {
        assert_eq!(e.code, "limit.exceeded");
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    // This is library exposure for C2.3, not an enabled command setting (symmetry_setting returns None).
    let line =
        Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(100.0, 0.0))], closed: false };
    let started = std::time::Instant::now();
    let mut total = 0u64;
    for m in maps(Symmetry::Kaleidoscope { angle: 13.0, mirrors: 16 }) {
        let (_, units) = outline_measured(&line.transformed(&m), &pressure, &wide).unwrap();
        total += units;
    }
    eprintln!("32-image kaleidoscope library exposure: {:?}, summed work={total}", started.elapsed());
}
