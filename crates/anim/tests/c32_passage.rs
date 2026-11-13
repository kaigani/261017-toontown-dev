//! M3 C3.2 (builder; C32-R1-01): passage frames are the earliest crossing of the control point's arc length, to 1e-4
//! frame, also when the velocity curve rises past the arc and falls back within one frame. The oracle works in exact
//! rationals: each velocity sub-segment's value cubic, its derivative's roots bracketed by rational bisection, the
//! earliest monotone piece that reaches the arc, then rational bisection of the crossing. Time handles sit at thirds, so
//! frame = start + s · length exactly and the oracle needs no time solve. Nothing here calls the product to compute an
//! expected value.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use proptest::prelude::*;
use tt_anim::function::KeyView;
use tt_anim::tcb::{Path, Point, Velocity, round3};
use tt_model::entities::channel::CurveType;

type R = BigRational;

fn q(n: i64, d: i64) -> R {
    R::new(BigInt::from(n), BigInt::from(d))
}

fn f(x: &R) -> f64 {
    x.to_f64().unwrap()
}

/// A node of the velocity curve in one keyframe segment: frame, value `u`, right and left value handles. Time handles are
/// a third of the neighbouring sub-segment, so the time cubic is linear.
#[derive(Clone)]
struct Node {
    frame: i64,
    u: R,
    left_dv: R,
    right_dv: R,
}

/// y(s) of the sub-segment `a → b`, exactly.
fn cubic(a: &Node, b: &Node, s: &R) -> R {
    let one = R::one();
    let v = &one - s;
    let y = [a.u.clone(), &a.u + &a.right_dv, &b.u + &b.left_dv, b.u.clone()];
    &v * &v * &v * &y[0]
        + R::from_integer(3.into()) * &v * &v * s * &y[1]
        + R::from_integer(3.into()) * &v * s * s * &y[2]
        + s * s * s * &y[3]
}

/// y′(s) / 3 of the sub-segment, exactly.
fn slope(a: &Node, b: &Node, s: &R) -> R {
    let one = R::one();
    let v = &one - s;
    let (p, qq, r) = (a.right_dv.clone(), &(&b.u + &b.left_dv) - &(&a.u + &a.right_dv), -b.left_dv.clone());
    &v * &v * &p + R::from_integer(2.into()) * &v * s * &qq + s * s * &r
}

/// A root of `g` in [lo, hi] where g changes sign, by rational bisection to 2^-60.
fn root(g: &dyn Fn(&R) -> R, mut lo: R, mut hi: R) -> R {
    let up = g(&lo) < R::zero();
    for _ in 0..60 {
        let mid = (&lo + &hi) / R::from_integer(2.into());
        if (g(&mid) < R::zero()) == up {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / R::from_integer(2.into())
}

/// The parameters in (0, 1) splitting a sub-segment into pieces where y is monotone: the sign changes of y′, found on
/// both sides of the derivative's vertex (y′ is a quadratic, so monotone on each side).
fn splits(a: &Node, b: &Node) -> Vec<R> {
    let g = |s: &R| slope(a, b, s);
    // The vertex of y′: where its own derivative vanishes; y′ = A s² + B s + C with these exact coefficients.
    let c0 = g(&R::zero());
    let c1 = g(&R::one());
    let half = g(&q(1, 2));
    // A = 2(c0 + c1 − 2·half), B = c1 − c0 − A.
    let qa = R::from_integer(2.into()) * (&c0 + &c1 - R::from_integer(2.into()) * &half);
    let qb = &c1 - &c0 - &qa;
    let mut cuts = vec![R::zero()];
    if !qa.is_zero() {
        let vertex = -qb.clone() / (R::from_integer(2.into()) * &qa);
        if vertex > R::zero() && vertex < R::one() {
            cuts.push(vertex);
        }
    }
    cuts.push(R::one());
    // Exact roots when the discriminant is the square of a rational (a touch at a rational extremum must be exact).
    let qc = c0.clone();
    if !qa.is_zero() {
        let disc = &qb * &qb - R::from_integer(4.into()) * &qa * &qc;
        if disc >= R::zero() {
            let (n, d) = (disc.numer().clone(), disc.denom().clone());
            let (rn, rd) = (n.sqrt(), d.sqrt());
            if &rn * &rn == n && &rd * &rd == d {
                let sq = R::new(rn, rd);
                let mut exact: Vec<R> = [(-&qb - &sq), (-&qb + &sq)]
                    .into_iter()
                    .map(|x| x / (R::from_integer(2.into()) * &qa))
                    .filter(|x| *x > R::zero() && *x < R::one())
                    .collect();
                exact.sort();
                exact.dedup();
                return exact;
            }
        }
    } else if !qb.is_zero() {
        let x = -qc / &qb;
        return if x > R::zero() && x < R::one() { vec![x] } else { Vec::new() };
    }
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (lo, hi) = (w[0].clone(), w[1].clone());
        let (glo, ghi) = (g(&lo), g(&hi));
        if (glo < R::zero()) != (ghi < R::zero()) && !glo.is_zero() && !ghi.is_zero() {
            out.push(root(&g, lo, hi));
        }
    }
    out
}

/// The earliest frame of the segment's curve where u ≥ `arc` (path length 1, so V = u), exactly bracketed.
fn oracle(nodes: &[Node], arc: &R) -> f64 {
    for w in nodes.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let mut cuts = vec![R::zero()];
        cuts.extend(splits(a, b));
        cuts.push(R::one());
        let len = R::from_integer((b.frame - a.frame).into());
        let frame = |s: &R| R::from_integer(a.frame.into()) + s * &len;
        if cubic(a, b, &R::zero()) >= *arc {
            return f(&frame(&R::zero()));
        }
        for c in cuts.windows(2) {
            if cubic(a, b, &c[1]) >= *arc {
                let g = |s: &R| cubic(a, b, s) - arc;
                // The piece is monotone and starts below the arc; its end reaches it exactly or passes it.
                if g(&c[1]).is_zero() {
                    return f(&frame(&c[1]));
                }
                let s = root(&g, c[0].clone(), c[1].clone());
                return f(&frame(&s));
            }
        }
    }
    f(&R::from_integer(nodes.last().unwrap().frame.into()))
}

