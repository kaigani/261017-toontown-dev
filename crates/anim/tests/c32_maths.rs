//! M3 C3.2: the evaluator's maths against independent oracles: exact rationals for the Hermite form, tangents and
//! Bézier identities, hand-derived poses for matrices and projections, an independent chord sum and a fine quadrature
//! for arc length. Nothing here calls the product to compute an expected value.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use proptest::prelude::*;
use tt_anim::camera::{CameraView, NEAR_PLANE};
use tt_anim::function::{self, BEZIER_MAX_ITERATIONS, KeyView, preset_points};
use tt_anim::matrix::Mat4;
use tt_anim::node::local_matrix;
use tt_anim::tcb::{self, Path, Point, Velocity};
use tt_model::entities::channel::{CurveType, Easing};

/// An exact rational (enough range for the small test inputs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Q(i128, i128);

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

impl Q {
    fn new(n: i128, d: i128) -> Self {
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Self(s * n / g, s * d / g)
    }
    fn int(n: i128) -> Self {
        Self(n, 1)
    }
    fn f(self) -> f64 {
        self.0 as f64 / self.1 as f64
    }
}

impl std::ops::Add for Q {
    type Output = Q;
    fn add(self, o: Q) -> Q {
        Q::new(self.0 * o.1 + o.0 * self.1, self.1 * o.1)
    }
}
impl std::ops::Sub for Q {
    type Output = Q;
    fn sub(self, o: Q) -> Q {
        Q::new(self.0 * o.1 - o.0 * self.1, self.1 * o.1)
    }
}
impl std::ops::Mul for Q {
    type Output = Q;
    fn mul(self, o: Q) -> Q {
        Q::new(self.0 * o.0, self.1 * o.1)
    }
}

fn point(x: f64, y: f64, z: f64, frame: Option<f64>) -> Point {
    Point { pos: [x, y, z], t: 0.0, c: 0.0, b: 0.0, frame, constant: false, lock_depth: false }
}

fn close(a: f64, b: f64, tolerance: f64) {
    assert!((a - b).abs() <= tolerance, "{a} vs {b} (tolerance {tolerance})");
}

// ----- 05 §2: matrices ------------------------------------------------------------------------------------------------

