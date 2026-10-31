//! The derived planar graph of one layer (02 §3.5, §3.12; 10 §4.8 derived zones).
//!
//! Every stroke of the layer — pencil and invisible centre paths, brush outlines — is flattened (see [`super::flat`])
//! and its edges are put on an integer grid of 1/[`GRID`] du. The arrangement is **snap rounded**: every edge end and
//! every proper crossing is a *hot pixel*, and every edge passing through a hot pixel (the closed unit square around
//! it) is broken there. Snap-rounded edges meet only at hot pixels, so the half-edge structure built from them is
//! consistent, and all predicates are exact integer arithmetic (i128): the graph is identical on every platform.
//!
//! Faces are traced with the face on the left of each half-edge. A counter-clockwise cycle bounds a face; the other
//! cycle of each connected component is that component's outside, assigned as a hole to the smallest face of another
//! component that contains it (exact point in polygon). A face inside a brush outline (non-zero winding of that
//! outline's edges) is the brush stroke's own fill (02 §13.4) and not a zone; every other bounded face is a **zone**
//! with its holes, parent zone, area, bounds and an interior point. A zone's ID is a SHA-256 of its canonical boundary
//! (the bounding strokes and sides, and the bounding stretches of brush outlines), so it changes whenever the
//! boundary changes and is the same on every OS (02 §3.12 item 4).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sha2::{Digest, Sha256};

use super::flat;
use crate::geom::{P, Rect};
use crate::path::{GeomError, Path};
use crate::work::Work;

/// Grid of the planar map: 1/64 du, the brush union's grid (coordinates up to ±10^7 du stay well inside i64).
pub const GRID: f64 = 64.0;

/// A grid point.
pub type Ip = [i64; 2];

/// Units charged per snapped edge piece, merged edge, half-edge and zone (the work of building, sorting, tracing and
/// describing them; calibrated so a unit is about one to three nanoseconds of release time, as for brush outlines).
const UNITS_PER_PIECE: usize = 256;
const UNITS_PER_EDGE: usize = 256;
const UNITS_PER_HALF_EDGE: usize = 32;
const UNITS_PER_ZONE: usize = 2048;
const UNITS_PER_HOLE_TEST: usize = 64;

/// The kind of stroke an edge comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeKind {
    /// A brush outline (closed; its inside is the stroke's fill).
    Brush,
    /// A pencil centre line.
    Pencil,
    /// An invisible stroke.
    Invisible,
}

impl EdgeKind {
    /// Whether the stroke is rendered (pencil lines and brush strokes).
    #[must_use]
    pub fn visible(self) -> bool {
        self != EdgeKind::Invisible
    }
}

/// One stroke of the layer: its ID, kind and path (the centre path, or a brush stroke's outline).
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeIn {
    /// Stroke ID.
    pub id: String,
    /// Kind.
    pub kind: EdgeKind,
    /// Path.
    pub path: Path,
}

/// The part of a stroke an edge lies on.
#[derive(Debug, Clone, PartialEq)]
pub struct Src {
    /// Index of the stroke.
    pub stroke: usize,
    /// Whether the stroke runs from the edge's `u` to its `v`.
    pub forward: bool,
    /// Path parameter at `u`.
    pub s_u: f64,
    /// Path parameter at `v`.
    pub s_v: f64,
}

/// An undirected edge of the snap-rounded arrangement (`u < v`); coincident pieces of several strokes share one edge.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Lower vertex index.
    pub u: usize,
    /// Higher vertex index.
    pub v: usize,
    /// The stroke pieces lying on the edge.
    pub srcs: Vec<Src>,
}

/// A boundary cycle (face on the left of each half-edge).
#[derive(Debug, Clone, PartialEq)]
pub struct Cycle {
    /// Half-edges in order.
    pub half: Vec<usize>,
    /// Twice the signed area (grid units²; positive = counter-clockwise).
    pub area2: i128,
    /// Connected component.
    pub comp: usize,
    /// Bounding box (grid units).
    pub bbox: [i64; 4],
}

/// A face: face 0 is the unbounded face.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// The counter-clockwise outer cycle (none for the unbounded face).
    pub outer: Option<usize>,
    /// Hole cycles (the outsides of the components inside the face).
    pub holes: Vec<usize>,
    /// A point strictly inside the face (du); `None` for the unbounded face and for slivers below the grid.
    pub interior: Option<P>,
    /// The brush strokes whose fill covers the face.
    pub brushes: Vec<usize>,
    /// The zone of the face.
    pub zone: Option<usize>,
}

/// A stretch of a stroke on a zone's boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// Index of the stroke.
    pub stroke: usize,
    /// Whether the zone lies on the stroke's left (relative to point order).
    pub left: bool,
    /// Path parameter range (ascending).
    pub s0: f64,
    /// End of the range.
    pub s1: f64,
    /// Length (du).
    pub length: f64,
}

/// A hole of a zone.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneHole {
    /// IDs of the strokes bounding the hole.
    pub boundary: Vec<String>,
    /// The zone inside the hole along the hole's longest boundary stretch, if any.
    pub zone: Option<usize>,
}

