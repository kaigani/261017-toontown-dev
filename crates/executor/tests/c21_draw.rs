//! M2 C2.1 drawing commands (02 §13.5): strokes, text, groups, delete, pivot, content insertion and read-back on the
//! fixture project — targeting (§13.1 item 4), createDrawing (§1.1), locked drawings, drawing revisions and
//! `drawing.staleRevision` (§3.12), stable IDs, undo/redo, geometry errors, and every result against its schema.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;
#[path = "common/recorded.rs"]
mod recorded;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const BOB: &str = "scene:#1/element:\"Bob\"";
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const D2_LOCKED: &str = "dr_01K742SG0M5J3BAJSW8241WA87";
const PENCIL: &str = "st_01K742SG0R61SYK9GG0HGYVNBT";
const INVISIBLE: &str = "st_01K742SG0S0NT23BWCZN64SFQ5";
const BRUSH: &str = "st_01K742SG0QD1F2HFZSX8R4V7WP";
const GROUP: &str = "nd_01K742SG0TX3WF1BSWWGMWM5WG";
const CAPTION: &str = "nd_01K742SG0WFAQJFKAZKV4CD56C";

/// Runs a command, checks success and the result against the declared result schema.
fn okv(h: &ExecutorHandle, id: &str, params: Value) -> Value {
    let r = ok(h, id, params);
    let schema = tt_executor::registry().unwrap().get(id).unwrap().result_schema();
    let errors = recorded::schema_errors(&schema, &r);
    assert!(errors.is_empty(), "{id}: result schema: {errors:?}\n{r}");
    r
}

fn drawing(h: &ExecutorHandle, id: &str) -> Value {
    okv(h, "draw.drawing.get", json!({ "drawingId": id }))
}

fn strokes_of(d: &Value) -> Vec<Value> {
    fn walk(nodes: &Value, out: &mut Vec<Value>) {
        for n in nodes.as_array().into_iter().flatten() {
            for s in n["strokes"].as_array().into_iter().flatten() {
                out.push(s.clone());
            }
            walk(&n["children"], out);
        }
    }
    let mut out = Vec::new();
    walk(&d["nodes"], &mut out);
    out
}

fn pts(list: &[(f64, f64, f64)]) -> Value {
    Value::Array(list.iter().map(|(x, y, p)| json!({ "x": x, "y": y, "pressure": p })).collect())
}

#[test]
fn a_brush_stroke_keeps_spine_pressure_and_pen_and_gets_a_closed_outline() {
    let h = fixture();
    let before = drawing(&h, D1);
    let r = okv(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 1, "kind": "brush",
                "points": pts(&[(0.0, 0.0, 0.1), (20.0, 10.0, 0.5), (40.0, 12.0, 1.0), (60.0, 5.0, 0.6), (80.0, 0.0, 0.2)]),
                "pen": { "tipShape": "round", "minSize": 2, "maxSize": 10, "tipAngle": 0 } }),
    );
    assert_eq!(r["drawingId"], D1);
    assert_eq!(r["createdDrawing"], false);
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 1);
    assert_eq!(r["zoneIds"], json!([]));
    let after = drawing(&h, D1);
    // A new layer on top (02 §3.1), with the brush hint.
    let top = after["nodes"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(top["type"], "layer");
    assert_eq!(top["id"], r["layerId"]);
    assert_eq!(top["layerKind"], "brush");
    let s = &top["strokes"][0];
    assert_eq!(s["kind"], "brush");
    assert!(s["bezier"].as_array().unwrap().len() >= 4, "spine kept: {s}");
    assert_eq!(s["pen"], json!({ "tipShape": "round", "minSize": 2.0, "maxSize": 10.0, "tipAngle": 0.0 }));
    let pressure = s["pressure"].as_array().unwrap();
    assert_eq!(pressure.first().unwrap()["t"], 0.0);
    assert_eq!(pressure.last().unwrap()["t"], 1.0);
    assert!(s["outline"][0].as_array().unwrap().len() >= 6);
    // The drawing revision rises with the content (REV-02, 02 §3.12 item 4); untouched strokes keep their IDs.
    assert!(after["rev"].as_u64().unwrap() > before["rev"].as_u64().unwrap());
    assert_eq!(after["rev"], r["rev"]);
    let ids_before: Vec<Value> = strokes_of(&before).iter().map(|s| s["id"].clone()).collect();
    let ids_after: Vec<Value> = strokes_of(&after).iter().map(|s| s["id"].clone()).collect();
    assert_eq!(&ids_after[..ids_before.len()], &ids_before[..]);
    // One undo step, labelled with the tool (02 §7.2.1); undo restores the exact content, redo the exact result.
    let hist = ok(&h, "edit.history.get", json!({}));
    assert_eq!(hist["entries"].as_array().unwrap().last().unwrap()["label"], "Brush Tool");
    ok(&h, "edit.undo", json!({}));
    let undone = drawing(&h, D1);
    assert_eq!(undone["nodes"], before["nodes"]);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(drawing(&h, D1)["nodes"], after["nodes"]);
}

