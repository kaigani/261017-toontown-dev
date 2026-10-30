//! Property tests of the vector engine (M2 C2.1): fitting is deterministic, within tolerance and timing free;
//! outlines are closed, finite and cover the swept tips; Bézier lists round-trip; text edits keep runs covering the
//! text and one paragraph per line. Seeds are fixed (proptest's deterministic RNG with a recorded seed), so a
//! failure reproduces on every machine.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::fit::{Sample, clean, fit, pressure_at, tolerance};
use tt_vector::geom::P;
use tt_vector::path::Path;
use tt_vector::text::{Content, Format, default_para, default_run};

/// A runner with a fixed seed (recorded here; ADR-0004 rule 4).
fn runner(cases: u32) -> TestRunner {
    let seed: [u8; 32] = *b"toontown-m2-c21-vector-props-001";
    TestRunner::new_with_rng(
        Config { cases, failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
    )
}

fn samples() -> impl Strategy<Value = Vec<Sample>> {
    prop::collection::vec((-500.0f64..500.0, -500.0f64..500.0, 0.0f64..=1.0), 1..80)
        .prop_map(|v| v.into_iter().map(|(x, y, p)| Sample { p: P::new(x, y), pressure: p }).collect())
}

/// A smooth random walk (freehand-like input).
fn walk() -> impl Strategy<Value = Vec<Sample>> {
    (prop::collection::vec((-3.0f64..3.0, -3.0f64..3.0, 0.0f64..=1.0), 2..200), -100.0f64..100.0, -100.0f64..100.0)
        .prop_map(|(steps, x0, y0)| {
            let (mut x, mut y, mut dx, mut dy) = (x0, y0, 1.0, 0.0);
            steps
                .into_iter()
                .map(|(ax, ay, p)| {
                    dx = (dx + ax * 0.2).clamp(-3.0, 3.0);
                    dy = (dy + ay * 0.2).clamp(-3.0, 3.0);
                    x += dx;
                    y += dy;
                    Sample { p: P::new(x, y), pressure: p }
                })
                .collect()
        })
}

fn seg_dist(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let l = ab.dot(ab);
    let t = if l > 0.0 { ((p - a).dot(ab) / l).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

#[test]
fn fitting_is_deterministic_and_within_tolerance() {
    runner(200)
        .run(&(walk(), 0u8..=10), |(raw, s)| {
            let smooth = f64::from(s);
            let cleaned = clean(&raw, "/points").unwrap();
            let a = fit(&cleaned, smooth, false);
            let b = fit(&cleaned, smooth, false);
            prop_assert_eq!(&a, &b);
            prop_assert!(a.path.check("/p").is_ok());
            let poly = a.path.flatten(0.01);
            for s in &cleaned {
                let d = poly.windows(2).map(|w| seg_dist(s.p, w[0], w[1])).fold(f64::INFINITY, f64::min);
                let d = if poly.len() == 1 { s.p.dist(poly[0]) } else { d };
                // The fit honours the tolerance (plus the flattening error and a small margin for the corner joins).
                prop_assert!(
                    d <= tolerance(smooth) * 1.5 + 0.05,
                    "smoothness {}: {} > {}",
                    smooth,
                    d,
                    tolerance(smooth)
                );
            }
            let pr = &a.pressure;
            prop_assert_eq!(pr.first().unwrap()[0], 0.0);
            prop_assert_eq!(pr.last().unwrap()[0], 1.0);
            prop_assert!(pr.windows(2).all(|w| w[0][0] <= w[1][0]));
            prop_assert!(pr.iter().all(|[_, p]| (0.0..=1.0).contains(p)));
            Ok(())
        })
        .unwrap();
}

#[test]
fn fitting_ignores_timing_only_positions_and_pressures_matter() {
    // The same samples delivered with different "timing" (duplicated consecutive samples, as a slow pointer would
    // produce) fit identically: duplicates are dropped before fitting (02 §3.3 step 1).
    runner(100)
        .run(&walk(), |raw| {
            let mut slow = Vec::new();
            for s in &raw {
                slow.push(*s);
                slow.push(*s);
            }
            let a = fit(&clean(&raw, "/p").unwrap(), 3.0, false);
            let b = fit(&clean(&slow, "/p").unwrap(), 3.0, false);
            prop_assert_eq!(a, b);
            Ok(())
        })
        .unwrap();
}

#[test]
fn outlines_are_closed_finite_and_cover_every_tip() {
    runner(120)
        .run(&(walk(), 0.5f64..20.0, 0.1f64..1.0, 0usize..4, 0.0f64..360.0), |(raw, max, minf, tip, angle)| {
            let pen = BrushPen {
                tip: [Tip::Round, Tip::Oval, Tip::Square, Tip::Flat][tip],
                min_size: max * minf,
                max_size: max,
                tip_angle: angle,
            };
            let f = fit(&clean(&raw, "/p").unwrap(), 3.0, false);
            let o = outline(&f.path, &f.pressure, &pen).unwrap();
            prop_assert!(o.closed);
            prop_assert!(o.check("/o").is_ok());
            let ob = o.bounds().unwrap().grown(max / 16.0 + 0.2);
            // Every spine anchor lies inside the outline's bounds (the tip, at least 10 % of the maximum wide, is centred
            // on the spine; zero-width sections would legitimately sweep nothing).
            for a in &f.path.anchors {
                prop_assert!(a.p.x >= ob.min_x && a.p.x <= ob.max_x && a.p.y >= ob.min_y && a.p.y <= ob.max_y);
            }
            // The outline does not reach beyond the swept tips (bounds of the spine grown by half the widest tip).
            let sb = f.path.bounds().unwrap().grown(max * 0.75 + 0.5);
            let b = o.bounds().unwrap();
            prop_assert!(b.min_x >= sb.min_x && b.max_x <= sb.max_x && b.min_y >= sb.min_y && b.max_y <= sb.max_y);
            prop_assert_eq!(outline(&f.path, &f.pressure, &pen).unwrap(), o, "reproducible");
            Ok(())
        })
        .unwrap();
}

#[test]
fn bezier_lists_round_trip() {
    runner(200)
        .run(&(samples(), any::<bool>()), |(raw, closed)| {
            let pts: Vec<P> = raw.iter().map(|s| s.p).collect();
            let n = pts.len();
            let take = if closed {
                if n >= 3 { n - n % 3 } else { 1 }
            } else if n >= 4 {
                n - (n - 1) % 3
            } else {
                return Ok(());
            };
            let list = &pts[..take];
            // A one-segment closed loop whose handles collapse onto its anchor is the same path as a dab.
            if closed && take == 3 && list.iter().all(|p| *p == list[0]) {
                return Ok(());
            }
            let p = Path::from_bezier_list(list, closed, "/b").unwrap();
            // Handles are stored as offsets (10 §4.9), so a control point read back is `anchor + (point − anchor)`:
            // equal up to one rounding of the subtraction and the addition.
            let back_list = p.to_bezier_list();
            prop_assert_eq!(back_list.len(), list.len());
            for (a, b) in back_list.iter().zip(list) {
                let tol = 4.0 * f64::EPSILON * (1.0 + b.x.abs().max(b.y.abs()) * 2.0);
                prop_assert!((a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol, "{:?} vs {:?}", a, b);
            }
            let back = Path::from_points(&p.to_points(), p.closed);
            prop_assert_eq!(back, p);
            Ok(())
        })
        .unwrap();
}

#[derive(Debug, Clone)]
enum Op {
    Insert(usize, String),
    Delete(usize, usize),
    Bold(usize, usize, bool),
    Align(usize, usize),
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    let text = prop::collection::vec(prop::sample::select(vec!['a', 'b', ' ', '\n', 'é', '日']), 0..6)
        .prop_map(|v| v.into_iter().collect::<String>());
    let op = prop_oneof![
        (0usize..40, text).prop_map(|(a, t)| Op::Insert(a, t)),
        (0usize..40, 0usize..10).prop_map(|(a, l)| Op::Delete(a, l)),
        (0usize..40, 0usize..10, any::<bool>()).prop_map(|(a, l, b)| Op::Bold(a, l, b)),
        (0usize..40, 0usize..10).prop_map(|(a, l)| Op::Align(a, l)),
    ];
    prop::collection::vec(op, 1..30)
}

#[test]
fn text_edits_keep_runs_covering_and_one_paragraph_per_line() {
    runner(300)
        .run(&ops(), |ops| {
            let mut c = Content::new("Hello\nworld", default_run(None), default_para());
            for op in ops {
                let n = c.chars.len();
                match op {
                    Op::Insert(a, t) => c.insert(a.min(n), &t),
                    Op::Delete(a, l) => c.delete(a.min(n), l),
                    Op::Bold(a, l, b) => {
                        c.format(a.min(n), l.min(n - a.min(n)), &Format { bold: Some(b), ..Format::default() })
                    }
                    Op::Align(a, l) => c.format(
                        a.min(n),
                        l.min(n - a.min(n)),
                        &Format { align: Some("right".into()), ..Format::default() },
                    ),
                }
                let total: u32 = c.runs.iter().map(|r| r.length).sum();
                prop_assert_eq!(total as usize, c.chars.len());
                let mut pos = 0;
                for r in &c.runs {
                    prop_assert_eq!(r.start, pos);
                    pos += r.length;
                }
                prop_assert!(!c.runs.is_empty());
                let fmt = |r: &tt_vector::text::Run| tt_vector::text::Run { start: 0, length: 0, ..r.clone() };
                prop_assert!(c.runs.windows(2).all(|w| fmt(&w[0]) != fmt(&w[1])), "neighbouring runs differ in format");
                let lines = c.chars.iter().filter(|ch| **ch == '\n').count() + 1;
                prop_assert_eq!(c.paras.len(), lines);
                let covered: u32 = c.paras.iter().map(|p| p.length).sum();
                prop_assert_eq!(covered as usize, c.chars.len());
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn pressure_interpolation_stays_in_range() {
    runner(200)
        .run(&prop::collection::vec((0.0f64..=1.0, 0.0f64..=1.0), 1..20), |mut m| {
            m.sort_by(|a, b| a.0.total_cmp(&b.0));
            let map: Vec<[f64; 2]> = m.into_iter().map(|(t, p)| [t, p]).collect();
            for k in 0..=20 {
                let p = pressure_at(&map, f64::from(k) / 20.0);
                prop_assert!((0.0..=1.0).contains(&p));
            }
            Ok(())
        })
        .unwrap();
}
