//! Smooth (02 §8.3; M2 C2.3): removes anchors from a path while every point of the original stays within a tolerance
//! of the result. Each candidate anchor's two segments are refitted as one cubic with the outer tangents kept (least
//! squares on chord-length parameters); the anchor goes when the refit's deviation from the original shape, measured
//! both ways, is within the tolerance. Path ends and protected anchors (junctions shared with other strokes,
//! unselected anchors of a point selection) never go, so topology is kept. Deterministic: removable anchors are ranked
//! by deviation (ties: lower index), and the work is bounded per anchor (C23-R1-07).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::geom::{Cubic, P};
use crate::path::{Anchor, GeomError, Path};
use crate::work::Work;

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

/// Original segments one refit may span: bounds the work of every evaluation (each invocation smooths further).
pub const MAX_SPAN: usize = 32;

/// Samples per original segment and per refit.
const ORIG_SAMPLES: usize = 16;
const FIT_SAMPLES: usize = 64;

/// Distance from `q` to the polyline `line`, searched in a window around `hint` (both curves run the same way), with
/// the nearest segment index; every examined segment is one unit of work.
fn near_windowed(q: P, line: &[P], hint: usize, work: &mut u64) -> (f64, usize) {
    if line.len() < 2 {
        return (line.first().map_or(f64::INFINITY, |p| p.dist(q)), 0);
    }
    let last = line.len() - 2;
    let lo = hint.saturating_sub(8);
    let hi = (hint + 8).min(last);
    let mut best = (f64::INFINITY, hint.min(last));
    for k in lo..=hi {
        *work += 1;
        let d = to_polyline(q, &line[k..k + 2]);
        if d < best.0 {
            best = (d, k);
        }
    }
    best
}

/// The largest distance between a refit and the original points it replaces, both ways; a windowed search can only
/// overestimate, so the shape stays within the tolerance.
fn deviation(r: &Cubic, orig: &[P], work: &mut u64) -> f64 {
    let fine: Vec<P> = (0..=FIT_SAMPLES).map(|k| r.eval(k as f64 / FIT_SAMPLES as f64)).collect();
    let mut worst = 0.0f64;
    let mut hint = 0usize;
    for (n, q) in orig.iter().enumerate() {
        let guess = (n * FIT_SAMPLES / orig.len().max(1)).max(hint);
        let (d, k) = near_windowed(*q, &fine, guess, work);
        hint = k;
        worst = worst.max(d);
    }
    let mut hint = 0usize;
    for (n, q) in fine.iter().enumerate() {
        let guess = (n * orig.len() / fine.len()).max(hint);
        let (d, k) = near_windowed(*q, orig, guess, work);
        hint = k;
        worst = worst.max(d);
    }
    worst
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

/// [`smooth_within`] without a work limit (library callers that bound their input themselves).
#[must_use]
pub fn smooth(path: &Path, tolerance: f64, fixed: &dyn Fn(usize) -> bool) -> (Path, usize) {
    let mut work = Work::unlimited("smoothing");
    smooth_within(path, tolerance, fixed, &mut work).unwrap_or_else(|_| (path.clone(), 0))
}

/// Smooths `path` (02 §8.3; C23-R1-07): anchors whose index `fixed` returns true never go. Removable anchors are
/// ranked by the deviation of the refit that would replace them (measured against the original shape); the best goes,
/// and only its two neighbours are evaluated again, so the work is bounded per anchor. A refit spans at most
/// [`MAX_SPAN`] original segments; each invocation smooths further. Every distance evaluation is charged to `work`.
///
/// # Errors
/// `limit.exceeded` from `work`; `command.timeout` when the watchdog cancels the call.
pub fn smooth_within(
    path: &Path,
    tolerance: f64,
    fixed: &dyn Fn(usize) -> bool,
    work: &mut Work,
) -> Result<(Path, usize), GeomError> {
    let n = path.anchors.len();
    let min = if path.closed { 3 } else { 2 };
    if n <= min {
        return Ok((path.clone(), 0));
    }
    let segs = path.segments();
    let orig: Vec<Vec<P>> =
        segs.iter().map(|c| (0..=ORIG_SAMPLES).map(|k| c.eval(k as f64 / ORIG_SAMPLES as f64)).collect()).collect();
    let mut anchors: Vec<Anchor> = path.anchors.clone();
    let mut alive = vec![true; n];
    let mut prev: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let mut version = vec![0u32; n];
    let mut count = n;
    let span_len = |a: usize, b: usize| if b > a { b - a } else { b + n - a };
    // The refit and deviation of removing anchor `i` now, if allowed.
    let evaluate = |i: usize,
                    anchors: &[Anchor],
                    prev: &[usize],
                    next: &[usize],
                    work: &mut Work|
     -> Result<Option<(f64, Cubic)>, GeomError> {
        if (!path.closed && (i == 0 || i == n - 1)) || fixed(i) {
            return Ok(None);
        }
        let (p, q) = (prev[i], next[i]);
        if span_len(p, q) > MAX_SPAN {
            return Ok(None);
        }
        let (ap, ai, aq) = (anchors[p], anchors[i], anchors[q]);
        let a = Cubic { p0: ap.p, p1: ap.p + ap.outh, p2: ai.p + ai.inh, p3: ai.p };
        let b = Cubic { p0: ai.p, p1: ai.p + ai.outh, p2: aq.p + aq.inh, p3: aq.p };
        let r = refit(&a, &b);
        let mut span: Vec<P> = Vec::new();
        let mut k = p;
        while k != q {
            span.extend(orig.get(k).into_iter().flatten().copied());
            k = (k + 1) % n;
        }
        let mut units = 0u64;
        let dev = deviation(&r, &span, &mut units);
        work.spend(units)?;
        Ok((dev <= tolerance).then_some((dev, r)))
    };
    // Min-heap on (deviation, index) with lazy invalidation by version.
    let mut heap: BinaryHeap<Reverse<(OrdF64, usize, u32)>> = BinaryHeap::new();
    let mut fits: Vec<Option<Cubic>> = vec![None; n];
    for i in 0..n {
        if let Some((dev, r)) = evaluate(i, &anchors, &prev, &next, work)? {
            fits[i] = Some(r);
            heap.push(Reverse((OrdF64(dev), i, version[i])));
        }
    }
    let mut removed = 0;
    while let Some(Reverse((_, i, v))) = heap.pop() {
        if count <= min {
            break;
        }
        if !alive[i] || v != version[i] {
            continue;
        }
        let Some(r) = fits[i] else { continue };
        let (p, q) = (prev[i], next[i]);
        anchors[p].outh = r.p1 - r.p0;
        anchors[q].inh = r.p2 - r.p3;
        alive[i] = false;
        next[p] = q;
        prev[q] = p;
        count -= 1;
        removed += 1;
        for j in [p, q] {
            version[j] += 1;
            fits[j] = None;
            if let Some((dev, r)) = evaluate(j, &anchors, &prev, &next, work)? {
                fits[j] = Some(r);
                heap.push(Reverse((OrdF64(dev), j, version[j])));
            }
        }
    }
    let kept: Vec<Anchor> = (0..n).filter(|i| alive[*i]).map(|i| anchors[i]).collect();
    Ok((Path { anchors: kept, closed: path.closed }, removed))
}

/// A total order on deviations for the heap.
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF64(f64);

impl Eq for OrdF64 {}

impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
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
