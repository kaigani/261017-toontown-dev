//! Extract Centre Line of spineless brush outlines (02 §8.5, decision 12; M2 C2.4, ticket C23-T1): the chordal axis
//! of a constrained Delaunay triangulation of the outline.
//!
//! 1. The outline is flattened; zero-width bridges that join holes to the outer contour (SI-053: an edge present in
//!    both directions) are dropped, leaving the outer contour and the holes as constraint edges.
//! 2. A constrained Delaunay triangulation (`spade`, exact predicates) of those edges; a triangle is inside when its
//!    centroid has non-zero winding with respect to the outline.
//! 3. The axis: in a sleeve triangle (one boundary edge) the segment between the midpoints of its two inner edges; in a
//!    junction triangle (no boundary edge) segments from its centroid to the midpoints of its three inner edges; a
//!    terminal triangle (two boundary edges) ends the axis at its inner edge's midpoint. Boundary edges are subdivided
//!    first (Steiner points about 1 du apart) so the triangles are well shaped.
//! 4. Branches from a free end to a junction that reach less than the local width there (decision 12) or the shape's
//!    average width are pruned, and junctions closer than their local width merge, repeatedly.
//! 5. The remaining chains are fitted into open (or, around a hole, closed) paths; the average width is the outline's
//!    area divided by the axis length.
//!
//! Refusals (`drawing.centreLineNoSpine` for the caller, which keeps the stroke): fewer than three distinct outline
//! points, constraint edges that cross (a self-intersecting outline), no inside triangle, or no axis left after pruning.
//! Every step charges the work account, so an outline too large refuses with `limit.exceeded` before the watchdog.

use std::collections::{BTreeMap, BTreeSet};

use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

use crate::fit::{self, Sample};
use crate::geom::P;
use crate::path::{GeomError, Path};
use crate::work::Work;

/// The work one outline's centre line may do, in units of about one vertex or triangle visit.
pub const CENTRE_WORK_BUDGET: u64 = 200_000_000;

/// The fitting smoothness of the axis chains.
const SMOOTHNESS: f64 = 2.0;

/// The inside probes along a free end's extension towards the tip.
const END_PROBES: usize = 32;

/// A centre line: paths and the average width of the source shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// The axis chains.
    pub lines: Vec<Path>,
    /// Outline area ÷ axis length (du).
    pub width: f64,
}

/// Why an outline has no centre line.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The algorithm does not apply (reason for the warning).
    NotSupported(&'static str),
    /// The work budget would be exceeded.
    Geometry(GeomError),
}

impl From<GeomError> for Refusal {
    fn from(e: GeomError) -> Self {
        Refusal::Geometry(e)
    }
}

type Key = (i64, i64);

fn key(p: P) -> Key {
    ((p.x * 64.0).round() as i64, (p.y * 64.0).round() as i64)
}

fn pt(k: Key) -> P {
    P::new(k.0 as f64 / 64.0, k.1 as f64 / 64.0)
}

/// Non-zero winding of `q` with respect to closed polygon edges.
fn winding(edges: &[(P, P)], q: P) -> i32 {
    let mut w = 0;
    for &(a, b) in edges {
        if a.y <= q.y {
            if b.y > q.y && (b.x - a.x) * (q.y - a.y) - (q.x - a.x) * (b.y - a.y) > 0.0 {
                w += 1;
            }
        } else if b.y <= q.y && (b.x - a.x) * (q.y - a.y) - (q.x - a.x) * (b.y - a.y) < 0.0 {
            w -= 1;
        }
    }
    w
}

