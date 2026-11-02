// Round 2 independent seed variant: all 11 properties unchanged; seeds XOR 0x5D.
//! M2 C2.4 readiness item 2 (C24-R1-01 ruling): every topology-changing command route against the independent exact
//! oracle (`crates/vector/tests/oracle`, which imports no product geometry). For each command family a seeded
//! property builds a lattice arrangement through the commands, paints every zone a colour, applies the command, and
//! compares the layer's read-back with expectations the oracle computes from the state before the command and the
//! command's semantics alone:
//!
//! - **partition**: the product's zones are exactly the oracle's bounded faces (each zone's interior point lies in one
//!   face, one zone per face) and each zone's area equals the oracle's face area with its vertices snapped to the
//!   1/64-du grid (exact);
//! - **boundary membership**: a zone's `boundaryStrokeIds` are the strokes on its face's cycles (every oracle edge must
//!   carry at least one stroke);
//! - **canonical zone IDs**: recomputed from the oracle's cycles (snapped vertices, per-edge `strokeId:side` labels,
//!   straight-through vertices left out, smallest rotation, holes sorted; SHA-256, Crockford base 32);
//! - **side fills**: a zone whose ID survives keeps its fill, any other takes the fill of the old face it overlaps most
//!   (exact overlay areas; overlaps within the documented snapping bound ignored; ties to the lower old zone ID; none:
//!   the old face holding its interior point), moved faces keep their own fill, erased faces are unpainted.
//!
//! The geometry before a command is the layer's persisted pieces (junctions already on the grid); the expected
//! geometry after it is derived from that and the command alone (added, replaced, removed, clipped, mapped segments).
//! Families: `draw.stroke.add`, `draw.stroke.set`, `draw.object.delete`, `draw.cut.apply`, `draw.erase.apply`,
//! `draw.flatten.apply`, `draw.optimize.apply`, `draw.smooth.apply`, `draw.linesToBrush.apply`,
//! `draw.centreLine.apply`, `draw.breakApart.apply`, `draw.transform.apply`, `draw.envelope.apply` and
//! `draw.point.move` (snapping included); `draw.vectorize.run` is in `c24_topology.rs`. Arrangements whose snapped
//! combinatorics would differ from the exact ones (a vertex within 6 grid units of a non-incident edge, or two vertices
//! snapping together) are skipped before the command runs. Cases: `TT_ROUTE_CASES` (default 256 in `check`; 4096 run
//! locally). `c23_r3_topology.rs` stays the consistency check it is (it calls the product's own noding and map).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "test")]

mod common;
#[path = "../../vector/tests/oracle/mod.rs"]
mod oracle;

use std::collections::BTreeSet;

use common::*;
use oracle::{Arrangement, Pt, Q};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:#1/element:\"Bob\"";
/// Lattice spacing (du) and the grid (units per du).
const STEP: i64 = 20;
const GRID: i64 = 64;
/// Snap-safety margin (grid units): above the junction tolerance (1/32 du = 2 units) plus rounding.
const MARGIN: i64 = 6;

// Reviewer instrumentation: count actual command dispatches so bypassed snap-unsafe cases are visible.
thread_local! {
    static DISPATCHES: std::cell::RefCell<std::collections::BTreeMap<String, usize>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}
fn record_dispatch(id: &str) {
    DISPATCHES.with(|c| *c.borrow_mut().entry(id.to_owned()).or_default() += 1);
}
fn ok(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    record_dispatch(id);
    common::ok(h, id, params)
}
fn run(h: &ExecutorHandle, id: &str, params: Value) -> tt_executor::Envelope {
    record_dispatch(id);
    common::run(h, id, params)
}
struct CoverageGuard(String);
impl Drop for CoverageGuard {
    fn drop(&mut self) {
        DISPATCHES.with(|c| println!("REVIEW_ROUTE_DISPATCHES {} {:?}", self.0, c.borrow()));
    }
}

fn cases() -> u32 {
    std::env::var("TT_ROUTE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(256)
}

fn runner(seed: &[u8; 32]) -> TestRunner {
    TestRunner::new_with_rng(
        Config { cases: cases(), failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed.map(|b| b ^ 0x5D)),
    )
}

type Spec = (bool, Vec<(i64, i64)>);

/// Grid-unit segments.
type Segs = Vec<((i64, i64), (i64, i64))>;

fn arrangement_strategy() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec((any::<bool>(), prop::collection::vec((0i64..5, 0i64..5), 2..5)), 1..5)
}

fn lattice(p: (i64, i64)) -> (i64, i64) {
    (p.0 * STEP, p.1 * STEP)
}

fn closed(s: &Spec) -> bool {
    s.0 && s.1.len() >= 3
}

/// A straight polyline in the content shape (`bezier`: anchors with handles on them), du.
fn bezier(points: &[(i64, i64)], is_closed: bool) -> Vec<Value> {
    let mut bez = Vec::new();
    for (k, &(x, y)) in points.iter().enumerate() {
        if k > 0 {
            let (px, py) = points[k - 1];
            bez.push(json!({ "x": px, "y": py }));
            bez.push(json!({ "x": x, "y": y }));
        }
        bez.push(json!({ "x": x, "y": y }));
    }
    if is_closed {
        let (lx, ly) = points[points.len() - 1];
        let (fx, fy) = points[0];
        bez.push(json!({ "x": lx, "y": ly }));
        bez.push(json!({ "x": fx, "y": fy }));
    }
    bez
}

fn layer_json(list: &[Spec], id: &str) -> Value {
    let strokes: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let p: Vec<(i64, i64)> = s.1.iter().map(|&q| lattice(q)).collect();
            json!({ "id": format!("{id}_s{i}"), "kind": "pencil", "closed": closed(s), "bezier": bezier(&p, closed(s)),
                    "colourId": null, "lineStyle": { "thickness": 1, "lineType": "solid" } })
        })
        .collect();
    json!({ "type": "layer", "id": id, "layerKind": "pencil", "strokes": strokes })
}

fn fresh() -> ExecutorHandle {
    let h = fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
    );
    h
}

