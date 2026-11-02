//! An independent exact planar-arrangement oracle (M2 C2.4 readiness item 2), sharing no code with the product's
//! planar map: segments with integer end points are split at every exact intersection and collinear overlap
//! (rational coordinates in `i128`), half-edges are ordered around each vertex by exact angle, faces are traced, and
//! every bounded face's exact area is its outer cycle's area minus the cycles of the components directly inside it.
#![allow(dead_code, clippy::unwrap_used, reason = "test oracle")]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// A rational number `n / d`, `d > 0`, reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Q {
    pub n: i128,
    pub d: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl Q {
    pub fn new(n: i128, d: i128) -> Self {
        let s = if d < 0 { -1 } else { 1 };
        let g = gcd(n, d);
        Q { n: s * n / g, d: s * d / g }
    }
    pub fn int(n: i64) -> Self {
        Q { n: i128::from(n), d: 1 }
    }
    fn add(self, o: Q) -> Q {
        Q::new(self.n * o.d + o.n * self.d, self.d * o.d)
    }
    fn sub(self, o: Q) -> Q {
        Q::new(self.n * o.d - o.n * self.d, self.d * o.d)
    }
    fn mul(self, o: Q) -> Q {
        Q::new(self.n * o.n, self.d * o.d)
    }
    fn div(self, o: Q) -> Q {
        Q::new(self.n * o.d, self.d * o.n)
    }
    fn sign(self) -> i32 {
        self.n.signum() as i32
    }
    fn cmp_q(self, o: Q) -> Ordering {
        (self.n * o.d).cmp(&(o.n * self.d))
    }
    pub fn f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

pub type Pt = (Q, Q);

/// An input segment between integer points.
pub type Seg = ((i64, i64), (i64, i64));

fn cross(o: Pt, a: Pt, b: Pt) -> Q {
    a.0.sub(o.0).mul(b.1.sub(o.1)).sub(a.1.sub(o.1).mul(b.0.sub(o.0)))
}

fn on_segment(p: Pt, a: Pt, b: Pt) -> bool {
    cross(a, b, p).sign() == 0
        && p.0.cmp_q(min(a.0, b.0)) != Ordering::Less
        && p.0.cmp_q(max(a.0, b.0)) != Ordering::Greater
        && p.1.cmp_q(min(a.1, b.1)) != Ordering::Less
        && p.1.cmp_q(max(a.1, b.1)) != Ordering::Greater
}

fn min(a: Q, b: Q) -> Q {
    if a.cmp_q(b) == Ordering::Greater { b } else { a }
}

fn max(a: Q, b: Q) -> Q {
    if a.cmp_q(b) == Ordering::Less { b } else { a }
}

/// The intersection point of two properly crossing segments, if any.
fn crossing(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<Pt> {
    let r = (b.0.sub(a.0), b.1.sub(a.1));
    let s = (d.0.sub(c.0), d.1.sub(c.1));
    let den = r.0.mul(s.1).sub(r.1.mul(s.0));
    if den.sign() == 0 {
        return None;
    }
    let ca = (c.0.sub(a.0), c.1.sub(a.1));
    let t = ca.0.mul(s.1).sub(ca.1.mul(s.0)).div(den);
    let u = ca.0.mul(r.1).sub(ca.1.mul(r.0)).div(den);
    let zero = Q::int(0);
    let one = Q::int(1);
    let inside = |v: Q| v.cmp_q(zero) != Ordering::Less && v.cmp_q(one) != Ordering::Greater;
    (inside(t) && inside(u)).then(|| (a.0.add(t.mul(r.0)), a.1.add(t.mul(r.1))))
}

/// The arrangement's bounded faces: exact areas (du²), sorted.
pub fn face_areas(segments: &[Seg]) -> Vec<Q> {
    let segs: Vec<(Pt, Pt)> = segments
        .iter()
        .filter(|(a, b)| a != b)
        .map(|&((ax, ay), (bx, by))| ((Q::int(ax), Q::int(ay)), (Q::int(bx), Q::int(by))))
        .collect();
    // Split points of each segment: its ends, crossings with others, and others' ends lying on it.
    let mut edges: BTreeSet<(Pt, Pt)> = BTreeSet::new();
    for (i, &(a, b)) in segs.iter().enumerate() {
        let mut pts: Vec<Pt> = vec![a, b];
        for (j, &(c, d)) in segs.iter().enumerate() {
            if i == j {
                continue;
            }
            if let Some(p) = crossing(a, b, c, d) {
                pts.push(p);
            }
            for q in [c, d] {
                if on_segment(q, a, b) {
                    pts.push(q);
                }
            }
        }
        // Order along the segment by the larger coordinate delta.
        let horizontal = a.0.sub(b.0).n.abs() * a.1.sub(b.1).d >= a.1.sub(b.1).n.abs() * a.0.sub(b.0).d;
        pts.sort_by(
            |p, q| if horizontal { p.0.cmp_q(q.0).then(p.1.cmp_q(q.1)) } else { p.1.cmp_q(q.1).then(p.0.cmp_q(q.0)) },
        );
        pts.dedup();
        for w in pts.windows(2) {
            let (p, q) = (w[0], w[1]);
            edges.insert(if p < q { (p, q) } else { (q, p) });
        }
    }
    // Half-edges, ordered counter-clockwise around each vertex.
    let mut out: BTreeMap<Pt, Vec<Pt>> = BTreeMap::new();
    for &(p, q) in &edges {
        out.entry(p).or_default().push(q);
        out.entry(q).or_default().push(p);
    }
    let half = |v: Pt, w: Pt| -> i32 {
        let (dx, dy) = (w.0.sub(v.0).sign(), w.1.sub(v.1).sign());
        if dy > 0 || (dy == 0 && dx > 0) { 0 } else { 1 }
    };
    for (v, list) in out.iter_mut() {
        let v = *v;
        list.sort_by(|a, b| half(v, *a).cmp(&half(v, *b)).then_with(|| 0.cmp(&cross(v, *a, *b).sign())));
    }
    // Faces: from half-edge (u → v), the next is (v → w) with w the neighbour of v just before u in counter-clockwise
    // order (the face lies on the left).
    let mut used: BTreeSet<(Pt, Pt)> = BTreeSet::new();
    let mut cycles: Vec<Vec<Pt>> = Vec::new();
    for (&u, list) in &out {
        for &v in list {
            if used.contains(&(u, v)) {
                continue;
            }
            let mut cycle = Vec::new();
            let (mut a, mut b) = (u, v);
            while used.insert((a, b)) {
                cycle.push(a);
                let around = &out[&b];
                let k = around.iter().position(|x| *x == a).unwrap();
                let w = around[(k + around.len() - 1) % around.len()];
                (a, b) = (b, w);
            }
            cycles.push(cycle);
        }
    }
    let area2 = |c: &[Pt]| -> Q {
        let n = c.len();
        (0..n).fold(Q::int(0), |s, i| s.add(c[i].0.mul(c[(i + 1) % n].1).sub(c[(i + 1) % n].0.mul(c[i].1))))
    };
    let areas: Vec<Q> = cycles.iter().map(|c| area2(c)).collect();
    // Positive cycles bound faces; negative (or zero-area tree) cycles are components' outer boundaries.
    let inside = |poly: &[Pt], p: Pt| -> bool {
        // Ray casting to +x with exact arithmetic (p is never on the polygon here: it is a vertex of another component).
        let mut c = false;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.1.cmp_q(p.1) == Ordering::Greater) != (b.1.cmp_q(p.1) == Ordering::Greater) {
                let x = a.0.add(p.1.sub(a.1).mul(b.0.sub(a.0)).div(b.1.sub(a.1)));
                if p.0.cmp_q(x) == Ordering::Less {
                    c = !c;
                }
            }
        }
        c
    };
    let mut face_area: Vec<Q> = areas.iter().map(|a| if a.sign() > 0 { *a } else { Q::int(0) }).collect();
    for (i, c) in cycles.iter().enumerate() {
        if areas[i].sign() > 0 {
            continue;
        }
        // The smallest positive cycle containing this component (not sharing its vertices).
        let p = c[0];
        let mut best: Option<usize> = None;
        for (j, d) in cycles.iter().enumerate() {
            if areas[j].sign() > 0
                && !d.contains(&p)
                && inside(d, p)
                && best.is_none_or(|b| areas[j].cmp_q(areas[b]) == Ordering::Less)
            {
                best = Some(j);
            }
        }
        if let Some(j) = best {
            face_area[j] = face_area[j].add(areas[i]);
        }
    }
    let mut v: Vec<Q> = face_area.into_iter().filter(|a| a.sign() > 0).map(|a| a.div(Q::int(2))).collect();
    v.sort_by(|a, b| a.cmp_q(*b));
    v
}
