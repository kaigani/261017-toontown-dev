//! The interactive canvas's drawing payloads through the real path (M4 C4.2b; design note 1 §2, §6, §8; T4, T5): a
//! drawing built by gestures, walked by the final renderer's own builder, tessellated and encoded by
//! `ExecutorHandle::canvas_drawing`; its bytes are the real-path half of the golden corpus
//! (`desktop/src/canvas/testdata/drawing.ttrd`, decoded by Vitest too; `TT_UPDATE_CANVAS_CORPUS=1` rewrites it).

use serde_json::{Value, json};
use tt_render::canvas::{Record, SlotKind, Vertex, decode};

use crate::common::*;
use crate::equivalence::{D1, drag, pointer, ready};

fn colour(h: &tt_executor::ExecutorHandle, name: &str) -> String {
    let doc = h.snapshot().unwrap().unwrap();
    doc.palettes()
        .flat_map(|p| p.swatches().iter())
        .find(|s| s.name() == name)
        .unwrap_or_else(|| panic!("swatch {name}"))
        .colour_id()
        .as_str()
        .to_owned()
}

/// A drawing with a pencil corner, a brush stroke, a solid-painted rectangle and two gradient-painted ones.
fn built() -> tt_executor::ExecutorHandle {
    let h = ready();
    let ok_env = |e: tt_executor::Envelope| assert!(e.is_ok(), "{e:?}");
    ok_env(pointer(&h, "pencil", drag(&[(700.0, 300.0), (760.0, 300.0), (760.0, 360.0)])));
    ok_env(pointer(&h, "brush", drag(&[(800.0, 300.0), (830.0, 320.0), (860.0, 300.0), (890.0, 330.0)])));
    for (x, name) in [(980.0, None), (1100.0, Some("Sky")), (1220.0, Some("Glow"))] {
        ok_env(pointer(&h, "rectangle", drag(&[(x, 420.0), (x + 80.0, 480.0)])));
        if let Some(n) = name {
            ok(&h, "colour.current.set", json!({ "colourId": colour(&h, n) }));
        }
        ok_env(pointer(&h, "paint", drag(&[(x + 40.0, 450.0), (x + 40.0, 450.0)])));
    }
    h
}

fn to_json(d: &tt_render::canvas::CanvasDrawing) -> Value {
    let verts = |v: &[Vertex]| v.iter().map(|v| json!([v.x, v.y, v.slot, v.flags])).collect::<Vec<_>>();
    json!({
        "generation": d.identity.generation.to_string(),
        "rev": d.identity.rev.to_string(),
        "drawingId": d.identity.drawing_id,
        "bucket": d.identity.bucket,
        "flags": d.flags,
        "bounds": d.bounds,
        "slots": d.slots.iter().map(|s| json!({
            "id": s.id,
            "kind": match s.kind { SlotKind::Solid => "solid", SlotKind::Gradient => "gradient", SlotKind::Texture => "texture" },
            "rgba": s.rgba,
        })).collect::<Vec<_>>(),
        "records": d.records.iter().map(|r| match r {
            Record::Solid { vertices, indices } => json!({ "kind": "solid", "vertices": verts(vertices), "indices": indices }),
            Record::Gradient { radial, drawing_to_unit, stops, vertices, indices } => json!({
                "kind": "gradient", "radial": radial, "drawingToUnit": drawing_to_unit,
                "stops": stops.iter().map(|(p, c)| json!([p, c])).collect::<Vec<_>>(),
                "vertices": verts(vertices), "indices": indices,
            }),
            Record::Bitmap { width, height, quad, image_id, slot } => json!({
                "kind": "bitmap", "width": width, "height": height, "quad": quad, "imageId": image_id, "slot": slot,
            }),
        }).collect::<Vec<_>>(),
    })
}

fn corpus(name: &str, bytes: &[u8]) {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/canvas/testdata").join(name);
    if std::env::var_os("TT_UPDATE_CANVAS_CORPUS").is_some() {
        std::fs::write(&path, bytes).unwrap();
    } else {
        let committed =
            std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e} (TT_UPDATE_CANVAS_CORPUS=1 writes it)"));
        assert!(committed == bytes, "{name}: the encoded bytes differ from the committed corpus file");
    }
}

