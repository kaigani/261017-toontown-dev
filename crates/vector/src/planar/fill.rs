//! Zone fills (02 §3.5, §3.12 item 1; 10 §4.9; SI-060).
//!
//! Zone fills are persisted on the sides of the strokes bounding the zone. Every pencil and invisible piece bounding a
//! zone carries the zone's fill on that side; when they disagree, the fill held by the longest bounding length wins and
//! the zone is reported (`drawing.fillInconsistent`). A zone bounded only by stretches of brush outlines — a brush
//! stroke never being split, its one outline may border several zones — keeps its fill in the brush stroke's
//! `ext["dev.toontown.zones"].exterior` records, keyed by the side and the stretch of outline (path parameters), rewritten
//! deterministically on every update. Fills are colour IDs plus mapping matrices, never baked colours.
//!
//! Across an edit, a zone with the same ID keeps its fill; any other new zone takes the fill of the old zone it
//! overlaps most (exact polygon intersection on the grid; ties to the lower old zone ID): a split zone gives both
//! parts its fill, merged zones take the larger part's.

use std::collections::BTreeMap;

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::{Overlay, ShapeType};
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;

use super::graph::{EdgeKind, Ip, LayerMap, key_param};
use crate::path::GeomError;
use crate::work::Work;

/// The extension namespace of brush-bounded zone fills (SI-060).
pub const EXT_NAMESPACE: &str = "dev.toontown.zones";

/// A zone fill: a colour ID (`None` = unpainted) and a gradient/texture mapping matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    /// Colour ID.
    pub colour_id: Option<String>,
    /// Mapping matrix `[a, b, c, d, e, f]`.
    pub matrix: Option<[f64; 6]>,
}

impl Fill {
    /// Whether the fill paints (has a colour).
    #[must_use]
    pub fn painted(&self) -> bool {
        self.colour_id.is_some()
    }
}

/// The fills recorded on a stroke's sides.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SideFills {
    /// Left side.
    pub left: Option<Fill>,
    /// Right side.
    pub right: Option<Fill>,
}

/// A brush-outline stretch fill record (SI-060).
#[derive(Debug, Clone, PartialEq)]
pub struct Exterior {
    /// Whether the zone lies on the outline's left.
    pub left: bool,
    /// Path parameter range of the stretch.
    pub s0: f64,
    /// End of the range.
    pub s1: f64,
    /// The fill.
    pub fill: Fill,
}

/// The fills stored on a layer, aligned with the map's strokes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stored {
    /// Side fills of each stroke.
    pub sides: Vec<SideFills>,
    /// Exterior records of each stroke (brush strokes only).
    pub exterior: Vec<Vec<Exterior>>,
}

/// The fills of a layer's zones as read back.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Read {
    /// Fill of each zone (`None` = unpainted).
    pub fills: Vec<Option<Fill>>,
    /// Zones whose bounding strokes disagreed (repaired to the longest bounding length).
    pub inconsistent: Vec<usize>,
}

fn same(a: &Option<Fill>, b: &Option<Fill>) -> bool {
    let norm = |f: &Option<Fill>| f.as_ref().filter(|f| f.painted()).cloned();
    norm(a) == norm(b)
}

/// Reads the zone fills of a layer.
#[must_use]
pub fn read(map: &LayerMap, stored: &Stored) -> Read {
    let mut out = Read { fills: Vec::with_capacity(map.zones.len()), inconsistent: Vec::new() };
    for (z, zone) in map.zones.iter().enumerate() {
        let mut votes: Vec<(Option<Fill>, f64)> = Vec::new();
        let lines: Vec<_> = zone.pieces.iter().filter(|p| map.strokes[p.stroke].1 != EdgeKind::Brush).collect();
        if lines.is_empty() {
            for p in &zone.pieces {
                for r in stored.exterior.get(p.stroke).into_iter().flatten() {
                    let overlap = (r.s1.min(p.s1) - r.s0.max(p.s0)).max(0.0);
                    if r.left == p.left && overlap > 0.0 {
                        let span = (p.s1 - p.s0).max(1e-12);
                        votes.push((Some(r.fill.clone()), p.length * overlap / span));
                    }
                }
            }
        } else {
            for p in lines {
                let side =
                    stored.sides.get(p.stroke).and_then(|s| if p.left { s.left.clone() } else { s.right.clone() });
                votes.push((side.filter(Fill::painted), p.length));
            }
        }
        let mut tally: Vec<(Option<Fill>, f64)> = Vec::new();
        for (f, w) in votes {
            match tally.iter_mut().find(|(t, _)| same(t, &f)) {
                Some(t) => t.1 += w,
                None => tally.push((f, w)),
            }
        }
        if tally.len() > 1 {
            out.inconsistent.push(z);
        }
        let best = tally.into_iter().fold(None, |best: Option<(Option<Fill>, f64)>, (f, w)| match best {
            Some((_, bw)) if bw >= w => best,
            _ => Some((f, w)),
        });
        out.fills.push(best.and_then(|b| b.0));
    }
    out
}

