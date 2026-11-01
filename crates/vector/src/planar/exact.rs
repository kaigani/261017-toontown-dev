//! Exact areas of intersection of grid polygons (C22-R2-04; ADR-0017 §4): carry-over ranks old zones by the exact area
//! they share with a new zone, so unequal overlaps never become a tie.
//!
//! Zones are rings on the 1/64 du grid (integer coordinates, the interior on the left of every edge: outer rings
//! counter-clockwise, holes clockwise). By Green's theorem, twice the area of `A ∩ B` is the sum of `cross(p, q)` over
//! the boundary of the intersection, and a piece `p + t·(q − p)`, `t ∈ [t₁, t₂]`, of a straight edge contributes
//! `(t₂ − t₁) · cross(p, q)`. So each edge of `A` is cut at every point where it meets `B`'s boundary. A piece
//! counts when it lies inside `B`, or along `B`'s boundary with `B`'s interior on the same side (shared boundary is
//! counted once). Each edge of `B` is cut the same way, and a piece counts when it lies strictly inside `A`.
//!
//! Every predicate is exact. Segment tests and cut parameters are `i128` ratios: grid coordinates within ±10⁷ du are at
//! most 6.4 × 10⁸, so orientations stay below 3.3 × 10¹⁸ and the cross-multiplied comparisons of parameters below
//! 1.1 × 10³⁷. Inside tests of a piece's midpoint and the sums are arbitrary-precision rationals (`num-bigint`). All
//! work is charged to the caller's account.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

use super::graph::Ip;
use crate::path::GeomError;
use crate::work::Work;

/// A parameter along an edge: `num / den`, `den > 0`.
#[derive(Debug, Clone, Copy)]
struct Param {
    num: i128,
    den: i128,
}

impl Param {
    fn new(num: i128, den: i128) -> Self {
        if den < 0 { Param { num: -num, den: -den } } else { Param { num, den } }
    }

    fn cmp(&self, o: &Param) -> std::cmp::Ordering {
        (self.num * o.den).cmp(&(o.num * self.den))
    }

    fn big(&self) -> BigRational {
        BigRational::new(BigInt::from(self.num), BigInt::from(self.den))
    }
}

fn orient(a: Ip, b: Ip, c: Ip) -> i128 {
    let (ux, uy) = (i128::from(b[0] - a[0]), i128::from(b[1] - a[1]));
    let (vx, vy) = (i128::from(c[0] - a[0]), i128::from(c[1] - a[1]));
    ux * vy - uy * vx
}

fn dot(a: Ip, b: Ip, c: Ip, d: Ip) -> i128 {
    i128::from(b[0] - a[0]) * i128::from(d[0] - c[0]) + i128::from(b[1] - a[1]) * i128::from(d[1] - c[1])
}

fn edges(rings: &[Vec<Ip>]) -> Vec<(Ip, Ip)> {
    let mut out = Vec::new();
    for r in rings {
        for k in 0..r.len() {
            let (p, q) = (r[k], r[(k + 1) % r.len()]);
            if p != q {
                out.push((p, q));
            }
        }
    }
    out
}

/// Whether the point `p + t·(q − p)` (exact) lies inside the rings (crossing number of a ray towards +x).
fn inside(p: Ip, q: Ip, t: &BigRational, rings: &[(Ip, Ip)]) -> bool {
    let (n, d) = (t.numer(), t.denom());
    let (vx, vy) = (BigInt::from(q[0] - p[0]), BigInt::from(q[1] - p[1]));
    // The point is (mx / d, my / d) with d > 0.
    let mx = BigInt::from(p[0]) * d + n * &vx;
    let my = BigInt::from(p[1]) * d + n * &vy;
    let mut odd = false;
    for &(c, e) in rings {
        let (cy, ey) = (BigInt::from(c[1]) * d, BigInt::from(e[1]) * d);
        if (cy > my) == (ey > my) {
            continue;
        }
        // x of the edge at the point's height, compared with the point's x: mx/d < cx + (my/d − cy)·dx/dy.
        let (dx, dy) = (BigInt::from(e[0] - c[0]), BigInt::from(e[1] - c[1]));
        let lhs = &mx * &dy;
        let rhs = BigInt::from(c[0]) * d * &dy + (&my - BigInt::from(c[1]) * d) * &dx;
        let left_of = if dy.is_positive() { lhs < rhs } else { lhs > rhs };
        if left_of {
            odd = !odd;
        }
    }
    odd
}

