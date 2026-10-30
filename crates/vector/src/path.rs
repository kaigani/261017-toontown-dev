//! Bézier paths in the two forms the spec uses, and their validation.
//!
//! - **Persisted anchors** (10 §4.9): `[x, y, inX, inY, outX, outY]` per anchor, handles as offsets from the anchor.
//! - **Bézier lists** (02 §13.4): flat control points; an open path of n segments has 3n + 1 points, a closed one 3n
//!   (its last segment returns to point 0); anchor k is point 3k.

use serde_json::{Value, json};

use crate::geom::{Cubic, P, Rect};

/// The largest coordinate magnitude accepted in drawing space (du). Brush outlines are computed on an integer grid of
/// 1/64 du whose range this keeps inside (`brush`), and 10⁷ du is 100 000 fields from the centre.
pub const MAX_COORD: f64 = 1.0e7;

/// The most anchors (or input samples) one stroke may have (02 §12.2 `drawing.invalidGeometry` beyond).
pub const MAX_ANCHORS: usize = 100_000;

/// A geometry error: `drawing.invalidGeometry` with the offending parameter pointer (02 §12.2).
#[derive(Debug, Clone, PartialEq)]
pub struct GeomError {
    /// Error code.
    pub code: &'static str,
    /// Message.
    pub message: String,
    /// JSON Pointer of the offending input.
    pub pointer: String,
}

impl GeomError {
    /// `drawing.invalidGeometry` at `pointer`.
    #[must_use]
    pub fn invalid(pointer: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: "drawing.invalidGeometry", message: message.into(), pointer: pointer.into() }
    }
}

impl std::fmt::Display for GeomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}: {}", self.code, self.pointer, self.message)
    }
}

/// Checks that a coordinate is finite and inside [`MAX_COORD`].
///
/// # Errors
/// `drawing.invalidGeometry`.
pub fn check_coord(v: f64, pointer: &str) -> Result<(), GeomError> {
    if !v.is_finite() || v.abs() > MAX_COORD {
        return Err(GeomError::invalid(pointer, format!("coordinates are finite and within ±{MAX_COORD} du")));
    }
    Ok(())
}

/// One anchor with its handle offsets.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Anchor {
    /// Anchor position.
    pub p: P,
    /// Incoming handle offset.
    pub inh: P,
    /// Outgoing handle offset.
    pub outh: P,
}

impl Anchor {
    /// An anchor without handles.
    #[must_use]
    pub fn corner(p: P) -> Self {
        Self { p, inh: P::default(), outh: P::default() }
    }

    /// The persisted `[x, y, inX, inY, outX, outY]`.
    #[must_use]
    pub fn to_array(&self) -> [f64; 6] {
        [self.p.x, self.p.y, self.inh.x, self.inh.y, self.outh.x, self.outh.y]
    }

    /// From the persisted form.
    #[must_use]
    pub fn from_array(a: &[f64; 6]) -> Self {
        Self { p: P::new(a[0], a[1]), inh: P::new(a[2], a[3]), outh: P::new(a[4], a[5]) }
    }
}

/// A path: anchors and whether it is closed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    /// Anchors in order.
    pub anchors: Vec<Anchor>,
    /// Closed (the last segment returns to the first anchor).
    pub closed: bool,
}

impl Path {
    /// From persisted anchors.
    #[must_use]
    pub fn from_points(points: &[[f64; 6]], closed: bool) -> Self {
        Self { anchors: points.iter().map(Anchor::from_array).collect(), closed }
    }

    /// The persisted anchors.
    #[must_use]
    pub fn to_points(&self) -> Vec<[f64; 6]> {
        self.anchors.iter().map(Anchor::to_array).collect()
    }

    /// The persisted anchors as JSON.
    #[must_use]
    pub fn points_json(&self) -> Value {
        Value::Array(self.anchors.iter().map(|a| json!(a.to_array())).collect())
    }

