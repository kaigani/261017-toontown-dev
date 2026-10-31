//! Brush outlines (02 §3.4): the union of the pen tip swept along the spine at the local width
//! `width(p) = min + p × (max − min)`, so self-overlaps form one clean outline without internal edges; a spine of
//! one anchor is a single dab.
//!
//! The sweep places the tip at samples along the spine and takes, for each pair of consecutive samples, the convex
//! hull of the two placed tips (the exact Minkowski sweep of a convex tip along a chord). The hulls are united on an
//! integer grid of 1/[`GRID`] du with `i_overlay` (integer arithmetic, so the union is identical on every platform)
//! hierarchically, [`UNION_CHUNK`] shapes per overlay call, so heavily overlapping tips never meet in one call; the
//! resulting contours are fitted back to Béziers within [`OUTLINE_TOLERANCE`] (or 1/16 of the pen width when that is
//! finer). The complete operation — placements, hulls, unions, bridges, densification and the final fit — is charged
//! to one work budget, [`OUTLINE_WORK_BUDGET`]; past it the outline is refused with `limit.exceeded` before any
//! further step starts (M2 C2.1 obligation 11, C21-R2-03). 10 §4.9 stores one closed path per stroke, so holes (a stroke that loops) and separate pieces are
//! joined to the outer contour by zero-width bridges; filled with the non-zero rule the bridge edges cancel (SI-053).

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use tt_model::entities::pen::Tip;

use crate::fit::{self, Sample, pressure_at};
use crate::geom::{P, signed_area};
use crate::num;
use crate::path::{GeomError, MAX_ANCHORS, Path};

/// Grid of the union: 1/64 du (coordinates up to ±10^7 du stay inside the engine's i32 range).
pub const GRID: f64 = 64.0;

/// Fitting tolerance of outline contours (du).
pub const OUTLINE_TOLERANCE: f64 = 0.1;

/// Most tip placements along one spine; a spine that needs more is refused with `limit.exceeded` (bounded work,
/// M2 C2.1 obligation 11).
pub const MAX_PLACEMENTS: usize = 100_000;

/// Shapes united per overlay call (the union is hierarchical: chunks, then chunks of their results). Small, because
/// the work of one call grows with its vertices times how many of its shapes cross each other: 64 nearly coincident
/// disks per call took 14 s for 99 000 alternating knots on a closed spine, 4 per call take 1.8 s (C21-R3 CI).
pub const UNION_CHUNK: usize = 4;

/// The work one complete brush geometry call may do (centre fit, placements, hulls, unions, bridges, densification
/// and the outline fit), in units of about one point operation: sample visits of both fits, placements, hull points,
/// overlay input vertices (charged [`OVERLAY_COST`] per vertex and shape of each call, before the call), bridge vertex
/// pairs and densified points. Calibrated on the adversarial corpus of `crates/vector/tests/c21_worst_case.rs`
/// (1.4–3.5 ns per unit in release, the upper end when all tips overlap), so an accepted call returns in a few
/// seconds at most, far inside the 30 s document watchdog; the corpus records every observation.
pub const OUTLINE_WORK_BUDGET: u64 = 1_200_000_000;

/// Units charged per input vertex and per shape of one overlay call (sorting and sweeping the vertices, and the
/// crossings of up to that many overlapping shapes).
const OVERLAY_COST: u64 = 16;

/// One outline's work account.
struct Work {
    used: u64,
    limit: u64,
}

impl Work {
    /// Charges `n` units, refusing once the budget is passed.
    fn spend(&mut self, n: u64) -> Result<(), GeomError> {
        self.used = self.used.saturating_add(n);
        if self.used > self.limit {
            return Err(Work::exceeded());
        }
        Ok(())
    }

    fn exceeded() -> GeomError {
        GeomError {
            code: "limit.exceeded",
            message: format!("sweeping this brush stroke needs more than {OUTLINE_WORK_BUDGET} units of work"),
            pointer: "/points".to_owned(),
        }
    }
}

/// The pen a brush stroke is swept with (02 §6.1, §13.4 `pen`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushPen {
    /// Tip shape.
    pub tip: Tip,
    /// Width at lowest pressure (du).
    pub min_size: f64,
    /// Width at full pressure (du).
    pub max_size: f64,
    /// Orientation of non-round tips (degrees, counter-clockwise).
    pub tip_angle: f64,
}