#[test]
fn t4_a_drawing_built_by_gestures_encodes_to_the_committed_corpus_bytes() {
    let h = built();
    let p = h.canvas_drawing(None, 5, 0, tt_executor::canvas::PayloadOptions::default()).unwrap();
    assert_eq!(p.drawing_id, D1, "the current drawing");
    let got = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
    assert_eq!(json!(p.rev), got["rev"], "the payload is at the drawing's revision");
    let d = decode(&p.bytes).unwrap();
    assert_eq!(d.identity.drawing_id, D1.trim_start_matches("dr_"));
    assert_eq!((d.identity.generation, d.identity.bucket, d.identity.rev), (5, 0, p.rev));
    // The slots are the fixture's colour ids, with the gradient swatches marked as such; both gradients are records.
    let gradient_ids = [colour(&h, "Sky"), colour(&h, "Glow")];
    for id in &gradient_ids {
        let s = d.slots.iter().find(|s| &s.id == id).unwrap_or_else(|| panic!("slot {id}"));
        assert_eq!(s.kind, SlotKind::Gradient);
    }
    let kinds: Vec<bool> = d.records.iter().map(|r| matches!(r, Record::Gradient { radial: true, .. })).collect();
    assert!(kinds.contains(&true), "the radial gradient is a record");
    assert!(d.records.iter().any(|r| matches!(r, Record::Gradient { radial: false, .. })), "the linear one too");
    assert!(
        d.records.iter().any(|r| matches!(r, Record::Solid { vertices, .. } if vertices.iter().any(|v| v.flags == 1))),
        "the pencil line is stroke geometry"
    );
    corpus("drawing.ttrd", &p.bytes);
    let mut text = serde_json::to_string_pretty(&to_json(&d)).unwrap();
    text.push('\n');
    corpus("drawing.json", text.as_bytes());
}

#[test]
fn t5_the_same_revision_and_bucket_rebuild_identically_and_an_edit_changes_the_revision() {
    let h = built();
    let a = h.canvas_drawing(Some(D1), 1, 0, tt_executor::canvas::PayloadOptions::default()).unwrap();
    let b = h.canvas_drawing(None, 1, 0, tt_executor::canvas::PayloadOptions::default()).unwrap();
    assert_eq!(a, b, "an evicted drawing is rebuilt identically (same revision, same bucket)");
    let finer = h.canvas_drawing(Some(D1), 1, 2, tt_executor::canvas::PayloadOptions::default()).unwrap();
    assert_eq!(finer.rev, a.rev, "a bucket change is a reload at the same revision");
    assert!(finer.bytes.len() > a.bytes.len(), "a finer bucket flattens curves into more vertices");
    let e = pointer(&h, "pencil", drag(&[(700.0, 200.0), (720.0, 210.0)]));
    assert!(e.is_ok(), "{e:?}");
    let c = h.canvas_drawing(None, 1, 0, tt_executor::canvas::PayloadOptions::default()).unwrap();
    assert!(c.rev > a.rev, "an edit moves the drawing's revision ({} -> {})", a.rev, c.rev);
    assert_eq!(decode(&c.bytes).unwrap().identity.rev, c.rev);
}

#[test]
fn a_payload_names_its_refusals() {
    let h = ready();
    let e = h
        .canvas_drawing(Some("dr_01K7ZZZZZZZZZZZZZZZZZZZZZZ"), 1, 0, tt_executor::canvas::PayloadOptions::default())
        .unwrap_err();
    assert_eq!(e.code, "ref.notFound", "{e:?}");
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": null }));
    let e = h.canvas_drawing(None, 1, 0, tt_executor::canvas::PayloadOptions::default()).unwrap_err();
    assert_eq!(e.code, "drawing.noTarget", "{e:?}");
}