/// The total parameter length of the pieces of edge `p → q` that count, against the other polygon's edges.
/// `on_same_side`: whether pieces along the other boundary with its interior on the same side count (pass over `A`).
fn counted(p: Ip, q: Ip, other: &[(Ip, Ip)], on_same_side: bool, work: &mut Work) -> Result<BigRational, GeomError> {
    work.spend_n(other.len().saturating_mul(4))?;
    let len2 = dot(p, q, p, q);
    let mut cuts: Vec<Param> = vec![Param::new(0, 1), Param::new(1, 1)];
    // Collinear edges of the other polygon: their parameter interval along p → q and whether they run alike.
    let mut along: Vec<(Param, Param, bool)> = Vec::new();
    for &(c, d) in other {
        let (o1, o2) = (orient(p, q, c), orient(p, q, d));
        if o1 == 0 && o2 == 0 {
            let (tc, td) = (Param::new(dot(p, q, p, c), len2), Param::new(dot(p, q, p, d), len2));
            let (lo, hi) = if tc.cmp(&td).is_le() { (tc, td) } else { (td, tc) };
            for t in [tc, td] {
                if t.num > 0 && t.num < t.den {
                    cuts.push(t);
                }
            }
            along.push((lo, hi, dot(p, q, c, d) > 0));
            continue;
        }
        let (o3, o4) = (orient(c, d, p), orient(c, d, q));
        if o1.signum() * o2.signum() > 0 || o3.signum() * o4.signum() > 0 || o3 == o4 {
            continue;
        }
        let t = Param::new(o3, o3 - o4);
        if t.num > 0 && t.num < t.den {
            cuts.push(t);
        }
    }
    cuts.sort_by(Param::cmp);
    cuts.dedup_by(|a, b| a.cmp(b).is_eq());
    work.spend_n(cuts.len().saturating_mul(other.len().saturating_add(16)))?;
    let mut total = BigRational::zero();
    for w in cuts.windows(2) {
        let (t1, t2) = (w[0], w[1]);
        // Along the other boundary: counted once, from the first polygon's pass, when both interiors are on one side.
        let on = along.iter().find(|(lo, hi, _)| lo.cmp(&t1).is_le() && t2.cmp(hi).is_le());
        let take = match on {
            Some(&(_, _, same)) => on_same_side && same,
            None => {
                let mid = (t1.big() + t2.big()) / BigInt::from(2);
                inside(p, q, &mid, other)
            }
        };
        if take {
            total += t2.big() - t1.big();
        }
    }
    Ok(total)
}

/// Twice the exact area of the intersection of two polygons given as grid rings (interior on the left of each edge).
///
/// # Errors
/// `limit.exceeded` when the account runs out.
pub fn overlap_area2(a: &[Vec<Ip>], b: &[Vec<Ip>], work: &mut Work) -> Result<BigRational, GeomError> {
    let (ea, eb) = (edges(a), edges(b));
    let mut sum = BigRational::zero();
    for &(p, q) in &ea {
        let l = counted(p, q, &eb, true, work)?;
        if !l.is_zero() {
            sum += l * BigRational::from_integer(BigInt::from(orient([0, 0], p, q)));
        }
    }
    for &(p, q) in &eb {
        let l = counted(p, q, &ea, false, work)?;
        if !l.is_zero() {
            sum += l * BigRational::from_integer(BigInt::from(orient([0, 0], p, q)));
        }
    }
    Ok(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: i64, y: i64, s: i64) -> Vec<Ip> {
        vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]]
    }

    fn area(a: &[Vec<Ip>], b: &[Vec<Ip>]) -> BigRational {
        overlap_area2(a, b, &mut Work::unlimited("t")).unwrap_or_else(|_| BigRational::zero())
    }

    fn int(n: i64) -> BigRational {
        BigRational::from_integer(BigInt::from(n))
    }

    #[test]
    fn identical_shared_and_disjoint_squares() {
        assert_eq!(area(&[sq(0, 0, 10)], &[sq(0, 0, 10)]), int(200));
        assert_eq!(area(&[sq(0, 0, 10)], &[sq(5, 0, 10)]), int(100));
        assert_eq!(area(&[sq(0, 0, 10)], &[sq(10, 0, 10)]), int(0), "a shared edge has no area");
        assert_eq!(area(&[sq(0, 0, 10)], &[sq(20, 0, 10)]), int(0));
    }

    #[test]
    fn holes_and_a_sloping_cut_are_exact() {
        // A ring 0..20 with a hole 5..15 (clockwise) against a square inside the hole and one across it.
        let mut hole = sq(5, 5, 10);
        hole.reverse();
        let ring = vec![sq(0, 0, 20), hole];
        assert_eq!(area(&ring, &[sq(6, 6, 2)]), int(0));
        assert_eq!(area(&ring, &[sq(0, 0, 20)]), int(600));
        // A triangle cutting a unit square on a slope: area 1 − 9/8320 (in grid units, scaled by 64² per du²).
        let tri = vec![vec![[0, 0], [6400, 0], [0, 65]]];
        let got = area(&tri, &[sq(64, 0, 64)]);
        let want = BigRational::new(BigInt::from(2 * 64 * 64 * 8311), BigInt::from(8320));
        assert_eq!(got, want);
    }
}
