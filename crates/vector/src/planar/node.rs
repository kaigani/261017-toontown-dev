//! Persistent noding of a layer's strokes (02 §3.2, §3.12 item 1; SI-059).
//!
//! Pencil and invisible strokes are split where they cross any stroke of the layer (themselves included) and where an
//! open stroke's end meets another stroke (T-junctions). The pieces meet at exactly the same coordinate, so junction
//! anchors are shared. A junction within [`JUNCTION_TOLERANCE`] of an existing anchor uses that anchor (no new piece),
//! so noding an already noded layer changes nothing and unchanged strokes keep their IDs. Only strokes the edit
//! changed have their ends moved onto a junction; unchanged strokes are only split. **Brush outlines are never split
//! or moved**: their crossings are vertices of the derived graph only, so a brush spine never goes stale through
//! noding.

use std::collections::{BTreeMap, HashMap};

use super::flat::{self, Flat};
use super::graph::{EdgeKind, Ip, snap};
use crate::geom::{Cubic, P};
use crate::path::{Anchor, GeomError, Path};
use crate::work::Work;

/// Distance (du) within which two junction candidates, or a junction and an anchor, are the same point.
pub const JUNCTION_TOLERANCE: f64 = 1.0 / 32.0;

/// A stroke to node.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeStroke {
    /// Stroke ID.
    pub id: String,
    /// Kind.
    pub kind: EdgeKind,
    /// Path (centre path; a brush stroke's outline).
    pub path: Path,
    /// Whether the edit being committed added or changed the stroke (only such strokes have their ends moved).
    pub changed: bool,
}

/// What noding does to one stroke.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Split into pieces (in stroke order); the stroke is replaced by them.
    Split(Vec<Path>),
    /// Kept with its ends moved onto junctions.
    Moved(Path),
}

/// The changes of a noding pass, by input index (strokes not listed are unchanged).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Noding {
    /// Outcomes.
    pub outcomes: BTreeMap<usize, Outcome>,
}

/// A junction on a stroke: path parameter and the coordinate the pieces meet at.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Junction {
    s: f64,
    at: P,
}

/// A crossing or T-junction before resolution: where it was found and the two strokes meeting there.
struct Record {
    at: P,
    parts: [(usize, f64); 2],
}

struct EdgeRef {
    stroke: usize,
    a: P,
    b: P,
    /// The ends on the 1/64 du grid (exact predicates, C22-R2-01).
    ia: Ip,
    ib: Ip,
    sa: f64,
    sb: f64,
    /// Index of the edge within its stroke's polyline.
    k: usize,
}

fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Orientation of `c` relative to `a`→`b` on the grid: positive left, negative right, zero collinear (exact).
use super::predicates::orient;

fn on_segment(p: Ip, a: Ip, b: Ip) -> bool {
    orient(a, b, p) == 0
        && p[0] >= a[0].min(b[0])
        && p[0] <= a[0].max(b[0])
        && p[1] >= a[1].min(b[1])
        && p[1] <= a[1].max(b[1])
}

/// The fraction of `p`'s projection along `a`→`b` (only used to place a junction's path parameter).
fn fraction(p: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 }
}

/// How two segments meet, decided by exact orientation signs on the snapped 1/64 du grid (ADR-0017 §1; C22-R2-01).
enum Meet {
    /// A crossing strictly inside both: the fractions along each.
    Proper(f64, f64),
    /// An end of either lies on the other: the fractions (0 or 1 at an end) and the point (that end).
    Vertex(f64, f64, P),
}

/// How two flattened edges meet on the grid; `None` when they do not, or are collinear (an overlap is never a junction).
fn meet(e: &EdgeRef, f: &EdgeRef) -> Option<Meet> {
    let (o1, o2) = (orient(e.ia, e.ib, f.ia), orient(e.ia, e.ib, f.ib));
    let (o3, o4) = (orient(f.ia, f.ib, e.ia), orient(f.ia, f.ib, e.ib));
    if o1 == 0 && o2 == 0 {
        return None;
    }
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        // The crossing point in floating point from the stored geometry (unchanged coordinates for ordinary input),
        // else, where floating point degenerates, from the exact grid fractions.
        let (r, sd) = (e.b - e.a, f.b - f.a);
        let den = cross(r, sd);
        let inside = |x: f64| x > 0.0 && x < 1.0;
        if den != 0.0 {
            let (t, u) = (cross(f.a - e.a, sd) / den, cross(f.a - e.a, r) / den);
            if inside(t) && inside(u) {
                return Some(Meet::Proper(t, u));
            }
        }
        let den = o3 - o4;
        let t = (o3 as f64 / den as f64).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
        let du = o1 - o2;
        let u = (o1 as f64 / du as f64).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
        return Some(Meet::Proper(t, u));
    }
    if on_segment(f.ia, e.ia, e.ib) {
        return Some(Meet::Vertex(end_fraction(f.ia, e, f.a), 0.0, f.a));
    }
    if on_segment(f.ib, e.ia, e.ib) {
        return Some(Meet::Vertex(end_fraction(f.ib, e, f.b), 1.0, f.b));
    }
    if on_segment(e.ia, f.ia, f.ib) {
        return Some(Meet::Vertex(0.0, end_fraction(e.ia, f, e.a), e.a));
    }
    if on_segment(e.ib, f.ia, f.ib) {
        return Some(Meet::Vertex(1.0, end_fraction(e.ib, f, e.b), e.b));
    }
    None
}

