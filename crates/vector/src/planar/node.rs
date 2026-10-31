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
use super::graph::EdgeKind;
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
    sa: f64,
    sb: f64,
    /// Index of the edge within its stroke's polyline.
    k: usize,
}

fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Proper crossing of two segments: the fractions along each.
fn crossing(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let r = b - a;
    let s = d - c;
    let den = cross(r, s);
    if den == 0.0 {
        return None;
    }
    let t = cross(c - a, s) / den;
    let u = cross(c - a, r) / den;
    if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 { Some((t, u)) } else { None }
}

/// Where two non-parallel segments meet at an end of either (a stroke passing through another's vertex, or two vertices
/// touching): the fractions (0 or 1 at the vertex) and the vertex itself, exactly. Fractions within 1e-12 outside the
/// segments count (rounding at a vertex); crossings strictly inside both are [`crossing`]'s.
fn at_vertex(a: P, b: P, c: P, d: P) -> Option<(f64, f64, P)> {
    const EPS: f64 = 1e-12;
    let (r, s) = (b - a, d - c);
    let den = cross(r, s);
    if den == 0.0 {
        return None;
    }
    let (t, u) = (cross(c - a, s) / den, cross(c - a, r) / den);
    let inside = |x: f64| x > 0.0 && x < 1.0;
    if (inside(t) && inside(u)) || !(-EPS..=1.0 + EPS).contains(&t) || !(-EPS..=1.0 + EPS).contains(&u) {
        return None;
    }
    let (t, u) = (t.clamp(0.0, 1.0), u.clamp(0.0, 1.0));
    // At least one fraction is at an end: that vertex is the point.
    let at = if u == 0.0 {
        c
    } else if u == 1.0 {
        d
    } else if t == 0.0 {
        a
    } else {
        b
    };
    Some((t, u, at))
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
                edges.push(EdgeRef { stroke: i, a, b, sa, sb, k });
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
                if let Some((t, u)) = crossing(e.a, e.b, f.a, f.b) {
                    records.push(Record {
                        at: e.a.lerp(e.b, t),
                        parts: [
                            (e.stroke, flat::param_between(e.sa, e.sb, t)),
                            (f.stroke, flat::param_between(f.sa, f.sb, u)),
                        ],
                    });
                } else if let Some((t, u, at)) = at_vertex(e.a, e.b, f.a, f.b) {
                    // A stroke through another's vertex crosses (or touches) it there (02 §3.12 item 1).
                    records.push(Record {
                        at,
                        parts: [
                            (e.stroke, flat::param_between(e.sa, e.sb, t)),
                            (f.stroke, flat::param_between(f.sa, f.sb, u)),
                        ],
                    });
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
            // The nearest point of every other stroke within the tolerance: an end where several strokes meet (the
            // neighbouring piece of a split stroke and a line through the junction) is a junction with each of them.
            let mut best: BTreeMap<usize, (f64, usize, f64, P)> = BTreeMap::new();
            let key = (cell(p.x), cell(p.y));
            let Some(list) = grid.get(&key) else { continue };
            work.spend_n(list.len())?;
            for &n in list {
                let e = &edges[n];
                if e.stroke == i {
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
    let junctions = resolve(strokes, &records, work)?;
    let mut outcomes = BTreeMap::new();
    for (i, s) in strokes.iter().enumerate() {
        if !splittable(s.kind) || junctions[i].is_empty() {
            continue;
        }
        // Each junction is resolved and may become a piece (a split and a new stroke of a few anchors).
        work.spend_n(junctions[i].len().saturating_mul(256))?;
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
    pieces.retain(|p| p.anchors.len() >= 2);
    Some(Outcome::Split(pieces))
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
    let (mut s, end) = (from.s, to.s);
    while s < end - 1e-12 {
        let j = (s.floor() as usize).min(usize::MAX - 1);
        let seg = segs[j % n];
        let t0 = s - j as f64;
        let t1 = ((j + 1) as f64).min(end) - j as f64;
        let part = sub_cubic(&seg, t0, t1);
        cubics.push(part);
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
    Path { anchors, closed: false }
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
