//! The exact geometric predicates of the drawing engine (ADR-0017 §1; C23-R1-08): orientation, segment crossing and
//! containment, decided in exact i128 arithmetic on the snap-rounded grid (1/64 du). The planar map's noding, faces and
//! fills, and the drawing operations that classify geometry against it (the Cutter and Eraser, Optimize's coverage
//! tests, region selection), all decide through this one module, so a crossing the map nodes is a crossing the
//! operations see, and a determinant of ±1 on the grid is never lost to floating-point rounding.

use super::graph::{Ip, snap, winding};
use crate::geom::P;

/// Twice the signed area of the triangle `a b c` (grid units): positive counter-clockwise, zero collinear.
#[must_use]
pub fn orient(a: Ip, b: Ip, c: Ip) -> i128 {
    let (ax, ay) = (i128::from(a[0]), i128::from(a[1]));
    (i128::from(b[0]) - ax) * (i128::from(c[1]) - ay) - (i128::from(b[1]) - ay) * (i128::from(c[0]) - ax)
}

/// Whether the closed grid segments `p q` and `r s` cross at a single interior point of both (a proper crossing;
/// touching, endpoint contact and collinear overlap are not crossings).
#[must_use]
pub fn proper_crossing(p: Ip, q: Ip, r: Ip, s: Ip) -> bool {
    let (d1, d2) = (orient(p, q, r).signum(), orient(p, q, s).signum());
    let (d3, d4) = (orient(r, s, p).signum(), orient(r, s, q).signum());
    d1 * d2 < 0 && d3 * d4 < 0
}

/// Whether the segments `p q` and `r s` (drawing units, snapped) cross properly.
#[must_use]
pub fn segments_cross(p: P, q: P, r: P, s: P) -> bool {
    proper_crossing(snap(p), snap(q), snap(r), snap(s))
}

/// Whether `q` lies inside the closed polygon `poly` (drawing units, snapped): non-zero winding; a point on the
/// boundary counts as inside when `boundary_inside`.
#[must_use]
pub fn inside_polygon(poly: &[P], q: P, boundary_inside: bool) -> bool {
    let ring: Vec<Ip> = poly.iter().map(|p| snap(*p)).collect();
    let (w, on) = winding(&ring, snap(q), 1);
    if on { boundary_inside } else { w != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_determinant_on_the_grid_is_exact() {
        let n = 500_000_000i64;
        let (a, b, c) = (n, n - 1, n - 2);
        // orient((-a,-b),(a,b),(b,c)) has determinant 2(a c - b b) = -2: exact here, zero in f64.
        assert_eq!(orient([-a, -b], [a, b], [b, c]), -2);
        assert!(proper_crossing([-a, -b], [a, b], [-b, -c], [b, c]));
        // Endpoint contact and collinear overlap are not proper crossings.
        assert!(!proper_crossing([0, 0], [10, 0], [10, 0], [10, 10]));
        assert!(!proper_crossing([0, 0], [10, 0], [5, 0], [15, 0]));
        let sq = [P::new(0.0, 0.0), P::new(10.0, 0.0), P::new(10.0, 10.0), P::new(0.0, 10.0)];
        assert!(inside_polygon(&sq, P::new(5.0, 5.0), false));
        assert!(!inside_polygon(&sq, P::new(10.0, 5.0), false));
        assert!(inside_polygon(&sq, P::new(10.0, 5.0), true));
        assert!(!inside_polygon(&sq, P::new(15.0, 5.0), true));
    }
}
