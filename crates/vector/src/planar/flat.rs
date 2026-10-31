//! Flattening of stroke paths for the planar map: polylines whose vertices carry their path parameter
//! `s = segment index + t`, so a point found on the polyline maps back to the Bézier (persistent splits) and a stretch of
//! a brush outline has a stable key (SI-060).

use crate::geom::{Cubic, P};
use crate::path::{GeomError, Path};
use crate::work::Work;

/// Flattening tolerance (du): half a grid step of the planar map.
pub const FLAT_TOLERANCE: f64 = 1.0 / 128.0;

/// Most polyline pieces per Bézier segment.
const MAX_PIECES: f64 = 4096.0;

/// A flattened path.
#[derive(Debug, Clone, PartialEq)]
pub struct Flat {
    /// Vertices (a closed path does not repeat its first vertex).
    pub pts: Vec<P>,
    /// Path parameter of each vertex.
    pub params: Vec<f64>,
    /// Whether the polyline closes back to its first vertex.
    pub closed: bool,
}

impl Flat {
    /// The polyline's edges as `(a, b, s_a, s_b)`; a closed polyline's last edge returns to the first vertex at the
    /// parameter of the path's end.
    #[must_use]
    pub fn edges(&self, segment_count: usize) -> Vec<(P, P, f64, f64)> {
        let n = self.pts.len();
        let mut out = Vec::with_capacity(n);
        for i in 0..n.saturating_sub(1) {
            out.push((self.pts[i], self.pts[i + 1], self.params[i], self.params[i + 1]));
        }
        if self.closed && n >= 2 {
            out.push((self.pts[n - 1], self.pts[0], self.params[n - 1], segment_count as f64));
        }
        out
    }
}

/// Distance from `p` to the segment `a`–`b`.
#[must_use]
pub fn segment_distance(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

/// The number of uniform parameter steps that keep `c` within `tol` of its chords (Wang's bound); 1 for a segment
/// whose control points lie on its chord.
#[must_use]
pub fn pieces(c: &Cubic, tol: f64) -> u32 {
    if segment_distance(c.p1, c.p0, c.p3) <= tol / 8.0 && segment_distance(c.p2, c.p0, c.p3) <= tol / 8.0 {
        return 1;
    }
    let dd1 = (c.p2 - c.p1 * 2.0 + c.p0).len();
    let dd2 = (c.p3 - c.p2 * 2.0 + c.p1).len();
    let m = dd1.max(dd2) * 6.0;
    let n = if m <= 0.0 { 1.0 } else { (m / (8.0 * tol)).sqrt().ceil().clamp(1.0, MAX_PIECES) };
    n as u32
}

/// Flattens `path` within [`FLAT_TOLERANCE`]; consecutive equal vertices are dropped.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn flatten(path: &Path, work: &mut Work) -> Result<Flat, GeomError> {
    let segs = path.segments();
    let mut pts: Vec<P> = Vec::new();
    let mut params: Vec<f64> = Vec::new();
    if segs.is_empty() {
        if let Some(a) = path.anchors.first() {
            pts.push(a.p);
            params.push(0.0);
        }
        return Ok(Flat { pts, params, closed: false });
    }
    let push = |pts: &mut Vec<P>, params: &mut Vec<f64>, p: P, s: f64| {
        if pts.last() != Some(&p) {
            pts.push(p);
            params.push(s);
        }
    };
    let last = segs.len() - 1;
    for (j, c) in segs.iter().enumerate() {
        let n = pieces(c, FLAT_TOLERANCE);
        work.spend(u64::from(n))?;
        let base = j as f64;
        for i in 0..n {
            let t = f64::from(i) / f64::from(n);
            let p = if i == 0 { c.p0 } else { c.eval(t) };
            push(&mut pts, &mut params, p, base + t);
        }
        if j == last && !path.closed {
            push(&mut pts, &mut params, c.p3, base + 1.0);
        }
    }
    if path.closed && pts.len() >= 2 && pts.last() == pts.first() {
        pts.pop();
        params.pop();
    }
    let closed = path.closed && pts.len() >= 2;
    Ok(Flat { pts, params, closed })
}

/// The Bézier parameter of a point at fraction `f` along the polyline edge from `s_a` to `s_b` (linear within an
/// edge, which spans one uniform parameter step of one segment).
#[must_use]
pub fn param_between(s_a: f64, s_b: f64, f: f64) -> f64 {
    s_a + (s_b - s_a) * f.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Anchor;

    #[test]
    fn a_straight_segment_is_one_edge_and_a_curve_is_within_tolerance() {
        let line =
            Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(10.0, 0.0))], closed: false };
        let f = flatten(&line, &mut Work::unlimited("t")).unwrap_or_else(|_| Flat {
            pts: vec![],
            params: vec![],
            closed: false,
        });
        assert_eq!(f.pts.len(), 2);
        assert_eq!(f.params, vec![0.0, 1.0]);
        let arc = Path {
            anchors: vec![
                Anchor { p: P::new(0.0, 0.0), inh: P::default(), outh: P::new(0.0, 55.0) },
                Anchor { p: P::new(100.0, 100.0), inh: P::new(-55.0, 0.0), outh: P::default() },
            ],
            closed: false,
        };
        let f = flatten(&arc, &mut Work::unlimited("t")).unwrap_or_else(|_| Flat {
            pts: vec![],
            params: vec![],
            closed: false,
        });
        let c = arc.segments()[0];
        for w in f.params.windows(2) {
            let (a, b) = (c.eval(w[0]), c.eval(w[1]));
            for k in 1..8 {
                let t = w[0] + (w[1] - w[0]) * f64::from(k) / 8.0;
                assert!(segment_distance(c.eval(t), a, b) <= FLAT_TOLERANCE * 1.01);
            }
        }
    }

    #[test]
    fn a_closed_path_does_not_repeat_its_start() {
        let sq = Path {
            anchors: [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
                .iter()
                .map(|&(x, y)| Anchor::corner(P::new(x, y)))
                .collect(),
            closed: true,
        };
        let f = flatten(&sq, &mut Work::unlimited("t")).unwrap_or_else(|_| Flat {
            pts: vec![],
            params: vec![],
            closed: false,
        });
        assert_eq!(f.pts.len(), 4);
        assert!(f.closed);
        assert_eq!(f.edges(4).last().map(|e| e.3), Some(4.0));
    }
}
