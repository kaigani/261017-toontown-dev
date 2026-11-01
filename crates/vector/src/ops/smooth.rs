//! Smooth (02 §8.3; M2 C2.3): removes anchors from a path while every point of the original stays within a tolerance
//! of the result. Each candidate anchor's two segments are refitted as one cubic with the outer tangents kept (least
//! squares on chord-length parameters); the anchor goes when the refit's deviation from the two segments, sampled at
//! matched parameters both ways, is within the tolerance. Path ends and protected anchors (junctions shared with
//! other strokes, unselected anchors of a point selection) never go, so topology is kept. Deterministic: candidates
//! are tried in order, cheapest deviation first within a pass, and passes repeat until nothing changes.

use crate::geom::{Cubic, P};
use crate::path::{Anchor, Path};

const SAMPLES: usize = 16;

fn tangent_out(c: &Cubic) -> P {
    for v in [c.p1 - c.p0, c.p2 - c.p0, c.p3 - c.p0] {
        if v.len() > 1e-12 {
            return v * (1.0 / v.len());
        }
    }
    P::default()
}

fn tangent_in(c: &Cubic) -> P {
    for v in [c.p2 - c.p3, c.p1 - c.p3, c.p0 - c.p3] {
        if v.len() > 1e-12 {
            return v * (1.0 / v.len());
        }
    }
    P::default()
}

/// Points along two consecutive segments with their chord-length parameters on the joined curve.
fn samples(a: &Cubic, b: &Cubic) -> Vec<(f64, P)> {
    let mut pts: Vec<P> = Vec::with_capacity(2 * SAMPLES + 1);
    for k in 0..SAMPLES {
        pts.push(a.eval(k as f64 / SAMPLES as f64));
    }
    for k in 0..=SAMPLES {
        pts.push(b.eval(k as f64 / SAMPLES as f64));
    }
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        let last = acc.last().copied().unwrap_or(0.0);
        acc.push(last + w[0].dist(w[1]));
    }
    let total = acc.last().copied().unwrap_or(0.0);
    pts.into_iter().zip(acc).map(|(p, s)| (if total > 0.0 { s / total } else { 0.0 }, p)).collect()
}

