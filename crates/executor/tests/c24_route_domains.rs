//! M2 C2.4 round 2 (C24-R2-01 ruling): non-vacuous route properties for the domains the round-1 route oracle left out.
//! Every property generates a starting state through the commands, paints its zones, applies the command and compares
//! the read-back layer with expectations derived from the persisted starting state and the command's semantics; the
//! oracles are in `route_oracle/` (no product code: the exact rational oracle, a snap-rounded map of flattened strokes
//! with brush occupancy, overlap areas, a TrueType reader).
//!
//! Two kinds of expectation, stated per property:
//!
//! - **exact** where the command's result is exact geometry (straight segments: Centre Line of a converted loop, a
//!   polygonal brush erased or flattened with pencil lines, an envelope's projective image): the expected layer's
//!   snap-rounded map is built from the starting state and the command, and the product's zones must be its zone faces
//!   exactly — one zone per face, equal snapped areas, boundary strokes and canonical zone IDs (edge labels from the
//!   output strokes lying on each edge, as the round-1 oracle), and the fill the carry-over rule gives from the
//!   starting state's map;
//! - **two-stage** where the command fits curves (a brush outline, glyph outlines, a vectorized boundary, a smoothed
//!   path, a snap onto a cubic): (a) the output geometry must lie within a stated tolerance of the geometry the
//!   semantics predict from the starting state (pen half-width around the spine, the font's glyph contours, the mask's
//!   pixel boundary, the independently simplified polyline, the nearest point of the cubic), and the zone count and
//!   areas within the corresponding tolerance of the predicted faces; (b) the exact snap-rounded map of the output
//!   geometry, flattened independently, must equal the product's zones exactly (partition, areas, boundary strokes,
//!   canonical IDs), with the fills the carry-over rule gives from the starting state.
//!
//! Every property counts generated cases and dispatched commands separately (printed as `ROUTE_DOMAIN …`) and requires
//! the command under test to have run in at least half of the cases. Cases: `TT_ROUTE_CASES` (default 256 in `check
//! properties`; 4096 run locally).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout, missing_docs, reason = "test")]

mod common;
mod route_oracle;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use route_oracle::oracle::{self, Pt, Q};
use route_oracle::{Dp, F2, GRID, Input, SnapMap, overlap2, path_edges, polyline_dist, snapping_bound};
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:#1/element:\"Bob\"";

// ---------------------------------------------------------------------------------------------------------------
// Harness: seeded runner, dispatch counts.

thread_local! {
    static DISPATCHES: RefCell<BTreeMap<String, usize>> = const { RefCell::new(BTreeMap::new()) };
}

fn ok(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    DISPATCHES.with(|c| *c.borrow_mut().entry(id.to_owned()).or_default() += 1);
    common::ok(h, id, params)
}

fn cases() -> u32 {
    std::env::var("TT_ROUTE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(256)
}

/// Runs a seeded property; prints generated cases and dispatched commands, and requires each command of
/// `under_test` to have been dispatched in at least half of the cases.
fn property<S: Strategy>(
    name: &str,
    seed: &[u8; 32],
    under_test: &[&str],
    strategy: S,
    op: impl Fn(&S::Value) -> Result<(), TestCaseError>,
) where
    S::Value: std::fmt::Debug,
{
    DISPATCHES.with(|c| c.borrow_mut().clear());
    let generated = Cell::new(0u32);
    let mut runner = TestRunner::new_with_rng(
        Config { cases: cases(), failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, seed),
    );
    runner
        .run(&strategy, |v| {
            generated.set(generated.get() + 1);
            op(&v)
        })
        .unwrap();
    let d = DISPATCHES.with(|c| c.borrow().clone());
    println!("ROUTE_DOMAIN {name}: cases {} generated {} dispatched {d:?}", cases(), generated.get());
    for c in under_test {
        let n = d.get(*c).copied().unwrap_or(0);
        assert!(n * 2 >= cases() as usize, "{name}: {c} dispatched in {n} of {} cases", cases());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reading layers.

#[derive(Debug, Clone)]
struct Sk {
    id: String,
    kind: String,
    bez: Vec<F2>,
    closed: bool,
    outline: Vec<Vec<F2>>,
    thickness: Option<f64>,
    pen_max: Option<f64>,
}

#[derive(Debug, Clone)]
struct Zn {
    id: String,
    area: f64,
    boundary: Vec<String>,
    colour: Option<String>,
    interior: F2,
}

#[derive(Debug, Clone)]
struct Ls {
    strokes: Vec<Sk>,
    zones: Vec<Zn>,
}

fn pts(v: &Value) -> Vec<F2> {
    v.as_array().into_iter().flatten().map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap())).collect()
}

fn node_state(node: &Value) -> Ls {
    let strokes = node["strokes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| Sk {
            id: s["id"].as_str().unwrap().to_owned(),
            kind: s["kind"].as_str().unwrap().to_owned(),
            bez: pts(&s["bezier"]),
            closed: s["closed"] == true,
            outline: s["outline"].as_array().into_iter().flatten().map(pts).collect(),
            thickness: s["lineStyle"]["thickness"].as_f64(),
            pen_max: s["pen"]["maxSize"].as_f64(),
        })
        .collect();
    let zones = node["zones"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|z| Zn {
            id: z["id"].as_str().unwrap().to_owned(),
            area: z["area"].as_f64().unwrap(),
            boundary: z["boundaryStrokeIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect(),
            colour: z["colourId"].as_str().map(str::to_owned),
            interior: (z["interiorPoint"]["x"].as_f64().unwrap(), z["interiorPoint"]["y"].as_f64().unwrap()),
        })
        .collect();
    Ls { strokes, zones }
}

fn drawing(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({ "drawingId": d, "include": ["zones", "outline"] }))
}

fn find_node<'a>(nodes: &'a Value, id: &str) -> Option<&'a Value> {
    for n in nodes.as_array()? {
        if n["id"] == id {
            return Some(n);
        }
        if let Some(c) = find_node(&n["children"], id) {
            return Some(c);
        }
    }
    None
}

fn layer(h: &ExecutorHandle, d: &str, id: &str) -> Ls {
    let g = drawing(h, d);
    node_state(find_node(&g["nodes"], id).unwrap_or(&Value::Null))
}

/// The snap-rounded map inputs of a layer's strokes as persisted (brush strokes by their outline contours).
fn inputs(ls: &Ls) -> Vec<Input> {
    let mut out = Vec::new();
    for s in &ls.strokes {
        if s.kind == "brush" {
            for c in &s.outline {
                out.push(Input { id: s.id.clone(), brush: true, edges: path_edges(c, true) });
            }
        } else {
            out.push(Input { id: s.id.clone(), brush: false, edges: path_edges(&s.bez, s.closed) });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Exact comparison.

fn ring_pts(r: &[route_oracle::Ip]) -> Vec<Pt> {
    r.iter().map(|p| oracle::pt(p.0, p.1)).collect()
}

/// The canonical zone ID and boundary strokes of face `f` of `map`, its edges labelled by the strokes of `labels`
/// lying on them (`labels` is the product's output map; for an expected map the output strokes only name the edges).
fn id_and_boundary(map: &SnapMap, f: usize, labels: &SnapMap) -> (String, Vec<String>) {
    let rings = &map.faces[f].rings;
    let face = oracle::Face {
        outer: ring_pts(&rings[0]),
        holes: rings[1..].iter().map(|r| ring_pts(r)).collect(),
        area2: Q::int(0),
        probe: oracle::pt(0, 0),
    };
    let id = oracle::zone_id(&face, &|p: &Pt, q: &Pt| labels.label(oracle::snap_pt(p), oracle::snap_pt(q)));
    let mut b = BTreeSet::new();
    for r in rings {
        for k in 0..r.len() {
            for s in labels.srcs(r[k], r[(k + 1) % r.len()]) {
                b.insert(labels.inputs[s.input].id.clone());
            }
        }
    }
    (id, b.into_iter().collect())
}

/// The product's zones against the zone faces of `map`: one zone per zone face (its interior point strictly inside),
/// equal snapped areas, boundary strokes, canonical IDs and the expected fills. Returns the face of each zone.
fn verify(
    ls: &Ls,
    map: &SnapMap,
    labels: &SnapMap,
    fill: &dyn Fn(usize, &Zn) -> Option<String>,
    what: &str,
) -> Result<Vec<usize>, TestCaseError> {
    let zone_faces = map.zone_faces();
    prop_assert_eq!(
        ls.zones.len(),
        zone_faces.len(),
        "{}: zones {:?} vs {} oracle zone faces",
        what,
        ls.zones,
        zone_faces.len()
    );
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for z in &ls.zones {
        let f = map.locate(Dp::from_du(z.interior.0, z.interior.1));
        prop_assert!(f.is_some_and(|f| !map.faces[f].brush), "{}: zone {} interior point in no zone face", what, z.id);
        let f = f.unwrap();
        prop_assert!(seen.insert(f), "{}: two zones in face {}", what, f);
        prop_assert_eq!(z.area, map.faces[f].area2 as f64 / 2.0 / (GRID * GRID), "{}: zone {} area", what, z.id);
        let (id, boundary) = id_and_boundary(map, f, labels);
        prop_assert_eq!(&z.boundary, &boundary, "{}: zone {} boundary strokes", what, z.id);
        prop_assert_eq!(&z.id, &id, "{}: zone ID of face {}", what, f);
        prop_assert_eq!(&z.colour, &fill(f, z), "{}: fill of zone {}", what, z.id);
        out.push(f);
    }
    Ok(out)
}

/// The corners of a face's rings (vertices where the boundary turns), as a set.
fn corners(rings: &[Vec<route_oracle::Ip>]) -> BTreeSet<route_oracle::Ip> {
    let mut out = BTreeSet::new();
    for r in rings {
        let n = r.len();
        for k in 0..n {
            let (a, b, c) = (r[(k + n - 1) % n], r[k], r[(k + 1) % n]);
            let turn = i128::from(b.0 - a.0) * i128::from(c.1 - b.1) - i128::from(b.1 - a.1) * i128::from(c.0 - b.0);
            let onward = i128::from(b.0 - a.0) * i128::from(c.0 - b.0) + i128::from(b.1 - a.1) * i128::from(c.1 - b.1);
            if turn != 0 || onward <= 0 {
                out.insert(b);
            }
        }
    }
    out
}

/// The product's zones against the zone faces of an expected map `emap` (from the starting state and the command):
/// one zone per expected zone face, its interior point strictly inside, equal snapped areas, and the expected fill;
/// the product's own face at that point (`prod`, the output map) must be the same region (equal area, the same
/// corners), and the zone's boundary strokes and canonical ID are that face's (labels from the output strokes: where
/// the product cut its pieces along an expected edge is not predicted).
fn verify_expected(
    ls: &Ls,
    emap: &SnapMap,
    prod: &SnapMap,
    fill: &dyn Fn(usize, &Zn) -> Option<String>,
    what: &str,
) -> Result<(), TestCaseError> {
    let zone_faces = emap.zone_faces();
    prop_assert_eq!(
        ls.zones.len(),
        zone_faces.len(),
        "{}: zones {:?} vs {} expected zone faces",
        what,
        ls.zones,
        zone_faces.len()
    );
    let mut seen = BTreeSet::new();
    for z in &ls.zones {
        let at = Dp::from_du(z.interior.0, z.interior.1);
        let f = emap.locate(at);
        prop_assert!(
            f.is_some_and(|f| !emap.faces[f].brush),
            "{}: zone {} interior point in no expected zone face",
            what,
            z.id
        );
        let f = f.unwrap();
        prop_assert!(seen.insert(f), "{}: two zones in expected face {}", what, f);
        let g = prod.locate(at);
        prop_assert_eq!(
            z.area,
            emap.faces[f].area2 as f64 / 2.0 / (GRID * GRID),
            "{}: zone {} area; expected face corners {:?}, output face corners {:?}",
            what,
            z.id,
            corners(&emap.faces[f].rings),
            g.map(|g| corners(&prod.faces[g].rings))
        );
        prop_assert!(g.is_some(), "{}: zone {} in no face of the output map", what, z.id);
        let g = g.unwrap();
        prop_assert_eq!(prod.faces[g].area2, emap.faces[f].area2, "{}: output face of zone {}", what, z.id);
        prop_assert_eq!(
            corners(&prod.faces[g].rings),
            corners(&emap.faces[f].rings),
            "{}: output face corners of zone {}",
            what,
            z.id
        );
        prop_assert_eq!(&z.boundary, &prod.faces[g].boundary, "{}: zone {} boundary strokes", what, z.id);
        prop_assert_eq!(&z.id, &prod.faces[g].id, "{}: zone ID", what);
        prop_assert_eq!(&z.colour, &fill(f, z), "{}: fill of zone {}", what, z.id);
    }
    Ok(())
}

/// A layer before a command: its map (checked against its zones) and the fill of each zone face.
struct Old {
    map: SnapMap,
    fills: BTreeMap<usize, Option<String>>,
    ids: BTreeMap<String, usize>,
}

fn old_state(ls: &Ls) -> Result<Old, TestCaseError> {
    let map = SnapMap::build(inputs(ls));
    let faces = verify(ls, &map, &map, &|_, z| z.colour.clone(), "before")?;
    let fills = faces.iter().zip(&ls.zones).map(|(f, z)| (*f, z.colour.clone())).collect();
    let ids = faces.iter().zip(&ls.zones).map(|(f, z)| (z.id.clone(), *f)).collect();
    Ok(Old { map, fills, ids })
}

/// The carry-over rule (ADR-0017 history 5, C22-R2-04), recomputed: a zone whose ID survives keeps its fill; any
/// other takes the fill of the old zone it overlaps most (exact overlaps; overlaps within the snapping bound ignored;
/// ties to the lower old zone ID); none: the old zone holding its interior point.
fn carried(old: &Old, new: &SnapMap, f: usize, z: &Zn) -> Option<String> {
    if let Some(o) = old.ids.get(&z.id) {
        return old.fills[o].clone();
    }
    let nb = new.faces[f].bbox;
    let mut best: Option<(Q, usize)> = None;
    for &o in old.fills.keys() {
        let ob = old.map.faces[o].bbox;
        if ob[0] >= nb[2] || nb[0] >= ob[2] || ob[1] >= nb[3] || nb[1] >= ob[3] {
            continue;
        }
        let a2 = overlap2(&new.faces[f].rings, &old.map.faces[o].rings);
        if a2.clone().f64() / 2.0 <= snapping_bound(&new.faces[f].rings, &old.map.faces[o].rings) {
            continue;
        }
        let better = match &best {
            None => true,
            Some((b, bo)) => match a2.cmp_to(b) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => old.map.faces[o].id < old.map.faces[*bo].id,
                std::cmp::Ordering::Less => false,
            },
        };
        if better {
            best = Some((a2, o));
        }
    }
    match best {
        Some((_, o)) => old.fills[&o].clone(),
        None => {
            old.map.locate(Dp::from_du(z.interior.0, z.interior.1)).and_then(|o| old.fills.get(&o).cloned().flatten())
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Building states.

fn fresh() -> ExecutorHandle {
    // The product's font database is built once per process on first use: the fixture font is registered before any
    // handle exists (every property starts here), whichever property runs first.
    font();
    let h = common::fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{"op": "replace", "path": "/exposure", "value": [[null, 24]]}] }),
    );
    h
}

fn palette(h: &ExecutorHandle) -> Vec<String> {
    h.snapshot()
        .unwrap()
        .unwrap()
        .palettes()
        .flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned()))
        .collect()
}

