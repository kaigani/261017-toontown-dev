// Round 2 seed copy from reviewed head; original assertions retained.
// Reviewer R1 variant: unchanged builder assertions, 4096 cases per property; seed bytes XOR 0x5D.
//! M2 C2.4 readiness item 2: topology properties of the planar map against the independent exact oracle
//! (`oracle/mod.rs`), on seeded degenerate arrangements with shrinking. Strokes join points of a small lattice, so
//! shared end points, T-junctions, collinear overlaps, coincident strokes and shared edges between closed shapes are
//! common. Properties: the zones are the oracle's bounded faces (exact areas within the snapping bound); maps with
//! determinant ±1 (quarter turns, reflections, whole-unit translations) keep the face areas; zone IDs and areas do not
//! depend on stroke order; adding strokes carries every fill into the zones it splits; side fills read back as written
//! after adding strokes, beside overlapping, retracing and folded strokes too (C24-T1); a settled layer is a fixpoint of
//! noding (C22-T2); interior points locate their zones; settling is idempotent.
//!
//! Every failing seed found during development is kept below as a fixed regression case (see the C2.4 handoff).
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::panic, missing_docs, reason = "test")]

mod oracle;

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{Value, json};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::EdgeKind;
use tt_vector::planar::fill::Fill;
use tt_vector::planar::layer::{inputs, path_of, settle};
use tt_vector::planar::node::{NodeStroke, node};
use tt_vector::planar::{LayerMap, build};
use tt_vector::work::Work;

fn runner(seed: &[u8; 32], _cases: u32) -> TestRunner {
    TestRunner::new_with_rng(
        Config { cases: 4096, failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed.map(|b| b ^ 0x5D)),
    )
}

/// A stroke: open or closed polyline through lattice points (index pairs on a 5 × 5 lattice of 10-du spacing).
type Spec = (bool, Vec<(i64, i64)>);

/// A map of lattice points.
type Map<'a> = &'a dyn Fn((i64, i64)) -> (i64, i64);

fn arrangement() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec((any::<bool>(), prop::collection::vec((0i64..5, 0i64..5), 2..6)), 1..7)
}

fn points(s: &Spec) -> Vec<(i64, i64)> {
    s.1.iter().map(|&(x, y)| (x * 10, y * 10)).collect()
}

fn closed(s: &Spec) -> bool {
    s.0 && s.1.len() >= 3
}

fn segments(list: &[Spec], map: &dyn Fn((i64, i64)) -> (i64, i64)) -> Vec<((i64, i64), (i64, i64))> {
    let mut out = Vec::new();
    for s in list {
        let p: Vec<(i64, i64)> = points(s).into_iter().map(map).collect();
        for w in p.windows(2) {
            out.push((w[0], w[1]));
        }
        if closed(s) {
            out.push((p[p.len() - 1], p[0]));
        }
    }
    out
}

fn stroke_json(list: &[Spec], map: &dyn Fn((i64, i64)) -> (i64, i64), offset: usize) -> Vec<Value> {
    list.iter()
        .enumerate()
        .map(|(i, s)| {
            let path = Path {
                anchors: points(s).into_iter().map(map).map(|(x, y)| Anchor::corner(P::new(x as f64, y as f64))).collect(),
                closed: closed(s),
            };
            json!({
                "id": format!("st_{:026}", i + offset), "type": "stroke", "rev": 1, "kind": "pencil", "closed": path.closed,
                "points": path.points_json(), "colourId": "col_ink", "fillMatrix": null,
                "lineStyle": {"thickness": 1, "lineType": "solid"}, "spine": null, "sides": {"left": null, "right": null}
            })
        })
        .collect()
}

fn settle_all(before: Option<&[Value]>, after: &[Value], painted: &BTreeMap<String, Option<Fill>>) -> Vec<Value> {
    let mut n = 0u32;
    let mut fresh = || -> Result<String, ()> {
        n += 1;
        Ok(format!("st_p{n:024}"))
    };
    settle(before, after, None, painted, &mut fresh, &mut Work::unlimited("t")).unwrap().strokes
}

fn map_of(strokes: &[Value]) -> LayerMap {
    build(&inputs(strokes), &mut Work::unlimited("t")).unwrap()
}