/// The centre line of a closed brush outline.
///
/// # Errors
/// [`Refusal`].
pub fn centre_line(outline: &Path, work: &mut Work) -> Result<Axis, Refusal> {
    let samples = outline.flatten(0.25);
    work.spend_n(samples.len())?;
    let mut ring: Vec<Key> = samples.iter().map(|p| key(*p)).collect();
    ring.dedup();
    while ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let distinct: BTreeSet<Key> = ring.iter().copied().collect();
    if distinct.len() < 3 {
        return Err(Refusal::NotSupported("the outline has fewer than three distinct points"));
    }
    let n = ring.len();
    let all: Vec<(Key, Key)> = (0..n).map(|i| (ring[i], ring[(i + 1) % n])).filter(|(a, b)| a != b).collect();
    // Bridges (an edge in both directions) cancel; what remains bounds the shape.
    let mut count: BTreeMap<(Key, Key), i64> = BTreeMap::new();
    for &(a, b) in &all {
        *count.entry((a, b)).or_default() += 1;
        *count.entry((b, a)).or_default() -= 1;
    }
    let boundary: BTreeSet<(Key, Key)> = all
        .iter()
        .filter(|(a, b)| count.get(&(*a, *b)).copied().unwrap_or(0) > 0)
        .map(|(a, b)| if a < b { (*a, *b) } else { (*b, *a) })
        .collect();
    let winding_edges: Vec<(P, P)> = all.iter().map(|(a, b)| (pt(*a), pt(*b))).collect();
    // Boundary edges are subdivided (Steiner points about 1 du apart, coarser for very long outlines) so the
    // triangles are well shaped and the axis follows the middle of the shape, not its diagonals.
    let perimeter: f64 = boundary.iter().map(|(a, b)| pt(*a).dist(pt(*b))).sum();
    let step = (perimeter / 20_000.0).max(1.0);
    let mut pieces: Vec<(Key, Key)> = Vec::new();
    for (a, b) in &boundary {
        let (pa, pb) = (pt(*a), pt(*b));
        let k = (pa.dist(pb) / step).ceil().clamp(1.0, 100_000.0) as usize;
        work.spend_n(k)?;
        let mut prev = *a;
        for i in 1..=k {
            let q = if i == k { *b } else { key(pa.lerp(pb, i as f64 / k as f64)) };
            if q != prev {
                pieces.push((prev, q));
            }
            prev = q;
        }
    }
    let boundary_keys: BTreeSet<(Key, Key)> =
        pieces.iter().map(|(a, b)| if a < b { (*a, *b) } else { (*b, *a) }).collect();
    // The triangulation.
    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = ConstrainedDelaunayTriangulation::new();
    let mut handle = BTreeMap::new();
    for k in pieces.iter().flat_map(|(a, b)| [*a, *b]) {
        if handle.contains_key(&k) {
            continue;
        }
        work.spend(16)?;
        let p = pt(k);
        let h = cdt
            .insert(Point2::new(p.x, p.y))
            .map_err(|_| Refusal::NotSupported("an outline point cannot be triangulated"))?;
        handle.insert(k, h);
    }
    for (a, b) in &pieces {
        work.spend(16)?;
        let (Some(&ha), Some(&hb)) = (handle.get(a), handle.get(b)) else { continue };
        if ha == hb {
            continue;
        }
        if !cdt.can_add_constraint(ha, hb) {
            return Err(Refusal::NotSupported("the outline crosses itself"));
        }
        cdt.add_constraint(ha, hb);
    }
    // Inside triangles, their boundary edges and inner neighbours.
    let pos = |h: spade::handles::FixedVertexHandle| {
        let p = cdt.vertex(h).position();
        P::new(p.x, p.y)
    };
    let mut inside: BTreeMap<usize, [spade::handles::FixedVertexHandle; 3]> = BTreeMap::new();
    for f in cdt.inner_faces() {
        // The winding test visits every outline edge.
        work.spend_n(winding_edges.len().saturating_add(8))?;
        let vs = f.vertices().map(|v| v.fix());
        let c = (pos(vs[0]) + pos(vs[1]) + pos(vs[2])) * (1.0 / 3.0);
        if winding(&winding_edges, c) != 0 {
            inside.insert(f.fix().index(), vs);
        }
    }
    if inside.is_empty() {
        return Err(Refusal::NotSupported("the outline encloses no area"));
    }
    let edge_key = |a: spade::handles::FixedVertexHandle, b: spade::handles::FixedVertexHandle| {
        let (x, y) = (a.index(), b.index());
        if x < y { (x, y) } else { (y, x) }
    };
    // How many inside triangles use each edge: 2 = an inner edge (an axis node at its midpoint).
    let mut uses: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for vs in inside.values() {
        for i in 0..3 {
            *uses.entry(edge_key(vs[i], vs[(i + 1) % 3])).or_default() += 1;
        }
    }
    let is_inner = |a, b| {
        uses.get(&edge_key(a, b)).copied() == Some(2)
            && !{
                let (ka, kb) = (key(pos(a)), key(pos(b)));
                boundary_keys.contains(&if ka < kb { (ka, kb) } else { (kb, ka) })
            }
    };
    // The axis graph: nodes by position key, segments.
    let mut node_of: BTreeMap<Key, usize> = BTreeMap::new();
    let mut nodes: Vec<P> = Vec::new();
    let mut adj: Vec<BTreeSet<usize>> = Vec::new();
    let mut node = |p: P, nodes: &mut Vec<P>, adj: &mut Vec<BTreeSet<usize>>| -> usize {
        // Positions on a finer grid so distinct midpoints never merge.
        let k = ((p.x * 4096.0).round() as i64, (p.y * 4096.0).round() as i64);
        *node_of.entry(k).or_insert_with(|| {
            nodes.push(p);
            adj.push(BTreeSet::new());
            nodes.len() - 1
        })
    };
    let mut junction_width: BTreeMap<usize, f64> = BTreeMap::new();
    let mut area = 0.0;
    for vs in inside.values() {
        work.spend(8)?;
        let (a, b, c) = (pos(vs[0]), pos(vs[1]), pos(vs[2]));
        area += ((b - a).x * (c - a).y - (b - a).y * (c - a).x).abs() / 2.0;
        let inner: Vec<(usize, usize)> =
            (0..3).filter(|&i| is_inner(vs[i], vs[(i + 1) % 3])).map(|i| (i, (i + 1) % 3)).collect();
        let mid = |(i, j): (usize, usize)| (pos(vs[i]) + pos(vs[j])) * 0.5;
        let link = |x: usize, y: usize, adj: &mut Vec<BTreeSet<usize>>| {
            if x != y {
                adj[x].insert(y);
                adj[y].insert(x);
            }
        };
        match inner.len() {
            1 => {
                // A terminal triangle ends the axis at its inner edge (with the subdivided boundary, within about a
                // unit of the tip; running on to its apex would make a spur into every corner).
                node(mid(inner[0]), &mut nodes, &mut adj);
            }
            2 => {
                let m0 = node(mid(inner[0]), &mut nodes, &mut adj);
                let m1 = node(mid(inner[1]), &mut nodes, &mut adj);
                link(m0, m1, &mut adj);
            }
            3 => {
                let centroid = (a + b + c) * (1.0 / 3.0);
                let j = node(centroid, &mut nodes, &mut adj);
                // The local width (decision 12): the diameter of the junction triangle's circumcircle, the largest disc
                // there that holds no outline point (Delaunay; the outline is sampled about 1 du apart).
                let cross = ((b - a).x * (c - a).y - (b - a).y * (c - a).x).abs();
                let radius = if cross > 0.0 { a.dist(b) * b.dist(c) * c.dist(a) / (2.0 * cross) } else { 0.0 };
                junction_width.insert(j, 2.0 * radius);
                for e in &inner {
                    let m = node(mid(*e), &mut nodes, &mut adj);
                    link(j, m, &mut adj);
                }
            }
            _ => {}
        }
    }
    // Pruning: a branch from a free end to a node of degree ≥ 3 shorter than the local width there goes (decision 12),
    // or shorter than the shape's average width (outline area ÷ current axis length, recomputed each pass): near a
    // corner a junction lies close to the boundary, so its own width would keep corner spurs.
    for _ in 0..16 {
        let total: f64 = (0..nodes.len())
            .flat_map(|i| adj[i].iter().filter(move |&&j| j > i).map(move |&j| (i, j)))
            .map(|(i, j)| nodes[i].dist(nodes[j]))
            .sum();
        let average = if total > 0.0 { area / total } else { 0.0 };
        let mut removed = false;
        for start in 0..nodes.len() {
            work.spend(4)?;
            if adj[start].len() != 1 {
                continue;
            }
            let mut path = vec![start];
            let (mut prev, mut cur, mut len) = (start, start, 0.0);
            loop {
                let next: Vec<usize> = adj[cur].iter().copied().filter(|x| *x != prev).collect();
                if adj[cur].len() >= 3 || next.is_empty() || path.len() > nodes.len() {
                    break;
                }
                let nx = next[0];
                len += nodes[cur].dist(nodes[nx]);
                path.push(nx);
                (prev, cur) = (cur, nx);
            }
            work.spend_n(path.len())?;
            if adj[cur].len() < 3 {
                continue;
            }
            // The branch's reach is its end's distance from the junction: the chain through triangle midpoints
            // zigzags in a corner fan, so its length overstates a corner spur.
            let reach = nodes[start].dist(nodes[cur]).min(len);
            let width = junction_width.get(&cur).copied().unwrap_or(0.0).max(average);
            if reach < width {
                for w in path.windows(2) {
                    adj[w[0]].remove(&w[1]);
                    adj[w[1]].remove(&w[0]);
                }
                removed = true;
            }
        }
        // Junction clusters: a chain between two nodes of degree ≥ 3 shorter than the local width at either end (their
        // inscribed discs overlap by more than half) is contracted (the nodes merge at its midpoint), so branches meet
        // at one point.
        for i in 0..nodes.len() {
            work.spend(4)?;
            if adj[i].len() < 3 {
                continue;
            }
            let wi = junction_width.get(&i).copied().unwrap_or(0.0);
            // The chains from `i` through degree-2 nodes (a junction triangle links its centroid to its edge midpoints,
            // so neighbouring junctions are two segments apart) to another junction.
            let mut near: Option<(usize, Vec<usize>)> = None;
            for &f in &adj[i] {
                let mut chain = vec![i, f];
                let (mut prev, mut cur, mut len) = (i, f, nodes[i].dist(nodes[f]));
                while adj[cur].len() == 2 && chain.len() <= nodes.len() {
                    let Some(nx) = adj[cur].iter().copied().find(|x| *x != prev) else { break };
                    len += nodes[cur].dist(nodes[nx]);
                    chain.push(nx);
                    (prev, cur) = (cur, nx);
                }
                work.spend_n(chain.len())?;
                let wj = junction_width.get(&cur).copied().unwrap_or(0.0);
                if cur != i && adj[cur].len() >= 3 && len < wi.max(wj).max(average / 2.0) {
                    near = Some((cur, chain));
                    break;
                }
            }
            if let Some((j, chain)) = near {
                for w in chain.windows(2) {
                    adj[w[0]].remove(&w[1]);
                    adj[w[1]].remove(&w[0]);
                }
                let wj = junction_width.get(&j).copied().unwrap_or(0.0);
                junction_width.insert(i, wi.max(wj));
                let mid = (nodes[i] + nodes[j]) * 0.5;
                nodes[i] = mid;
                let others: Vec<usize> = adj[j].iter().copied().filter(|&k| k != i).collect();
                adj[j].clear();
                for k in others {
                    adj[k].remove(&j);
                    if k != i {
                        adj[k].insert(i);
                        adj[i].insert(k);
                    }
                }
                junction_width.remove(&j);
                removed = true;
            }
        }
        if !removed {
            break;
        }
    }
    // Chains between nodes of degree ≠ 2 (and cycles of degree-2 nodes).
    let mut used: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut chains: Vec<(Vec<usize>, bool)> = Vec::new();
    let walk = |s: usize,
                first: usize,
                used: &mut BTreeSet<(usize, usize)>,
                adj: &Vec<BTreeSet<usize>>|
     -> (Vec<usize>, bool) {
        let mut pts = vec![s];
        let (mut prev, mut cur) = (s, first);
        used.insert((s.min(first), s.max(first)));
        loop {
            pts.push(cur);
            if adj[cur].len() != 2 || cur == s {
                return (pts, cur == s);
            }
            let Some(nx) = adj[cur].iter().copied().find(|x| *x != prev) else { return (pts, false) };
            if !used.insert((cur.min(nx), cur.max(nx))) {
                return (pts, nx == s);
            }
            (prev, cur) = (cur, nx);
        }
    };
    for s in 0..nodes.len() {
        if adj[s].len() == 2 || adj[s].is_empty() {
            continue;
        }
        for &f in &adj[s].clone() {
            if !used.contains(&(s.min(f), s.max(f))) {
                chains.push(walk(s, f, &mut used, &adj));
            }
        }
    }
    for s in 0..nodes.len() {
        if adj[s].len() == 2 {
            for &f in &adj[s].clone() {
                if !used.contains(&(s.min(f), s.max(f))) {
                    chains.push(walk(s, f, &mut used, &adj));
                }
            }
        }
    }
    // Free ends (degree 1) fan into a corner of the shape's end: each is trimmed back by the average width and
    // extended the same distance along the chain's direction, so it runs straight into the tip.
    let raw_length: f64 =
        chains.iter().map(|(c, _)| c.windows(2).map(|w| nodes[w[0]].dist(nodes[w[1]])).sum::<f64>()).sum();
    let w = if raw_length > 0.0 { area / raw_length } else { 0.0 };
    let mut lines = Vec::new();
    let mut length = 0.0;
    for (chain, closed) in chains {
        work.spend_n(chain.len())?;
        let free = |i: Option<&usize>| i.is_some_and(|&i| adj[i].len() == 1);
        let (free_first, free_last) = (free(chain.first()), free(chain.last()));
        let mut pts: Vec<P> = chain.iter().map(|&i| nodes[i]).collect();
        if closed && pts.len() > 1 {
            pts.pop();
        }
        if pts.len() < 2 {
            continue;
        }
        if !closed {
            // Each of an end's probes visits every outline edge.
            let inside = |q: P| winding(&winding_edges, q) != 0;
            if free_last {
                work.spend_n(winding_edges.len().saturating_mul(END_PROBES))?;
                straighten_end(&mut pts, w, &inside);
            }
            if free_first {
                work.spend_n(winding_edges.len().saturating_mul(END_PROBES))?;
                pts.reverse();
                straighten_end(&mut pts, w, &inside);
                pts.reverse();
            }
        }
        let pts = smooth_chain(&pts, closed, w, work)?;
        length += pts.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>()
            + if closed { pts[pts.len() - 1].dist(pts[0]) } else { 0.0 };
        let samples: Vec<Sample> = pts.iter().map(|p| Sample { p: *p, pressure: 1.0 }).collect();
        let left = CENTRE_WORK_BUDGET.saturating_sub(work.used());
        let tol = fit::tolerance(SMOOTHNESS).max(w / 10.0);
        let (f, used_units) = fit::fit_with_bounded(&samples, tol, SMOOTHNESS, closed && pts.len() >= 3, left)?;
        work.spend(used_units)?;
        lines.push(f.path);
    }
    if lines.is_empty() || length <= 0.0 {
        return Err(Refusal::NotSupported("no centre line remains after pruning"));
    }
    Ok(Axis { lines, width: area / length })
}