/// The product's view of the same curve: pins at the first and last node frames on a unit straight path.
fn product(nodes: &[Node], arc: f64) -> f64 {
    let first = nodes.first().unwrap().frame as f64;
    let last = nodes.last().unwrap().frame as f64;
    let point = |x: f64, frame: f64| Point {
        pos: [x, 0.0, 0.0],
        t: 0.0,
        c: 0.0,
        b: 0.0,
        frame: Some(frame),
        constant: false,
        lock_depth: false,
    };
    let path = Path::new(vec![point(0.0, first), point(1.0, last)], false);
    assert_eq!(path.length(), 1.0);
    let mut keys = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let prev = i.checked_sub(1).map_or(0.0, |p| (n.frame - nodes[p].frame) as f64 / 3.0);
        let next = nodes.get(i + 1).map_or(0.0, |x| (x.frame - n.frame) as f64 / 3.0);
        keys.push(KeyView {
            frame: n.frame as f64,
            value: f(&n.u),
            constant: false,
            left: (-prev, f(&n.left_dv)),
            right: (next, f(&n.right_dv)),
            easing: None,
        });
    }
    let v = Velocity { curve: CurveType::Spline, keys };
    v.passage(&path, 0, arc, &mut |_| Ok::<_, ()>(())).unwrap()
}

fn node(frame: i64, u: R, left: R, right: R) -> Node {
    Node { frame, u, left_dv: left, right_dv: right }
}

fn check(nodes: &[Node], arc: R) {
    let want = oracle(nodes, &arc);
    let got = product(nodes, f(&arc));
    // Bisected to 1e-4, then rounded half up to 3 decimals.
    assert!((got - want).abs() <= 0.0006, "got {got}, exact {want}");
    assert_eq!(got, round3(got), "reported to 3 decimals");
}

/// M3 C3.4 round 3: the passage property's counterexample. The second sub-segment's maximum equals the arc exactly
/// (0.9 at s = 1/2, frame 3, a tangent touch); the curve falls back and crosses again near frame 3.6. The earliest
/// frame with V ≥ arc is the touch, which doubles evaluate one rounding error low (`tcb::PASSAGE_TOUCH`).
#[test]
fn a_tangent_touch_at_an_extremum_is_the_passage() {
    let nodes = [
        node(1, q(0, 1), q(0, 1), q(0, 10)),
        node(2, q(5, 10), q(0, 10), q(7, 10)),
        node(4, q(1, 1), q(-3, 10), q(0, 1)),
    ];
    check(&nodes, q(90, 100));
    assert_eq!(product(&nodes, 0.9), 3.0);
}

