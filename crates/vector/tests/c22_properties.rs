//! Property tests of the planar map (M2 C2.2): the derived graph is deterministic and independent of stroke order,
//! interior points locate their zones, settling is idempotent (noded layers stay noded, unchanged strokes keep their
//! IDs), painted fills read back exactly, and a zone whose boundary an edit leaves alone keeps its ID and its fill.
//! Seeds are fixed (proptest's ChaCha RNG with a recorded seed), so a failure reproduces on every machine.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use serde_json::{Value, json};
use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::build;
use tt_vector::planar::fill::Fill;
use tt_vector::planar::layer::{inputs, settle};
use tt_vector::work::Work;

fn runner(cases: u32) -> TestRunner {
    let seed: [u8; 32] = *b"toontown-m2-c22-planar-props-001";
    TestRunner::new_with_rng(
        Config { cases, failure_persistence: None, ..Config::default() },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed),
    )
}

/// One stroke: kind (0 pencil, 1 invisible, 2 brush), closed, points on a 1 du lattice (exact, reproducible).
type Spec = (u8, bool, Vec<(i32, i32)>);

fn strokes() -> impl Strategy<Value = Vec<Spec>> {
    prop::collection::vec((0u8..3, any::<bool>(), prop::collection::vec((-40i32..40, -40i32..40), 2..7)), 1..7)
}

fn to_json(list: &[Spec]) -> Vec<Value> {
    let mut out = Vec::new();
    for (i, (kind, closed, pts)) in list.iter().enumerate() {
        let path = Path {
            anchors: pts.iter().map(|&(x, y)| Anchor::corner(P::new(f64::from(x), f64::from(y)))).collect(),
            closed: *closed && pts.len() >= 3,
        };
        let (k, points, closed) = match kind {
            2 => {
                let pen = BrushPen { tip: Tip::Round, min_size: 3.0, max_size: 3.0, tip_angle: 0.0 };
                match outline(&Path { closed: false, ..path.clone() }, &[[0.0, 1.0], [1.0, 1.0]], &pen) {
                    Ok(o) => ("brush", o.points_json(), true),
                    Err(_) => continue,
                }
            }
            1 => ("invisible", path.points_json(), path.closed),
            _ => ("pencil", path.points_json(), path.closed),
        };
        out.push(json!({
            "id": format!("st_{i:026}"), "type": "stroke", "rev": 1, "kind": k, "closed": closed, "points": points,
            "colourId": if k == "invisible" { Value::Null } else { json!("col_ink") }, "fillMatrix": null,
            "lineStyle": if k == "pencil" { json!({"thickness": 1, "lineType": "solid"}) } else { Value::Null },
            "spine": null, "sides": {"left": null, "right": null}
        }));
    }
    out
}

fn settled(
    before: Option<&[Value]>,
    after: &[Value],
    painted: &BTreeMap<String, Option<Fill>>,
) -> tt_vector::planar::layer::Settled {
    let mut n = 0u32;
    let mut fresh = || -> Result<String, ()> {
        n += 1;
        Ok(format!("st_p{n:024}"))
    };
    settle(before, after, None, painted, &mut fresh, &mut Work::unlimited("t")).unwrap()
}

fn red() -> Option<Fill> {
    Some(Fill { colour_id: Some("col_red".into()), matrix: None })
}

#[test]
fn the_map_is_deterministic_order_independent_and_interior_points_locate_their_zones() {
    runner(200)
        .run(&strokes(), |list| {
            let v = to_json(&list);
            let s = settled(None, &v, &BTreeMap::new()).strokes;
            let a = build(&inputs(&s), &mut Work::unlimited("t")).unwrap();
            prop_assert_eq!(&a, &build(&inputs(&s), &mut Work::unlimited("t")).unwrap());
            let mut rev = s.clone();
            rev.reverse();
            let b = build(&inputs(&rev), &mut Work::unlimited("t")).unwrap();
            let ids = |m: &tt_vector::planar::LayerMap| {
                let mut v: Vec<(String, i128)> = m.zones.iter().map(|z| (z.id.clone(), z.area2)).collect();
                v.sort();
                v
            };
            prop_assert_eq!(ids(&a), ids(&b), "zone IDs and areas do not depend on stroke order");
            for (z, zone) in a.zones.iter().enumerate() {
                prop_assert!(zone.area > 0.0);
                prop_assert_eq!(a.zone_at(zone.interior), Some(z), "interior point of {}", zone.id);
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn settling_is_idempotent_and_painted_fills_read_back() {
    runner(200)
        .run(&strokes(), |list| {
            let v = to_json(&list);
            let first = settled(None, &v, &BTreeMap::new());
            let again = settled(Some(&first.strokes), &first.strokes, &BTreeMap::new());
            prop_assert!(!again.changed, "a settled layer is noded");
            prop_assert!(again.created.is_empty());
            let painted: BTreeMap<String, Option<Fill>> =
                first.map.zones.iter().map(|z| (z.id.clone(), red())).collect();
            let p = settled(Some(&first.strokes), &first.strokes, &painted);
            prop_assert!(p.fills.iter().all(|f| *f == red()));
            let back = settled(Some(&p.strokes), &p.strokes, &BTreeMap::new());
            prop_assert!(!back.changed, "stored fills rewrite to the same bytes");
            prop_assert!(back.fills.iter().all(|f| *f == red()), "{:?}", back.fills);
            Ok(())
        })
        .unwrap();
}

#[test]
fn zones_an_edit_leaves_alone_keep_their_id_and_fill() {
    runner(150)
        .run(&(strokes(), (-40i32..40, -40i32..40, -40i32..40, -40i32..40)), |(list, (x0, y0, x1, y1))| {
            let v = to_json(&list);
            let first = settled(None, &v, &BTreeMap::new());
            let painted: BTreeMap<String, Option<Fill>> = first.map.zones.iter().map(|z| (z.id.clone(), red())).collect();
            let p = settled(Some(&first.strokes), &first.strokes, &painted);
            let mut after = p.strokes.clone();
            after.push(json!({
                "id": "st_cut000000000000000000000", "type": "stroke", "rev": 0, "kind": "invisible", "closed": false,
                "points": [[x0, y0, 0, 0, 0, 0], [x1 + 100, y1 + 100, 0, 0, 0, 0]], "colourId": null, "fillMatrix": null,
                "lineStyle": null, "spine": null, "sides": {"left": null, "right": null}
            }));
            if x0 == x1 + 100 && y0 == y1 + 100 {
                return Ok(());
            }
            let e = settled(Some(&p.strokes), &after, &BTreeMap::new());
            let before: BTreeMap<&str, &Option<Fill>> = p.map.zones.iter().map(|z| z.id.as_str()).zip(p.fills.iter()).collect();
            for (z, zone) in e.map.zones.iter().enumerate() {
                if let Some(f) = before.get(zone.id.as_str()) {
                    prop_assert_eq!(&e.fills[z], *f, "zone {} kept its boundary, so its fill", zone.id);
                }
            }
            Ok(())
        })
        .unwrap();
}