#[test]
fn elementary_matrices_follow_05_2_1() {
    // Counter-clockwise: (1, 0) goes to (0, 1) under R(90), exactly.
    assert_eq!(Mat4::rotate(90.0).apply([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    assert_eq!(Mat4::rotate(180.0).apply([1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
    assert_eq!(Mat4::rotate(-90.0).apply([1.0, 0.0, 0.0]), [0.0, -1.0, 0.0]);
    // Positive skew leans the top to the right: x′ = x + y·tan κ.
    let sheared = Mat4::skew(45.0).apply([0.0, 2.0, 0.0]);
    close(sheared[0], 2.0, 1e-12);
    close(sheared[1], 2.0, 0.0);
    // Row-major with the translation in m03, m13, m23.
    let t = Mat4::translate(3.0, 4.0, 5.0);
    assert_eq!((t.0[3], t.0[7], t.0[11]), (3.0, 4.0, 5.0));
    assert_eq!(Mat4::scale(-1.0, 2.0).determinant(), -2.0);
    assert_eq!(Mat4::skew(30.0).determinant(), 1.0, "shear preserves area");
}

#[test]
fn local_matrix_order_is_scale_skew_rotate_translate_about_the_pivots() {
    // Hand-derived. P = (10, 20, 3), θ = 90, S = (2, 3), cr = (1, 0), cs = (0, 1), no skew.
    // A point (1, 1): − cs → (1, 0); S → (2, 0); + cs → (2, 1); − cr → (1, 1); R90 → (−1, 1); + cr → (0, 1); + P.
    let m = local_matrix([10.0, 20.0, 3.0], 90.0, 0.0, [2.0, 3.0], [1.0, 0.0], [0.0, 1.0]);
    assert_eq!(m.apply([1.0, 1.0, 0.0]), [10.0, 21.0, 3.0]);
    // Only translation changes Z.
    assert_eq!(m.apply([0.0, 0.0, 7.0])[2], 10.0);
    // A mirrored node has determinant < 0; rotation and skew keep it.
    let mirrored = local_matrix([0.0; 3], 33.0, 20.0, [-2.0, 3.0], [0.0, 0.0], [0.0, 0.0]);
    close(mirrored.determinant(), -6.0, 1e-12);
}

#[test]
fn decomposition_reports_a_negative_sy_for_a_mirror() {
    let d = (Mat4::rotate(30.0).mul(&Mat4::skew(20.0)).mul(&Mat4::scale(2.0, -3.0))).decompose();
    close(d.rotation, 30.0, 1e-9);
    close(d.skew, 20.0, 1e-9);
    close(d.sx, 2.0, 1e-12);
    close(d.sy, -3.0, 1e-12);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn decomposition_inverts_the_composition(
        theta in -179.0..179.0f64, kappa in -80.0..80.0f64,
        sx in 0.01..50.0f64, sy in prop_oneof![0.01..50.0f64, -50.0..-0.01f64],
    ) {
        let d = Mat4::rotate(theta).mul(&Mat4::skew(kappa)).mul(&Mat4::scale(sx, sy)).decompose();
        prop_assert!((d.rotation - theta).abs() < 1e-7, "{d:?}");
        prop_assert!((d.sx - sx).abs() < 1e-9, "{d:?}");
        prop_assert!((d.sy - sy).abs() < 1e-9, "{d:?}");
        prop_assert!((d.skew - kappa).abs() < 1e-7, "{d:?}");
    }

    #[test]
    fn inverse_targets_any_world_point(
        p in prop::array::uniform3(-100.0..100.0f64), theta in -180.0..180.0f64, kappa in -80.0..80.0f64,
        s in prop::array::uniform2(0.05..20.0f64), flip in any::<bool>(), q in prop::array::uniform3(-50.0..50.0f64),
    ) {
        let sx = if flip { -s[0] } else { s[0] };
        let m = local_matrix(p, theta, kappa, [sx, s[1]], [1.0, 2.0], [-3.0, 0.5]);
        let back = m.inverse().unwrap().apply(m.apply(q));
        for i in 0..3 {
            prop_assert!((back[i] - q[i]).abs() < 1e-6, "{back:?} {q:?}");
        }
    }
}

// ----- 05 §9.1: the curve --------------------------------------------------------------------------------------------

/// Dout and Din of 05 §9.1, and C(s), written out in exact arithmetic for one axis.
fn exact_tangents(before: Q, after: Q, t: Q, c: Q, b: Q) -> (Q, Q) {
    let one = Q::int(1);
    let half = Q::new(1, 2);
    let dout = (one - t) * (one + b) * (one + c) * half * before + (one - t) * (one - b) * (one - c) * half * after;
    let din = (one - t) * (one + b) * (one - c) * half * before + (one - t) * (one - b) * (one + c) * half * after;
    (dout, din)
}

fn exact_hermite(p0: Q, d0: Q, p1: Q, d1: Q, s: Q) -> Q {
    let s2 = s * s;
    let s3 = s2 * s;
    (Q::int(2) * s3 - Q::int(3) * s2 + Q::int(1)) * p0
        + (s3 - Q::int(2) * s2 + s) * d0
        + (Q::int(-2) * s3 + Q::int(3) * s2) * p1
        + (s3 - s2) * d1
}

#[test]
fn hermite_with_tcb_equals_the_exact_rational_formula() {
    // Four points with rational TCB; interior segment and both reflected ends.
    let xs = [Q::int(0), Q::int(4), Q::new(13, 2), Q::int(9)];
    let tcbs = [
        (Q::new(1, 2), Q::new(-1, 4), Q::new(1, 8)),
        (Q::new(-1, 2), Q::new(1, 2), Q::new(-3, 4)),
        (Q::new(1, 4), Q::new(0, 1), Q::new(1, 2)),
        (Q::new(0, 1), Q::new(1, 8), Q::new(-1, 8)),
    ];
    let points: Vec<Point> = (0..4)
        .map(|i| Point {
            pos: [xs[i].f(), 0.0, 0.0],
            t: tcbs[i].0.f(),
            c: tcbs[i].1.f(),
            b: tcbs[i].2.f(),
            frame: Some(1.0 + i as f64),
            constant: false,
            lock_depth: false,
        })
        .collect();
    let path = Path::new(points, false);
    let before = [xs[1] - xs[0], xs[1] - xs[0], xs[2] - xs[1], xs[3] - xs[2]];
    let after = [xs[1] - xs[0], xs[2] - xs[1], xs[3] - xs[2], xs[3] - xs[2]];
    for seg in 0..3 {
        let (d0, _) = exact_tangents(before[seg], after[seg], tcbs[seg].0, tcbs[seg].1, tcbs[seg].2);
        let (_, d1) = exact_tangents(before[seg + 1], after[seg + 1], tcbs[seg + 1].0, tcbs[seg + 1].1, tcbs[seg + 1].2);
        for k in 0..=8 {
            let s = Q::new(k, 8);
            let want = exact_hermite(xs[seg], d0, xs[seg + 1], d1, s);
            close(path.curve(seg, s.f())[0], want.f(), 1e-12);
        }
    }
}

#[test]
fn curve_endpoints_and_special_cases() {
    // A curve passes exactly through its points.
    let path = Path::new(
        vec![point(0.0, 0.0, 0.0, Some(1.0)), point(3.0, 1.0, 2.0, None), point(7.0, -2.0, 0.5, Some(9.0))],
        false,
    );
    assert_eq!(path.curve(0, 0.0), [0.0, 0.0, 0.0]);
    assert_eq!(path.curve(0, 1.0), [3.0, 1.0, 2.0]);
    assert_eq!(path.curve(1, 1.0), [7.0, -2.0, 0.5]);
    // Two points with t = c = b = 0: a straight line, travelled at parameter speed.
    let line = Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), point(8.0, 6.0, 0.0, Some(5.0))], false);
    assert_eq!(line.curve(0, 0.25), [2.0, 1.5, 0.0]);
    close(line.length(), 10.0, 1e-12);
    // Tension 1 zeroes the tangents: the segment eases between its ends with a cusp at each point.
    let mut sharp = point(4.0, 0.0, 0.0, None);
    sharp.t = 1.0;
    let mut a = point(0.0, 0.0, 0.0, Some(1.0));
    a.t = 1.0;
    let path = Path::new(vec![a, sharp], false);
    assert_eq!(path.derivative(0, 0.0), [0.0, 0.0, 0.0]);
    assert_eq!(path.curve(0, 0.5), [2.0, 0.0, 0.0]);
    // A single point is a constant position; coincident neighbours keep the curve finite.
    let single = Path::new(vec![point(1.0, 2.0, 3.0, Some(1.0))], false);
    assert_eq!(single.segments(), 0);
    assert_eq!(single.length(), 0.0);
    let twin = Path::new(vec![point(1.0, 1.0, 0.0, Some(1.0)), point(1.0, 1.0, 0.0, Some(5.0))], false);
    assert_eq!(twin.length(), 0.0);
    assert_eq!(twin.curve(0, 0.5), [1.0, 1.0, 0.0]);
    // Reversing a path with symmetric parameters reverses the curve.
    let forward = Path::new(
        vec![point(0.0, 0.0, 0.0, Some(1.0)), point(2.0, 3.0, 0.0, None), point(5.0, 1.0, 0.0, Some(9.0))],
        false,
    );
    let back = Path::new(
        vec![point(5.0, 1.0, 0.0, Some(1.0)), point(2.0, 3.0, 0.0, None), point(0.0, 0.0, 0.0, Some(9.0))],
        false,
    );
    for k in 0..=16 {
        let s = f64::from(k) / 16.0;
        let a = forward.curve(0, s);
        let b = back.curve(1, 1.0 - s);
        close(a[0], b[0], 1e-12);
        close(a[1], b[1], 1e-12);
    }
    close(forward.length(), back.length(), 1e-9);
}

#[test]
fn arc_length_is_the_64_step_chord_sum_and_near_the_true_length() {
    let pts = vec![point(0.0, 0.0, 0.0, Some(1.0)), point(4.0, 3.0, 0.0, None), point(8.0, 0.0, 2.0, Some(24.0))];
    let path = Path::new(pts, false);
    // An independent chord sum through the rational form of C (t = c = b = 0: Catmull–Rom with reflected ends).
    let c = |seg: usize, s: f64| -> [f64; 3] {
        let p = [[0.0, 0.0, 0.0], [4.0, 3.0, 0.0], [8.0, 0.0, 2.0]];
        let m = |a: [f64; 3], b: [f64; 3]| [(b[0] - a[0]) / 2.0, (b[1] - a[1]) / 2.0, (b[2] - a[2]) / 2.0];
        let reflect_start = [p[0][0] - (p[1][0] - p[0][0]), p[0][1] - (p[1][1] - p[0][1]), p[0][2] - (p[1][2] - p[0][2])];
        let reflect_end = [2.0 * p[2][0] - p[1][0], 2.0 * p[2][1] - p[1][1], 2.0 * p[2][2] - p[1][2]];
        let (d0, d1) = if seg == 0 { (m(reflect_start, p[1]), m(p[0], p[2])) } else { (m(p[0], p[2]), m(p[1], reflect_end)) };
        let (a, b) = (p[seg], p[seg + 1]);
        let (s2, s3) = (s * s, s * s * s);
        let mut q = [0.0; 3];
        for i in 0..3 {
            q[i] = (2.0 * s3 - 3.0 * s2 + 1.0) * a[i] + (s3 - 2.0 * s2 + s) * d0[i] + (-2.0 * s3 + 3.0 * s2) * b[i] + (s3 - s2) * d1[i];
        }
        q
    };
    let chord = |steps: usize| -> f64 {
        let mut total = 0.0;
        for seg in 0..2 {
            let mut prev = c(seg, 0.0);
            for i in 1..=steps {
                let q = c(seg, i as f64 / steps as f64);
                total += ((q[0] - prev[0]).powi(2) + (q[1] - prev[1]).powi(2) + (q[2] - prev[2]).powi(2)).sqrt();
                prev = q;
            }
        }
        total
    };
    close(path.length(), chord(64), 1e-9);
    assert!((path.length() - chord(20_000)).abs() < 1e-3, "the 64-step sum stays near the true length");
    // Locating an arc length and evaluating lands within a chord of that distance from the start.
    let (seg, s) = path.locate(path.arc_of_point(1), 0, 2);
    let q = path.curve(seg, s);
    close(q[0], 4.0, 1e-9);
    close(q[1], 3.0, 1e-9);
    assert_eq!(path.locate(-1.0, 0, 2), (0, 0.0));
    assert_eq!(path.locate(1e9, 0, 2), (1, 1.0));
}

#[test]
fn constant_z_measures_arc_length_in_the_plane() {
    let mut a = point(0.0, 0.0, 0.0, Some(1.0));
    a.lock_depth = true;
    let flat = Path::new(vec![a, point(3.0, 4.0, 100.0, Some(5.0))], false);
    close(flat.length(), 5.0, 1e-12);
    let node_flag = Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), point(3.0, 4.0, 100.0, Some(5.0))], true);
    close(node_flag.length(), 5.0, 1e-12);
    // Z holds the segment's start until the next keyframe, and is exact at it.
    let v = Velocity::default();
    assert_eq!(tcb::travel(&flat, &v, false, 3.0).position, [1.5, 2.0, 0.0]);
    assert_eq!(tcb::travel(&flat, &v, false, 5.0).position, [3.0, 4.0, 100.0]);
}