/// A zone (02 §3.5, §13.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Derived ID.
    pub id: String,
    /// Its face.
    pub face: usize,
    /// Area (du²), holes excluded.
    pub area: f64,
    /// Twice the area in grid units² (exact).
    pub area2: i128,
    /// Bounds (du).
    pub bounds: Rect,
    /// A point strictly inside.
    pub interior: P,
    /// IDs of the bounding strokes (outer and hole boundaries), sorted.
    pub boundary: Vec<String>,
    /// Holes.
    pub holes: Vec<ZoneHole>,
    /// The zone whose hole this zone lies in.
    pub parent: Option<usize>,
    /// Boundary stretches (merged per stroke and side).
    pub pieces: Vec<Piece>,
    /// Whether any bounding stroke is visible (pencil or brush).
    pub visible_boundary: bool,
}

/// The planar map of one layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerMap {
    /// Stroke IDs and kinds, in input order.
    pub strokes: Vec<(String, EdgeKind)>,
    /// Grid vertices (sorted).
    pub verts: Vec<Ip>,
    /// Edges.
    pub edges: Vec<Edge>,
    /// Outgoing half-edges of each vertex, counter-clockwise from +x.
    pub out: Vec<Vec<usize>>,
    /// Next half-edge around the face on the left.
    pub next: Vec<usize>,
    /// Cycle of each half-edge.
    pub cycle_of: Vec<usize>,
    /// Cycles.
    pub cycles: Vec<Cycle>,
    /// Face of each cycle.
    pub face_of_cycle: Vec<usize>,
    /// Faces (0 = unbounded).
    pub faces: Vec<Face>,
    /// Zones.
    pub zones: Vec<Zone>,
    /// The edges of each stroke as `(edge, forward)`, in stroke order.
    pub stroke_edges: Vec<Vec<(usize, bool)>>,
    /// Bounded faces by the box of their outer cycle (point location).
    pub face_index: BoxIndex,
}

/// A grid index of boxes (grid units): each box is listed in the cells it covers; boxes covering more than
/// [`BoxIndex::MAX_CELLS`] cells are kept in a list every query checks. Point location and hole assignment look only
/// at the boxes of the query's cell.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoxIndex {
    size: i64,
    cells: BTreeMap<(i64, i64), Vec<usize>>,
    large: Vec<usize>,
}

impl BoxIndex {
    /// Most cells one box is listed in.
    pub const MAX_CELLS: i64 = 256;

    fn new(boxes: &[(usize, [i64; 4])], work: &mut Work) -> Result<Self, GeomError> {
        let extent: i128 = if boxes.is_empty() {
            64
        } else {
            boxes.iter().map(|(_, b)| i128::from((b[2] - b[0]).max(b[3] - b[1]))).sum::<i128>() / boxes.len() as i128
        };
        let size = i64::try_from(
            u64::try_from(i64::try_from(extent).unwrap_or(64).clamp(64, 1 << 26)).unwrap_or(64).next_power_of_two(),
        )
        .unwrap_or(64);
        let mut idx = BoxIndex { size, cells: BTreeMap::new(), large: Vec::new() };
        for &(id, b) in boxes {
            let (x0, y0, x1, y1) =
                (b[0].div_euclid(size), b[1].div_euclid(size), b[2].div_euclid(size), b[3].div_euclid(size));
            let n = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
            if n > Self::MAX_CELLS {
                idx.large.push(id);
                continue;
            }
            work.spend(u64::try_from(n).unwrap_or(u64::MAX))?;
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    idx.cells.entry((cx, cy)).or_default().push(id);
                }
            }
        }
        Ok(idx)
    }

    /// The boxes that may contain a point (grid units).
    fn query(&self, x: i64, y: i64) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .cells
            .get(&(x.div_euclid(self.size.max(1)), y.div_euclid(self.size.max(1))))
            .cloned()
            .unwrap_or_default();
        out.extend(self.large.iter().copied());
        out
    }
}

/// The grid point of `p`.
#[must_use]
pub fn snap(p: P) -> Ip {
    [(p.x * GRID).round() as i64, (p.y * GRID).round() as i64]
}

/// The point of a grid point (du).
#[must_use]
pub fn unsnap(q: Ip) -> P {
    P::new(q[0] as f64 / GRID, q[1] as f64 / GRID)
}

fn orient(a: Ip, b: Ip, c: Ip) -> i128 {
    let (ax, ay) = (i128::from(a[0]), i128::from(a[1]));
    (i128::from(b[0]) - ax) * (i128::from(c[1]) - ay) - (i128::from(b[1]) - ay) * (i128::from(c[0]) - ax)
}