#[test]
fn the_scene_lists_the_current_drawing_at_its_revision_with_the_aids_and_the_solid_colours() {
    let h = built();
    let s = h.canvas_scene("drawingView").unwrap();
    let v = serde_json::to_value(&s).unwrap();
    let rev = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }))["rev"].clone();
    assert_eq!(
        v["entries"],
        json!([{ "drawingId": D1, "rev": rev, "role": "current", "opacity": 1.0, "tint": null, "shade": false, "outline": false }])
    );
    assert_eq!(v["background"], json!([240, 240, 240, 255]), "02 §10.7's default");
    assert_eq!(v["grid"], json!({ "visible": false, "type": "normal", "overlay": false }));
    // The stamp crosses JSON as an exact number (a wrapping hash would not: found in the running application).
    assert!(s.palette_stamp < (1 << 53), "{}", s.palette_stamp);
    // Every solid swatch's value is there; the gradient swatches are not (their records carry the stops).
    let doc = h.snapshot().unwrap().unwrap();
    for sw in doc.palettes().flat_map(|p| p.swatches().iter()) {
        let solid = format!("{:?}", sw.kind()).to_lowercase() == "solid";
        assert_eq!(v["colours"].get(sw.colour_id().as_str()).is_some(), solid, "{}", sw.name());
    }
    // The aids follow their commands; a palette edit moves the stamp; the canvas never derives these itself.
    ok(&h, "drawAid.grid.set", json!({ "visible": true, "type": "field12", "overlay": true }));
    ok(&h, "drawAid.background.set", json!({ "colour": "#102030FF" }));
    let t = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    assert_eq!(t["grid"], json!({ "visible": true, "type": "field12", "overlay": true }));
    assert_eq!(t["background"], json!([16, 32, 48, 255]));
    assert_eq!(h.canvas_scene("cameraView").unwrap_err().code, "view.notFound");
}

