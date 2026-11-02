//! Independent oracles of the C24-R2-01 route domains (`c24_route_domains.rs`), sharing no code with the product (no
//! product crate is imported here; the exact rational oracle `crates/vector/tests/oracle` is included as a submodule):
//!
//! - [`flatten`]: a stored Bézier path as the planar map documents it is flattened (02 §3.12; ADR-0017: each cubic in
//!   uniform parameter steps, as many as Wang's bound needs for 1/128 du, one step for a cubic whose handles lie
//!   within 1/1024 du of its chord), and [`snap`]ped to the 1/64-du grid (rounded to nearest);
//! - [`SnapMap`]: the snap-rounded arrangement of such polylines (ADR-0017: every end and every rounded proper
//!   crossing is a hot pixel; every segment passing through a hot pixel's closed unit square is broken there), with
//!   faces traced with exact integer predicates, holes assigned to the smallest containing face of another component,
//!   brush occupancy by the non-zero winding of each brush's snapped pieces, and per-face canonical zone IDs through
//!   the oracle's `canonical_cycle`/`zone_id`;
//! - [`overlap2`]: exact overlap areas of two snapped faces (the oracle's rational arrangement of both boundaries);
//! - [`ttf`]: a minimal TrueType reader (`head`, `hhea`, `OS/2`, `maxp`, `cmap` format 4, `hmtx`, `loca`, `glyf`,
//!   `kern` format 0) for the fixture font;
//! - distances from points to polylines for the within-tolerance stage of curved routes.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test oracle")]

#[path = "../../../vector/tests/oracle/mod.rs"]
pub mod oracle;

pub mod ttf;

use std::collections::{BTreeMap, BTreeSet};

use oracle::{Pt, Q};

/// A grid point (1/64 du).
pub type Ip = (i64, i64);

/// Grid units per du.
pub const GRID: f64 = 64.0;

/// The flattening tolerance of the planar map (du).
pub const FLAT_TOL: f64 = 1.0 / 128.0;

/// A point (du).
pub type F2 = (f64, f64);

fn sub(a: F2, b: F2) -> F2 {
    (a.0 - b.0, a.1 - b.1)
}

fn dot(a: F2, b: F2) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn len(a: F2) -> f64 {
    dot(a, a).sqrt()
}

/// Distance from `p` to the segment `a`–`b` (du).
pub fn seg_dist(p: F2, a: F2, b: F2) -> f64 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 > 0.0 { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    len(sub(p, (a.0 + ab.0 * t, a.1 + ab.1 * t)))
}

/// A point of a cubic (Bernstein form).
pub fn eval(c: &[F2; 4], t: f64) -> F2 {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let cc = 3.0 * mt * t * t;
    let d = t * t * t;
    (a * c[0].0 + b * c[1].0 + cc * c[2].0 + d * c[3].0, a * c[0].1 + b * c[1].1 + cc * c[2].1 + d * c[3].1)
}

/// Uniform parameter steps keeping a cubic within `tol` of its chords (Wang's bound); one for a straight cubic.
pub fn pieces(c: &[F2; 4], tol: f64) -> u32 {
    if seg_dist(c[1], c[0], c[3]) <= tol / 8.0 && seg_dist(c[2], c[0], c[3]) <= tol / 8.0 {
        return 1;
    }
    let dd1 = len((c[2].0 - c[1].0 * 2.0 + c[0].0, c[2].1 - c[1].1 * 2.0 + c[0].1));
    let dd2 = len((c[3].0 - c[2].0 * 2.0 + c[1].0, c[3].1 - c[2].1 * 2.0 + c[1].1));
    let m = dd1.max(dd2) * 6.0;
    let n = if m <= 0.0 { 1.0 } else { (m / (8.0 * tol)).sqrt().ceil().clamp(1.0, 4096.0) };
    n as u32
}

/// The cubics of a flat Bézier list `p0, c1, c2, p1, …` (closed: a last segment back to `p0`).
pub fn cubics(bez: &[F2], closed: bool) -> Vec<[F2; 4]> {
    let n = bez.len();
    let mut out = Vec::new();
    let mut k = 0;
    while k + 3 < n || (closed && n >= 3 && k + 3 == n) {
        out.push([bez[k], bez[k + 1], bez[k + 2], bez[if k + 3 < n { k + 3 } else { 0 }]]);
        k += 3;
    }
    out
}