// ----- 05 §10: functions --------------------------------------------------------------------------------------------

fn eased(e: Easing) -> KeyView {
    KeyView { easing: Some(e), ..KeyView::plain(1.0, 0.0) }
}

#[test]
fn presets_match_the_normative_table_and_their_endpoints() {
    assert_eq!(preset_points(Easing::EaseInOut), Some(((1.0 / 3.0, 0.0), (2.0 / 3.0, 1.0))));
    assert_eq!(preset_points(Easing::FastInOut), Some(((0.0, 0.5), (1.0, 0.5))));
    assert_eq!(preset_points(Easing::ExtremeEaseInOut), Some(((0.5, 0.0), (0.5, 1.0))));
    assert_eq!(preset_points(Easing::ExtremeFastInOut), Some(((0.0, 1.0), (1.0, 0.0))));
    assert_eq!(preset_points(Easing::SlowAcceleration), Some(((1.0 / 3.0, 0.0), (1.0, 1.0))));
    assert_eq!(preset_points(Easing::SlowDeceleration), Some(((0.0, 0.0), (2.0 / 3.0, 1.0))));
    assert_eq!(preset_points(Easing::Constant), None);
    let keys = |e| vec![eased(e), KeyView::plain(25.0, 10.0)];
    for e in [
        Easing::Linear,
        Easing::EaseInOut,
        Easing::FastInOut,
        Easing::ExtremeEaseInOut,
        Easing::ExtremeFastInOut,
        Easing::SlowAcceleration,
        Easing::SlowDeceleration,
    ] {
        let k = keys(e);
        assert_eq!(function::evaluate(CurveType::Interpolation, &k, 1.0, None), Some(0.0), "{e:?} start");
        assert_eq!(function::evaluate(CurveType::Interpolation, &k, 25.0, None), Some(10.0), "{e:?} end");
        // All these presets are symmetric or monotone enough to stay in range except the overshoot-free check below.
        let mid = function::evaluate(CurveType::Interpolation, &k, 13.0, None).unwrap();
        assert!((0.0..=10.0).contains(&mid), "{e:?}: {mid}");
    }
    // Linear is exact, not solved.
    let l = keys(Easing::Linear);
    assert_eq!(function::evaluate(CurveType::Interpolation, &l, 7.0, None), Some(2.5));
    // Point symmetry of easeInOut: halfway in time is halfway in value.
    close(function::evaluate(CurveType::Interpolation, &keys(Easing::EaseInOut), 13.0, None).unwrap(), 5.0, 1e-9);
    // easeInOut in closed form: x(u) = u (P1x, P2x at thirds), y(u) = 3u² − 2u³.
    let u: f64 = 0.25;
    let want = 10.0 * (3.0 * u * u - 2.0 * u * u * u);
    close(function::evaluate(CurveType::Interpolation, &keys(Easing::EaseInOut), 7.0, None).unwrap(), want, 1e-9);
}