fn colours(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

/// A persisted stroke as straight segments on the grid.
#[derive(Debug, Clone)]
struct Stroke {
    id: String,
    /// Grid-unit segments in stroke order (`from`, `to`; zero-length ones kept: a collapsed contour's anchors are snap
    /// candidates, C24-R2-06; `segments_of` leaves them out of every arrangement); a brush stroke's straight outline
    /// contours.
    segs: Segs,
    /// A brush stroke's curved outline contours, flattened within 1/128 du (grid units): outside the exact oracle, so
    /// `check` requires them to occupy no expected face (C24-R2-01).
    curves: Vec<Vec<Pt>>,
    /// The persisted anchors (repeated ones included: anchor indices are the product's) and whether it is closed.
    anchors: Vec<(i64, i64)>,
    closed: bool,
}

#[derive(Debug, Clone)]
struct Zone {
    id: String,
    area: f64,
    boundary: Vec<String>,
    colour: Option<String>,
    interior: Pt,
}

/// An anchor snapped to the grid as the planar map snaps it (rounded to the nearest 1/64 du); anchors are grid points
/// except where a command placed one off the grid (Snap to Contour projects onto a segment; the envelope's projective
/// solve leaves rounding).
fn grid(v: &Value) -> (i64, i64) {
    let (x, y) = (v["x"].as_f64().unwrap() * GRID as f64, v["y"].as_f64().unwrap() * GRID as f64);
    (x.round() as i64, y.round() as i64)
}

fn exact(v: &Value) -> Pt {
    (Q::from_f64(v["x"].as_f64().unwrap() * GRID as f64), Q::from_f64(v["y"].as_f64().unwrap() * GRID as f64))
}

fn to_pt(p: (i64, i64)) -> Pt {
    oracle::pt(p.0, p.1)
}

/// The straight segments of a flat Bézier list (`p0, c1, c2, p1, …`): every control point must lie on its chord
/// (a straight cubic), else the case uses a curve the exact oracle does not model.
fn straight_segments(bez: &[Value], is_closed: bool) -> Segs {
    let segs = try_straight(bez, is_closed);
    assert!(segs.is_some(), "a curved segment in {bez:?}");
    segs.unwrap_or_default()
}

/// The straight segments of a flat Bézier list, or `None` when a segment is curved.
fn try_straight(bez: &[Value], is_closed: bool) -> Option<Segs> {
    let mut out = Vec::new();
    let mut k = 0;
    let n = bez.len();
    while k + 3 < n || (is_closed && k + 3 == n) {
        let (ra, rb) = (&bez[k], &bez[if k + 3 < n { k + 3 } else { 0 }]);
        let (a, b) = (grid(ra), grid(rb));
        for c in [&bez[k + 1], &bez[k + 2]] {
            // Handles lie on the chord between the stored anchors (anywhere along it).
            if !(oracle::point_on_segment(&exact(c), &exact(ra), &exact(rb)) || near_chord(c, ra, rb)) {
                return None;
            }
        }
        out.push((a, b));
        k += 3;
    }
    Some(out)
}

fn read(h: &ExecutorHandle, d: &str, layer: &str) -> (Vec<Stroke>, Vec<Zone>) {
    let r = ok(h, "draw.drawing.get", json!({ "drawingId": d, "include": ["zones", "outline"] }));
    let node = r["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).cloned().unwrap_or(Value::Null);
    let mut strokes = Vec::new();
    for s in node["strokes"].as_array().into_iter().flatten() {
        let mut curves = Vec::new();
        let segs = if s["kind"] == "brush" {
            // A curved brush outline contour is outside the exact oracle: `check` requires it to occupy no expected
            // face; a straight one is a polygon the oracle models.
            let mut segs = Vec::new();
            for c in s["outline"].as_array().into_iter().flatten().filter_map(Value::as_array) {
                match try_straight(c, true) {
                    Some(v) => segs.extend(v),
                    None => curves.push(flatten_contour(c)),
                }
            }
            segs
        } else {
            straight_segments(s["bezier"].as_array().unwrap(), s["closed"] == true)
        };
        let bez = s["bezier"].as_array().cloned().unwrap_or_default();
        let anchors: Vec<(i64, i64)> = bez.iter().step_by(3).map(grid).collect();
        strokes.push(Stroke {
            id: s["id"].as_str().unwrap().to_owned(),
            segs,
            curves,
            anchors,
            closed: s["closed"] == true,
        });
    }
    let zones = node["zones"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|z| Zone {
            id: z["id"].as_str().unwrap().to_owned(),
            area: z["area"].as_f64().unwrap(),
            boundary: z["boundaryStrokeIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect(),
            colour: z["colourId"].as_str().map(str::to_owned),
            interior: (
                Q::from_f64(z["interiorPoint"]["x"].as_f64().unwrap() * GRID as f64),
                Q::from_f64(z["interiorPoint"]["y"].as_f64().unwrap() * GRID as f64),
            ),
        })
        .collect();
    (strokes, zones)
}

/// A closed flat Bézier contour (`p0, c1, c2, p1, …`, closing back to `p0`) flattened in grid units: each cubic in
/// uniform parameter steps, as many as Wang's bound needs for 1/128 du (half a grid unit), its vertices exact.
fn flatten_contour(bez: &[Value]) -> Vec<Pt> {
    let f = |v: &Value| (v["x"].as_f64().unwrap() * GRID as f64, v["y"].as_f64().unwrap() * GRID as f64);
    let n = bez.len();
    let mut out = Vec::new();
    let mut k = 0;
    while n >= 3 && k + 3 <= n {
        let (p0, p1, p2) = (f(&bez[k]), f(&bez[k + 1]), f(&bez[k + 2]));
        let p3 = f(&bez[if k + 3 < n { k + 3 } else { 0 }]);
        let dd = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| (a.0 - 2.0 * b.0 + c.0).hypot(a.1 - 2.0 * b.1 + c.1);
        let m = 6.0 * dd(p0, p1, p2).max(dd(p1, p2, p3));
        let pieces = (m / (8.0 * 0.5)).sqrt().ceil().clamp(1.0, 4096.0) as u32;
        for i in 0..pieces {
            let t = f64::from(i) / f64::from(pieces);
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let x = a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0;
            let y = a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1;
            out.push((Q::from_f64(x), Q::from_f64(y)));
        }
        k += 3;
    }
    out.dedup();
    out
}

/// The stretch of the grid segment `p`–`q` that the grid segment `a`–`b` runs along (grid units from `p`), if longer
/// than two units (a short piece meeting `p`–`q` at a vertex comes within a unit of it over 1/sin of their angle): the points of `p`–`q` within one grid unit of `a`–`b`'s line and projecting inside `a`–`b` (a split
/// leaves its junction rounded onto the grid, so pieces of one line lie up to a unit off each other's snapped
/// segments), for segments parallel within that unit over the shorter one's length (a shallow crossing is not a
/// stretch).
fn stretch(p: (i64, i64), q: (i64, i64), a: (i64, i64), b: (i64, i64)) -> Option<(f64, f64)> {
    let (dx, dy) = ((q.0 - p.0) as f64, (q.1 - p.1) as f64);
    let (ex, ey) = ((b.0 - a.0) as f64, (b.1 - a.1) as f64);
    let (l, m) = (dx.hypot(dy), ex.hypot(ey));
    if l == 0.0 || m == 0.0 {
        return None;
    }
    if (dx * ey - dy * ex).abs() / (l * m) > 2.0 / l.min(m) {
        return None;
    }
    // Along p–q (parameter t in [0, 1]): the distance from a–b's line, and the position along a–b.
    let (ux, uy) = (ex / m, ey / m);
    let (px, py) = ((p.0 - a.0) as f64, (p.1 - a.1) as f64);
    let perp = (px * uy - py * ux, dx * uy - dy * ux);
    let along = (px * ux + py * uy, dx * ux + dy * uy);
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for (c0, c1, min, max) in [(perp.0, perp.1, -1.0, 1.0), (along.0, along.1, 0.0, m)] {
        if c1.abs() < 1e-12 {
            if c0 < min || c0 > max {
                return None;
            }
        } else {
            let (t0, t1) = ((min - c0) / c1, (max - c0) / c1);
            lo = lo.max(t0.min(t1));
            hi = hi.min(t0.max(t1));
        }
    }
    (hi > lo && (hi - lo) * l > 2.0).then_some((lo * l, hi * l))
}

/// The non-zero winding number of a closed polygon around `p` (exact; `p` never on the polygon).
fn winding(poly: &[Pt], p: &Pt) -> i32 {
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

/// A handle within 1e-6 grid units of the chord between two stored anchors (a floating-point map's rounding).
fn near_chord(c: &Value, ra: &Value, rb: &Value) -> bool {
    let f = |v: &Value| (v["x"].as_f64().unwrap() * GRID as f64, v["y"].as_f64().unwrap() * GRID as f64);
    let ((x, y), (ax, ay), (bx, by)) = (f(c), f(ra), f(rb));
    let len = (bx - ax).hypot(by - ay);
    if len == 0.0 {
        return (x - ax).hypot(y - ay) < 1e-6;
    }
    let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / (len * len);
    let d = ((bx - ax) * (y - ay) - (by - ay) * (x - ax)).abs() / len;
    d < 1e-6 && t > -1e-9 && t < 1.0 + 1e-9
}

/// A stroke's anchors in order and whether it is closed (its last segment returns to its first anchor).
fn anchors_of(s: &Stroke) -> (Vec<(i64, i64)>, bool) {
    (s.anchors.clone(), s.closed)
}

/// All segments of some strokes.
fn segments_of(strokes: &[Stroke]) -> Vec<(Pt, Pt)> {
    // Zero-length segments (a collapsed contour's retained anchors) are snap candidates but never edges (C24-R2-06).
    strokes.iter().flat_map(|s| s.segs.iter().filter(|(a, b)| a != b).map(|&(a, b)| (to_pt(a), to_pt(b)))).collect()
}

/// The `strokeId:side` labels of the product strokes lying on the oracle edge `p → q` (snapped coordinates), and the
/// IDs of those strokes.
fn label(strokes: &[Stroke], p: &Pt, q: &Pt) -> (String, Vec<String>) {
    let (sp, sq) = (oracle::snap_pt(p), oracle::snap_pt(q));
    let mut labels = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for s in strokes {
        for &(a, b) in &s.segs {
            let on = |v: (i64, i64)| through_pixel(a, b, v);
            if on(sp) && on(sq) {
                let dot =
                    i128::from(b.0 - a.0) * i128::from(sq.0 - sp.0) + i128::from(b.1 - a.1) * i128::from(sq.1 - sp.1);
                labels.insert(format!("{}:{}", s.id, if dot > 0 { 'L' } else { 'R' }));
                ids.insert(s.id.clone());
            }
        }
    }
    (labels.into_iter().collect::<Vec<_>>().join(","), ids.into_iter().collect())
}

/// Whether the grid segment `a`–`b` meets the closed unit square around the grid point `v` (snap rounding, ADR-0017:
/// a segment passing through a vertex's pixel is routed through that vertex), computed independently with doubled
/// coordinates and exact signs.
fn through_pixel(a: (i64, i64), b: (i64, i64), v: (i64, i64)) -> bool {
    let (a2, b2) = ((2 * a.0, 2 * a.1), (2 * b.0, 2 * b.1));
    let (lo, hi) = ((2 * v.0 - 1, 2 * v.1 - 1), (2 * v.0 + 1, 2 * v.1 + 1));
    if a2.0.max(b2.0) < lo.0 || a2.0.min(b2.0) > hi.0 || a2.1.max(b2.1) < lo.1 || a2.1.min(b2.1) > hi.1 {
        return false;
    }
    let side = |c: (i64, i64)| {
        let o = i128::from(b2.0 - a2.0) * i128::from(c.1 - a2.1) - i128::from(b2.1 - a2.1) * i128::from(c.0 - a2.0);
        o.signum()
    };
    let signs = [side((lo.0, lo.1)), side((hi.0, lo.1)), side((hi.0, hi.1)), side((lo.0, hi.1))];
    !(signs.iter().all(|s| *s > 0) || signs.iter().all(|s| *s < 0))
}

/// The product's zones matched to the oracle's faces (the zone index of each face).
fn match_zones(arr: &Arrangement, zones: &[Zone]) -> Result<Vec<usize>, TestCaseError> {
    prop_assert_eq!(zones.len(), arr.faces.len(), "zones {:?} vs {} oracle faces", zones, arr.faces.len());
    let mut of_face: Vec<Option<usize>> = vec![None; arr.faces.len()];
    for (z, zone) in zones.iter().enumerate() {
        let f = arr.locate(&zone.interior);
        prop_assert!(f.is_some(), "zone {} interior point lies in no oracle face", zone.id);
        let f = f.unwrap();
        prop_assert!(of_face[f].is_none(), "two zones in oracle face {}", f);
        of_face[f] = Some(z);
    }
    Ok(of_face.into_iter().map(Option::unwrap).collect())
}

type FillRule<'a> = dyn Fn(&Arrangement, &[String], usize) -> Option<String> + 'a;

/// Checks a layer's read-back against the oracle's arrangement of `expected` and the expected fill of each face;
/// returns the arrangement and the zone IDs per face.
///
/// Besides the faces (partition, areas, boundary strokes, IDs, fills), the strokes themselves are compared with the
/// expectation (C24-R2-01: a command that changes strokes without changing faces must not pass as a no-op):
/// - **multiplicity**: `expected` is a multiset of stroke segments; along every oracle edge (bounding a face or not),
///   on each stretch between the ends of the product segments running along it (within a grid unit of its line: a
///   split rounds its junction onto the grid), the number of product segments covering it equals the number of
///   expected segments containing the edge;
/// - **converse coverage**: every straight product segment is covered by oracle edges along it (no unexpected stroke);
/// - **brush occupancy**: no expected face is occupied by a curved brush outline (none of its flattened vertices lies
///   in a face, and no face's interior point has non-zero winding with respect to it): an unmodelled brush may only
///   lie outside every expected face.
fn check(
    strokes: &[Stroke],
    zones: &[Zone],
    expected: &[(Pt, Pt)],
    fill: &FillRule<'_>,
) -> Result<(Arrangement, Vec<String>), TestCaseError> {
    let mut arr = oracle::arrangement(expected);
    let zone_of = match_zones(&arr, zones)?;
    // Each face's probe becomes its zone's published interior point (just checked to lie in that face): the carry
    // rule's fallback is the old zone holding the zone's interior point, which can lie in a snapping sliver beside the
    // oracle's own probe (C24-R2, XOR-5D point-move seed).
    for (f, &z) in zone_of.iter().enumerate() {
        arr.faces[f].probe = zones[z].interior.clone();
    }
    let label_of = |p: &Pt, q: &Pt| label(strokes, p, q).0;
    let ids: Vec<String> = arr.faces.iter().map(|f| oracle::zone_id(f, &label_of)).collect();
    for (f, face) in arr.faces.iter().enumerate() {
        let zone = &zones[zone_of[f]];
        let area = oracle::snapped_area2(face) as f64 / 2.0 / (GRID * GRID) as f64;
        prop_assert_eq!(zone.area, area, "zone {} area", zone.id);
        let mut members = BTreeSet::new();
        for cycle in std::iter::once(&face.outer).chain(&face.holes) {
            for k in 0..cycle.len() {
                let (p, q) = (&cycle[k], &cycle[(k + 1) % cycle.len()]);
                let (labels, on) = label(strokes, p, q);
                prop_assert!(
                    !labels.is_empty(),
                    "no product stroke on oracle edge {:?} -> {:?}",
                    oracle::snap_pt(p),
                    oracle::snap_pt(q)
                );
                members.extend(on);
            }
        }
        prop_assert_eq!(&zone.boundary, &members.into_iter().collect::<Vec<_>>(), "zone {} boundary strokes", zone.id);
        prop_assert_eq!(&zone.id, &ids[f], "zone ID of oracle face {}", f);
        prop_assert_eq!(&zone.colour, &fill(&arr, &ids, f), "fill of zone {}", zone.id);
    }
    for (p, q) in &arr.edges {
        let (sp, sq) = (oracle::snap_pt(p), oracle::snap_pt(q));
        let want = expected
            .iter()
            .filter(|(a, b)| oracle::point_on_segment(p, a, b) && oracle::point_on_segment(q, a, b))
            .count();
        let l = ((sq.0 - sp.0) as f64).hypot((sq.1 - sp.1) as f64);
        let on: Vec<(f64, f64)> =
            strokes.iter().flat_map(|s| &s.segs).filter_map(|&(a, b)| stretch(sp, sq, a, b)).collect();
        let mut cuts: Vec<f64> = vec![0.0, l];
        cuts.extend(on.iter().flat_map(|&(t0, t1)| [t0, t1]));
        cuts.sort_by(f64::total_cmp);
        for w in cuts.windows(2).filter(|w| w[1] - w[0] > 2.0) {
            let m = (w[0] + w[1]) / 2.0;
            let have = on.iter().filter(|&&(t0, t1)| t0 < m && m < t1).count();
            prop_assert_eq!(have, want, "stroke segments on oracle edge {:?} -> {:?} at {} of {}", sp, sq, m, l);
        }
    }
    for s in strokes {
        for &(a, b) in &s.segs {
            let covered: f64 = arr
                .edges
                .iter()
                .filter_map(|(p, q)| stretch(oracle::snap_pt(p), oracle::snap_pt(q), a, b))
                .map(|(t0, t1)| t1 - t0)
                .sum();
            let len = ((b.0 - a.0) as f64).hypot((b.1 - a.1) as f64);
            prop_assert!(
                (covered - len).abs() <= 2.0,
                "stroke {} segment {:?} -> {:?} is not on the expected arrangement ({} of {} grid units covered)",
                s.id,
                a,
                b,
                covered,
                len
            );
        }
        for c in &s.curves {
            for v in c {
                prop_assert!(arr.locate(v).is_none(), "brush {} outline enters an expected face", s.id);
            }
            for face in &arr.faces {
                prop_assert_eq!(winding(c, &face.probe), 0, "brush {} outline occupies an expected face", s.id);
            }
        }
    }
    Ok((arr, ids))
}

/// Twice the length of `face`'s boundary inside the box of `old` (grid units): the documented snapping bound of the
/// carry-over (ADR-0017 history 5), computed independently.
fn snapping_bound(face: &oracle::Face, old: &oracle::Face) -> f64 {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &old.outer {
        let (x, y) = (p.0.clone().f64(), p.1.clone().f64());
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let mut len = 0.0;
    for c in std::iter::once(&face.outer).chain(&face.holes) {
        for k in 0..c.len() {
            let a = (c[k].0.clone().f64(), c[k].1.clone().f64());
            let b = (c[(k + 1) % c.len()].0.clone().f64(), c[(k + 1) % c.len()].1.clone().f64());
            // Liang-Barsky clip of a-b to the box.
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
                len += (t1 - t0) * dx.hypot(dy);
            }
        }
    }
    2.0 * len
}

/// The state of a layer before a command, as the oracle sees it.
struct Old {
    arr: Arrangement,
    segs: Vec<(Pt, Pt)>,
    fills: Vec<Option<String>>,
    ids: Vec<String>,
}

/// Carry-over, recomputed (see the module comment). Overlaps are ranked on the snapped polygons, as the product's rule
/// states ("exact polygon intersection on the grid"): the new faces with their vertices snapped (the old ones already
/// are: the state before a command is persisted grid geometry).
fn carried<'a>(old: &'a Old, new_segs: &[(Pt, Pt)]) -> impl Fn(&Arrangement, &[String], usize) -> Option<String> + 'a {
    let _ = new_segs;
    move |arr: &Arrangement, ids: &[String], f: usize| {
        if let Some(o) = old.ids.iter().position(|i| *i == ids[f]) {
            return old.fills[o].clone();
        }
        let snapped: Vec<(Pt, Pt)> = arr
            .edges
            .iter()
            .map(|(p, q)| (to_pt(oracle::snap_pt(p)), to_pt(oracle::snap_pt(q))))
            .filter(|(p, q)| p != q)
            .collect();
        let grid_arr = oracle::arrangement(&snapped);
        // The snapped face with the same snapped vertices (an interior point may cross a snapped boundary).
        let key = |face: &oracle::Face| -> BTreeSet<(i64, i64)> {
            std::iter::once(&face.outer).chain(&face.holes).flatten().map(oracle::snap_pt).collect()
        };
        let want = key(&arr.faces[f]);
        // With no overlap above the bound, the old zone holding the zone's interior point (the face's probe: `check`
        // sets it to the zone's published interior point).
        let Some(g) = grid_arr.faces.iter().position(|gf| key(gf) == want) else {
            return old.arr.locate(&arr.faces[f].probe).and_then(|o| old.fills[o].clone());
        };
        let over = oracle::overlaps(&old.arr, &grid_arr, &old.segs, &snapped);
        let mut best: Option<(Q, usize)> = None;
        for ((n, o), a2) in &over {
            if *n != g {
                continue;
            }
            let area = a2.clone().f64() / 2.0;
            if area <= snapping_bound(&grid_arr.faces[g], &old.arr.faces[*o]) {
                continue;
            }
            let better = match &best {
                None => true,
                Some((b, bo)) => match a2.cmp_to(b) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => old.ids[*o] < old.ids[*bo],
                    std::cmp::Ordering::Less => false,
                },
            };
            if better {
                best = Some((a2.clone(), *o));
            }
        }
        match best {
            Some((_, o)) => old.fills[o].clone(),
            None => old.arr.locate(&arr.faces[f].probe).and_then(|o| old.fills[o].clone()),
        }
    }
}

fn spec_segments(list: &[Spec]) -> Vec<(Pt, Pt)> {
    list.iter()
        .flat_map(|s| {
            let p: Vec<(i64, i64)> = s.1.iter().map(|&q| lattice(q)).collect();
            poly_segs(&p, closed(s))
        })
        .collect()
}

/// Paints every zone of a layer (colours in turn) and returns its state; checks it against the oracle first. `None`
/// when the persisted pieces are not snap-safe (a rounded junction leaves a sub-grid sliver beside a degenerate input).
fn paint(h: &ExecutorHandle, d: &str, layer: &str) -> Result<Option<Old>, TestCaseError> {
    let (strokes, zones) = read(h, d, layer);
    let pre = segments_of(&strokes);
    if !safe(&pre) {
        return Ok(None);
    }
    let (arr, ids) = check(&strokes, &zones, &pre, &|_, _, _| None)?;
    let palette = colours(h);
    for (f, id) in ids.iter().enumerate() {
        ok(
            h,
            "colour.paint.apply",
            json!({ "drawingId": d, "objectIds": [id], "colourId": palette[f % palette.len()] }),
        );
    }
    let (_, zones) = read(h, d, layer);
    let zone_of = match_zones(&arr, &zones)?;
    let fills: Vec<Option<String>> = (0..arr.faces.len()).map(|f| zones[zone_of[f]].colour.clone()).collect();
    let ids: Vec<String> = (0..arr.faces.len()).map(|f| zones[zone_of[f]].id.clone()).collect();
    Ok(Some(Old { arr, segs: pre, fills, ids }))
}

/// Builds the arrangement through the commands, checks and paints it. `None` when the arrangement is not snap-safe.
fn setup(h: &ExecutorHandle, list: &[Spec]) -> Result<Option<(String, String, Old)>, TestCaseError> {
    if !safe(&spec_segments(list)) {
        return Ok(None);
    }
    let r = ok(
        h,
        "draw.content.add",
        json!({ "elementId": BOB, "frame": 1, "createDrawing": true, "select": false,
                "content": { "nodes": [layer_json(list, "l_a")] } }),
    );
    let d = r["drawingId"].as_str().unwrap().to_owned();
    let layer = r["idMap"]["l_a"].as_str().unwrap_or("l_a").to_owned();
    Ok(paint(h, &d, &layer)?.map(|old| (d, layer, old)))
}

fn without(segs: &[(Pt, Pt)], remove: &[(Pt, Pt)]) -> Vec<(Pt, Pt)> {
    let mut out = segs.to_vec();
    for r in remove {
        if let Some(i) = out.iter().position(|s| s == r) {
            out.remove(i);
        }
    }
    out
}

/// Grid segments of a du polyline.
fn poly_segs(points: &[(i64, i64)], is_closed: bool) -> Vec<(Pt, Pt)> {
    let g: Vec<(i64, i64)> = points.iter().map(|p| (p.0 * GRID, p.1 * GRID)).collect();
    let mut v: Vec<(Pt, Pt)> = g.windows(2).map(|w| (to_pt(w[0]), to_pt(w[1]))).collect();
    if is_closed {
        v.push((to_pt(g[g.len() - 1]), to_pt(g[0])));
    }
    v
}

fn safe(segs: &[(Pt, Pt)]) -> bool {
    oracle::snap_safe(&oracle::arrangement(segs), MARGIN)
}

fn property<S: Strategy>(
    seed: &[u8; 32],
    strategy: S,
    op: impl Fn(&ExecutorHandle, &S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    DISPATCHES.with(|c| c.borrow_mut().clear());
    let _coverage = CoverageGuard(format!("seed={} xor=5D cases={}", String::from_utf8_lossy(seed), cases()));
    runner(seed).run(&strategy, |v| op(&fresh(), &v)).unwrap();
}

#[test]
fn stroke_add_matches_the_oracle() {
    let extra = prop::collection::vec((0i64..5, 0i64..5), 2..5);
    property(b"toontown-m2-c24-oracle-route-add", (arrangement_strategy(), extra), |h, (list, extra)| {
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let p: Vec<(i64, i64)> = extra.iter().map(|&q| lattice(q)).collect();
        let mut after = old.segs.clone();
        after.extend(poly_segs(&p, false));
        if !safe(&after) {
            return Ok(());
        }
        ok(
            h,
            "draw.stroke.add",
            json!({ "drawingId": d, "kind": "pencil", "layer": layer, "fit": false, "bezier": bezier(&p, false) }),
        );
        let (strokes, zones) = read(h, &d, &layer);
        check(&strokes, &zones, &after, &carried(&old, &after))?;
        Ok(())
    });
}

#[test]
fn stroke_set_and_delete_match_the_oracle() {
    let extra = prop::collection::vec((0i64..5, 0i64..5), 2..4);
    property(
        b"toontown-m2-c24-oracle-route-set",
        (arrangement_strategy(), extra, any::<prop::sample::Index>(), any::<bool>()),
        |h, (list, extra, pick, delete)| {
            let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
            let (strokes, _) = read(h, &d, &layer);
            let target = &strokes[pick.index(strokes.len())];
            let removed = segments_of(std::slice::from_ref(target));
            let mut after = without(&old.segs, &removed);
            if *delete {
                if !safe(&after) {
                    return Ok(());
                }
                ok(h, "draw.object.delete", json!({ "drawingId": d, "ids": [target.id] }));
            } else {
                let p: Vec<(i64, i64)> = extra.iter().map(|&q| lattice(q)).collect();
                after.extend(poly_segs(&p, false));
                if !safe(&after) {
                    return Ok(());
                }
                ok(
                    h,
                    "draw.stroke.set",
                    json!({ "drawingId": d, "strokeId": target.id, "bezier": bezier(&p, false), "closed": false }),
                );
            }
            let (strokes, zones) = read(h, &d, &layer);
            check(&strokes, &zones, &after, &carried(&old, &after))?;
            Ok(())
        },
    );
}

/// The pieces of `a`–`b` (split where it meets `others`) whose middle lies in a bounded face of `arr`.
fn inside_pieces(a: &Pt, b: &Pt, others: &[(Pt, Pt)], arr: &Arrangement) -> Vec<(Pt, Pt)> {
    let mut ts = vec![Q::int(0)];
    ts.extend(oracle::cut_parameters(a, b, others));
    ts.push(Q::int(1));
    let mut out = Vec::new();
    for w in ts.windows(2) {
        let mid = oracle::lerp(a, b, &w[0].plus(&w[1]).over(&Q::int(2)));
        if arr.locate(&mid).is_some() {
            out.push((oracle::lerp(a, b, &w[0]), oracle::lerp(a, b, &w[1])));
        }
    }
    out
}

#[test]
fn segment_cut_matches_the_oracle() {
    // The cut runs between points off the lattice, so it crosses strokes in their interiors.
    let ends = ((0i64..5, 0i64..5), (0i64..5, 0i64..5));
    property(b"toontown-m2-c24-oracle-route-cut", (arrangement_strategy(), ends), |h, (list, (p, q))| {
        if p == q {
            return Ok(());
        }
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let (a, b) = ((p.0 * STEP + STEP / 2, p.1 * STEP + 3), (q.0 * STEP + 7, q.1 * STEP + STEP / 2));
        let (ga, gb) = (to_pt((a.0 * GRID, a.1 * GRID)), to_pt((b.0 * GRID, b.1 * GRID)));
        let mut after = old.segs.clone();
        after.extend(inside_pieces(&ga, &gb, &old.segs, &old.arr));
        if !safe(&after) {
            return Ok(());
        }
        let env = run(
            h,
            "draw.cut.apply",
            json!({ "drawingId": d, "segment": { "from": {"x": a.0, "y": a.1}, "to": {"x": b.0, "y": b.1} } }),
        );
        if let Some(e) = &env.error {
            // A cut that meets no stroke has nothing to cut.
            prop_assert_eq!(e.code.as_str(), "drawing.nothingToDo");
            prop_assert!(oracle::cut_parameters(&ga, &gb, &old.segs).is_empty());
            return Ok(());
        }
        let (strokes, zones) = read(h, &d, &layer);
        check(&strokes, &zones, &after, &carried(&old, &after))?;
        Ok(())
    });
}

fn inside_polygon(poly: &[Pt], p: &Pt) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let above = |v: &Pt| v.1.cmp_to(&p.1) == std::cmp::Ordering::Greater;
        if above(a) != above(b) {
            let x = a.0.plus(&p.1.minus(&a.1).times(&b.0.minus(&a.0)).over(&b.1.minus(&a.1)));
            if p.0.cmp_to(&x) == std::cmp::Ordering::Less {
                c = !c;
            }
        }
    }
    c
}