/// A flattened path (du): vertices, and whether it closes back to its first vertex.
pub fn flatten(bez: &[F2], closed: bool) -> (Vec<F2>, bool) {
    let segs = cubics(bez, closed);
    let mut pts: Vec<F2> = Vec::new();
    if segs.is_empty() {
        if let Some(a) = bez.first() {
            pts.push(*a);
        }
        return (pts, false);
    }
    let last = segs.len() - 1;
    for (j, c) in segs.iter().enumerate() {
        let n = pieces(c, FLAT_TOL);
        for i in 0..n {
            let t = f64::from(i) / f64::from(n);
            let p = if i == 0 { c[0] } else { eval(c, t) };
            if pts.last() != Some(&p) {
                pts.push(p);
            }
        }
        if j == last && !closed && pts.last() != Some(&c[3]) {
            pts.push(c[3]);
        }
    }
    if closed && pts.len() >= 2 && pts.last() == pts.first() {
        pts.pop();
    }
    let is_closed = closed && pts.len() >= 2;
    (pts, is_closed)
}

/// The grid point of a point (du): rounded to the nearest 1/64 du.
pub fn snap(p: F2) -> Ip {
    ((p.0 * GRID).round() as i64, (p.1 * GRID).round() as i64)
}

/// The snapped edges of a flattened path (zero-length ones left out), in path order.
pub fn path_edges(bez: &[F2], closed: bool) -> Vec<(Ip, Ip)> {
    let (pts, is_closed) = flatten(bez, closed);
    let g: Vec<Ip> = pts.iter().map(|p| snap(*p)).collect();
    let mut out: Vec<(Ip, Ip)> = g.windows(2).filter(|w| w[0] != w[1]).map(|w| (w[0], w[1])).collect();
    if is_closed && g.len() >= 2 && g[g.len() - 1] != g[0] {
        out.push((g[g.len() - 1], g[0]));
    }
    out
}

fn orient(a: Ip, b: Ip, c: Ip) -> i128 {
    i128::from(b.0 - a.0) * i128::from(c.1 - a.1) - i128::from(b.1 - a.1) * i128::from(c.0 - a.0)
}

/// `n / d` rounded to the nearest integer, halves away from zero (`d > 0`).
fn round_div(n: i128, d: i128) -> i128 {
    if n >= 0 { (2 * n + d) / (2 * d) } else { -((-2 * n + d) / (2 * d)) }
}

/// The rounded proper crossing of two grid segments (interior to both).
fn crossing(a: Ip, b: Ip, c: Ip, d: Ip) -> Option<Ip> {
    let (o1, o2) = (orient(a, b, c).signum(), orient(a, b, d).signum());
    let (o3, o4) = (orient(c, d, a).signum(), orient(c, d, b).signum());
    if o1 * o2 >= 0 || o3 * o4 >= 0 {
        return None;
    }
    let r = (i128::from(b.0 - a.0), i128::from(b.1 - a.1));
    let s = (i128::from(d.0 - c.0), i128::from(d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    let ca = (i128::from(c.0 - a.0), i128::from(c.1 - a.1));
    let num = ca.0 * s.1 - ca.1 * s.0;
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    Some(((i128::from(a.0) + round_div(r.0 * num, den)) as i64, (i128::from(a.1) + round_div(r.1 * num, den)) as i64))
}

/// Whether the grid segment `a`–`b` meets the closed unit square around `v` (doubled coordinates, exact signs).
pub fn through_pixel(a: Ip, b: Ip, v: Ip) -> bool {
    let (a2, b2) = ((2 * a.0, 2 * a.1), (2 * b.0, 2 * b.1));
    let (lo, hi) = ((2 * v.0 - 1, 2 * v.1 - 1), (2 * v.0 + 1, 2 * v.1 + 1));
    if a2.0.max(b2.0) < lo.0 || a2.0.min(b2.0) > hi.0 || a2.1.max(b2.1) < lo.1 || a2.1.min(b2.1) > hi.1 {
        return false;
    }
    let signs = [(lo.0, lo.1), (hi.0, lo.1), (hi.0, hi.1), (lo.0, hi.1)].map(|c| orient(a2, b2, c).signum());
    !(signs.iter().all(|s| *s > 0) || signs.iter().all(|s| *s < 0))
}

/// One stroke of a layer for the snap-rounded oracle: its ID, whether it is a brush (its inside is not a zone) and
/// its snapped edges in stroke order.
#[derive(Debug, Clone)]
pub struct Input {
    pub id: String,
    pub brush: bool,
    pub edges: Vec<(Ip, Ip)>,
}

/// A stroke piece on an edge: the input, and whether it runs from the edge's lower to its higher end point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Src {
    pub input: usize,
    pub forward: bool,
}

/// A point with an exact dyadic position: `(x / s, y / s)` grid units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dp {
    pub x: i128,
    pub y: i128,
    pub s: i128,
}

