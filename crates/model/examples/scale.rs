//! Measures the model at the sizes 10 §5 says it MUST handle: a drawing with 100 000 strokes, a palette with 10 000
//! swatches, a scene with 1 000 elements, a bank with 10 000 drawings. Prints one JSON log (environment, sizes, wall
//! times in ms) to stdout. Run: `cargo run --release -p tt-model --example scale > scale.json`.
#![allow(clippy::print_stdout, clippy::unwrap_used, missing_docs, reason = "measurement tool")]

use std::time::Instant;

use serde_json::{Value, json};
use tt_model::change;
use tt_model::document::{Document, EntryKey, RenderOptions};
use tt_model::entities::drawing::{Drawing, DrawingBank};
use tt_model::entities::element::Element;
use tt_model::entities::palette::Palette;
use tt_model::entities::scene::Scene;
use tt_model::fixture::sample_parts;
use tt_model::object::from_json;

fn ms(t: Instant) -> f64 {
    (t.elapsed().as_secs_f64() * 1e6).round() / 1e3
}

fn time(log: &mut serde_json::Map<String, Value>, name: &str, f: impl FnOnce()) {
    let t = Instant::now();
    f();
    log.insert(name.to_owned(), json!(ms(t)));
}

fn main() {
    let mut parts = sample_parts(1).unwrap();
    let mut log = serde_json::Map::new();

    // 100 000 strokes in one layer of drawing 1.
    let drawing_idx = parts.drawings.iter().position(|d| d.name() == "1").unwrap();
    let mut d: Value = serde_json::to_value(&parts.drawings[drawing_idx]).unwrap();
    let strokes: Vec<Value> = (0..100_000)
        .map(|i| {
            let x = f64::from(i % 1000) * 1.5;
            let y = f64::from(i / 1000) * 2.25;
            json!({"id": format!("s{i}"), "type": "stroke", "rev": 1, "kind": "pencil", "closed": false,
                   "points": [[x, y, 0, 0, 0.5, 0.25], [x + 1.0, y + 0.5, -0.5, 0, 0, 0], [x + 2.0, y, 0, 0, 0, 0]],
                   "colourId": null, "fillMatrix": null, "lineStyle": {"thickness": 1, "lineType": "solid"},
                   "spine": null, "sides": {"left": null, "right": null}})
        })
        .collect();
    d["nodes"][0]["strokes"] = Value::Array(strokes);
    let text = serde_json::to_string(&d).unwrap();
    log.insert("drawingJsonBytes".into(), json!(text.len()));
    let mut big: Option<Drawing> = None;
    time(&mut log, "drawing100kParseValidateMs", || {
        let v: Value = serde_json::from_str(&text).unwrap();
        big = Some(from_json::<Drawing>(&v).unwrap());
    });
    parts.drawings[drawing_idx] = big.take().unwrap();

    // 10 000 swatches in palette 1.
    let mut p: Value = serde_json::to_value(&parts.palettes[0]).unwrap();
    let styles: Vec<String> =
        p["styles"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_owned()).collect();
    let mut swatches = p["swatches"].as_array().unwrap().clone();
    for i in 0..10_000 {
        let values: serde_json::Map<String, Value> =
            styles.iter().map(|s| (s.clone(), json!({"rgba": format!("#{:06X}FF", i * 97 % 0xFF_FFFF)}))).collect();
        swatches
            .push(json!({"colourId": format!("col_x{i}"), "type": "swatch", "rev": 1, "name": format!("Colour {i}"),
                             "kind": "solid", "deformable": false, "values": values}));
    }
    p["swatches"] = Value::Array(swatches);
    time(&mut log, "palette10kParseValidateMs", || parts.palettes[0] = from_json::<Palette>(&p).unwrap());

    // 10 000 drawings in the second scene's bank and 1 000 peg elements in the second scene.
    let bank_idx = parts
        .banks
        .iter()
        .position(|b| {
            b.drawings().ids().is_empty()
                && parts.elements.iter().any(|e| e.drawing_bank_id().as_ref() == Some(b.id()) && e.name() == "Drawing")
        })
        .unwrap();
    let mut stubs = Vec::new();
    time(&mut log, "drawings10kParseValidateMs", || {
        for i in 0..10_000 {
            let id = format!("dr_x{i}");
            let dj = json!({"id": id, "type": "drawing", "rev": 1, "name": format!("{}", i + 1), "locked": false,
                            "pivot": {"x": 0, "y": 0}, "mouthShape": null, "nodes": [], "colourRecovery": [],
                            "createdAt": "2026-10-17T09:30:12.345Z"});
            parts.drawings.push(from_json::<Drawing>(&dj).unwrap());
            stubs.push(json!({"id": id, "type": "drawing"}));
        }
    });
    let mut b: Value = serde_json::to_value(&parts.banks[bank_idx]).unwrap();
    b["drawings"] = Value::Array(stubs);
    parts.banks[bank_idx] = from_json::<DrawingBank>(&b).unwrap();
    let scene_idx = 1;
    let mut s: Value = serde_json::to_value(&parts.scenes[scene_idx]).unwrap();
    time(&mut log, "elements1kParseValidateMs", || {
        for i in 0..1000 {
            let id = format!("el_x{i}");
            let ej = json!({"id": id, "type": "element", "rev": 1, "name": format!("Peg {i}"), "elementType": "peg",
                "start": 1, "duration": 48, "loops": 1, "loopType": "count", "positionLoop": "cycle", "role": "normal",
                "deformable": false, "visibleInScene": true, "locked": false, "collapsed": false, "markerColour": null,
                "backgroundColour": null, "columnWidth": 80,
                "transform": {"offset": {"x": 0, "y": 0, "z": 0}, "rotation": 0, "skew": 0, "scale": {"x": 1, "y": 1},
                    "keepProportions": false, "pivot": {"x": 0, "y": 0}, "applyPivotTo": "rotation",
                    "useChildDrawingPivot": false, "lockDepth": false, "orientToPath": false},
                "exportGroup": {"groupOnVectorExport": false, "addComposite": false}, "channels": {},
                "createdAt": "2026-10-17T09:30:12.345Z"});
            parts.elements.push(from_json::<Element>(&ej).unwrap());
            s["elements"].as_array_mut().unwrap().push(json!({"id": id, "type": "element"}));
            s["tree"].as_array_mut().unwrap().push(json!({"id": id, "children": []}));
        }
    });
    parts.scenes[scene_idx] = from_json::<Scene>(&s).unwrap();

    let mut doc: Option<Document> = None;
    let p2 = parts.clone();
    time(&mut log, "documentAssembleAndCheckMs", || doc = Some(Document::from_parts(p2.clone()).unwrap()));
    let doc = doc.unwrap();
    log.insert("objectCount".into(), json!(doc.object_count()));
    let drawing_id = parts.drawings[drawing_idx].id().clone();
    time(&mut log, "drawingCanonicalWriteMs", || {
        let _ = doc.entry_canonical(&EntryKey::Drawing(drawing_id.clone())).unwrap();
    });
    time(&mut log, "drawingObjectJsonDepthAllWithPathsMs", || {
        let _ = doc.object_json(drawing_id.as_str(), RenderOptions { depth: None, path: true }).unwrap();
    });
    time(&mut log, "documentCloneMs", || {
        let _ = doc.clone();
    });
    let key0 = EntryKey::Drawing(drawing_id.clone());
    time(&mut log, "drawingEntryRawJsonMs", || {
        let _ = doc.entry_raw_json(&key0).unwrap();
    });
    time(&mut log, "drawingOwnViewsMs", || {
        let _ = change::own_objects(&doc, &key0).unwrap();
    });
    // One stroke changed: diff cost is proportional to the changed entry (the 100k-stroke drawing).
    let mut parts2 = parts.clone();
    let mut d2: Value = serde_json::to_value(&parts2.drawings[drawing_idx]).unwrap();
    d2["nodes"][0]["strokes"][50_000]["closed"] = json!(true);
    parts2.drawings[drawing_idx] = from_json::<Drawing>(&d2).unwrap();
    let doc2 = Document::assemble(parts2);
    // Whole-document diff: doc2 was assembled from cloned parts, so no entry is shared and every object is compared
    // (the executor's snapshots share unchanged entries and diff only those that changed).
    let mut n = 0;
    time(&mut log, "diffWholeDocumentNoSharedEntriesMs", || n = change::diff(&doc, &doc2).unwrap().len());
    log.insert("diffWholeDocumentChanges".into(), json!(n));
    // The cost when only the drawing entry changed: own views of that entry on both sides, compared per object.
    let key = EntryKey::Drawing(drawing_id.clone());
    let mut changed = 0;
    time(&mut log, "diffOneStrokeIn100kStrokeEntryMs", || {
        let a: std::collections::HashMap<String, Value> =
            change::own_objects(&doc, &key).unwrap().into_iter().map(|o| (o.key, o.json)).collect();
        for o in change::own_objects(&doc2, &key).unwrap() {
            if a.get(&o.key) != Some(&o.json) {
                let _ = change::diff_json(&a[&o.key], &o.json, "", o.ty);
                changed += 1;
            }
        }
    });
    log.insert("diffOneStrokeChangedObjects".into(), json!(changed));
    let out = json!({
        "tool": "crates/model/examples/scale.rs",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "timesAreWallClockMs": true,
        "commit": option_env!("TT_COMMIT").unwrap_or("see receipt"),
        "measurements": log,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