#[test]
fn region_erase_matches_the_oracle() {
    // A rectangle between half-lattice lines: it never runs along a stroke.
    let rect = ((0i64..4, 0i64..4), (1i64..4, 1i64..4));
    property(b"toontown-m2-c24-oracle-route-era", (arrangement_strategy(), rect), |h, (list, ((x, y), (w, ht)))| {
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let (x0, y0) = (x * STEP + STEP / 2, y * STEP + STEP / 2);
        let (x1, y1) = (x0 + w * STEP, y0 + ht * STEP);
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let region: Vec<Pt> = corners.iter().map(|&(cx, cy)| to_pt((cx * GRID, cy * GRID))).collect();
        let region_segs: Vec<(Pt, Pt)> = (0..4).map(|i| (region[i].clone(), region[(i + 1) % 4].clone())).collect();
        let mut after = Vec::new();
        for (a, b) in &old.segs {
            let mut ts = vec![Q::int(0)];
            ts.extend(oracle::cut_parameters(a, b, &region_segs));
            ts.push(Q::int(1));
            for w in ts.windows(2) {
                let mid = oracle::lerp(a, b, &w[0].plus(&w[1]).over(&Q::int(2)));
                if !inside_polygon(&region, &mid) {
                    after.push((oracle::lerp(a, b, &w[0]), oracle::lerp(a, b, &w[1])));
                }
            }
        }
        for (a, b) in &region_segs {
            after.extend(inside_pieces(a, b, &old.segs, &old.arr));
        }
        if !safe(&after) {
            return Ok(());
        }
        let pts: Vec<Value> = corners.iter().map(|&(cx, cy)| json!({ "x": cx, "y": cy })).collect();
        let env = run(h, "draw.erase.apply", json!({ "drawingId": d, "region": pts, "layers": [layer] }));
        if let Some(e) = &env.error {
            prop_assert_eq!(e.code.as_str(), "drawing.nothingToDo");
            prop_assert_eq!(after.len(), old.segs.len());
            return Ok(());
        }
        let (strokes, zones) = read(h, &d, &layer);
        let carry = carried(&old, &after);
        let fill = |arr: &Arrangement, ids: &[String], f: usize| {
            if inside_polygon(&region, &arr.faces[f].probe) { None } else { carry(arr, ids, f) }
        };
        check(&strokes, &zones, &after, &fill)?;
        Ok(())
    });
}