/// The fraction along `e` of a grid point on it: exactly 0 or 1 at its ends.
fn end_fraction(p: Ip, e: &EdgeRef, at: P) -> f64 {
    if p == e.ia {
        0.0
    } else if p == e.ib {
        1.0
    } else {
        fraction(at, e.a, e.b).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON)
    }
}

type Dir = [i128; 2];

/// The grid directions of a stroke's polyline away from a point on edge `k` at fraction `t` (0 or 1 at a vertex), to
/// the nearest vertices on either side that snap elsewhere; `None` at an open path's end.
fn directions(f: &Flat, k: usize, t: f64, at: P) -> Option<(Dir, Dir)> {
    let n = f.pts.len();
    let next_of = |v: usize| {
        if v + 1 < n {
            Some(v + 1)
        } else if f.closed {
            Some(0)
        } else {
            None
        }
    };
    let prev_of = |v: usize| {
        if v > 0 {
            Some(v - 1)
        } else if f.closed {
            Some(n - 1)
        } else {
            None
        }
    };
    let v = snap(at);
    let mut p = if t <= 0.0 { prev_of(k)? } else { k };
    let mut q = if t >= 1.0 { next_of(next_of(k)?)? } else { next_of(k)? };
    for _ in 0..n {
        if snap(f.pts[p]) != v {
            break;
        }
        p = prev_of(p)?;
    }
    for _ in 0..n {
        if snap(f.pts[q]) != v {
            break;
        }
        q = next_of(q)?;
    }
    let d = |w: Ip| [i128::from(w[0] - v[0]), i128::from(w[1] - v[1])];
    let (dp, dq) = (d(snap(f.pts[p])), d(snap(f.pts[q])));
    (dp != [0, 0] && dq != [0, 0]).then_some((dp, dq))
}

/// The grid direction of an open path at its start (`at_end` false) or end, away from the end towards the nearest
/// vertex that snaps elsewhere; `None` when every vertex snaps onto the end.
fn end_direction(f: &Flat, at_end: bool) -> Option<Dir> {
    let n = f.pts.len();
    let order: Vec<usize> = if at_end { (0..n).rev().collect() } else { (0..n).collect() };
    let v = snap(f.pts[*order.first()?]);
    let w = order.iter().map(|&k| snap(f.pts[k])).find(|w| *w != v)?;
    Some([i128::from(w[0] - v[0]), i128::from(w[1] - v[1])])
}

fn cross_i(a: Dir, b: Dir) -> i128 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot_i(a: Dir, b: Dir) -> i128 {
    a[0] * b[0] + a[1] * b[1]
}

/// Whether `x` lies strictly inside the counter-clockwise arc of directions from `u` to `w` (exact).
fn inside_arc(u: Dir, w: Dir, x: Dir) -> bool {
    let c = cross_i(u, w);
    if c > 0 {
        cross_i(u, x) > 0 && cross_i(x, w) > 0
    } else if c < 0 {
        !(cross_i(w, x) >= 0 && cross_i(x, u) >= 0)
    } else if dot_i(u, w) < 0 {
        cross_i(u, x) > 0
    } else {
        false
    }
}

/// Whether two polylines through one point cross there: the second's two directions lie strictly on different sides of
/// the first (exact signs on the grid). A touch, a turn-back, or a direction shared by both (overlap, retracing) is not
/// a crossing.
fn crosses_at(a: (Dir, Dir), b: (Dir, Dir)) -> bool {
    let same = |x: Dir, y: Dir| cross_i(x, y) == 0 && dot_i(x, y) > 0;
    if [a.0, a.1].iter().any(|x| same(*x, b.0) || same(*x, b.1)) || same(a.0, a.1) || same(b.0, b.1) {
        return false;
    }
    inside_arc(a.1, a.0, b.0) != inside_arc(a.1, a.0, b.1)
}

/// The closest point of a segment to `p`: fraction and point.
fn closest(p: P, a: P, b: P) -> (f64, P) {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (t, a + ab * t)
}

/// Whether a stroke kind is split by noding.
fn splittable(k: EdgeKind) -> bool {
    k != EdgeKind::Brush
}

