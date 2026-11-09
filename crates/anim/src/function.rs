//! The 1-D function model (05 §10.3) and the easing presets (§10.4). Evaluation order is fixed (ADR-0019): locate the
//! segment, take a key's own value at its frame, otherwise evaluate the segment's shape; the two straight-line forms
//! (zero handles, the `linear` preset) are computed directly, so exact inputs give exact outputs.

use tt_model::entities::channel::{CurveType, Easing, Key};

/// Bisection steps of the cubic time solve: after 52 halvings the parameter is known to one unit in the last place of
/// a value near 1, well inside the spec's 1e-9 (ADR-0019).
pub const BEZIER_MAX_ITERATIONS: u32 = 52;

/// A key as the evaluator reads it (frame real for the velocity pins it stands in for).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyView {
    /// Local frame.
    pub frame: f64,
    /// Value.
    pub value: f64,
    /// The segment starting here holds the value.
    pub constant: bool,
    /// Left handle `(dt, dv)`.
    pub left: (f64, f64),
    /// Right handle `(dt, dv)`.
    pub right: (f64, f64),
    /// Easing preset of the segment starting here.
    pub easing: Option<Easing>,
}

impl KeyView {
    /// A key with no handles or preset.
    #[must_use]
    pub fn plain(frame: f64, value: f64) -> Self {
        Self { frame, value, constant: false, left: (0.0, 0.0), right: (0.0, 0.0), easing: None }
    }
}

impl From<&Key> for KeyView {
    fn from(k: &Key) -> Self {
        let h = k.handles();
        Self {
            frame: f64::from(*k.frame()),
            value: *k.value(),
            constant: *k.constant_segment(),
            left: (h.left.dt, h.left.dv),
            right: (h.right.dt, h.right.dv),
            easing: *k.easing(),
        }
    }
}

/// P1 and P2 of a preset in normalised segment space (05 §10.4, normative); `None` for `constant`.
#[must_use]
pub fn preset_points(e: Easing) -> Option<((f64, f64), (f64, f64))> {
    const THIRD: f64 = 1.0 / 3.0;
    const TWO_THIRDS: f64 = 2.0 / 3.0;
    Some(match e {
        Easing::Constant => return None,
        Easing::Linear => ((THIRD, THIRD), (TWO_THIRDS, TWO_THIRDS)),
        Easing::EaseInOut => ((THIRD, 0.0), (TWO_THIRDS, 1.0)),
        Easing::FastInOut => ((0.0, 0.5), (1.0, 0.5)),
        Easing::ExtremeEaseInOut => ((0.5, 0.0), (0.5, 1.0)),
        Easing::ExtremeFastInOut => ((0.0, 1.0), (1.0, 0.0)),
        Easing::SlowAcceleration => ((THIRD, 0.0), (1.0, 1.0)),
        Easing::SlowDeceleration => ((0.0, 0.0), (TWO_THIRDS, 1.0)),
    })
}

/// A cubic Bernstein form with end values `p0`, `p3`.
fn bernstein(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let v = 1.0 - u;
    v * v * v * p0 + 3.0 * v * v * u * p1 + 3.0 * v * u * u * p2 + u * u * u * p3
}