#[test]
fn the_scene_carries_onion_skin_light_tables_and_shade_in_layer_order() {
    let h = ready();
    ok(&h, "app.context.set", json!({ "elementId": crate::equivalence::BOB, "frame": 10 }));
    let roles = |h: &tt_executor::ExecutorHandle| -> Vec<Value> {
        serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap()["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| json!([e["role"], e["drawingId"], e["opacity"], e["tint"], e["shade"], e["outline"]]))
            .collect()
    };
    let cur = ok(&h, "draw.drawing.get", json!({}))["drawingId"].clone();
    assert_ne!(cur, json!(D1), "frame 10 shows Bob's second drawing");
    assert_eq!(roles(&h), vec![json!(["current", cur, 1.0, null, false, false])], "no aid: only the current drawing");
    // Onion skin (02 §10.2): Bob's exposure order is d1, d2, d1 (distinct drawings); at d2 the previous and the next are
    // d1, at 50 % (distance 1), tinted with the previous and next colours.
    ok(&h, "drawAid.onionSkin.set", json!({ "enabled": true, "previous": 2, "next": 1 }));
    ok(&h, "drawAid.onionSkinStyle.set", json!({ "outline": true }));
    assert_eq!(
        roles(&h),
        vec![
            json!(["onion", D1, 0.5, [255, 0, 0, 255], false, true]),
            json!(["onion", D1, 0.5, [0, 192, 0, 255], false, true]),
            json!(["current", cur, 1.0, null, false, false]),
        ]
    );
    ok(&h, "drawAid.onionSkinStyle.set", json!({ "transparency": false, "colourShading": false, "outline": false }));
    assert_eq!(roles(&h)[0], json!(["onion", D1, 1.0, null, false, false]), "opaque and untinted when both are off");
    ok(&h, "drawAid.onionSkin.set", json!({ "enabled": false }));
    // The auto light table (02 §10.3): the other elements' drawings at the current frame (the Background image's), not
    // the Notes annotation (hidden in the Exposure Sheet); a static entry over the work (02 §10.4); both shaded.
    ok(&h, "drawAid.autoLightTable.set", json!({ "enabled": true }));
    ok(
        &h,
        "drawAid.staticLightTable.add",
        json!({ "elementId": crate::equivalence::BOB, "frame": 1, "placement": "overlay" }),
    );
    let r = roles(&h);
    let names: Vec<&str> = r.iter().map(|e| e[0].as_str().unwrap()).collect();
    assert_eq!(names, ["autoLight", "current", "staticOver"], "{r:?}");
    assert_eq!(r[2][1], json!(D1));
    assert!(r[0][4].as_bool().unwrap() && r[2][4].as_bool().unwrap(), "light tables are shaded by default");
    ok(&h, "drawAid.lightTableShade.set", json!({ "auto": false, "static": false }));
    assert!(roles(&h).iter().all(|e| e[4] == json!(false)), "shade follows its settings");
    // The outline-only payload (02 §10.2) is its own variant: no fills, stroke geometry, the outline flag.
    let o = decode(
        &h.canvas_drawing(Some(D1), 1, 0, tt_executor::canvas::PayloadOptions { outline: true, ..Default::default() })
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert_eq!(o.flags, 1);
    assert!(
        o.records.iter().all(|r| matches!(r, Record::Solid { vertices, .. } if vertices.iter().all(|v| v.flags == 1))
            || matches!(r, Record::Gradient { .. })),
        "outlines only"
    );
}

#[test]
fn the_fast_pencil_payload_drops_round_joins_and_says_so() {
    let h = built();
    let full = h.canvas_drawing(Some(D1), 1, 0, tt_executor::canvas::PayloadOptions::default()).unwrap();
    let fast = tt_executor::canvas::PayloadOptions { fast_pencil: true, ..Default::default() };
    let fast = h.canvas_drawing(Some(D1), 1, 0, fast).unwrap();
    let (a, b) = (decode(&full.bytes).unwrap(), decode(&fast.bytes).unwrap());
    assert_eq!((a.flags, b.flags), (0, 2), "header flag 2 marks approximate pencil strokes");
    let count = |d: &tt_render::canvas::CanvasDrawing| -> usize {
        d.records
            .iter()
            .map(|r| match r {
                Record::Solid { vertices, .. } | Record::Gradient { vertices, .. } => vertices.len(),
                Record::Bitmap { .. } => 4,
            })
            .sum()
    };
    assert!(count(&b) < count(&a), "fewer pieces without the joins ({} vs {})", count(&b), count(&a));
}

/// A rectangle drawn at `[x0, y0]`-`[x1, y1]` (drawing units) and selected; its selection bounds as `draw.selection.get`
/// answers them, `[minX, minY, maxX, maxY]`.
fn selected_rectangle(h: &tt_executor::ExecutorHandle, x0: f64, y0: f64, x1: f64, y1: f64) -> [f64; 4] {
    let r = ok(h, "draw.rectangle.add", json!({ "from": { "x": x0, "y": y0 }, "to": { "x": x1, "y": y1 } }));
    let ids = r["strokeIds"].clone();
    ok(h, "draw.selection.set", json!({ "by": "ids", "ids": ids }));
    let b = ok(h, "draw.selection.get", json!({}))["bounds"].clone();
    let f = |k: &str| b[k].as_f64().unwrap();
    [f("x"), f("y"), f("x") + f("width"), f("y") + f("height")]
}

/// The headless Drawing view's mapping at 100 % (SI-063): drawing unit to view pixel.
fn v(x: f64, y: f64) -> (f64, f64) {
    (x + 960.0, 540.0 - y)
}

/// Frame-loop note §7.1 (C42B-R1-22): the scene carries the current drawing's selection as the geometry of
/// `draw.selection.get` (each object, the box, its centre as the pivot, the point selection) and the drawing pivot;
/// `null` when nothing is selected.
#[test]
fn the_scene_carries_the_selection_and_the_drawing_pivot() {
    let h = ready();
    ok(&h, "draw.selection.set", json!({ "by": "none" }));
    let s = h.canvas_scene("drawingView").unwrap();
    assert_eq!(s.selection, None, "nothing selected");
    let pivot = ok(&h, "draw.drawing.get", json!({}))["pivot"].clone();
    assert_eq!(s.drawing_pivot, Some([pivot["x"].as_f64().unwrap(), pivot["y"].as_f64().unwrap()]));
    // One object.
    let a = selected_rectangle(&h, 300.0, 100.0, 400.0, 160.0);
    let sel = h.canvas_scene("drawingView").unwrap().selection.expect("one object selected");
    assert_eq!(sel.bounds, Some(a), "the box of draw.selection.get");
    assert_eq!(sel.objects, vec![a], "one object, its own bounds");
    assert_eq!(sel.pivot, Some([(a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0]), "the centre of the box");
    assert!(sel.points.is_empty());
    // Two objects: each one's bounds, and the box around both.
    let first = ok(&h, "draw.selection.get", json!({}))["selectedIds"].clone();
    let b = selected_rectangle(&h, 300.0, -200.0, 360.0, -120.0);
    let second = ok(&h, "draw.selection.get", json!({}))["selectedIds"].clone();
    let both: Vec<Value> = first.as_array().unwrap().iter().chain(second.as_array().unwrap()).cloned().collect();
    ok(&h, "draw.selection.set", json!({ "by": "ids", "ids": both }));
    let sel = h.canvas_scene("drawingView").unwrap().selection.unwrap();
    assert_eq!(sel.objects, vec![a, b]);
    let get = ok(&h, "draw.selection.get", json!({}))["bounds"].clone();
    let g = |k: &str| get[k].as_f64().unwrap();
    assert_eq!(sel.bounds, Some([g("x"), g("y"), g("x") + g("width"), g("y") + g("height")]));
    assert_eq!(sel.bounds, Some([a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]));
    // A point selection alone: its anchors, no box.
    ok(&h, "draw.selection.set", json!({ "by": "none" }));
    let stroke = first[0].as_str().unwrap().to_owned();
    ok(&h, "draw.pointSelection.set", json!({ "points": [{ "strokeId": stroke, "anchor": 0 }] }));
    let sel = h.canvas_scene("drawingView").unwrap().selection.expect("a point selection");
    assert_eq!((sel.objects.len(), sel.bounds, sel.pivot, sel.points.len()), (0, None, None, 1));
    ok(&h, "draw.pointSelection.set", json!({ "points": [] }));
    assert_eq!(h.canvas_scene("drawingView").unwrap().selection, None, "cleared");
}

/// The command a Select-tool drag produces (frame-loop note §7.3), and its parameters.
fn select_drag(h: &tt_executor::ExecutorHandle, events: Vec<Value>) -> Result<(String, Value), String> {
    let env = pointer(h, "select", events);
    if let Some(e) = env.error {
        return Err(e.code);
    }
    let c = env.result.unwrap()["commands"][1].clone();
    Ok((c["commandId"].as_str().unwrap().to_owned(), c["params"].clone()))
}

/// Frame-loop note §7.3 (C42B-R1-22): one case per row of the handle table, through the kernel's gesture code (the
/// path of the window and of the headless `pointer` alike), with the factors, angle and pivot it defines; the order of
/// the tests where handle areas overlap; the constrained corner; the degenerate scale.
#[test]
fn each_select_handle_produces_its_command() {
    let h = ready();
    let [x0, y0, x1, y1] = selected_rectangle(&h, 300.0, 100.0, 400.0, 160.0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (w, hgt) = (x1 - x0, y1 - y0);
    // A drag from drawing point a to drawing point b, in view pixels.
    let d = |a: (f64, f64), b: (f64, f64)| {
        let (p, q) = (v(a.0, a.1), v(b.0, b.1));
        drag(&[p, ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0), q])
    };
    let close = |got: &Value, want: Value| {
        let num = |v: &Value, path: &[&str]| path.iter().fold(v, |v, k| &v[*k]).as_f64().unwrap_or(f64::NAN);
        for path in [&["scale", "x"][..], &["scale", "y"], &["pivot", "x"], &["pivot", "y"], &["rotate"]] {
            let (a, b) = (num(got, path), num(&want, path));
            assert!(a.is_nan() && b.is_nan() || (a - b).abs() < 1e-9, "{path:?}: {got} against {want}");
        }
    };
    // Top-right corner (corner 2) out by (20, 12): both axes about the bottom-left corner.
    let (id, p) = select_drag(&h, d((x1, y1), (x1 + 20.0, y1 + 12.0))).unwrap();
    assert_eq!(id, "draw.transform.apply");
    close(&p, json!({ "scale": { "x": (w + 20.0) / w, "y": (hgt + 12.0) / hgt }, "pivot": { "x": x0, "y": y0 } }));
    ok(&h, "edit.undo", json!({}));
    // The same corner with the constrain modifier: the larger factor on both axes.
    let mut ev = d((x1, y1), (x1 + 20.0, y1 + 12.0));
    for e in &mut ev {
        e["modifiers"] = json!(["shift"]);
    }
    let (_, p) = select_drag(&h, ev).unwrap();
    let s = ((w + 20.0) / w).max((hgt + 12.0) / hgt);
    close(&p, json!({ "scale": { "x": s, "y": s }, "pivot": { "x": x0, "y": y0 } }));
    ok(&h, "edit.undo", json!({}));
    // Right side (side 1): x only, about the midpoint of the left side.
    let (_, p) = select_drag(&h, d((x1, cy), (x1 + 50.0, cy + 30.0))).unwrap();
    close(&p, json!({ "scale": { "x": (w + 50.0) / w, "y": 1.0 }, "pivot": { "x": x0, "y": cy } }));
    ok(&h, "edit.undo", json!({}));
    // Bottom side (side 0): y only, about the midpoint of the top side.
    let (_, p) = select_drag(&h, d((cx, y0), (cx + 9.0, y0 - 30.0))).unwrap();
    close(&p, json!({ "scale": { "x": 1.0, "y": (hgt + 30.0) / hgt }, "pivot": { "x": cx, "y": y1 } }));
    ok(&h, "edit.undo", json!({}));
    // Just outside the top-right corner (about 11 view pixels out along the diagonal): rotate about the centre; a
    // quarter turn counter-clockwise around the centre gives +90.
    let out = (x1 + 8.0, y1 + 8.0);
    let (ox, oy) = (out.0 - cx, out.1 - cy);
    let (_, p) = select_drag(&h, d(out, (cx - oy, cy + ox))).unwrap();
    close(&p, json!({ "rotate": 90.0, "pivot": { "x": cx, "y": cy } }));
    ok(&h, "edit.undo", json!({}));
    // Inside the box (away from the handles): a move, as built.
    let (_, p) = select_drag(&h, d((cx + 10.0, cy + 5.0), (cx + 40.0, cy - 15.0))).unwrap();
    assert_eq!(p["translate"], json!({ "x": 30.0, "y": -20.0 }), "{p}");
    ok(&h, "edit.undo", json!({}));
    // Far outside (beyond the rotation zone): a marquee.
    let (id, _) = select_drag(&h, d((x1 + 60.0, y1 + 60.0), (x1 + 90.0, y1 + 90.0))).unwrap();
    assert_eq!(id, "draw.selection.set");
    let [x0, y0, x1, y1] = selected_rectangle(&h, 300.0, 100.0, 400.0, 160.0);
    // A corner dragged onto its pivot: a degenerate scale, refused with nothing committed.
    let rev = ok(&h, "draw.drawing.get", json!({}))["rev"].clone();
    assert_eq!(select_drag(&h, d((x1, y1), (x0, y0))), Err("arg.invalid".to_owned()));
    assert_eq!(ok(&h, "draw.drawing.get", json!({}))["rev"], rev, "nothing committed");
    // The drawing pivot on a corner wins over the corner handle (the order of the tests).
    ok(&h, "draw.pivot.set", json!({ "point": { "x": x1, "y": y1 } }));
    let (id, _) = select_drag(&h, d((x1, y1), (x1 + 20.0, y1 + 20.0))).unwrap();
    assert_eq!(id, "draw.pivot.set");
}
