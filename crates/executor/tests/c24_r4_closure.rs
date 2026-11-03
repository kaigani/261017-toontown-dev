//! Round-4 scoped closure: builder harness/oracle helpers, fresh A7 seed; four enabled tests.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    dead_code,
    reason = "test"
)]

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
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed.map(|b| b ^ 0xA7)),
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

// ---------------------------------------------------------------------------------------------------------------
// 1. Lines to Brush, then Centre Line, on closed loops.

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

/// An independent trace of a binary mask (02 §11.2 as `tt_vector::vectorize` documents it, rewritten here): ink is
/// 8-connected and paper 4-connected (paper outside the image belongs to the exterior); every pixel side between ink
/// and paper is a unit edge of the pixel-corner lattice, directed with the ink on its right in image coordinates (y
/// down), walked into closed loops turning left at a saddle; a loop of positive area is an ink region's outer boundary,
/// of negative area one of its holes. Each loop is taken through the midpoints of its unit edges and mapped so the
/// image width spans 1200 du centred on the origin, y up.
struct MaskTrace {
    /// Each ink region's contours in drawing coordinates: its outer boundary first, then its holes.
    regions: Vec<Vec<Vec<F2>>>,
    /// Each enclosed paper component: its enclosing region, that region's contour bounding it, and the regions lying
    /// inside it (islands, bounded there by their outer contours).
    zones: Vec<(usize, usize, Vec<usize>)>,
}

/// A directed unit edge leaving a lattice vertex: its end, its ink region, the paper component on its left.
type UnitEdge = ((i64, i64), usize, usize);