/// A straight polyline as a flat Bézier list (handles on the anchors).
fn straight(points: &[F2], closed: bool) -> Vec<Value> {
    let mut bez = Vec::new();
    let n = points.len();
    for k in 0..n {
        let p = points[k];
        bez.push(json!({ "x": p.0, "y": p.1 }));
        if k + 1 < n || closed {
            let q = points[(k + 1) % n];
            bez.push(json!({ "x": p.0, "y": p.1 }));
            bez.push(json!({ "x": q.0, "y": q.1 }));
        }
    }
    bez
}

fn pencil(id: &str, points: &[F2], closed: bool) -> Value {
    json!({ "id": id, "kind": "pencil", "closed": closed, "bezier": straight(points, closed), "colourId": null,
            "lineStyle": { "thickness": 1, "lineType": "solid" } })
}

/// A spineless brush stroke whose outline is straight closed contours.
fn brush(id: &str, contours: &[Vec<F2>], colour: &Value) -> Value {
    let outline: Vec<Value> = contours
        .iter()
        .map(|c| {
            let mut v = straight(c, true);
            v.truncate(3 * c.len());
            Value::Array(v)
        })
        .collect();
    json!({ "id": id, "kind": "brush", "closed": true, "colourId": colour, "outline": outline })
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<F2> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A new drawing with the given layers; returns the drawing and the layer IDs in order.
fn create(h: &ExecutorHandle, layers: Vec<Value>) -> (String, Vec<String>) {
    let ids: Vec<String> = layers.iter().map(|l| l["id"].as_str().unwrap().to_owned()).collect();
    let r = ok(
        h,
        "draw.content.add",
        json!({ "elementId": BOB, "frame": 1, "createDrawing": true, "select": false, "content": { "nodes": layers } }),
    );
    let d = r["drawingId"].as_str().unwrap().to_owned();
    let mapped = ids.iter().map(|i| r["idMap"][i].as_str().unwrap_or(i).to_owned()).collect();
    (d, mapped)
}

fn layer_json(id: &str, kind: &str, strokes: Vec<Value>) -> Value {
    json!({ "type": "layer", "id": id, "layerKind": kind, "strokes": strokes })
}

/// Paints every zone of a layer a palette colour; returns the painted state.
fn paint(h: &ExecutorHandle, d: &str, id: &str) -> Ls {
    let ls = layer(h, d, id);
    let cs = palette(h);
    for (i, z) in ls.zones.iter().enumerate() {
        ok(h, "colour.paint.apply", json!({ "drawingId": d, "objectIds": [z.id], "colourId": cs[i % cs.len()] }));
    }
    layer(h, d, id)
}

// ---------------------------------------------------------------------------------------------------------------
// Geometry helpers (du).

fn area(poly: &[F2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].0 * poly[(i + 1) % n].1 - poly[(i + 1) % n].0 * poly[i].1).sum::<f64>() / 2.0
}

fn perimeter(poly: &[F2], closed: bool) -> f64 {
    let n = poly.len();
    let m = if closed { n } else { n.saturating_sub(1) };
    (0..m).map(|i| (poly[(i + 1) % n].0 - poly[i].0).hypot(poly[(i + 1) % n].1 - poly[i].1)).sum()
}

/// The one-sided distance: the farthest point of `a` (densified) from `b`.
fn reach(a: &[F2], a_closed: bool, b: &[F2], b_closed: bool) -> f64 {
    route_oracle::densify(a, a_closed, 0.25).iter().map(|p| polyline_dist(*p, b, b_closed)).fold(0.0, f64::max)
}

fn hausdorff(a: &[F2], a_closed: bool, b: &[F2], b_closed: bool) -> f64 {
    reach(a, a_closed, b, b_closed).max(reach(b, b_closed, a, a_closed))
}

/// A stroke's path flattened (du).
fn flat(s: &Sk) -> (Vec<F2>, bool) {
    route_oracle::flatten(&s.bez, s.closed)
}

// ---------------------------------------------------------------------------------------------------------------
// 1. Lines to Brush, then Centre Line, on closed loops.