    /// The segments (n − 1 for an open path, n for a closed one; none for a single open anchor).
    #[must_use]
    pub fn segments(&self) -> Vec<Cubic> {
        let n = self.anchors.len();
        // A single anchor is a dab (no segment), unless it is a closed loop with handles.
        let count = match n {
            0 => 0,
            1 => usize::from(
                self.closed && (self.anchors[0].inh != P::default() || self.anchors[0].outh != P::default()),
            ),
            _ if self.closed => n,
            _ => n - 1,
        };
        (0..count)
            .map(|i| {
                let a = self.anchors[i];
                let b = self.anchors[(i + 1) % n];
                Cubic { p0: a.p, p1: a.p + a.outh, p2: b.p + b.inh, p3: b.p }
            })
            .collect()
    }

    /// The 02 §13.4 Bézier list.
    #[must_use]
    pub fn to_bezier_list(&self) -> Vec<P> {
        let segs = self.segments();
        if segs.is_empty() {
            return self.anchors.iter().map(|a| a.p).collect();
        }
        let mut out = Vec::with_capacity(segs.len() * 3 + 1);
        for s in &segs {
            out.push(s.p0);
            out.push(s.p1);
            out.push(s.p2);
        }
        if !self.closed {
            out.push(segs[segs.len() - 1].p3);
        }
        out
    }

    /// The 02 §13.4 Bézier list as JSON points.
    #[must_use]
    pub fn bezier_json(&self) -> Value {
        Value::Array(self.to_bezier_list().into_iter().map(|p| json!({ "x": p.x, "y": p.y })).collect())
    }

    /// From a 02 §13.4 Bézier list (validated: counts, finite coordinates; `pointer` names the list parameter).
    ///
    /// # Errors
    /// `drawing.invalidGeometry`.
    pub fn from_bezier_list(pts: &[P], closed: bool, pointer: &str) -> Result<Self, GeomError> {
        for (i, p) in pts.iter().enumerate() {
            check_coord(p.x, &format!("{pointer}/{i}/x"))?;
            check_coord(p.y, &format!("{pointer}/{i}/y"))?;
        }
        let n = pts.len();
        let ok = if closed { n == 1 || (n >= 3 && n.is_multiple_of(3)) } else { n >= 4 && n % 3 == 1 };
        if !ok {
            return Err(GeomError::invalid(
                pointer,
                if closed {
                    "a closed Bézier list has 3n points (n ≥ 1 segments), or one point for a dab".to_owned()
                } else {
                    "an open Bézier list has 3n + 1 points (n ≥ 1 segments)".to_owned()
                },
            ));
        }
        if n / 3 + 1 > MAX_ANCHORS {
            return Err(GeomError::invalid(pointer, format!("at most {MAX_ANCHORS} anchors per stroke")));
        }
        if n == 1 {
            return Ok(Path { anchors: vec![Anchor::corner(pts[0])], closed: true });
        }
        let count = if closed { n / 3 } else { (n - 1) / 3 + 1 };
        let mut anchors = vec![Anchor::default(); count];
        for (k, a) in anchors.iter_mut().enumerate() {
            a.p = pts[(3 * k) % n];
        }
        let segs = if closed { n / 3 } else { (n - 1) / 3 };
        for s in 0..segs {
            let start = 3 * s;
            let next = (s + 1) % count;
            anchors[s].outh = pts[start + 1] - pts[start];
            anchors[next].inh = pts[start + 2] - pts[(start + 3) % n];
        }
        Ok(Path { anchors, closed })
    }