impl BrushPen {
    /// The width at pressure `p` (02 §3.4).
    #[must_use]
    pub fn width(&self, p: f64) -> f64 {
        self.min_size + p * (self.max_size - self.min_size)
    }
}

/// The tip outline of width `w` centred on the origin (counter-clockwise). Oval tips are 2:1 ellipses and flat tips
/// 5:1 rectangles, their long axis at the tip angle; square tips are squares of side `w` (SI-054).
#[must_use]
pub fn tip_polygon(pen: &BrushPen, w: f64) -> Vec<P> {
    let r = w / 2.0;
    if r <= 0.0 {
        return vec![P::default()];
    }
    match pen.tip {
        Tip::Round | Tip::Oval => {
            let (a, b) = if pen.tip == Tip::Round { (r, r) } else { (r, r / 2.0) };
            // Enough sides that the polygon deviates from the ellipse by at most 1/40 du (8–64 sides); the polygon is
            // circumscribed (vertices at r / cos(pi / sides)), so the tip polygon contains the exact tip.
            let err = 0.025_f64.min(a / 4.0);
            let sides = (num::PI / num::acos(1.0 - err / a)).ceil().clamp(8.0, 64.0) as u32;
            let grow = 1.0 / num::cos(num::PI / f64::from(sides));
            let (a, b) = (a * grow, b * grow);
            let rot = if pen.tip == Tip::Oval { num::rotation(pen.tip_angle) } else { crate::geom::Affine::IDENTITY };
            (0..sides)
                .map(|k| {
                    let u = num::dir(360.0 * f64::from(k) / f64::from(sides));
                    rot.apply(P::new(u.x * a, u.y * b))
                })
                .collect()
        }
        Tip::Square | Tip::Flat => {
            let (hx, hy) = if pen.tip == Tip::Square { (r, r) } else { (r, r / 5.0) };
            let rot = num::rotation(pen.tip_angle);
            [P::new(-hx, -hy), P::new(hx, -hy), P::new(hx, hy), P::new(-hx, hy)].iter().map(|p| rot.apply(*p)).collect()
        }
    }
}

