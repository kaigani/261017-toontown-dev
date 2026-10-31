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
            // 02 §3.3 against the input polyline, both directions (C21-R1-01): every curve point within the tolerance of
            // the polyline, and every polyline point within the tolerance of the curve.
            let tol = tolerance(smooth);
            let input: Vec<P> = cleaned.iter().map(|s| s.p).collect();
            let near_input = |q: P| -> f64 {
                if input.len() == 1 {
                    return q.dist(input[0]);
                }
                input.windows(2).map(|w| seg_dist(q, w[0], w[1])).fold(f64::INFINITY, f64::min)
            };
            for seg in a.path.segments() {
                for k in 0..=64 {
                    let d = near_input(seg.eval(f64::from(k) / 64.0));
                    prop_assert!(
                        d <= tol + 1e-9,
                        "smoothness {}: curve point {} du from the polyline (tolerance {})",
                        smooth,
                        d,
                        tol
                    );
                }
            }
            let curve = a.path.flatten(0.001);
            for w in input.windows(2) {
                for k in 0..=8 {
                    let q = w[0].lerp(w[1], f64::from(k) / 8.0);
                    let d = if curve.len() == 1 {
                        q.dist(curve[0])
                    } else {
                        curve.windows(2).map(|c| seg_dist(q, c[0], c[1])).fold(f64::INFINITY, f64::min)
                    };
                    prop_assert!(
                        d <= tol + 0.001 + 1e-9,
                        "smoothness {}: polyline point {} du from the curve (tolerance {})",
                        smooth,
                        d,
                        tol
                    );
                }
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

fn winding_contains(poly: &[P], p: P) -> bool {
    let mut winding = 0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if a.y <= p.y && b.y > p.y && (b - a).cross(p - a) > 0.0 {
            winding += 1;
        }
        if a.y > p.y && b.y <= p.y && (b - a).cross(p - a) < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

/// 02 §3.4 (C21-R1-02, SI-053 corpus): for random spines (loops and self-crossings), pressure maps (including zero
/// pressure that splits the sweep into separate pieces), every tip shape and angle, every point well inside a swept
/// tip at sampled arc-length positions and at every pressure knot lies inside the outline (non-zero winding, the
/// bridged single contour of SI-053), and every outline vertex lies within the outline tolerance of some tip.
#[test]
fn outlines_contain_every_swept_tip_including_holes_and_separate_pieces() {
    runner(80)
        .run(
            &(
                walk(),
                prop::collection::vec((0.0f64..=1.0, 0.0f64..=1.0), 1..6),
                1.0f64..20.0,
                0usize..4,
                0.0f64..360.0,
                0.0f64..0.5,
            ),
            |(raw, knots, max, tip, angle, minf)| {
                let pen = BrushPen {
                    tip: [Tip::Round, Tip::Oval, Tip::Square, Tip::Flat][tip],
                    min_size: max * minf,
                    max_size: max,
                    tip_angle: angle,
                };
                let f = fit(&clean(&raw, "/p").unwrap(), 2.0, false);
                let mut map: Vec<[f64; 2]> = knots.into_iter().map(|(t, p)| [t, p]).collect();
                map.push([0.0, 1.0]);
                map.push([1.0, 0.6]);
                map.sort_by(|a, b| a[0].total_cmp(&b[0]));
                let Ok(o) = outline(&f.path, &map, &pen) else {
                    // Zero width throughout is refused, never a fake outline.
                    prop_assert!(
                        pen.max_size * map.iter().map(|m| m[1]).fold(0.0, f64::max) <= pen.min_size.max(1e-9)
                            || pen.min_size == 0.0
                    );
                    return Ok(());
                };
                let poly = o.flatten(0.002);
                let segs = f.path.segments();
                let lens: Vec<f64> = segs.iter().map(tt_vector::geom::Cubic::length).collect();
                let total: f64 = lens.iter().sum();
                let point_at = |t: f64| -> P {
                    if segs.is_empty() || total <= 0.0 {
                        return f.path.anchors[0].p;
                    }
                    let mut target = t * total;
                    for (s, l) in segs.iter().zip(&lens) {
                        if target <= *l {
                            // Bisection on arc length.
                            let (mut lo, mut hi) = (0.0, 1.0);
                            for _ in 0..40 {
                                let mid = (lo + hi) / 2.0;
                                if s.length_to(mid) < target { lo = mid } else { hi = mid }
                            }
                            return s.eval((lo + hi) / 2.0);
                        }
                        target -= l;
                    }
                    segs[segs.len() - 1].p3
                };
                let mut ts: Vec<f64> = (0..=40).map(|k| f64::from(k) / 40.0).collect();
                ts.extend(map.iter().map(|m| m[0]));
                // Probe points: the tip's centre region shrunk by the outline tolerance plus a grid step.
                let margin = tt_vector::brush::OUTLINE_TOLERANCE.min(max / 16.0) + 2.0 / tt_vector::brush::GRID;
                for t in ts {
                    let w = pen.width(tt_vector::fit::pressure_at(&map, t));
                    let c = point_at(t);
                    let tip = tt_vector::brush::tip_polygon(&pen, w);
                    // Inradius of the tip: probes are the tip scaled about its centre so every probe is at least
                    // `margin` inside every edge; tips thinner than twice the margin are skipped.
                    let inradius = match pen.tip {
                        Tip::Round | Tip::Square => w / 2.0,
                        Tip::Oval => w / 4.0,
                        Tip::Flat => w / 10.0,
                    };
                    if inradius <= 2.0 * margin {
                        continue;
                    }
                    let k = 1.0 - margin / inradius;
                    for v in tip.iter().chain(std::iter::once(&P::default())) {
                        let q = c + *v * k;
                        prop_assert!(winding_contains(&poly, q), "tip at t={} (width {}) not covered at {:?}", t, w, q);
                    }
                }
                Ok(())
            },
        )
        .unwrap();
}