fn trace_mask(mask: &[Vec<bool>]) -> MaskTrace {
    let (hgt, wid) = (mask.len() as i64, mask[0].len() as i64);
    let ink = |x: i64, y: i64| x >= 0 && y >= 0 && x < wid && y < hgt && mask[y as usize][x as usize];
    // Labels: ink regions (8-connected) and paper components (4-connected; those touching the border are the
    // exterior, label usize::MAX).
    let label = |is_ink: bool, neighbours: &[(i64, i64)]| {
        let mut lab = vec![vec![usize::MAX - 1; wid as usize]; hgt as usize];
        let mut n = 0;
        for y in 0..hgt {
            for x in 0..wid {
                if ink(x, y) != is_ink || lab[y as usize][x as usize] != usize::MAX - 1 {
                    continue;
                }
                let mut comp = vec![(x, y)];
                let mut stack = vec![(x, y)];
                lab[y as usize][x as usize] = n;
                let mut border = false;
                while let Some((cx, cy)) = stack.pop() {
                    border |= cx == 0 || cy == 0 || cx == wid - 1 || cy == hgt - 1;
                    for &(dx, dy) in neighbours {
                        let (nx, ny) = (cx + dx, cy + dy);
                        if nx >= 0
                            && ny >= 0
                            && nx < wid
                            && ny < hgt
                            && ink(nx, ny) == is_ink
                            && lab[ny as usize][nx as usize] == usize::MAX - 1
                        {
                            lab[ny as usize][nx as usize] = n;
                            stack.push((nx, ny));
                            comp.push((nx, ny));
                        }
                    }
                }
                if !is_ink && border {
                    for (cx, cy) in comp {
                        lab[cy as usize][cx as usize] = usize::MAX;
                    }
                } else {
                    n += 1;
                }
            }
        }
        (lab, n)
    };
    let eight = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
    let four = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    let (ink_lab, regions_n) = label(true, &eight);
    let (paper_lab, _) = label(false, &four);
    let paper_of = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= wid || y >= hgt { usize::MAX } else { paper_lab[y as usize][x as usize] }
    };
    // Directed unit edges, ink on the right: (start, end, ink region, paper component on the left).
    let mut out: BTreeMap<(i64, i64), Vec<UnitEdge>> = BTreeMap::new();
    for y in 0..hgt {
        for x in 0..wid {
            if !ink(x, y) {
                continue;
            }
            let r = ink_lab[y as usize][x as usize];
            let sides = [
                ((x, y - 1), (x, y), (x + 1, y)),
                ((x + 1, y), (x + 1, y), (x + 1, y + 1)),
                ((x, y + 1), (x + 1, y + 1), (x, y + 1)),
                ((x - 1, y), (x, y + 1), (x, y)),
            ];
            for (n, a, b) in sides {
                if !ink(n.0, n.1) {
                    out.entry(a).or_default().push((b, r, paper_of(n.0, n.1)));
                }
            }
        }
    }
    let s = 1200.0 / wid as f64;
    let to_du = |a: (i64, i64), b: (i64, i64)| {
        let (mx, my) = ((a.0 + b.0) as f64 / 2.0, (a.1 + b.1) as f64 / 2.0);
        ((mx - wid as f64 / 2.0) * s, (hgt as f64 / 2.0 - my) * s)
    };
    let mut regions: Vec<Vec<Vec<F2>>> = vec![Vec::new(); regions_n];
    let mut outer_paper: Vec<usize> = vec![usize::MAX; regions_n];
    let mut holes: Vec<(usize, usize, usize)> = Vec::new(); // (region, contour index, paper component)
    let mut outers: Vec<(usize, Vec<F2>)> = Vec::new();
    while let Some((&start, _)) = out.iter().find(|(_, v)| !v.is_empty()) {
        let (first_end, r, paper) = out.get_mut(&start).unwrap().remove(0);
        let mut verts = vec![start];
        let (mut prev, mut cur) = (start, first_end);
        while cur != start {
            verts.push(cur);
            let list = out.get_mut(&cur).unwrap();
            let k = if list.len() == 1 {
                0
            } else {
                // A saddle: turn left (y down: left of (dx, dy) is (dy, -dx)).
                let d = (cur.0 - prev.0, cur.1 - prev.1);
                let left = (cur.0 + d.1, cur.1 - d.0);
                list.iter().position(|e| e.0 == left).unwrap_or(0)
            };
            let (next, _, _) = list.remove(k);
            (prev, cur) = (cur, next);
        }
        let n = verts.len();
        let area2: i64 = (0..n).map(|i| verts[i].0 * verts[(i + 1) % n].1 - verts[(i + 1) % n].0 * verts[i].1).sum();
        let contour: Vec<F2> = (0..n).map(|i| to_du(verts[i], verts[(i + 1) % n])).collect();
        if area2 > 0 {
            outer_paper[r] = paper;
            outers.push((r, contour));
        } else {
            holes.push((r, 0, paper));
            regions[r].push(contour);
            let last = holes.len() - 1;
            holes[last].1 = regions[r].len();
        }
    }
    for (r, c) in outers {
        regions[r].insert(0, c);
    }
    let zones = holes
        .iter()
        .map(|&(r, k, paper)| {
            (r, k, (0..regions_n).filter(|&i| outer_paper[i] == paper && paper != usize::MAX).collect())
        })
        .collect();
    MaskTrace { regions, zones }
}

/// Whether a point lies inside a closed polygon (non-zero winding).
fn inside_poly(poly: &[F2], p: F2) -> bool {
    let mut w = 0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let o = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        if a.1 <= p.1 {
            if b.1 > p.1 && o > 0.0 {
                w += 1;
            }
        } else if b.1 <= p.1 && o < 0.0 {
            w -= 1;
        }
    }
    w != 0
}

