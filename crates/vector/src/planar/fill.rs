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

use std::collections::{BTreeMap, BTreeSet};

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
    /// Whether each stroke carries the exterior-record key (then its records, even none, are authoritative; without
    /// it a brush stroke's own side fills are read, as other writers may store them).
    pub managed: Vec<bool>,
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
                if !stored.managed.get(p.stroke).copied().unwrap_or(false) {
                    let side =
                        stored.sides.get(p.stroke).and_then(|s| if p.left { s.left.clone() } else { s.right.clone() });
                    if let Some(f) = side.filter(Fill::painted) {
                        votes.push((Some(f), p.length));
                    }
                    continue;
                }
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

/// The fills to store for given zone fills: side fills of the pencil and invisible strokes bounding zones, and exterior records of
/// brush strokes bounding brush-only zones (sorted; same input → same records). A brush stroke's own sides are kept
/// as `previous` holds them (noding and painting never rewrite a brush stroke's sides); its exterior key is written
/// when it has records, or to override side fills it carries, and is absent otherwise.
#[must_use]
pub fn write(map: &LayerMap, fills: &[Option<Fill>], previous: &Stored) -> Stored {
    let n = map.strokes.len();
    let mut stored =
        Stored { sides: vec![SideFills::default(); n], exterior: vec![Vec::new(); n], managed: vec![false; n] };
    let painted = |z: Option<usize>| z.and_then(|z| fills.get(z).cloned().flatten()).filter(Fill::painted);
    for (k, (_, kind)) in map.strokes.iter().enumerate() {
        if *kind == EdgeKind::Brush {
            stored.sides[k] = previous.sides.get(k).cloned().unwrap_or_default();
            continue;
        }
        // A side bounding a zone carries that zone's fill; a side bounding none keeps what it holds.
        let (l, r) = map.stroke_sides(k);
        let prev = previous.sides.get(k).cloned().unwrap_or_default();
        // An unchanged meaning keeps its stored form (`null` and an unpainted fill object both mean unpainted).
        let keep = |zone: Option<usize>, prev: Option<Fill>| -> Option<Fill> {
            if zone.is_none() {
                return prev;
            }
            let want = painted(zone);
            if same(&want, &prev) { prev } else { want }
        };
        stored.sides[k] = SideFills { left: keep(l, prev.left), right: keep(r, prev.right) };
    }
    for (z, zone) in map.zones.iter().enumerate() {
        if zone.pieces.iter().any(|p| map.strokes[p.stroke].1 != EdgeKind::Brush) {
            continue;
        }
        for p in &zone.pieces {
            let legacy = previous.sides.get(p.stroke).is_some_and(|s| {
                s.left.as_ref().is_some_and(Fill::painted) || s.right.as_ref().is_some_and(Fill::painted)
            });
            if legacy || previous.managed.get(p.stroke).copied().unwrap_or(false) {
                stored.managed[p.stroke] = true;
            }
        }
        let Some(fill) = painted(Some(z)) else { continue };
        for p in &zone.pieces {
            stored.managed[p.stroke] = true;
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

/// A grid index of boxes (grid units) for positive-area overlap queries; boxes spanning too many cells, and queries
/// spanning too many cells, fall back to a list scanned in full.
struct Boxes {
    size: i64,
    boxes: Vec<(usize, [i64; 4])>,
    cells: BTreeMap<(i64, i64), Vec<usize>>,
    large: Vec<usize>,
}

impl Boxes {
    const MAX_CELLS: i64 = 256;

    fn new(boxes: &[(usize, [i64; 4])], work: &mut Work) -> Result<Self, GeomError> {
        let extent: i128 = if boxes.is_empty() {
            64
        } else {
            boxes.iter().map(|(_, b)| i128::from((b[2] - b[0]).max(b[3] - b[1]))).sum::<i128>() / boxes.len() as i128
        };
        let size = i64::try_from(extent.clamp(64, 1 << 26)).unwrap_or(64);
        let mut idx = Boxes { size, boxes: boxes.to_vec(), cells: BTreeMap::new(), large: Vec::new() };
        for (k, &(_, b)) in boxes.iter().enumerate() {
            let (x0, y0, x1, y1) = idx.range(b);
            let n = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
            if n > Self::MAX_CELLS {
                idx.large.push(k);
                continue;
            }
            work.spend(u64::try_from(n).unwrap_or(u64::MAX))?;
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    idx.cells.entry((cx, cy)).or_default().push(k);
                }
            }
        }
        Ok(idx)
    }

    fn range(&self, b: [i64; 4]) -> (i64, i64, i64, i64) {
        let s = self.size.max(1);
        (b[0].div_euclid(s), b[1].div_euclid(s), b[2].div_euclid(s), b[3].div_euclid(s))
    }

    /// The IDs of the boxes overlapping `q` with positive area, ascending.
    fn overlapping(&self, q: [i64; 4], work: &mut Work) -> Result<Vec<usize>, GeomError> {
        let (x0, y0, x1, y1) = self.range(q);
        let mut ks: Vec<usize> = self.large.clone();
        if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > Self::MAX_CELLS {
            ks = (0..self.boxes.len()).collect();
        } else {
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    if let Some(list) = self.cells.get(&(cx, cy)) {
                        ks.extend_from_slice(list);
                    }
                }
            }
        }
        work.spend_n(ks.len())?;
        let mut out: Vec<usize> = ks
            .into_iter()
            .filter_map(|k| {
                let (id, b) = self.boxes[k];
                (b[0] < q[2] && q[0] < b[2] && b[1] < q[3] && q[1] < b[3]).then_some(id)
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }
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
    // A zone ID is a hash of the zone's whole boundary (geometry, strokes, sides), so an old zone with the ID of a new
    // one has the same polygon: its overlap with that zone is its whole area and with every other new zone nothing.
    // Those pairs are settled without overlays; every other old zone is a candidate for each new zone whose bounding
    // box it overlaps with positive area (C22-R1-02), scored by exact polygon intersection.
    let matched: BTreeSet<usize> = new.zones.iter().filter_map(|z| by_id.get(z.id.as_str()).copied()).collect();
    let rest: Vec<(usize, [i64; 4])> =
        (0..old.zones.len()).filter(|i| !matched.contains(i)).map(|i| (i, old.zone_box(i))).collect();
    let index = Boxes::new(&rest, work)?;
    let mut out = Vec::with_capacity(new.zones.len());
    for (n, zone) in new.zones.iter().enumerate() {
        if let Some(&i) = by_id.get(zone.id.as_str()) {
            out.push(old_fills.get(i).cloned().flatten());
            continue;
        }
        let cands = index.overlapping(new.zone_box(n), work)?;
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
            face_index: crate::planar::graph::BoxIndex::default(),
        }
    }

    #[test]
    fn written_fills_read_back() {
        let m = build_map(&[line("s", &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true)]);
        let stored = write(&m, &[paint("red")], &Stored::default());
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
    fn the_largest_overlap_wins_even_when_no_interior_point_lies_in_the_other_zone() {
        // Two old bars crossing the new bar where neither old interior point lies in it nor its interior in them:
        // red overlaps 6 du², blue 4 du²; with equal overlaps the lower old zone ID wins.
        let old = build_map(&[
            line("r", &[(0.0, 4.0), (10.0, 4.0), (10.0, 6.0), (0.0, 6.0)], true),
            line("b", &[(20.0, 4.0), (30.0, 4.0), (30.0, 6.0), (20.0, 6.0)], true),
        ]);
        let red = old.zone_at(P::new(5.0, 5.0)).unwrap_or(0);
        let mut fills = vec![None, None];
        fills[red] = paint("red");
        fills[1 - red] = paint("blue");
        let wide = build_map(&[line("n", &[(7.0, 0.0), (22.0, 0.0), (22.0, 100.0), (7.0, 100.0)], true)]);
        assert_eq!(carry(&old, &fills, &wide, &mut Work::unlimited("t")).unwrap_or_default(), vec![paint("red")]);
        let even = build_map(&[line("n", &[(8.0, 0.0), (22.0, 0.0), (22.0, 100.0), (8.0, 100.0)], true)]);
        let lower = if old.zones[red].id < old.zones[1 - red].id { "red" } else { "blue" };
        assert_eq!(carry(&old, &fills, &even, &mut Work::unlimited("t")).unwrap_or_default(), vec![paint(lower)]);
    }

    #[test]
    fn disagreeing_sides_resolve_to_the_longest_bounding_length() {
        let m = build_map(&[
            line("long", &[(0.0, 0.0), (100.0, 0.0), (100.0, 10.0)], false),
            line("short", &[(100.0, 10.0), (0.0, 10.0), (0.0, 0.0)], false),
        ]);
        let mut stored = write(&m, &[paint("red")], &Stored::default());
        stored.sides[1].left = paint("blue");
        let r = read(&m, &stored);
        assert_eq!(r.fills, vec![paint("red")]);
        assert_eq!(r.inconsistent, vec![0]);
    }
}