impl Dp {
    /// A published point (du) on a dyadic grid (the planar map's interior points lie on a 1/(64 × 16384) du grid).
    pub fn from_du(x: f64, y: f64) -> Dp {
        let s = 16384i128;
        let (fx, fy) = (x * GRID * s as f64, y * GRID * s as f64);
        assert!(fx.fract() == 0.0 && fy.fract() == 0.0, "an interior point off the dyadic grid: ({x}, {y})");
        Dp { x: fx as i128, y: fy as i128, s }
    }

    pub fn exact(&self) -> Pt {
        (Q::new(self.x, self.s), Q::new(self.y, self.s))
    }
}

/// Twice the signed area of a ring (grid units²).
pub fn ring_area2(r: &[Ip]) -> i128 {
    let n = r.len();
    (0..n)
        .map(|i| i128::from(r[i].0) * i128::from(r[(i + 1) % n].1) - i128::from(r[(i + 1) % n].0) * i128::from(r[i].1))
        .sum()
}

/// Winding number of a ring around a dyadic point, and whether the point lies on the ring.
pub fn winding_dp(r: &[Ip], p: Dp) -> (i32, bool) {
    let mut w = 0;
    let n = r.len();
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let (ax, ay, bx, by) =
            (i128::from(a.0) * p.s, i128::from(a.1) * p.s, i128::from(b.0) * p.s, i128::from(b.1) * p.s);
        let o = (bx - ax) * (p.y - ay) - (by - ay) * (p.x - ax);
        if o == 0 && p.x >= ax.min(bx) && p.x <= ax.max(bx) && p.y >= ay.min(by) && p.y <= ay.max(by) {
            return (0, true);
        }
        if ay <= p.y {
            if by > p.y && o > 0 {
                w += 1;
            }
        } else if by <= p.y && o < 0 {
            w -= 1;
        }
    }
    (w, false)
}

