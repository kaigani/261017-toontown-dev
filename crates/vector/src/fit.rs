//! Curve fitting of freehand samples (02 §3.3).
//!
//! 1. Consecutive duplicate positions are dropped.
//! 2. Corners (direction changes sharper than [`CORNER_TURN_DEGREES`], measured between neighbours at least one
//!    tolerance away) split the samples into runs.
//! 3. Each run is fitted by least squares with chord-length parameters improved by Newton steps; when the largest
//!    error exceeds the tolerance the run is split at that sample and both halves are fitted recursively.
//! 4. The tolerance is `0.25 du + 0.5 du × smoothness` (§3.3 recommendation).
//! 5. Pressures are smoothed with a centred moving average of half-width `round(smoothness)` samples and stored as
//!    `(t, p)` with `t` the normalised arc length along the fitted spine.
//!
//! The result depends only on sample positions, pressures and settings (never timing), with a fixed operation order.

use crate::geom::{Cubic, P};
use crate::path::{Anchor, GeomError, MAX_ANCHORS, Path, check_coord};

/// Turns sharper than this make a corner.
pub const CORNER_TURN_DEGREES: f64 = 60.0;

/// The pressure-map simplification tolerance: samples whose pressure is within this of the linear interpolation of
/// their kept neighbours are dropped (Douglas–Peucker on `(t, p)`).
pub const PRESSURE_TOLERANCE: f64 = 1.0e-3;

/// One input sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Position (du).
    pub p: P,
    /// Pressure in [0, 1].
    pub pressure: f64,
}

/// The fitting tolerance for a smoothness 0–10 (02 §3.3 step 4).
#[must_use]
pub fn tolerance(smoothness: f64) -> f64 {
    0.25 + 0.5 * smoothness
}

/// A fitted spine.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    /// The path (one anchor for a dab).
    pub path: Path,
    /// The pressure map `(t, p)`, ordered by `t`, first `t = 0`, last `t = 1`.
    pub pressure: Vec<[f64; 2]>,
}

/// Validates samples (finite, bounded, pressure 0–1, count) and drops consecutive duplicates.
///
/// # Errors
/// `drawing.invalidGeometry` at `pointer/<i>/…`.
pub fn clean(samples: &[Sample], pointer: &str) -> Result<Vec<Sample>, GeomError> {
    if samples.is_empty() {
        return Err(GeomError::invalid(pointer, "at least one point is needed"));
    }
    if samples.len() > MAX_ANCHORS {
        return Err(GeomError::invalid(pointer, format!("at most {MAX_ANCHORS} points per stroke")));
    }
    let mut out: Vec<Sample> = Vec::with_capacity(samples.len());
    for (i, s) in samples.iter().enumerate() {
        check_coord(s.p.x, &format!("{pointer}/{i}/x"))?;
        check_coord(s.p.y, &format!("{pointer}/{i}/y"))?;
        if !(0.0..=1.0).contains(&s.pressure) {
            return Err(GeomError::invalid(format!("{pointer}/{i}/pressure"), "pressure is in [0, 1]"));
        }
        if out.last().is_some_and(|l| l.p == s.p) {
            continue;
        }
        out.push(*s);
    }
    Ok(out)
}

/// Fits cleaned samples (see [`clean`]); `closed` fits a loop back to the first sample.
#[must_use]
pub fn fit(samples: &[Sample], smoothness: f64, closed: bool) -> Fitted {
    fit_with(samples, tolerance(smoothness), smoothness, closed)
}

