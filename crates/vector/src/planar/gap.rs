//! Point tests and gap closing on a layer (03 §8.1, §8.6, §8.8; 02 §7.3.5).
//!
//! **Automatic gap closing** (paint, paint unpainted and unpaint by point, when the region under the point is not
//! closed): candidate closing segments join each open stroke end to the nearest other stroke end, and to the nearest
//! point of another stroke, when no farther than the tolerance. All candidates are inserted, the zones are found
//! again, and the region under the point must now be a zone of at least 4 × tolerance²; only the candidates on that
//! zone's boundary are kept (they become invisible strokes), the others are discarded. **Close Gap** joins the stroke
//! end (else the stroke point) nearest each end of a drag. All in a fixed order, so the same input gives the same
//! segments.

use super::flat::{self, segment_distance};
use super::graph::{self, EdgeKind, StrokeIn};
use crate::geom::P;
use crate::path::{Anchor, GeomError, Path};
use crate::work::Work;

/// Gap-closing tolerances in view pixels (03 §8.8): small, medium, large. Agent commands evaluate them at 100 % zoom,
/// where one view pixel is one drawing unit (03 §7.2; SI-063).
pub const TOLERANCE_PX: [f64; 3] = [3.0, 8.0, 16.0];

/// The ID prefix of candidate closing segments while they are tested.
const CANDIDATE: &str = "\u{1}gap";

/// The distance from `p` to a flattened stroke.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn distance_to(path: &Path, p: P, work: &mut Work) -> Result<f64, GeomError> {
    let f = flat::flatten(path, work)?;
    let count = path.segments().len();
    if f.pts.len() == 1 {
        return Ok(f.pts[0].dist(p));
    }
    Ok(f.edges(count).iter().map(|(a, b, _, _)| segment_distance(p, *a, *b)).fold(f64::INFINITY, f64::min))
}

/// Whether `p` lies inside a closed outline (non-zero winding of its flattened polygon).
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn inside(path: &Path, p: P, work: &mut Work) -> Result<bool, GeomError> {
    let f = flat::flatten(path, work)?;
    let n = f.pts.len();
    if n < 3 {
        return Ok(false);
    }
    let mut w = 0i32;
    for i in 0..n {
        let (a, b) = (f.pts[i], f.pts[(i + 1) % n]);
        let o = (b - a).cross(p - a);
        if a.y <= p.y {
            if b.y > p.y && o > 0.0 {
                w += 1;
            }
        } else if b.y <= p.y && o < 0.0 {
            w -= 1;
        }
    }
    Ok(w != 0)
}

/// The ends of the open pencil and invisible strokes of a layer: `(stroke index, point)`.
fn open_ends(strokes: &[StrokeIn]) -> Vec<(usize, P)> {
    let mut out = Vec::new();
    for (k, s) in strokes.iter().enumerate() {
        if s.kind == EdgeKind::Brush || s.path.closed || s.path.anchors.len() < 2 {
            continue;
        }
        out.push((k, s.path.anchors[0].p));
        out.push((k, s.path.anchors[s.path.anchors.len() - 1].p));
    }
    out
}

/// The nearest point of a stroke's flattened path to `p`, with its distance.
fn nearest_on(path: &Path, p: P, work: &mut Work) -> Result<Option<(f64, P)>, GeomError> {
    let f = flat::flatten(path, work)?;
    let count = path.segments().len();
    let mut best: Option<(f64, P)> = None;
    for (a, b, _, _) in f.edges(count) {
        let ab = b - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + ab * t;
        let d = p.dist(q);
        if best.is_none_or(|x| d < x.0) {
            best = Some((d, q));
        }
    }
    Ok(best)
}

fn segment(a: P, b: P) -> Path {
    Path { anchors: vec![Anchor::corner(a), Anchor::corner(b)], closed: false }
}

