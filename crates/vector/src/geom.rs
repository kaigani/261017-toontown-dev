//! Points, affine maps and cubic Bézier segments in drawing space (02 §2.1: du, +y up).
//!
//! Only IEEE-754 correctly rounded operations (`+ − × ÷ sqrt`) in a fixed order are used here; transcendental
//! functions come from [`crate::num`] (pure software), so results are identical on every platform (ADR-0004 rule 3).

use std::ops::{Add, Mul, Neg, Sub};

/// A point or vector in drawing units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct P {
    /// X (du).
    pub x: f64,
    /// Y (du, up).
    pub y: f64,
}

impl P {
    /// A point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Dot product.
    #[must_use]
    pub fn dot(self, o: P) -> f64 {
        self.x * o.x + self.y * o.y
    }

    /// 2-D cross product (z of the 3-D cross product).
    #[must_use]
    pub fn cross(self, o: P) -> f64 {
        self.x * o.y - self.y * o.x
    }

    /// Euclidean length.
    #[must_use]
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Distance to `o`.
    #[must_use]
    pub fn dist(self, o: P) -> f64 {
        (self - o).len()
    }

    /// The unit vector in this direction, or `None` for a zero vector.
    #[must_use]
    pub fn unit(self) -> Option<P> {
        let l = self.len();
        if l > 0.0 && l.is_finite() { Some(P::new(self.x / l, self.y / l)) } else { None }
    }

    /// Linear interpolation `self + (o − self)·t`.
    #[must_use]
    pub fn lerp(self, o: P, t: f64) -> P {
        P::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }

    /// Rotated by +90° (counter-clockwise in a y-up space).
    #[must_use]
    pub fn perp(self) -> P {
        P::new(-self.y, self.x)
    }

    /// Both coordinates finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        P::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        P::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for P {
    type Output = P;
    fn mul(self, k: f64) -> P {
        P::new(self.x * k, self.y * k)
    }
}

impl Neg for P {
    type Output = P;
    fn neg(self) -> P {
        P::new(-self.x, -self.y)
    }
}

/// An affine map `[a, b, c, d, e, f]`: (x, y) ↦ (a·x + c·y + e, b·x + d·y + f) (02 §13.4 convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    /// The identity.
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// A translation.
    #[must_use]
    pub const fn translate(dx: f64, dy: f64) -> Self {
        Affine([1.0, 0.0, 0.0, 1.0, dx, dy])
    }

    /// Applies the map to a point.
    #[must_use]
    pub fn apply(&self, p: P) -> P {
        let [a, b, c, d, e, f] = self.0;
        P::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
    }

    /// Applies the linear part to a vector (handles are offsets).
    #[must_use]
    pub fn apply_vec(&self, v: P) -> P {
        let [a, b, c, d, _, _] = self.0;
        P::new(a * v.x + c * v.y, b * v.x + d * v.y)
    }

    /// `self ∘ o` (apply `o` first).
    #[must_use]
    pub fn then_after(&self, o: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = o.0;
        Affine([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }

    /// Determinant of the linear part (negative for a reflection).
    #[must_use]
    pub fn det(&self) -> f64 {
        let [a, b, c, d, _, _] = self.0;
        a * d - b * c
    }
}

/// An axis-aligned rectangle `{x, y, width, height}` (02 §13.4 `bounds`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Minimum x.
    pub min_x: f64,
    /// Minimum y.
    pub min_y: f64,
    /// Maximum x.
    pub max_x: f64,
    /// Maximum y.
    pub max_y: f64,
}

impl Rect {
    /// The rectangle of one point.
    #[must_use]
    pub fn of(p: P) -> Self {
        Rect { min_x: p.x, min_y: p.y, max_x: p.x, max_y: p.y }
    }

    /// Extended to include `p`.
    #[must_use]
    pub fn with(self, p: P) -> Self {
        Rect {
            min_x: self.min_x.min(p.x),
            min_y: self.min_y.min(p.y),
            max_x: self.max_x.max(p.x),
            max_y: self.max_y.max(p.y),
        }
    }

    /// The union of two rectangles.
    #[must_use]
    pub fn union(self, o: Rect) -> Self {
        self.with(P::new(o.min_x, o.min_y)).with(P::new(o.max_x, o.max_y))
    }