fn sign(v: i128) -> i8 {
    match v.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// `n / d` rounded to the nearest integer, halves away from zero (`d > 0`).
fn round_div(n: i128, d: i128) -> i128 {
    if n >= 0 { (2 * n + d) / (2 * d) } else { -((-2 * n + d) / (2 * d)) }
}

/// The rounded proper crossing of two segments, if they cross at a point interior to both.
fn crossing(a: Ip, b: Ip, c: Ip, d: Ip) -> Option<Ip> {
    let (o1, o2) = (sign(orient(a, b, c)), sign(orient(a, b, d)));
    let (o3, o4) = (sign(orient(c, d, a)), sign(orient(c, d, b)));
    if o1 * o2 >= 0 || o3 * o4 >= 0 {
        return None;
    }
    let r = [i128::from(b[0] - a[0]), i128::from(b[1] - a[1])];
    let s = [i128::from(d[0] - c[0]), i128::from(d[1] - c[1])];
    let den = r[0] * s[1] - r[1] * s[0];
    let ca = [i128::from(c[0] - a[0]), i128::from(c[1] - a[1])];
    let num = ca[0] * s[1] - ca[1] * s[0];
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    let x = i128::from(a[0]) + round_div(r[0] * num, den);
    let y = i128::from(a[1]) + round_div(r[1] * num, den);
    Some([i64::try_from(x).ok()?, i64::try_from(y).ok()?])
}

/// Whether the segment `a`–`b` meets the closed unit square around `h`.
fn through_pixel(a: Ip, b: Ip, h: Ip) -> bool {
    // Doubled coordinates: the square is [2h − 1, 2h + 1].
    let (a2, b2) = ([a[0] * 2, a[1] * 2], [b[0] * 2, b[1] * 2]);
    let (lo, hi) = ([h[0] * 2 - 1, h[1] * 2 - 1], [h[0] * 2 + 1, h[1] * 2 + 1]);
    if a2[0].max(b2[0]) < lo[0] || a2[0].min(b2[0]) > hi[0] || a2[1].max(b2[1]) < lo[1] || a2[1].min(b2[1]) > hi[1] {
        return false;
    }
    let corners = [[lo[0], lo[1]], [hi[0], lo[1]], [hi[0], hi[1]], [lo[0], hi[1]]];
    let signs: Vec<i8> = corners.iter().map(|c| sign(orient(a2, b2, *c))).collect();
    !(signs.iter().all(|s| *s > 0) || signs.iter().all(|s| *s < 0))
}

/// Angle class of a direction for counter-clockwise sorting from +x.
fn half_plane(d: Ip) -> u8 {
    u8::from(!(d[1] > 0 || (d[1] == 0 && d[0] > 0)))
}

fn ccw_cmp(a: Ip, b: Ip) -> std::cmp::Ordering {
    half_plane(a).cmp(&half_plane(b)).then_with(|| {
        let c = i128::from(a[0]) * i128::from(b[1]) - i128::from(a[1]) * i128::from(b[0]);
        0.cmp(&c)
    })
}

/// A stretch of a stroke: parameter range and length.
type Stretch = (f64, f64, f64);

/// A segment of the input on the grid.
struct Seg {
    a: Ip,
    b: Ip,
    stroke: usize,
    sa: f64,
    sb: f64,
}

fn bbox_of(a: Ip, b: Ip) -> [i64; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

/// Spatial hash of the arrangement.
struct Cells {
    size: i64,
}

impl Cells {
    fn cell(&self, x: i64) -> i64 {
        x.div_euclid(self.size)
    }

    fn range(&self, b: [i64; 4]) -> (i64, i64, i64, i64) {
        (self.cell(b[0]), self.cell(b[1]), self.cell(b[2]), self.cell(b[3]))
    }
}

/// Builds the planar map of a layer.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn build(strokes: &[StrokeIn], work: &mut Work) -> Result<LayerMap, GeomError> {
    // 1. Input segments on the grid.
    let mut segs: Vec<Seg> = Vec::new();
    for (k, s) in strokes.iter().enumerate() {
        let f = flat::flatten(&s.path, work)?;
        let count = s.path.segments().len();
        for (a, b, sa, sb) in f.edges(count) {
            let (ia, ib) = (snap(a), snap(b));
            if ia != ib {
                segs.push(Seg { a: ia, b: ib, stroke: k, sa, sb });
            }
        }
    }
    work.spend_n(segs.len())?;
    // 2. Hot pixels: ends and rounded proper crossings.
    let mut hot: BTreeSet<Ip> = BTreeSet::new();
    for s in &segs {
        hot.insert(s.a);
        hot.insert(s.b);
    }
    let extent: i64 = if segs.is_empty() {
        64
    } else {
        let total: i128 = segs.iter().map(|s| i128::from((s.b[0] - s.a[0]).abs().max((s.b[1] - s.a[1]).abs()))).sum();
        i64::try_from(total / segs.len() as i128).unwrap_or(64)
    };
    let cells = Cells {
        size: i64::try_from(u64::try_from(extent.clamp(64, 1 << 24)).unwrap_or(64).next_power_of_two()).unwrap_or(64),
    };
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        let (x0, y0, x1, y1) = cells.range(bbox_of(s.a, s.b));
        let n = u64::try_from((x1 - x0 + 1).saturating_mul(y1 - y0 + 1)).unwrap_or(u64::MAX);
        work.spend(n)?;
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                grid.entry((cx, cy)).or_default().push(i);
            }
        }
    }
    for (&(cx, cy), list) in &grid {
        // Every pair of the cell is tested (about one bounding-box comparison each, a few nanoseconds).
        work.spend_n(list.len().saturating_mul(list.len()).saturating_mul(3) / 2)?;
        for (n, &i) in list.iter().enumerate() {
            let bi = bbox_of(segs[i].a, segs[i].b);
            for &j in &list[n + 1..] {
                let bj = bbox_of(segs[j].a, segs[j].b);
                if bi[2] < bj[0] || bj[2] < bi[0] || bi[3] < bj[1] || bj[3] < bi[1] {
                    continue;
                }
                // Test each pair once: in the cell holding the low corner of the boxes' overlap.
                if (cells.cell(bi[0].max(bj[0])), cells.cell(bi[1].max(bj[1]))) != (cx, cy) {
                    continue;
                }
                if let Some(x) = crossing(segs[i].a, segs[i].b, segs[j].a, segs[j].b) {
                    hot.insert(x);
                }
            }
        }
    }
    drop(grid);
    // 3. Break every segment at the hot pixels it passes through.
    let mut hot_cells: HashMap<(i64, i64), Vec<Ip>> = HashMap::new();
    for h in &hot {
        hot_cells.entry((cells.cell(h[0]), cells.cell(h[1]))).or_default().push(*h);
    }
    let mut vertex_set: BTreeSet<Ip> = BTreeSet::new();
    // (from, to, stroke, s_from, s_to)
    let mut pieces: Vec<(Ip, Ip, usize, f64, f64)> = Vec::new();
    for s in &segs {
        let b = bbox_of(s.a, s.b);
        let (x0, y0, x1, y1) = cells.range([b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1]);
        let mut on: Vec<(i128, Ip)> = Vec::new();
        let d = [i128::from(s.b[0] - s.a[0]), i128::from(s.b[1] - s.a[1])];
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                let Some(list) = hot_cells.get(&(cx, cy)) else { continue };
                work.spend_n(list.len())?;
                for h in list {
                    if through_pixel(s.a, s.b, *h) {
                        let t = i128::from(h[0] - s.a[0]) * d[0] + i128::from(h[1] - s.a[1]) * d[1];
                        on.push((t, *h));
                    }
                }
            }
        }
        on.sort_unstable();
        on.dedup_by(|x, y| x.1 == y.1);
        let len2 = d[0] * d[0] + d[1] * d[1];
        let at = |t: i128| -> f64 {
            let f = if len2 > 0 { (t as f64 / len2 as f64).clamp(0.0, 1.0) } else { 0.0 };
            flat::param_between(s.sa, s.sb, f)
        };
        for w in on.windows(2) {
            if w[0].1 != w[1].1 {
                vertex_set.insert(w[0].1);
                vertex_set.insert(w[1].1);
                pieces.push((w[0].1, w[1].1, s.stroke, at(w[0].0), at(w[1].0)));
            }
        }
    }
    drop(hot_cells);
    work.spend_n(pieces.len().saturating_mul(UNITS_PER_PIECE))?;
    // 4. Vertices and merged edges.
    let verts: Vec<Ip> = vertex_set.into_iter().collect();
    let index: BTreeMap<Ip, usize> = verts.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let mut edge_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut edges: Vec<Edge> = Vec::new();
    for (p, q, stroke, sp, sq) in pieces {
        let (Some(&ip), Some(&iq)) = (index.get(&p), index.get(&q)) else { continue };
        let (u, v, src) = if ip < iq {
            (ip, iq, Src { stroke, forward: true, s_u: sp, s_v: sq })
        } else {
            (iq, ip, Src { stroke, forward: false, s_u: sq, s_v: sp })
        };
        let e = *edge_of.entry((u, v)).or_insert_with(|| {
            edges.push(Edge { u, v, srcs: Vec::new() });
            edges.len() - 1
        });
        edges[e].srcs.push(src);
    }
    work.spend_n(edges.len().saturating_mul(UNITS_PER_EDGE))?;
    let origin = |h: usize| if h.is_multiple_of(2) { edges[h / 2].u } else { edges[h / 2].v };
    let dest = |h: usize| if h.is_multiple_of(2) { edges[h / 2].v } else { edges[h / 2].u };
    // 5. Half-edges around each vertex, counter-clockwise.
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); verts.len()];
    for h in 0..edges.len() * 2 {
        out[origin(h)].push(h);
    }
    let mut pos = vec![0usize; edges.len() * 2];
    for (v, list) in out.iter_mut().enumerate() {
        let o = verts[v];
        list.sort_by(|&x, &y| {
            let (dx, dy) = (verts[dest(x)], verts[dest(y)]);
            ccw_cmp([dx[0] - o[0], dx[1] - o[1]], [dy[0] - o[0], dy[1] - o[1]]).then(x.cmp(&y))
        });
        for (i, h) in list.iter().enumerate() {
            pos[*h] = i;
        }
    }
    let next: Vec<usize> = (0..edges.len() * 2)
        .map(|h| {
            let v = dest(h);
            let deg = out[v].len();
            out[v][(pos[h ^ 1] + deg - 1) % deg]
        })
        .collect();
    // 6. Cycles.
    let mut cycle_of = vec![usize::MAX; edges.len() * 2];
    let mut cycles: Vec<Cycle> = Vec::new();
    for start in 0..edges.len() * 2 {
        if cycle_of[start] != usize::MAX {
            continue;
        }
        let id = cycles.len();
        let mut half = Vec::new();
        let mut h = start;
        let mut area2: i128 = 0;
        let mut bbox = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
        loop {
            cycle_of[h] = id;
            half.push(h);
            let (a, b) = (verts[origin(h)], verts[dest(h)]);
            area2 += i128::from(a[0]) * i128::from(b[1]) - i128::from(a[1]) * i128::from(b[0]);
            bbox = [bbox[0].min(a[0]), bbox[1].min(a[1]), bbox[2].max(a[0]), bbox[3].max(a[1])];
            h = next[h];
            if h == start || cycle_of[h] != usize::MAX {
                break;
            }
        }
        work.spend_n(half.len().saturating_mul(UNITS_PER_HALF_EDGE))?;
        cycles.push(Cycle { half, area2, comp: 0, bbox });
    }
    // 7. Connected components.
    let mut parent: Vec<usize> = (0..verts.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for e in &edges {
        let (a, b) = (find(&mut parent, e.u), find(&mut parent, e.v));
        if a != b {
            parent[a.max(b)] = a.min(b);
        }
    }
    for c in &mut cycles {
        if let Some(&h) = c.half.first() {
            c.comp = find(&mut parent, origin(h));
        }
    }
    // 8. Faces: one per counter-clockwise cycle; the other cycles are holes of the smallest containing face.
    let mut faces: Vec<Face> =
        vec![Face { outer: None, holes: Vec::new(), interior: None, brushes: Vec::new(), zone: None }];
    let mut face_of_cycle = vec![0usize; cycles.len()];
    let mut ccw: Vec<usize> = Vec::new();
    for (i, c) in cycles.iter().enumerate() {
        if c.area2 > 0 {
            face_of_cycle[i] = faces.len();
            faces.push(Face { outer: Some(i), holes: Vec::new(), interior: None, brushes: Vec::new(), zone: None });
            ccw.push(i);
        }
    }
    let face_index = BoxIndex::new(&ccw.iter().map(|&c| (face_of_cycle[c], cycles[c].bbox)).collect::<Vec<_>>(), work)?;
    let poly_of = |c: &Cycle| -> Vec<Ip> { c.half.iter().map(|&h| verts[origin(h)]).collect() };
    for i in 0..cycles.len() {
        if cycles[i].area2 > 0 {
            continue;
        }
        let Some(&h0) = cycles[i].half.first() else { continue };
        let probe = verts[origin(h0)];
        let mut host = 0;
        // The smallest containing face of another component: candidates from the index, smallest first.
        let mut cands: Vec<usize> =
            face_index.query(probe[0], probe[1]).into_iter().filter_map(|f| faces[f].outer).collect();
        work.spend_n(cands.len().saturating_mul(UNITS_PER_HOLE_TEST))?;
        cands.sort_by(|&a, &b| cycles[a].area2.cmp(&cycles[b].area2).then(a.cmp(&b)));
        for &c in &cands {
            let cy = &cycles[c];
            if cy.comp == cycles[i].comp
                || probe[0] < cy.bbox[0]
                || probe[0] > cy.bbox[2]
                || probe[1] < cy.bbox[1]
                || probe[1] > cy.bbox[3]
            {
                continue;
            }
            work.spend_n(cy.half.len())?;
            if winding(&poly_of(cy), probe, 1).0 != 0 {
                host = face_of_cycle[c];
                break;
            }
        }
        face_of_cycle[i] = host;
        faces[host].holes.push(i);
    }
    // 9. Interior points.
    for face in faces.iter_mut().skip(1) {
        let rings: Vec<Vec<Ip>> = face.outer.iter().chain(face.holes.iter()).map(|&c| poly_of(&cycles[c])).collect();
        work.spend_n(rings.iter().map(Vec::len).sum::<usize>().saturating_mul(16))?;
        face.interior = interior_point(&rings);
    }
    // 10. Brush fills covering faces.
    let mut stroke_edges: Vec<Vec<(usize, bool)>> = vec![Vec::new(); strokes.len()];
    for (e, edge) in edges.iter().enumerate() {
        for s in &edge.srcs {
            stroke_edges[s.stroke].push((e, s.forward));
        }
    }
    for list in &mut stroke_edges {
        list.sort_unstable();
    }
    let brush_ids: Vec<usize> = (0..strokes.len()).filter(|&k| strokes[k].kind == EdgeKind::Brush).collect();
    let brush_boxes: Vec<[i64; 4]> = brush_ids
        .iter()
        .map(|&k| {
            let mut b = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
            for &(e, _) in &stroke_edges[k] {
                for v in [edges[e].u, edges[e].v] {
                    let q = verts[v];
                    b = [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])];
                }
            }
            b
        })
        .collect();
    for face in faces.iter_mut().skip(1) {
        let Some(p) = face.interior else { continue };
        let q = [p.x * GRID, p.y * GRID];
        for (n, &k) in brush_ids.iter().enumerate() {
            let b = brush_boxes[n];
            if q[0] < b[0] as f64 || q[0] > b[2] as f64 || q[1] < b[1] as f64 || q[1] > b[3] as f64 {
                continue;
            }
            work.spend_n(stroke_edges[k].len())?;
            let directed: Vec<(Ip, Ip)> = stroke_edges[k]
                .iter()
                .flat_map(|&(e, fwd)| {
                    let (a, b) = (verts[edges[e].u], verts[edges[e].v]);
                    // Every stroke piece on the edge counts (a bridge's two opposite pieces cancel).
                    edges[e]
                        .srcs
                        .iter()
                        .filter(move |s| s.stroke == k && s.forward == fwd)
                        .map(move |_| if fwd { (a, b) } else { (b, a) })
                })
                .collect();
            if directed_winding(&directed, q) != 0 {
                face.brushes.push(k);
            }
        }
    }
    let mut map = LayerMap {
        strokes: strokes.iter().map(|s| (s.id.clone(), s.kind)).collect(),
        verts,
        edges,
        out,
        next,
        cycle_of,
        cycles,
        face_of_cycle,
        faces,
        zones: Vec::new(),
        stroke_edges,
        face_index,
    };
    map.make_zones(work)?;
    Ok(map)
}

