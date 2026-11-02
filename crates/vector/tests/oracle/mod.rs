//! An independent exact planar-arrangement oracle (M2 C2.4 readiness item 2), sharing no code with the product's
//! planar map: segments are split at every exact intersection and collinear overlap (arbitrary-precision rationals),
//! half-edges are ordered around each vertex by exact angle, faces are traced, and every bounded face is its outer
//! cycle minus the cycles of the components directly inside it. Besides face areas it gives each face's cycles, an
//! exact interior point, point location, snapping to the integer grid (ties to even), snap safety, the canonical zone
//! ID text and hash (C22-R1-03 ruling, recomputed), and exact overlay overlaps (C24-R1-01).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, reason = "test oracle")]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// An exact rational number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Q(pub BigRational);

impl Q {
    pub fn new(n: i128, d: i128) -> Self {
        Q(BigRational::new(BigInt::from(n), BigInt::from(d)))
    }
    pub fn int(n: i64) -> Self {
        Q(BigRational::from_integer(BigInt::from(n)))
    }
    pub fn plus(&self, o: &Q) -> Q {
        Q(&self.0 + &o.0)
    }
    pub fn minus(&self, o: &Q) -> Q {
        Q(&self.0 - &o.0)
    }
    pub fn times(&self, o: &Q) -> Q {
        Q(&self.0 * &o.0)
    }
    pub fn over(&self, o: &Q) -> Q {
        Q(&self.0 / &o.0)
    }
    pub fn signum(&self) -> i32 {
        if self.0.is_zero() {
            0
        } else if self.0.is_positive() {
            1
        } else {
            -1
        }
    }
    pub fn cmp_to(&self, o: &Q) -> Ordering {
        self.0.cmp(&o.0)
    }
    pub fn abs(&self) -> Q {
        Q(self.0.abs())
    }
    pub fn f64(self) -> f64 {
        self.0.to_f64().unwrap_or(f64::NAN)
    }
    /// The exact value of a finite `f64` (a dyadic rational).
    pub fn from_f64(x: f64) -> Q {
        Q(BigRational::from_float(x).expect("a finite number"))
    }
}

pub type Pt = (Q, Q);

/// An input segment between integer points.
pub type Seg = ((i64, i64), (i64, i64));

pub fn pt(x: i64, y: i64) -> Pt {
    (Q::int(x), Q::int(y))
}

fn zero() -> Q {
    Q(BigRational::zero())
}

fn one() -> Q {
    Q(BigRational::one())
}

fn half() -> Q {
    Q::new(1, 2)
}

fn cross(o: &Pt, a: &Pt, b: &Pt) -> Q {
    a.0.minus(&o.0).times(&b.1.minus(&o.1)).minus(&a.1.minus(&o.1).times(&b.0.minus(&o.0)))
}

fn min(a: &Q, b: &Q) -> Q {
    if a.cmp_to(b) == Ordering::Greater { b.clone() } else { a.clone() }
}

fn max(a: &Q, b: &Q) -> Q {
    if a.cmp_to(b) == Ordering::Less { b.clone() } else { a.clone() }
}

/// Whether `p` lies on the closed segment `a`–`b` (exact).
pub fn point_on_segment(p: &Pt, a: &Pt, b: &Pt) -> bool {
    cross(a, b, p).signum() == 0
        && p.0.cmp_to(&min(&a.0, &b.0)) != Ordering::Less
        && p.0.cmp_to(&max(&a.0, &b.0)) != Ordering::Greater
        && p.1.cmp_to(&min(&a.1, &b.1)) != Ordering::Less
        && p.1.cmp_to(&max(&a.1, &b.1)) != Ordering::Greater
}

/// The intersection point of two segments crossing at one point (end points included), if any.
fn crossing(a: &Pt, b: &Pt, c: &Pt, d: &Pt) -> Option<Pt> {
    let r = (b.0.minus(&a.0), b.1.minus(&a.1));
    let s = (d.0.minus(&c.0), d.1.minus(&c.1));
    let den = r.0.times(&s.1).minus(&r.1.times(&s.0));
    if den.signum() == 0 {
        return None;
    }
    let ca = (c.0.minus(&a.0), c.1.minus(&a.1));
    let t = ca.0.times(&s.1).minus(&ca.1.times(&s.0)).over(&den);
    let u = ca.0.times(&r.1).minus(&ca.1.times(&r.0)).over(&den);
    let inside = |v: &Q| v.signum() >= 0 && v.cmp_to(&one()) != Ordering::Greater;
    (inside(&t) && inside(&u)).then(|| (a.0.plus(&t.times(&r.0)), a.1.plus(&t.times(&r.1))))
}