/// Fits cleaned samples with an explicit tolerance (du); `smoothness` sets the pressure smoothing window.
#[must_use]
pub fn fit_with(samples: &[Sample], tol: f64, smoothness: f64, closed: bool) -> Fitted {
    let pts: Vec<P> = samples.iter().map(|s| s.p).collect();
    if pts.len() <= 1 {
        let p = pts.first().copied().unwrap_or_default();
        let pr = samples.first().map_or(1.0, |s| s.pressure);
        return Fitted {
            path: Path { anchors: vec![Anchor::corner(p)], closed: true },
            pressure: vec![[0.0, pr], [1.0, pr]],
        };
    }
    let closed = closed && pts.len() >= 3;
    // The chain to fit, as indices into `pts` (a loop repeats its start; it starts at a corner when it has one).
    let corners = corners(&pts, tol, closed);
    let n = pts.len();
    let chain: Vec<usize> = if closed {
        let start = corners.first().copied().unwrap_or(0);
        (0..=n).map(|k| (start + k) % n).collect()
    } else {
        (0..n).collect()
    };
    let chain_pts: Vec<P> = chain.iter().map(|&i| pts[i]).collect();
    let mut breaks: Vec<usize> = vec![0];
    for (k, i) in chain.iter().enumerate().skip(1).take(chain.len().saturating_sub(2)) {
        if corners.contains(i) {
            breaks.push(k);
        }
    }
    breaks.push(chain_pts.len() - 1);
    let mut segs: Vec<Cubic> = Vec::new();
    // Per chain point: (segment index, parameter) once fitted.
    let mut params: Vec<(usize, f64)> = vec![(0, 0.0); chain_pts.len()];
    let smooth_loop = closed && corners.is_empty();
    for w in breaks.windows(2) {
        let (a, b) = (w[0], w[1]);
        let run = &chain_pts[a..=b];
        let (t1, t2) = if smooth_loop {
            let t = (chain_pts[1] - chain_pts[chain_pts.len() - 2]).unit().unwrap_or(P::new(1.0, 0.0));
            (t, -t)
        } else {
            (left_tangent(run), right_tangent(run))
        };
        let mut local: Vec<(usize, f64)> = vec![(0, 0.0); run.len()];
        fit_run(run, t1, t2, tol, 0, &mut segs, &mut local);
        for (k, l) in local.iter().enumerate() {
            params[a + k] = *l;
        }
    }
    let path = path_of(&segs, closed);
    let pressure = pressure_map(samples, &chain, &params, &segs, smoothness);
    Fitted { path, pressure }
}

/// Joins samples with straight segments (`fit: false`, 02 §13.1 item 6).
#[must_use]
pub fn polyline(samples: &[Sample], closed: bool, smoothness: f64) -> Fitted {
    let pts: Vec<P> = samples.iter().map(|s| s.p).collect();
    if pts.len() <= 1 {
        return fit(samples, smoothness, closed);
    }
    let closed = closed && pts.len() >= 3;
    let mut segs: Vec<Cubic> = pts.windows(2).map(|w| Cubic::line(w[0], w[1])).collect();
    if closed {
        segs.push(Cubic::line(pts[pts.len() - 1], pts[0]));
    }
    let chain: Vec<usize> = (0..pts.len()).collect();
    let params: Vec<(usize, f64)> =
        (0..pts.len()).map(|i| if i + 1 == pts.len() && !closed { (i - 1, 1.0) } else { (i, 0.0) }).collect();
    let path = Path { anchors: pts.iter().map(|p| Anchor::corner(*p)).collect(), closed };
    let pressure = pressure_map(samples, &chain, &params, &segs, smoothness);
    Fitted { path, pressure }
}

fn path_of(segs: &[Cubic], closed: bool) -> Path {
    let mut anchors: Vec<Anchor> = Vec::with_capacity(segs.len() + 1);
    for s in segs {
        if let Some(last) = anchors.last_mut() {
            last.outh = s.p1 - s.p0;
        } else {
            anchors.push(Anchor { p: s.p0, inh: P::default(), outh: s.p1 - s.p0 });
        }
        anchors.push(Anchor { p: s.p3, inh: s.p2 - s.p3, outh: P::default() });
    }
    if closed && anchors.len() >= 2 {
        // The last anchor is the first one again: its incoming handle moves to the first anchor.
        if let Some(last) = anchors.pop() {
            anchors[0].inh = last.inh;
        }
    }
    Path { anchors, closed }
}

/// Indices of corner samples.
fn corners(pts: &[P], tol: f64, closed: bool) -> Vec<usize> {
    let n = pts.len();
    let cos_limit = crate::num::cos(crate::num::rad(CORNER_TURN_DEGREES));
    let mut cand: Vec<(usize, f64)> = Vec::new();
    let reach = |i: usize, back: bool| -> Option<P> {
        // The nearest sample at least `tol` away in one direction (wrapping for loops), else the farthest one.
        let mut last = None;
        for step in 1..n {
            let j = if back {
                if closed {
                    (i + n - step) % n
                } else if step > i {
                    break;
                } else {
                    i - step
                }
            } else if closed {
                (i + step) % n
            } else if i + step >= n {
                break;
            } else {
                i + step
            };
            last = Some(pts[j]);
            if pts[j].dist(pts[i]) >= tol {
                return Some(pts[j]);
            }
        }
        last
    };
    let range: Vec<usize> = if closed { (0..n).collect() } else { (1..n.saturating_sub(1)).collect() };
    for i in range {
        let (Some(a), Some(b)) = (reach(i, true), reach(i, false)) else { continue };
        let (Some(u), Some(v)) = ((pts[i] - a).unit(), (b - pts[i]).unit()) else { continue };
        let c = u.dot(v);
        if c < cos_limit {
            cand.push((i, c));
        }
    }
    // Neighbouring candidates are one corner: keep the sharpest of each consecutive run.
    let mut out: Vec<usize> = Vec::new();
    let mut k = 0;
    while k < cand.len() {
        let mut best = cand[k];
        let mut j = k + 1;
        while j < cand.len() && cand[j].0 == cand[j - 1].0 + 1 {
            if cand[j].1 < best.1 {
                best = cand[j];
            }
            j += 1;
        }
        out.push(best.0);
        k = j;
    }
    out
}

