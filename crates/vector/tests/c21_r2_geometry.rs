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
                if !name.starts_with("walk-") {
                    eprintln!(
                        "case {name} smooth={smooth} closed={closed} forward={forward:.9} reverse={reverse:.9} anchors={}",
                        f.path.anchors.len()
                    );
                }
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
    a.length = 2;
    let mut b = a.clone();
    b.start = 2;
    b.length = 2;
    b.bold = true;
    // Bengali spacing vowel signs have CCC=0 yet compose across that boundary.
    // The following ASCII Z is unrelated to the composition and must stay bold.
    let c = Content::from_parts("\u{0995}\u{09c7}\u{09be}Z", vec![a, b], vec![default_para()]);
    assert_eq!(c.text(), "\u{0995}\u{09cb}Z");
    eprintln!("Bengali NFC runs: {:?}", c.runs);
    assert!(
        c.runs.iter().any(|r| r.start <= 2 && r.start + r.length > 2 && r.bold),
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
                start.elapsed() <= Duration::from_secs(30),
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
            assert!(
                start.elapsed() <= Duration::from_secs(30),
                "accepted 99000 pressure knots hold the brush worker beyond the 30s document watchdog"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn contains(poly: &[P], p: P) -> bool {
    let mut winding = 0;
    for w in poly.windows(2) {
        let a = w[0];
        let b = w[1];
        if a.y <= p.y && b.y > p.y && (b - a).cross(p - a) > 0.0 {
            winding += 1;
        }
        if a.y > p.y && b.y <= p.y && (b - a).cross(p - a) < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

#[test]
fn swept_tip_containment_matrix_at_and_between_knots() {
    use tt_model::entities::pen::Tip;
    use tt_vector::{
        brush::{BrushPen, outline},
        path::{Anchor, Path},
    };
    let points = [P::new(0.0, 0.0), P::new(20.0, 0.0), P::new(20.0, 0.0), P::new(20.0, 20.0), P::new(20.0, 0.0)];
    let spine = Path { anchors: points.into_iter().map(Anchor::corner).collect(), closed: false };
    for tip in [Tip::Round, Tip::Oval, Tip::Square, Tip::Flat] {
        for angle in [0.0, 45.0, 90.0] {
            for min in [0.0, 12.0] {
                let pen = BrushPen { tip, min_size: min, max_size: 12.0, tip_angle: angle };
                let pressure = [[0.0, 0.0], [0.2, 1.0], [0.35, 0.0], [0.35, 1.0], [0.8, 0.2], [1.0, 1.0]];
                let path = outline(&spine, &pressure, &pen).unwrap();
                assert!(path.closed);
                assert!(path.check("/outline").is_ok());
                assert!(Path::from_bezier_list(&path.to_bezier_list(), true, "/bezier").is_ok());
                let poly = path.flatten(0.001);
                let mut probes = pressure.to_vec();
                for w in pressure.windows(2) {
                    for k in 1..10 {
                        let u = f64::from(k) / 10.0;
                        probes.push([w[0][0] + (w[1][0] - w[0][0]) * u, w[0][1] + (w[1][1] - w[0][1]) * u]);
                    }
                }
                for [t, p] in probes {
                    let d = t * 60.0;
                    let center = if d <= 20.0 {
                        P::new(d, 0.0)
                    } else if d <= 40.0 {
                        P::new(20.0, d - 20.0)
                    } else {
                        P::new(20.0, 60.0 - d)
                    };
                    let r = (min + p * (12.0 - min)) / 2.0;
                    if r < 0.4 {
                        continue;
                    }
                    for k in 0..32 {
                        let u = tt_vector::num::dir(f64::from(k) * 360.0 / 32.0);
                        let q = match tip {
                            Tip::Round => u * (r - 0.3),
                            Tip::Oval => P::new(u.x * (r - 0.3), u.y * (r / 2.0 - 0.3).max(0.0)),
                            Tip::Square => P::new(u.x.signum() * (r - 0.3), u.y.signum() * (r - 0.3)),
                            Tip::Flat => P::new(u.x.signum() * (r - 0.3), u.y.signum() * (r / 5.0 - 0.3).max(0.0)),
                        };
                        let q = if tip == Tip::Round { q } else { tt_vector::num::rotation(angle).apply(q) };
                        assert!(
                            contains(&poly, center + q),
                            "tip={tip:?} angle={angle} min={min} t={t} pressure={p} probe={:?}",
                            center + q
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn open_and_closed_anchor_limit_boundaries() {
    use tt_vector::path::{Anchor, MAX_ANCHORS, Path};
    for closed in [false, true] {
        for count in [MAX_ANCHORS, MAX_ANCHORS + 1] {
            let path = Path {
                anchors: (0..count).map(|i| Anchor::corner(P::new(i as f64, (i % 2) as f64))).collect(),
                closed,
            };
            assert_eq!(path.check("/path").is_ok(), count == MAX_ANCHORS);
            assert_eq!(Path::from_bezier_list(&path.to_bezier_list(), closed, "/bezier").is_ok(), count == MAX_ANCHORS);
        }
    }
}

#[test]
fn hundred_thousand_collinear_samples_respect_polyline_tolerance() {
    let samples: Vec<_> = (0..100_000).map(|i| Sample { p: P::new(f64::from(i), 0.0), pressure: 1.0 }).collect();
    for smooth in [0.0, 1.0, 2.0, 3.0] {
        for closed in [false, true] {
            let start = std::time::Instant::now();
            let (f, visits) = fit_bounded(&samples, smooth, closed, FIT_WORK_BUDGET).unwrap();
            for s in f.path.segments() {
                for i in 0..=10000 {
                    let p = s.eval(f64::from(i) / 10000.0);
                    let e = distance(p, P::new(0.0, 0.0), P::new(99999.0, 0.0));
                    assert!(e <= tolerance(smooth) + 1e-6, "smooth={smooth} closed={closed} error={e}");
                }
            }
            // Continuous output joins both extrema, so its reverse distance from every input point is also zero.
            assert!(f.path.anchors.iter().any(|a| a.p.x.abs() < 1e-6));
            assert!(f.path.anchors.iter().any(|a| (a.p.x - 99999.0).abs() < 1e-6));
            eprintln!(
                "100k straight smooth={smooth} closed={closed} anchors={} visits={visits} elapsed={:?}",
                f.path.anchors.len(),
                start.elapsed()
            );
        }
    }
}

#[test]
fn round_loop_union_grid_error_is_within_outline_tolerance() {
    use tt_model::entities::pen::Tip;
    use tt_vector::{
        brush::{BrushPen, outline},
        path::{Anchor, Path},
    };
    let points = [P::new(0.0, 0.0), P::new(30.0, 0.0), P::new(30.0, 30.0), P::new(0.0, 30.0), P::new(0.0, 0.0)];
    let spine = Path { anchors: points[..4].iter().copied().map(Anchor::corner).collect(), closed: true };
    let out = outline(
        &spine,
        &[[0.0, 1.0], [1.0, 1.0]],
        &BrushPen { tip: Tip::Round, min_size: 4.0, max_size: 4.0, tip_angle: 0.0 },
    )
    .unwrap();
    let poly = out.flatten(0.001);
    let mut worst = 0.0f64;
    let mut mismatches = 0;
    // Independent exact capsule-union oracle on a 1/8-du grid, including the hole and bridge.
    for x in -24..=264 {
        for y in -24..=264 {
            let p = P::new(f64::from(x) / 8.0, f64::from(y) / 8.0);
            let d = to_poly(p, &points);
            if contains(&poly, p) != (d < 2.0) {
                mismatches += 1;
                worst = worst.max((d - 2.0).abs());
            }
        }
    }
    eprintln!(
        "round loop grid: {} points, {mismatches} boundary disagreements, worst boundary distance={worst:.9} du",
        289 * 289
    );
    assert!(worst <= 0.1 + 1.0 / 64.0 + 0.025, "union/fitting/grid error {worst}");
}
