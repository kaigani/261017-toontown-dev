//! Brush outlines (02 §3.4): the union of the pen tip swept along the spine at the local width
//! `width(p) = min + p × (max − min)`, so self-overlaps form one clean outline without internal edges; a spine of
//! one anchor is a single dab.
//!
//! The sweep places the tip at samples along the spine and takes, for each pair of consecutive samples, the convex
//! hull of the two placed tips (the exact Minkowski sweep of a convex tip along a chord). The hulls are united on an
//! integer grid of 1/[`GRID`] du with `i_overlay` (integer arithmetic, so the union is identical on every platform),
//! and the resulting contours are fitted back to Béziers within [`OUTLINE_TOLERANCE`] (or 1/16 of the pen width when
//! that is finer). 10 §4.9 stores one closed path per stroke, so holes (a stroke that loops) and separate pieces are
//! joined to the outer contour by zero-width bridges; filled with the non-zero rule the bridge edges cancel (SI-053).

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use tt_model::entities::pen::Tip;

use crate::fit::{self, Sample, pressure_at};
use crate::geom::{P, signed_area};
use crate::num;
use crate::path::{GeomError, Path};

/// Grid of the union: 1/64 du (coordinates up to ±10⁷ du stay inside the engine's i32 range).
pub const GRID: f64 = 64.0;

/// Fitting tolerance of outline contours (du).
pub const OUTLINE_TOLERANCE: f64 = 0.1;

/// Most tip placements along one spine (the step grows for very long strokes).
pub const MAX_PLACEMENTS: usize = 20_000;

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
            // Enough sides that the polygon deviates from the ellipse by at most 1/40 du (8–64 sides).
            let err = 0.025_f64.min(a / 4.0);
            let sides = (num::PI / num::acos(1.0 - err / a)).ceil().clamp(8.0, 64.0) as u32;
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
    // |p| ≤ 10⁷ + 100 du, so |p × 64| < 2³⁰: inside the engine's range.
    IntPoint::new((p.x * GRID).round() as i32, (p.y * GRID).round() as i32)
}

fn from_int(p: IntPoint<i32>) -> P {
    P::new(f64::from(p.x) / GRID, f64::from(p.y) / GRID)
}

/// Tip placements along the spine: positions with their normalised arc-length positions.
fn placements(spine: &Path, max_size: f64) -> Vec<(P, f64)> {
    let segs = spine.segments();
    let lens: Vec<f64> = segs.iter().map(crate::geom::Cubic::length).collect();
    let total: f64 = lens.iter().sum();
    let first = spine.anchors.first().map(|a| a.p).unwrap_or_default();
    if segs.is_empty() || total <= 0.0 {
        return vec![(first, 0.0)];
    }
    let mut step = (max_size / 4.0).clamp(0.25, 8.0);
    if total / step > MAX_PLACEMENTS as f64 {
        step = total / MAX_PLACEMENTS as f64;
    }
    let mut out = vec![(first, 0.0)];
    let mut acc = 0.0;
    for (s, len) in segs.iter().zip(&lens) {
        let n = (len / step).ceil().clamp(1.0, MAX_PLACEMENTS as f64) as u32;
        for i in 1..=n {
            let u = f64::from(i) / f64::from(n);
            let t = ((acc + s.length_to(u)) / total).clamp(0.0, 1.0);
            out.push((s.eval(u), t));
        }
        acc += len;
    }
    out
}

/// The outline of a brush stroke: one closed path (02 §3.4; SI-053 for holes).
///
/// # Errors
/// `drawing.invalidGeometry` when the swept area is empty (zero width throughout).
pub fn outline(spine: &Path, pressure: &[[f64; 2]], pen: &BrushPen) -> Result<Path, GeomError> {
    let places = placements(spine, pen.max_size);
    let tips: Vec<Vec<P>> = places
        .iter()
        .map(|(q, t)| tip_polygon(pen, pen.width(pressure_at(pressure, *t))).into_iter().map(|v| v + *q).collect())
        .collect();
    let mut overlay: Overlay<i32> = Overlay::new(tips.len() * 16);
    let mut add = |poly: Vec<P>| {
        let mut ints: Vec<IntPoint<i32>> = poly.into_iter().map(to_int).collect();
        ints.dedup();
        if ints.len() >= 3 {
            overlay.add_contour(&ints, ShapeType::Subject);
        }
    };
    if tips.len() == 1 {
        add(hull(tips[0].clone()));
    } else {
        for w in tips.windows(2) {
            let mut both = w[0].clone();
            both.extend_from_slice(&w[1]);
            add(hull(both));
        }
    }
    let shapes = overlay.overlay(OverlayRule::Subject, FillRule::NonZero);
    let mut contours: Vec<Vec<P>> = Vec::new();
    for shape in shapes {
        for contour in shape {
            let c: Vec<P> = contour.into_iter().map(from_int).collect();
            if c.len() >= 3 && signed_area(&c) != 0.0 {
                contours.push(c);
            }
        }
    }
    let Some(merged) = bridge(contours) else {
        return Err(GeomError::invalid("/pen", "the brush sweeps no area (zero width)"));
    };
    let tol = OUTLINE_TOLERANCE.min(pen.max_size / 16.0).max(1.0 / GRID);
    // Long straight edges (square and flat tips, bridges) are subdivided so the fit is constrained between their ends
    // and cannot bulge away from the polygon between sparse vertices.
    let step = (4.0 * tol).max(0.25);
    let mut dense: Vec<P> = Vec::with_capacity(merged.len());
    for (i, a) in merged.iter().enumerate() {
        let b = merged[(i + 1) % merged.len()];
        dense.push(*a);
        let n = (a.dist(b) / step).floor().min(4096.0) as u32;
        for k in 1..n {
            dense.push(a.lerp(b, f64::from(k) / f64::from(n)));
        }
    }
    let samples: Vec<Sample> = dense.iter().map(|p| Sample { p: *p, pressure: 1.0 }).collect();
    let f = fit::fit_with(&samples, tol, 0.0, true);
    Ok(f.path)
}

/// Joins contours into one closed polygon with zero-width bridges (largest contour first; each further contour is
/// spliced in at its vertex nearest to the merged polygon).
fn bridge(mut contours: Vec<Vec<P>>) -> Option<Vec<P>> {
    if contours.is_empty() {
        return None;
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
    let mut merged = it.next()?;
    for c in it {
        let mut best = (f64::INFINITY, 0, 0);
        for (i, m) in merged.iter().enumerate() {
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
    Some(merged)
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
