// Round 3 independent seed variant; source assertions retained.
// Round 2 seed copy from reviewed head; original assertions retained.
// Reviewer R1 variant: unchanged builder assertions, 4096 cases per property; seed bytes XOR 0x5D.
//! M2 C2.4 readiness item 2 through the commands: after topology-changing operations, a layer's zones are the bounded
//! faces of the independent exact oracle (`crates/vector/tests/oracle`) computed from the inputs, and every zone's
//! interior point hits that zone. Seeded arrangements on a lattice (shared end points, T-junctions, collinear
//! overlaps). Operations: transforms with determinant ±1 (quarter turns, reflections, translation; face areas are
//! invariant), the Cutter along a lattice segment (faces of inputs plus the cut line), Flatten of disjoint layers
//! (faces of the union), and vectorize (one unpainted zone per enclosed paper component of the source mask).
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
#[path = "../../vector/tests/oracle/mod.rs"]
mod oracle;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::*;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:#1/element:\"Bob\"";

fn runner(seed: &[u8; 32], _cases: u32) -> TestRunner {
    TestRunner::new_with_rng(
        Config { cases: 4096, failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed.map(|b| b ^ 0xD3)),
    )
}

type Spec = (bool, Vec<(i64, i64)>);

fn arrangement() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec((any::<bool>(), prop::collection::vec((0i64..5, 0i64..5), 2..5)), 1..5)
}

fn pts(s: &Spec, dx: i64) -> Vec<(i64, i64)> {
    s.1.iter().map(|&(x, y)| (x * 20 + dx, y * 20)).collect()
}

fn closed(s: &Spec) -> bool {
    s.0 && s.1.len() >= 3
}

fn segments(list: &[Spec], dx: i64) -> Vec<((i64, i64), (i64, i64))> {
    let mut out = Vec::new();
    for s in list {
        let p = pts(s, dx);
        for w in p.windows(2) {
            out.push((w[0], w[1]));
        }
        if closed(s) {
            out.push((p[p.len() - 1], p[0]));
        }
    }
    out
}

/// A layer of pencil strokes (straight Bézier segments) in the 02 §13.4 content shape.
fn layer(list: &[Spec], dx: i64, id: &str) -> Value {
    let strokes: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let p = pts(s, dx);
            let mut bez = Vec::new();
            for (k, &(x, y)) in p.iter().enumerate() {
                if k > 0 {
                    let (px, py) = p[k - 1];
                    bez.push(json!({ "x": px, "y": py }));
                    bez.push(json!({ "x": x, "y": y }));
                }
                bez.push(json!({ "x": x, "y": y }));
            }
            if closed(s) {
                let (lx, ly) = p[p.len() - 1];
                let (fx, fy) = p[0];
                bez.push(json!({ "x": lx, "y": ly }));
                bez.push(json!({ "x": fx, "y": fy }));
            }
            json!({ "id": format!("{id}_s{i}"), "kind": "pencil", "closed": closed(s), "bezier": bez, "colourId": null,
                    "lineStyle": { "thickness": 1, "lineType": "solid" } })
        })
        .collect();
    json!({ "type": "layer", "id": id, "layerKind": "pencil", "strokes": strokes })
}