#[test]
fn flatten_of_overlapping_layers_matches_the_oracle() {
    property(b"toontown-m2-c24-oracle-route-fla", (arrangement_strategy(), arrangement_strategy()), |h, (a, b)| {
        let mut both = spec_segments(a);
        both.extend(spec_segments(b));
        if !safe(&spec_segments(b)) || !safe(&both) {
            return Ok(());
        }
        let Some((d, layer, old)) = setup(h, a)? else { return Ok(()) };
        let r = ok(
            h,
            "draw.content.add",
            json!({ "drawingId": d, "select": false, "content": { "nodes": [layer_json(b, "l_b")] } }),
        );
        let other = r["idMap"]["l_b"].as_str().unwrap().to_owned();
        let (second, _) = read(h, &d, &other);
        let mut after = old.segs.clone();
        after.extend(segments_of(&second));
        if !safe(&after) {
            return Ok(());
        }
        let merged = ok(h, "draw.flatten.apply", json!({ "drawingId": d, "ids": [layer, other] }));
        let target = merged["layerId"].as_str().unwrap().to_owned();
        let (strokes, zones) = read(h, &d, &target);
        check(&strokes, &zones, &after, &carried(&old, &after))?;
        Ok(())
    });
}

fn run_points(r: &(bool, i64, i64, i64)) -> Vec<(i64, i64)> {
    let (vertical, line, start, len) = *r;
    (start..(start + len).min(5)).map(|k| if vertical { (line, k) } else { (k, line) }).collect()
}