fn left_tangent(run: &[P]) -> P {
    (run[1] - run[0]).unit().unwrap_or(P::new(1.0, 0.0))
}

fn right_tangent(run: &[P]) -> P {
    let n = run.len();
    (run[n - 2] - run[n - 1]).unit().unwrap_or(P::new(-1.0, 0.0))
}

fn chord_params(run: &[P]) -> Vec<f64> {
    let mut u = vec![0.0; run.len()];
    for i in 1..run.len() {
        u[i] = u[i - 1] + run[i].dist(run[i - 1]);
    }
    let total = u[run.len() - 1];
    if total > 0.0 {
        for x in &mut u {
            *x /= total;
        }
    }
    u
}

fn generate(run: &[P], u: &[f64], t1: P, t2: P) -> Cubic {
    let first = run[0];
    let last = run[run.len() - 1];
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, &t) in run.iter().zip(u) {
        let mt = 1.0 - t;
        let b0 = mt * mt * mt;
        let b1 = 3.0 * t * mt * mt;
        let b2 = 3.0 * t * t * mt;
        let b3 = t * t * t;
        let a1 = t1 * b1;
        let a2 = t2 * b2;
        c00 += a1.dot(a1);
        c01 += a1.dot(a2);
        c11 += a2.dot(a2);
        let tmp = *p - (first * (b0 + b1) + last * (b2 + b3));
        x0 += a1.dot(tmp);
        x1 += a2.dot(tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = first.dist(last);
    let eps = 1.0e-6 * seg;
    let (mut al, mut ar) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (seg / 3.0, seg / 3.0)
    };
    if !(al > eps && ar > eps) || !al.is_finite() || !ar.is_finite() {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    Cubic { p0: first, p1: first + t1 * al, p2: last + t2 * ar, p3: last }
}

fn max_error(run: &[P], c: &Cubic, u: &[f64]) -> (f64, usize) {
    let mut worst = 0.0;
    let mut at = run.len() / 2;
    for i in 1..run.len() - 1 {
        let d = c.eval(u[i]).dist(run[i]);
        if d > worst {
            worst = d;
            at = i;
        }
    }
    (worst, at)
}

fn reparameterize(run: &[P], c: &Cubic, u: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = u
        .iter()
        .zip(run)
        .map(|(&t, p)| {
            let q = c.eval(t) - *p;
            let d1 = c.deriv(t);
            let d2 = c.deriv2(t);
            let num = q.dot(d1);
            let den = d1.dot(d1) + q.dot(d2);
            if den.abs() < 1e-12 { t } else { (t - num / den).clamp(0.0, 1.0) }
        })
        .collect();
    // Keep the parameters ordered (Newton can swap neighbours on noisy input).
    for i in 1..out.len() {
        if out[i] < out[i - 1] {
            out[i] = out[i - 1];
        }
    }
    out
}

fn fit_run(run: &[P], t1: P, t2: P, tol: f64, depth: u32, segs: &mut Vec<Cubic>, params: &mut [(usize, f64)]) {
    if run.len() == 2 {
        let d = run[0].dist(run[1]) / 3.0;
        let c = Cubic { p0: run[0], p1: run[0] + t1 * d, p2: run[1] + t2 * d, p3: run[1] };
        params[0] = (segs.len(), 0.0);
        params[1] = (segs.len(), 1.0);
        segs.push(c);
        return;
    }
    let mut u = chord_params(run);
    let mut c = generate(run, &u, t1, t2);
    let (mut err, mut split) = max_error(run, &c, &u);
    if err > tol && err < 4.0 * tol {
        for _ in 0..4 {
            let u2 = reparameterize(run, &c, &u);
            let c2 = generate(run, &u2, t1, t2);
            let (e2, s2) = max_error(run, &c2, &u2);
            u = u2;
            c = c2;
            err = e2;
            split = s2;
            if err <= tol {
                break;
            }
        }
    }
    if err <= tol || depth >= 40 {
        for (i, t) in u.iter().enumerate() {
            params[i] = (segs.len(), *t);
        }
        segs.push(c);
        return;
    }
    let split = split.clamp(1, run.len() - 2);
    let center =
        (run[split - 1] - run[split + 1]).unit().unwrap_or_else(|| (run[split - 1] - run[split]).unit().unwrap_or(t2));
    let (left, right) = params.split_at_mut(split);
    let mut lp = vec![(0, 0.0); split + 1];
    fit_run(&run[..=split], t1, center, tol, depth + 1, segs, &mut lp);
    left.copy_from_slice(&lp[..split]);
    fit_run(&run[split..], -center, t2, tol, depth + 1, segs, right);
}

/// The pressure map: smoothed pressures at the normalised arc length of each sample along the fitted spine.
fn pressure_map(
    samples: &[Sample],
    chain: &[usize],
    params: &[(usize, f64)],
    segs: &[Cubic],
    smoothness: f64,
) -> Vec<[f64; 2]> {
    let half = (smoothness + 0.5).floor().clamp(0.0, 10.0) as usize;
    let raw: Vec<f64> = samples.iter().map(|s| s.pressure).collect();
    let smooth: Vec<f64> = (0..raw.len())
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half).min(raw.len() - 1);
            let mut sum = 0.0;
            for v in &raw[lo..=hi] {
                sum += v;
            }
            sum / ((hi - lo + 1) as f64)
        })
        .collect();
    let mut start = vec![0.0; segs.len() + 1];
    for (i, s) in segs.iter().enumerate() {
        start[i + 1] = start[i] + s.length();
    }
    let total = start[segs.len()];
    let mut map: Vec<[f64; 2]> = Vec::with_capacity(chain.len());
    let mut last_t = 0.0;
    for (k, &i) in chain.iter().enumerate() {
        let (s, u) = params[k];
        let t = if total > 0.0 { ((start[s] + segs[s].length_to(u)) / total).clamp(0.0, 1.0) } else { 0.0 };
        let t = if t < last_t { last_t } else { t };
        last_t = t;
        map.push([t, smooth[i].clamp(0.0, 1.0)]);
    }
    if let Some(f) = map.first_mut() {
        f[0] = 0.0;
    }
    if let Some(l) = map.last_mut() {
        l[0] = 1.0;
    }
    simplify_pressure(&map)
}