/// Vectorize's read-back against the independent trace of its mask (C24-R3-02 ruling). (a) One brush stroke per ink
/// region, its outline (one contour, holes bridged: the zero-width bridges cancel) giving exactly the region's traced
/// contours, each within the fit tolerance of its traced contour both ways (0.25 du + 0.5 du × smoothness 3, 02 §3.3),
/// plus the flattening tolerance and a grid cell; (b) one unpainted zone per enclosed paper component, its interior
/// point inside that component's traced hole and outside its islands, bounded by exactly the strokes of the enclosing
/// region and of the islands, its area within the bounding contours' perimeter × that envelope of the traced area;
/// (c) the exact snap-rounded map of the output equals the product's zones (partition, areas, boundaries, IDs).
fn vectorize_comparison(ls: &Ls, mask: &[Vec<bool>]) -> Result<(), TestCaseError> {
    let t = trace_mask(mask);
    // Two independent counts of the enclosed paper components agree (the traced holes, a flood fill).
    prop_assert_eq!(t.zones.len(), enclosed_paper(mask).len(), "traced holes vs enclosed paper components");
    let env = 0.25 + 0.5 * 3.0 + 1.0 / 128.0 + 1.0 / 64.0;
    let near = |a: &[F2], b: &[F2]| hausdorff(a, true, b, true) <= env;
    prop_assert_eq!(ls.strokes.len(), t.regions.len(), "brush strokes for {} ink regions", t.regions.len());
    let mut stroke_of: Vec<Option<String>> = vec![None; t.regions.len()];
    for s in &ls.strokes {
        prop_assert_eq!(s.kind.as_str(), "brush");
        prop_assert_eq!(s.outline.len(), 1, "stroke {} outline contours", s.id);
        let contours = unbridged(&route_oracle::flatten(&s.outline[0], true).0);
        let region = (0..t.regions.len()).find(|&r| {
            stroke_of[r].is_none() && t.regions[r].len() == contours.len() && {
                let mut used = BTreeSet::new();
                contours.iter().all(|c| {
                    let m = (0..t.regions[r].len()).find(|k| !used.contains(k) && near(c, &t.regions[r][*k]));
                    m.is_some_and(|k| used.insert(k))
                })
            }
        });
        prop_assert!(
            region.is_some(),
            "stroke {}'s contours {:?} match no traced ink region {:?}",
            s.id,
            contours,
            t.regions
        );
        stroke_of[region.unwrap()] = Some(s.id.clone());
    }
    prop_assert_eq!(
        ls.zones.len(),
        t.zones.len(),
        "zones {:?} for {} enclosed paper components",
        ls.zones,
        t.zones.len()
    );
    let mut used = BTreeSet::new();
    for z in &ls.zones {
        prop_assert!(z.colour.is_none());
        let k = (0..t.zones.len()).find(|k| {
            let (r, c, islands) = &t.zones[*k];
            !used.contains(k)
                && inside_poly(&t.regions[*r][*c], z.interior)
                && islands.iter().all(|&i| !inside_poly(&t.regions[i][0], z.interior))
        });
        prop_assert!(k.is_some(), "zone {} at {:?} lies in no traced paper component", z.id, z.interior);
        let k = k.unwrap();
        used.insert(k);
        let (r, c, islands) = &t.zones[k];
        let hole = &t.regions[*r][*c];
        let want = area(hole).abs() - islands.iter().map(|&i| area(&t.regions[i][0]).abs()).sum::<f64>();
        let band =
            (perimeter(hole, true) + islands.iter().map(|&i| perimeter(&t.regions[i][0], true)).sum::<f64>()) * env;
        prop_assert!((z.area - want).abs() <= band, "zone {} area {} vs traced {} (band {})", z.id, z.area, want, band);
        let mut bounding: Vec<String> =
            std::iter::once(*r).chain(islands.iter().copied()).filter_map(|i| stroke_of[i].clone()).collect();
        bounding.sort();
        bounding.dedup();
        prop_assert_eq!(&z.boundary, &bounding, "zone {} boundary strokes", z.id);
    }
    let map = SnapMap::build(inputs(ls));
    verify(ls, &map, &map, &|_, _| None, "vectorized")?;
    Ok(())
}