#[test]
fn pencil_and_invisible_strokes_have_their_shapes_and_labels() {
    let h = fixture();
    let r = okv(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "thickness": 3, "layer": "new",
                "bezier": [{"x": -50, "y": 0}, {"x": -20, "y": 30}, {"x": 20, "y": 30}, {"x": 50, "y": 0}] }),
    );
    let d = drawing(&h, D1);
    let s = strokes_of(&d).into_iter().find(|s| s["id"] == r["strokeIds"][0]).unwrap();
    assert_eq!(s["kind"], "pencil");
    assert_eq!(s["lineStyle"], json!({ "thickness": 3.0, "lineType": "solid" }));
    assert_eq!(
        s["bezier"],
        json!([{"x": -50.0, "y": 0.0}, {"x": -20.0, "y": 30.0}, {"x": 20.0, "y": 30.0}, {"x": 50.0, "y": 0.0}]),
        "verbatim Béziers"
    );
    assert!(s["colourId"].is_string(), "the current colour");
    let inv = okv(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "fit": false, "points": [{"x": 0, "y": -10}, {"x": 0, "y": 110}] }),
    );
    assert_eq!(inv["hints"], json!(["drawing.strokesHidden"]));
    // Auto layer for an invisible stroke: the top-most layer whose strokes it crosses (02 §7.3.6) — the new pencil
    // layer above; one that crosses nothing gets a new top layer.
    assert_eq!(inv["layerId"], r["layerId"]);
    let labels: Vec<Value> = ok(&h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["label"].clone())
        .collect();
    assert_eq!(&labels[labels.len() - 2..], &[json!("Pencil Tool"), json!("Stroke Tool")]);
    let alone = okv(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "fit": false, "points": [{"x": 900, "y": 900}, {"x": 950, "y": 900}] }),
    );
    assert_ne!(alone["layerId"], r["layerId"]);
    ok(&h, "edit.undo", json!({}));
}

#[test]
fn drawing_into_an_empty_cell_creates_the_drawing_in_the_same_step() {
    let h = fixture();
    let rev = rev(&h);
    let r = okv(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 7, "kind": "pencil", "points": pts(&[(0.0, 0.0, 1.0), (50.0, 50.0, 1.0)]) }),
    );
    assert_eq!(r["createdDrawing"], true);
    let did = r["drawingId"].as_str().unwrap().to_owned();
    assert_eq!(rev + 1, crate::rev(&h), "one transaction (REV-01)");
    let d = drawing(&h, &did);
    assert_eq!(d["name"], "3", "the next unused integer of the bank (fixture names 1, 2, 10)");
    let exposure = get(&h, BOB)["exposure"].clone();
    assert_eq!(exposure[1], json!([did, 1]), "frame 7 exposes the new drawing: {exposure}");
    // createDrawing: false refuses the empty cell.
    let e = err(
        &h,
        "draw.stroke.add",
        json!({ "elementId": BOB, "frame": 8, "createDrawing": false, "points": pts(&[(0.0, 0.0, 1.0), (5.0, 5.0, 1.0)]) }),
    );
    assert_eq!(e.code, "drawing.noTarget");
    // One undo removes the drawing and the exposure change together.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(err(&h, "draw.drawing.get", json!({ "drawingId": did })).code, "ref.notFound");
    assert_eq!(get(&h, BOB)["exposure"][1], json!([null, 2]));
}