#[test]
fn optimize_and_smooth_keep_the_oracles_partition() {
    property(
        b"toontown-m2-c24-oracle-route-opt",
        (arrangement_strategy(), (any::<bool>(), 0i64..5, 0i64..2, 3i64..6), any::<bool>()),
        |h, (list, run, smooth)| {
            let pts = run_points(run);
            if pts.len() < 3 {
                return Ok(());
            }
            let mut list = list.clone();
            list.push((false, pts));
            let Some((d, layer, old)) = setup(h, &list)? else { return Ok(()) };
            let (strokes, _) = read(h, &d, &layer);
            if *smooth {
                // Smoothing pieces whose anchors are collinear leaves them straight lines.
                let ids: Vec<String> = strokes
                    .iter()
                    .filter(|s| {
                        s.segs.len() >= 2
                            && s.segs.windows(2).all(|w| {
                                let (a, b, c) = (w[0].0, w[0].1, w[1].1);
                                let collinear = i128::from(b.0 - a.0) * i128::from(c.1 - a.1)
                                    == i128::from(b.1 - a.1) * i128::from(c.0 - a.0);
                                let onward = i128::from(b.0 - a.0) * i128::from(c.0 - b.0)
                                    + i128::from(b.1 - a.1) * i128::from(c.1 - b.1)
                                    > 0;
                                collinear && onward
                            })
                    })
                    .map(|s| s.id.clone())
                    .collect();
                if ids.is_empty() {
                    return Ok(());
                }
                ok(h, "draw.smooth.apply", json!({ "drawingId": d, "ids": ids, "tolerance": 1 }));
            } else {
                ok(h, "draw.optimize.apply", json!({ "drawingId": d, "ids": [layer] }));
            }
            let (strokes, zones) = read(h, &d, &layer);
            check(&strokes, &zones, &old.segs, &carried(&old, &old.segs))?;
            Ok(())
        },
    );
}