#[test]
fn lines_to_brush_then_centre_line_on_closed_loops() {
    // One or two lattice rectangles (disjoint, or one inside the other with a margin above the pen width), every
    // zone painted; Lines to Brush converts a random non-empty subset of the loops.
    // (a) Each converted loop becomes one brush stroke whose spine is the loop and whose outline lies within the pen's
    //     half maximum size of it and encloses it (non-zero winding around points of the loop), and invisible centre
    //     pieces covering the loop exactly once; unconverted loops are unchanged pencil lines. Zones: one per
    //     rectangle face, its area within [face − perimeter × half-width, face]; fills kept (carry-over). Exact stage
    //     on the output map.
    // Centre Line on the brushes then gives, exactly: a pencil line along each spine plus the retained invisible
    // centre (each loop edge covered twice), no brush, the rectangles' faces with the original fills.
    let rects = (0i64..3, 0i64..3, 2i64..5, 2i64..5, any::<bool>(), 0u8..4);
    property(
        "lines_to_brush_then_centre_line_on_closed_loops",
        b"c24-r2-domain-l2b-loop----------",
        &["draw.linesToBrush.apply", "draw.centreLine.apply"],
        rects,
        |&(x, y, w, ht, nested, pick)| {
            let s = 20.0;
            let outer = rect(x as f64 * s, y as f64 * s, (x + w) as f64 * s, (y + ht) as f64 * s);
            let mut loops = vec![outer.clone()];
            if nested && w >= 3 && ht >= 3 {
                loops.push(rect(
                    (x as f64 + 1.0) * s,
                    (y as f64 + 1.0) * s,
                    (x + w - 1) as f64 * s,
                    (y + ht - 1) as f64 * s,
                ));
            } else {
                loops.push(rect((x + w + 1) as f64 * s, y as f64 * s, (x + w + 3) as f64 * s, (y + 2) as f64 * s));
            }
            let h = fresh();
            let strokes: Vec<Value> =
                loops.iter().enumerate().map(|(i, l)| pencil(&format!("lp{i}"), l, true)).collect();
            let (d, ids) = create(&h, vec![layer_json("l_a", "pencil", strokes)]);
            let la = &ids[0];
            let before = paint(&h, &d, la);
            let old = old_state(&before)?;
            let chosen: Vec<usize> = match pick % 3 {
                0 => vec![0],
                1 => vec![1],
                _ => vec![0, 1],
            };
            let by_loop: Vec<String> = loops
                .iter()
                .map(|l| before.strokes.iter().find(|s| s.bez.first() == Some(&l[0])).unwrap().id.clone())
                .collect();
            let conv: Vec<&String> = chosen.iter().map(|&i| &by_loop[i]).collect();
            let r = ok(&h, "draw.linesToBrush.apply", json!({ "drawingId": d, "ids": conv }));
            let brushes: Vec<String> =
                r["strokeIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
            prop_assert_eq!(brushes.len(), chosen.len());
            let after = layer(&h, &d, la);
            // (a) semantic geometry.
            let mut reach_max: f64 = 0.0;
            for &i in &chosen {
                let l = &loops[i];
                let b = after
                    .strokes
                    .iter()
                    .find(|s| s.kind == "brush" && brushes.contains(&s.id) && s.bez.iter().any(|p| *p == l[1]))
                    .ok_or_else(|| TestCaseError::fail(format!("no brush along loop {i}")))?;
                let spine: BTreeSet<(i64, i64)> =
                    b.bez.iter().step_by(3).map(|p| ((p.0 * 64.0) as i64, (p.1 * 64.0) as i64)).collect();
                let want: BTreeSet<(i64, i64)> = l.iter().map(|p| ((p.0 * 64.0) as i64, (p.1 * 64.0) as i64)).collect();
                prop_assert_eq!(&spine, &want, "brush spine of loop {}", i);
                let half = b.pen_max.unwrap_or(f64::INFINITY) / 2.0;
                prop_assert!(half.is_finite() && half > 0.0);
                reach_max = reach_max.max(half);
                // The outline is fitted within min(0.1 du, pen width / 16) of the swept tips (02 §3.4, ADR-0016).
                let fit = (0.1f64).min(2.0 * half / 16.0).max(1.0 / 64.0);
                for c in &b.outline {
                    let (cf, _) = route_oracle::flatten(c, true);
                    let far = reach(&cf, true, l, true);
                    prop_assert!(
                        far <= half + fit + 1.0 / 64.0,
                        "outline of loop {} reaches {} du from its spine (half size {})",
                        i,
                        far,
                        half
                    );
                }
                // The outline encloses the loop: points of the loop have non-zero winding.
                let edges: Vec<(route_oracle::Ip, route_oracle::Ip)> =
                    b.outline.iter().flat_map(|c| path_edges(c, true)).collect();
                for p in route_oracle::densify(l, true, 5.0) {
                    let w = route_oracle::directed_winding(
                        &edges,
                        Dp::from_du(
                            (p.0 * 64.0).round() / 64.0 + 1.0 / 4096.0,
                            (p.1 * 64.0).round() / 64.0 + 1.0 / 8192.0,
                        ),
                    );
                    prop_assert!(w != 0, "loop {} point {:?} outside its brush", i, p);
                }
                // Invisible centre pieces cover the loop exactly once.
                let mut covered = 0.0;
                for s in after.strokes.iter().filter(|s| s.kind == "invisible") {
                    let (f, c) = flat(s);
                    if reach(&f, c, l, true) <= 1.0 / 64.0 {
                        covered += perimeter(&f, c);
                    }
                }
                prop_assert!(
                    (covered - perimeter(l, true)).abs() < 1e-6,
                    "invisible centre of loop {}: {} of {}",
                    i,
                    covered,
                    perimeter(l, true)
                );
            }
            for (i, l) in loops.iter().enumerate().filter(|(i, _)| !chosen.contains(i)) {
                prop_assert!(
                    after.strokes.iter().any(|s| s.kind == "pencil"
                        && s.closed
                        && s.bez.iter().step_by(3).count() == 4
                        && s.bez[0] == l[0]),
                    "loop {} unchanged",
                    i
                );
            }
            // Zones: one per rectangle face, areas within the band.
            let faces: Vec<f64> = if nested && w >= 3 && ht >= 3 {
                vec![area(&loops[0]) - area(&loops[1]), area(&loops[1])]
            } else {
                vec![area(&loops[0]), area(&loops[1])]
            };
            prop_assert_eq!(after.zones.len(), 2, "zones after Lines to Brush: {:?}", after.zones);
            let mut got: Vec<f64> = after.zones.iter().map(|z| z.area).collect();
            got.sort_by(f64::total_cmp);
            let mut want = faces.clone();
            want.sort_by(f64::total_cmp);
            // Each converted loop's brush covers up to its half width plus the fit tolerance on both sides of the loop,
            // and a round tip a quarter disc beyond each convex corner.
            let fit = (0.1f64).min(2.0 * reach_max / 16.0).max(1.0 / 64.0);
            let band = (reach_max + fit + 1.0 / 64.0) * (perimeter(&loops[0], true) + perimeter(&loops[1], true))
                + 8.0 * std::f64::consts::PI * (reach_max + fit).powi(2)
                + 0.5;
            for (g, wv) in got.iter().zip(&want) {
                prop_assert!(*g <= wv + 0.5 && *g >= wv - band, "zone area {} vs face {} (band {})", g, wv, band);
            }
            // (b) exact stage on the output.
            let map = SnapMap::build(inputs(&after));
            verify(&after, &map, &map, &|f, z| carried(&old, &map, f, z), "after Lines to Brush")?;
            // Centre Line: exact.
            let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": d, "ids": brushes }));
            prop_assert!(r["skippedIds"].as_array().unwrap().is_empty(), "{}", r);
            let fin = layer(&h, &d, la);
            prop_assert!(fin.strokes.iter().all(|s| s.kind != "brush"));
            for (i, l) in loops.iter().enumerate() {
                for kind in ["pencil", "invisible"] {
                    let want = if kind == "invisible" && !chosen.contains(&i) { 0.0 } else { perimeter(l, true) };
                    let mut covered = 0.0;
                    for s in fin.strokes.iter().filter(|s| s.kind == kind) {
                        let (f, c) = flat(s);
                        if reach(&f, c, l, true) <= 1.0 / 64.0 {
                            covered += perimeter(&f, c);
                        }
                    }
                    prop_assert!(
                        (covered - want).abs() < 1e-6,
                        "{} cover of loop {}: {} of {}",
                        kind,
                        i,
                        covered,
                        want
                    );
                }
            }
            let mut expected: Vec<Input> = Vec::new();
            for (i, l) in loops.iter().enumerate() {
                expected.push(Input { id: format!("p{i}"), brush: false, edges: path_edges(&loop_bez(l), true) });
                if chosen.contains(&i) {
                    expected.push(Input { id: format!("c{i}"), brush: false, edges: path_edges(&loop_bez(l), true) });
                }
            }
            let emap = SnapMap::build(expected);
            let labels = SnapMap::build(inputs(&fin));
            verify_expected(&fin, &emap, &labels, &|f, z| carried(&old, &emap, f, z), "after Centre Line")?;
            Ok(())
        },
    );
}

/// A closed polygon as a flat Bézier point list (du).
fn loop_bez(l: &[F2]) -> Vec<F2> {
    let n = l.len();
    let mut v = Vec::new();
    for k in 0..n {
        v.push(l[k]);
        v.push(l[k]);
        v.push(l[(k + 1) % n]);
    }
    v
}

// ---------------------------------------------------------------------------------------------------------------
// 2. Centre Line of a spineless outline.

#[test]
fn centre_line_of_a_spineless_ring() {
    // A spineless brush whose outline is a rectangular ring (outer rectangle, inner rectangle inset by the band b, one
    // contour joined by a zero-width bridge, SI-053),
    // with the hole zone painted. Exact before: the hole is the one zone (brush occupancy). Centre Line (medial
    // axis, 02 §8.5; holes become closed lines): (a) pencil lines within b/2 of the ring's mid-rectangle (inset b/2)
    // and reaching every part of it within b/2, of width within 1 du of b; no brush left; one zone, inside the centre
    // loop, of area within the mid-rectangle's perimeter × b/2 of the mid-rectangle's area, with the hole's fill;
    // (b) exact stage on the output.
    property(
        "centre_line_of_a_spineless_ring",
        b"c24-r2-domain-spineless---------",
        &["draw.centreLine.apply"],
        (0i64..4, 0i64..4, 3i64..7, 3i64..7, 2i64..6),
        |&(x, y, w, ht, band)| {
            let (s, b) = (10.0, band as f64 * 2.0);
            let (x0, y0, x1, y1) =
                (x as f64 * s, y as f64 * s, (x + w) as f64 * s + 2.0 * b, (y + ht) as f64 * s + 2.0 * b);
            // One closed contour (SI-053): the outer rectangle, a zero-width bridge to the inner one, the inner one the
            // other way round, the bridge back.
            let (ox, ix) = (rect(x0, y0, x1, y1), rect(x0 + b, y0 + b, x1 - b, y1 - b));
            let ring = vec![ox[0], ox[1], ox[2], ox[3], ox[0], ix[0], ix[3], ix[2], ix[1], ix[0]];
            let h = fresh();
            let c = json!(palette(&h)[2]);
            let (d, ids) =
                create(&h, vec![layer_json("l_b", "brush", vec![brush("ring", std::slice::from_ref(&ring), &c)])]);
            let lb = &ids[0];
            let before = paint(&h, &d, lb);
            let old = old_state(&before)?;
            let hole = rect(x0 + b, y0 + b, x1 - b, y1 - b);
            let ebefore = SnapMap::build(vec![Input {
                id: before.strokes[0].id.clone(),
                brush: true,
                edges: path_edges(&loop_bez(&ring), true),
            }]);
            verify_expected(&before, &ebefore, &old.map, &|_, z| z.colour.clone(), "before (expected)")?;
            prop_assert_eq!(before.zones.len(), 1);
            prop_assert_eq!(before.zones[0].area, area(&hole));
            let r = ok(&h, "draw.centreLine.apply", json!({ "drawingId": d, "ids": [before.strokes[0].id] }));
            prop_assert!(r["skippedIds"].as_array().unwrap().is_empty(), "{}", r);
            let after = layer(&h, &d, lb);
            prop_assert!(after.strokes.iter().all(|s| s.kind != "brush"), "brush left: {:?}", after.strokes);
            let mid = rect(x0 + b / 2.0, y0 + b / 2.0, x1 - b / 2.0, y1 - b / 2.0);
            let lines: Vec<(Vec<F2>, bool)> = after.strokes.iter().filter(|s| s.kind == "pencil").map(flat).collect();
            prop_assert!(!lines.is_empty());
            for (l, c) in &lines {
                let far = reach(l, *c, &mid, true);
                prop_assert!(far <= b / 2.0 + 1.0 / 64.0, "centre line {} du from the mid-rectangle (band {})", far, b);
            }
            let all: Vec<F2> = lines.iter().flat_map(|(l, c)| route_oracle::densify(l, *c, 0.25)).collect();
            for p in route_oracle::densify(&mid, true, 1.0) {
                let near = all.iter().map(|q| (q.0 - p.0).hypot(q.1 - p.1)).fold(f64::INFINITY, f64::min);
                prop_assert!(near <= b / 2.0 + 0.25, "mid-rectangle point {:?} {} du from every centre line", p, near);
            }
            for s in after.strokes.iter().filter(|s| s.kind == "pencil") {
                let t = s.thickness.unwrap_or(0.0);
                prop_assert!((t - b).abs() <= 1.0, "centre line width {} for band {}", t, b);
            }
            prop_assert_eq!(after.zones.len(), 1, "zones {:?}", after.zones);
            let tol = perimeter(&mid, true) * b / 2.0;
            prop_assert!(
                (after.zones[0].area - area(&mid)).abs() <= tol,
                "zone area {} vs mid-rectangle {}",
                after.zones[0].area,
                area(&mid)
            );
            prop_assert_eq!(&after.zones[0].colour, &before.zones[0].colour);
            let map = SnapMap::build(inputs(&after));
            verify(&after, &map, &map, &|f, z| carried(&old, &map, f, z), "after Centre Line")?;
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 3. Break Apart against the fixture font.

fn font() -> &'static route_oracle::ttf::Font {
    static FONT: std::sync::OnceLock<route_oracle::ttf::Font> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fonts/B612-Regular.ttf"),
        )
        .unwrap();
        tt_vector::fonts::register_test_font(bytes.clone());
        route_oracle::ttf::Font::parse(bytes).unwrap()
    })
}