/// A strictly interior point of a face (outer ring, then hole rings): on a horizontal half-grid line between two
/// vertex levels, the middle of the first inside interval rounded to 1/1024 grid units and checked exactly.
fn interior(rings: &[Vec<Ip>]) -> Option<Dp> {
    let mut ys: Vec<i64> = rings.iter().flatten().map(|p| p.1).collect();
    ys.sort_unstable();
    ys.dedup();
    for s in [4i128, 1024, 1 << 20] {
        for w in ys.windows(2) {
            // Several lines between the levels (the middle first).
            for frac in [1i128, 3, 7, 15] {
                let y2 = i128::from(w[0]) * 2 * (frac + 1) / 2 + (i128::from(w[1]) - i128::from(w[0])) * frac;
                let yd = 2 * (frac + 1) / 2;
                // y = y2 / (2 (frac + 1) / 2) … expressed with denominator yd' = frac + 1.
                let _ = (y2, yd);
                let den = frac + 1;
                let ynum = i128::from(w[0]) * den + (i128::from(w[1]) - i128::from(w[0])) * (den / 2).max(1);
                // ynum / den lies strictly between the levels.
                let mut xs: Vec<f64> = Vec::new();
                for r in rings {
                    let n = r.len();
                    for i in 0..n {
                        let (a, b) = (r[i], r[(i + 1) % n]);
                        let (ay, by) = (i128::from(a.1) * den, i128::from(b.1) * den);
                        if (ay < ynum && by > ynum) || (by < ynum && ay > ynum) {
                            let t = (ynum - ay) as f64 / (by - ay) as f64;
                            xs.push(a.0 as f64 + (b.0 - a.0) as f64 * t);
                        }
                    }
                }
                xs.sort_by(f64::total_cmp);
                for pair in xs.chunks(2) {
                    if pair.len() < 2 || pair[1] - pair[0] <= 1e-9 {
                        continue;
                    }
                    let xm = (pair[0] + pair[1]) / 2.0;
                    let sc = s * den;
                    let p = Dp { x: (xm * sc as f64).round() as i128, y: ynum * s, s: sc };
                    if strictly_inside(rings, p) {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn strictly_inside(rings: &[Vec<Ip>], p: Dp) -> bool {
    let mut parity = false;
    for r in rings {
        let (w, on) = winding_dp(r, p);
        if on {
            return false;
        }
        if w % 2 != 0 {
            parity = !parity;
        }
    }
    parity
}

/// A bounded face of the snap-rounded map.
#[derive(Debug, Clone)]
pub struct SFace {
    /// Outer ring (counter-clockwise), then hole rings (face on the left).
    pub rings: Vec<Vec<Ip>>,
    /// Twice the area (grid units²).
    pub area2: i128,
    /// A strictly interior point.
    pub probe: Dp,
    /// Covered by a brush stroke's fill (not a zone).
    pub brush: bool,
    /// The canonical zone ID.
    pub id: String,
    /// The strokes on its boundary (sorted IDs).
    pub boundary: Vec<String>,
    /// Bounding box `[x0, y0, x1, y1]` of the outer ring.
    pub bbox: [i64; 4],
}

/// The snap-rounded arrangement of a layer's strokes.
#[derive(Debug, Clone)]
pub struct SnapMap {
    pub inputs: Vec<Input>,
    /// Edges (`u < v`) with the stroke pieces on them.
    pub edges: BTreeMap<(Ip, Ip), Vec<Src>>,
    pub faces: Vec<SFace>,
}

fn bbox(a: Ip, b: Ip) -> [i64; 4] {
    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
}

fn half_plane(d: Ip) -> u8 {
    u8::from(!(d.1 > 0 || (d.1 == 0 && d.0 > 0)))
}

impl SnapMap {
    /// Builds the snap-rounded map of `inputs`.
    pub fn build(inputs: Vec<Input>) -> SnapMap {
        // 1. Segments; hot pixels at their ends and their rounded proper crossings.
        let mut segs: Vec<(Ip, Ip, usize)> = Vec::new();
        for (k, inp) in inputs.iter().enumerate() {
            for &(a, b) in &inp.edges {
                if a != b {
                    segs.push((a, b, k));
                }
            }
        }
        let mut hot: BTreeSet<Ip> = BTreeSet::new();
        for s in &segs {
            hot.insert(s.0);
            hot.insert(s.1);
        }
        let mut order: Vec<usize> = (0..segs.len()).collect();
        order.sort_by_key(|&i| bbox(segs[i].0, segs[i].1)[0]);
        for (n, &i) in order.iter().enumerate() {
            let bi = bbox(segs[i].0, segs[i].1);
            for &j in &order[n + 1..] {
                let bj = bbox(segs[j].0, segs[j].1);
                if bj[0] > bi[2] {
                    break;
                }
                if bi[3] < bj[1] || bj[3] < bi[1] {
                    continue;
                }
                if let Some(x) = crossing(segs[i].0, segs[i].1, segs[j].0, segs[j].1) {
                    hot.insert(x);
                }
            }
        }
        // 2. Each segment broken at every hot pixel it passes through.
        let mut edges: BTreeMap<(Ip, Ip), Vec<Src>> = BTreeMap::new();
        for &(a, b, k) in &segs {
            let bb = bbox(a, b);
            let d = (i128::from(b.0 - a.0), i128::from(b.1 - a.1));
            let mut on: Vec<(i128, Ip)> = hot
                .range((bb[0] - 1, i64::MIN)..=(bb[2] + 1, i64::MAX))
                .filter(|h| h.1 >= bb[1] - 1 && h.1 <= bb[3] + 1 && through_pixel(a, b, **h))
                .map(|h| (i128::from(h.0 - a.0) * d.0 + i128::from(h.1 - a.1) * d.1, *h))
                .collect();
            on.sort_unstable();
            on.dedup_by(|x, y| x.1 == y.1);
            for w in on.windows(2) {
                let (p, q) = (w[0].1, w[1].1);
                if p != q {
                    let (key, forward) = if p < q { ((p, q), true) } else { ((q, p), false) };
                    edges.entry(key).or_default().push(Src { input: k, forward });
                }
            }
        }
        // 3. Half-edges around each vertex, counter-clockwise; cycles with the face on the left.
        let mut around: BTreeMap<Ip, Vec<Ip>> = BTreeMap::new();
        for &(u, v) in edges.keys() {
            around.entry(u).or_default().push(v);
            around.entry(v).or_default().push(u);
        }
        for (o, list) in around.iter_mut() {
            let o = *o;
            list.sort_by(|x, y| {
                let (dx, dy) = ((x.0 - o.0, x.1 - o.1), (y.0 - o.0, y.1 - o.1));
                half_plane(dx).cmp(&half_plane(dy)).then_with(|| {
                    let c = i128::from(dx.0) * i128::from(dy.1) - i128::from(dx.1) * i128::from(dy.0);
                    0.cmp(&c)
                })
            });
        }
        let mut used: BTreeSet<(Ip, Ip)> = BTreeSet::new();
        let mut cycles: Vec<Vec<Ip>> = Vec::new();
        for (u, list) in &around {
            for v in list {
                if used.contains(&(*u, *v)) {
                    continue;
                }
                let mut cyc = Vec::new();
                let (mut a, mut b) = (*u, *v);
                while used.insert((a, b)) {
                    cyc.push(a);
                    let l = &around[&b];
                    let k = l.iter().position(|x| *x == a).unwrap();
                    let w = l[(k + l.len() - 1) % l.len()];
                    (a, b) = (b, w);
                }
                cycles.push(cyc);
            }
        }
        // 4. Components; faces are the counter-clockwise cycles, the others holes of the smallest containing face of
        // another component.
        let verts: Vec<Ip> = around.keys().copied().collect();
        let index: BTreeMap<Ip, usize> = verts.iter().enumerate().map(|(i, v)| (*v, i)).collect();
        let mut parent: Vec<usize> = (0..verts.len()).collect();
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for &(u, v) in edges.keys() {
            let (a, b) = (find(&mut parent, index[&u]), find(&mut parent, index[&v]));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
        let comp: Vec<usize> = cycles.iter().map(|c| find(&mut parent, index[&c[0]])).collect();
        let area: Vec<i128> = cycles.iter().map(|c| ring_area2(c)).collect();
        let ring_box = |c: &[Ip]| {
            c.iter().fold([i64::MAX, i64::MAX, i64::MIN, i64::MIN], |b, p| {
                [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]
            })
        };
        let face_cycles: Vec<usize> = (0..cycles.len()).filter(|&i| area[i] > 0).collect();
        let mut holes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in (0..cycles.len()).filter(|&i| area[i] <= 0) {
            let probe = cycles[i][0];
            let p = Dp { x: i128::from(probe.0), y: i128::from(probe.1), s: 1 };
            let mut best: Option<usize> = None;
            for &c in &face_cycles {
                if comp[c] == comp[i] {
                    continue;
                }
                let b = ring_box(&cycles[c]);
                if probe.0 < b[0] || probe.0 > b[2] || probe.1 < b[1] || probe.1 > b[3] {
                    continue;
                }
                if winding_dp(&cycles[c], p).0 != 0
                    && best.is_none_or(|x| area[c] < area[x] || (area[c] == area[x] && c < x))
                {
                    best = Some(c);
                }
            }
            if let Some(c) = best {
                holes.entry(c).or_default().push(i);
            }
        }
        let mut map = SnapMap { inputs, edges, faces: Vec::new() };
        let mut brush_edges: Vec<(usize, Vec<(Ip, Ip)>)> = Vec::new();
        for (k, inp) in map.inputs.iter().enumerate().filter(|(_, i)| i.brush) {
            let mut list = Vec::new();
            for (&(u, v), srcs) in &map.edges {
                for s in srcs.iter().filter(|s| s.input == k) {
                    list.push(if s.forward { (u, v) } else { (v, u) });
                }
            }
            let _ = inp;
            brush_edges.push((k, list));
        }
        for &c in &face_cycles {
            let mut rings = vec![cycles[c].clone()];
            rings.extend(holes.get(&c).into_iter().flatten().map(|&h| cycles[h].clone()));
            let area2 = rings.iter().map(|r| ring_area2(r)).sum();
            let probe = interior(&rings).expect("an interior point");
            let brush = brush_edges.iter().any(|(_, list)| directed_winding(list, probe) != 0);
            let mut boundary = BTreeSet::new();
            for r in &rings {
                for k in 0..r.len() {
                    let (p, q) = (r[k], r[(k + 1) % r.len()]);
                    for s in map.srcs(p, q) {
                        boundary.insert(map.inputs[s.input].id.clone());
                    }
                }
            }
            let face = oracle::Face {
                outer: rings[0].iter().map(|p| oracle::pt(p.0, p.1)).collect(),
                holes: rings[1..].iter().map(|r| r.iter().map(|p| oracle::pt(p.0, p.1)).collect()).collect(),
                area2: Q::int(0),
                probe: oracle::pt(0, 0),
            };
            let id = oracle::zone_id(&face, &|p: &Pt, q: &Pt| map.label(oracle::snap_pt(p), oracle::snap_pt(q)));
            let bbox = ring_box(&rings[0]);
            map.faces.push(SFace { rings, area2, probe, brush, id, boundary: boundary.into_iter().collect(), bbox });
        }
        map
    }

    /// The stroke pieces on the edge `p`–`q`.
    pub fn srcs(&self, p: Ip, q: Ip) -> Vec<Src> {
        let key = if p < q { (p, q) } else { (q, p) };
        self.edges.get(&key).cloned().unwrap_or_default()
    }

    /// The canonical label of the edge traversed `p → q`: the sorted `strokeId:side` list (side `L` when the stroke
    /// runs the same way).
    pub fn label(&self, p: Ip, q: Ip) -> String {
        let along = p < q;
        let mut t: Vec<String> = self
            .srcs(p, q)
            .iter()
            .map(|s| format!("{}:{}", self.inputs[s.input].id, if along == s.forward { 'L' } else { 'R' }))
            .collect();
        t.sort();
        t.dedup();
        t.join(",")
    }

    /// The face strictly containing a point.
    pub fn locate(&self, p: Dp) -> Option<usize> {
        self.faces.iter().position(|f| {
            let b = f.bbox;
            if p.x < i128::from(b[0]) * p.s
                || p.x > i128::from(b[2]) * p.s
                || p.y < i128::from(b[1]) * p.s
                || p.y > i128::from(b[3]) * p.s
            {
                return false;
            }
            let (w, on) = winding_dp(&f.rings[0], p);
            if on || w == 0 {
                return false;
            }
            f.rings[1..].iter().all(|h| {
                let (w, on) = winding_dp(h, p);
                !on && w == 0
            })
        })
    }

    /// The faces that are zones (not covered by a brush).
    pub fn zone_faces(&self) -> Vec<usize> {
        (0..self.faces.len()).filter(|&f| !self.faces[f].brush).collect()
    }
}

/// Winding number of directed edges around a dyadic point (never on them).
pub fn directed_winding(edges: &[(Ip, Ip)], p: Dp) -> i32 {
    let mut w = 0;
    for &(a, b) in edges {
        let (ax, ay, bx, by) =
            (i128::from(a.0) * p.s, i128::from(a.1) * p.s, i128::from(b.0) * p.s, i128::from(b.1) * p.s);
        let o = (bx - ax) * (p.y - ay) - (by - ay) * (p.x - ax);
        if ay <= p.y {
            if by > p.y && o > 0 {
                w += 1;
            }
        } else if by <= p.y && o < 0 {
            w -= 1;
        }
    }
    w
}

/// Twice the exact area of the overlap of two faces (each its outer ring and hole rings): the faces of the rational
/// arrangement of both boundaries, each tested in both.
pub fn overlap2(a: &[Vec<Ip>], b: &[Vec<Ip>]) -> Q {
    let segs = |rings: &[Vec<Ip>]| -> Vec<(Pt, Pt)> {
        rings
            .iter()
            .flat_map(|r| {
                (0..r.len()).map(move |k| {
                    (oracle::pt(r[k].0, r[k].1), oracle::pt(r[(k + 1) % r.len()].0, r[(k + 1) % r.len()].1))
                })
            })
            .collect()
    };
    let mut all = segs(a);
    all.extend(segs(b));
    let arr = oracle::arrangement(&all);
    let inside = |rings: &[Vec<Ip>], p: &Pt| {
        let mut parity = false;
        for r in rings {
            let poly: Vec<Pt> = r.iter().map(|q| oracle::pt(q.0, q.1)).collect();
            if winding_q(&poly, p) % 2 != 0 {
                parity = !parity;
            }
        }
        parity
    };
    let mut total = Q::int(0);
    for f in &arr.faces {
        if inside(a, &f.probe) && inside(b, &f.probe) {
            total = total.plus(&f.area2);
        }
    }
    total
}

/// Winding number of a rational polygon around a point (never on it).
pub fn winding_q(poly: &[Pt], p: &Pt) -> i32 {
    let mut w = 0;
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let o = b.0.minus(&a.0).times(&p.1.minus(&a.1)).minus(&b.1.minus(&a.1).times(&p.0.minus(&a.0))).signum();
        if a.1.cmp_to(&p.1).is_le() {
            if b.1.cmp_to(&p.1).is_gt() && o > 0 {
                w += 1;
            }
        } else if b.1.cmp_to(&p.1).is_le() && o < 0 {
            w -= 1;
        }
    }
    w
}

/// Twice the length (grid units) of a face's boundary inside the box of `old`: the documented snapping bound of the
/// carry-over (ADR-0017 history 5), computed independently.
pub fn snapping_bound(face: &[Vec<Ip>], old: &[Vec<Ip>]) -> f64 {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &old[0] {
        let (x, y) = (p.0 as f64, p.1 as f64);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let mut total = 0.0;
    for c in face {
        for k in 0..c.len() {
            let a = (c[k].0 as f64, c[k].1 as f64);
            let b = (c[(k + 1) % c.len()].0 as f64, c[(k + 1) % c.len()].1 as f64);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let (mut t0, mut t1) = (0.0f64, 1.0f64);
            let mut keep = true;
            for (p, qv) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
                if p == 0.0 {
                    if qv < 0.0 {
                        keep = false;
                    }
                } else {
                    let r = qv / p;
                    if p < 0.0 {
                        t0 = t0.max(r);
                    } else {
                        t1 = t1.min(r);
                    }
                }
            }
            if keep && t1 > t0 {
                total += (t1 - t0) * dx.hypot(dy);
            }
        }
    }
    2.0 * total
}

/// Distance from a point to a polyline (du).
pub fn polyline_dist(p: F2, line: &[F2], closed: bool) -> f64 {
    if line.len() == 1 {
        return len(sub(p, line[0]));
    }
    let mut best = f64::INFINITY;
    for k in 0..line.len() - 1 {
        best = best.min(seg_dist(p, line[k], line[k + 1]));
    }
    if closed {
        best = best.min(seg_dist(p, line[line.len() - 1], line[0]));
    }
    best
}

/// Points along a polyline at most `step` du apart (its vertices included).
pub fn densify(line: &[F2], closed: bool, step: f64) -> Vec<F2> {
    let mut out = Vec::new();
    let n = line.len();
    let m = if closed { n } else { n.saturating_sub(1) };
    for k in 0..m {
        let (a, b) = (line[k], line[(k + 1) % n]);
        let parts = (len(sub(b, a)) / step).ceil().max(1.0) as usize;
        for i in 0..parts {
            let t = i as f64 / parts as f64;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    if !closed && let Some(l) = line.last() {
        out.push(*l);
    }
    out
}