/// The fills to store for given zone fills: side fills of every stroke and exterior records of brush strokes
/// bounding brush-only zones (sorted; same input → same records).
#[must_use]
pub fn write(map: &LayerMap, fills: &[Option<Fill>]) -> Stored {
    let mut stored =
        Stored { sides: vec![SideFills::default(); map.strokes.len()], exterior: vec![Vec::new(); map.strokes.len()] };
    let painted = |z: Option<usize>| z.and_then(|z| fills.get(z).cloned().flatten()).filter(Fill::painted);
    for (k, (_, kind)) in map.strokes.iter().enumerate() {
        if *kind == EdgeKind::Brush {
            continue;
        }
        let (l, r) = map.stroke_sides(k);
        stored.sides[k] = SideFills { left: painted(l), right: painted(r) };
    }
    for (z, zone) in map.zones.iter().enumerate() {
        if zone.pieces.iter().any(|p| map.strokes[p.stroke].1 != EdgeKind::Brush) {
            continue;
        }
        let Some(fill) = painted(Some(z)) else { continue };
        for p in &zone.pieces {
            stored.exterior[p.stroke].push(Exterior { left: p.left, s0: p.s0, s1: p.s1, fill: fill.clone() });
        }
    }
    for list in &mut stored.exterior {
        list.sort_by(|a, b| {
            key_param(a.s0).cmp(&key_param(b.s0)).then(a.left.cmp(&b.left)).then(key_param(a.s1).cmp(&key_param(b.s1)))
        });
    }
    stored
}

fn int_point(q: Ip) -> IntPoint<i32> {
    IntPoint::new(i32::try_from(q[0]).unwrap_or(i32::MAX), i32::try_from(q[1]).unwrap_or(i32::MAX))
}

/// Twice the exact area of the intersection of two zones given as grid rings (outer first, holes reversed).
fn overlap2(a: &[Vec<Ip>], b: &[Vec<Ip>], work: &mut Work) -> Result<i128, GeomError> {
    let count: usize = a.iter().chain(b.iter()).map(Vec::len).sum();
    work.spend_n(count.saturating_mul(32))?;
    let mut overlay: Overlay<i32> = Overlay::new(count);
    for r in a {
        let ring: Vec<IntPoint<i32>> = r.iter().map(|q| int_point(*q)).collect();
        overlay.add_contour(&ring, ShapeType::Subject);
    }
    for r in b {
        let ring: Vec<IntPoint<i32>> = r.iter().map(|q| int_point(*q)).collect();
        overlay.add_contour(&ring, ShapeType::Clip);
    }
    let shapes = overlay.overlay(OverlayRule::Intersect, FillRule::NonZero);
    let mut area2: i128 = 0;
    for shape in shapes {
        for contour in shape {
            let n = contour.len();
            for i in 0..n {
                let (p, q) = (contour[i], contour[(i + 1) % n]);
                area2 += i128::from(p.x) * i128::from(q.y) - i128::from(p.y) * i128::from(q.x);
            }
        }
    }
    Ok(area2.abs())
}