    /// The exact bounds of the path (`None` when it has no anchors).
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        let first = self.anchors.first()?;
        let mut r = Rect::of(first.p);
        for s in self.segments() {
            r = r.union(s.bounds());
        }
        Some(r)
    }

    /// The arc length.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.segments().iter().map(Cubic::length).sum()
    }

    /// Checks the path: anchor counts (02 §12.2) and finite, bounded coordinates.
    ///
    /// # Errors
    /// `drawing.invalidGeometry`.
    pub fn check(&self, pointer: &str) -> Result<(), GeomError> {
        let min = if self.closed { 1 } else { 2 };
        if self.anchors.len() < min {
            return Err(GeomError::invalid(
                pointer,
                format!(
                    "a{} path has at least {min} anchor{}",
                    if self.closed { " closed" } else { "n open" },
                    if min > 1 { "s" } else { "" }
                ),
            ));
        }
        if self.anchors.len() > MAX_ANCHORS {
            return Err(GeomError::invalid(pointer, format!("at most {MAX_ANCHORS} anchors per stroke")));
        }
        for (i, a) in self.anchors.iter().enumerate() {
            for (j, v) in a.to_array().iter().enumerate() {
                check_coord(*v, &format!("{pointer}/{i}/{j}"))?;
            }
            // Handles are offsets: the control point itself must also be in range.
            check_coord(a.p.x + a.inh.x, &format!("{pointer}/{i}/2"))?;
            check_coord(a.p.y + a.inh.y, &format!("{pointer}/{i}/3"))?;
            check_coord(a.p.x + a.outh.x, &format!("{pointer}/{i}/4"))?;
            check_coord(a.p.y + a.outh.y, &format!("{pointer}/{i}/5"))?;
        }
        Ok(())
    }

    /// The path with every point mapped by `m` (handles by its linear part).
    #[must_use]
    pub fn transformed(&self, m: &crate::geom::Affine) -> Path {
        Path {
            anchors: self
                .anchors
                .iter()
                .map(|a| Anchor { p: m.apply(a.p), inh: m.apply_vec(a.inh), outh: m.apply_vec(a.outh) })
                .collect(),
            closed: self.closed,
        }
    }

    /// The polyline of the path (start point included), each chord within `tol` of the curve.
    #[must_use]
    pub fn flatten(&self, tol: f64) -> Vec<P> {
        let mut out = Vec::new();
        if let Some(a) = self.anchors.first() {
            out.push(a.p);
        }
        for s in self.segments() {
            crate::geom::flatten_into(&s, tol, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_lists_round_trip_open_and_closed() {
        let open = [P::new(0.0, 0.0), P::new(10.0, 5.0), P::new(20.0, 5.0), P::new(30.0, 0.0)];
        let p = Path::from_bezier_list(&open, false, "/bezier").unwrap();
        assert_eq!(p.anchors.len(), 2);
        assert_eq!(p.to_points()[0], [0.0, 0.0, 0.0, 0.0, 10.0, 5.0]);
        assert_eq!(p.to_points()[1], [30.0, 0.0, -10.0, 5.0, 0.0, 0.0]);
        assert_eq!(p.to_bezier_list(), open);
        let closed = [
            P::new(0.0, 0.0),
            P::new(1.0, 0.0),
            P::new(2.0, 0.0),
            P::new(3.0, 0.0),
            P::new(3.0, 1.0),
            P::new(3.0, 2.0),
        ];
        let c = Path::from_bezier_list(&closed, true, "/bezier").unwrap();
        assert_eq!(c.anchors.len(), 2);
        assert_eq!(c.to_bezier_list(), closed);
    }

    #[test]
    fn malformed_lists_are_invalid_geometry() {
        let e = Path::from_bezier_list(&[P::new(0.0, 0.0), P::new(1.0, 1.0)], false, "/bezier").unwrap_err();
        assert_eq!(e.code, "drawing.invalidGeometry");
        let e = Path::from_bezier_list(&[P::new(f64::NAN, 0.0)], true, "/bezier").unwrap_err();
        assert_eq!(e.pointer, "/bezier/0/x");
        assert!(Path::from_bezier_list(&[P::new(2e7, 0.0)], true, "/b").is_err());
    }
}