    /// Whether the rectangles overlap or touch.
    #[must_use]
    pub fn touches(&self, o: &Rect) -> bool {
        self.min_x <= o.max_x && o.min_x <= self.max_x && self.min_y <= o.max_y && o.min_y <= self.max_y
    }

    /// Grown by `d` on every side.
    #[must_use]
    pub fn grown(self, d: f64) -> Self {
        Rect { min_x: self.min_x - d, min_y: self.min_y - d, max_x: self.max_x + d, max_y: self.max_y + d }
    }

    /// `{x, y, width, height}` as JSON.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "x": self.min_x, "y": self.min_y,
            "width": self.max_x - self.min_x, "height": self.max_y - self.min_y,
        })
    }
}

/// A cubic Bézier segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cubic {
    /// Start anchor.
    pub p0: P,
    /// First control point.
    pub p1: P,
    /// Second control point.
    pub p2: P,
    /// End anchor.
    pub p3: P,
}

/// Gauss–Legendre nodes and weights on [0, 1] (8 points), for arc lengths.
const GL8: [(f64, f64); 8] = [
    (0.019855071751231856, 0.05061426814518813),
    (0.10166676129318664, 0.11119051722668724),
    (0.2372337950418355, 0.15685332293894363),
    (0.4082826787521751, 0.181341891689181),
    (0.591717321247825, 0.181341891689181),
    (0.7627662049581645, 0.15685332293894363),
    (0.8983332387068134, 0.11119051722668724),
    (0.9801449282487681, 0.05061426814518813),
];

impl Cubic {
    /// A straight segment (control points at thirds).
    #[must_use]
    pub fn line(a: P, b: P) -> Self {
        Cubic { p0: a, p1: a.lerp(b, 1.0 / 3.0), p2: a.lerp(b, 2.0 / 3.0), p3: b }
    }

    /// The point at `t`.
    #[must_use]
    pub fn eval(&self, t: f64) -> P {
        let mt = 1.0 - t;
        let a = mt * mt * mt;
        let b = 3.0 * mt * mt * t;
        let c = 3.0 * mt * t * t;
        let d = t * t * t;
        P::new(
            a * self.p0.x + b * self.p1.x + c * self.p2.x + d * self.p3.x,
            a * self.p0.y + b * self.p1.y + c * self.p2.y + d * self.p3.y,
        )
    }

    /// The first derivative at `t`.
    #[must_use]
    pub fn deriv(&self, t: f64) -> P {
        let mt = 1.0 - t;
        let a = (self.p1 - self.p0) * (3.0 * mt * mt);
        let b = (self.p2 - self.p1) * (6.0 * mt * t);
        let c = (self.p3 - self.p2) * (3.0 * t * t);
        a + b + c
    }

    /// The second derivative at `t`.
    #[must_use]
    pub fn deriv2(&self, t: f64) -> P {
        let a = (self.p2 - self.p1 * 2.0 + self.p0) * (6.0 * (1.0 - t));
        let b = (self.p3 - self.p2 * 2.0 + self.p1) * (6.0 * t);
        a + b
    }

    /// The two halves at `t` (de Casteljau).
    #[must_use]
    pub fn split(&self, t: f64) -> (Cubic, Cubic) {
        let a = self.p0.lerp(self.p1, t);
        let b = self.p1.lerp(self.p2, t);
        let c = self.p2.lerp(self.p3, t);
        let d = a.lerp(b, t);
        let e = b.lerp(c, t);
        let m = d.lerp(e, t);
        (Cubic { p0: self.p0, p1: a, p2: d, p3: m }, Cubic { p0: m, p1: e, p2: c, p3: self.p3 })
    }

    /// Arc length from 0 to `t` (8-point Gauss–Legendre on 4 sub-intervals).
    #[must_use]
    pub fn length_to(&self, t: f64) -> f64 {
        let mut sum = 0.0;
        for k in 0..4 {
            let lo = t * f64::from(k) / 4.0;
            let hi = t * f64::from(k + 1) / 4.0;
            let h = hi - lo;
            for (x, w) in GL8 {
                sum += w * h * self.deriv(lo + h * x).len();
            }
        }
        sum
    }