#[test]
fn break_apart_matches_the_fonts_glyph_outlines() {
    // One glyph of the fixture font (letters with zero, one or two counters), size and position random. The glyph's
    // contours are read from the font file by the independent reader and scaled by size / units-per-em; the product's
    // glyph outline is compared after aligning the two bounding boxes' lower-left corners (the layout's origin is
    // not predicted; the scale is: both boxes' sizes must agree). (a) As many outline contours as font contours, each
    // within 1/16 du of one of them (Hausdorff); one zone per counter (a contour wound opposite to the outer ones),
    // each of the counter's area within its perimeter × 1/16 du, bounded by the glyph's brush stroke and unpainted;
    // (b) exact stage on the output.
    let letters = ['B', 'O', '8', 'A', 'D', 'P', 'R', 'H', 'L', 'T', 'o', 'e', 'b', 'q', '0', '4'];
    property(
        "break_apart_matches_the_fonts_glyph_outlines",
        b"c24-r2-domain-breakap-----------",
        &["draw.breakApart.apply"],
        (0usize..letters.len(), 30i64..120, -5i64..5, -5i64..5),
        |&(k, size, px, py)| {
            let f = font();
            let ch = letters[k];
            let g = f.glyph_index(ch).ok_or_else(|| TestCaseError::fail(format!("no glyph for {ch}")))?;
            let Some(contours) = f.contours(g, 16) else { return Ok(()) };
            let scale = size as f64 / f.units_per_em;
            let pred: Vec<Vec<F2>> =
                contours.iter().map(|c| c.iter().map(|p| (p.0 * scale, p.1 * scale)).collect()).collect();
            let h = fresh();
            let (d, _) = create(
                &h,
                vec![layer_json("l_t", "pencil", vec![pencil("x", &[(500.0, 500.0), (520.0, 500.0)], false)])],
            );
            let t = ok(
                &h,
                "draw.text.add",
                json!({ "drawingId": d, "position": {"x": px * 20, "y": py * 20}, "text": ch.to_string(), "format": {"font": "B612", "size": size} }),
            );
            let r = ok(&h, "draw.breakApart.apply", json!({ "drawingId": d, "textIds": [t["textId"]] }));
            let groups = r["groupIds"].as_array().unwrap();
            prop_assert_eq!(groups.len(), 1);
            let g = drawing(&h, &d);
            let group = find_node(&g["nodes"], groups[0].as_str().unwrap()).unwrap();
            let gl = node_state(&group["children"][0]);
            prop_assert_eq!(gl.strokes.len(), 1, "glyph strokes");
            let st = &gl.strokes[0];
            prop_assert_eq!(st.kind.as_str(), "brush");
            // The outline is one contour joined by zero-width bridges (SI-053): edges traversed both ways cancel; the
            // remaining edges form the glyph's contours.
            prop_assert_eq!(st.outline.len(), 1);
            let (poly, _) = route_oracle::flatten(&st.outline[0], true);
            let out = unbridged(&poly);
            prop_assert_eq!(out.len(), pred.len(), "contours of {}", ch);
            let bb = |cs: &[Vec<F2>]| {
                cs.iter()
                    .flatten()
                    .fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
                        (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
                    })
            };
            let (pb, ob) = (bb(&pred), bb(&out));
            let tol = 1.0 / 16.0;
            prop_assert!(
                ((pb.2 - pb.0) - (ob.2 - ob.0)).abs() <= tol && ((pb.3 - pb.1) - (ob.3 - ob.1)).abs() <= tol,
                "glyph {} box {:?} vs {:?}",
                ch,
                pb,
                ob
            );
            let (dx, dy) = (ob.0 - pb.0, ob.1 - pb.1);
            let moved: Vec<Vec<F2>> = pred.iter().map(|c| c.iter().map(|p| (p.0 + dx, p.1 + dy)).collect()).collect();
            let mut used = BTreeSet::new();
            for o in &out {
                let m = (0..moved.len())
                    .filter(|i| !used.contains(i))
                    .find(|&i| hausdorff(o, true, &moved[i], true) <= tol);
                prop_assert!(m.is_some(), "an outline contour of {} matches no font contour", ch);
                used.insert(m.unwrap());
            }
            let outer_sign = moved.iter().map(|c| area(c)).max_by(|a, b| a.abs().total_cmp(&b.abs())).unwrap().signum();
            let counters: Vec<&Vec<F2>> = moved.iter().filter(|c| area(c).signum() != outer_sign).collect();
            prop_assert_eq!(gl.zones.len(), counters.len(), "zones of {}", ch);
            for z in &gl.zones {
                prop_assert_eq!(&z.boundary, &vec![st.id.clone()]);
                prop_assert!(z.colour.is_none());
                let ok_area = counters.iter().any(|c| (z.area - area(c).abs()).abs() <= perimeter(c, true) * tol);
                prop_assert!(ok_area, "zone area {} of {} matches no counter", z.area, ch);
            }
            let map = SnapMap::build(inputs(&gl));
            verify(&gl, &map, &map, &|_, _| None, "glyph")?;
            Ok(())
        },
    );
}