/// One cubic through `a.p0` and `b.p3` with the outer tangents of `a` and `b`, fitted to their samples.
fn refit(a: &Cubic, b: &Cubic) -> Cubic {
    let (p0, p3) = (a.p0, b.p3);
    let (t1, t2) = (tangent_out(a), tangent_in(b));
    let s = samples(a, b);
    let (mut c11, mut c12, mut c22, mut x1, mut x2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (u, p) in &s {
        let mt = 1.0 - u;
        let b0 = mt * mt * mt;
        let b1 = 3.0 * u * mt * mt;
        let b2 = 3.0 * u * u * mt;
        let b3 = u * u * u;
        let a1 = t1 * b1;
        let a2 = t2 * b2;
        let rest = *p - (p0 * (b0 + b1) + p3 * (b2 + b3));
        c11 += a1.dot(a1);
        c12 += a1.dot(a2);
        c22 += a2.dot(a2);
        x1 += a1.dot(rest);
        x2 += a2.dot(rest);
    }
    let det = c11 * c22 - c12 * c12;
    let chord = p0.dist(p3);
    let (mut l1, mut l2) =
        if det.abs() > 1e-12 { ((x1 * c22 - x2 * c12) / det, (c11 * x2 - c12 * x1) / det) } else { (0.0, 0.0) };
    if !(l1.is_finite() && l2.is_finite()) || l1 < 1e-6 * chord || l2 < 1e-6 * chord {
        l1 = chord / 3.0;
        l2 = chord / 3.0;
    }
    Cubic { p0, p1: p0 + t1 * l1, p2: p3 + t2 * l2, p3 }
}

/// The largest distance between a refit and the original points it replaces (`orig`: dense samples of the original
/// segments spanned), measured both ways.
fn deviation(r: &Cubic, orig: &[P]) -> f64 {
    let fine: Vec<P> = (0..=8 * SAMPLES).map(|k| r.eval(k as f64 / (8 * SAMPLES) as f64)).collect();
    let there = orig.iter().map(|q| to_polyline(*q, &fine)).fold(0.0f64, f64::max);
    let back = fine.iter().step_by(4).map(|q| to_polyline(*q, orig)).fold(0.0f64, f64::max);
    there.max(back)
}

/// The distance from `q` to a polyline.
#[must_use]
pub fn to_polyline(q: P, line: &[P]) -> f64 {
    if line.len() == 1 {
        return line[0].dist(q);
    }
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let l2 = ab.dot(ab);
            let t = if l2 > 0.0 { ((q - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            (a + ab * t).dist(q)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Smooths `path`: anchors whose index `fixed` returns true never go. Returns the smoothed path and the number of
/// anchors removed.
#[must_use]
pub fn smooth(path: &Path, tolerance: f64, fixed: &dyn Fn(usize) -> bool) -> (Path, usize) {
    // Track original indices so `fixed` keeps its meaning while anchors go.
    let mut anchors: Vec<(usize, Anchor)> = path.anchors.iter().copied().enumerate().collect();
    let min = if path.closed { 3 } else { 2 };
    // Dense samples of each original segment: a refit is always measured against the original shape.
    let segs = path.segments();
    let orig: Vec<Vec<P>> =
        segs.iter().map(|c| (0..=4 * SAMPLES).map(|k| c.eval(k as f64 / (4 * SAMPLES) as f64)).collect()).collect();
    let total = path.anchors.len();
    let span = |from: usize, to: usize| -> Vec<P> {
        let mut out = Vec::new();
        let mut k = from;
        while k != to {
            out.extend(orig.get(k).into_iter().flatten().copied());
            k = (k + 1) % total;
        }
        out
    };
    let mut removed = 0;
    loop {
        let n = anchors.len();
        if n <= min {
            break;
        }
        let mut best: Option<(f64, usize, Cubic)> = None;
        for i in 0..n {
            if !path.closed && (i == 0 || i == n - 1) {
                continue;
            }
            if fixed(anchors[i].0) {
                continue;
            }
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;
            let (ap, ai, an) = (anchors[prev].1, anchors[i].1, anchors[next].1);
            let a = Cubic { p0: ap.p, p1: ap.p + ap.outh, p2: ai.p + ai.inh, p3: ai.p };
            let b = Cubic { p0: ai.p, p1: ai.p + ai.outh, p2: an.p + an.inh, p3: an.p };
            let r = refit(&a, &b);
            let dev = deviation(&r, &span(anchors[prev].0, anchors[next].0));
            if dev <= tolerance && best.as_ref().is_none_or(|(d, _, _)| dev < *d) {
                best = Some((dev, i, r));
            }
        }
        let Some((_, i, r)) = best else { break };
        let n = anchors.len();
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        anchors[prev].1.outh = r.p1 - r.p0;
        anchors[next].1.inh = r.p2 - r.p3;
        anchors.remove(i);
        removed += 1;
    }
    (Path { anchors: anchors.into_iter().map(|(_, a)| a).collect(), closed: path.closed }, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collinear_points_go_and_the_shape_stays_within_tolerance() {
        let pts: Vec<[f64; 6]> = (0..=10).map(|k| [f64::from(k) * 10.0, 0.0, 0.0, 0.0, 0.0, 0.0]).collect();
        let (p, n) = smooth(&Path::from_points(&pts, false), 1.0, &|_| false);
        assert_eq!((n, p.anchors.len()), (9, 2));
        // A wavy line keeps its peaks when the tolerance is small.
        let wave: Vec<[f64; 6]> =
            (0..=8).map(|k| [f64::from(k) * 10.0, if k % 2 == 0 { 0.0 } else { 20.0 }, 0.0, 0.0, 0.0, 0.0]).collect();
        let (w, n) = smooth(&Path::from_points(&wave, false), 1.0, &|_| false);
        assert_eq!((n, w.anchors.len()), (0, 9));
        // Fixed anchors stay.
        let (f, _) = smooth(&Path::from_points(&pts, false), 1.0, &|i| i == 5);
        assert!(f.anchors.iter().any(|a| a.p == P::new(50.0, 0.0)));
    }

    #[test]
    fn a_closed_circle_of_many_anchors_keeps_at_least_three_and_stays_close() {
        let pts: Vec<[f64; 6]> = (0..32)
            .map(|k| {
                let t = f64::from(k) / 32.0 * std::f64::consts::TAU;
                [100.0 * libm::cos(t), 100.0 * libm::sin(t), 0.0, 0.0, 0.0, 0.0]
            })
            .collect();
        let src = Path::from_points(&pts, true);
        let (p, n) = smooth(&src, 1.0, &|_| false);
        assert!(n > 0 && p.anchors.len() >= 3);
        let mut ring = p.flatten(0.01);
        ring.push(ring[0]);
        for q in src.flatten(0.01) {
            let d = to_polyline(q, &ring);
            assert!(d <= 1.0 + 0.02, "{d}");
        }
    }
}