#[test]
fn an_excursion_within_one_frame_is_found_at_its_first_crossing() {
    // Frames 1 → 3: u rises from 0 to a peak of 0.75 at frame 1.5 and is back to 0 at frame 2, then rises linearly to 1.
    let nodes =
        [node(1, q(0, 1), q(0, 1), q(1, 1)), node(2, q(0, 1), q(1, 1), q(0, 1)), node(3, q(1, 1), q(0, 1), q(0, 1))];
    // The first crossing of ½: s = (1 − √⅓)/2, frame 1.211; the old whole-frame scan reported 2.500.
    let exact = 1.0 + (1.0 - (1.0_f64 / 3.0).sqrt()) / 2.0;
    check(&nodes, q(1, 2));
    assert_eq!(product(&nodes, 0.5), round3(exact));
    // An arc above the peak is first reached on the way up to frame 3.
    check(&nodes, q(4, 5));
    assert!(product(&nodes, 0.8) > 2.0);
}

#[test]
fn a_rise_and_fall_twice_within_one_frame_reports_the_first_rise() {
    // One frame-long sub-segment whose value goes up, down and up again: y = (0, 1.5, −0.9, 0.6) in Bézier form.
    let nodes = [
        node(10, q(0, 1), q(0, 1), q(3, 2)),
        node(11, q(3, 5), q(-3, 2), q(0, 1)),
        node(12, q(1, 1), q(0, 1), q(0, 1)),
    ];
    for arc in [q(1, 5), q(3, 10), q(2, 5), q(1, 2), q(59, 100)] {
        check(&nodes, arc.clone());
        assert!(product(&nodes, f(&arc)) < 11.0, "found inside the first frame");
    }
}

#[test]
fn crossings_exactly_at_whole_frames_report_those_frames() {
    // Linear pieces through a key at frame 4 with u = ½: arc ½ is passed exactly at frame 4.
    let nodes =
        [node(2, q(0, 1), q(0, 1), q(0, 1)), node(4, q(1, 2), q(0, 1), q(0, 1)), node(6, q(1, 1), q(0, 1), q(0, 1))];
    assert_eq!(product(&nodes, 0.5), 4.0);
    check(&nodes, q(1, 2));
    // A curve that touches the arc exactly at a key and falls back: the touch is the passage.
    let touch = [
        node(1, q(0, 1), q(0, 1), q(0, 1)),
        node(3, q(3, 4), q(0, 1), q(0, 1)),
        node(5, q(1, 4), q(0, 1), q(0, 1)),
        node(7, q(1, 1), q(0, 1), q(0, 1)),
    ];
    assert_eq!(product(&touch, 0.75), 3.0);
    // At or below the start pin: the start.
    assert_eq!(product(&nodes, 0.0), 2.0);
    // The end pin when nothing earlier reaches the arc.
    assert_eq!(product(&nodes, 1.0), 6.0);
}

#[test]
fn the_search_is_charged_and_a_refusing_meter_stops_it() {
    let nodes =
        [node(1, q(0, 1), q(0, 1), q(1, 1)), node(2, q(0, 1), q(1, 1), q(0, 1)), node(3, q(1, 1), q(0, 1), q(0, 1))];
    let point = |x: f64, frame: f64| Point {
        pos: [x, 0.0, 0.0],
        t: 0.0,
        c: 0.0,
        b: 0.0,
        frame: Some(frame),
        constant: false,
        lock_depth: false,
    };
    let path = Path::new(vec![point(0.0, 1.0), point(1.0, 3.0)], false);
    let keys = vec![
        KeyView { right: (1.0 / 3.0, 1.0), ..KeyView::plain(1.0, 0.0) },
        KeyView { left: (-1.0 / 3.0, 1.0), ..KeyView::plain(2.0, 0.0) },
    ];
    let v = Velocity { curve: CurveType::Spline, keys };
    let mut units = 0u64;
    let got = v.passage(&path, 0, 0.5, &mut |n| {
        units += n;
        Ok::<_, ()>(())
    });
    assert!(got.is_ok() && units > 0 && units < 100, "{units}");
    assert_eq!(v.passage(&path, 0, 0.5, &mut |_| Err::<(), _>("stop")), Err("stop"));
    let _ = nodes;
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(std::env::var("TT_C32_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(256)))]

    /// Random handles (any sign, so curves overshoot and fall back) and arcs: the product agrees with the oracle.
    #[test]
    fn passage_agrees_with_the_exact_oracle(
        h in prop::array::uniform4(-30i64..30), mid in 0i64..10, len_a in 1i64..6, len_b in 1i64..6, arc in 1i64..99,
    ) {
        let nodes = [
            node(1, q(0, 1), q(0, 1), q(h[0], 10)),
            node(1 + len_a, q(mid, 10), q(h[1], 10), q(h[2], 10)),
            node(1 + len_a + len_b, q(1, 1), q(h[3], 10), q(0, 1)),
        ];
        let arc = q(arc, 100);
        let want = oracle(&nodes, &arc);
        let got = product(&nodes, f(&arc));
        prop_assert!((got - want).abs() <= 0.0006, "got {got}, exact {want}");
    }
}