    /// Arc length of the whole segment.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length_to(1.0)
    }

    /// The exact bounding box (extrema of the curve, not of the control polygon).
    #[must_use]
    pub fn bounds(&self) -> Rect {
        let mut r = Rect::of(self.p0).with(self.p3);
        for t in extrema(self.p0.x, self.p1.x, self.p2.x, self.p3.x)
            .into_iter()
            .chain(extrema(self.p0.y, self.p1.y, self.p2.y, self.p3.y))
            .flatten()
        {
            r = r.with(self.eval(t));
        }
        r
    }

    /// Maximum distance of the control points from the chord (a flatness bound).
    #[must_use]
    pub fn flatness(&self) -> f64 {
        let d = self.p3 - self.p0;
        let l = d.len();
        if l == 0.0 {
            return self.p0.dist(self.p1).max(self.p0.dist(self.p2));
        }
        let a = (self.p1 - self.p0).cross(d).abs() / l;
        let b = (self.p2 - self.p0).cross(d).abs() / l;
        a.max(b)
    }

    /// The segment transformed by `m`.
    #[must_use]
    pub fn transformed(&self, m: &Affine) -> Cubic {
        Cubic { p0: m.apply(self.p0), p1: m.apply(self.p1), p2: m.apply(self.p2), p3: m.apply(self.p3) }
    }
}

/// Parameters in (0, 1) where one coordinate of a cubic has a derivative root.
fn extrema(a: f64, b: f64, c: f64, d: f64) -> [Option<f64>; 2] {
    // Derivative / 3 = qa t² + qb t + qc.
    let qa = -a + 3.0 * b - 3.0 * c + d;
    let qb = 2.0 * (a - 2.0 * b + c);
    let qc = b - a;
    let inside = |t: f64| if t > 0.0 && t < 1.0 { Some(t) } else { None };
    if qa.abs() < 1e-12 {
        if qb.abs() < 1e-12 {
            return [None, None];
        }
        return [inside(-qc / qb), None];
    }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return [None, None];
    }
    let s = disc.sqrt();
    [inside((-qb + s) / (2.0 * qa)), inside((-qb - s) / (2.0 * qa))]
}

/// Flattens a cubic into points (excluding its start) so that every chord deviates by at most `tol`; the number of
/// pieces depends only on the geometry (fixed rule, no recursion order effects).
pub fn flatten_into(c: &Cubic, tol: f64, out: &mut Vec<P>) {
    // Wang's bound: n = ceil(sqrt(3·max|second difference| / (4·tol))).
    let dd1 = (c.p2 - c.p1 * 2.0 + c.p0).len();
    let dd2 = (c.p3 - c.p2 * 2.0 + c.p1).len();
    let m = dd1.max(dd2) * 6.0;
    let n = if m <= 0.0 { 1.0 } else { (m / (8.0 * tol)).sqrt().ceil().clamp(1.0, 512.0) };
    // `n` is a small positive integer stored in an f64; the loop counts with integers.
    let steps = n as u32;
    for i in 1..=steps {
        out.push(c.eval(f64::from(i) / n));
    }
}

/// Signed area of a closed polygon (positive = counter-clockwise in a y-up space).
#[must_use]
pub fn signed_area(poly: &[P]) -> f64 {
    let mut s = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        s += a.cross(b);
    }
    s / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_length_and_bounds() {
        let c = Cubic::line(P::new(0.0, 0.0), P::new(3.0, 4.0));
        assert!((c.length() - 5.0).abs() < 1e-12);
        let b = c.bounds();
        assert_eq!((b.min_x, b.max_y), (0.0, 4.0));
    }

    #[test]
    fn bounds_cover_bulging_curves() {
        let c = Cubic { p0: P::new(0.0, 0.0), p1: P::new(0.0, 10.0), p2: P::new(10.0, 10.0), p3: P::new(10.0, 0.0) };
        let b = c.bounds();
        assert!((b.max_y - 7.5).abs() < 1e-12, "{b:?}");
    }

    #[test]
    fn affine_composition() {
        let t = Affine::translate(1.0, 2.0);
        let s = Affine([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
        let ts = t.then_after(&s);
        assert_eq!(ts.apply(P::new(1.0, 1.0)), P::new(3.0, 4.0));
    }
}
