//! C22-R2-04: exact overlap areas against an independent big-rational reference. Random non-convex (star-shaped) grid
//! polygons, some with a clockwise hole, are intersected with random convex grid polygons. The reference clips each
//! ring with Sutherland–Hodgman in exact rationals and takes the shoelace area; `planar::exact::overlap_area2` must
//! agree exactly. Fixed seed `toontown-m2-c22-r2-exact-001` (an LCG; the same cases on every OS).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use tt_vector::planar::exact::overlap_area2;
use tt_vector::work::Work;

type Ip = [i64; 2];
type Q = (BigRational, BigRational);

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

fn r(n: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(n))
}

/// A star-shaped (generally non-convex) counter-clockwise polygon around `c`.
fn star(g: &mut Lcg, c: Ip, k: usize, rmax: u64) -> Vec<Ip> {
    let mut pts: Vec<(f64, Ip)> = Vec::new();
    // One point per sector of the full turn: the centre stays in the kernel, so the polygon is simple.
    let sector = 3600 / k as u64;
    let mut i = 0u64;
    while pts.len() < k && i < 64 * k as u64 {
        let a = ((i % k as u64) * sector + g.next(sector)) as f64 / 3600.0 * std::f64::consts::TAU;
        i += 1;
        let rad = (g.next(rmax) + 3) as f64;
        let p = [c[0] + (rad * a.cos()).round() as i64, c[1] + (rad * a.sin()).round() as i64];
        let ang = ((p[1] - c[1]) as f64).atan2((p[0] - c[0]) as f64);
        if p != c && !pts.iter().any(|(b, q)| (b - ang).abs() < 1e-9 || *q == p) {
            pts.push((ang, p));
        }
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ring: Vec<Ip> = pts.into_iter().map(|(_, p)| p).collect();
    // Counter-clockwise, as zone rings are (sorted by angle around a centre outside the hull it can be clockwise).
    let s: i128 = (0..ring.len()).map(|k| orient([0, 0], ring[k], ring[(k + 1) % ring.len()])).sum();
    if s < 0 {
        ring.reverse();
    }
    ring
}

fn orient(a: Ip, b: Ip, c: Ip) -> i128 {
    i128::from(b[0] - a[0]) * i128::from(c[1] - a[1]) - i128::from(b[1] - a[1]) * i128::from(c[0] - a[0])
}

/// The convex hull (counter-clockwise, monotone chain) of random points.
fn convex(g: &mut Lcg, n: usize, span: u64) -> Vec<Ip> {
    let mut pts: Vec<Ip> = (0..n).map(|_| [g.next(span) as i64, g.next(span) as i64]).collect();
    pts.sort_unstable();
    pts.dedup();
    let mut lower: Vec<Ip> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && orient(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Ip> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && orient(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Sutherland–Hodgman: the part of `subject` left of every edge of the convex counter-clockwise `clip` (exact).
fn clip(subject: &[Ip], clip: &[Ip]) -> Vec<Q> {
    let mut poly: Vec<Q> = subject.iter().map(|p| (r(p[0]), r(p[1]))).collect();
    for k in 0..clip.len() {
        let (c, d) = (clip[k], clip[(k + 1) % clip.len()]);
        let (cx, cy, dx, dy) = (r(c[0]), r(c[1]), r(d[0]), r(d[1]));
        let side = |p: &Q| (&dx - &cx) * (&p.1 - &cy) - (&dy - &cy) * (&p.0 - &cx);
        let mut out = Vec::new();
        for i in 0..poly.len() {
            let (p, q) = (&poly[i], &poly[(i + 1) % poly.len()]);
            let (sp, sq) = (side(p), side(q));
            if !sp.is_negative() {
                out.push(p.clone());
            }
            if (sp.is_positive() && sq.is_negative()) || (sp.is_negative() && sq.is_positive()) {
                let t = &sp / (&sp - &sq);
                out.push((&p.0 + (&q.0 - &p.0) * &t, &p.1 + (&q.1 - &p.1) * &t));
            }
        }
        poly = out;
        if poly.is_empty() {
            break;
        }
    }
    poly
}

fn area2(poly: &[Q]) -> BigRational {
    let mut s = BigRational::zero();
    for i in 0..poly.len() {
        let (p, q) = (&poly[i], &poly[(i + 1) % poly.len()]);
        s += &p.0 * &q.1 - &q.0 * &p.1;
    }
    s.abs()
}

#[test]
fn exact_overlap_areas_match_a_big_rational_reference() {
    let mut g = Lcg(u64::from_le_bytes(*b"tt-c22r2"));
    let mut nonzero = 0;
    for case in 0..300 {
        let k = 5 + g.next(10) as usize;
        let outer = star(&mut g, [100, 100], k, 90);
        let n = 4 + g.next(8) as usize;
        let b = convex(&mut g, n, 220);
        if b.len() < 3 {
            continue;
        }
        // Every fourth case has a clockwise hole: a small square near the centre (inside the star's kernel).
        let hole: Option<Vec<Ip>> =
            (case % 4 == 0).then(|| vec![[98, 98], [98, 102], [102, 102], [102, 98]]).filter(|_| {
                let inside = |p: Ip| (0..outer.len()).all(|k| orient(outer[k], outer[(k + 1) % outer.len()], p) > 0);
                [[98, 98], [102, 102], [98, 102], [102, 98]].into_iter().all(inside)
            });
        let mut want = area2(&clip(&outer, &b));
        let mut a = vec![outer.clone()];
        if let Some(h) = &hole {
            let mut ccw = h.clone();
            ccw.reverse();
            want -= area2(&clip(&ccw, &b));
            a.push(h.clone());
        }
        let got = overlap_area2(&a, std::slice::from_ref(&b), &mut Work::unlimited("t")).unwrap();
        assert_eq!(got, want, "case {case}: A {a:?} B {b:?}");
        // Symmetric.
        let back = overlap_area2(std::slice::from_ref(&b), &a, &mut Work::unlimited("t")).unwrap();
        assert_eq!(back, want, "case {case} reversed");
        if !want.is_zero() {
            nonzero += 1;
        }
    }
    assert!(nonzero > 100, "{nonzero}");
}