#[test]
fn vectorize_masks_enclose_the_predicted_paper_components() {
    // Random masks of up to 7 × 7 pixels with a one-pixel ink frame, no diagonal-only contact, vectorized; the
    // read-back is compared with the oracle's own trace of the mask in the fixed source-to-drawing mapping
    // (`vectorize_comparison`: boundaries, holes and adjacency within the fitting envelope, then the exact map stage;
    // C24-R3-02).
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
            vectorize_comparison(&ls, &mask)
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// 5. Smooth with a changed shape.

/// Douglas–Peucker on a closed polygon (fixed first vertex and the vertex farthest from it), within `tol`.

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

/// Whether two edges of different inputs cross properly at a point exactly half a grid unit off the grid in a
/// coordinate (a rounding tie).

/// Whether the snapped arrangement of `inputs` keeps its combinatorics (the round-1 oracle's snap-safety, 6 grid units).
fn safe(inputs: &[Input]) -> bool {
    let segs: Vec<(Pt, Pt)> = inputs
        .iter()
        .flat_map(|i| i.edges.iter().map(|&(a, b)| (oracle::pt(a.0, a.1), oracle::pt(b.0, b.1))))
        .collect();
    oracle::snap_safe(&oracle::arrangement(&segs), 6)
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
    // (the mapped-back point queried as the product queries points: rounded to 1/256 du, refused on a boundary).
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
                    // The product's point query (ADR-0017 history 17): the point rounded to 1/256 du (a quarter grid
                    // unit), refused on a boundary; a refused or zone-less point keeps the carry-over fill.
                    let dp = Dp { x: (p.0 * 256.0).round() as i128, y: (p.1 * 256.0).round() as i128, s: 4 };
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

/// The nearest point of a cubic to `p`: dense sampling, then golden-section refinement.

/// Shrunk from `flatten_of_pencil_and_polygonal_brush_layers_is_exact` (seed `c24-r2-domain-mixflat`, 4096 cases): a
/// pencil triangle (40,0)-(40,80)-(60,0) closed along the x-axis, and a brush rectangle (40,50)-(50,80) whose left edge
/// runs along the triangle's and whose corner only touches it (no junction there). Flatten discarded the whole piece
/// (40,0)-(40,80)-(47.5,50) because its middle segment's middle lies under the brush, its visible part included, and the
/// triangle's zone was lost. A piece now goes only when every segment's middle is under cover (product fix, ADR-0017
/// history 16).
#[test]
fn border_paper_and_diagonal_ink_bridge() {
    // Two ink lobes meet only at the (3,3)/(4,4) diagonal; paper has one hole and one border component.
    let mask = ["........", ".###....", ".#.#....", ".###....", "....###.", "....###.", "....###.", "........"]
        .map(|r| r.bytes().map(|b| b == b'#').collect::<Vec<_>>())
        .to_vec();
    assert_eq!(enclosed_paper(&mask).len(), 1);
    let (hgt, wid) = (mask.len(), mask[0].len());
    let result = (|| -> Result<(), TestCaseError> {
        let h = fresh();
        let dir = std::env::temp_dir().join(format!("tt-c24-r2-domain-{}", tt_persist::fsx::random_token(8)));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mask.png");
        let rgba: Vec<u8> =
            mask.iter().flatten().flat_map(|&ink| if ink { [0, 0, 0, 255] } else { [255, 255, 255, 255] }).collect();
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
        vectorize_comparison(&ls, &mask)
    })();
    result.unwrap();
}
#[test]
fn quarter_grid_ties_and_boundary_refusal_match_readback() {
    use tt_vector::geom::P;
    use tt_vector::path::{Anchor, Path};
    use tt_vector::planar::{EdgeKind, StrokeIn, build};
    use tt_vector::work::Work;
    let poly = rect(0.0, 0.0, 10.0, 10.0);
    let bez = loop_bez(&poly);
    let path = Path { anchors: poly.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed: true };
    let product =
        build(&[StrokeIn { id: "tie".into(), kind: EdgeKind::Pencil, path }], &mut Work::unlimited("r4")).unwrap();
    let oracle = SnapMap::build(vec![Input { id: "tie".into(), brush: false, edges: path_edges(&bez, true) }]);
    for x in [-1.0 / 512.0, 1.0 / 512.0, 0.0, 10.0 - 1.0 / 512.0, 10.0 + 1.0 / 512.0] {
        let q = Dp { x: (x * 256.0_f64).round() as i128, y: 5 * 256, s: 4 };
        let expected = oracle.locate(q);
        let actual = product.zone_at(P::new(x, 5.0));
        assert_eq!(actual.is_some(), expected.is_some(), "x={x} expected={expected:?} actual={actual:?}");
        if x == 0.0 {
            assert!(product.face_at(P::new(x, 5.0)).is_none());
        }
    }
}
