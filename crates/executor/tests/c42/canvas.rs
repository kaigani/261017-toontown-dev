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
    let p = h.canvas_drawing(None, 5, 0).unwrap();
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
    let a = h.canvas_drawing(Some(D1), 1, 0).unwrap();
    let b = h.canvas_drawing(None, 1, 0).unwrap();
    assert_eq!(a, b, "an evicted drawing is rebuilt identically (same revision, same bucket)");
    let finer = h.canvas_drawing(Some(D1), 1, 2).unwrap();
    assert_eq!(finer.rev, a.rev, "a bucket change is a reload at the same revision");
    assert!(finer.bytes.len() > a.bytes.len(), "a finer bucket flattens curves into more vertices");
    let e = pointer(&h, "pencil", drag(&[(700.0, 200.0), (720.0, 210.0)]));
    assert!(e.is_ok(), "{e:?}");
    let c = h.canvas_drawing(None, 1, 0).unwrap();
    assert!(c.rev > a.rev, "an edit moves the drawing's revision ({} -> {})", a.rev, c.rev);
    assert_eq!(decode(&c.bytes).unwrap().identity.rev, c.rev);
}

#[test]
fn a_payload_names_its_refusals() {
    let h = ready();
    let e = h.canvas_drawing(Some("dr_01K7ZZZZZZZZZZZZZZZZZZZZZZ"), 1, 0).unwrap_err();
    assert_eq!(e.code, "ref.notFound", "{e:?}");
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": null }));
    let e = h.canvas_drawing(None, 1, 0).unwrap_err();
    assert_eq!(e.code, "drawing.noTarget", "{e:?}");
}