/// An exact integer map on grid coordinates (`m` row-major, translation in du) and its inverse.
#[derive(Debug, Clone, Copy)]
struct Map {
    m: [i64; 4],
    t: (i64, i64),
}

impl Map {
    fn apply(&self, p: &Pt) -> Pt {
        let [a, b, c, dd] = self.m;
        (
            Q::int(a).times(&p.0).plus(&Q::int(b).times(&p.1)).plus(&Q::int(self.t.0 * GRID)),
            Q::int(c).times(&p.0).plus(&Q::int(dd).times(&p.1)).plus(&Q::int(self.t.1 * GRID)),
        )
    }
    fn inverse(&self, p: &Pt) -> Pt {
        let [a, b, c, dd] = self.m;
        let det = Q::int(a * dd - b * c);
        let (x, y) = (p.0.minus(&Q::int(self.t.0 * GRID)), p.1.minus(&Q::int(self.t.1 * GRID)));
        (
            Q::int(dd).times(&x).minus(&Q::int(b).times(&y)).over(&det),
            Q::int(-c).times(&x).plus(&Q::int(a).times(&y)).over(&det),
        )
    }
}

#[test]
fn transforms_match_the_oracles_transformed_geometry() {
    let maps = [
        ("rotate", Map { m: [0, -1, 1, 0], t: (0, 0) }),
        ("half turn", Map { m: [-1, 0, 0, -1], t: (0, 0) }),
        ("mirror", Map { m: [-1, 0, 0, 1], t: (0, 0) }),
        ("translate", Map { m: [1, 0, 0, 1], t: (37, -11) }),
        ("rotate and translate", Map { m: [0, 1, -1, 0], t: (5, 13) }),
    ];
    property(b"toontown-m2-c24-oracle-route-tra", (arrangement_strategy(), 0usize..5), |h, (list, which)| {
        let (name, map) = maps[*which];
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let after: Vec<(Pt, Pt)> = old.segs.iter().map(|(a, b)| (map.apply(a), map.apply(b))).collect();
        if !safe(&after) {
            return Ok(());
        }
        let (strokes, _) = read(h, &d, &layer);
        let ids: Vec<&str> = strokes.iter().map(|s| s.id.as_str()).collect();
        let [a, b, c, dd] = map.m;
        // matrix [a b c d e f]: x' = a x + c y + e, y' = b x + d y + f.
        ok(h, "draw.transform.apply", json!({ "drawingId": d, "ids": ids, "matrix": [a, c, b, dd, map.t.0, map.t.1] }));
        let (strokes, zones) = read(h, &d, &layer);
        // Faces move with their strokes and keep their own fill.
        let fill = |arr: &Arrangement, _: &[String], f: usize| {
            old.arr.locate(&map.inverse(&arr.faces[f].probe)).and_then(|o| old.fills[o].clone())
        };
        check(&strokes, &zones, &after, &fill).map_err(|e| TestCaseError::fail(format!("{name}: {e}")))?;
        Ok(())
    });
}

