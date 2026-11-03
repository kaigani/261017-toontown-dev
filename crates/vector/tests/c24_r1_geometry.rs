//! Independent overlap lifetime and medial geometry probes.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::panic, missing_docs, reason = "review test")]
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tt_vector::ops::centre::{CENTRE_WORK_BUDGET, centre_line};
use tt_vector::planar::{
    build,
    fill::{self, Fill},
    layer::{inputs, settle, stored},
};
use tt_vector::{
    geom::P,
    path::{Anchor, Path},
    work::Work,
};

fn poly(points: &[(f64, f64)], closed: bool) -> Path {
    Path { anchors: points.iter().map(|&(x, y)| Anchor::corner(P::new(x, y))).collect(), closed }
}
fn stroke(id: &str, points: &[(f64, f64)], closed: bool) -> Value {
    json!({"id":id,"kind":"pencil","closed":closed,"points":poly(points,closed).points_json(),"sides":{"left":null,"right":null},"ext":{"org.review.opaque":{"colourId":"literal","nested":[1,{"x":false}]}}})
}
fn settled(before: Option<&[Value]>, after: &[Value], paint: &BTreeMap<String, Option<Fill>>) -> Vec<Value> {
    let mut i = 0;
    settle(
        before,
        after,
        None,
        paint,
        &mut || {
            i += 1;
            Ok::<_, ()>(format!("piece_{i}"))
        },
        &mut Work::unlimited("review"),
    )
    .unwrap()
    .strokes
}
fn check_readback(before: &[Value], after: &[Value]) {
    let old = build(&inputs(before), &mut Work::unlimited("review")).unwrap();
    let new = build(&inputs(after), &mut Work::unlimited("review")).unwrap();
    let oldfills = fill::read(&old, &stored(before));
    let newfills = fill::read(&new, &stored(after));
    assert!(newfills.inconsistent.is_empty(), "{:?}", newfills.inconsistent);
    for (i, z) in new.zones.iter().enumerate() {
        let want = old.zone_at(z.interior).and_then(|k| oldfills.fills[k].clone());
        assert_eq!(newfills.fills[i], want, "zone at {:?}", z.interior);
    }
}
#[test]
fn three_way_owner_deletion_split_and_import_keep_fill_and_opaque_namespaces() {
    let square = [(0., 0.), (100., 0.), (100., 100.), (0., 100.)];
    let original = vec![
        stroke("a", &square, true),
        stroke("b", &square, true),
        stroke("c", &square, true),
        stroke("fold", &[(-10., 50.), (50., 50.), (100., 50.), (50., 50.), (-10., 50.)], false),
    ];
    let base = settled(None, &original, &BTreeMap::new());
    let m = build(&inputs(&base), &mut Work::unlimited("review")).unwrap();
    assert_eq!(m.zones.len(), 2);
    let paint = m
        .zones
        .iter()
        .enumerate()
        .map(|(i, z)| (z.id.clone(), Some(Fill { colour_id: Some(format!("col_{i}")), matrix: None })))
        .collect();
    let painted = settled(Some(&base), &base, &paint);
    // Import pre-existing records through JSON without a previous map, then force noding.
    let imported: Vec<Value> = serde_json::from_str(&serde_json::to_string(&painted).unwrap()).unwrap();
    let loaded = settled(None, &imported, &BTreeMap::new());
    check_readback(&painted, &loaded);
    let mut remaining = loaded.clone();
    let loaded_map = build(&inputs(&loaded), &mut Work::unlimited("review")).unwrap();
    let edge = loaded_map.edges.iter().find(|e| e.srcs.len() >= 3).unwrap();
    let sources: Vec<_> = edge.srcs.iter().collect();
    let owning_piece = fill::owner(&loaded_map, &sources).unwrap();
    let owner = json!(loaded_map.strokes[owning_piece.stroke].0);
    remaining.retain(|s| s["id"] != owner);
    let removed = settled(Some(&loaded), &remaining, &BTreeMap::new());
    check_readback(&loaded, &removed);
    let mut cross = removed.clone();
    cross.push(stroke("cross", &[(25., -10.), (25., 110.)], false));
    let split = settled(Some(&removed), &cross, &BTreeMap::new());
    check_readback(&removed, &split);
    for s in &split {
        assert_eq!(s.pointer("/ext/org.review.opaque/nested"), Some(&json!([1,{"x":false}])));
    }
}

fn independent_area(points: &[(f64, f64)]) -> f64 {
    (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a.0 * b.1 - a.1 * b.0
        })
        .sum::<f64>()
        .abs()
        / 2.
}
#[test]
fn thin_neck_t_and_subunit_bars_have_finite_axes_and_independent_average_width() {
    let shapes = [
        vec![(0., 0.), (100., 0.), (100., 0.25), (0., 0.25)],
        vec![(0., 0.), (100., 0.), (100., 0.75), (0., 0.75)],
        vec![(0., 0.), (80., 0.), (80., 10.), (45., 10.), (45., 80.), (35., 80.), (35., 10.), (0., 10.)],
        vec![
            (0., 0.),
            (30., 0.),
            (30., 14.),
            (70., 14.),
            (70., 0.),
            (100., 0.),
            (100., 30.),
            (70., 30.),
            (70., 16.),
            (30., 16.),
            (30., 30.),
            (0., 30.),
        ],
    ];
    for (i, points) in shapes.iter().enumerate() {
        let axis = centre_line(&poly(points, true), &mut Work::new(CENTRE_WORK_BUDGET, "review centre")).unwrap();
        let len: f64 =
            axis.lines.iter().map(|l| l.flatten(0.001).windows(2).map(|p| p[0].dist(p[1])).sum::<f64>()).sum();
        let expected = independent_area(points) / len;
        println!("shape {i}: lines={} width={} independent={expected} length={len}", axis.lines.len(), axis.width);
        assert!(len.is_finite() && len > 0.);
        assert!((axis.width - expected).abs() <= expected * 0.05, "shape {i}: {} vs {expected}", axis.width);
    }
}