/// The fills of a new map's zones carried over from an old map and its fills.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn carry(
    old: &LayerMap,
    old_fills: &[Option<Fill>],
    new: &LayerMap,
    work: &mut Work,
) -> Result<Vec<Option<Fill>>, GeomError> {
    let by_id: BTreeMap<&str, usize> = old.zones.iter().enumerate().map(|(i, z)| (z.id.as_str(), i)).collect();
    // Old zones whose interior point lies in each new zone.
    let mut inside: Vec<Vec<usize>> = vec![Vec::new(); new.zones.len()];
    for (i, z) in old.zones.iter().enumerate() {
        work.spend_n(new.faces.len())?;
        if let Some(n) = new.zone_at(z.interior) {
            inside[n].push(i);
        }
    }
    let mut out = Vec::with_capacity(new.zones.len());
    for (n, zone) in new.zones.iter().enumerate() {
        if let Some(&i) = by_id.get(zone.id.as_str()) {
            out.push(old_fills.get(i).cloned().flatten());
            continue;
        }
        let mut cands = inside[n].clone();
        work.spend_n(old.faces.len())?;
        if let Some(i) = old.zone_at(zone.interior) {
            cands.push(i);
        }
        cands.sort_unstable();
        cands.dedup();
        let new_rings = new.zone_rings_grid(n);
        let mut best: Option<(i128, usize)> = None;
        for i in cands {
            let area = overlap2(&old.zone_rings_grid(i), &new_rings, work)?;
            let better = match best {
                None => area > 0,
                Some((ba, bi)) => area > ba || (area == ba && old.zones[i].id < old.zones[bi].id),
            };
            if better {
                best = Some((area, i));
            }
        }
        out.push(best.and_then(|(_, i)| old_fills.get(i).cloned().flatten()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::P;
    use crate::path::{Anchor, Path};
    use crate::planar::graph::{StrokeIn, build};

    fn line(id: &str, pts: &[(f64, f64)], closed: bool) -> StrokeIn {
        StrokeIn {
            id: id.to_owned(),
            kind: EdgeKind::Pencil,
            path: Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed },
        }
    }

    fn paint(c: &str) -> Option<Fill> {
        Some(Fill { colour_id: Some(c.to_owned()), matrix: None })
    }

    fn build_map(s: &[StrokeIn]) -> LayerMap {
        build(s, &mut Work::unlimited("t"))
            .unwrap_or_else(|_| build(&[], &mut Work::unlimited("t")).unwrap_or_else(|_| unreachable_map()))
    }

    fn unreachable_map() -> LayerMap {
        LayerMap {
            strokes: vec![],
            verts: vec![],
            edges: vec![],
            out: vec![],
            next: vec![],
            cycle_of: vec![],
            cycles: vec![],
            face_of_cycle: vec![],
            faces: vec![],
            zones: vec![],
            stroke_edges: vec![],
        }
    }

    #[test]
    fn written_fills_read_back() {
        let m = build_map(&[line("s", &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true)]);
        let stored = write(&m, &[paint("red")]);
        assert_eq!(stored.sides[0].left, paint("red"));
        assert_eq!(read(&m, &stored).fills, vec![paint("red")]);
    }

    #[test]
    fn a_split_zone_gives_both_parts_its_fill_and_a_merge_takes_the_larger_part() {
        let sq = line("s", &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true);
        let old = build_map(std::slice::from_ref(&sq));
        let new = build_map(&[sq.clone(), line("cut", &[(30.0, -1.0), (30.0, 101.0)], false)]);
        assert_eq!(new.zones.len(), 2);
        let fills = carry(&old, &[paint("red")], &new, &mut Work::unlimited("t")).unwrap_or_default();
        assert_eq!(fills, vec![paint("red"), paint("red")]);
        // Merge: the larger part (70 %) is blue.
        let left = new.zone_at(P::new(10.0, 50.0)).unwrap_or(0);
        let mut split_fills = vec![None, None];
        split_fills[left] = paint("green");
        split_fills[1 - left] = paint("blue");
        let merged = carry(&new, &split_fills, &old, &mut Work::unlimited("t")).unwrap_or_default();
        assert_eq!(merged, vec![paint("blue")]);
    }

    #[test]
    fn disagreeing_sides_resolve_to_the_longest_bounding_length() {
        let m = build_map(&[
            line("long", &[(0.0, 0.0), (100.0, 0.0), (100.0, 10.0)], false),
            line("short", &[(100.0, 10.0), (0.0, 10.0), (0.0, 0.0)], false),
        ]);
        let mut stored = write(&m, &[paint("red")]);
        stored.sides[1].left = paint("blue");
        let r = read(&m, &stored);
        assert_eq!(r.fills, vec![paint("red")]);
        assert_eq!(r.inconsistent, vec![0]);
    }
}