#[test]
fn spline_handles_constant_segments_and_holds() {
    // A spline with handles at thirds on the line is the line.
    let a = KeyView { right: (4.0, 2.0), ..KeyView::plain(1.0, 0.0) };
    let b = KeyView { left: (-4.0, -2.0), ..KeyView::plain(13.0, 6.0) };
    let k = [a, b];
    close(function::evaluate(CurveType::Spline, &k, 7.0, None).unwrap(), 3.0, 1e-9);
    // No handles at all is the straight line, exactly.
    let plain = [KeyView::plain(1.0, 0.0), KeyView::plain(5.0, 1.0)];
    assert_eq!(function::evaluate(CurveType::Spline, &plain, 2.0, None), Some(0.25));
    // Handle frame offsets beyond the segment are clamped: still a function of time, still ends exactly.
    let wild = [
        KeyView { right: (100.0, 5.0), ..KeyView::plain(1.0, 0.0) },
        KeyView { left: (-100.0, 5.0), ..KeyView::plain(5.0, 1.0) },
    ];
    let mut last = f64::MIN;
    for i in 0..=40 {
        let f = 1.0 + f64::from(i) * 0.1;
        let x = function::evaluate(CurveType::Spline, &wild, f, None).unwrap();
        assert!(x.is_finite());
        last = last.max(x);
    }
    assert!(last.is_finite());
    assert_eq!(function::evaluate(CurveType::Spline, &wild, 5.0, None), Some(1.0));
    // Constant segments hold, then jump at the next key.
    let hold = [KeyView { constant: true, ..KeyView::plain(1.0, 2.0) }, KeyView::plain(5.0, 9.0)];
    assert_eq!(function::evaluate(CurveType::Spline, &hold, 4.999, None), Some(2.0));
    assert_eq!(function::evaluate(CurveType::Spline, &hold, 5.0, None), Some(9.0));
    // Holds before and after.
    assert_eq!(function::evaluate(CurveType::Spline, &hold, -3.0, None), Some(2.0));
    assert_eq!(function::evaluate(CurveType::Spline, &hold, 99.0, None), Some(9.0));
    assert_eq!(function::evaluate(CurveType::Spline, &[], 1.0, None), None);
    // velocityBased: value follows V; a stationary V falls back to the time fraction.
    let vb = [KeyView::plain(1.0, 0.0), KeyView::plain(5.0, 100.0)];
    let v = |f: f64| (f - 1.0) * (f - 1.0);
    assert_eq!(function::evaluate(CurveType::VelocityBased, &vb, 3.0, Some(&v)), Some(25.0));
    let flat = |_: f64| 7.0;
    assert_eq!(function::evaluate(CurveType::VelocityBased, &vb, 3.0, Some(&flat)), Some(50.0));
    assert_eq!(function::evaluate(CurveType::VelocityBased, &vb, 3.0, None), Some(50.0));
    assert!(BEZIER_MAX_ITERATIONS >= 30, "the time solve reaches 1e-9");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// x is monotone for clamped handles, so a later frame never has an earlier parameter; checked through an
    /// independent bisection on the same polynomial.
    #[test]
    fn the_time_solve_agrees_with_an_independent_bisection(
        a in 0.0..12.0f64, c in -12.0..0.0f64, b in -5.0..5.0f64, d in -5.0..5.0f64, f in 1.0..13.0f64,
    ) {
        let keys = [
            KeyView { right: (a, b), ..KeyView::plain(1.0, 0.0) },
            KeyView { left: (c, d), ..KeyView::plain(13.0, 1.0) },
        ];
        let got = function::evaluate(CurveType::Spline, &keys, f, None).unwrap();
        let bern = |p: [f64; 4], u: f64| {
            let v = 1.0 - u;
            v * v * v * p[0] + 3.0 * v * v * u * p[1] + 3.0 * v * u * u * p[2] + u * u * u * p[3]
        };
        let xs = [0.0, a, 12.0 + c, 12.0];
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..200 {
            let m = (lo + hi) / 2.0;
            if bern(xs, m) < f - 1.0 { lo = m } else { hi = m }
        }
        let want = bern([0.0, b, 1.0 + d, 1.0], (lo + hi) / 2.0);
        prop_assert!((got - want).abs() < 1e-6, "{got} {want}");
    }
}