/// `a + t (b − a)`.
pub fn lerp(a: &Pt, b: &Pt, t: &Q) -> Pt {
    (a.0.plus(&t.times(&b.0.minus(&a.0))), a.1.plus(&t.times(&b.1.minus(&a.1))))
}

fn twice_area(c: &[Pt]) -> Q {
    let n = c.len();
    (0..n).fold(zero(), |s, i| s.plus(&c[i].0.times(&c[(i + 1) % n].1).minus(&c[(i + 1) % n].0.times(&c[i].1))))
}

/// Even-odd containment of `p` (never on the cycle) in a closed cycle; an edge traversed twice cancels.
fn inside_cycle(poly: &[Pt], p: &Pt) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        if (a.1.cmp_to(&p.1) == Ordering::Greater) != (b.1.cmp_to(&p.1) == Ordering::Greater) {
            let x = a.0.plus(&p.1.minus(&a.1).times(&b.0.minus(&a.0)).over(&b.1.minus(&a.1)));
            if p.0.cmp_to(&x) == Ordering::Less {
                c = !c;
            }
        }
    }
    c
}

/// An exact point strictly inside the region bounded by `cycles` (even-odd): on a horizontal line between two vertex
/// levels, the middle of the first inside interval.
fn interior(cycles: &[Vec<Pt>]) -> Option<Pt> {
    let mut ys: Vec<Q> = cycles.iter().flatten().map(|p| p.1.clone()).collect();
    ys.sort();
    ys.dedup();
    for w in ys.windows(2) {
        let y = w[0].plus(&w[1]).times(&half());
        let mut xs: Vec<Q> = Vec::new();
        for c in cycles {
            for i in 0..c.len() {
                let (a, b) = (&c[i], &c[(i + 1) % c.len()]);
                if (a.1.cmp_to(&y) == Ordering::Greater) != (b.1.cmp_to(&y) == Ordering::Greater) {
                    xs.push(a.0.plus(&y.minus(&a.1).times(&b.0.minus(&a.0)).over(&b.1.minus(&a.1))));
                }
            }
        }
        xs.sort();
        for pair in xs.chunks(2) {
            if pair.len() == 2 && pair[0].cmp_to(&pair[1]) == Ordering::Less {
                return Some((pair[0].plus(&pair[1]).times(&half()), y));
            }
        }
    }
    None
}

/// A bounded face: its counter-clockwise outer cycle, the clockwise hole cycles of the components directly inside
/// it (each traversed with the face on the left), twice its exact area, and an exact interior point.
#[derive(Debug, Clone)]
pub struct Face {
    pub outer: Vec<Pt>,
    pub holes: Vec<Vec<Pt>>,
    pub area2: Q,
    pub probe: Pt,
}

/// An arrangement's bounded faces and its edges (each split at every vertex lying on it).
#[derive(Debug, Clone)]
pub struct Arrangement {
    pub faces: Vec<Face>,
    pub edges: Vec<(Pt, Pt)>,
}

