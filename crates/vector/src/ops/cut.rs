//! Geometry of the Cutter and Eraser (02 §7.2.7, §7.2.2; M2 C2.3): the invisible boundary stroke a cut stages so the
//! settle pass splits every crossing line and zone exactly where the boundary runs, and the inside test that sorts the
//! resulting pieces.

use serde_json::{Value, json};

use crate::geom::P;
use crate::path::{Anchor, Path};

/// Whether `q` is inside the closed polygon `poly` (non-zero winding).
#[must_use]
pub fn inside(poly: &[P], q: P) -> bool {
    let n = poly.len();
    let mut w = 0i32;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let o = (b - a).cross(q - a);
        if a.y <= q.y {
            if b.y > q.y && o > 0.0 {
                w += 1;
            }
        } else if b.y <= q.y && o < 0.0 {
            w -= 1;
        }
    }
    w != 0
}

/// Whether two closed polygons overlap (an edge crossing, or one containing a vertex of the other).
#[must_use]
pub fn polygons_meet(a: &[P], b: &[P]) -> bool {
    if a.iter().any(|p| inside(b, *p)) || b.iter().any(|p| inside(a, *p)) {
        return true;
    }
    let edges = |v: &[P]| -> Vec<(P, P)> { (0..v.len()).map(|i| (v[i], v[(i + 1) % v.len()])).collect() };
    let (ea, eb) = (edges(a), edges(b));
    ea.iter().any(|x| eb.iter().any(|y| segments_cross(x.0, x.1, y.0, y.1)))
}

fn segments_cross(p: P, q: P, r: P, s: P) -> bool {
    let d1 = (q - p).cross(r - p);
    let d2 = (q - p).cross(s - p);
    let d3 = (s - r).cross(p - r);
    let d4 = (s - r).cross(q - r);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Whether an open polyline meets a closed polygon (a point inside, or an edge crossing).
#[must_use]
pub fn polyline_meets(line: &[P], poly: &[P]) -> bool {
    if line.iter().any(|p| inside(poly, *p)) {
        return true;
    }
    let n = poly.len();
    line.windows(2).any(|w| (0..n).any(|i| segments_cross(w[0], w[1], poly[i], poly[(i + 1) % n])))
}

/// Whether two open polylines cross.
#[must_use]
pub fn polylines_cross(a: &[P], b: &[P]) -> bool {
    a.windows(2).any(|x| b.windows(2).any(|y| segments_cross(x[0], x[1], y[0], y[1])))
}

/// An invisible stroke (10 §4.9) through `pts` with corner anchors; `closed` for a polygon.
#[must_use]
pub fn boundary_stroke(id: &str, pts: &[P], closed: bool) -> Value {
    let path = Path { anchors: pts.iter().map(|p| Anchor::corner(*p)).collect(), closed };
    json!({
        "id": id, "type": "stroke", "rev": 0, "kind": "invisible", "closed": closed, "points": path.points_json(),
        "colourId": null, "fillMatrix": null, "lineStyle": null, "spine": null,
        "sides": { "left": null, "right": null },
    })
}

/// A point in the middle of a stored stroke (the middle of its middle segment): after noding against a boundary no
/// piece crosses it, so this point decides the piece's side.
#[must_use]
pub fn middle(points: &[[f64; 6]], closed: bool) -> Option<P> {
    let path = Path::from_points(points, closed);
    let segs = path.segments();
    segs.get(segs.len() / 2).map(|c| c.eval(0.5)).or_else(|| path.anchors.first().map(|a| a.p))
}

/// The anchors of a stored point list.
#[must_use]
pub fn anchors_of(points: &[[f64; 6]]) -> Vec<P> {
    points.iter().map(|a| P::new(a[0], a[1])).collect()
}

/// Removes repeated consecutive points (and a closing repeat); `None` when fewer than `min` distinct points remain.
#[must_use]
pub fn distinct(pts: &[P], min: usize) -> Option<Vec<P>> {
    let mut out: Vec<P> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last() != Some(p) {
            out.push(*p);
        }
    }
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    (out.len() >= min).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winding_inside_and_the_boundary_is_a_closed_invisible_stroke() {
        let sq = [P::new(0.0, 0.0), P::new(10.0, 0.0), P::new(10.0, 10.0), P::new(0.0, 10.0)];
        assert!(inside(&sq, P::new(5.0, 5.0)));
        assert!(!inside(&sq, P::new(15.0, 5.0)));
        let bar = [P::new(-5.0, 4.0), P::new(15.0, 4.0), P::new(15.0, 6.0), P::new(-5.0, 6.0)];
        assert!(polygons_meet(&sq, &bar), "a bar across the square meets it");
        let far = [P::new(20.0, 0.0), P::new(30.0, 0.0), P::new(30.0, 10.0)];
        assert!(!polygons_meet(&sq, &far));
        let s = boundary_stroke("st_x", &sq, true);
        assert_eq!((s["kind"].as_str(), s["closed"].as_bool()), (Some("invisible"), Some(true)));
        assert_eq!(s["points"].as_array().map(Vec::len), Some(4));
        assert_eq!(distinct(&[P::new(0.0, 0.0), P::new(0.0, 0.0), P::new(1.0, 0.0)], 2).map(|v| v.len()), Some(2));
        assert!(distinct(&[P::new(0.0, 0.0), P::new(0.0, 0.0)], 2).is_none());
        let m = middle(&[[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], [10.0, 0.0, 0.0, 0.0, 0.0, 0.0]], false);
        assert_eq!(m, Some(P::new(5.0, 0.0)));
    }
}