// ----- 05 §10.5: velocity -------------------------------------------------------------------------------------------

fn two_keys() -> Path {
    Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), point(10.0, 0.0, 0.0, Some(11.0))], false)
}

#[test]
fn velocity_pins_clamping_and_intermediate_keys() {
    let path = two_keys();
    let linear = Velocity::default();
    // Default: linear, constant speed; exact at the keyframes.
    assert_eq!(linear.value(&path, 1.0), 0.0);
    assert_eq!(linear.value(&path, 6.0), 5.0);
    assert_eq!(linear.value(&path, 11.0), 10.0);
    assert_eq!(tcb::travel(&path, &linear, false, 6.0).position, [5.0, 0.0, 0.0]);
    // Outside the keyframes: the pins hold.
    assert_eq!(linear.value(&path, -5.0), 0.0);
    assert_eq!(linear.value(&path, 500.0), 10.0);
    assert_eq!(tcb::travel(&path, &linear, false, 500.0).position, [10.0, 0.0, 0.0]);
    // A pause: u stays 0.5 from frame 3 to frame 9 (stored normalised).
    let pause = Velocity {
        curve: CurveType::Interpolation,
        keys: vec![KeyView::plain(3.0, 0.5), KeyView::plain(9.0, 0.5)],
    };
    assert_eq!(pause.value(&path, 3.0), 5.0);
    assert_eq!(pause.value(&path, 6.0), 5.0);
    assert_eq!(pause.value(&path, 2.0), 2.5);
    // A V outside the segment travels only to its end (05 §10.5: clamped), and may run backward inside it.
    let over = Velocity {
        curve: CurveType::Interpolation,
        keys: vec![KeyView::plain(4.0, 1.5), KeyView::plain(8.0, -0.5)],
    };
    assert_eq!(tcb::travel(&path, &over, false, 4.0).position, [10.0, 0.0, 0.0]);
    assert_eq!(tcb::travel(&path, &over, false, 8.0).position, [0.0, 0.0, 0.0]);
    assert_eq!(over.value(&path, 4.0), 15.0, "V itself is reported unclamped");
    // Zero-length segments stay put.
    let twin = Path::new(vec![point(2.0, 2.0, 0.0, Some(1.0)), point(2.0, 2.0, 0.0, Some(9.0))], false);
    assert_eq!(tcb::travel(&twin, &linear, false, 5.0).position, [2.0, 2.0, 0.0]);
}