/// The arrangement of segments with rational end points.
pub fn arrangement(segments: &[(Pt, Pt)]) -> Arrangement {
    let segs: Vec<(Pt, Pt)> = segments.iter().filter(|(a, b)| a != b).cloned().collect();
    let mut edges: BTreeSet<(Pt, Pt)> = BTreeSet::new();
    for (i, (a, b)) in segs.iter().enumerate() {
        let mut pts: Vec<Pt> = vec![a.clone(), b.clone()];
        for (j, (c, d)) in segs.iter().enumerate() {
            if i == j {
                continue;
            }
            if let Some(p) = crossing(a, b, c, d) {
                pts.push(p);
            }
            for p in [c, d] {
                if point_on_segment(p, a, b) {
                    pts.push(p.clone());
                }
            }
        }
        let horizontal = a.0.minus(&b.0).abs().cmp_to(&a.1.minus(&b.1).abs()) != Ordering::Less;
        pts.sort_by(|p, q| {
            if horizontal { p.0.cmp_to(&q.0).then(p.1.cmp_to(&q.1)) } else { p.1.cmp_to(&q.1).then(p.0.cmp_to(&q.0)) }
        });
        pts.dedup();
        for w in pts.windows(2) {
            let (p, q) = (w[0].clone(), w[1].clone());
            edges.insert(if p < q { (p, q) } else { (q, p) });
        }
    }
    let mut out: BTreeMap<Pt, Vec<Pt>> = BTreeMap::new();
    for (p, q) in &edges {
        out.entry(p.clone()).or_default().push(q.clone());
        out.entry(q.clone()).or_default().push(p.clone());
    }
    let side = |v: &Pt, w: &Pt| -> i32 {
        let (dx, dy) = (w.0.minus(&v.0).signum(), w.1.minus(&v.1).signum());
        if dy > 0 || (dy == 0 && dx > 0) { 0 } else { 1 }
    };
    for (v, list) in out.iter_mut() {
        list.sort_by(|a, b| side(v, a).cmp(&side(v, b)).then_with(|| 0.cmp(&cross(v, a, b).signum())));
    }
    let mut used: BTreeSet<(Pt, Pt)> = BTreeSet::new();
    let mut cycles: Vec<Vec<Pt>> = Vec::new();
    for (u, list) in &out {
        for v in list {
            if used.contains(&(u.clone(), v.clone())) {
                continue;
            }
            let mut cycle = Vec::new();
            let (mut a, mut b) = (u.clone(), v.clone());
            while used.insert((a.clone(), b.clone())) {
                cycle.push(a.clone());
                let around = &out[&b];
                let k = around.iter().position(|x| *x == a).unwrap();
                let w = around[(k + around.len() - 1) % around.len()].clone();
                (a, b) = (b, w);
            }
            cycles.push(cycle);
        }
    }
    let areas: Vec<Q> = cycles.iter().map(|c| twice_area(c)).collect();
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); cycles.len()];
    for (i, c) in cycles.iter().enumerate() {
        if areas[i].signum() > 0 {
            continue;
        }
        let p = &c[0];
        let mut best: Option<usize> = None;
        for (j, d) in cycles.iter().enumerate() {
            if areas[j].signum() > 0
                && !d.contains(p)
                && inside_cycle(d, p)
                && best.is_none_or(|b| areas[j].cmp_to(&areas[b]) == Ordering::Less)
            {
                best = Some(j);
            }
        }
        if let Some(j) = best {
            holes[j].push(i);
        }
    }
    let mut faces = Vec::new();
    for (i, c) in cycles.iter().enumerate() {
        if areas[i].signum() <= 0 {
            continue;
        }
        let hs: Vec<Vec<Pt>> = holes[i].iter().map(|&h| cycles[h].clone()).collect();
        let area2 = holes[i].iter().fold(areas[i].clone(), |s, &h| s.plus(&areas[h]));
        let mut all = vec![c.clone()];
        all.extend(hs.iter().cloned());
        let probe = interior(&all).unwrap();
        faces.push(Face { outer: c.clone(), holes: hs, area2, probe });
    }
    Arrangement { faces, edges: edges.into_iter().collect() }
}

/// The arrangement's bounded faces: exact areas, sorted.
pub fn face_areas(segments: &[Seg]) -> Vec<Q> {
    let segs: Vec<(Pt, Pt)> = segments.iter().map(|&((ax, ay), (bx, by))| (pt(ax, ay), pt(bx, by))).collect();
    let mut v: Vec<Q> = arrangement(&segs).faces.into_iter().map(|f| f.area2.times(&half())).collect();
    v.sort();
    v
}

impl Arrangement {
    /// The face strictly containing `p` (`None`: outside every bounded face, or on an edge).
    pub fn locate(&self, p: &Pt) -> Option<usize> {
        if self.edges.iter().any(|(a, b)| point_on_segment(p, a, b)) {
            return None;
        }
        self.faces.iter().position(|f| inside_cycle(&f.outer, p) && !f.holes.iter().any(|h| inside_cycle(h, p)))
    }

    /// The vertices.
    pub fn vertices(&self) -> BTreeSet<Pt> {
        self.edges.iter().flat_map(|(p, q)| [p.clone(), q.clone()]).collect()
    }
}