/// A chain resampled about 1 du apart and averaged over about one width along it (prefix sums; the window shrinks
/// towards an open chain's ends, which stay fixed, so chains still meet at their junctions): the midpoints of a
/// staircase outline's triangles zigzag, which would overstate the axis length and fit into many anchors.
fn smooth_chain(pts: &[P], closed: bool, w: f64, work: &mut Work) -> Result<Vec<P>, Refusal> {
    let mut ring: Vec<P> = pts.to_vec();
    if closed {
        ring.push(pts[0]);
    }
    let total: f64 = ring.windows(2).map(|s| s[0].dist(s[1])).sum();
    if total <= 0.0 || w <= 0.0 {
        return Ok(pts.to_vec());
    }
    let step = (total / 100_000.0).max(1.0);
    let count = (total / step).ceil().max(1.0) as usize;
    work.spend_n(count + ring.len())?;
    // Resample by arc length.
    let mut out: Vec<P> = Vec::with_capacity(count + 1);
    let (mut seg, mut seg_start) = (0, 0.0);
    for k in 0..=count {
        let at = total * k as f64 / count as f64;
        while seg + 2 < ring.len() && seg_start + ring[seg].dist(ring[seg + 1]) < at {
            seg_start += ring[seg].dist(ring[seg + 1]);
            seg += 1;
        }
        let d = ring[seg].dist(ring[seg + 1]);
        let t = if d > 0.0 { ((at - seg_start) / d).clamp(0.0, 1.0) } else { 0.0 };
        out.push(ring[seg].lerp(ring[seg + 1], t));
    }
    if closed {
        out.pop();
    }
    let n = out.len();
    let half = ((w / 2.0) / step).ceil() as usize;
    if n < 3 || half == 0 {
        return Ok(out);
    }
    // Prefix sums over the (for a closed chain, tripled) sequence.
    let seq: Vec<P> = if closed { out.iter().chain(&out).chain(&out).copied().collect() } else { out.clone() };
    let mut prefix = Vec::with_capacity(seq.len() + 1);
    prefix.push(P::default());
    for p in &seq {
        let last = prefix[prefix.len() - 1];
        prefix.push(last + *p);
    }
    let mean = |a: usize, b: usize| (prefix[b + 1] - prefix[a]) * (1.0 / (b + 1 - a) as f64);
    Ok((0..n)
        .map(|i| {
            if closed {
                let h = half.min(n / 2 - 1);
                mean(n + i - h, n + i + h)
            } else {
                let h = half.min(i).min(n - 1 - i);
                mean(i - h, i + h)
            }
        })
        .collect())
}

