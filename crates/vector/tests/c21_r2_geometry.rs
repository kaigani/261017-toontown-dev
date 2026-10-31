//! Independent dense two-direction fitting oracle (02 section 3.3).
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
use tt_vector::fit::{FIT_WORK_BUDGET, Sample, clean, fit_bounded, tolerance};
use tt_vector::geom::P;

fn distance(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let t = if ab.dot(ab) == 0.0 { 0.0 } else { ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0) };
    p.dist(a + ab * t)
}
fn to_poly(p: P, poly: &[P]) -> f64 {
    if poly.len() == 1 {
        return p.dist(poly[0]);
    }
    poly.windows(2).map(|w| distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}
fn next(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / ((1u64 << 53) as f64)
}

#[test]
fn dense_bidirectional_fitting_corpus() {
    let mut corpus: Vec<(String, Vec<P>)> = vec![
        ("two".into(), vec![P::new(0.0, 0.0), P::new(100.0, 20.0)]),
        ("three".into(), vec![P::new(0.0, 0.0), P::new(100.0, 50.0), P::new(200.0, 0.0)]),
        ("four-cusp".into(), vec![P::new(0.0, 0.0), P::new(10.0, 0.0), P::new(0.0, 0.0), P::new(-10.0, 0.0)]),
        ("straight".into(), (0..40).map(|i| P::new(f64::from(i), 0.0)).collect()),
        ("circle".into(), (0..40).map(|i| tt_vector::num::dir(f64::from(i) * 9.0) * 20.0).collect()),
        ("zigzag".into(), (0..40).map(|i| P::new(f64::from(i), f64::from(i % 2) * 3.0)).collect()),
        (
            "s-curve".into(),
            (0..40).map(|i| P::new(f64::from(i), tt_vector::num::sin(f64::from(i) * 0.2) * 10.0)).collect(),
        ),
        ("outlier".into(), (0..40).map(|i| P::new(f64::from(i), if i == 20 { 5.0 } else { 0.001 })).collect()),
    ];
    for seed in 0..160u64 {
        let mut state = seed;
        let scale = [0.1, 1.0, 10.0, 100.0][(seed % 4) as usize];
        let mut p = P::default();
        let raw = (0..(3 + seed % 18))
            .map(|_| {
                p = p + P::new(next(&mut state) - 0.5, next(&mut state) - 0.5) * scale;
                p
            })
            .collect();
        corpus.push((format!("walk-{seed}"), raw));
    }
    let mut failures = Vec::new();
    let mut worst_ratio = 0.0f64;
    for (name, points) in corpus {
        for smooth in [0.0, 1.0, 2.0, 3.0] {
            for closed in [false, true] {
                let raw: Vec<_> = points.iter().map(|p| Sample { p: *p, pressure: 1.0 }).collect();
                let samples = clean(&raw, "/points").unwrap();
                let Ok((f, _)) = fit_bounded(&samples, smooth, closed, FIT_WORK_BUDGET) else { continue };
                let mut input = points.clone();
                if closed && input.len() >= 3 {
                    input.push(input[0]);
                }
                let mut output = Vec::new();
                let mut forward = 0.0f64;
                for s in f.path.segments() {
                    for k in 0..=512 {
                        let p = s.eval(f64::from(k) / 512.0);
                        forward = forward.max(to_poly(p, &input));
                        output.push(p);
                    }
                }
                if output.is_empty() {
                    output.push(f.path.anchors[0].p);
                    forward = to_poly(output[0], &input);
                }
                let mut reverse = 0.0f64;
                for w in input.windows(2) {
                    for k in 0..=64 {
                        reverse = reverse.max(to_poly(w[0].lerp(w[1], f64::from(k) / 64.0), &output));
                    }
                }
                let error = forward.max(reverse);
                let ratio = error / tolerance(smooth);
                if ratio > worst_ratio {
                    worst_ratio = ratio;
                    eprintln!(
                        "worst {name} smooth={smooth} closed={closed} forward={forward:.9} reverse={reverse:.9} anchors={} ratio={ratio:.9}",
                        f.path.anchors.len()
                    );
                }
                if error > tolerance(smooth) + 0.001 {
                    failures.push(format!("{name} smooth={smooth} closed={closed}: curve->poly={forward}, poly->curve={reverse}, anchors={}",f.path.anchors.len()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "02 3.3 tolerance failures: {failures:#?}");
}

#[test]
fn nfc_composition_preserves_unaffected_character_formats() {
    use tt_vector::text::{Content, default_para, default_run};
    let mut a = default_run(None);
    a.length = 1;
    let mut b = a.clone();
    b.start = 1;
    b.length = 2;
    b.bold = true;
    // Bengali spacing vowel signs have CCC=0 yet compose across that boundary.
    // The following ASCII Z is unrelated to the composition and must stay bold.
    let c = Content::from_parts("\u{09c7}\u{09be}Z", vec![a, b], vec![default_para()]);
    assert_eq!(c.text(), "\u{09cb}Z");
    eprintln!("Bengali NFC runs: {:?}", c.runs);
    assert!(
        c.runs.iter().any(|r| r.start <= 1 && r.start + r.length > 1 && r.bold),
        "NFC composition must preserve Z's format"
    );
}

#[test]
#[ignore = "subprocess worker run by the bounded parent"]
fn alternating_pressure_worker() {
    let samples: Vec<_> =
        (0..100_000).map(|i| Sample { p: P::new(f64::from(i), 0.0), pressure: f64::from(i % 2) }).collect();
    let start = std::time::Instant::now();
    let (f, visits) = fit_bounded(&samples, 0.0, false, FIT_WORK_BUDGET).unwrap();
    eprintln!("alternating pressure: {:?}, charged visits={visits}, knots={}", start.elapsed(), f.pressure.len());
}

#[test]
fn alternating_pressure_fitting_respects_watchdog() {
    use std::time::{Duration, Instant};
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "alternating_pressure_worker", "--ignored", "--nocapture"])
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            assert!(
                false,
                "100k collinear samples with alternating pressure exceed the 30s watchdog; pressure simplification must be budgeted"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn spatial_dedupe_preserves_pressure_extrema_for_the_brush_sweep() {
    use tt_model::entities::pen::Tip;
    use tt_vector::brush::{BrushPen, outline};
    let raw = [Sample { p: P::new(0.0, 0.0), pressure: 1.0 }, Sample { p: P::new(0.001, 0.0), pressure: 0.0 }];
    let (f, _) = fit_bounded(&clean(&raw, "/points").unwrap(), 0.0, false, FIT_WORK_BUDGET).unwrap();
    let pen = BrushPen { tip: Tip::Round, min_size: 0.0, max_size: 100.0, tip_angle: 0.0 };
    let out = outline(&f.path, &f.pressure, &pen).unwrap();
    let bounds = out.bounds().unwrap();
    eprintln!("dense pressure peak: fitted pressure={:?}, bounds={bounds:?}", f.pressure);
    assert!(bounds.max_y >= 49.8, "02 3.3 step 5 / 3.4: smoothness zero must retain the radius-50 starting tip");
}

#[test]
#[ignore = "subprocess worker run by the bounded parent"]
fn many_pressure_knots_brush_worker() {
    use tt_model::entities::pen::Tip;
    use tt_vector::{
        brush::{BrushPen, outline},
        path::{Anchor, Path},
    };
    let spine =
        Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(100.0, 0.0))], closed: false };
    let pressure: Vec<_> = (0..99_000).map(|i| [f64::from(i) / 98_999.0, f64::from(i % 2)]).collect();
    let start = std::time::Instant::now();
    let result =
        outline(&spine, &pressure, &BrushPen { tip: Tip::Round, min_size: 0.0, max_size: 100.0, tip_angle: 0.0 });
    eprintln!("99000-knot brush: {:?}, result anchors/error={:?}", start.elapsed(), result.map(|p| p.anchors.len()));
}

#[test]
fn many_pressure_knots_brush_respects_watchdog() {
    use std::time::{Duration, Instant};
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "many_pressure_knots_brush_worker", "--ignored", "--nocapture"])
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            assert!(false, "accepted 99000 pressure knots hold the brush worker beyond the 30s document watchdog");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