#[test]
fn passage_frames_are_found_to_the_stated_tolerance() {
    let path = two_keys();
    let mut units = 0u64;
    let mut spend = |n: u64| -> Result<(), ()> {
        units += n;
        Ok(())
    };
    // Linear: arc 2.5 is reached at frame 3.5.
    assert_eq!(Velocity::default().passage(&path, 0, 2.5, &mut spend), Ok(3.5));
    // An easeInOut segment: V(f) = 10·(3u² − 2u³) with u = (f − 1) ÷ 10 reaches 5 at frame 6.
    let eased_v = Velocity { curve: CurveType::Interpolation, keys: vec![eased(Easing::EaseInOut)] };
    assert_eq!(eased_v.passage(&path, 0, 5.0, &mut spend), Ok(6.0));
    // The smallest frame of a non-monotonic curve: V passes 8 on the way up at frame 3, not on the way back.
    let back = Velocity {
        curve: CurveType::Interpolation,
        keys: vec![KeyView::plain(4.0, 1.2), KeyView::plain(8.0, 0.1)],
    };
    assert_eq!(back.passage(&path, 0, 8.0, &mut spend), Ok(3.0));
    // An arc at or before the start is passed at the start; a zero-length segment too.
    assert_eq!(Velocity::default().passage(&path, 0, 0.0, &mut spend), Ok(1.0));
    let twin = Path::new(vec![point(2.0, 2.0, 0.0, Some(1.0)), point(2.0, 2.0, 0.0, Some(9.0))], false);
    assert_eq!(Velocity::default().passage(&twin, 0, 0.0, &mut spend), Ok(1.0));
    assert!(units > 0, "the search is charged");
    // A refusing meter stops it.
    let mut refuse = |_: u64| -> Result<(), &'static str> { Err("stop") };
    assert_eq!(Velocity::default().passage(&path, 0, 2.5, &mut refuse), Err("stop"));
    assert_eq!(tcb::round3(1.23456), 1.235);
}