/// Convex hull (Andrew's monotone chain), counter-clockwise, without collinear points.
fn hull(mut pts: Vec<P>) -> Vec<P> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let mut lower: Vec<P> = Vec::new();
    for p in &pts {
        while lower.len() >= 2
            && (lower[lower.len() - 1] - lower[lower.len() - 2]).cross(*p - lower[lower.len() - 2]) <= 0.0
        {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<P> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2
            && (upper[upper.len() - 1] - upper[upper.len() - 2]).cross(*p - upper[upper.len() - 2]) <= 0.0
        {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn to_int(p: P) -> IntPoint<i32> {
    // |p| ≤ 10^7 + 100 du, so |p × 64| < 2^30: inside the engine's range.
    IntPoint::new((p.x * GRID).round() as i32, (p.y * GRID).round() as i32)
}

fn from_int(p: IntPoint<i32>) -> P {
    P::new(f64::from(p.x) / GRID, f64::from(p.y) / GRID)
}

/// Flattening tolerance of the spine for tip placements: one grid step (1/64 du).
pub const PLACEMENT_TOLERANCE: f64 = 1.0 / GRID;

/// The arc-length position `target` (du) on a segment of length `len`: the parameter (bisection, 48 steps).
fn param_at_length(s: &crate::geom::Cubic, target: f64, len: f64) -> f64 {
    if target <= 0.0 {
        return 0.0;
    }
    if target >= len {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..48 {
        let mid = (lo + hi) / 2.0;
        if s.length_to(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Tip placements along the spine with their normalised arc-length positions: every vertex of the spine flattened
/// within [`PLACEMENT_TOLERANCE`] (curvature) and every knot of the pressure map. Between two consecutive placements
/// the spine is a chord and the width is linear, so the swept tip is exactly the convex hull of the two placed tips
/// (a convex tip scaled linearly along a segment); their union is the sweep.
///
/// # Errors
/// `limit.exceeded` when the spine needs more than [`MAX_PLACEMENTS`] placements (bounded work).
fn placements(spine: &Path, pressure: &[[f64; 2]]) -> Result<Vec<(P, f64, f64)>, GeomError> {
    let segs = spine.segments();
    let lens: Vec<f64> = segs.iter().map(crate::geom::Cubic::length).collect();
    let total: f64 = lens.iter().sum();
    let first = spine.anchors.first().map(|a| a.p).unwrap_or_default();
    if segs.is_empty() || total <= 0.0 {
        // A dab: the widest pressure of the map.
        let p = pressure.iter().map(|k| k[1]).fold(pressure_at(pressure, 0.0), f64::max);
        return Ok(vec![(first, 0.0, p)]);
    }
    // Curvature: uniform parameter steps from Wang's bound for the placement tolerance.
    let mut out: Vec<(P, f64, f64)> = vec![(first, 0.0, pressure_at(pressure, 0.0))];
    let mut acc = 0.0;
    for (s, len) in segs.iter().zip(&lens) {
        let dd1 = (s.p2 - s.p1 * 2.0 + s.p0).len();
        let dd2 = (s.p3 - s.p2 * 2.0 + s.p1).len();
        let m = dd1.max(dd2) * 6.0;
        // A segment whose control points lie within the tolerance of its chord is that chord within the tolerance
        // (convex hull property): its ends suffice, however its parameter speed varies (corner anchors have zero
        // handles, which Wang's bound would flatten into dozens of placements).
        let straight = chord_distance(s.p1, s.p0, s.p3) <= PLACEMENT_TOLERANCE
            && chord_distance(s.p2, s.p0, s.p3) <= PLACEMENT_TOLERANCE;
        let n = if m <= 0.0 || straight { 1.0 } else { (m / (8.0 * PLACEMENT_TOLERANCE)).sqrt().ceil().max(1.0) };
        if out.len() as f64 + n > MAX_PLACEMENTS as f64 {
            return Err(too_many());
        }
        let steps = n as u32;
        for i in 1..=steps {
            let u = f64::from(i) / n;
            let t = ((acc + s.length_to(u)) / total).clamp(0.0, 1.0);
            out.push((s.eval(u), t, pressure_at(pressure, t)));
        }
        acc += len;
    }
    // Pressure knots strictly inside the spine, at their arc-length positions.
    let mut starts = Vec::with_capacity(segs.len());
    let mut a = 0.0;
    for len in &lens {
        starts.push(a);
        a += len;
    }
    // Every knot is placed with its own pressure, so a step (several knots at one position) places both widths.
    for [t, p] in pressure {
        let target = t.clamp(0.0, 1.0) * total;
        let k = starts.partition_point(|s| *s <= target).saturating_sub(1);
        let u = param_at_length(&segs[k], target - starts[k], lens[k]);
        out.push((segs[k].eval(u), t.clamp(0.0, 1.0), *p));
    }
    if out.len() > MAX_PLACEMENTS {
        return Err(too_many());
    }
    // In order of arc length (stable: equal positions keep their order), consecutive duplicates removed.
    out.sort_by(|x, y| x.1.total_cmp(&y.1));
    out.dedup_by(|b, a| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    Ok(out)
}

/// Distance from `p` to the segment `a`–`b`.
fn chord_distance(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

fn too_many() -> GeomError {
    GeomError {
        code: "limit.exceeded",
        message: format!("the brush stroke needs more than {MAX_PLACEMENTS} tip placements"),
        pointer: "/points".to_owned(),
    }
}

/// The outline of a brush stroke: one closed path (02 §3.4; SI-053 for holes).
///
/// # Errors
/// `drawing.invalidGeometry` when the swept area is empty (zero width throughout); `limit.exceeded` beyond
/// [`MAX_PLACEMENTS`] tip placements or [`OUTLINE_WORK_BUDGET`] units of work.
pub fn outline(spine: &Path, pressure: &[[f64; 2]], pen: &BrushPen) -> Result<Path, GeomError> {
    outline_within(spine, pressure, pen, OUTLINE_WORK_BUDGET).map(|(p, _)| p)
}

/// [`outline`] and the units of work it used (the worst-case corpus records them).
///
/// # Errors
/// As [`outline`].
pub fn outline_measured(spine: &Path, pressure: &[[f64; 2]], pen: &BrushPen) -> Result<(Path, u64), GeomError> {
    outline_within(spine, pressure, pen, OUTLINE_WORK_BUDGET)
}

/// [`outline`] within `budget` units (a brush stroke passes what its centre fit left of [`OUTLINE_WORK_BUDGET`]),
/// with the units used.
///
/// # Errors
/// As [`outline`]; `limit.exceeded` past `budget`, before the step that would pass it starts, or when the outline
/// needs more than [`MAX_ANCHORS`] anchors.
pub fn outline_within(
    spine: &Path,
    pressure: &[[f64; 2]],
    pen: &BrushPen,
    budget: u64,
) -> Result<(Path, u64), GeomError> {
    let mut work = Work { used: 0, limit: budget };
    let places = placements(spine, pressure)?;
    work.spend(places.len() as u64)?;
    let tips: Vec<Vec<P>> =
        places.iter().map(|(q, _, p)| tip_polygon(pen, pen.width(*p)).into_iter().map(|v| v + *q).collect()).collect();
    work.spend(tips.iter().map(|t| t.len() as u64).sum::<u64>().saturating_mul(4))?;
    let to_ints = |poly: Vec<P>| {
        let mut ints: Vec<IntPoint<i32>> = poly.into_iter().map(to_int).collect();
        ints.dedup();
        ints
    };
    let mut hulls: Vec<Vec<Vec<IntPoint<i32>>>> = Vec::with_capacity(tips.len());
    if tips.len() == 1 {
        hulls.push(vec![to_ints(hull(tips[0].clone()))]);
    } else {
        for w in tips.windows(2) {
            let mut both = w[0].clone();
            both.extend_from_slice(&w[1]);
            hulls.push(vec![to_ints(hull(both))]);
        }
    }
    let united = union(hulls, &mut work)?;
    let mut contours: Vec<Vec<P>> = Vec::new();
    for contour in united {
        let c: Vec<P> = straightened(contour).into_iter().map(from_int).collect();
        if c.len() >= 3 && signed_area(&c) != 0.0 {
            contours.push(c);
        }
    }
    let Some(merged) = bridge(contours, &mut work)? else {
        return Err(GeomError::invalid("/pen", "the brush sweeps no area (zero width)"));
    };
    let tol = OUTLINE_TOLERANCE.min(pen.max_size / 16.0).max(1.0 / GRID);
    // Long straight edges (square and flat tips, bridges) are subdivided so the fit is constrained between their ends
    // and cannot bulge away from the polygon between sparse vertices.
    let step = (4.0 * tol).max(0.25);
    let mut dense: Vec<P> = Vec::with_capacity(merged.len());
    for (i, a) in merged.iter().enumerate() {
        let b = merged[(i + 1) % merged.len()];
        let n = (a.dist(b) / step).floor().min(4096.0) as u32;
        work.spend(u64::from(n.max(1)))?;
        dense.push(*a);
        for k in 1..n {
            dense.push(a.lerp(b, f64::from(k) / f64::from(n)));
        }
    }
    let samples: Vec<Sample> = dense.iter().map(|p| Sample { p: *p, pressure: 1.0 }).collect();
    let left = work.limit.saturating_sub(work.used);
    let (f, used) = fit::fit_with_bounded(&samples, tol, 0.0, true, left).map_err(|_| Work::exceeded())?;
    work.spend(used)?;
    if f.path.anchors.len() > MAX_ANCHORS {
        return Err(GeomError {
            code: "limit.exceeded",
            message: format!("the brush outline needs more than {MAX_ANCHORS} anchors"),
            pointer: "/points".to_owned(),
        });
    }
    Ok((f.path, work.used))
}

/// The non-zero union of shapes (each a list of contours with the overlay's orientation: holes reversed), united
/// [`UNION_CHUNK`] at a time and then the results again until one remains; every call is charged before it runs.
/// The point set is the same as one call over all shapes (union is associative and exact on the integer grid).
fn union(mut level: Vec<Vec<Vec<IntPoint<i32>>>>, work: &mut Work) -> Result<Vec<Vec<IntPoint<i32>>>, GeomError> {
    loop {
        let mut next: Vec<Vec<Vec<IntPoint<i32>>>> = Vec::with_capacity(level.len().div_ceil(UNION_CHUNK));
        for chunk in level.chunks(UNION_CHUNK) {
            let vertices: usize = chunk.iter().flatten().map(Vec::len).sum();
            work.spend((vertices as u64).saturating_mul(chunk.len() as u64).saturating_mul(OVERLAY_COST))?;
            let mut overlay: Overlay<i32> = Overlay::new(vertices);
            for contour in chunk.iter().flatten() {
                if contour.len() >= 3 {
                    overlay.add_contour(contour, ShapeType::Subject);
                }
            }
            let shapes = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
            next.push(shapes.into_iter().flatten().collect());
        }
        if next.len() <= 1 {
            return Ok(next.into_iter().flatten().collect());
        }
        level = next;
    }
}

/// A contour without its exactly collinear vertices (integer cross product zero, same direction): a vertex where an
/// intermediate union split a boundary edge. So the outline depends only on the united point set, not on how the
/// hierarchical union grouped the shapes.
fn straightened(contour: Vec<IntPoint<i32>>) -> Vec<IntPoint<i32>> {
    let mut c = contour;
    loop {
        let n = c.len();
        if n < 4 {
            return c;
        }
        let keep: Vec<bool> = (0..n)
            .map(|i| {
                let (a, b, d) = (c[(i + n - 1) % n], c[i], c[(i + 1) % n]);
                let (ux, uy) = (i64::from(b.x) - i64::from(a.x), i64::from(b.y) - i64::from(a.y));
                let (vx, vy) = (i64::from(d.x) - i64::from(b.x), i64::from(d.y) - i64::from(b.y));
                !(ux * vy - uy * vx == 0 && ux * vx + uy * vy > 0)
            })
            .collect();
        if keep.iter().all(|k| *k) {
            return c;
        }
        c = c.into_iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| p).collect();
    }
}

/// Joins contours into one closed polygon with zero-width bridges (largest contour first; each further contour is
/// spliced in at its vertex nearest to the merged polygon).
fn bridge(mut contours: Vec<Vec<P>>, work: &mut Work) -> Result<Option<Vec<P>>, GeomError> {
    if contours.is_empty() {
        return Ok(None);
    }
    // Largest |area| first; ties broken by the first vertex so the order is a pure function of the geometry.
    contours.sort_by(|a, b| {
        signed_area(b)
            .abs()
            .total_cmp(&signed_area(a).abs())
            .then(a[0].x.total_cmp(&b[0].x))
            .then(a[0].y.total_cmp(&b[0].y))
    });
    let mut it = contours.into_iter();
    let Some(mut merged) = it.next() else { return Ok(None) };
    for c in it {
        // The nearest pair (merged vertex, contour vertex), first in merged order then contour order on ties. Only
        // merged vertices within d0 — the distance from the contour's first vertex to the merged polygon's nearest
        // vertex, an upper bound of the minimum — of the contour's box can take part, so the exact search runs over
        // those candidates only (C2.2: the splice is linear in the merged polygon per contour, not quadratic).
        work.spend((merged.len() as u64).saturating_mul(2))?;
        let d0 = merged.iter().map(|m| m.dist(c[0])).fold(f64::INFINITY, f64::min);
        let (bx0, by0, bx1, by1) =
            c.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
                (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y))
            });
        let near = |m: &P| {
            let dx = (bx0 - m.x).max(0.0).max(m.x - bx1);
            let dy = (by0 - m.y).max(0.0).max(m.y - by1);
            (dx * dx + dy * dy).sqrt() <= d0
        };
        let candidates: Vec<usize> = (0..merged.len()).filter(|&i| near(&merged[i])).collect();
        work.spend((candidates.len() as u64).saturating_mul(c.len() as u64))?;
        let mut best = (f64::INFINITY, 0, 0);
        for &i in &candidates {
            let m = merged[i];
            for (j, p) in c.iter().enumerate() {
                let d = m.dist(*p);
                if d < best.0 {
                    best = (d, i, j);
                }
            }
        }
        let (_, i, j) = best;
        let mut next: Vec<P> = Vec::with_capacity(merged.len() + c.len() + 2);
        next.extend_from_slice(&merged[..=i]);
        next.extend(c[j..].iter().chain(&c[..j]).copied());
        next.push(c[j]);
        next.push(merged[i]);
        next.extend_from_slice(&merged[i + 1..]);
        merged = next;
    }
    Ok(Some(merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Anchor;

    fn pen(min: f64, max: f64) -> BrushPen {
        BrushPen { tip: Tip::Round, min_size: min, max_size: max, tip_angle: 0.0 }
    }

    #[test]
    fn a_straight_stroke_outlines_a_capsule() {
        let spine =
            Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(100.0, 0.0))], closed: false };
        let o = outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen(2.0, 8.0)).unwrap();
        let b = o.bounds().unwrap();
        assert!((b.min_x + 4.0).abs() < 0.2 && (b.max_x - 104.0).abs() < 0.2, "{b:?}");
        assert!((b.max_y - 4.0).abs() < 0.2 && (b.min_y + 4.0).abs() < 0.2, "{b:?}");
        assert!(o.closed);
    }

    #[test]
    fn pressure_varies_the_width() {
        let spine =
            Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(100.0, 0.0))], closed: false };
        let o = outline(&spine, &[[0.0, 0.0], [1.0, 1.0]], &pen(2.0, 10.0)).unwrap();
        let poly = o.flatten(0.01);
        let near_start = poly.iter().filter(|p| p.x.abs() < 1.0).map(|p| p.y.abs()).fold(0.0, f64::max);
        let near_end = poly.iter().filter(|p| (p.x - 100.0).abs() < 1.0).map(|p| p.y.abs()).fold(0.0, f64::max);
        assert!(near_start < 1.5 && near_end > 4.5, "{near_start} {near_end}");
    }

    #[test]
    fn a_dab_is_one_tip() {
        let spine = Path { anchors: vec![Anchor::corner(P::new(10.0, 10.0))], closed: true };
        let o = outline(&spine, &[[0.0, 1.0]], &pen(4.0, 4.0)).unwrap();
        let b = o.bounds().unwrap();
        assert!((b.max_x - 12.0).abs() < 0.1 && (b.min_x - 8.0).abs() < 0.1, "{b:?}");
    }

    #[test]
    fn a_looping_stroke_is_one_clean_closed_path() {
        let pts: Vec<Sample> = (0..=80)
            .map(|i| {
                let a = f64::from(i) / 72.0 * 2.0 * num::PI;
                Sample { p: P::new(num::cos(a) * 30.0, num::sin(a) * 30.0), pressure: 1.0 }
            })
            .collect();
        let spine = fit::fit(&pts, 2.0, false).path;
        let o = outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen(4.0, 4.0)).unwrap();
        assert!(o.closed && o.anchors.len() > 4);
    }

    #[test]
    fn zero_width_is_invalid_geometry() {
        let spine =
            Path { anchors: vec![Anchor::corner(P::new(0.0, 0.0)), Anchor::corner(P::new(10.0, 0.0))], closed: false };
        assert_eq!(outline(&spine, &[[0.0, 1.0]], &pen(0.0, 0.0)).unwrap_err().code, "drawing.invalidGeometry");
    }

    #[test]
    fn outlines_are_reproducible() {
        let spine = Path {
            anchors: vec![
                Anchor { p: P::new(0.0, 0.0), inh: P::default(), outh: P::new(30.0, 40.0) },
                Anchor { p: P::new(90.0, 10.0), inh: P::new(-20.0, 30.0), outh: P::default() },
            ],
            closed: false,
        };
        let p = BrushPen { tip: Tip::Flat, min_size: 1.0, max_size: 12.0, tip_angle: 30.0 };
        let a = outline(&spine, &[[0.0, 0.2], [0.5, 1.0], [1.0, 0.4]], &p).unwrap();
        let b = outline(&spine, &[[0.0, 0.2], [0.5, 1.0], [1.0, 0.4]], &p).unwrap();
        assert_eq!(a, b);
    }
}