/// Solves `x(u) = target` for `x0 = 0`, `x3 = t`, with `0 ≤ x1, x2 ≤ t` (monotonic, 05 §10.3).
fn solve_time(x1: f64, x2: f64, t: f64, target: f64) -> f64 {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..BEZIER_MAX_ITERATIONS {
        let mid = (lo + hi) / 2.0;
        let x = bernstein(0.0, x1, x2, t, mid);
        if x == target {
            return mid;
        }
        if x < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// The straight line between two keys.
fn line(a: &KeyView, b: &KeyView, f: f64) -> f64 {
    a.value + (b.value - a.value) * ((f - a.frame) / (b.frame - a.frame))
}

/// The value of the segment `a → b` at `f`, strictly between their frames. `progress` is the node's `V(f)` for a
/// `velocityBased` curve (none without a motion path: the time fraction applies, as for `V(t1) = V(t0)`).
#[must_use]
pub fn segment_value(curve: CurveType, a: &KeyView, b: &KeyView, f: f64, progress: Option<&dyn Fn(f64) -> f64>) -> f64 {
    if a.constant {
        return a.value;
    }
    let t = b.frame - a.frame;
    match curve {
        CurveType::Spline => {
            let (rt, rv) = a.right;
            let (lt, lv) = b.left;
            if rt == 0.0 && rv == 0.0 && lt == 0.0 && lv == 0.0 {
                return line(a, b, f);
            }
            // Handle frame offsets are clamped (05 §10.3), which keeps the curve a function of time.
            let x1 = rt.clamp(0.0, t);
            let x2 = t + lt.clamp(-t, 0.0);
            let u = solve_time(x1, x2, t, f - a.frame);
            bernstein(a.value, a.value + rv, b.value + lv, b.value, u)
        }
        CurveType::Interpolation => match a.easing.unwrap_or(Easing::Linear) {
            Easing::Linear => line(a, b, f),
            Easing::Constant => a.value,
            e => {
                let Some(((x1, y1), (x2, y2))) = preset_points(e) else { return a.value };
                let u = solve_time(x1, x2, 1.0, (f - a.frame) / t);
                a.value + (b.value - a.value) * bernstein(0.0, y1, y2, 1.0, u)
            }
        },
        CurveType::VelocityBased => {
            let u = match progress {
                Some(v) => {
                    let (v0, v1) = (v(a.frame), v(b.frame));
                    if v1 == v0 { (f - a.frame) / t } else { (v(f) - v0) / (v1 - v0) }
                }
                None => (f - a.frame) / t,
            };
            a.value + (b.value - a.value) * u
        }
    }
}

/// The Bézier control points `(x, y)` of the segment `a → b` in frames and values, for the curve shapes that are
/// cubics in their parameter (a spline with handles, an interpolation preset other than `linear`); `None` for the shapes
/// that are straight or flat (zero handles, `linear`, `constant`, a constant segment) and for `velocityBased`. The handle
/// frame offsets are clamped exactly as [`segment_value`] clamps them, so the two describe the same curve.
#[must_use]
pub fn segment_bezier(curve: CurveType, a: &KeyView, b: &KeyView) -> Option<([f64; 4], [f64; 4])> {
    if a.constant {
        return None;
    }
    let t = b.frame - a.frame;
    match curve {
        CurveType::Spline => {
            let (rt, rv) = a.right;
            let (lt, lv) = b.left;
            if rt == 0.0 && rv == 0.0 && lt == 0.0 && lv == 0.0 {
                return None;
            }
            let x = [a.frame, a.frame + rt.clamp(0.0, t), b.frame + lt.clamp(-t, 0.0), b.frame];
            Some((x, [a.value, a.value + rv, b.value + lv, b.value]))
        }
        CurveType::Interpolation => {
            let e = a.easing.unwrap_or(Easing::Linear);
            if e == Easing::Linear {
                return None;
            }
            let ((x1, y1), (x2, y2)) = preset_points(e)?;
            let d = b.value - a.value;
            Some((
                [a.frame, a.frame + x1 * t, a.frame + x2 * t, b.frame],
                [a.value, a.value + y1 * d, a.value + y2 * d, b.value],
            ))
        }
        CurveType::VelocityBased => None,
    }
}

/// The frames strictly inside a Bézier segment where its value has a local extremum: the roots in (0, 1) of the
/// derivative of the value's cubic, a quadratic solved in closed form, mapped to frames through the time cubic. Between
/// two consecutive frames of `[start, extrema…, end]` the value is monotone.
#[must_use]
pub fn extremum_frames(x: [f64; 4], y: [f64; 4]) -> Vec<f64> {
    // y′(s) is proportional to (y1 − y0)(1 − s)² + 2(y2 − y1)(1 − s)s + (y3 − y2)s² = A s² + B s + C.
    let (p, q, r) = (y[1] - y[0], y[2] - y[1], y[3] - y[2]);
    let (qa, qb, qc) = (p - 2.0 * q + r, 2.0 * (q - p), p);
    // The cancellation-free form: h = −(B + sign(B)·√(B² − 4AC)) / 2, roots h / A and C / h. A near-zero A (a cubic
    // that is almost a quadratic) then gives one huge root, filtered out, and an accurate one.
    let mut roots = Vec::with_capacity(2);
    let disc = qb * qb - 4.0 * qa * qc;
    if disc >= 0.0 {
        let sq = disc.sqrt();
        let h = -(qb + if qb < 0.0 { -sq } else { sq }) / 2.0;
        if qa != 0.0 {
            roots.push(h / qa);
        }
        if h != 0.0 {
            roots.push(qc / h);
        }
    }
    let mut out: Vec<f64> = roots
        .into_iter()
        .filter(|s| *s > 0.0 && *s < 1.0)
        .map(|s| bernstein(x[0], x[1], x[2], x[3], s))
        .filter(|f| *f > x[0] && *f < x[3])
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup();
    out
}

/// The value of a key list at local frame `f`: held before the first and after the last key (05 §2.10), a key's own
/// value at its frame. `None` for an empty list.
#[must_use]
pub fn evaluate(curve: CurveType, keys: &[KeyView], f: f64, progress: Option<&dyn Fn(f64) -> f64>) -> Option<f64> {
    let (first, last) = (keys.first()?, keys.last()?);
    if f <= first.frame {
        return Some(first.value);
    }
    if f >= last.frame {
        return Some(last.value);
    }
    let i = keys.partition_point(|k| k.frame <= f);
    let (a, b) = (keys.get(i.checked_sub(1)?)?, keys.get(i)?);
    if a.frame == f {
        return Some(a.value);
    }
    Some(segment_value(curve, a, b, f, progress))
}