#[test]
fn orient_angle_follows_the_tangent_and_searches_past_constant_segments() {
    let mut spend = |_: u64| -> Result<(), ()> { Ok(()) };
    let path = two_keys();
    assert_eq!(tcb::orient_angle(&path, Some((0, 0.5)), 6.0, &mut spend), Ok(0.0));
    let up = Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), point(0.0, 5.0, 0.0, Some(5.0))], false);
    assert_eq!(tcb::orient_angle(&up, Some((0, 0.5)), 3.0, &mut spend), Ok(90.0));
    // Single point: no valid tangent at all.
    let single = Path::new(vec![point(1.0, 1.0, 0.0, Some(1.0))], false);
    assert_eq!(tcb::orient_angle(&single, None, 1.0, &mut spend), Ok(0.0));
    // A constant second segment searches backward along the tweened first one.
    let mut b = point(10.0, 0.0, 0.0, Some(5.0));
    b.constant = true;
    let hold = Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), b, point(10.0, 10.0, 0.0, Some(9.0))], false);
    // The last valid tangent is Din(1) = ½(10, 0) + ½(0, 10): 45°.
    let got = tcb::orient_angle(&hold, None, 6.0, &mut spend).unwrap();
    close(got, 45.0, 1e-9);
    // A constant first segment finds the next valid angle forward.
    let mut a = point(0.0, 0.0, 0.0, Some(1.0));
    a.constant = true;
    let hold2 = Path::new(vec![a, point(0.0, 0.0, 0.0, Some(5.0)), point(0.0, 10.0, 0.0, Some(9.0))], false);
    let got = tcb::orient_angle(&hold2, None, 2.0, &mut spend).unwrap();
    assert!((got - 90.0).abs() < 1e-9, "{got}");
    // The depth of a tangent is ignored: straight into the screen has no direction.
    let deep = Path::new(vec![point(0.0, 0.0, 0.0, Some(1.0)), point(0.0, 0.0, 9.0, Some(5.0))], false);
    assert_eq!(tcb::orient_angle(&deep, Some((0, 0.5)), 3.0, &mut spend), Ok(0.0));
}

// ----- 05 §2.6: projection -------------------------------------------------------------------------------------------

/// The default camera: FOV 2·atan(0.375), at (0, 0, 16), 1920 × 1080.
fn default_camera(fov: f64) -> CameraView {
    CameraView::new(&Mat4::translate(0.0, 0.0, 16.0), fov, 1920.0, 1080.0)
}

#[test]
fn the_default_camera_fills_twelve_fields_of_the_table_plane() {
    let fov = 2.0 * libm::atan(0.375) * (180.0 / std::f64::consts::PI);
    let cam = default_camera(fov);
    let px = |x: f64, y: f64, z: f64| cam.project([x, y, z]).pixel.unwrap();
    let right = px(6.0, 0.0, 0.0);
    close(right[0], 1920.0, 1e-9);
    close(right[1], 540.0, 1e-9);
    let left = px(-6.0, 0.0, 0.0);
    close(left[0], 0.0, 1e-9);
    // 16:9 frame height is 6.75 fields; y is down.
    let top = px(0.0, 3.375, 0.0);
    close(top[1], 0.0, 1e-9);
    let bottom = px(0.0, -3.375, 0.0);
    close(bottom[1], 1080.0, 1e-9);
    assert_eq!(px(0.0, 0.0, 0.0), [960.0, 540.0]);
    assert_eq!(cam.project([0.0, 0.0, 0.0]).depth, 16.0);
    // An element at depth Z scales by D ÷ (D − Z): F values enlarge (Z = 8 → ×2), B values shrink (Z = −16 → ×½).
    close(px(3.0, 0.0, 8.0)[0] - 960.0, 2.0 * 480.0, 1e-9);
    close(px(3.0, 0.0, -16.0)[0] - 960.0, 0.5 * 480.0, 1e-9);
    // Frame corners on Z = 0, top-left first, clockwise.
    let c = cam.frame_corners_at_z0().unwrap();
    close(c[0][0], -6.0, 1e-9);
    close(c[0][1], 3.375, 1e-9);
    close(c[2][0], 6.0, 1e-9);
    close(c[2][1], -3.375, 1e-9);
}

