//! Glyph shapes of a text object (02 §8.7, §13.5 render; M2 C2.3): each visible character's outline placed where the
//! text layout puts it (`text::layout_placed`), in the text box's local frame. Break Text Apart turns them into brush
//! shapes; the renderer fills them. A glyph's contours are joined into one closed path by zero-width bridges (SI-053),
//! keeping their curves, so the non-zero rule renders counters (holes) as holes.

use crate::fonts;
use crate::geom::P;
use crate::path::{Anchor, Path};
use crate::text::{self, Content};

/// One character's outline.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphShape {
    /// Code point index in the content.
    pub index: usize,
    /// The run's colour.
    pub colour_id: Option<String>,
    /// Closed contours in the text box's local frame (du, y up).
    pub contours: Vec<Path>,
}

fn contour_path(segs: &[[f64; 8]], at: P, size: f64) -> Option<Path> {
    let n = segs.len();
    if n < 2 {
        return None;
    }
    let map = |x: f64, y: f64| P::new(at.x + x * size, at.y + y * size);
    let anchors = (0..n)
        .map(|i| {
            let s = segs[i];
            let prev = segs[(i + n - 1) % n];
            let p = map(s[0], s[1]);
            Anchor { p, inh: map(prev[4], prev[5]) - p, outh: map(s[2], s[3]) - p }
        })
        .collect();
    Some(Path { anchors, closed: true })
}

/// The glyph shapes of a text object laid out at `fixed_width` (auto width when `None`). Characters without a glyph
/// in the resolved face, and blank glyphs, have no shape.
#[must_use]
pub fn shapes(c: &Content, fixed_width: Option<f64>) -> Vec<GlyphShape> {
    let (_, placed) = text::layout_placed(c, fixed_width);
    let mut out = Vec::new();
    for g in placed {
        let r = c.run_at(g.index);
        let face = fonts::resolve(&r.font, r.bold, r.italic).face;
        let Some(contours) = face.outline(g.ch) else { continue };
        let paths: Vec<Path> = contours.iter().filter_map(|k| contour_path(k, P::new(g.x, g.y), g.size)).collect();
        if !paths.is_empty() {
            out.push(GlyphShape { index: g.index, colour_id: r.colour_id.clone(), contours: paths });
        }
    }
    out
}

fn area(p: &Path) -> f64 {
    let pts = p.flatten(0.5);
    let n = pts.len();
    (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum::<f64>() / 2.0
}

/// Joins closed contours into one closed path with zero-width bridges between anchors (largest contour first; each
/// further contour spliced in at the anchor pair nearest to the merged path), keeping every curve. `None` for none.
#[must_use]
pub fn bridge_paths(contours: &[Path]) -> Option<Path> {
    let mut list: Vec<&Path> = contours.iter().filter(|c| c.anchors.len() >= 2).collect();
    list.sort_by(|a, b| {
        area(b)
            .abs()
            .total_cmp(&area(a).abs())
            .then(a.anchors[0].p.x.total_cmp(&b.anchors[0].p.x))
            .then(a.anchors[0].p.y.total_cmp(&b.anchors[0].p.y))
    });
    let mut it = list.into_iter();
    let mut merged: Vec<Anchor> = it.next()?.anchors.clone();
    for c in it {
        let mut best = (f64::INFINITY, 0, 0);
        for (i, m) in merged.iter().enumerate() {
            for (j, a) in c.anchors.iter().enumerate() {
                let d = m.p.dist(a.p);
                if d < best.0 {
                    best = (d, i, j);
                }
            }
        }
        let (_, i, j) = best;
        let k = c.anchors.len();
        let mut next: Vec<Anchor> = Vec::with_capacity(merged.len() + k + 2);
        next.extend_from_slice(&merged[..i]);
        // The merged anchor, its out-handle kept for after the bridge back.
        next.push(Anchor { p: merged[i].p, inh: merged[i].inh, outh: P::default() });
        for q in 0..k {
            let a = c.anchors[(j + q) % k];
            next.push(if q == 0 { Anchor { p: a.p, inh: P::default(), outh: a.outh } } else { a });
        }
        next.push(Anchor { p: c.anchors[j].p, inh: c.anchors[j].inh, outh: P::default() });
        next.push(Anchor { p: merged[i].p, inh: P::default(), outh: merged[i].outh });
        next.extend_from_slice(&merged[i + 1..]);
        merged = next;
    }
    Some(Path { anchors: merged, closed: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64, ccw: bool) -> Path {
        let mut pts = vec![P::new(x, y), P::new(x + s, y), P::new(x + s, y + s), P::new(x, y + s)];
        if !ccw {
            pts.reverse();
        }
        Path { anchors: pts.into_iter().map(Anchor::corner).collect(), closed: true }
    }

    #[test]
    fn bridging_keeps_both_contours_and_their_anchor_count() {
        let outer = square(0.0, 0.0, 10.0, true);
        let hole = square(3.0, 3.0, 4.0, false);
        let b = bridge_paths(&[hole.clone(), outer.clone()]).unwrap_or(Path { anchors: vec![], closed: true });
        assert_eq!(b.anchors.len(), 4 + 4 + 2);
        assert!(b.closed);
        // The bridge edges are zero-width: the signed area is the outer minus the hole.
        assert!((area(&b) - (100.0 - 16.0)).abs() < 1e-9, "{}", area(&b));
    }

    #[test]
    fn placed_glyphs_follow_the_layout_and_carry_the_run_colour() {
        let c = Content::new("Ab", text::default_run(Some("col_ink".into())), text::default_para());
        let shapes = shapes(&c, None);
        // Machines without a readable face lay out with a metric-only fallback and have no outlines.
        if let Some(a) = shapes.first() {
            assert_eq!(a.colour_id.as_deref(), Some("col_ink"));
            assert!(!a.contours.is_empty());
            let (_, placed) = text::layout_placed(&c, None);
            assert_eq!(placed.len(), 2);
            assert!(placed[1].x > placed[0].x);
        }
    }
}