#[test]
fn locked_drawings_refuse_every_edit_and_stale_revisions_refuse_id_edits() {
    let h = fixture();
    for (id, params) in [
        ("draw.stroke.add", json!({ "drawingId": D2_LOCKED, "points": pts(&[(0.0, 0.0, 1.0), (5.0, 5.0, 1.0)]) })),
        ("draw.pivot.set", json!({ "drawingId": D2_LOCKED, "point": {"x": 1, "y": 1} })),
        ("draw.text.add", json!({ "drawingId": D2_LOCKED, "position": {"x": 0, "y": 0}, "text": "x" })),
        ("draw.content.add", json!({ "drawingId": D2_LOCKED, "content": { "nodes": [] } })),
        ("draw.object.delete", json!({ "drawingId": D2_LOCKED, "ids": ["anything"] })),
    ] {
        let before = rev(&h);
        assert_eq!(err(&h, id, params).code, "drawing.locked", "{id}");
        assert_eq!(rev(&h), before);
    }
    let d = drawing(&h, D1);
    let stale = d["rev"].as_u64().unwrap();
    okv(&h, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 3, "y": 4}, "expectRev": stale }));
    let e =
        err(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": PENCIL, "thickness": 2, "expectRev": stale }));
    assert_eq!(e.code, "drawing.staleRevision");
    assert_eq!(e.details["expectedRev"], stale);
    let fresh = drawing(&h, D1)["rev"].clone();
    okv(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": PENCIL, "thickness": 2, "expectRev": fresh }));
}

#[test]
fn stroke_set_edits_geometry_and_kind_and_keeps_the_id() {
    let h = fixture();
    let r = okv(&h, "draw.stroke.set", json!({ "strokeId": format!("{D1}/{PENCIL}"), "kind": "invisible" }));
    assert_eq!(r["strokeIds"], json!([PENCIL]));
    let s = strokes_of(&drawing(&h, D1)).into_iter().find(|s| s["id"] == PENCIL).unwrap();
    assert_eq!((s["kind"].clone(), s["colourId"].clone()), (json!("invisible"), Value::Null));
    okv(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": PENCIL, "kind": "pencil", "thickness": 5 }));
    let s = strokes_of(&drawing(&h, D1)).into_iter().find(|s| s["id"] == PENCIL).unwrap();
    assert_eq!(s["lineStyle"]["thickness"], 5.0);
    assert!(s["colourId"].is_string(), "made visible with the current colour");
    // A brush spine edit sweeps the outline again; pressure edits too.
    let before = strokes_of(&drawing(&h, D1)).into_iter().find(|s| s["id"] == BRUSH).unwrap();
    okv(
        &h,
        "draw.stroke.set",
        json!({ "drawingId": D1, "strokeId": BRUSH, "pressure": [{"t": 0, "p": 1}, {"t": 1, "p": 1}] }),
    );
    let after = strokes_of(&drawing(&h, D1)).into_iter().find(|s| s["id"] == BRUSH).unwrap();
    assert_ne!(after["outline"], before["outline"]);
    assert_eq!(after["pressure"], json!([{"t": 0.0, "p": 1.0}, {"t": 1.0, "p": 1.0}]));
    // Not applicable parameters are refused with their pointer.
    let e = err(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": BRUSH, "thickness": 2 }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("arg.invalid", Some("/thickness")));
    let e =
        err(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": INVISIBLE, "pressure": [{"t": 0, "p": 1}] }));
    assert_eq!(e.details["pointer"], "/pressure");
    let e = err(&h, "draw.stroke.set", json!({ "drawingId": D1, "strokeId": "st_nope", "thickness": 2 }));
    assert_eq!(e.code, "drawing.objectNotFound");
}

#[test]
fn geometry_errors_name_their_parameter_and_change_nothing() {
    let h = fixture();
    let before = rev(&h);
    for (params, pointer) in [
        (json!({ "drawingId": D1, "kind": "pencil", "bezier": [{"x": 0, "y": 0}, {"x": 1, "y": 1}] }), "/bezier"),
        (json!({ "drawingId": D1, "points": [{"x": 0, "y": 0}, {"x": 2e7, "y": 0}] }), "/points/1/x"),
        (
            json!({ "drawingId": D1, "points": [{"x": 0, "y": 0}, {"x": 5, "y": 5}], "pressure": [{"t": 0.5, "p": 1}, {"t": 0.2, "p": 1}] }),
            "/pressure/1",
        ),
        (json!({ "drawingId": D1, "kind": "pencil", "points": [{"x": 3, "y": 3}] }), "/points"),
        (
            json!({ "drawingId": D1, "points": [{"x": 0, "y": 0}], "pen": {"tipShape": "round", "minSize": 0, "maxSize": 0} }),
            "/pen",
        ),
    ] {
        let e = err(&h, "draw.stroke.add", params.clone());
        assert_eq!(e.code, "drawing.invalidGeometry", "{params}: {e:?}");
        assert_eq!(e.details["pointer"], pointer, "{params}");
    }
    // Malformed and unknown parameters are refused by the schema.
    assert_eq!(
        err(&h, "draw.stroke.add", json!({ "drawingId": D1, "points": [{"x": 0, "y": 0}], "bogus": 1 })).code,
        "arg.unknown"
    );
    assert_eq!(
        err(&h, "draw.stroke.add", json!({ "drawingId": D1, "kind": "spray", "points": [{"x": 0, "y": 0}] })).code,
        "arg.invalid"
    );
    assert_eq!(err(&h, "draw.stroke.add", json!({ "drawingId": D1 })).code, "arg.missing");
    assert_eq!(
        err(&h, "draw.stroke.add", json!({ "drawingId": D1, "colourId": "col_nope", "points": [{"x": 0, "y": 0}] }))
            .code,
        "colour.notFound"
    );
    assert_eq!(rev(&h), before);
}

#[test]
fn text_objects_add_edit_and_format_with_runs_paragraphs_and_bounds() {
    let h = fixture();
    let r = okv(
        &h,
        "draw.text.add",
        json!({ "drawingId": D1, "position": {"x": -100, "y": 300}, "text": "Hello\nworld", "format": {"size": 40, "align": "centre"} }),
    );
    let tid = r["textId"].as_str().unwrap().to_owned();
    assert!(r["bounds"]["width"].as_f64().unwrap() > 0.0);
    assert!(
        (r["bounds"]["y"].as_f64().unwrap() + r["bounds"]["height"].as_f64().unwrap() - 300.0).abs() < 1e-9,
        "top at the position"
    );
    let find = |h: &ExecutorHandle| -> Value {
        drawing(h, D1)["nodes"].as_array().unwrap().iter().find(|n| n["id"] == tid.as_str()).cloned().unwrap()
    };
    let t = find(&h);
    assert_eq!(t["runs"].as_array().unwrap().len(), 1);
    assert_eq!(t["runs"][0]["font"], "sans-serif");
    assert_eq!(t["runs"][0]["size"], 40.0);
    assert_eq!(t["paragraphs"].as_array().unwrap().len(), 2);
    assert_eq!(t["paragraphs"][1]["align"], "centre");
    okv(&h, "draw.textFormat.set", json!({ "drawingId": D1, "textId": tid, "start": 0, "length": 5, "bold": true }));
    let t = find(&h);
    assert_eq!(
        t["runs"].as_array().unwrap().iter().map(|r| (r["length"].clone(), r["bold"].clone())).collect::<Vec<_>>(),
        vec![(json!(5), json!(true)), (json!(6), json!(false))]
    );
    okv(&h, "draw.text.set", json!({ "drawingId": D1, "textId": tid, "insert": {"at": 5, "text": "!"} }));
    assert_eq!(find(&h)["text"], "Hello!\nworld");
    assert_eq!(find(&h)["runs"][0]["length"], 6, "typing continues the run before");
    okv(&h, "draw.text.set", json!({ "drawingId": D1, "textId": tid, "width": 120 }));
    assert_eq!(find(&h)["widthMode"], "fixed");
    okv(&h, "draw.text.set", json!({ "drawingId": D1, "textId": tid, "position": {"x": 0, "y": 0} }));
    assert_eq!(find(&h)["transform"], json!([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    let labels: Vec<Value> = ok(&h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["label"].clone())
        .collect();
    assert_eq!(
        &labels[labels.len() - 5..],
        &[
            json!("New Text Object"),
            json!("Text Property Modified"),
            json!("Typing"),
            json!("Set Text Width"),
            json!("Move Text Object")
        ]
    );
    // A missing font keeps its name and is reported (02 §3.7).
    let env = run(&h, "draw.textFormat.set", json!({ "drawingId": D1, "textId": tid, "font": "No Such Font 123" }));
    assert_eq!(env.status, Status::Ok);
    assert!(env.warnings.iter().any(|w| w.code == "drawing.fontMissing"), "{:?}", env.warnings);
    assert_eq!(find(&h)["runs"][0]["font"], "No Such Font 123");
    let lint = ok(&h, "doc.validate", json!({ "checks": ["missingFonts"] }));
    assert!(
        lint["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "missingFonts" && i["details"]["font"] == "No Such Font 123"),
        "{lint}"
    );
    // Outline colours cannot be stored (SI-055).
    assert_eq!(
        err(&h, "draw.textFormat.set", json!({ "drawingId": D1, "textId": tid, "outlineColourId": "x" })).details["pointer"],
        "/outlineColourId"
    );
}

#[test]
fn groups_delete_and_pivot() {
    let h = fixture();
    let before = drawing(&h, D1);
    let g = okv(&h, "draw.group.create", json!({ "drawingId": D1, "ids": [GROUP, CAPTION] }));
    let d = drawing(&h, D1);
    let nodes = d["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[1]["id"], g["groupId"]);
    assert_eq!(
        nodes[1]["children"].as_array().unwrap().iter().map(|c| c["id"].clone()).collect::<Vec<_>>(),
        vec![json!(GROUP), json!(CAPTION)],
        "stacking order kept"
    );
    let u = okv(&h, "draw.group.delete", json!({ "drawingId": D1, "groupIds": [g["groupId"]] }));
    assert_eq!(u["memberIds"], json!([GROUP, CAPTION]));
    assert_eq!(drawing(&h, D1)["nodes"], before["nodes"], "ungroup restores the members in place and order");
    // Grouping strokes separates them into their own layer inside the group.
    let g2 = okv(&h, "draw.group.create", json!({ "drawingId": D1, "ids": [PENCIL] }));
    // The group and the new layer holding the stroke are created (02 §13.1 item 11, C21-R1-08).
    assert_eq!(g2["createdIds"].as_array().unwrap().len(), 2);
    assert_eq!(g2["createdIds"][0], g2["groupId"]);
    assert_eq!(g2["drawingId"], D1);
    let d = drawing(&h, D1);
    assert!(strokes_of(&d).iter().any(|s| s["id"] == PENCIL), "the stroke keeps its ID");
    // Deleting every stroke of a layer removes the layer too.
    let del = okv(&h, "draw.object.delete", json!({ "drawingId": D1, "ids": [BRUSH, INVISIBLE] }));
    assert!(del["removedIds"].as_array().unwrap().contains(&json!("nd_01K742SG0PC20XDQKASM9CGZHR")), "{del}");
    let p = okv(&h, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 12.5, "y": -3} }));
    assert_eq!(p["pivot"], json!({ "x": 12.5, "y": -3.0 }));
    assert_eq!(drawing(&h, D1)["pivot"], json!({ "x": 12.5, "y": -3.0 }));
    assert_eq!(err(&h, "draw.group.create", json!({ "drawingId": D1, "ids": [] })).code, "drawing.nothingToDo");
}

#[test]
fn content_round_trips_through_read_back_with_new_ids() {
    let h = fixture();
    let src = drawing(&h, D1);
    // Into the empty cell at frame 7: a new drawing gets the content, IDs reassigned, colours kept.
    let r = okv(&h, "draw.content.add", json!({ "elementId": BOB, "frame": 7, "content": src.clone() }));
    assert_eq!(r["createdDrawing"], true);
    let map = r["idMap"].as_object().unwrap();
    assert!(map.contains_key(PENCIL) && map.contains_key(CAPTION));
    let copy = drawing(&h, r["drawingId"].as_str().unwrap());
    let mut s = serde_json::to_string(&src["nodes"]).unwrap();
    for (old, new) in map {
        s = s.replace(old.as_str(), new.as_str().unwrap());
    }
    let expected: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(copy["nodes"], expected, "geometry, runs and colours identical; only IDs differ");
    // The inserted top-level objects become the selection.
    let sel = ok(&h, "edit.selection.get", json!({ "context": "drawing" }));
    assert_eq!(sel["ids"].as_array().unwrap().len(), 3, "{sel}");
    // Offset moves geometry and transforms.
    let r2 = okv(
        &h,
        "draw.content.add",
        json!({ "drawingId": D1, "layer": "behind", "offset": {"x": 10, "y": -10},
        "content": { "nodes": [ src["nodes"][2].clone() ] } }),
    );
    let first = drawing(&h, D1)["nodes"][0].clone();
    assert_eq!(first["id"], r2["idMap"][CAPTION]);
    assert_eq!(first["transform"], json!([1.0, 0.0, 0.0, 1.0, -90.0, 190.0]));
    // Unknown colours are kept and reported until recovery (M2 C2.2).
    let env = run(
        &h,
        "draw.content.add",
        json!({ "drawingId": D1, "content": { "nodes": [ { "type": "layer", "id": "l", "layerKind": "pencil",
        "strokes": [ { "id": "s", "kind": "pencil", "bezier": [{"x": 0, "y": 0}, {"x": 1, "y": 0}, {"x": 2, "y": 0}, {"x": 3, "y": 0}],
                       "colourId": "col_from_elsewhere", "lineStyle": {"thickness": 1, "lineType": "solid"} } ] } ] } }),
    );
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    assert!(env.warnings.iter().any(|w| w.code == "colour.unresolved"));
    // Bitmaps must name an image the document holds.
    let e = err(
        &h,
        "draw.content.add",
        json!({ "drawingId": D1, "content": { "nodes": [ { "type": "bitmap", "id": "b", "mediaId": "nope.png", "pixelWidth": 1, "pixelHeight": 1, "transform": [1, 0, 0, 1, 0, 0] } ] } }),
    );
    assert_eq!(e.details["pointer"], "/content/nodes/0/mediaId");
    let d2 = okv(&h, "draw.drawing.get", json!({ "drawingId": D2_LOCKED }));
    let bmp = d2["nodes"][0].clone();
    okv(&h, "draw.content.add", json!({ "drawingId": D1, "content": { "nodes": [ bmp ] } }));
}

#[test]
fn read_back_options_flatten_ids_and_include() {
    let h = fixture();
    let flat = okv(&h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let items = flat["nodes"].as_array().unwrap();
    assert!(
        items.iter().all(|i| i.get("strokes").is_none() && i.get("children").is_none() && i.get("parentId").is_some())
    );
    assert_eq!(items.iter().filter(|i| i["type"] == "stroke").count(), 3);
    let one = okv(&h, "draw.drawing.get", json!({ "drawingId": D1, "ids": [format!("{D1}/{PENCIL}")] }));
    assert_eq!(one["nodes"].as_array().unwrap().len(), 1);
    let lean = okv(&h, "draw.drawing.get", json!({ "drawingId": D1, "include": ["bounds"] }));
    let s = &lean["nodes"][0]["strokes"][0];
    assert!(s.get("outline").is_none() && s.get("pressure").is_none());
    // Zones are reported as unavailable until the planar map (M2 C2.2), never as an empty success.
    let env = run(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    assert!(env.warnings.iter().any(|w| w.code == "doc.derivedUnavailable"));
    assert!(env.result.unwrap()["nodes"][0].get("zones").is_none());
    let cur = okv(&h, "draw.current.get", json!({}));
    assert_eq!(cur["cellEmpty"], true, "the camera is current in the fixture");
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 2 }));
    let cur = okv(&h, "draw.current.get", json!({}));
    assert_eq!(cur["drawingId"], D1);
    // With a current drawing, commands need no target.
    okv(&h, "draw.stroke.add", json!({ "kind": "invisible", "points": [{"x": 0, "y": 0}, {"x": 9, "y": 9}] }));
}

#[test]
fn drawing_events_come_from_the_commit_path_for_patches_batches_and_checkpoint_restore() {
    // 02 §13.6, spec A `draw.drawing.changed` (C21-R1-07): every commit that changes drawing content reports the
    // drawing, its revision, the committing command and the changed local IDs.
    let h = fixture();
    let rx = h.subscribe();
    let drawing_events = |rx: &std::sync::mpsc::Receiver<tt_executor::Event>| -> Vec<Value> {
        rx.try_iter().filter(|e| e.name == "draw.drawing.changed").map(|e| e.payload).collect()
    };
    ok(&h, "checkpoint.create", json!({"name":"before-strokes"}));
    let made = okv(&h, "draw.stroke.add", json!({"drawingId":D1,"points":pts(&[(0.0, 0.0, 1.0), (30.0, 0.0, 1.0)])}));
    let sid = made["strokeIds"][0].as_str().unwrap().to_owned();
    let added = drawing_events(&rx);
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0]["drawingId"], D1);
    assert_eq!(added[0]["commandId"], "draw.stroke.add");
    assert_eq!(added[0]["rev"], made["rev"]);
    assert!(added[0]["createdIds"].as_array().unwrap().contains(&json!(sid)), "{added:?}");

    let key = format!("{D1}/{PENCIL}");
    ok(&h, "doc.object.patch", json!({"targets":[key],"ops":[{"op":"replace","path":"/lineStyle/thickness","value":2}]}));
    let patched = drawing_events(&rx);
    assert_eq!(patched.len(), 1, "{patched:?}");
    assert_eq!(patched[0]["commandId"], "doc.object.patch");
    assert_eq!(patched[0]["modifiedIds"], json!([PENCIL]));

    let env = batch(
        &h,
        json!({"label":"pivot and delete", "steps":[
            {"id":"draw.pivot.set","params":{"drawingId":D1,"point":{"x":1,"y":2}}},
            {"id":"draw.object.delete","params":{"drawingId":D1,"ids":[sid]}}
        ]}),
    );
    assert!(env.is_ok(), "{env:?}");
    let batched = drawing_events(&rx);
    assert_eq!(batched.len(), 1, "one atomic commit, one event: {batched:?}");
    assert_eq!(batched[0]["commandId"], "batch");
    assert!(batched[0]["removedIds"].as_array().unwrap().contains(&json!(sid)), "{batched:?}");

    ok(&h, "checkpoint.restore", json!({"name":"before-strokes"}));
    let restored = drawing_events(&rx);
    assert_eq!(restored.len(), 1, "{restored:?}");
    assert_eq!(restored[0]["drawingId"], D1);
    assert_eq!(restored[0]["commandId"], "checkpoint.restore");
    assert_eq!(restored[0]["modifiedIds"], json!([PENCIL]), "{restored:?}");
}