#[test]
fn near_plane_roll_and_rigid_camera() {
    let cam = default_camera(41.1121);
    // At or behind the near plane: not projected.
    assert_eq!(cam.project([0.0, 0.0, 16.0 - NEAR_PLANE]).pixel, None);
    assert_eq!(cam.project([0.0, 0.0, 16.0]).pixel, None);
    assert_eq!(cam.project([0.0, 0.0, 20.0]).pixel, None);
    assert!(cam.project([0.0, 0.0, 16.0 - NEAR_PLANE - 1e-9]).pixel.is_some());
    // Roll 90°: the scene appears rotated −90° in the frame: world +X (right) goes to frame down... of camera +Y→−X.
    let rolled = CameraView::new(&Mat4::translate(0.0, 0.0, 16.0).mul(&Mat4::rotate(90.0)), 90.0, 1000.0, 1000.0);
    // tan 45° = 1: at depth 16 a point 16 to the world +Y is on camera +X: the right edge of the frame.
    let edge = rolled.project([0.0, 16.0, 0.0]).pixel.unwrap();
    close(edge[0], 1000.0, 1e-9);
    close(edge[1], 500.0, 1e-9);
    // Inherited scale and skew are ignored: the view equals the one of the same position and roll without them.
    let parent = Mat4::translate(1.0, 2.0, 30.0).mul(&Mat4::rotate(30.0)).mul(&Mat4::scale(5.0, 0.25));
    let plain = Mat4::translate(1.0, 2.0, 30.0).mul(&Mat4::rotate(30.0));
    let a = CameraView::new(&parent, 50.0, 800.0, 600.0);
    let b = CameraView::new(&plain, 50.0, 800.0, 600.0);
    for i in 0..16 {
        close(a.view.0[i], b.view.0[i], 1e-9);
    }
    // A camera at or behind the table plane has no corners on it; a wider FOV shows more; aspect follows W ÷ H.
    assert_eq!(CameraView::new(&Mat4::translate(0.0, 0.0, 0.05), 50.0, 800.0, 600.0).frame_corners_at_z0(), None);
    let wide = CameraView::new(&Mat4::translate(0.0, 0.0, 10.0), 90.0, 400.0, 400.0);
    let corners = wide.frame_corners_at_z0().unwrap();
    close(corners[1][0], 10.0, 1e-9);
    close(corners[1][1], 10.0, 1e-9);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn unprojecting_inverts_projection(
        p in prop::array::uniform3(-20.0..20.0f64), roll in -180.0..180.0f64, fov in 5.0..170.0f64,
        cam_z in 25.0..60.0f64, w in 16.0..4000.0f64, h in 16.0..4000.0f64,
    ) {
        let world = Mat4::translate(1.0, -2.0, cam_z).mul(&Mat4::rotate(roll));
        let cam = CameraView::new(&world, fov, w, h);
        let r = cam.project(p);
        let px = r.pixel.unwrap();
        let back = cam.unproject(px, r.depth).unwrap();
        for i in 0..3 {
            prop_assert!((back[i] - p[i]).abs() < 1e-6, "{back:?} {p:?}");
        }
    }
}

// ----- 05 §2.10, §8.4: timing -----------------------------------------------------------------------------------------

#[test]
fn cycle_mapping_is_exact() {
    use tt_anim::timing::Bar;
    let bar = Bar { start: 10.0, duration: 30.0, cycle: Some(12.0) };
    assert_eq!(bar.local(10.0), 1.0);
    assert_eq!(bar.local(5.0), 1.0, "held before the bar");
    assert_eq!(bar.local(100.0), 30.0, "held after the bar");
    assert_eq!(bar.scene(1.0), 10.0);
    assert_eq!(bar.cycle_frame(12.0), (12.0, 0.0));
    assert_eq!(bar.cycle_frame(13.0), (1.0, 1.0));
    assert_eq!(bar.cycle_frame(24.5), (12.5, 1.0));
    assert_eq!(bar.cycle_frame(25.0), (1.0, 2.0));
    assert_eq!(bar.cycle_frame(30.0), (6.0, 2.0), "a partial last cycle");
    assert!(bar.keyable(12.0));
    assert!(!bar.keyable(13.0), "a repeated cycle cannot be keyed");
    assert!(!bar.keyable(0.0));
    let plain = Bar { start: 1.0, duration: 8.0, cycle: None };
    assert_eq!(plain.cycle_frame(8.0), (8.0, 0.0));
    assert!(plain.keyable(8.0));
    assert!(!plain.keyable(9.0));
}
