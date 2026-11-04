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