/// `q` rounded to the nearest integer, ties to even.
pub fn snap(q: &Q) -> i64 {
    let fl = q.0.floor();
    let frac = &q.0 - &fl;
    let h = BigRational::new(BigInt::from(1), BigInt::from(2));
    let base = fl.to_integer();
    let v = match frac.cmp(&h) {
        Ordering::Less => base,
        Ordering::Greater => base + 1,
        Ordering::Equal => {
            if (&base % BigInt::from(2)).is_zero() {
                base
            } else {
                base + 1
            }
        }
    };
    v.to_i64().expect("a grid coordinate")
}

pub fn snap_pt(p: &Pt) -> (i64, i64) {
    (snap(&p.0), snap(&p.1))
}

/// Twice a face's area with every vertex snapped to the integer grid (the snapped arrangement's area, exact).
pub fn snapped_area2(f: &Face) -> i128 {
    let ring = |c: &[Pt]| -> i128 {
        let s: Vec<(i128, i128)> = c.iter().map(|p| (i128::from(snap(&p.0)), i128::from(snap(&p.1)))).collect();
        (0..s.len()).map(|i| s[i].0 * s[(i + 1) % s.len()].1 - s[(i + 1) % s.len()].0 * s[i].1).sum()
    };
    ring(&f.outer) + f.holes.iter().map(|h| ring(h)).sum::<i128>()
}

/// Whether snapping the arrangement to the integer grid keeps its combinatorics: distinct vertices snap to distinct
/// points, and every vertex lies farther than `margin` from every edge not incident to it.
pub fn snap_safe(a: &Arrangement, margin: i64) -> bool {
    let verts = a.vertices();
    let snapped: BTreeSet<(i64, i64)> = verts.iter().map(snap_pt).collect();
    if snapped.len() != verts.len() {
        return false;
    }
    let m2 = Q::int(margin * margin);
    for v in &verts {
        for (a0, b0) in &a.edges {
            if v == a0 || v == b0 {
                continue;
            }
            let (dx, dy) = (b0.0.minus(&a0.0), b0.1.minus(&a0.1));
            let len2 = dx.times(&dx).plus(&dy.times(&dy));
            let t = v.0.minus(&a0.0).times(&dx).plus(&v.1.minus(&a0.1).times(&dy)).over(&len2);
            let t = if t.signum() < 0 {
                zero()
            } else if t.cmp_to(&one()) == Ordering::Greater {
                one()
            } else {
                t
            };
            let (ex, ey) = (v.0.minus(&a0.0.plus(&t.times(&dx))), v.1.minus(&a0.1.plus(&t.times(&dy))));
            if ex.times(&ex).plus(&ey.times(&ey)).cmp_to(&m2) != Ordering::Greater {
                return false;
            }
        }
    }
    true
}

/// The canonical text of one boundary cycle (the C22-R1-03 zone-ID ruling, recomputed): in traversal order, each
/// snapped vertex followed by the labels of the edge leaving it (`label(from, to)`: the sorted `strokeId:side` list of
/// the strokes on that edge); a vertex where the boundary goes straight on with the same labels is left out; the cycle
/// starts at its smallest token sequence.
pub fn canonical_cycle(cycle: &[Pt], label: &dyn Fn(&Pt, &Pt) -> String) -> String {
    let n = cycle.len();
    let labels: Vec<String> = (0..n).map(|k| label(&cycle[k], &cycle[(k + 1) % n])).collect();
    let s: Vec<(i64, i64)> = cycle.iter().map(snap_pt).collect();
    let mut tokens: Vec<String> = Vec::new();
    for k in 0..n {
        let prev = (k + n - 1) % n;
        let (a, v, b) = (s[prev], s[k], s[(k + 1) % n]);
        let cr = i128::from(v.0 - a.0) * i128::from(b.1 - a.1) - i128::from(v.1 - a.1) * i128::from(b.0 - a.0);
        let dot = i128::from(v.0 - a.0) * i128::from(b.0 - v.0) + i128::from(v.1 - a.1) * i128::from(b.1 - v.1);
        let straight = n > 2 && labels[prev] == labels[k] && cr == 0 && dot > 0;
        if !straight {
            tokens.push(format!("{},{};{}", v.0, v.1, labels[k]));
        }
    }
    if tokens.is_empty() {
        return String::new();
    }
    let m = tokens.len();
    let first = tokens.iter().min().cloned().unwrap();
    let best = (0..m)
        .filter(|&x| tokens[x] == first)
        .min_by(|&x, &y| (0..m).map(|i| &tokens[(x + i) % m]).cmp((0..m).map(|i| &tokens[(y + i) % m])))
        .unwrap();
    (0..m).map(|i| tokens[(best + i) % m].as_str()).collect::<Vec<_>>().join("|")
}