/// The product's zone areas against the oracle's faces. Junctions are snapped to the 1/64-du grid, so each area may
/// move by at most (vertices × longest edge ÷ 64) du²; on this lattice crossings are rarely off-grid.
fn check_against_oracle(list: &[Spec], map: &dyn Fn((i64, i64)) -> (i64, i64)) -> Result<(), TestCaseError> {
    let strokes = settle_all(None, &stroke_json(list, map, 0), &BTreeMap::new());
    let m = map_of(&strokes);
    let mut product: Vec<f64> = m.zones.iter().map(|z| z.area).collect();
    product.sort_by(f64::total_cmp);
    let faces: Vec<f64> = oracle::face_areas(&segments(list, map)).into_iter().map(oracle::Q::f64).collect();
    prop_assert_eq!(product.len(), faces.len(), "zone count: product {:?} oracle {:?}", product, faces);
    let segs = segments(list, map).len() as f64;
    for (a, b) in product.iter().zip(&faces) {
        prop_assert!(
            (a - b).abs() <= segs * 60.0 / 64.0 + 1e-9,
            "area {} vs oracle {} (product {:?}, oracle {:?})",
            a,
            b,
            product,
            faces
        );
    }
    for (z, zone) in m.zones.iter().enumerate() {
        prop_assert_eq!(m.zone_at(zone.interior), Some(z), "interior point of {}", zone.id);
    }
    Ok(())
}

#[test]
fn zones_are_the_oracles_bounded_faces_on_degenerate_arrangements() {
    runner(b"toontown-m2-c24-topology-props-1", 384)
        .run(&arrangement(), |list| check_against_oracle(&list, &|p| p))
        .unwrap();
}

