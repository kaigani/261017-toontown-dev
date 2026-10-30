//! M2 C2.1 obligation 9 (02 §13.1 item 9, 11 P7): a fixed script of drawing commands — fitted brush strokes with
//! pressure and every tip shape, pencil and invisible strokes, text, groups, deletion, content insertion, a spine
//! re-sweep and a drawing created in an empty cell — produces the same canonical entries in two executors of one run
//! and the committed golden digest on every OS (CI: Windows x86_64 and macOS aarch64). No randomness is involved:
//! IDs come from the transaction's seeded stream and geometry from correctly rounded operations (ADR-0016 §3).
//!
//! Normalised before hashing: `createdAt` of drawings created by the script (wall clock, session metadata) and the
//! project's `modifiedAt`. Text layout bounds are not part of the persisted state. Recording:
//! `TT_UPDATE_GOLDEN=1 cargo test -p tt-executor --test c21_determinism`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;
use tt_vector::num::{cos, sin};

const BOB: &str = "scene:#1/element:\"Bob\"";
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn walk(n: usize, seed: f64) -> Value {
    // A fixed freehand path (a deterministic function of n; no RNG).
    Value::Array(
        (0..n)
            .map(|i| {
                let t = i as f64 / (n as f64 - 1.0);
                let x = t * 200.0 - 100.0;
                // tt-vector's software sine and cosine, so the inputs themselves are identical on every platform.
                let y = sin(t * 7.0 + seed) * 40.0 + cos(t * 23.0) * 6.0;
                json!({ "x": (x * 1000.0).round() / 1000.0, "y": (y * 1000.0).round() / 1000.0, "pressure": (sin(t * 3.0 + seed).abs() * 1000.0).round() / 1000.0 })
            })
            .collect(),
    )
}

fn script(h: &ExecutorHandle) {
    let walk_a = walk(120, 0.3);
    let walk_b = walk(64, 1.7);
    for (tip, angle) in [("round", 0), ("oval", 25), ("square", 0), ("flat", 30)] {
        ok(
            h,
            "draw.stroke.add",
            json!({ "drawingId": D1, "kind": "brush", "points": walk_a.clone(), "smoothness": 2,
            "pen": { "tipShape": tip, "minSize": 1, "maxSize": 9, "tipAngle": angle } }),
        );
    }
    ok(h, "draw.stroke.add", json!({ "drawingId": D1, "kind": "pencil", "points": walk_b.clone(), "thickness": 2.5 }));
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "points": walk_b.clone(), "fit": false, "closed": true, "layer": "top" }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "points": [{"x": -100, "y": 0}, {"x": 100, "y": 0}], "fit": false }),
    );
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "brush", "points": [{"x": 3, "y": 4, "pressure": 0.5}] }),
    );
    let t = ok(
        h,
        "draw.text.add",
        json!({ "drawingId": D1, "position": {"x": -50, "y": 80}, "text": "Déterminisme\nzwei", "format": {"size": 30, "italic": true} }),
    );
    ok(
        h,
        "draw.textFormat.set",
        json!({ "drawingId": D1, "textId": t["textId"], "start": 2, "length": 4, "bold": true, "kerning": 12 }),
    );
    ok(h, "draw.text.set", json!({ "drawingId": D1, "textId": t["textId"], "insert": {"at": 0, "text": "¡"}, }));
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    let n = d["nodes"].as_array().unwrap().len();
    let ids: Vec<Value> = d["nodes"].as_array().unwrap()[n - 3..].iter().map(|x| x["id"].clone()).collect();
    ok(h, "draw.group.create", json!({ "drawingId": D1, "ids": ids }));
    let brush = d["nodes"][3]["strokes"][0]["id"].clone();
    ok(
        h,
        "draw.stroke.set",
        json!({ "drawingId": D1, "strokeId": brush, "pressure": [{"t": 0, "p": 0.1}, {"t": 0.5, "p": 1}, {"t": 1, "p": 0.3}] }),
    );
    ok(h, "draw.object.delete", json!({ "drawingId": D1, "ids": ["st_01K742SG0S0NT23BWCZN64SFQ5"] }));
    let copy = ok(h, "draw.drawing.get", json!({ "drawingId": D1 }));
    ok(
        h,
        "draw.content.add",
        json!({ "elementId": BOB, "frame": 7, "content": copy, "offset": {"x": 5.5, "y": -2.25} }),
    );
    ok(h, "draw.pivot.set", json!({ "drawingId": D1, "point": {"x": 1.25, "y": -7} }));
}

/// The canonical text of every entry the script can touch, normalised as documented above.
fn state(h: &ExecutorHandle) -> String {
    let doc = h.snapshot().unwrap().unwrap();
    let mut out = String::new();
    for key in doc.entry_keys() {
        let mut v = doc.entry_raw_json(&key).unwrap();
        if let Some(m) = v.as_object_mut() {
            m.remove("modifiedAt");
            if m.get("type").and_then(Value::as_str) == Some("drawing")
                && m.get("rev").and_then(Value::as_u64) != Some(1)
            {
                m.insert("createdAt".into(), json!("<created by the script>"));
            }
        }
        out.push_str(&format!("{key:?}\n"));
        out.push_str(&tt_model::canonical::to_canonical_string(&v).unwrap());
    }
    out
}

#[test]
fn command_created_geometry_is_identical_across_runs_and_platforms() {
    let a = fixture();
    script(&a);
    let b = fixture();
    script(&b);
    let sa = state(&a);
    assert_eq!(sa, state(&b), "two executors of one run");
    let digest = tt_persist::fsx::sha256(sa.as_bytes());
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/c21-determinism.json");
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        let d = a.snapshot().unwrap().unwrap();
        let drawing =
            d.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D1).unwrap())).unwrap();
        let text = serde_json::to_string_pretty(&json!({ "sha256": digest, "bytes": sa.len(), "drawing1": drawing }))
            .unwrap()
            + "\n";
        std::fs::write(&golden, text).unwrap();
        return;
    }
    let g: Value = serde_json::from_str(&std::fs::read_to_string(&golden).unwrap()).unwrap();
    if g["sha256"] != digest.as_str() {
        // Show where the first drawing differs from the recorded one before failing.
        let d = a.snapshot().unwrap().unwrap();
        let drawing =
            d.entry_raw_json(&tt_model::document::EntryKey::Drawing(tt_model::Id::parse(D1).unwrap())).unwrap();
        assert_eq!(drawing, g["drawing1"], "drawing 1 differs from the golden recording");
        panic!("canonical state digest {digest} differs from the golden {}", g["sha256"]);
    }
}