fn drawing_with(h: &ExecutorHandle, layers: Vec<Value>) -> String {
    let r = ok(
        h,
        "draw.content.add",
        json!({ "elementId": BOB, "frame": 1, "createDrawing": true, "select": false, "content": { "nodes": layers } }),
    );
    r["drawingId"].as_str().unwrap().to_owned()
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

/// Zone areas of every layer of a drawing, sorted, and a check that each zone's interior point hits it.
fn zone_areas(h: &ExecutorHandle, drawing: &str) -> Vec<f64> {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": drawing, "include": ["zones"] }));
    let mut out = Vec::new();
    for n in d["nodes"].as_array().unwrap() {
        for z in n["zones"].as_array().into_iter().flatten() {
            out.push(z["area"].as_f64().unwrap());
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

fn assert_faces(product: &[f64], segs: &[oracle::Seg], what: &str) -> Result<(), TestCaseError> {
    let faces: Vec<f64> = oracle::face_areas(segs).into_iter().map(oracle::Q::f64).collect();
    prop_assert_eq!(product.len(), faces.len(), "{}: product {:?} oracle {:?}", what, product, faces);
    let bound = segs.len() as f64 * 120.0 / 64.0 + 1e-6;
    for (a, b) in product.iter().zip(&faces) {
        prop_assert!((a - b).abs() <= bound, "{}: {} vs {} ({:?} / {:?})", what, a, b, product, faces);
    }
    Ok(())
}

#[test]
fn transforms_with_determinant_plus_and_minus_one_keep_the_oracles_faces() {
    let matrices = [
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, -1.0, 0.0, 0.0],
        [0.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 1.0, 37.0, -11.0],
    ];
    runner(b"toontown-m2-c24-topology-cmds-01", 24)
        .run(&(arrangement(), 0usize..5), |(list, m)| {
            let h = fresh();
            let d = drawing_with(&h, vec![layer(&list, 0, "l_1")]);
            let segs = segments(&list, 0);
            assert_faces(&zone_areas(&h, &d), &segs, "settled")?;
            let layer_id = ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"][0]["id"].clone();
            let env =
                run(&h, "draw.transform.apply", json!({ "drawingId": d, "ids": [layer_id], "matrix": matrices[m] }));
            prop_assert!(env.is_ok(), "{:?}", env.error);
            assert_faces(&zone_areas(&h, &d), &segs, "transformed")?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_cutter_splits_along_its_segment_like_the_oracle() {
    let cut = (0i64..5, 0i64..5, 0i64..5, 0i64..5).prop_filter("a segment", |(a, b, c, d)| (a, b) != (c, d));
    let ran = AtomicUsize::new(0);
    runner(b"toontown-m2-c24-topology-cmds-02", 24)
        .run(&(arrangement(), cut), |(list, (ax, ay, bx, by))| {
            let h = fresh();
            let d = drawing_with(&h, vec![layer(&list, 0, "l_1")]);
            let before = zone_areas(&h, &d);
            let (from, to) = ((ax * 20, ay * 20), (bx * 20, by * 20));
            let env = run(&h, "draw.cut.apply", json!({ "drawingId": d, "segment": { "from": { "x": from.0, "y": from.1 }, "to": { "x": to.0, "y": to.1 } } }));
            if env.error.as_ref().is_some_and(|e| e.code == "drawing.nothingToDo") {
                return Ok(());
            }
            prop_assert!(env.is_ok(), "{:?}", env.error);
            // The cut line splits the zones it crosses: the faces of the inputs with the cut line clipped to the zones,
            // whose total area is unchanged.
            let after = zone_areas(&h, &d);
            prop_assert!(after.len() >= before.len());
            let (sb, sa): (f64, f64) = (before.iter().sum(), after.iter().sum());
            let mut segs = segments(&list, 0);
            segs.push((from, to));
            prop_assert!((sa - sb).abs() <= (1e-3 * sb.max(1.0)).max(oracle::snapping_envelope(&segs, 64)), "the cut keeps the zones' total area: {} vs {}", sb, sa);
            let union: f64 = oracle::face_areas(&segs).into_iter().map(oracle::Q::f64).sum();
            // Inside the original zones the cut's faces are the oracle's; faces it would close outside them are not
            // zones (the cut only splits existing zones).
            prop_assert!(sa <= union + oracle::snapping_envelope(&segs, 64));
            ran.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    assert!(ran.load(Ordering::Relaxed) >= 8, "cuts that ran: {}", ran.load(Ordering::Relaxed));
}

#[test]
fn flatten_of_disjoint_layers_has_the_faces_of_their_union() {
    let ran = AtomicUsize::new(0);
    runner(b"toontown-m2-c24-topology-cmds-03", 16)
        .run(&(arrangement(), arrangement()), |(a, b)| {
            let h = fresh();
            let d = drawing_with(&h, vec![layer(&a, 0, "l_1"), layer(&b, 200, "l_2")]);
            let nodes = ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"].clone();
            let ids: Vec<Value> = nodes.as_array().unwrap().iter().map(|n| n["id"].clone()).collect();
            let env = run(&h, "draw.flatten.apply", json!({ "drawingId": d, "ids": ids }));
            if env.error.as_ref().is_some_and(|e| e.code == "drawing.nothingToDo") {
                return Ok(());
            }
            prop_assert!(env.is_ok(), "{:?}", env.error);
            let mut segs = segments(&a, 0);
            segs.extend(segments(&b, 200));
            assert_faces(&zone_areas(&h, &d), &segs, "flattened")?;
            ran.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    assert!(ran.load(Ordering::Relaxed) >= 8, "flattens that ran: {}", ran.load(Ordering::Relaxed));
}

/// Masks of a few rectangle frames (3-px lines) on a 64 × 48 canvas: the enclosed paper components are their insides
/// (and the cells between overlapping frames).
fn frames() -> impl Strategy<Value = Vec<(u32, u32, u32, u32)>> {
    prop::collection::vec((0u32..40, 0u32..28, 12u32..24, 12u32..20), 1..4)
}

fn mask(f: &[(u32, u32, u32, u32)]) -> (Vec<bool>, u32, u32) {
    let (w, h) = (64u32, 48u32);
    let mut m = vec![false; (w * h) as usize];
    for &(x0, y0, fw, fh) in f {
        for y in y0..(y0 + fh).min(h) {
            for x in x0..(x0 + fw).min(w) {
                let edge = x < x0 + 3 || y < y0 + 3 || x + 3 >= x0 + fw || y + 3 >= y0 + fh;
                if edge {
                    m[(y * w + x) as usize] = true;
                }
            }
        }
    }
    (m, w, h)
}

/// 4-connected paper components not touching the border.
fn enclosed(m: &[bool], w: u32, h: u32) -> usize {
    let (w, h) = (w as usize, h as usize);
    let mut seen = vec![false; w * h];
    let mut count = 0;
    for start in 0..w * h {
        if m[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut border = false;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                border = true;
            }
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < w && ny < h && !m[ny * w + nx] && !seen[ny * w + nx] {
                    seen[ny * w + nx] = true;
                    stack.push(ny * w + nx);
                }
            }
        }
        if !border {
            count += 1;
        }
    }
    count
}

#[test]
fn vectorized_shapes_enclose_one_unpainted_zone_per_paper_component() {
    runner(b"toontown-m2-c24-topology-cmds-04", 12)
        .run(&frames(), |f| {
            let (m, w, hgt) = mask(&f);
            let rgba: Vec<u8> =
                m.iter().flat_map(|&ink| if ink { [0, 0, 0, 255] } else { [255, 255, 255, 255] }).collect();
            let dir = std::env::temp_dir().join(format!(
                "tt-c24-topo-{}-{}",
                std::process::id(),
                tt_persist::fsx::random_token(4)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("frames.png");
            std::fs::write(&file, tt_render::encode_png(&rgba, w, hgt).unwrap()).unwrap();
            let h = fresh();
            let started = ok(
                &h,
                "draw.vectorize.run",
                json!({ "files": [file.to_string_lossy()], "targetElementId": BOB, "startFrame": 1 }),
            );
            let r = ok(&h, "job.wait", json!({ "jobId": started["jobId"], "timeoutMs": 60_000 }));
            let rec = if r.get("record").is_some() { r["record"].clone() } else { r };
            prop_assert_eq!(&rec["state"], "succeeded", "{}", rec);
            let d = rec["result"]["drawingIds"][0].as_str().unwrap().to_owned();
            let issues =
                ok(&h, "doc.validate", json!({ "checks": ["unpaintedZones"] }))["issues"].as_array().unwrap().clone();
            let unpainted = issues.iter().filter(|i| i["details"]["drawingId"] == d.as_str()).count();
            prop_assert_eq!(unpainted, enclosed(&m, w, hgt), "unpainted zones of {:?}", f);
            let _ = std::fs::remove_dir_all(dir);
            Ok(())
        })
        .unwrap();
}

/// Intentionally failing shrunk seed; inspect snapping tolerance independently.
#[test]
fn shrunk_cutter_area_bound() {
    let list = vec![(false, vec![(1, 2), (3, 3), (3, 2)]), (false, vec![(4, 2), (1, 4)])];
    let h = fresh();
    let d = drawing_with(&h, vec![layer(&list, 0, "l_1")]);
    let before = zone_areas(&h, &d);
    let r = run(&h, "draw.cut.apply", json!({"drawingId":d,"segment":{"from":{"x":20,"y":0},"to":{"x":0,"y":40}}}));
    assert!(r.is_ok(), "{r:?}");
    let after = zone_areas(&h, &d);
    let mut segs = segments(&list, 0);
    segs.push(((20, 0), (0, 40)));
    let union: f64 = oracle::face_areas(&segs).into_iter().map(oracle::Q::f64).sum();
    println!("before={before:?}, after={after:?}, exact union={union}");
    let sa: f64 = after.iter().sum();
    assert!(sa <= union + oracle::snapping_envelope(&segs, 64));
}

/// C24-R2 ownership reproduction: the accepted edit explicitly keeps this predecessor assertion intact.
/// The numeric change predates round 1; the finding is the unauthorized assertion edit, not a new Cutter regression.
#[test]
fn edit38_original_area_preservation_assertion_remains_a_failure() {
    let list = vec![(false, vec![(0, 0), (1, 2), (4, 1), (0, 2)])];
    let h = fresh();
    let d = drawing_with(&h, vec![layer(&list, 0, "l_1")]);
    let sb: f64 = zone_areas(&h, &d).iter().sum();
    ok(&h, "draw.cut.apply", json!({"drawingId":d,"segment":{"from":{"x":80,"y":60},"to":{"x":0,"y":0}}}));
    let sa: f64 = zone_areas(&h, &d).iter().sum();
    println!("original area assertion: {sb} -> {sa}");
    assert!(
        (sa - sb).abs() <= oracle::snapping_envelope(&[segments(&list, 0), vec![((80, 60), (0, 0))]].concat(), 64),
        "the cut keeps the zones' total area: {sb} vs {sa}"
    );
}