#[test]
fn determinant_plus_and_minus_one_maps_keep_the_faces() {
    // Quarter turns, reflections and whole-unit translations: the oracle and the product agree on every image, and
    // the face areas of every image equal the original's.
    let maps: [Map<'_>; 5] =
        [&|(x, y)| (-y, x), &|(x, y)| (-x, -y), &|(x, y)| (-x, y), &|(x, y)| (y, x), &|(x, y)| (x - 113, y + 257)];
    runner(b"toontown-m2-c24-topology-props-2", 128)
        .run(&arrangement(), |list| {
            let base = oracle::face_areas(&segments(&list, &|p| p));
            for m in maps {
                check_against_oracle(&list, m)?;
                prop_assert_eq!(oracle::face_areas(&segments(&list, m)), base.clone(), "the image has the same faces");
                let strokes = settle_all(None, &stroke_json(&list, m, 0), &BTreeMap::new());
                let mut a: Vec<f64> = map_of(&strokes).zones.iter().map(|z| z.area).collect();
                a.sort_by(f64::total_cmp);
                let mut b: Vec<f64> = map_of(&settle_all(None, &stroke_json(&list, &|p| p, 0), &BTreeMap::new()))
                    .zones
                    .iter()
                    .map(|z| z.area)
                    .collect();
                b.sort_by(f64::total_cmp);
                prop_assert_eq!(a.len(), b.len());
                for (x, y) in a.iter().zip(&b) {
                    prop_assert!((x - y).abs() < 1e-6, "{} vs {}", x, y);
                }
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn zone_ids_and_areas_do_not_depend_on_stroke_order_and_settling_is_idempotent() {
    runner(b"toontown-m2-c24-topology-props-3", 256)
        .run(&arrangement(), |list| {
            let a = settle_all(None, &stroke_json(&list, &|p| p, 0), &BTreeMap::new());
            let mut rev = a.clone();
            rev.reverse();
            let key = |m: &LayerMap| {
                let mut v: Vec<(String, i128)> = m.zones.iter().map(|z| (z.id.clone(), z.area2)).collect();
                v.sort();
                v
            };
            prop_assert_eq!(key(&map_of(&a)), key(&map_of(&rev)));
            let again = settle_all(Some(&a), &a, &BTreeMap::new());
            prop_assert_eq!(&again, &a, "settling a settled layer changes nothing");
            Ok(())
        })
        .unwrap();
}

/// Carry-over (03 §8; C22-R1-02, C22-R2-04): with strokes only added, each new zone lies inside one old zone (or none),
/// so `fill::carry` gives it exactly that zone's fill — never a neighbour's through a snapping sliver.
#[test]
fn adding_strokes_carries_each_fill_into_the_zones_it_splits() {
    let pair = (arrangement(), arrangement());
    runner(b"toontown-m2-c24-topology-props-4", 192)
        .run(&pair, |(first, extra)| {
            let base = settle_all(None, &stroke_json(&first, &|p| p, 0), &BTreeMap::new());
            let old = map_of(&base);
            let fills: Vec<Option<Fill>> = (0..old.zones.len())
                .map(|i| Some(Fill { colour_id: Some(format!("col_{i:04}")), matrix: None }))
                .collect();
            let mut after = base.clone();
            after.extend(stroke_json(&extra, &|p| p, 100));
            let new = map_of(&settle_all(Some(&base), &after, &BTreeMap::new()));
            let carried = tt_vector::planar::fill::carry(&old, &fills, &new, &mut Work::unlimited("t")).unwrap();
            for (z, zone) in new.zones.iter().enumerate() {
                let expect = old.zone_at(zone.interior).and_then(|i| fills[i].clone());
                prop_assert_eq!(&carried[z], &expect, "zone {} at {:?}", zone.id, zone.interior);
            }
            Ok(())
        })
        .unwrap();
}

/// Paints every zone of `first` with its own colour, adds `extra`, and checks that every zone reads back the fill of the
/// old zone holding its interior point (none outside them).
fn side_fills_read_back(first: &[Spec], extra: &[Spec]) -> Result<(), TestCaseError> {
    let base = settle_all(None, &stroke_json(first, &|p| p, 0), &BTreeMap::new());
    let old = map_of(&base);
    let painted: BTreeMap<String, Option<Fill>> = old
        .zones
        .iter()
        .enumerate()
        .map(|(i, z)| (z.id.clone(), Some(Fill { colour_id: Some(format!("col_{i:04}")), matrix: None })))
        .collect();
    let painted_strokes = settle_all(Some(&base), &base, &painted);
    // Painting reads back as painted, and settling again changes nothing.
    let old_read = tt_vector::planar::fill::read(&old, &tt_vector::planar::layer::stored(&painted_strokes));
    for (i, zone) in old.zones.iter().enumerate() {
        prop_assert_eq!(&old_read.fills[i], &painted[&zone.id], "painted zone {}", zone.id);
    }
    prop_assert_eq!(&settle_all(Some(&painted_strokes), &painted_strokes, &BTreeMap::new()), &painted_strokes);
    let mut after = painted_strokes.clone();
    after.extend(stroke_json(extra, &|p| p, 100));
    let settled = settle_all(Some(&painted_strokes), &after, &BTreeMap::new());
    let new = map_of(&settled);
    let read = tt_vector::planar::fill::read(&new, &tt_vector::planar::layer::stored(&settled));
    prop_assert!(read.inconsistent.is_empty(), "inconsistent zones {:?}", read.inconsistent);
    for (z, zone) in new.zones.iter().enumerate() {
        let expect = old.zone_at(zone.interior).and_then(|i| old_read.fills[i].clone());
        prop_assert_eq!(&read.fills[z], &expect, "zone {} at {:?}", zone.id, zone.interior);
    }
    Ok(())
}

/// Side fills read back as written after adding strokes, also beside collinearly overlapping, retracing and folded
/// strokes (not split there, C22-R3-03): the owner rule (C24-T1 ruling, `fill::owner`) resolves every overlapped edge.
#[test]
fn side_fills_read_back_after_adding_strokes() {
    let pair = (arrangement(), arrangement());
    runner(b"toontown-m2-c24-topology-props-5", 256)
        .run(&pair, |(first, extra)| side_fills_read_back(&first, &extra))
        .unwrap();
}

/// Ticket C24-T1: the shrunk seed (a closed stroke retracing an edge another stroke overlaps; the added stroke closes a
/// new zone below the overlap) and retrace and fold variants, kept as permanent regressions.
#[test]
fn side_fills_read_back_beside_collinear_overlaps() {
    let cases: Vec<(Vec<Spec>, Vec<Spec>)> = vec![
        // The shrunk seed.
        (
            vec![
                (false, vec![(0, 0), (2, 3), (0, 3)]),
                (false, vec![(3, 2), (1, 3), (2, 3)]),
                (true, vec![(0, 3), (2, 3), (0, 3), (0, 1)]),
            ],
            vec![(false, vec![(0, 1), (0, 0)])],
        ),
        // Retrace: a stroke running back over another stroke's edge, partly (the overlap ends inside both).
        (
            vec![(true, vec![(0, 0), (4, 0), (4, 4), (0, 4)]), (false, vec![(1, 0), (3, 0), (3, 2), (1, 2), (1, 0)])],
            vec![(false, vec![(2, 0), (2, 4)])],
        ),
        // Retrace: two strokes overlapping in opposite directions along one edge of two squares.
        (
            vec![
                (true, vec![(0, 0), (2, 0), (2, 2), (0, 2)]),
                (true, vec![(4, 0), (4, 2), (2, 2), (2, 0)]),
                (false, vec![(2, 4), (2, 0)]),
            ],
            vec![(false, vec![(0, 1), (4, 1)])],
        ),
        // Fold: one open stroke turning back on itself at 180° along a square's edge.
        (
            vec![(true, vec![(0, 0), (3, 0), (3, 3), (0, 3)]), (false, vec![(0, 0), (3, 0), (1, 0), (1, 2)])],
            vec![(false, vec![(2, 0), (2, 3)])],
        ),
        // Fold inside a closed stroke (it retraces its own edge), split by an added stroke.
        (vec![(true, vec![(0, 2), (3, 2), (0, 2), (0, 0), (3, 0)])], vec![(false, vec![(3, 0), (3, 2)])]),
        // A fold whose two passes border different zones after the edit.
        (
            vec![(false, vec![(0, 2), (4, 2), (0, 2)]), (true, vec![(0, 0), (4, 0), (4, 4), (0, 4)])],
            vec![(false, vec![(2, 0), (2, 4)])],
        ),
    ];
    for (i, (first, extra)) in cases.iter().enumerate() {
        side_fills_read_back(first, extra).unwrap_or_else(|e| panic!("case {i}: {e}"));
    }
}

/// A settled layer is a fixpoint of noding (C22-T2): noding it again, its strokes marked changed or not, changes
/// nothing.
fn settled_is_a_fixpoint(list: &[Spec]) -> Result<(), TestCaseError> {
    let settled = settle_all(None, &stroke_json(list, &|p| p, 0), &BTreeMap::new());
    for changed in [false, true] {
        let input: Vec<NodeStroke> = settled
            .iter()
            .map(|v| NodeStroke {
                id: v["id"].as_str().unwrap().to_owned(),
                kind: EdgeKind::Pencil,
                path: path_of(v),
                changed,
            })
            .collect();
        let out = node(&input, &mut Work::unlimited("t")).unwrap();
        prop_assert!(out.outcomes.is_empty(), "changed = {}: {:?}", changed, out.outcomes);
    }
    Ok(())
}

#[test]
fn a_settled_layer_is_a_fixpoint_of_noding() {
    runner(b"toontown-m2-c24-topology-props-6", 512).run(&arrangement(), |list| settled_is_a_fixpoint(&list)).unwrap();
    // Shrunk seed (C22-T2): a crossing of two lines at a vertex of a third stroke retracing both was a junction of
    // the two only; the third was split at that vertex by the next noding (the new piece ends rest on it).
    settled_is_a_fixpoint(&[
        (false, vec![(1, 4), (1, 3), (0, 4)]),
        (false, vec![(1, 4), (1, 0)]),
        (false, vec![(0, 4), (2, 2)]),
    ])
    .unwrap();
}

/// Regression cases found during development (shrunk), kept as fixed tests.
#[test]
fn fixed_regressions() {
    let cases: Vec<Vec<Spec>> = vec![
        // Two coincident closed squares (duplicate strokes): one face.
        vec![(true, vec![(0, 0), (2, 0), (2, 2), (0, 2)]), (true, vec![(0, 0), (2, 0), (2, 2), (0, 2)])],
        // A square with a T-junction from a dangling segment and a collinear overlap on its edge.
        vec![
            (true, vec![(0, 0), (3, 0), (3, 3), (0, 3)]),
            (false, vec![(1, 0), (1, 2)]),
            (false, vec![(2, 0), (4, 0)]),
        ],
        // Two squares sharing an edge.
        vec![(true, vec![(0, 0), (1, 0), (1, 1), (0, 1)]), (true, vec![(1, 0), (2, 0), (2, 1), (1, 1)])],
        // Seed toontown-m2-c24-topology-props-2 (shrunk): a repeated anchor (zero-length segment) in a stroke crossed off
        // the grid gave a 0.078 du² sliver: the flattened edge after the repeat spanned the zero-length segment's
        // parameters, so a junction on it was placed inside that segment. Fixed in `flat::flatten` (vertex `out`).
        vec![(false, vec![(4, 0), (2, 3), (3, 3)]), (true, vec![(3, 4), (3, 4), (1, 1)])],
        vec![(false, vec![(4, 0), (2, 3), (3, 3)]), (false, vec![(3, 4), (3, 4), (1, 1)])],
        // A square inside a square (a hole component).
        vec![(true, vec![(0, 0), (4, 0), (4, 4), (0, 4)]), (true, vec![(1, 1), (2, 1), (2, 2), (1, 2)])],
    ];
    for c in &cases {
        check_against_oracle(c, &|p| p).unwrap();
    }
}

/// Intentionally failing fixed reproduction shrunk from reviewer seed XOR A7.
#[test]
fn shrunk_determinant_case() {
    let list = vec![
        (true, vec![(1, 1), (0, 0), (3, 4), (1, 1)]),
        (false, vec![(4, 1), (1, 2), (0, 0)]),
        (false, vec![(1, 4), (2, 1)]),
    ];
    let maps: [Map<'_>; 6] =
        [&|(x, y)| (x, y), &|(x, y)| (-y, x), &|(x, y)| (-x, -y), &|(x, y)| (-x, y), &|(x, y)| (y, x), &|(x, y)| {
            (x - 113, y + 257)
        }];
    let mut areas = Vec::new();
    for (i, m) in maps.iter().enumerate() {
        let s = settle_all(None, &stroke_json(&list, m, 0), &BTreeMap::new());
        let map = map_of(&s);
        let mut a: Vec<f64> = map.zones.iter().map(|z| z.area).collect();
        a.sort_by(f64::total_cmp);
        println!("map {i}: areas={a:?}; strokes={}", serde_json::to_string(&s).unwrap());
        areas.push(a);
    }
    for a in &areas[1..] {
        assert_eq!(a, &areas[0], "determinant ?1 and whole-grid translation preserve face area");
    }
}

#[test]
fn integer_translations_and_compositions_preserve_all_snapped_anchor_sequences() {
    let list = vec![
        (true, vec![(1, 1), (0, 0), (3, 4), (1, 1)]),
        (false, vec![(4, 1), (1, 2), (0, 0)]),
        (false, vec![(1, 4), (2, 1)]),
    ];
    let normalize = |strokes: &[Value], dx: i64, dy: i64| {
        let mut paths: Vec<Vec<(i64, i64)>> = strokes
            .iter()
            .map(|s| {
                s["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        (
                            ((p[0].as_f64().unwrap() - dx as f64) * 64.).round() as i64,
                            ((p[1].as_f64().unwrap() - dy as f64) * 64.).round() as i64,
                        )
                    })
                    .collect()
            })
            .collect();
        paths.sort();
        paths
    };
    let base = settle_all(None, &stroke_json(&list, &|p| p, 0), &BTreeMap::new());
    let want = normalize(&base, 0, 0);
    for (dx, dy) in [(37, -11), (-113, 257), (293, -282), (10000, -9000), (-1, 1)] {
        let translated = settle_all(None, &stroke_json(&list, &|(x, y)| (x + dx, y + dy), 0), &BTreeMap::new());
        assert_eq!(normalize(&translated, dx, dy), want, "translation {dx},{dy}");
    }
    // Composition T(-113,257) then T(406,-539) equals T(293,-282), including a second settle.
    let first = settle_all(None, &stroke_json(&list, &|(x, y)| (x - 113, y + 257), 0), &BTreeMap::new());
    let mut moved = first.clone();
    for s in &mut moved {
        for p in s["points"].as_array_mut().unwrap() {
            p[0] = json!(p[0].as_f64().unwrap() + 406.);
            p[1] = json!(p[1].as_f64().unwrap() - 539.);
        }
    }
    let twice = settle_all(Some(&first), &moved, &BTreeMap::new());
    assert_eq!(normalize(&twice, 293, -282), want);
}

#[test]
fn independent_snapping_envelope_matches_a_rectangles_analytic_formula() {
    let sides = vec![((0, 0), (13, 0)), ((13, 0), (13, 7)), ((13, 7), (0, 7)), ((0, 7), (0, 0))];
    let delta = std::f64::consts::SQRT_2 / 128. + 1. / 32.;
    let expected = 40. * delta + 4. * delta * delta;
    assert!((oracle::snapping_envelope(&sides, 64) - expected).abs() < 1e-12);
    let moved = sides.iter().map(|&(a, b)| ((a.0 - 43, a.1 + 71), (b.0 - 43, b.1 + 71))).collect::<Vec<_>>();
    assert_eq!(oracle::snapping_envelope(&sides, 64), oracle::snapping_envelope(&moved, 64));
}