/// Douglas–Peucker on `(t, p)` with [`PRESSURE_TOLERANCE`] on `p` (iterative; fixed order).
#[must_use]
pub fn simplify_pressure(map: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if map.len() <= 2 {
        return map.to_vec();
    }
    let mut keep = vec![false; map.len()];
    keep[0] = true;
    keep[map.len() - 1] = true;
    let mut stack = vec![(0usize, map.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (ta, pa) = (map[a][0], map[a][1]);
        let (tb, pb) = (map[b][0], map[b][1]);
        let mut worst = 0.0;
        let mut at = 0;
        for (i, m) in map.iter().enumerate().take(b).skip(a + 1) {
            let lin = if tb > ta { pa + (pb - pa) * ((m[0] - ta) / (tb - ta)) } else { pa };
            let d = (m[1] - lin).abs();
            if d > worst {
                worst = d;
                at = i;
            }
        }
        if worst > PRESSURE_TOLERANCE {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    map.iter().zip(keep).filter(|(_, k)| *k).map(|(m, _)| *m).collect()
}

/// The pressure at `t` (linear interpolation of the map, 02 §3.2).
#[must_use]
pub fn pressure_at(map: &[[f64; 2]], t: f64) -> f64 {
    match map {
        [] => 1.0,
        [one] => one[1],
        _ => {
            if t <= map[0][0] {
                return map[0][1];
            }
            for w in map.windows(2) {
                let ([t0, p0], [t1, p1]) = (w[0], w[1]);
                if t <= t1 {
                    return if t1 > t0 { p0 + (p1 - p0) * ((t - t0) / (t1 - t0)) } else { p1 };
                }
            }
            map[map.len() - 1][1]
        }
    }
}

/// Validates a given pressure map (02 §3.2: `(t, p)` in [0, 1], ordered by `t`).
///
/// # Errors
/// `drawing.invalidGeometry`.
pub fn check_pressure(map: &[[f64; 2]], pointer: &str) -> Result<(), GeomError> {
    if map.is_empty() {
        return Err(GeomError::invalid(pointer, "a pressure map has at least one sample"));
    }
    if map.len() > MAX_ANCHORS {
        return Err(GeomError::invalid(pointer, format!("at most {MAX_ANCHORS} pressure samples")));
    }
    let mut last = 0.0;
    for (i, [t, p]) in map.iter().enumerate() {
        if !(0.0..=1.0).contains(t) || !(0.0..=1.0).contains(p) || *t < last {
            return Err(GeomError::invalid(
                format!("{pointer}/{i}"),
                "pressure samples are (t, p) in [0, 1], ordered by t",
            ));
        }
        last = *t;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(pts: &[(f64, f64)]) -> Vec<Sample> {
        pts.iter().map(|&(x, y)| Sample { p: P::new(x, y), pressure: 1.0 }).collect()
    }

    #[test]
    fn a_straight_drag_fits_one_segment() {
        let s: Vec<(f64, f64)> = (0..50).map(|i| (f64::from(i), 0.0)).collect();
        let f = fit(&clean(&samples(&s), "/points").unwrap(), 3.0, false);
        assert_eq!(f.path.anchors.len(), 2);
        assert_eq!(f.path.anchors[1].p, P::new(49.0, 0.0));
    }

    #[test]
    fn a_square_keeps_its_corners() {
        let mut s = Vec::new();
        for i in 0..10 {
            s.push((f64::from(i) * 10.0, 0.0));
        }
        for i in 0..10 {
            s.push((100.0, f64::from(i) * 10.0));
        }
        for i in 0..=10 {
            s.push((100.0 - f64::from(i) * 10.0, 100.0));
        }
        let f = fit(&clean(&samples(&s), "/points").unwrap(), 0.0, false);
        let anchors: Vec<P> = f.path.anchors.iter().map(|a| a.p).collect();
        assert!(anchors.contains(&P::new(100.0, 0.0)), "{anchors:?}");
        assert!(anchors.contains(&P::new(100.0, 100.0)), "{anchors:?}");
    }

    #[test]
    fn fitting_error_is_within_tolerance() {
        let s: Vec<(f64, f64)> = (0..200)
            .map(|i| {
                let a = f64::from(i) / 199.0 * 3.0;
                (a * 40.0, crate::num::sin(a) * 30.0)
            })
            .collect();
        for smooth in [0.0, 3.0, 10.0] {
            let cleaned = clean(&samples(&s), "/p").unwrap();
            let f = fit(&cleaned, smooth, false);
            let poly = f.path.flatten(0.01);
            for p in &cleaned {
                let d = poly.windows(2).map(|w| seg_dist(p.p, w[0], w[1])).fold(f64::INFINITY, f64::min);
                assert!(d <= tolerance(smooth) * 1.05 + 0.02, "smoothness {smooth}: {d}");
            }
        }
    }

    fn seg_dist(p: P, a: P, b: P) -> f64 {
        let ab = b - a;
        let l = ab.dot(ab);
        let t = if l > 0.0 { ((p - a).dot(ab) / l).clamp(0.0, 1.0) } else { 0.0 };
        p.dist(a + ab * t)
    }

    #[test]
    fn pressure_maps_are_ordered_and_span_the_spine() {
        let s: Vec<Sample> =
            (0..30).map(|i| Sample { p: P::new(f64::from(i), 0.0), pressure: f64::from(i) / 29.0 }).collect();
        let f = fit(&s, 0.0, false);
        assert_eq!(f.pressure.first().unwrap()[0], 0.0);
        assert_eq!(f.pressure.last().unwrap()[0], 1.0);
        assert!(check_pressure(&f.pressure, "/p").is_ok());
        assert!((pressure_at(&f.pressure, 0.5) - 0.5).abs() < 0.02);
    }

    #[test]
    fn a_click_is_a_dab() {
        let f = fit(&clean(&samples(&[(5.0, 5.0), (5.0, 5.0)]), "/p").unwrap(), 3.0, false);
        assert_eq!(f.path.anchors.len(), 1);
        assert!(f.path.closed);
    }

    #[test]
    fn closed_loops_return_to_the_start() {
        let s: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = f64::from(i) / 64.0 * 2.0 * crate::num::PI;
                (crate::num::cos(a) * 50.0, crate::num::sin(a) * 50.0)
            })
            .collect();
        let f = fit(&clean(&samples(&s), "/p").unwrap(), 1.0, true);
        assert!(f.path.closed);
        assert!(f.path.anchors.len() >= 2);
    }
}