/// A face's zone ID: SHA-256 of its outer cycle's canonical text followed by `/` and each hole's, sorted, as 26
/// Crockford base-32 characters after `zn_`.
pub fn zone_id(f: &Face, label: &dyn Fn(&Pt, &Pt) -> String) -> String {
    use sha2::{Digest, Sha256};
    let mut text = canonical_cycle(&f.outer, label);
    let mut holes: Vec<String> = f.holes.iter().map(|h| canonical_cycle(h, label)).collect();
    holes.sort();
    for h in holes {
        text.push('/');
        text.push_str(&h);
    }
    let digest = Sha256::digest(text.as_bytes());
    const A: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut out = String::from("zn_");
    let (mut acc, mut bits) = (0u32, 0);
    for &byte in digest.iter() {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 && out.len() < 29 {
            bits -= 5;
            out.push(char::from(A[((acc >> bits) & 31) as usize]));
        }
        if out.len() == 29 {
            break;
        }
    }
    out
}

/// Exact overlap areas (twice) between the faces of `new` and of `old`: the faces of the overlay of both segment sets,
/// each located in both.
pub fn overlaps(
    old: &Arrangement,
    new: &Arrangement,
    old_segs: &[(Pt, Pt)],
    new_segs: &[(Pt, Pt)],
) -> BTreeMap<(usize, usize), Q> {
    let mut all = old_segs.to_vec();
    all.extend_from_slice(new_segs);
    let overlay = arrangement(&all);
    let mut out: BTreeMap<(usize, usize), Q> = BTreeMap::new();
    for f in &overlay.faces {
        if let (Some(n), Some(o)) = (new.locate(&f.probe), old.locate(&f.probe)) {
            let e = out.entry((n, o)).or_insert_with(zero);
            *e = e.plus(&f.area2);
        }
    }
    out
}

/// The parameters (sorted, in (0, 1)) where segment `a`–`b` crosses or touches the segments `others`.
pub fn cut_parameters(a: &Pt, b: &Pt, others: &[(Pt, Pt)]) -> Vec<Q> {
    let (dx, dy) = (b.0.minus(&a.0), b.1.minus(&a.1));
    let param = |p: &Pt| if dx.signum() != 0 { p.0.minus(&a.0).over(&dx) } else { p.1.minus(&a.1).over(&dy) };
    let mut ts: Vec<Q> = Vec::new();
    for (c, d) in others {
        if let Some(p) = crossing(a, b, c, d) {
            ts.push(param(&p));
        }
        for p in [c, d] {
            if point_on_segment(p, a, b) {
                ts.push(param(p));
            }
        }
    }
    ts.retain(|t| t.signum() > 0 && t.cmp_to(&one()) == Ordering::Less);
    ts.sort();
    ts.dedup();
    ts
}

/// The snapping-error envelope of an arrangement's total face area (C24-R1-02 ruling), in the segments' units with
/// `grid` units per segment unit: every vertex of the snapped arrangement lies within δ of the exact one — half a grid
/// cell's diagonal for rounding plus the junction tolerance of one-thirty-second for junction resolution — so each face's
/// area moves by at most its perimeter × δ plus its vertex count × δ². Independent of the product (exact faces here).
pub fn snapping_envelope(segments: &[Seg], grid: i64) -> f64 {
    let delta = std::f64::consts::SQRT_2 / (2.0 * grid as f64) + 1.0 / 32.0;
    let segs: Vec<(Pt, Pt)> = segments.iter().map(|&((ax, ay), (bx, by))| (pt(ax, ay), pt(bx, by))).collect();
    let mut total = 0.0;
    for f in arrangement(&segs).faces {
        for c in std::iter::once(&f.outer).chain(&f.holes) {
            for k in 0..c.len() {
                let (a, b) = (&c[k], &c[(k + 1) % c.len()]);
                let (dx, dy) = (b.0.minus(&a.0).f64(), b.1.minus(&a.1).f64());
                total += dx.hypot(dy) * delta + delta * delta;
            }
        }
    }
    total
}