/// Nodes a layer's strokes.
///
/// # Errors
/// `limit.exceeded` past the work budget.
pub fn node(strokes: &[NodeStroke], work: &mut Work) -> Result<Noding, GeomError> {
    let flats: Vec<Flat> = strokes.iter().map(|s| flat::flatten(&s.path, work)).collect::<Result<_, _>>()?;
    let mut edges: Vec<EdgeRef> = Vec::new();
    for (i, f) in flats.iter().enumerate() {
        for (k, (a, b, sa, sb)) in f.edges(strokes[i].path.segments().len()).into_iter().enumerate() {
            if a != b {
                edges.push(EdgeRef { stroke: i, a, b, ia: snap(a), ib: snap(b), sa, sb, k });
            }
        }
    }
    work.spend_n(edges.len())?;
    if edges.is_empty() {
        return Ok(Noding::default());
    }
    let total: f64 = edges.iter().map(|e| e.a.dist(e.b)).sum();
    let size = (total / edges.len() as f64).clamp(0.125, 1.0e5) * 2.0;
    let cell = |x: f64| (x / size).floor() as i64;
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (n, e) in edges.iter().enumerate() {
        let (x0, x1) = (cell(e.a.x.min(e.b.x) - JUNCTION_TOLERANCE), cell(e.a.x.max(e.b.x) + JUNCTION_TOLERANCE));
        let (y0, y1) = (cell(e.a.y.min(e.b.y) - JUNCTION_TOLERANCE), cell(e.a.y.max(e.b.y) + JUNCTION_TOLERANCE));
        work.spend(u64::try_from((x1 - x0 + 1).saturating_mul(y1 - y0 + 1)).unwrap_or(u64::MAX))?;
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                grid.entry((cx, cy)).or_default().push(n);
            }
        }
    }
    // Junction records: one per crossing or T-junction, with its two participants (stroke, parameter).
    let mut records: Vec<Record> = Vec::new();
    // Crossings (each pair tested once, in the cell of the low corner of the boxes' overlap).
    let mut cells: Vec<(&(i64, i64), &Vec<usize>)> = grid.iter().collect();
    cells.sort_by_key(|(k, _)| **k);
    for (&(cx, cy), list) in cells {
        // Every pair of the cell is tested (about one bounding-box comparison each, a few nanoseconds).
        work.spend_n(list.len().saturating_mul(list.len()).saturating_mul(3) / 2)?;
        for (n, &i) in list.iter().enumerate() {
            for &j in &list[n + 1..] {
                let (e, f) = (&edges[i], &edges[j]);
                if e.stroke == f.stroke {
                    // Adjacent edges of one polyline meet at their shared vertex, not in a crossing.
                    let count = flats[e.stroke].pts.len();
                    let closed = flats[e.stroke].closed;
                    let adjacent =
                        e.k.abs_diff(f.k) == 1 || (closed && (e.k.max(f.k) + 1 == count && e.k.min(f.k) == 0));
                    if adjacent {
                        continue;
                    }
                }
                let lo = (
                    cell(e.a.x.min(e.b.x).max(f.a.x.min(f.b.x)) - JUNCTION_TOLERANCE),
                    cell(e.a.y.min(e.b.y).max(f.a.y.min(f.b.y)) - JUNCTION_TOLERANCE),
                );
                if lo != (cx, cy) {
                    continue;
                }
                if !splittable(strokes[e.stroke].kind) && !splittable(strokes[f.stroke].kind) {
                    continue;
                }
                match meet(e, f) {
                    Some(Meet::Proper(t, u)) => records.push(Record {
                        at: e.a.lerp(e.b, t),
                        parts: [
                            (e.stroke, flat::param_between(e.sa, e.sb, t)),
                            (f.stroke, flat::param_between(f.sa, f.sb, u)),
                        ],
                    }),
                    Some(Meet::Vertex(t, u, at))
                        if let (Some(da), Some(db)) =
                            (directions(&flats[e.stroke], e.k, t, at), directions(&flats[f.stroke], f.k, u, at))
                            && crosses_at(da, db) =>
                    {
                        // A stroke crossing another exactly at a vertex of either (02 §3.12 item 1); touches,
                        // turn-backs, shared directions (retracing) and path ends (the T-junction rule's) are not.
                        records.push(Record {
                            at,
                            parts: [
                                (e.stroke, flat::param_between(e.sa, e.sb, t)),
                                (f.stroke, flat::param_between(f.sa, f.sb, u)),
                            ],
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    // T-junctions: an open stroke's end on another stroke.
    for (i, s) in strokes.iter().enumerate() {
        if s.path.closed || !splittable(s.kind) || flats[i].pts.len() < 2 {
            continue;
        }
        let count = s.path.segments().len() as f64;
        let ends = [(0.0, flats[i].pts[0]), (count, flats[i].pts[flats[i].pts.len() - 1])];
        for (s_end, p) in ends {
            // The end's own direction on the grid (towards the nearest vertex that snaps elsewhere): an end running
            // along another stroke (overlap, retracing, continuation) is no junction with it, by the same exact
            // collinearity test as crossings (C22-R3-03); only a non-parallel end makes a T-junction.
            let Some(dir) = end_direction(&flats[i], s_end > 0.0) else { continue };
            // The nearest point of every other stroke within the tolerance: an end where several strokes meet (the
            // neighbouring piece of a split stroke and a line through the junction) is a junction with each of them.
            let mut best: BTreeMap<usize, (f64, usize, f64, P)> = BTreeMap::new();
            let key = (cell(p.x), cell(p.y));
            let Some(list) = grid.get(&key) else { continue };
            work.spend_n(list.len())?;
            for &n in list {
                let e = &edges[n];
                // The stroke's own interior counts too (an end touching its own path is a junction like any other;
                // M2 C2.4, found while testing incremental noding: once split, the touch would be found as a
                // piece-on-piece T-junction, so settling was not a fixpoint), but not the Bézier segment the end
                // belongs to (its flattened edges all run from the end).
                let own_end_segment = e.stroke == i && if s_end > 0.0 { e.sa >= count - 1.0 } else { e.sb <= 1.0 };
                if own_end_segment {
                    continue;
                }
                let along = [i128::from(e.ib[0] - e.ia[0]), i128::from(e.ib[1] - e.ia[1])];
                if along == [0, 0] || cross_i(dir, along) == 0 {
                    continue;
                }
                let (t, q) = closest(p, e.a, e.b);
                let d = p.dist(q);
                if d <= JUNCTION_TOLERANCE && best.get(&e.stroke).is_none_or(|b| d < b.0) {
                    best.insert(e.stroke, (d, n, t, q));
                }
            }
            // The nearest stroke, and every other stroke as near (ties: the end-to-end neighbour and a line through
            // the same point); strokes merely passing within the tolerance are not pulled onto the junction.
            let nearest = best.values().map(|b| b.0).fold(f64::INFINITY, f64::min);
            for (_, n, t, q) in best.into_values().filter(|b| b.0 <= nearest + 1e-9) {
                let e = &edges[n];
                let s_other = flat::param_between(e.sa, e.sb, t);
                // Where they meet: the changed stroke moves onto the other one; an unchanged end stays put.
                let at = if strokes[i].changed { q } else { p };
                records.push(Record { at, parts: [(i, s_end), (e.stroke, s_other)] });
            }
        }
    }
    // Each junction of a pencil or invisible stroke is resolved and may become a piece (a split and a new stroke of a
    // few anchors): charged before the records are clustered, so a layer with too many crossings is refused first.
    let splits = records.iter().flat_map(|r| r.parts).filter(|(k, _)| splittable(strokes[*k].kind)).count();
    work.spend_n(splits.saturating_mul(256))?;
    let junctions = resolve(strokes, &records, work)?;
    let mut outcomes = BTreeMap::new();
    for (i, s) in strokes.iter().enumerate() {
        if !splittable(s.kind) || junctions[i].is_empty() {
            continue;
        }
        if let Some(o) = apply(s, &junctions[i]) {
            outcomes.insert(i, o);
        }
    }
    Ok(Noding { outcomes })
}

/// Resolves every junction once for all of its participants (C22-R1-01): a record within [`JUNCTION_TOLERANCE`] of a
/// junction's first record belongs to that junction. Its coordinate is the nearest existing anchor of an unchanged pencil or
/// invisible participant (then lowest stroke index and parameter), else the point of the junction's first record.
/// Every participant gets that exact coordinate, so the pieces meeting there share their junction anchor.
fn resolve(strokes: &[NodeStroke], records: &[Record], work: &mut Work) -> Result<Vec<Vec<Junction>>, GeomError> {
    work.spend_n(records.len().saturating_mul(16))?;
    // Records in order: each joins the lowest-numbered junction whose first record lies within the tolerance, else
    // starts a junction of its own (no chaining, so no participant moves by more than the tolerance plus the distance
    // to an authoritative anchor).
    let cell = |p: P| ((p.x / JUNCTION_TOLERANCE).floor() as i64, (p.y / JUNCTION_TOLERANCE).floor() as i64);
    let mut firsts: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, r) in records.iter().enumerate() {
        let (cx, cy) = cell(r.at);
        let mut joined: Option<usize> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(list) = firsts.get(&(cx + dx, cy + dy)) else { continue };
                work.spend_n(list.len())?;
                for &f in list {
                    if records[f].at.dist(r.at) <= JUNCTION_TOLERANCE && joined.is_none_or(|j| f < j) {
                        joined = Some(f);
                    }
                }
            }
        }
        match joined {
            Some(f) => clusters.entry(f).or_default().push(i),
            None => {
                firsts.entry((cx, cy)).or_default().push(i);
                clusters.entry(i).or_default().push(i);
            }
        }
    }
    let mut junctions: Vec<Vec<Junction>> = vec![Vec::new(); strokes.len()];
    for (first, members) in clusters {
        let mut best: Option<(f64, usize, f64, P)> = None;
        for &m in &members {
            let r = &records[m];
            for &(k, sk) in &r.parts {
                let st = &strokes[k];
                if st.changed || !splittable(st.kind) {
                    continue;
                }
                let probe = Junction { s: sk, at: r.at };
                let Some((param, p)) = near_anchor(&st.path, st.path.segments().len(), &probe) else { continue };
                let key = (p.dist(r.at), k, param);
                if best.is_none_or(|b| key.partial_cmp(&(b.0, b.1, b.2)) == Some(std::cmp::Ordering::Less)) {
                    best = Some((key.0, k, param, p));
                }
            }
        }
        let at = best.map_or(records[first].at, |b| b.3);
        for &m in &members {
            for &(k, sk) in &records[m].parts {
                junctions[k].push(Junction { s: sk, at });
            }
        }
    }
    Ok(junctions)
}

/// The anchor at either end of the segment holding `j` (parameter), when within the junction tolerance of it: a
/// junction coincides with an existing anchor only along the path, so these two are the only candidates (constant
/// work per junction).
fn near_anchor(path: &Path, segs: usize, j: &Junction) -> Option<(f64, P)> {
    let n = path.anchors.len();
    if n == 0 {
        return None;
    }
    let base = (j.s.floor().max(0.0) as usize).min(segs);
    let mut best: Option<(f64, f64, P)> = None;
    for k in [base, base + 1] {
        if k > segs {
            continue;
        }
        let a = path.anchors[k % n];
        let d = a.p.dist(j.at);
        if d <= JUNCTION_TOLERANCE && best.is_none_or(|b| d < b.0) {
            // The end of an open path is the last anchor (parameter = segment count); a closed path's end is its start.
            let param = if path.closed && k == segs { 0.0 } else { k as f64 };
            best = Some((d, param, a.p));
        }
    }
    best.map(|(_, k, p)| (k, p))
}

/// Splits or moves a stroke at its junctions; `None` when nothing changes.
fn apply(s: &NodeStroke, js: &[Junction]) -> Option<Outcome> {
    let segs = s.path.segments();
    let count = segs.len() as f64;
    if segs.is_empty() {
        return None;
    }
    // Resolve each junction: at an existing anchor (no curve split) or at a curve point.
    let mut cuts: Vec<Junction> = Vec::new();
    let mut moves: Vec<(f64, P)> = Vec::new();
    for j in js {
        // The coordinate is already resolved for every participant: an anchor near it moves onto it (a no-op for the
        // authoritative anchor of an unchanged stroke).
        let resolved = match near_anchor(&s.path, segs.len(), j) {
            Some((k, _)) => Junction { s: k, at: j.at },
            None => *j,
        };
        let is_end = !s.path.closed && (resolved.s <= 0.0 || resolved.s >= count);
        if is_end {
            if resolved.at != anchor_at(&s.path, resolved.s) && moved_end(&moves, resolved.s).is_none() {
                moves.push((resolved.s, resolved.at));
            }
            continue;
        }
        cuts.push(resolved);
    }
    // A path that passes through a junction more than once is cut at every pass: each anchor exactly at a cut's point
    // is cut too, so no piece runs through the junction (and faces two zones on one side; M2 C2.4, found by the
    // topology properties). An open path's ends are its own junctions already.
    let anchor_cuts: Vec<Junction> = cuts
        .iter()
        .flat_map(|c| {
            s.path.anchors.iter().enumerate().filter_map(move |(k, a)| {
                let param = k as f64;
                let interior = s.path.closed || (k > 0 && k + 1 < s.path.anchors.len());
                (interior && a.p == c.at && (param - c.s).abs() > 1e-9).then_some(Junction { s: param, at: c.at })
            })
        })
        .collect();
    cuts.extend(anchor_cuts);
    cuts.sort_by(|a, b| a.s.total_cmp(&b.s));
    // Merge cuts at one point.
    let mut merged: Vec<Junction> = Vec::new();
    for c in cuts {
        match merged.last() {
            Some(m) if same_place(&segs, m, &c, s.path.closed) => {}
            _ => merged.push(c),
        }
    }
    if s.path.closed && merged.len() >= 2 {
        let (first, last) = (merged[0], merged[merged.len() - 1]);
        if same_place(&segs, &last, &Junction { s: first.s + count, at: first.at }, true) {
            merged.pop();
        }
    }
    // A cut at the same place as an open path's end is that end's junction: the end moves onto it and no piece of
    // zero length is left between them.
    if !s.path.closed {
        let start = Junction { s: 0.0, at: moved_end(&moves, 0.0).unwrap_or(s.path.anchors[0].p) };
        if let Some(first) = merged.first().copied()
            && same_place(&segs, &start, &first, false)
        {
            merged.remove(0);
            moves.retain(|m| m.0 > 0.0);
            if first.at != s.path.anchors[0].p {
                moves.push((0.0, first.at));
            }
        }
        let finish =
            Junction { s: count, at: moved_end(&moves, count).unwrap_or(s.path.anchors[s.path.anchors.len() - 1].p) };
        if let Some(last) = merged.last().copied()
            && same_place(&segs, &last, &finish, false)
        {
            merged.pop();
            moves.retain(|m| m.0 < count);
            if last.at != s.path.anchors[s.path.anchors.len() - 1].p {
                moves.push((count, last.at));
            }
        }
    }
    if merged.is_empty() {
        if moves.is_empty() {
            return None;
        }
        let mut p = s.path.clone();
        for (k, at) in moves {
            let idx = if k <= 0.0 { 0 } else { p.anchors.len() - 1 };
            p.anchors[idx].p = at;
        }
        return Some(Outcome::Moved(p));
    }
    // Cut the path into pieces between consecutive cuts (cyclically for a closed path).
    let mut pieces: Vec<Path> = Vec::new();
    if s.path.closed {
        for w in 0..merged.len() {
            let (a, b) = (merged[w], merged[(w + 1) % merged.len()]);
            let end = if b.s > a.s { b.s } else { b.s + count };
            pieces.push(sub_path(&segs, a, Junction { s: end, at: b.at }));
        }
    } else {
        let start = Junction { s: 0.0, at: moved_end(&moves, 0.0).unwrap_or(s.path.anchors[0].p) };
        let finish_at = moved_end(&moves, count).unwrap_or(s.path.anchors[s.path.anchors.len() - 1].p);
        let mut prev = start;
        for c in &merged {
            pieces.push(sub_path(&segs, prev, *c));
            prev = *c;
        }
        pieces.push(sub_path(&segs, prev, Junction { s: count, at: finish_at }));
    }
    // A piece between two cuts that resolved to one junction starts and ends there and stays within the junction
    // tolerance: it carries no geometry, and kept it would be a loop crossing its neighbours (micro-zones beside a
    // coincident edge; M2 C2.4, found by the topology properties). Its neighbours meet at that junction.
    pieces.retain(|p| p.anchors.len() >= 2 && !degenerate(p));
    Some(Outcome::Split(pieces))
}

/// Whether an open piece starts and ends at one point and every control point stays within the junction tolerance.
fn degenerate(p: &Path) -> bool {
    let (Some(first), Some(last)) = (p.anchors.first(), p.anchors.last()) else { return true };
    first.p == last.p
        && p.anchors.iter().all(|a| {
            a.p.dist(first.p) <= JUNCTION_TOLERANCE
                && (a.p + a.inh).dist(first.p) <= JUNCTION_TOLERANCE
                && (a.p + a.outh).dist(first.p) <= JUNCTION_TOLERANCE
        })
}

/// Whether two cuts (in parameter order) are one: the same junction, with the path between them staying near it (a
/// loop through one junction keeps both of its cuts).
fn same_place(segs: &[Cubic], a: &Junction, b: &Junction, closed: bool) -> bool {
    if (b.s - a.s).abs() < 1e-9 {
        return true;
    }
    if a.at.dist(b.at) > JUNCTION_TOLERANCE {
        return false;
    }
    let n = segs.len();
    let eval = |t: f64| {
        let t = if closed { t.rem_euclid(n as f64) } else { t.clamp(0.0, n as f64) };
        let j = (t.floor() as usize).min(n - 1);
        segs[j].eval(t - j as f64)
    };
    (1..8).all(|k| eval(a.s + (b.s - a.s) * f64::from(k) / 8.0).dist(a.at) <= 2.0 * JUNCTION_TOLERANCE)
}

fn moved_end(moves: &[(f64, P)], k: f64) -> Option<P> {
    moves.iter().find(|m| (m.0 - k).abs() < 1e-9).map(|m| m.1)
}

fn anchor_at(path: &Path, s: f64) -> P {
    if s <= 0.0 { path.anchors[0].p } else { path.anchors[path.anchors.len() - 1].p }
}

/// The part of a path between two parameters (`to.s` may exceed the segment count of a closed path: it wraps), with
/// its ends set to the junction coordinates.
fn sub_path(segs: &[Cubic], from: Junction, to: Junction) -> Path {
    let n = segs.len();
    let mut cubics: Vec<Cubic> = Vec::new();
    let mut straight: Vec<bool> = Vec::new();
    let (mut s, end) = (from.s, to.s);
    while s < end - 1e-12 {
        let j = (s.floor() as usize).min(usize::MAX - 1);
        let seg = segs[j % n];
        let t0 = s - j as f64;
        let t1 = ((j + 1) as f64).min(end) - j as f64;
        let part = sub_cubic(&seg, t0, t1);
        cubics.push(part);
        straight.push(is_straight(&seg));
        s = j as f64 + t1;
        if t1 >= 1.0 {
            s = (j + 1) as f64;
        }
    }
    let mut anchors: Vec<Anchor> = Vec::with_capacity(cubics.len() + 1);
    for c in &cubics {
        match anchors.last_mut() {
            Some(last) => last.outh = c.p1 - c.p0,
            None => anchors.push(Anchor { p: c.p0, inh: P::default(), outh: c.p1 - c.p0 }),
        }
        anchors.push(Anchor { p: c.p3, inh: c.p2 - c.p3, outh: P::default() });
    }
    if let Some(a) = anchors.first_mut() {
        a.p = from.at;
    }
    if let Some(a) = anchors.last_mut() {
        a.p = to.at;
    }
    // A piece of a straight segment stays a straight segment between its (junction) ends. The junction parameter is
    // interpolated along the flattened chord, which a straight cubic's parameter does not follow, so its de Casteljau
    // handles would overshoot the moved end and fold back along the line (M2 C2.4, found by the topology properties).
    for (i, st) in straight.iter().enumerate() {
        if *st && i + 1 < anchors.len() {
            anchors[i].outh = P::default();
            anchors[i + 1].inh = P::default();
        }
    }
    Path { anchors, closed: false }
}

/// Whether a cubic's control points lie on its chord (a straight segment, however parameterised).
fn is_straight(c: &Cubic) -> bool {
    let len = c.p0.dist(c.p3);
    let tol = 1e-9 * len.max(1.0);
    len > 0.0 && flat::segment_distance(c.p1, c.p0, c.p3) <= tol && flat::segment_distance(c.p2, c.p0, c.p3) <= tol
}

/// The part of a cubic between two parameters.
fn sub_cubic(c: &Cubic, t0: f64, t1: f64) -> Cubic {
    if t0 <= 0.0 && t1 >= 1.0 {
        return *c;
    }
    let (_, right) = if t0 > 0.0 { c.split(t0) } else { (*c, *c) };
    if t1 >= 1.0 {
        return right;
    }
    let u = if t0 < 1.0 { (t1 - t0) / (1.0 - t0) } else { 1.0 };
    right.split(u.clamp(0.0, 1.0)).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, pts: &[(f64, f64)], changed: bool) -> NodeStroke {
        NodeStroke {
            id: id.to_owned(),
            kind: EdgeKind::Pencil,
            path: Path { anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: false },
            changed,
        }
    }

    fn run(s: &[NodeStroke]) -> Noding {
        node(s, &mut Work::unlimited("t")).unwrap_or_default()
    }

    #[test]
    fn crossing_lines_split_both_at_one_shared_point() {
        let n = run(&[line("a", &[(0.0, 0.0), (10.0, 0.0)], false), line("b", &[(5.0, -5.0), (5.0, 5.0)], true)]);
        let (Some(Outcome::Split(a)), Some(Outcome::Split(b))) = (n.outcomes.get(&0), n.outcomes.get(&1)) else {
            panic!("{n:?}")
        };
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 2);
        let ja = a[0].anchors[a[0].anchors.len() - 1].p;
        let jb = b[0].anchors[b[0].anchors.len() - 1].p;
        assert_eq!(ja, jb, "junction anchors are shared exactly");
        assert!(ja.dist(P::new(5.0, 0.0)) < 1e-9);
    }

    #[test]
    fn noding_a_noded_layer_changes_nothing() {
        let first = run(&[line("a", &[(0.0, 0.0), (10.0, 0.0)], false), line("b", &[(5.0, -5.0), (5.0, 5.0)], true)]);
        let mut strokes = Vec::new();
        for (i, o) in &first.outcomes {
            let Outcome::Split(ps) = o else { continue };
            for (k, p) in ps.iter().enumerate() {
                strokes.push(NodeStroke {
                    id: format!("{i}.{k}"),
                    kind: EdgeKind::Pencil,
                    path: p.clone(),
                    changed: false,
                });
            }
        }
        assert_eq!(strokes.len(), 4);
        assert!(run(&strokes).outcomes.is_empty());
    }

    #[test]
    fn a_changed_end_near_a_line_snaps_onto_it_and_splits_it() {
        let n = run(&[line("a", &[(0.0, 0.0), (10.0, 0.0)], false), line("t", &[(4.0, 5.0), (4.0, 0.01)], true)]);
        let Some(Outcome::Split(a)) = n.outcomes.get(&0) else { panic!("{n:?}") };
        let Some(Outcome::Moved(t)) = n.outcomes.get(&1) else { panic!("{n:?}") };
        assert_eq!(a[0].anchors[1].p, t.anchors[1].p);
        assert!(t.anchors[1].p.dist(P::new(4.0, 0.0)) < 1e-9);
    }

    #[test]
    fn a_closed_loop_split_twice_gives_two_open_pieces() {
        let sq = NodeStroke {
            id: "sq".into(),
            kind: EdgeKind::Invisible,
            path: Path {
                anchors: [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
                    .iter()
                    .map(|&(x, y)| Anchor::corner(P::new(x, y)))
                    .collect(),
                closed: true,
            },
            changed: false,
        };
        let n = run(&[sq, line("x", &[(5.0, -5.0), (5.0, 15.0)], true)]);
        let Some(Outcome::Split(ps)) = n.outcomes.get(&0) else { panic!("{n:?}") };
        assert_eq!(ps.len(), 2);
        assert!(ps.iter().all(|p| !p.closed));
    }

    #[test]
    fn three_lines_crossing_near_one_point_share_one_junction() {
        // Pairwise crossings within the junction tolerance of each other are one junction for all three strokes.
        let n = run(&[
            line("h", &[(0.0, 0.0), (10.0, 0.0)], true),
            line("v", &[(5.0, -5.0), (5.0, 5.0)], true),
            line("d", &[(0.01, -4.99), (10.01, 5.01)], true),
        ]);
        let mut ends = Vec::new();
        for k in 0..3 {
            let Some(Outcome::Split(ps)) = n.outcomes.get(&k) else { panic!("{k}: {n:?}") };
            assert_eq!(ps.len(), 2, "{k}");
            ends.push(ps[0].anchors[ps[0].anchors.len() - 1].p);
            assert_eq!(ps[1].anchors[0].p, ends[k]);
        }
        assert!(ends.iter().all(|p| *p == ends[0]), "{ends:?}");
    }

    #[test]
    fn an_unchanged_anchor_near_a_crossing_is_the_junction_for_every_stroke() {
        let n = run(&[
            line("old", &[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], false),
            line("new", &[(5.01, -5.0), (5.01, 5.0)], true),
        ]);
        let (Some(Outcome::Split(a)), Some(Outcome::Split(b))) = (n.outcomes.get(&0), n.outcomes.get(&1)) else {
            panic!("{n:?}")
        };
        assert_eq!(a[0].anchors[a[0].anchors.len() - 1].p, P::new(5.0, 0.0), "the unchanged anchor stays");
        assert_eq!(b[0].anchors[b[0].anchors.len() - 1].p, P::new(5.0, 0.0), "the changed line meets it");
        assert_eq!(b[1].anchors[0].p, P::new(5.0, 0.0));
    }

    #[test]
    fn a_line_looping_through_its_own_crossing_is_cut_on_both_passes() {
        let n = run(&[line("loop", &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (5.0, 10.0), (5.0, -5.0)], true)]);
        let Some(Outcome::Split(ps)) = n.outcomes.get(&0) else { panic!("{n:?}") };
        assert_eq!(ps.len(), 3, "{ps:?}");
        assert_eq!(ps[0].anchors[ps[0].anchors.len() - 1].p, ps[2].anchors[0].p);
    }

    #[test]
    fn a_line_through_the_meeting_point_of_two_pieces_is_split_there() {
        // Two pieces of a split line meet at (5, 0); a new line through that point touches both piece ends only.
        let n = run(&[
            line("a", &[(0.0, 0.0), (5.0, 0.0)], false),
            line("b", &[(5.0, 0.0), (10.0, 0.0)], false),
            line("x", &[(5.0, -5.0), (5.0, 5.0)], true),
        ]);
        let Some(Outcome::Split(x)) = n.outcomes.get(&2) else { panic!("{n:?}") };
        assert_eq!(x.len(), 2);
        assert_eq!(x[0].anchors[x[0].anchors.len() - 1].p, P::new(5.0, 0.0));
        assert!(!n.outcomes.contains_key(&0) && !n.outcomes.contains_key(&1), "the pieces stay as they are");
    }

    #[test]
    fn a_line_through_another_lines_anchor_splits_both_there() {
        let n = run(&[
            line("a", &[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], false),
            line("x", &[(5.0, -5.0), (5.0, 5.0)], true),
        ]);
        let (Some(Outcome::Split(a)), Some(Outcome::Split(x))) = (n.outcomes.get(&0), n.outcomes.get(&1)) else {
            panic!("{n:?}")
        };
        assert_eq!((a.len(), x.len()), (2, 2));
        assert_eq!(a[0].anchors[a[0].anchors.len() - 1].p, P::new(5.0, 0.0));
        assert_eq!(x[0].anchors[x[0].anchors.len() - 1].p, P::new(5.0, 0.0), "exactly the shared vertex");
    }

    #[test]
    fn touching_or_retracing_at_a_vertex_is_not_a_crossing() {
        // A line touching another's anchor from one side, and a line retracing another exactly.
        let touch = run(&[
            line("a", &[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], false),
            line("t", &[(3.0, 5.0), (5.0, 0.0), (7.0, 5.0)], true),
        ]);
        assert!(touch.outcomes.is_empty(), "{touch:?}");
        let retrace = run(&[
            line("a", &[(0.0, 0.0), (5.0, 2.0), (10.0, 0.0)], false),
            line("b", &[(0.0, 0.0), (5.0, 2.0), (10.0, 0.0)], true),
        ]);
        assert!(retrace.outcomes.is_empty(), "{retrace:?}");
    }

    #[test]
    fn brush_outlines_are_never_split() {
        let brush = NodeStroke {
            id: "b".into(),
            kind: EdgeKind::Brush,
            path: Path {
                anchors: [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
                    .iter()
                    .map(|&(x, y)| Anchor::corner(P::new(x, y)))
                    .collect(),
                closed: true,
            },
            changed: false,
        };
        let n = run(&[brush, line("x", &[(5.0, -5.0), (5.0, 15.0)], true)]);
        assert!(!n.outcomes.contains_key(&0));
        let Some(Outcome::Split(ps)) = n.outcomes.get(&1) else { panic!("{n:?}") };
        assert_eq!(ps.len(), 3, "the pencil splits at both crossings of the outline");
    }
}