/// The contours of a closed polygon joined by zero-width bridges: edges traversed both ways removed, the rest chained
/// into open polylines (each a contour, closed back to its start).
fn unbridged(poly: &[F2]) -> Vec<Vec<F2>> {
    let n = poly.len();
    let key = |p: F2| ((p.0 * 4096.0).round() as i64, (p.1 * 4096.0).round() as i64);
    let mut edges: Vec<(F2, F2)> =
        (0..n).map(|i| (poly[i], poly[(i + 1) % n])).filter(|(a, b)| key(*a) != key(*b)).collect();
    let mut i = 0;
    while i < edges.len() {
        let (a, b) = edges[i];
        if let Some(j) = edges.iter().position(|&(c, d)| key(c) == key(b) && key(d) == key(a)) {
            let (hi, lo) = (i.max(j), i.min(j));
            edges.remove(hi);
            edges.remove(lo);
            i = 0;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    while let Some((a, b)) = edges.first().copied() {
        edges.remove(0);
        let mut line = vec![a, b];
        while let Some(j) = edges.iter().position(|&(c, _)| key(c) == key(*line.last().unwrap())) {
            line.push(edges.remove(j).1);
        }
        if key(*line.last().unwrap()) == key(line[0]) {
            line.pop();
        }
        out.push(line);
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// 4. Vectorize small masks.

/// Paper components of a mask enclosed by ink (4-connected paper, not touching the border).
fn enclosed_paper(mask: &[Vec<bool>]) -> Vec<Vec<(usize, usize)>> {
    let (hgt, wid) = (mask.len(), mask[0].len());
    let mut seen = vec![vec![false; wid]; hgt];
    let mut out = Vec::new();
    for y in 0..hgt {
        for x in 0..wid {
            if mask[y][x] || seen[y][x] {
                continue;
            }
            let mut comp = Vec::new();
            let mut stack = vec![(x, y)];
            seen[y][x] = true;
            let mut border = false;
            while let Some((cx, cy)) = stack.pop() {
                comp.push((cx, cy));
                if cx == 0 || cy == 0 || cx + 1 == wid || cy + 1 == hgt {
                    border = true;
                }
                let n = [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1)];
                for (nx, ny) in n {
                    if nx < wid && ny < hgt && !mask[ny][nx] && !seen[ny][nx] {
                        seen[ny][nx] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if !border {
                out.push(comp);
            }
        }
    }
    out
}

/// Whether two cells of the same kind touch only diagonally anywhere (4- and 8-connectivity would differ there).
fn diagonal_only(mask: &[Vec<bool>]) -> bool {
    for y in 0..mask.len() - 1 {
        for x in 0..mask[0].len() - 1 {
            let (a, b, c, d) = (mask[y][x], mask[y][x + 1], mask[y + 1][x], mask[y + 1][x + 1]);
            if a == d && b == c && a != b {
                return true;
            }
        }
    }
    false
}

#[test]
fn vectorize_masks_enclose_the_predicted_paper_components() {
    // Random masks of up to 7 × 7 pixels with a one-pixel ink frame, no diagonal-only contact. Predicted from the
    // mask: the paper components enclosed by ink; the drawing's pixel scale and origin are fitted from the ink's
    // bounding box (the frame), and must be the same on both axes. (a) One unpainted zone per enclosed component, its
    // interior point inside that component's pixels (a quarter pixel dilated), its area within the component's pixel
    // perimeter × half a pixel of its pixel area (the fit rounds pixel corners); (b) exact stage on the output.
    property(
        "vectorize_masks_enclose_the_predicted_paper_components",
        b"c24-r2-domain-vectori-----------",
        &["draw.vectorize.run"],
        (4usize..8, 4usize..8, any::<u64>()),
        |&(wid, hgt, bits)| {
            // Redrawn from rotated bits while two cells touch only diagonally (then 4- and 8-connectivity differ).
            let draw = |bits: u64| {
                let mut mask = vec![vec![true; wid]; hgt];
                for (y, row) in mask.iter_mut().enumerate().take(hgt - 1).skip(1) {
                    for (x, cell) in row.iter_mut().enumerate().take(wid - 1).skip(1) {
                        *cell = (bits >> ((y * 8 + x) % 64)) & 1 == 1;
                    }
                }
                mask
            };
            let Some(mask) = (0..64)
                .map(|k| draw(bits.rotate_left(k * 7) ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
                .find(|m| !diagonal_only(m))
            else {
                return Ok(());
            };
            let comps = enclosed_paper(&mask);
            let h = fresh();
            let dir = std::env::temp_dir().join(format!("tt-c24-r2-domain-{}", tt_persist::fsx::random_token(8)));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("mask.png");
            let rgba: Vec<u8> = mask
                .iter()
                .flatten()
                .flat_map(|&ink| if ink { [0, 0, 0, 255] } else { [255, 255, 255, 255] })
                .collect();
            std::fs::write(&file, tt_render::encode_png(&rgba, wid as u32, hgt as u32).unwrap()).unwrap();
            let start = ok(
                &h,
                "draw.vectorize.run",
                json!({ "files": [file.to_string_lossy()], "targetElementId": BOB, "startFrame": 1, "onConflict": "replace" }),
            );
            let rec = common::ok(&h, "job.wait", json!({ "jobId": start["jobId"], "timeoutMs": 60000 }));
            std::fs::remove_file(&file).unwrap();
            std::fs::remove_dir(&dir).unwrap();
            let rec = rec.get("record").unwrap_or(&rec).clone();
            prop_assert_eq!(&rec["state"], "succeeded", "{}", rec);
            let d = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
            let g = drawing(&h, &d);
            let ls = node_state(&g["nodes"][0]);
            prop_assert!(!ls.strokes.is_empty());
            let outline: Vec<F2> = ls
                .strokes
                .iter()
                .flat_map(|s| s.outline.iter().flat_map(|c| route_oracle::flatten(c, true).0))
                .collect();
            let (x0, y0, x1, y1) =
                outline.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
                    (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
                });
            let (sx, sy) = ((x1 - x0) / wid as f64, (y1 - y0) / hgt as f64);
            prop_assert!((sx - sy).abs() <= 0.02 * sx.max(sy), "pixel scale {} vs {}", sx, sy);
            let px = sx.max(sy);
            // Pixel (cx, cy) (row 0 at the top) covers [x0 + cx px, x0 + (cx + 1) px] × [y1 − (cy + 1) px, y1 − cy px]
            // (y up), or with rows the other way: try both orientations and keep the one the frame fits.
            prop_assert_eq!(ls.zones.len(), comps.len(), "zones {:?} for components {:?}", ls.zones, comps);
            let inside = |z: &Zn, comp: &[(usize, usize)], flip: bool| {
                comp.iter().any(|&(cx, cy)| {
                    let row = if flip { hgt - 1 - cy } else { cy };
                    let (ax, ay) = (x0 + cx as f64 * px, y0 + row as f64 * px);
                    z.interior.0 >= ax - px / 4.0
                        && z.interior.0 <= ax + px * 1.25
                        && z.interior.1 >= ay - px / 4.0
                        && z.interior.1 <= ay + px * 1.25
                })
            };
            let pixel_perimeter = |comp: &[(usize, usize)]| {
                let set: BTreeSet<(usize, usize)> = comp.iter().copied().collect();
                comp.iter()
                    .map(|&(x, y)| {
                        [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)]
                            .iter()
                            .filter(|n| !set.contains(n))
                            .count()
                    })
                    .sum::<usize>() as f64
            };
            let fits = |flip: bool| {
                let mut used = BTreeSet::new();
                ls.zones.iter().all(|z| {
                    let m = (0..comps.len()).filter(|i| !used.contains(i)).find(|&i| {
                        inside(z, &comps[i], flip)
                            && (z.area - comps[i].len() as f64 * px * px).abs()
                                <= pixel_perimeter(&comps[i]) * px * px / 2.0
                    });
                    m.is_some_and(|i| used.insert(i))
                })
            };
            prop_assert!(
                fits(false) || fits(true),
                "zones {:?} do not match the components {:?} (pixel {} du)",
                ls.zones,
                comps,
                px
            );
            for z in &ls.zones {
                prop_assert!(z.colour.is_none());
            }
            let map = SnapMap::build(inputs(&ls));
            verify(&ls, &map, &map, &|_, _| None, "vectorized")?;
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Smooth with a changed shape.

/// Douglas–Peucker on a closed polygon (fixed first vertex and the vertex farthest from it), within `tol`.
fn simplify_closed(p: &[F2], tol: f64) -> Vec<F2> {
    fn dp(p: &[F2], a: usize, b: usize, tol: f64, keep: &mut Vec<bool>) {
        let mut best = (0.0, 0);
        for i in a + 1..b {
            let d = route_oracle::seg_dist(p[i], p[a], p[b % p.len()]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            dp(p, a, best.1, tol, keep);
            dp(p, best.1, b, tol, keep);
        }
    }
    let n = p.len();
    let far = (1..n).max_by(|&i, &j| {
        let (di, dj) = ((p[i].0 - p[0].0).hypot(p[i].1 - p[0].1), (p[j].0 - p[0].0).hypot(p[j].1 - p[0].1));
        di.total_cmp(&dj).then(j.cmp(&i))
    });
    let far = far.unwrap_or(0);
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[far] = true;
    dp(p, 0, far, tol, &mut keep);
    dp(p, far, n, tol, &mut keep);
    (0..n).filter(|&i| keep[i]).map(|i| p[i]).collect()
}

#[test]
fn smooth_changes_the_shape_within_its_tolerance() {
    // A closed lattice rectangle with small bumps (1 to 3 du, out or in) on its sides, its zone painted; Smooth with a
    // tolerance of 5 du. The oracle simplifies the polygon independently (Douglas–Peucker within the tolerance): the
    // rectangle. (a) Anchors were removed and the shape changed; the result lies within the tolerance of the original (both ways, 02 §8.3) and within tolerance + bump of the simplified
    // polyline; one zone, its area within the simplified polyline's perimeter × (tolerance + bump) of its area,
    // keeping the fill; (b) exact stage on the output.
    property(
        "smooth_changes_the_shape_within_its_tolerance",
        b"c24-r2-domain-smoothx-----------",
        &["draw.smooth.apply"],
        (0i64..3, 0i64..3, 2i64..5, 2i64..5, prop::collection::vec((0usize..4, 1i64..4, any::<bool>()), 1..4)),
        |(x, y, w, ht, bumps)| {
            let s = 20.0;
            let (x0, y0, x1, y1) = (*x as f64 * s, *y as f64 * s, (x + w) as f64 * s, (y + ht) as f64 * s);
            let corners = rect(x0, y0, x1, y1);
            let mut sides: Vec<Vec<F2>> = (0..4).map(|_| Vec::new()).collect();
            let mut used = BTreeSet::new();
            let mut hb_max: f64 = 0.0;
            for &(side, hb, out) in bumps {
                if !used.insert(side) {
                    continue;
                }
                let (a, b) = (corners[side], corners[(side + 1) % 4]);
                let (dx, dy) = ((b.0 - a.0) / (b.0 - a.0).hypot(b.1 - a.1), (b.1 - a.1) / (b.0 - a.0).hypot(b.1 - a.1));
                let normal = if out { (dy, -dx) } else { (-dy, dx) };
                let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let hb = hb as f64;
                hb_max = hb_max.max(hb);
                sides[side] = vec![
                    (m.0 - 5.0 * dx, m.1 - 5.0 * dy),
                    (m.0 + hb * normal.0, m.1 + hb * normal.1),
                    (m.0 + 5.0 * dx, m.1 + 5.0 * dy),
                ];
            }
            let mut poly = Vec::new();
            for i in 0..4 {
                poly.push(corners[i]);
                poly.extend(sides[i].iter().copied());
            }
            let tol = 5.0;
            let simple = simplify_closed(&poly, tol);
            prop_assert_eq!(simple.len(), 4, "the simplified polyline of {:?}", poly);
            let h = fresh();
            let (d, ids) = create(&h, vec![layer_json("l_s", "pencil", vec![pencil("sm", &poly, true)])]);
            let la = &ids[0];
            let before = paint(&h, &d, la);
            let old = old_state(&before)?;
            let r =
                ok(&h, "draw.smooth.apply", json!({ "drawingId": d, "ids": [before.strokes[0].id], "tolerance": tol }));
            prop_assert!(r["removedPointCount"].as_u64().unwrap() > 0, "{}", r);
            let after = layer(&h, &d, la);
            prop_assert_eq!(after.strokes.len(), 1);
            let st = &after.strokes[0];
            let (res, closed) = flat(st);
            prop_assert!(closed);
            prop_assert!(st.bez.len() < before.strokes[0].bez.len(), "the shape changed");
            let dev = hausdorff(&res, true, &poly, true);
            prop_assert!(dev <= tol + 1.0 / 64.0, "deviation {} from the original", dev);
            let dev2 = hausdorff(&res, true, &simple, true);
            prop_assert!(dev2 <= tol + hb_max + 1.0 / 64.0, "deviation {} from the simplified polyline", dev2);
            prop_assert_eq!(after.zones.len(), 1);
            let band = perimeter(&simple, true) * (tol + hb_max);
            prop_assert!(
                (after.zones[0].area - area(&simple).abs()).abs() <= band,
                "zone area {} vs {}",
                after.zones[0].area,
                area(&simple)
            );
            prop_assert_eq!(&after.zones[0].colour, &before.zones[0].colour);
            let map = SnapMap::build(inputs(&after));
            verify(&after, &map, &map, &|f, z| carried(&old, &map, f, z), "after Smooth")?;
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 6. Polygonal brushes: SI-059 erasure, mixed-kind Flatten.

type Spec = (bool, Vec<(i64, i64)>);

fn arrangement_strategy() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec((any::<bool>(), prop::collection::vec((0i64..5, 0i64..5), 2..5)), 1..4)
}

fn spec_points(s: &Spec) -> (Vec<F2>, bool) {
    let p: Vec<F2> = s.1.iter().map(|&(x, y)| (x as f64 * 20.0, y as f64 * 20.0)).collect();
    let closed = s.0 && p.len() >= 3;
    (p, closed)
}

/// Lattice brush rectangles: corners on a 10-du lattice, at least 10 du wide and high.
fn brush_rects() -> impl Strategy<Value = Vec<(i64, i64, i64, i64)>> {
    prop::collection::vec((0i64..9, 0i64..9, 1i64..4, 1i64..4), 1..4)
}

fn brush_contour(r: &(i64, i64, i64, i64)) -> Vec<F2> {
    let (x, y, w, h) = *r;
    rect(x as f64 * 10.0, y as f64 * 10.0, (x + w) as f64 * 10.0, (y + h) as f64 * 10.0)
}

/// The expected inputs of a layer: pencil polylines and brush rectangles, named by their persisted strokes.
fn expected_inputs(pencils: &[(Vec<F2>, bool)], brushes: &[Vec<F2>]) -> Vec<Input> {
    let mut out = Vec::new();
    for (i, (p, c)) in pencils.iter().enumerate() {
        let bez: Vec<F2> = if *c { loop_bez(p) } else { open_bez(p) };
        out.push(Input { id: format!("p{i}"), brush: false, edges: path_edges(&bez, *c) });
    }
    for (i, b) in brushes.iter().enumerate() {
        out.push(Input { id: format!("b{i}"), brush: true, edges: path_edges(&loop_bez(b), true) });
    }
    out
}

fn open_bez(p: &[F2]) -> Vec<F2> {
    let mut v = Vec::new();
    for k in 0..p.len() {
        v.push(p[k]);
        if k + 1 < p.len() {
            v.push(p[k]);
            v.push(p[k + 1]);
        }
    }
    v
}

/// A layer's persisted pencil and invisible pieces as straight polylines (anchors; the generated layers are straight).
fn pieces_of(ls: &Ls) -> Vec<(Vec<F2>, bool)> {
    ls.strokes
        .iter()
        .filter(|s| s.kind != "brush")
        .map(|s| (s.bez.iter().step_by(3).copied().collect(), s.closed))
        .collect()
}

/// Pencil polylines split where they cross brush edges properly, at the exact crossing rounded half to even onto the
/// grid (the merged layer's noding, SI-067: brush outlines are never split, the pencil pieces meet there), so the
/// snap-safety check and the expected map see the pieces the merge produces.
fn noded_against(pencils: &[(Vec<F2>, bool)], brushes: &[Vec<F2>]) -> Vec<(Vec<F2>, bool)> {
    let g = |p: F2| route_oracle::snap(p);
    let edges: Vec<(route_oracle::Ip, route_oracle::Ip)> =
        brushes.iter().flat_map(|b| (0..b.len()).map(move |k| (g(b[k]), g(b[(k + 1) % b.len()])))).collect();
    let o = |a: route_oracle::Ip, b: route_oracle::Ip, c: route_oracle::Ip| {
        (i128::from(b.0 - a.0) * i128::from(c.1 - a.1) - i128::from(b.1 - a.1) * i128::from(c.0 - a.0)).signum()
    };
    let half_even = |n: i128, d: i128| {
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let (q, r) = (n.div_euclid(d), n.rem_euclid(d));
        match (2 * r).cmp(&d) {
            std::cmp::Ordering::Less => q,
            std::cmp::Ordering::Greater => q + 1,
            std::cmp::Ordering::Equal => q + (q & 1),
        }
    };
    pencils
        .iter()
        .map(|(p, closed)| {
            let n = p.len();
            let m = if *closed { n } else { n - 1 };
            let mut out: Vec<F2> = Vec::new();
            for k in 0..m {
                let (a, b) = (g(p[k]), g(p[(k + 1) % n]));
                out.push(p[k]);
                let mut cuts: Vec<(f64, F2)> = Vec::new();
                for &(c, d) in &edges {
                    if o(a, b, c) * o(a, b, d) >= 0 || o(c, d, a) * o(c, d, b) >= 0 {
                        continue;
                    }
                    let r = (i128::from(b.0 - a.0), i128::from(b.1 - a.1));
                    let sv = (i128::from(d.0 - c.0), i128::from(d.1 - c.1));
                    let den = r.0 * sv.1 - r.1 * sv.0;
                    let num = i128::from(c.0 - a.0) * sv.1 - i128::from(c.1 - a.1) * sv.0;
                    let x = half_even(i128::from(a.0) * den + r.0 * num, den);
                    let y = half_even(i128::from(a.1) * den + r.1 * num, den);
                    cuts.push((num as f64 / den as f64, (x as f64 / GRID, y as f64 / GRID)));
                }
                cuts.sort_by(|u, v| u.0.total_cmp(&v.0));
                out.extend(cuts.into_iter().map(|c| c.1));
            }
            if !*closed {
                out.push(p[n - 1]);
            }
            out.dedup();
            (out, *closed)
        })
        .collect()
}

/// Whether two edges of different inputs cross properly at a point exactly half a grid unit off the grid in a
/// coordinate (a rounding tie).
fn tie_crossing(inputs: &[Input]) -> bool {
    let o = |a: route_oracle::Ip, b: route_oracle::Ip, c: route_oracle::Ip| {
        (i128::from(b.0 - a.0) * i128::from(c.1 - a.1) - i128::from(b.1 - a.1) * i128::from(c.0 - a.0)).signum()
    };
    for (i, x) in inputs.iter().enumerate() {
        for y in &inputs[i + 1..] {
            for &(a, b) in &x.edges {
                for &(c, d) in &y.edges {
                    if o(a, b, c) * o(a, b, d) >= 0 || o(c, d, a) * o(c, d, b) >= 0 {
                        continue;
                    }
                    let (r, sv) = (
                        (i128::from(b.0 - a.0), i128::from(b.1 - a.1)),
                        (i128::from(d.0 - c.0), i128::from(d.1 - c.1)),
                    );
                    let den = r.0 * sv.1 - r.1 * sv.0;
                    let num = i128::from(c.0 - a.0) * sv.1 - i128::from(c.1 - a.1) * sv.0;
                    let half = |start: i64, dir: i128| {
                        (2 * (i128::from(start) * den + dir * num)).rem_euclid(2 * den.abs()) == den.abs()
                    };
                    if half(a.0, r.0) || half(a.1, r.1) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Whether the snapped arrangement of `inputs` keeps its combinatorics (the round-1 oracle's snap-safety, 6 grid units).
fn safe(inputs: &[Input]) -> bool {
    let segs: Vec<(Pt, Pt)> = inputs
        .iter()
        .flat_map(|i| i.edges.iter().map(|&(a, b)| (oracle::pt(a.0, a.1), oracle::pt(b.0, b.1))))
        .collect();
    oracle::snap_safe(&oracle::arrangement(&segs), 6)
}

#[test]
fn erasing_polygonal_brushes_keeps_partly_covered_ones_whole() {
    // A pencil lattice loop enclosing polygonal brush rectangles (exact outlines; brush interiors are not zones, so the
    // loop's zone has them as holes), every zone painted; an erase region (a lattice rectangle inside the loop, clear
    // of it, on the half lattice) meeting at least one brush. SI-059/SI-071: a brush wholly inside the region is removed, one partly
    // covered is kept whole (`keptBrushIds`), others untouched; the pencil loop is unchanged; the region's boundary
    // stays as invisible pieces where it crossed a zone (the round-1 erase rule). Exact: the expected map (the loop,
    // the remaining brushes, the region pieces) against the product's zones; zones inside the region unpainted, the
    // others filled by the carry-over rule (a removed brush gives its area back to the zone around it).
    property(
        "erasing_polygonal_brushes_keeps_partly_covered_ones_whole",
        b"c24-r2-domain-si059er-----------",
        &["draw.erase.apply"],
        (
            10i64..15,
            10i64..15,
            prop::collection::vec(prop::array::uniform4(0u8..=255), 1..4),
            prop::array::uniform4(0u8..=255),
        ),
        |(wl, hl, raw, rr)| {
            let (wl, hl) = (*wl, *hl);
            // Brush rectangles inside the loop's margin, built to fit: x in [1, wl − 2], width up to 3 and to wl − 1.
            let fit = |a: u8, lo: i64, hi: i64| lo + i64::from(a) % (hi - lo + 1).max(1);
            let rects: Vec<(i64, i64, i64, i64)> = raw
                .iter()
                .map(|r| {
                    let (x, y) = (fit(r[0], 1, wl - 2), fit(r[1], 1, hl - 2));
                    (x, y, fit(r[2], 1, 3.min(wl - 1 - x)), fit(r[3], 1, 3.min(hl - 1 - y)))
                })
                .collect();
            // The region overlaps the first brush (on the half lattice: [10 rx + 5, 10 (rx + rw) + 5]).
            let b0 = rects[0];
            let rx = fit(rr[0], (b0.0 - 2).max(1), b0.0 + b0.2 - 1);
            let ry = fit(rr[1], (b0.1 - 2).max(1), b0.1 + b0.3 - 1);
            let (rw_lo, rh_lo) = ((b0.0 - rx).max(1), (b0.1 - ry).max(1));
            if rx + rw_lo > wl - 1 || ry + rh_lo > hl - 1 {
                return Ok(());
            }
            let region = (rx, ry, fit(rr[2], rw_lo, 4.min(wl - 1 - rx)), fit(rr[3], rh_lo, 4.min(hl - 1 - ry)));
            let region = &region;
            let lp = rect(0.0, 0.0, wl as f64 * 10.0, hl as f64 * 10.0);
            let pencils = vec![(lp.clone(), true)];
            let brushes: Vec<Vec<F2>> = rects.iter().map(brush_contour).collect();
            let all = expected_inputs(&pencils, &brushes);
            if !safe(&all) {
                return Ok(());
            }
            // The region on the half lattice (corners at 10 k + 5 du): it never runs along a stroke.
            let reg: Vec<F2> = brush_contour(region).iter().map(|p| (p.0 + 5.0, p.1 + 5.0)).collect();
            let (rx0, ry0, rx1, ry1) = (reg[0].0, reg[0].1, reg[2].0, reg[2].1);
            let inside = |p: &F2| p.0 >= rx0 && p.0 <= rx1 && p.1 >= ry0 && p.1 <= ry1;
            let whole: Vec<bool> = brushes.iter().map(|b| b.iter().all(&inside)).collect();
            let meets: Vec<bool> =
                brushes.iter().map(|b| !(b[2].0 <= rx0 || b[0].0 >= rx1 || b[2].1 <= ry0 || b[0].1 >= ry1)).collect();
            if !meets.iter().any(|m| *m) {
                return Ok(());
            }
            let h = fresh();
            let c = json!(palette(&h)[3]);
            let mut strokes = vec![pencil("p0", &lp, true)];
            strokes
                .extend(brushes.iter().enumerate().map(|(i, b)| brush(&format!("b{i}"), std::slice::from_ref(b), &c)));
            let (d, ids) = create(&h, vec![layer_json("l_e", "pencil", strokes)]);
            let la = &ids[0];
            let before = paint(&h, &d, la);
            let old = old_state(&before)?;
            let ebefore = SnapMap::build(all);
            verify_expected(&before, &ebefore, &old.map, &|_, z| z.colour.clone(), "before (expected)")?;
            // Each brush rectangle's stroke (one to one: equal rectangles take distinct strokes).
            let mut brush_ids: Vec<String> = Vec::new();
            for b in &brushes {
                let want: BTreeSet<route_oracle::Ip> = b.iter().map(|p| route_oracle::snap(*p)).collect();
                let set = |s: &Sk| {
                    route_oracle::flatten(&s.outline[0], true)
                        .0
                        .iter()
                        .map(|p| route_oracle::snap(*p))
                        .collect::<BTreeSet<_>>()
                };
                let s = before
                    .strokes
                    .iter()
                    .find(|s| s.kind == "brush" && set(s) == want && !brush_ids.contains(&s.id))
                    .unwrap();
                brush_ids.push(s.id.clone());
            }
            let r = ok(
                &h,
                "draw.erase.apply",
                json!({ "drawingId": d, "layers": [la], "region": reg.iter().map(|p| json!({"x": p.0, "y": p.1})).collect::<Vec<_>>() }),
            );
            let kept: BTreeSet<String> =
                r["keptBrushIds"].as_array().into_iter().flatten().map(|v| v.as_str().unwrap().to_owned()).collect();
            let want_kept: BTreeSet<String> =
                (0..brushes.len()).filter(|&i| meets[i] && !whole[i]).map(|i| brush_ids[i].clone()).collect();
            prop_assert_eq!(&kept, &want_kept, "keptBrushIds");
            let after = layer(&h, &d, la);
            let present: BTreeSet<&String> =
                after.strokes.iter().filter(|s| s.kind == "brush").map(|s| &s.id).collect();
            for i in 0..brushes.len() {
                prop_assert_eq!(present.contains(&brush_ids[i]), !whole[i], "brush {} present", i);
            }
            let remaining: Vec<Vec<F2>> =
                (0..brushes.len()).filter(|&i| !whole[i]).map(|i| brushes[i].clone()).collect();
            // The region's boundary stays where it crossed a zone (the round-1 erase rule): its pieces inside old zone
            // faces, cut where they cross the old map's edges.
            let old_edges: Vec<(Pt, Pt)> =
                old.map.edges.keys().map(|&(a, b)| (oracle::pt(a.0, a.1), oracle::pt(b.0, b.1))).collect();
            let mut pieces = Vec::new();
            for k in 0..4 {
                let (a, b) = (route_oracle::snap(reg[k]), route_oracle::snap(reg[(k + 1) % 4]));
                let (pa, pb) = (oracle::pt(a.0, a.1), oracle::pt(b.0, b.1));
                let mut ts = vec![Q::int(0)];
                ts.extend(oracle::cut_parameters(&pa, &pb, &old_edges));
                ts.push(Q::int(1));
                for w in ts.windows(2) {
                    let (p, q) = (
                        oracle::snap_pt(&oracle::lerp(&pa, &pb, &w[0])),
                        oracle::snap_pt(&oracle::lerp(&pa, &pb, &w[1])),
                    );
                    let mid = Dp { x: i128::from(p.0 + q.0), y: i128::from(p.1 + q.1), s: 2 };
                    if p != q && old.map.locate(mid).is_some_and(|f| old.fills.contains_key(&f)) {
                        pieces.push((p, q));
                    }
                }
            }
            let mut eins = expected_inputs(&pencils, &remaining);
            eins.push(Input { id: "region".into(), brush: false, edges: pieces });
            let emap = SnapMap::build(eins);
            let labels = SnapMap::build(inputs(&after));
            let in_region = |p: F2| p.0 > rx0 && p.0 < rx1 && p.1 > ry0 && p.1 < ry1;
            verify_expected(
                &after,
                &emap,
                &labels,
                &|f, z| if in_region(z.interior) { None } else { carried(&old, &emap, f, z) },
                "after erase",
            )?;
            Ok(())
        },
    );
}

#[test]
fn flatten_of_pencil_and_polygonal_brush_layers_is_exact() {
    // A pencil lattice layer (zones painted) and a brush layer of polygonal brush rectangles overlapping it, flattened.
    // Exact: the merged layer's zones are the zone faces of pencils plus brushes (brush interiors not zones), boundary
    // strokes and IDs labelled by the merged strokes, fills by the carry-over rule from the pencil layer.
    property(
        "flatten_of_pencil_and_polygonal_brush_layers_is_exact",
        b"c24-r2-domain-mixflat-----------",
        &["draw.flatten.apply"],
        (arrangement_strategy(), brush_rects()),
        |(lines, rects)| {
            let pencils: Vec<(Vec<F2>, bool)> = lines.iter().map(spec_points).collect();
            let brushes: Vec<Vec<F2>> = rects.iter().map(brush_contour).collect();
            let all = expected_inputs(&pencils, &brushes);
            if !safe(&all) || !safe(&expected_inputs(&pencils, &[])) {
                return Ok(());
            }
            let h = fresh();
            let c = json!(palette(&h)[4]);
            let strokes: Vec<Value> =
                pencils.iter().enumerate().map(|(i, (p, cl))| pencil(&format!("p{i}"), p, *cl)).collect();
            let bstrokes: Vec<Value> =
                brushes.iter().enumerate().map(|(i, b)| brush(&format!("b{i}"), std::slice::from_ref(b), &c)).collect();
            let (d, ids) = create(&h, vec![layer_json("l_p", "pencil", strokes), layer_json("l_q", "brush", bstrokes)]);
            let before = paint(&h, &d, &ids[0]);
            let old = old_state(&before)?;
            let r = ok(&h, "draw.flatten.apply", json!({ "drawingId": d, "ids": [ids[0], ids[1]] }));
            let target = r["layerId"].as_str().unwrap().to_owned();
            let after = layer(&h, &d, &target);
            prop_assert_eq!(after.strokes.iter().filter(|s| s.kind == "brush").count(), brushes.len());
            // Expected: the pencil layer's persisted pieces (its junctions already on the grid) split where they cross a
            // brush edge (the merge's noding), and the brushes; skipped
            // when those are not snap-safe (a rounded junction within the junction tolerance of a brush vertex resolves
            // onto it, which pure snap rounding does not model; the round-1 oracle's rule), or when a proper crossing lies
            // exactly half a grid unit off the grid (noding rounds it half to even, the map's hot pixel half away).
            let pre = expected_inputs(&noded_against(&pieces_of(&before), &brushes), &brushes);
            if !safe(&pre) || tie_crossing(&pre) {
                return Ok(());
            }
            let emap = SnapMap::build(pre);
            let labels = SnapMap::build(inputs(&after));
            verify_expected(&after, &emap, &labels, &|f, z| carried(&old, &emap, f, z), "after Flatten")?;
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 7. Non-parallelogram envelope.

/// The projective map taking the unit square's corners (0,0), (1,0), (1,1), (0,1) to `c` (exact rationals).
fn homography(c: &[(Q, Q); 4]) -> [Q; 8] {
    // Heckbert's closed form.
    let (x0, y0) = c[0].clone();
    let (x1, y1) = c[1].clone();
    let (x2, y2) = c[2].clone();
    let (x3, y3) = c[3].clone();
    let sx = x0.minus(&x1).plus(&x2).minus(&x3);
    let sy = y0.minus(&y1).plus(&y2).minus(&y3);
    let (dx1, dx2) = (x1.minus(&x2), x3.minus(&x2));
    let (dy1, dy2) = (y1.minus(&y2), y3.minus(&y2));
    let det = dx1.times(&dy2).minus(&dx2.times(&dy1));
    let g = sx.times(&dy2).minus(&dx2.times(&sy)).over(&det);
    let hh = dx1.times(&sy).minus(&sx.times(&dy1)).over(&det);
    let a = x1.minus(&x0).plus(&g.times(&x1));
    let b = x3.minus(&x0).plus(&hh.times(&x3));
    let d = y1.minus(&y0).plus(&g.times(&y1));
    let e = y3.minus(&y0).plus(&hh.times(&y3));
    [a, b, x0, d, e, y0, g, hh]
}

fn project(m: &[Q; 8], u: &Q, v: &Q) -> (Q, Q) {
    let w = m[6].times(u).plus(&m[7].times(v)).plus(&Q::int(1));
    (m[0].times(u).plus(&m[1].times(v)).plus(&m[2]).over(&w), m[3].times(u).plus(&m[4].times(v)).plus(&m[5]).over(&w))
}

#[test]
fn non_parallelogram_envelopes_map_the_layer_projectively() {
    // A lattice arrangement, zones painted; an envelope onto a random convex quadrilateral that is not a parallelogram
    // (SI-070: control points mapped as points, projectively). Exact: every anchor mapped by the exact homography from
    // the arrangement's bounds, snapped; the expected map's zone faces against the product's zones (cases where a
    // mapped coordinate lies within 10⁻⁶ grid units of a rounding tie, or the mapped arrangement is not snap-safe,
    // are skipped before the command); fills: the source zone at each zone's interior point mapped back through the
    // three-corner affine fit (SI-070 round 1), or, where that lands in no source zone, the carry-over fill
    // (a point within 10⁻³ du of a source edge is a floating-point tie of the inverse map and not judged).
    property(
        "non_parallelogram_envelopes_map_the_layer_projectively",
        b"c24-r2-domain-envelop-----------",
        &["draw.envelope.apply"],
        (arrangement_strategy(), prop::array::uniform8(-6i64..7)),
        |(lines, jitter)| {
            let pencils: Vec<(Vec<F2>, bool)> = lines.iter().map(spec_points).collect();
            let all = expected_inputs(&pencils, &[]);
            if !safe(&all) {
                return Ok(());
            }
            let anchors: Vec<F2> = pencils.iter().flat_map(|(p, _)| p.iter().copied()).collect();
            let (ax0, ay0, ax1, ay1) =
                anchors.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
                    (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
                });
            let h = fresh();
            let strokes: Vec<Value> =
                pencils.iter().enumerate().map(|(i, (p, cl))| pencil(&format!("p{i}"), p, *cl)).collect();
            let (d, ids) = create(&h, vec![layer_json("l_v", "pencil", strokes)]);
            let la = &ids[0];
            let before = paint(&h, &d, la);
            let old = old_state(&before)?;
            let sids: Vec<String> = before.strokes.iter().map(|s| s.id.clone()).collect();
            // The envelope's source rectangle is the selection's bounds: the anchors' box grown by half the line
            // thickness (1 du lines).
            let b = ok(&h, "draw.bounds.get", json!({ "drawingId": d, "ids": sids }))["bounds"].clone();
            let (bx0, by0) = (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap());
            let (bx1, by1) = (bx0 + b["width"].as_f64().unwrap(), by0 + b["height"].as_f64().unwrap());
            prop_assert_eq!((bx0, by0, bx1, by1), (ax0 - 0.5, ay0 - 0.5, ax1 + 0.5, ay1 + 0.5), "bounds {}", b);
            let base = [(bx0, by0), (bx1, by0), (bx1, by1), (bx0, by1)];
            let corners: Vec<F2> = (0..4)
                .map(|i| (base[i].0 + 2.0 * jitter[2 * i] as f64, base[i].1 + 2.0 * jitter[2 * i + 1] as f64))
                .collect();
            let cross = |o: F2, a: F2, b: F2| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
            let turns: Vec<f64> =
                (0..4).map(|i| cross(corners[i], corners[(i + 1) % 4], corners[(i + 2) % 4])).collect();
            if !(turns.iter().all(|t| *t > 0.0) || turns.iter().all(|t| *t < 0.0)) {
                return Ok(());
            }
            let para = (corners[0].0 + corners[2].0 - corners[1].0 - corners[3].0).abs() < 1e-9
                && (corners[0].1 + corners[2].1 - corners[1].1 - corners[3].1).abs() < 1e-9;
            if para {
                return Ok(());
            }
            let cq: [(Q, Q); 4] = std::array::from_fn(|i| (Q::from_f64(corners[i].0), Q::from_f64(corners[i].1)));
            let m = homography(&cq);
            let (w, hgt) = (Q::from_f64(bx1 - bx0), Q::from_f64(by1 - by0));
            let map_pt = |p: F2| -> (Q, Q) {
                let u = Q::from_f64(p.0 - bx0).over(&w);
                let v = Q::from_f64(p.1 - by0).over(&hgt);
                project(&m, &u, &v)
            };
            // Mapped anchors on the grid (skip near ties).
            let mut tie = false;
            let snap_q = |q: &Q, tie: &mut bool| -> f64 {
                let g = q.times(&Q::int(64));
                let f = g.clone().f64();
                if (f - f.floor() - 0.5).abs() < 1e-6 {
                    *tie = true;
                }
                oracle::snap(&g) as f64 / 64.0
            };
            // The persisted pieces (junction anchors included: an envelope maps every control point) mapped.
            let mapped: Vec<(Vec<F2>, bool)> = pieces_of(&before)
                .iter()
                .map(|(p, c)| {
                    (
                        p.iter()
                            .map(|&a| {
                                let (x, y) = map_pt(a);
                                (snap_q(&x, &mut tie), snap_q(&y, &mut tie))
                            })
                            .collect(),
                        *c,
                    )
                })
                .collect();
            if tie {
                return Ok(());
            }
            let eins = expected_inputs(&mapped, &[]);
            if !safe(&eins) {
                return Ok(());
            }
            ok(
                &h,
                "draw.envelope.apply",
                json!({ "drawingId": d, "ids": sids, "corners": corners.iter().map(|p| json!({"x": p.0, "y": p.1})).collect::<Vec<_>>() }),
            );
            let after = layer(&h, &d, la);
            let emap = SnapMap::build(eins);
            let labels = SnapMap::build(inputs(&after));
            // The three-corner affine fit (corners 0, 1, 3) and its inverse.
            let (o, ex, ey) = (
                corners[0],
                ((corners[1].0 - corners[0].0) / (bx1 - bx0), (corners[1].1 - corners[0].1) / (bx1 - bx0)),
                ((corners[3].0 - corners[0].0) / (by1 - by0), (corners[3].1 - corners[0].1) / (by1 - by0)),
            );
            let det = ex.0 * ey.1 - ey.0 * ex.1;
            let back = |p: F2| {
                let (x, y) = (p.0 - o.0, p.1 - o.1);
                (bx0 + (ey.1 * x - ey.0 * y) / det, by0 + (-ex.1 * x + ex.0 * y) / det)
            };
            // A zone whose mapped-back point lies in no source zone keeps the carry-over fill.
            verify_expected(
                &after,
                &emap,
                &labels,
                &|f, z| {
                    let p = back(z.interior);
                    let dp = Dp {
                        x: (p.0 * 64.0 * 1024.0).round() as i128,
                        y: (p.1 * 64.0 * 1024.0).round() as i128,
                        s: 1024,
                    };
                    // Within 10⁻³ du of a source edge the source zone is a floating-point tie of the inverse map: either side.
                    let near = old.map.edges.keys().any(|&(a, b)| {
                        route_oracle::seg_dist(
                            (p.0 * GRID, p.1 * GRID),
                            (a.0 as f64, a.1 as f64),
                            (b.0 as f64, b.1 as f64),
                        ) < 1e-3 * GRID
                    });
                    if near {
                        return z.colour.clone();
                    }
                    match old.map.locate(dp).filter(|o| old.fills.contains_key(o)) {
                        Some(o) => old.fills[&o].clone(),
                        None => carried(&old, &emap, f, z),
                    }
                },
                "after envelope",
            )?;
            Ok(())
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 8. Snap onto a curved contour.

fn cubic_at(c: &[F2; 4], t: f64) -> F2 {
    route_oracle::eval(c, t)
}

/// The nearest point of a cubic to `p`: dense sampling, then golden-section refinement.
fn nearest_on_cubic(c: &[F2; 4], p: F2) -> (f64, F2) {
    let d = |t: f64| {
        let q = cubic_at(c, t);
        (q.0 - p.0).hypot(q.1 - p.1)
    };
    let n = 2048;
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let v = d(t);
        if v < best.0 {
            best = (v, t);
        }
    }
    let (mut a, mut b) = ((best.1 - 1.0 / n as f64).max(0.0), (best.1 + 1.0 / n as f64).min(1.0));
    let g = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..100 {
        let (m1, m2) = (b - g * (b - a), a + g * (b - a));
        if d(m1) < d(m2) {
            b = m2;
        } else {
            a = m1;
        }
    }
    let t = (a + b) / 2.0;
    (d(t), cubic_at(c, t))
}

#[test]
fn snapping_onto_a_curved_contour_reaches_its_nearest_point() {
    // A symmetric cubic hump closed by a straight chord (one zone, painted) and a vertical pencil line from the chord
    // up to within a few du of the hump; the line's top end is moved with Snap to Contour. Predicted: the nearest point
    // of the cubic to the requested position (02 §8.9, SI-070: within 8 du). (a) The moved end lies within 1/32 du
    // (junction tolerance) of it; two zones, split by the line, of areas within 1/8 du² per du of hump of the
    // predicted parts (the hump region's flattened polygon cut by the line at the predicted point); both keep the old
    // fill; (b) exact stage on the output.
    property(
        "snapping_onto_a_curved_contour_reaches_its_nearest_point",
        b"c24-r2-domain-snapcrv-----------",
        &["draw.point.move"],
        (4i64..10, 4i64..12, 1i64..9, 2i64..6, -3i64..4),
        |&(wid, hgt, xs, gap, dx)| {
            let (w, hh) = (wid as f64 * 10.0, hgt as f64 * 10.0);
            let c: [F2; 4] = [(0.0, 0.0), (0.0, hh), (w, hh), (w, 0.0)];
            let x = w * xs as f64 / 10.0;
            // The curve's height at x (it is a graph over x): bisection on the parameter.
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..80 {
                let m = (lo + hi) / 2.0;
                if cubic_at(&c, m).0 < x {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let ycurve = cubic_at(&c, lo).1;
            let start_y = (ycurve - gap as f64).floor();
            let req = (x + dx as f64, start_y + gap as f64 - 1.0);
            let (dist, target) = nearest_on_cubic(&c, req);
            // The curve must be the nearest contour (the chord y = 0 and the curve's ends farther), inside 8 du.
            if start_y <= 2.0 || dist > 7.5 || dist + 0.5 >= req.1 || target.0 <= 2.0 || target.0 >= w - 2.0 {
                return Ok(());
            }
            let h = fresh();
            let curve = json!({ "id": "cv", "kind": "pencil", "closed": false,
                "bezier": c.iter().map(|p| json!({"x": p.0, "y": p.1})).collect::<Vec<_>>(), "colourId": null,
                "lineStyle": { "thickness": 1, "lineType": "solid" } });
            let strokes = vec![
                curve,
                pencil("ch", &[(w, 0.0), (0.0, 0.0)], false),
                pencil("ln", &[(x, start_y), (x, 0.0)], false),
            ];
            let (d, ids) = create(&h, vec![layer_json("l_c", "pencil", strokes)]);
            let la = &ids[0];
            let before = paint(&h, &d, la);
            prop_assert_eq!(before.zones.len(), 1, "the dangling line splits nothing: {:?}", before.zones);
            let old = old_state(&before)?;
            let line = before
                .strokes
                .iter()
                .find(|s| s.bez.first() == Some(&(x, start_y)))
                .ok_or_else(|| TestCaseError::fail("the line"))?;
            ok(
                &h,
                "draw.point.move",
                json!({ "drawingId": d, "points": [{ "strokeId": line.id, "anchor": 0 }],
                        "delta": { "x": req.0 - x, "y": req.1 - start_y }, "snap": true }),
            );
            let after = layer(&h, &d, la);
            let tol_pt = 1.0 / 32.0 + 1.0 / 64.0;
            let moved = after
                .strokes
                .iter()
                .flat_map(|s| s.bez.iter().step_by(3))
                .any(|p| (p.0 - target.0).hypot(p.1 - target.1) <= tol_pt);
            prop_assert!(moved, "no anchor within {} du of the predicted snap point {:?}", tol_pt, target);
            // Predicted parts: the hump polygon (curve flattened finely, chord) cut by the line (x, 0) → target.
            let n = 4096;
            let mut left: Vec<F2> = vec![(x, 0.0), (0.0, 0.0)];
            let mut right: Vec<F2> = vec![target];
            let mut crossed = false;
            for i in 0..=n {
                let q = cubic_at(&c, i as f64 / n as f64);
                if !crossed && q.0 >= target.0 {
                    crossed = true;
                    left.push(target);
                }
                if crossed {
                    right.push(q);
                } else {
                    left.push(q);
                }
            }
            right.push((x, 0.0));
            let (al, ar) = (area(&left).abs(), area(&right).abs());
            prop_assert_eq!(after.zones.len(), 2, "zones after the snap {:?}", after.zones);
            let tol = (perimeter(&left, true) + perimeter(&right, true)) * (1.0 / 32.0 + 1.0 / 64.0);
            let mut got: Vec<f64> = after.zones.iter().map(|z| z.area).collect();
            got.sort_by(f64::total_cmp);
            let mut want = vec![al, ar];
            want.sort_by(f64::total_cmp);
            for (g, wv) in got.iter().zip(&want) {
                prop_assert!((g - wv).abs() <= tol, "zone area {} vs predicted {} (tolerance {})", g, wv, tol);
            }
            for z in &after.zones {
                prop_assert_eq!(&z.colour, &before.zones[0].colour);
            }
            let map = SnapMap::build(inputs(&after));
            verify(&after, &map, &map, &|f, z| carried(&old, &map, f, z), "after the snap")?;
            Ok(())
        },
    );
}

/// Shrunk from `flatten_of_pencil_and_polygonal_brush_layers_is_exact` (seed `c24-r2-domain-mixflat`, 4096 cases): a
/// pencil triangle (40,0)-(40,80)-(60,0) closed along the x-axis, and a brush rectangle (40,50)-(50,80) whose left edge
/// runs along the triangle's and whose corner only touches it (no junction there). Flatten discarded the whole piece
/// (40,0)-(40,80)-(47.5,50) because its middle segment's middle lies under the brush, its visible part included, and the
/// triangle's zone was lost. A piece now goes only when every segment's middle is under cover (product fix, ADR-0017
/// history 16).
#[test]
fn flatten_keeps_a_pencil_piece_that_a_brush_only_partly_covers() {
    let h = fresh();
    let c = json!(palette(&h)[4]);
    let pencils = [(vec![(40.0, 0.0), (40.0, 80.0), (60.0, 0.0), (0.0, 0.0)], false)];
    let b = brush_contour(&(4, 5, 1, 3));
    let strokes: Vec<Value> = pencils.iter().enumerate().map(|(i, (p, cl))| pencil(&format!("p{i}"), p, *cl)).collect();
    let (d, ids) = create(
        &h,
        vec![
            layer_json("l_p", "pencil", strokes),
            layer_json("l_q", "brush", vec![brush("b0", std::slice::from_ref(&b), &c)]),
        ],
    );
    let before = paint(&h, &d, &ids[0]);
    let old = old_state(&before).unwrap();
    let r = ok(&h, "draw.flatten.apply", json!({ "drawingId": d, "ids": [ids[0], ids[1]] }));
    let after = layer(&h, &d, r["layerId"].as_str().unwrap());
    let emap = SnapMap::build(expected_inputs(&pieces_of(&before), &[b]));
    let labels = SnapMap::build(inputs(&after));
    verify_expected(&after, &emap, &labels, &|f, z| carried(&old, &emap, f, z), "after Flatten").unwrap();
    assert_eq!(after.zones.len(), 1);
    assert_eq!(after.zones[0].area, 687.5);
    assert_eq!(after.zones[0].colour, before.zones[0].colour);
}