/// Winding number of a closed polygon (grid units) around `q·1/scale`, and whether `q` lies on its boundary; `q` is in
/// grid units multiplied by `scale`.
#[must_use]
pub fn winding(poly: &[Ip], q: Ip, scale: i64) -> (i32, bool) {
    let n = poly.len();
    let mut w = 0;
    let s = i128::from(scale);
    let (qx, qy) = (i128::from(q[0]), i128::from(q[1]));
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ax, ay, bx, by) = (i128::from(a[0]) * s, i128::from(a[1]) * s, i128::from(b[0]) * s, i128::from(b[1]) * s);
        let o = (bx - ax) * (qy - ay) - (by - ay) * (qx - ax);
        if o == 0 && qx >= ax.min(bx) && qx <= ax.max(bx) && qy >= ay.min(by) && qy <= ay.max(by) {
            return (0, true);
        }
        if ay <= qy {
            if by > qy && o > 0 {
                w += 1;
            }
        } else if by <= qy && o < 0 {
            w -= 1;
        }
    }
    (w, false)
}

/// Winding number of directed edges (grid units) around a point `q` (grid units, fractional).
fn directed_winding(edges: &[(Ip, Ip)], q: [f64; 2]) -> i32 {
    let mut w = 0;
    for (a, b) in edges {
        let (ax, ay, bx, by) = (a[0] as f64, a[1] as f64, b[0] as f64, b[1] as f64);
        let o = (bx - ax) * (q[1] - ay) - (by - ay) * (q[0] - ax);
        if ay <= q[1] {
            if by > q[1] && o > 0.0 {
                w += 1;
            }
        } else if by <= q[1] && o < 0.0 {
            w -= 1;
        }
    }
    w
}