/// The closing segments that make the region under `p` a zone, if candidates within `tol` can (03 §8.8); `None`
/// when no combination closes it.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn close_around(strokes: &[StrokeIn], p: P, tol: f64, work: &mut Work) -> Result<Option<Vec<(P, P)>>, GeomError> {
    let ends = open_ends(strokes);
    let mut candidates: Vec<(P, P)> = Vec::new();
    for (n, &(k, e)) in ends.iter().enumerate() {
        // The nearest other end.
        let mut best_end: Option<(f64, P)> = None;
        for (m, &(_, f)) in ends.iter().enumerate() {
            if m == n || f == e {
                continue;
            }
            let d = e.dist(f);
            if d <= tol && best_end.is_none_or(|b| d < b.0) {
                best_end = Some((d, f));
            }
        }
        if let Some((_, f)) = best_end {
            candidates.push((e, f));
        }
        // The nearest point of another stroke.
        let mut best_line: Option<(f64, P)> = None;
        for (j, s) in strokes.iter().enumerate() {
            if j == k {
                continue;
            }
            work.spend_n(s.path.anchors.len())?;
            if let Some((d, q)) = nearest_on(&s.path, e, work)?
                && d > 1e-9
                && d <= tol
                && best_line.is_none_or(|b| d < b.0)
            {
                best_line = Some((d, q));
            }
        }
        if let Some((_, q)) = best_line {
            candidates.push((e, q));
        }
    }
    // One candidate per pair of points, in a fixed order.
    let key = |a: P, b: P| {
        let (lo, hi) = if (a.x, a.y) <= (b.x, b.y) { (a, b) } else { (b, a) };
        [lo.x, lo.y, hi.x, hi.y]
    };
    candidates.sort_by(|x, y| {
        let (kx, ky) = (key(x.0, x.1), key(y.0, y.1));
        kx.iter().zip(ky.iter()).map(|(a, b)| a.total_cmp(b)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.dedup_by(|x, y| key(x.0, x.1) == key(y.0, y.1));
    if candidates.is_empty() {
        return Ok(None);
    }
    let mut trial: Vec<StrokeIn> = strokes.to_vec();
    for (i, (a, b)) in candidates.iter().enumerate() {
        trial.push(StrokeIn { id: format!("{CANDIDATE}{i}"), kind: EdgeKind::Invisible, path: segment(*a, *b) });
    }
    let map = graph::build(&trial, work)?;
    let Some(z) = map.zone_at(p) else { return Ok(None) };
    if map.zones[z].area < 4.0 * tol * tol {
        return Ok(None);
    }
    let mut kept: Vec<(P, P)> = Vec::new();
    for piece in &map.zones[z].pieces {
        let id = &map.strokes[piece.stroke].0;
        if let Some(i) = id.strip_prefix(CANDIDATE).and_then(|n| n.parse::<usize>().ok())
            && let Some(c) = candidates.get(i)
            && !kept.contains(c)
        {
            kept.push(*c);
        }
    }
    Ok(Some(kept))
}

/// The Close Gap segment for a drag from `from` to `to` (03 §8.6): each end of the drag goes to the nearest stroke
/// end within `tol`, else to the nearest point of a stroke within `tol`; `None` when either end finds nothing or both
/// find the same point.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn gap_segment(
    strokes: &[StrokeIn],
    from: P,
    to: P,
    tol: f64,
    work: &mut Work,
) -> Result<Option<(P, P)>, GeomError> {
    let ends = open_ends(strokes);
    let mut pick = |q: P| -> Result<Option<P>, GeomError> {
        let mut best: Option<(f64, P)> = None;
        for &(_, e) in &ends {
            let d = e.dist(q);
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, e));
            }
        }
        if best.is_some() {
            return Ok(best.map(|b| b.1));
        }
        for s in strokes {
            work.spend_n(s.path.anchors.len())?;
            if let Some((d, p)) = nearest_on(&s.path, q, work)?
                && d <= tol
                && best.is_none_or(|b| d < b.0)
            {
                best = Some((d, p));
            }
        }
        Ok(best.map(|b| b.1))
    };
    let (Some(a), Some(b)) = (pick(from)?, pick(to)?) else { return Ok(None) };
    if a.dist(b) <= 1e-9 {
        return Ok(None);
    }
    Ok(Some((a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, pts: &[(f64, f64)]) -> StrokeIn {
        StrokeIn {
            id: id.to_owned(),
            kind: EdgeKind::Pencil,
            path: Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: false },
        }
    }

    #[test]
    fn a_small_opening_closes_with_one_kept_segment() {
        // A square missing 2 du of its top edge.
        let s = [
            line("u", &[(0.0, 100.0), (0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (51.0, 100.0)]),
            line("v", &[(49.0, 100.0), (0.0, 100.0)]),
        ];
        let mut w = Work::unlimited("t");
        assert_eq!(close_around(&s, P::new(50.0, 50.0), 1.0, &mut w).ok().flatten(), None);
        let kept = close_around(&s, P::new(50.0, 50.0), 3.0, &mut w).ok().flatten().unwrap_or_default();
        assert_eq!(kept.len(), 1);
        assert!(kept[0].0.dist(kept[0].1) <= 2.0 + 1e-9);
    }

    #[test]
    fn close_gap_prefers_ends() {
        let s = [line("a", &[(0.0, 0.0), (10.0, 0.0)]), line("b", &[(12.0, 0.0), (20.0, 0.0)])];
        let g = gap_segment(&s, P::new(9.0, 1.0), P::new(13.0, 1.0), 16.0, &mut Work::unlimited("t")).ok().flatten();
        assert_eq!(g, Some((P::new(10.0, 0.0), P::new(12.0, 0.0))));
    }
}