/// Trims the last `w` (along the chain) off an open chain and extends it along the direction of the part before, as
/// far as the outline (at most `w`), so its end runs straight on to the tip.
fn straighten_end(pts: &mut Vec<P>, w: f64, inside: &dyn Fn(P) -> bool) {
    if w <= 0.0 || pts.len() < 2 {
        return;
    }
    let total: f64 = pts.windows(2).map(|s| s[0].dist(s[1])).sum();
    if total <= 2.0 * w {
        return;
    }
    let mut cut = 0.0;
    while pts.len() > 2 {
        let n = pts.len();
        let d = pts[n - 1].dist(pts[n - 2]);
        if cut + d > w {
            break;
        }
        cut += d;
        pts.pop();
    }
    let n = pts.len();
    let (a, b) = (pts[n - 2], pts[n - 1]);
    // The direction over about one width back from the new end.
    let mut back = n - 1;
    let mut along = 0.0;
    while back > 0 && along < w {
        along += pts[back].dist(pts[back - 1]);
        back -= 1;
    }
    let dir = b - pts[back];
    let len = dir.len();
    let dir = if len > 0.0 { dir * (1.0 / len) } else { b - a };
    let steps = END_PROBES;
    let mut end = b;
    for i in 1..=steps {
        let q = b + dir * (w * i as f64 / steps as f64);
        if !inside(q) {
            break;
        }
        end = q;
    }
    if end != b {
        pts.push(end);
    }
}
