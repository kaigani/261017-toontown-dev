//! Settling a layer after an edit (M2 C2.2): persistent noding, fill carry-over, painted fills, brush exterior records
//! (SI-059, SI-060).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tt_model::entities::pen::Tip;
use tt_vector::brush::{BrushPen, outline};
use tt_vector::geom::P;
use tt_vector::path::{Anchor, Path};
use tt_vector::planar::fill::Fill;
use tt_vector::planar::layer::{Settled, settle};
use tt_vector::work::Work;

fn stroke(id: &str, kind: &str, pts: &[(f64, f64)], closed: bool) -> Value {
    let points: Vec<Value> = pts.iter().map(|&(x, y)| json!([x, y, 0.0, 0.0, 0.0, 0.0])).collect();
    json!({
        "id": id, "type": "stroke", "rev": 1, "kind": kind, "closed": closed, "points": points,
        "colourId": if kind == "invisible" { Value::Null } else { json!("col_ink") },
        "fillMatrix": null,
        "lineStyle": if kind == "pencil" { json!({"thickness": 1, "lineType": "solid"}) } else { Value::Null },
        "spine": null, "sides": {"left": null, "right": null}
    })
}

fn brush(id: &str, path: &Path) -> Value {
    json!({
        "id": id, "type": "stroke", "rev": 1, "kind": "brush", "closed": true, "points": path.points_json(),
        "colourId": "col_ink", "fillMatrix": null, "lineStyle": null,
        "spine": {"points": [[-50, 0, 0, 0, 0, 0], [50, 0, 0, 0, 0, 0]], "pressure": [[0, 1], [1, 1]],
                  "pen": {"penId": null, "tip": "round", "minSize": 10, "maxSize": 10}},
        "sides": {"left": null, "right": null}
    })
}

fn red() -> Option<Fill> {
    Some(Fill { colour_id: Some("col_red".into()), matrix: None })
}

fn run(before: Option<&[Value]>, after: &[Value], painted: &BTreeMap<String, Option<Fill>>) -> Settled {
    let mut n = 0;
    let mut fresh = || -> Result<String, ()> {
        n += 1;
        Ok(format!("st_piece{n}"))
    };
    settle(before, after, None, painted, &mut fresh, &mut Work::unlimited("t")).unwrap()
}

fn square() -> Value {
    stroke("st_sq", "pencil", &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], true)
}

#[test]
fn painting_a_zone_writes_its_fill_on_the_bounding_sides() {
    let layer = vec![square()];
    let s = run(Some(&layer), &layer, &BTreeMap::new());
    assert!(!s.changed);
    assert_eq!(s.map.zones.len(), 1);
    let id = s.map.zones[0].id.clone();
    let painted = run(Some(&layer), &layer, &BTreeMap::from([(id, red())]));
    assert!(painted.changed);
    assert_eq!(painted.strokes[0]["sides"]["left"], json!({"colourId": "col_red", "matrix": null}));
    // Settling the painted layer again changes nothing (same input → same bytes).
    let again = run(Some(&painted.strokes), &painted.strokes, &BTreeMap::new());
    assert!(!again.changed);
    assert_eq!(again.fills, vec![red()]);
}

#[test]
fn a_crossing_line_splits_strokes_and_both_zone_parts_keep_the_fill() {
    let layer = vec![square()];
    let id = run(Some(&layer), &layer, &BTreeMap::new()).map.zones[0].id.clone();
    let painted = run(Some(&layer), &layer, &BTreeMap::from([(id, red())])).strokes;
    let mut after = painted.clone();
    after.push(stroke("st_cut", "invisible", &[(40.0, -20.0), (40.0, 120.0)], false));
    let s = run(Some(&painted), &after, &BTreeMap::new());
    assert_eq!(s.removed, vec!["st_sq", "st_cut"]);
    assert_eq!(s.created.len(), 2 + 3, "{:?}", s.created);
    assert_eq!(s.map.zones.len(), 2);
    assert_eq!(s.fills, vec![red(), red()]);
    // Settled strokes are noded: settling them again changes nothing.
    let again = run(Some(&s.strokes), &s.strokes, &BTreeMap::new());
    assert!(!again.changed);
    assert!(again.created.is_empty());
}

#[test]
fn a_pencil_crossing_a_brush_splits_at_the_outline_and_leaves_the_brush_byte_identical() {
    let spine =
        Path { anchors: vec![Anchor::corner(P::new(-50.0, 0.0)), Anchor::corner(P::new(50.0, 0.0))], closed: false };
    let pen = BrushPen { tip: Tip::Round, min_size: 10.0, max_size: 10.0, tip_angle: 0.0 };
    let bar = brush("st_bar", &outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen).unwrap());
    let before = vec![bar.clone()];
    let after = vec![bar.clone(), stroke("st_line", "pencil", &[(0.0, -30.0), (0.0, 30.0)], false)];
    let s = run(Some(&before), &after, &BTreeMap::new());
    assert_eq!(s.strokes[0], bar, "SI-059: noding never touches a brush stroke");
    assert_eq!(s.removed, vec!["st_line"]);
    assert_eq!(s.created.len(), 3, "the pencil splits at both crossings of the outline");
    let ends: Vec<f64> = s.strokes[1..].iter().map(|v| v["points"][0][1].as_f64().unwrap()).collect();
    assert!((ends[1] - -5.0).abs() < 0.05 && (ends[2] - 5.0).abs() < 0.05, "{ends:?}");
}

#[test]
fn a_brush_only_zone_keeps_its_fill_in_exterior_records() {
    let k = 0.552_284_749_830_793_4 * 40.0;
    let spine = Path {
        anchors: vec![
            Anchor { p: P::new(40.0, 0.0), inh: P::new(0.0, -k), outh: P::new(0.0, k) },
            Anchor { p: P::new(0.0, 40.0), inh: P::new(k, 0.0), outh: P::new(-k, 0.0) },
            Anchor { p: P::new(-40.0, 0.0), inh: P::new(0.0, k), outh: P::new(0.0, -k) },
            Anchor { p: P::new(0.0, -40.0), inh: P::new(-k, 0.0), outh: P::new(k, 0.0) },
        ],
        closed: true,
    };
    let pen = BrushPen { tip: Tip::Round, min_size: 8.0, max_size: 8.0, tip_angle: 0.0 };
    let ring = brush("st_ring", &outline(&spine, &[[0.0, 1.0], [1.0, 1.0]], &pen).unwrap());
    let layer = vec![ring];
    let s = run(Some(&layer), &layer, &BTreeMap::new());
    assert_eq!(s.map.zones.len(), 1);
    let id = s.map.zones[0].id.clone();
    let painted = run(Some(&layer), &layer, &BTreeMap::from([(id, red())]));
    let records = &painted.strokes[0]["ext"]["dev.toontown.zones"]["exterior"];
    assert!(records.as_array().is_some_and(|a| !a.is_empty()), "{}", painted.strokes[0]);
    let again = run(Some(&painted.strokes), &painted.strokes, &BTreeMap::new());
    assert!(!again.changed, "records rewrite to the same bytes");
    assert_eq!(again.fills, vec![red()]);
    // The outline and spine are untouched by painting.
    assert_eq!(painted.strokes[0]["points"], layer[0]["points"]);
    assert_eq!(painted.strokes[0]["spine"], layer[0]["spine"]);
}