/// Whether `q` (grid units × `scale`) is strictly inside the face given by its rings (outer first).
fn strictly_inside(rings: &[Vec<Ip>], q: Ip, scale: i64) -> bool {
    let Some((outer, holes)) = rings.split_first() else { return false };
    let (w, on) = winding(outer, q, scale);
    if on || w == 0 {
        return false;
    }
    holes.iter().all(|h| {
        let (w, on) = winding(h, q, scale);
        !on && w == 0
    })
}

/// A point well inside a face: the middle of the widest horizontal chord on one of seven scanlines (each between grid
/// rows, so it never meets a vertex), verified exactly; otherwise a point just left of a boundary edge's middle.
fn interior_point(rings: &[Vec<Ip>]) -> Option<P> {
    let outer = rings.first()?;
    let (ymin, ymax) = outer.iter().fold((i64::MAX, i64::MIN), |(lo, hi), q| (lo.min(q[1]), hi.max(q[1])));
    let mut best: Option<(f64, P)> = None;
    if ymax > ymin {
        for k in 0..7i64 {
            // Doubled y of a half-integer row inside (ymin, ymax).
            let row = ymin + ((ymax - ymin) * (2 * k + 1)) / 14;
            let y2 = (2 * row + 1).clamp(2 * ymin + 1, 2 * ymax - 1);
            let y = y2 as f64 / 2.0;
            let mut xs: Vec<f64> = Vec::new();
            for r in rings {
                let n = r.len();
                for i in 0..n {
                    let (a, b) = (r[i], r[(i + 1) % n]);
                    let (a2, b2) = (2 * a[1], 2 * b[1]);
                    if (a2 < y2 && b2 > y2) || (b2 < y2 && a2 > y2) {
                        let t = (y - a[1] as f64) / ((b[1] - a[1]) as f64);
                        xs.push(a[0] as f64 + (b[0] - a[0]) as f64 * t);
                    }
                }
            }
            xs.sort_by(f64::total_cmp);
            for w in xs.as_chunks::<2>().0 {
                let width = w[1] - w[0];
                if width <= 1e-9 || best.is_some_and(|(b, _)| b >= width) {
                    continue;
                }
                let xm = (w[0] + w[1]) / 2.0;
                let q = [(xm * 4.0).round() as i64, y2 * 2];
                if strictly_inside(rings, q, 4) {
                    best = Some((width, P::new(q[0] as f64 / 4.0 / GRID, q[1] as f64 / 4.0 / GRID)));
                }
            }
        }
    }
    if let Some((_, p)) = best {
        return Some(p);
    }
    for scale in [4i64, 64, 1024, 16384] {
        for r in rings {
            let n = r.len();
            for i in 0..n.min(256) {
                let (a, b) = (r[i], r[(i + 1) % n]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let half = scale / 2;
                let q = [half * (a[0] + b[0]) - d[1].signum(), half * (a[1] + b[1]) + d[0].signum()];
                if strictly_inside(rings, q, scale) {
                    let s = (scale as f64) * GRID;
                    return Some(P::new(q[0] as f64 / s, q[1] as f64 / s));
                }
            }
        }
    }
    None
}

/// Crockford base32 of the first 130 bits of a digest: 26 characters, the shape of the model's IDs.
fn base32_26(d: &[u8]) -> String {
    const A: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut out = String::with_capacity(26);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &byte in d {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 && out.len() < 26 {
            bits -= 5;
            out.push(char::from(A[((acc >> bits) & 31) as usize]));
        }
        if out.len() == 26 {
            break;
        }
    }
    out
}

/// Rounds a path parameter for boundary keys (1e-6).
#[must_use]
pub fn key_param(s: f64) -> i64 {
    (s * 1e6).round() as i64
}

impl LayerMap {
    /// The origin vertex of a half-edge.
    #[must_use]
    pub fn origin(&self, h: usize) -> usize {
        if h.is_multiple_of(2) { self.edges[h / 2].u } else { self.edges[h / 2].v }
    }

    /// The destination vertex of a half-edge.
    #[must_use]
    pub fn dest(&self, h: usize) -> usize {
        self.origin(h ^ 1)
    }

    /// The face on the left of a half-edge.
    #[must_use]
    pub fn face_of_half(&self, h: usize) -> usize {
        self.face_of_cycle[self.cycle_of[h]]
    }

    /// The polygon of a cycle (grid units).
    #[must_use]
    pub fn ring(&self, c: usize) -> Vec<Ip> {
        self.cycles[c].half.iter().map(|&h| self.verts[self.origin(h)]).collect()
    }

    fn rings(&self, f: usize) -> Vec<Vec<Ip>> {
        self.faces[f].outer.iter().chain(self.faces[f].holes.iter()).map(|&c| self.ring(c)).collect()
    }

    /// The stretches of strokes along the half-edges of some cycles, merged per stroke and side.
    fn pieces_of(&self, cycles: &[usize]) -> Vec<Piece> {
        let mut by: BTreeMap<(usize, bool), Vec<Stretch>> = BTreeMap::new();
        for &c in cycles {
            for &h in &self.cycles[c].half {
                let e = &self.edges[h / 2];
                let (a, b) = (unsnap(self.verts[e.u]), unsnap(self.verts[e.v]));
                let len = a.dist(b);
                for s in &e.srcs {
                    // The face lies on the left of the half-edge; it is the stroke's left when they run alike.
                    let along = h.is_multiple_of(2) == s.forward;
                    let (lo, hi) = if s.s_u <= s.s_v { (s.s_u, s.s_v) } else { (s.s_v, s.s_u) };
                    by.entry((s.stroke, along)).or_default().push((lo, hi, len));
                }
            }
        }
        let mut out = Vec::new();
        for ((stroke, left), mut list) in by {
            list.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
            let mut cur: Option<(f64, f64, f64)> = None;
            for (lo, hi, len) in list {
                match &mut cur {
                    Some(c) if lo <= c.1 + 1e-9 => {
                        c.1 = c.1.max(hi);
                        c.2 += len;
                    }
                    _ => {
                        if let Some(c) = cur.take() {
                            out.push(Piece { stroke, left, s0: c.0, s1: c.1, length: c.2 });
                        }
                        cur = Some((lo, hi, len));
                    }
                }
            }
            if let Some(c) = cur {
                out.push(Piece { stroke, left, s0: c.0, s1: c.1, length: c.2 });
            }
        }
        out
    }

    fn make_zones(&mut self, work: &mut Work) -> Result<(), GeomError> {
        let mut zones: Vec<Zone> = Vec::new();
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for f in 1..self.faces.len() {
            let face = &self.faces[f];
            let (Some(outer), Some(interior)) = (face.outer, face.interior) else { continue };
            if !face.brushes.is_empty() {
                continue;
            }
            let cycles: Vec<usize> = std::iter::once(outer).chain(face.holes.iter().copied()).collect();
            work.spend_n(
                cycles.iter().map(|&c| self.cycles[c].half.len()).sum::<usize>().saturating_add(UNITS_PER_ZONE),
            )?;
            let pieces = self.pieces_of(&cycles);
            let mut tokens: Vec<String> = pieces
                .iter()
                .map(|p| {
                    let (id, kind) = &self.strokes[p.stroke];
                    let side = if p.left { 'L' } else { 'R' };
                    if *kind == EdgeKind::Brush {
                        format!("{id}:{side}:{}:{}", key_param(p.s0), key_param(p.s1))
                    } else {
                        format!("{id}:{side}")
                    }
                })
                .collect();
            tokens.sort();
            let mut text = tokens.join("|");
            let mut id;
            let mut round = 0u32;
            loop {
                let digest = Sha256::digest(text.as_bytes());
                id = format!("zn_{}", base32_26(&digest));
                if !seen.contains_key(&id) {
                    break;
                }
                round += 1;
                text = format!("{text}#{round}");
            }
            seen.insert(id.clone(), zones.len());
            let area2: i128 = cycles.iter().map(|&c| self.cycles[c].area2).sum();
            let b = self.cycles[outer].bbox;
            let bounds = Rect::of(unsnap([b[0], b[1]])).with(unsnap([b[2], b[3]]));
            let boundary: Vec<String> =
                pieces.iter().map(|p| self.strokes[p.stroke].0.clone()).collect::<BTreeSet<_>>().into_iter().collect();
            let visible_boundary = pieces.iter().any(|p| self.strokes[p.stroke].1.visible());
            zones.push(Zone {
                id,
                face: f,
                area: area2 as f64 / 2.0 / (GRID * GRID),
                area2,
                bounds,
                interior,
                boundary,
                holes: Vec::new(),
                parent: None,
                pieces,
                visible_boundary,
            });
        }
        for (z, zone) in zones.iter().enumerate() {
            self.faces[zone.face].zone = Some(z);
        }
        // Holes and parents.
        let mut comp_host: BTreeMap<usize, usize> = BTreeMap::new();
        for (c, cy) in self.cycles.iter().enumerate() {
            if cy.area2 <= 0 {
                comp_host.insert(cy.comp, self.face_of_cycle[c]);
            }
        }
        for zone in &mut zones {
            let f = zone.face;
            let mut holes = Vec::new();
            for &c in &self.faces[f].holes {
                let pieces = self.pieces_of(&[c]);
                let boundary: Vec<String> = pieces
                    .iter()
                    .map(|p| self.strokes[p.stroke].0.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                // The zone across the hole boundary along its longest stretch.
                let mut lengths: BTreeMap<usize, f64> = BTreeMap::new();
                for &h in &self.cycles[c].half {
                    if let Some(inner) = self.faces[self.face_of_half(h ^ 1)].zone {
                        let e = &self.edges[h / 2];
                        *lengths.entry(inner).or_default() += unsnap(self.verts[e.u]).dist(unsnap(self.verts[e.v]));
                    }
                }
                let zone = lengths.into_iter().fold(None, |best: Option<(usize, f64)>, (k, l)| match best {
                    Some((_, bl)) if bl >= l => best,
                    _ => Some((k, l)),
                });
                holes.push(ZoneHole { boundary, zone: zone.map(|x| x.0) });
            }
            let outer_comp = self.faces[f].outer.map(|c| self.cycles[c].comp);
            let parent = outer_comp.and_then(|k| comp_host.get(&k)).and_then(|&host| self.faces[host].zone);
            zone.holes = holes;
            zone.parent = parent;
        }
        self.zones = zones;
        Ok(())
    }

    /// The face containing `p` (du) strictly; `None` on a boundary or outside every bounded face (0 = unbounded).
    #[must_use]
    pub fn face_at(&self, p: P) -> Option<usize> {
        let q = [(p.x * GRID * 4.0).round() as i64, (p.y * GRID * 4.0).round() as i64];
        let mut best: Option<(i128, usize)> = None;
        for f in self.face_index.query(q[0].div_euclid(4), q[1].div_euclid(4)) {
            let Some(outer) = self.faces[f].outer else { continue };
            let c = &self.cycles[outer];
            if q[0] < c.bbox[0] * 4 || q[0] > c.bbox[2] * 4 || q[1] < c.bbox[1] * 4 || q[1] > c.bbox[3] * 4 {
                continue;
            }
            if best.is_some_and(|(a, _)| a <= c.area2) {
                continue;
            }
            let rings = self.rings(f);
            let (w, on) = winding(&rings[0], q, 4);
            if on {
                return None;
            }
            if w != 0 {
                if rings[1..].iter().any(|h| winding(h, q, 4).1) {
                    return None;
                }
                if rings[1..].iter().all(|h| winding(h, q, 4).0 == 0) {
                    best = Some((c.area2, f));
                }
            }
        }
        Some(best.map_or(0, |(_, f)| f))
    }

    /// The zone containing `p`, if any.
    #[must_use]
    pub fn zone_at(&self, p: P) -> Option<usize> {
        self.face_at(p).and_then(|f| self.faces[f].zone)
    }

    /// The zones on the left and right of a stroke (by longest stretch).
    #[must_use]
    pub fn stroke_sides(&self, stroke: usize) -> (Option<usize>, Option<usize>) {
        let mut left: BTreeMap<usize, f64> = BTreeMap::new();
        let mut right: BTreeMap<usize, f64> = BTreeMap::new();
        for &(e, fwd) in &self.stroke_edges[stroke] {
            let edge = &self.edges[e];
            let len = unsnap(self.verts[edge.u]).dist(unsnap(self.verts[edge.v]));
            // The half-edge running with the stroke has the stroke's left face on its left.
            let h = e * 2 + usize::from(!fwd);
            if let Some(z) = self.faces[self.face_of_half(h)].zone {
                *left.entry(z).or_default() += len;
            }
            if let Some(z) = self.faces[self.face_of_half(h ^ 1)].zone {
                *right.entry(z).or_default() += len;
            }
        }
        let pick = |m: BTreeMap<usize, f64>| {
            m.into_iter().fold(None, |best: Option<(usize, f64)>, (k, l)| match best {
                Some((_, bl)) if bl >= l => best,
                _ => Some((k, l)),
            })
        };
        (pick(left).map(|x| x.0), pick(right).map(|x| x.0))
    }

    /// The polygon rings of a zone (du), outer first.
    #[must_use]
    pub fn zone_rings(&self, z: usize) -> Vec<Vec<P>> {
        self.rings(self.zones[z].face).into_iter().map(|r| r.into_iter().map(unsnap).collect()).collect()
    }

    /// The polygon rings of a zone on the grid, outer first.
    #[must_use]
    pub fn zone_rings_grid(&self, z: usize) -> Vec<Vec<Ip>> {
        self.rings(self.zones[z].face)
    }

    /// The index of a zone by ID.
    #[must_use]
    pub fn zone_by_id(&self, id: &str) -> Option<usize> {
        self.zones.iter().position(|z| z.id == id)
    }

    /// A cost measure for caches: vertices plus half-edges.
    #[must_use]
    pub fn cost(&self) -> usize {
        self.verts.len() + self.edges.len() * 2
    }
}