#[test]
fn affine_envelopes_match_the_oracle() {
    property(b"toontown-m2-c24-oracle-route-env", (arrangement_strategy(), -2i64..3), |h, (list, shear)| {
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let (strokes, _) = read(h, &d, &layer);
        let ids: Vec<&str> = strokes.iter().map(|s| s.id.as_str()).collect();
        // The envelope maps the selection's bounds (as `draw.bounds.get` reports them) onto the corners.
        let b = ok(h, "draw.bounds.get", json!({ "drawingId": d, "ids": ids }))["bounds"].clone();
        let gb = |v: f64| {
            let g = v * GRID as f64;
            assert!(g.fract() == 0.0, "bounds off the grid: {b}");
            g as i64
        };
        let (x0, y0) = (gb(b["x"].as_f64().unwrap()), gb(b["y"].as_f64().unwrap()));
        let (x1, y1) = (x0 + gb(b["width"].as_f64().unwrap()), y0 + gb(b["height"].as_f64().unwrap()));
        if x0 == x1 || y0 == y1 {
            return Ok(());
        }
        // x' = x + shear (y − y0), y' = y: integer on the grid, affine (a parallelogram target).
        let s = *shear;
        let map = |p: &Pt| (p.0.plus(&Q::int(s).times(&p.1.minus(&Q::int(y0)))), p.1.clone());
        let inv = |p: &Pt| (p.0.minus(&Q::int(s).times(&p.1.minus(&Q::int(y0)))), p.1.clone());
        let after: Vec<(Pt, Pt)> = old.segs.iter().map(|(a, b)| (map(a), map(b))).collect();
        if !safe(&after) {
            return Ok(());
        }
        let du = |v: i64| v as f64 / GRID as f64;
        let corners = json!([
            {"x": du(x0), "y": du(y0)}, {"x": du(x1), "y": du(y0)},
            {"x": du(x1 + s * (y1 - y0)), "y": du(y1)}, {"x": du(x0 + s * (y1 - y0)), "y": du(y1)}
        ]);
        ok(h, "draw.envelope.apply", json!({ "drawingId": d, "ids": ids, "corners": corners }));
        let (strokes, zones) = read(h, &d, &layer);
        let fill = |arr: &Arrangement, _: &[String], f: usize| {
            old.arr.locate(&inv(&arr.faces[f].probe)).and_then(|o| old.fills[o].clone())
        };
        check(&strokes, &zones, &after, &fill)?;
        Ok(())
    });
}

#[test]
fn point_moves_with_and_without_snapping_match_the_oracle() {
    let delta = (-2i64..3, -2i64..3, any::<bool>());
    property(
        b"toontown-m2-c24-oracle-route-pnt",
        (arrangement_strategy(), any::<prop::sample::Index>(), any::<prop::sample::Index>(), delta),
        |h, (list, pick, anchor, (dx, dy, snap))| {
            if (*dx, *dy) == (0, 0) {
                return Ok(());
            }
            let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
            let (strokes, _) = read(h, &d, &layer);
            let target = &strokes[pick.index(strokes.len())];
            if target.segs.is_empty() {
                return Ok(());
            }
            let (anchors, _) = anchors_of(target);
            let k = anchor.index(anchors.len());
            let at = anchors[k];
            // Shared junction anchors move together: every anchor of the layer at that exact coordinate.
            let moving: Vec<&Stroke> = strokes.iter().filter(|s| anchors_of(s).0.contains(&at)).collect();
            // With snapping the move lands 3 du short of a whole step; Snap to Contour takes the nearest point of a
            // stroke that does not move (its segments, interiors included) within 8 du; ties to the lower stroke ID,
            // then segment, then parameter (02 §8.9). Snaps onto anchors are checked exactly (see below).
            let step = |v: i64| if *snap { v * STEP * GRID - v.signum() * 3 * GRID } else { v * STEP * GRID };
            let off = (step(*dx), step(*dy));
            let q = to_pt((at.0 + off.0, at.1 + off.1));
            let mut moved_pt = q.clone();
            if *snap {
                let mut best: Option<(Q, String, usize, Q, Pt)> = None;
                for s in strokes.iter().filter(|s| !moving.iter().any(|m| m.id == s.id)) {
                    for (j, &(a, b)) in s.segs.iter().enumerate() {
                        let (pa, pb) = (to_pt(a), to_pt(b));
                        let (ddx, ddy) = (pb.0.minus(&pa.0), pb.1.minus(&pa.1));
                        let len2 = ddx.times(&ddx).plus(&ddy.times(&ddy));
                        let t = q.0.minus(&pa.0).times(&ddx).plus(&q.1.minus(&pa.1).times(&ddy)).over(&len2);
                        let t = if t.signum() < 0 {
                            Q::int(0)
                        } else if t.cmp_to(&Q::int(1)).is_gt() {
                            Q::int(1)
                        } else {
                            t
                        };
                        let on = oracle::lerp(&pa, &pb, &t);
                        let (ex, ey) = (q.0.minus(&on.0), q.1.minus(&on.1));
                        let d2 = ex.times(&ex).plus(&ey.times(&ey));
                        if d2.cmp_to(&Q::int((8 * GRID) * (8 * GRID))).is_gt() {
                            continue;
                        }
                        let better = match &best {
                            None => true,
                            Some((bd, bid, bj, bt, _)) => d2
                                .cmp_to(bd)
                                .then_with(|| s.id.cmp(bid))
                                .then(j.cmp(bj))
                                .then_with(|| t.cmp_to(bt))
                                .is_lt(),
                        };
                        if better {
                            best = Some((d2, s.id.clone(), j, t, on));
                        }
                    }
                }
                if let Some((_, _, _, t, on)) = best {
                    // A snap into a segment's interior lands off the grid, where the junction the noding gives it
                    // depends on the crossing of the snapped end with that segment; the oracle checks snaps onto
                    // anchors exactly and leaves those cases out.
                    if t.signum() > 0 && t.cmp_to(&Q::int(1)).is_lt() {
                        return Ok(());
                    }
                    moved_pt = on;
                }
            }
            let mut after = old.segs.clone();
            for m in &moving {
                after = without(&after, &segments_of(std::slice::from_ref(*m)));
                let (ring, ring_closed) = anchors_of(m);
                let ring: Vec<Pt> = ring.iter().map(|&a| if a == at { moved_pt.clone() } else { to_pt(a) }).collect();
                for w in ring.windows(2) {
                    after.push((w[0].clone(), w[1].clone()));
                }
                if ring_closed {
                    after.push((ring[ring.len() - 1].clone(), ring[0].clone()));
                }
            }
            if !safe(&after) {
                return Ok(());
            }
            ok(
                h,
                "draw.point.move",
                json!({ "drawingId": d, "points": [{ "strokeId": target.id, "anchor": k }],
                        "delta": { "x": off.0 as f64 / GRID as f64, "y": off.1 as f64 / GRID as f64 }, "snap": snap }),
            );
            let (strokes, zones) = read(h, &d, &layer);
            check(&strokes, &zones, &after, &carried(&old, &after))?;
            Ok(())
        },
    );
}

/// A lattice arrangement with an extra line outside it (x = 6 lattice steps), in the unbounded face of every zone.
fn with_isolated_line(list: &[Spec], len: i64) -> Vec<Spec> {
    let mut list = list.to_vec();
    list.push((false, vec![(6, 0), (6, len)]));
    list
}

#[test]
fn lines_to_brush_and_centre_line_keep_the_oracles_partition() {
    // Lines to Brush keeps the line's invisible centre stroke (02 §8.6) and puts the brush beside it; Centre Line turns
    // the brush back into a pencil line along its spine. The converted line lies outside every zone, so the oracle
    // predicts the line layer exactly: unchanged zones before Centre Line, and the spine line added after it. (A brush
    // outline crossing lines is a fitted curve, outside an exact oracle; Break Apart and vectorize cover brush zones by
    // counts.)
    property(b"toontown-m2-c24-oracle-route-l2b", (arrangement_strategy(), 1i64..5), |h, (list, len)| {
        let list = with_isolated_line(list, *len);
        let Some((d, layer, old)) = setup(h, &list)? else { return Ok(()) };
        let (strokes, _) = read(h, &d, &layer);
        let line = strokes
            .iter()
            .find(|s| {
                !s.segs.is_empty() && s.segs.iter().all(|&(a, b)| a.0 == 6 * STEP * GRID && b.0 == 6 * STEP * GRID)
            })
            .unwrap();
        let r = ok(h, "draw.linesToBrush.apply", json!({ "drawingId": d, "ids": [line.id] }));
        let brush = r["strokeIds"][0].as_str().unwrap().to_owned();
        let (strokes, zones) = read(h, &d, &layer);
        check(&strokes, &zones, &old.segs, &carried(&old, &old.segs))?;
        let r = ok(h, "draw.centreLine.apply", json!({ "drawingId": d, "ids": [brush] }));
        prop_assert!(r["skippedIds"].as_array().unwrap().is_empty(), "{}", r);
        let mut after = old.segs.clone();
        after.extend(poly_segs(&[(6 * STEP, 0), (6 * STEP, len * STEP)], false));
        let (strokes, zones) = read(h, &d, &layer);
        check(&strokes, &zones, &after, &carried(&old, &after))?;
        Ok(())
    });
}

#[test]
fn break_apart_leaves_the_line_layer_to_the_oracle_and_makes_one_group_per_glyph() {
    // Text beside a lattice arrangement is broken apart into glyph shapes, each in its own group and layer (02 §8.7):
    // the line layer is exactly the oracle's (partition, IDs, fills unchanged), and each glyph layer has as many
    // zones as its letter has counters (`H`, `I`, `L`, `T`: none; `O`: one), bounded by that glyph's brush stroke.
    let words = ["HI", "LOT", "TIL", "OH"];
    property(b"toontown-m2-c24-oracle-route-brk", (arrangement_strategy(), 0usize..4), |h, (list, which)| {
        let Some((d, layer, old)) = setup(h, list)? else { return Ok(()) };
        let word = words[*which];
        let t = ok(
            h,
            "draw.text.add",
            json!({ "drawingId": d, "position": {"x": 200, "y": 200}, "text": word, "format": {"size": 40} }),
        );
        let r = run(h, "draw.breakApart.apply", json!({ "drawingId": d, "textIds": [t["textId"]] }));
        if r.error.as_ref().is_some_and(|e| e.code == "text.glyphsUnavailable") {
            return Ok(());
        }
        let r = r.result.unwrap();
        let groups = r["groupIds"].as_array().unwrap();
        prop_assert_eq!(groups.len(), word.chars().count());
        let (strokes, zones) = read(h, &d, &layer);
        check(&strokes, &zones, &old.segs, &carried(&old, &old.segs))?;
        let all = ok(h, "draw.drawing.get", json!({ "drawingId": d, "include": ["zones"] }));
        for (g, ch) in groups.iter().zip(word.chars()) {
            let group = all["nodes"].as_array().unwrap().iter().find(|n| n["id"] == *g).unwrap();
            let glyph_layer = &group["children"][0];
            let stroke = glyph_layer["strokes"][0]["id"].clone();
            let zones = glyph_layer["zones"].as_array().cloned().unwrap_or_default();
            prop_assert_eq!(zones.len(), usize::from(ch == 'O'), "zones of glyph {}", ch);
            for z in &zones {
                prop_assert_eq!(&z["boundaryStrokeIds"], &json!([stroke]));
                prop_assert_eq!(&z["colourId"], &Value::Null);
            }
        }
        Ok(())
    });
}

/// C24-R2-01: test the oracle comparison itself against the state a no-op Centre Line would leave.
/// Intentionally failing: rejecting this mutant is required for the claimed independent changed-route proof.
#[test]
fn r1_01_route_oracle_rejects_a_noop_centre_line_mutant() {
    let h = fresh();
    let list = with_isolated_line(&[(true, vec![(0, 0), (4, 0), (4, 4), (0, 4)])], 3);
    let (d, layer, old) = setup(&h, &list).unwrap().unwrap();
    let (strokes, _) = read(&h, &d, &layer);
    let line = strokes
        .iter()
        .find(|s| !s.segs.is_empty() && s.segs.iter().all(|&(a, b)| a.0 == 6 * STEP * GRID && b.0 == 6 * STEP * GRID))
        .unwrap();
    ok(&h, "draw.linesToBrush.apply", json!({"drawingId":d,"ids":[line.id]}));
    let (unchanged_strokes, unchanged_zones) = read(&h, &d, &layer);
    // Exact same expected state as the shipped property after Centre Line.
    // Intentionally do not dispatch Centre Line: these are the unchanged pre-command strokes/zones of a no-op mutant.
    let mut after = old.segs.clone();
    after.extend(segments_of(std::slice::from_ref(line)));
    let verdict = check(&unchanged_strokes, &unchanged_zones, &after, &carried(&old, &after));
    assert!(verdict.is_err(), "the claimed Centre Line oracle accepts doing nothing: {verdict:?}");
}

/// Fixed case shrunk by the XOR-5D point-move property. The product snaps to the collapsed contour's anchor;
/// the shipped route oracle throws zero-length segments away and therefore predicts the unsnapped point instead.
#[test]
fn collapsed_other_contour_is_a_valid_snap_target_product_control() {
    let h = fresh();
    let list =
        vec![(false, vec![(2, 0), (2, 2)]), (false, vec![(4, 1), (4, 1)]), (false, vec![(2, 0), (0, 0), (3, 3)])];
    let (d, layer, old) = setup(&h, &list).unwrap().unwrap();
    let (strokes, _) = read(&h, &d, &layer);
    let at = (40 * GRID, 40 * GRID);
    let target = strokes
        .iter()
        .find(|s| s.anchors.first() == Some(&at) && s.anchors.last() == Some(&(60 * GRID, 60 * GRID)))
        .unwrap();
    let moved = to_pt((80 * GRID, 20 * GRID));
    let original = to_pt(at);
    let after = old
        .segs
        .iter()
        .map(|(a, b)| {
            (
                if *a == original { moved.clone() } else { a.clone() },
                if *b == original { moved.clone() } else { b.clone() },
            )
        })
        .collect::<Vec<_>>();
    ok(
        &h,
        "draw.point.move",
        json!({"drawingId":d,"points":[{"strokeId":target.id,"anchor":0}],
        "delta":{"x":37,"y":-17},"snap":true}),
    );
    let (strokes, zones) = read(&h, &d, &layer);
    check(&strokes, &zones, &after, &carried(&old, &after)).unwrap();
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].area, 400.0);
    assert!(strokes.iter().filter(|s| s.anchors.contains(&(80 * GRID, 20 * GRID))).count() >= 3);
    println!("collapsed snap: requested (77,23), other contour at (80,20), distance sqrt(18) < 8; correct area=400");
}
