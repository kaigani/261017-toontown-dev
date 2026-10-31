//! M2 C2.2 readiness item 10: replay with painting. An agent session drives real MCP sequences mixing stroke edits,
//! gap closing, painting by ID and by point, a gradient fill mapping, paint unpainted, colour recovery of pasted
//! content, an atomic batch, a dry run, checkpoint restore and undo/redo, then exports its activity as a batch script.
//! Replaying the script through actual script mode into a fresh executor with the same initial document reproduces
//! every canonical entry byte for byte and the same derived planar graph (zones, fills, side zone IDs).
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};
use support::*;
use tt_executor::ExecutorHandle;

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";

fn autonomous(dir: &std::path::Path) -> tt_mcp::ServerOptions {
    let mut o = opts(dir);
    o.policy = Some(tt_executor::agents::Policy::Autonomous);
    o
}

fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .iter()
        .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(k).unwrap()).unwrap())
        .collect()
}

fn zones(h: &ExecutorHandle) -> Value {
    h.invoke(tt_executor::Invocation::user("draw.drawing.get", json!({ "drawingId": D1, "include": ["zones"] })))
        .result
        .unwrap()["nodes"]
        .clone()
}

#[test]
fn painting_sessions_replay_byte_for_byte_through_script_mode() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let initial = h.snapshot().unwrap().unwrap();
    let colours: Vec<String> =
        initial.palettes().flat_map(|p| p.swatches().iter().map(|s| s.colour_id().as_str().to_owned())).collect();
    let (mut c, _) = Client::connect(&h, autonomous(&dir), json!({}));
    let ok = |c: &mut Client, id: &str, params: Value| {
        let e = c.run(id, params.clone());
        assert_eq!(e["status"], "ok", "{id} {params}: {e}");
        e
    };
    // A closed line, painted by point, then split by a stroke in the same layer (fills carried over).
    let r = ok(
        &mut c,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false, "closed": true,
                "points": [{"x": 300, "y": 300}, {"x": 400, "y": 300}, {"x": 400, "y": 400}, {"x": 300, "y": 400}] }),
    );
    let layer = r["result"]["layerId"].as_str().unwrap().to_owned();
    ok(
        &mut c,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 350, "y": 350}], "colourId": colours[1] }),
    );
    ok(
        &mut c,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "invisible", "layer": layer, "fit": false, "points": [{"x": 330, "y": 280}, {"x": 330, "y": 420}] }),
    );
    // Paint by zone ID with the revision read; a gradient mapping by drag.
    let read = ok(&mut c, "draw.drawing.get", json!({ "drawingId": D1, "include": ["zones"] }));
    let l = read["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == layer).unwrap().clone();
    let zone = l["zones"][0]["id"].clone();
    ok(
        &mut c,
        "colour.paint.apply",
        json!({ "drawingId": D1, "objectIds": [zone], "colourId": colours[2], "expectRev": read["result"]["rev"] }),
    );
    ok(
        &mut c,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 370, "y": 350}], "colourId": colours[3], "mapping": {"from": {"x": 330, "y": 350}, "to": {"x": 400, "y": 350}} }),
    );
    // Gap closing while painting an open square.
    let g = ok(
        &mut c,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": "new", "fit": false,
                "points": [{"x": 500, "y": 400}, {"x": 500, "y": 300}, {"x": 600, "y": 300}, {"x": 600, "y": 400}, {"x": 551, "y": 400}] }),
    );
    ok(
        &mut c,
        "draw.stroke.add",
        json!({ "drawingId": D1, "kind": "pencil", "layer": g["result"]["layerId"], "fit": false, "points": [{"x": 549, "y": 400}, {"x": 500, "y": 400}] }),
    );
    ok(
        &mut c,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 550, "y": 350}], "colourId": colours[4], "gapClosing": "small" }),
    );
    // An atomic batch painting and unpainting; a dry run (left out of the replay).
    let atomic = c.call(
        "batch",
        json!({ "label": "Repaint", "atomic": true, "steps": [
        { "id": "colour.paint.remove", "params": { "drawingId": D1, "points": [{"x": 550, "y": 350}] } },
        { "id": "colour.paintUnpainted.apply", "params": { "drawingId": D1, "all": true, "colourId": colours[5] } },
    ] }),
    );
    assert_eq!(atomic["structuredContent"]["status"], "ok", "{atomic}");
    let dry = c.call("run_command", json!({ "id": "colour.paint.apply", "dryRun": true, "params": { "drawingId": D1, "points": [{"x": 310, "y": 350}], "colourId": colours[0] } }));
    assert_eq!(dry["structuredContent"]["status"], "ok", "{dry}");
    // Checkpoint restore, undo and redo.
    ok(&mut c, "checkpoint.create", json!({ "name": "painted" }));
    ok(
        &mut c,
        "colour.paint.apply",
        json!({ "drawingId": D1, "points": [{"x": 310, "y": 350}], "colourId": colours[0] }),
    );
    ok(&mut c, "checkpoint.restore", json!({ "name": "painted" }));
    ok(&mut c, "edit.undo", json!({}));
    ok(&mut c, "edit.redo", json!({}));
    // Content pasted with a colour recovered from its copy.
    ok(
        &mut c,
        "draw.content.add",
        json!({ "drawingId": D1, "content": {
            "nodes": [{ "type": "layer", "id": "l", "layerKind": "pencil", "strokes": [
                { "id": "s", "kind": "pencil", "closed": false, "colourId": "col_01K9RRRRRRRRRRRRRRRRRRRRRR",
                  "lineStyle": {"thickness": 1, "lineType": "solid"},
                  "bezier": [{"x": 700, "y": 300}, {"x": 720, "y": 300}, {"x": 740, "y": 300}, {"x": 760, "y": 300}] }]}],
            "colourRecovery": [{ "colourId": "col_01K9RRRRRRRRRRRRRRRRRRRRRR", "paletteId": "pal_01K9RRRRRRRRRRRRRRRRRRRRRR",
                "paletteName": "Imported", "styles": [{"styleId": "sty_01K9RRRRRRRRRRRRRRRRRRRRRR", "styleName": "Default"}],
                "swatch": {"colourId": "col_01K9RRRRRRRRRRRRRRRRRRRRRR", "type": "swatch", "rev": 1, "name": "Ink",
                           "kind": "solid", "deformable": false, "values": {"sty_01K9RRRRRRRRRRRRRRRRRRRRRR": {"rgba": "#123456FF"}}} }]
        } }),
    );
    let target = dir.join("replay.json");
    let exported = c.run("agent.activity.export", json!({ "path": path(&target), "format": "batch" }));
    assert_eq!(exported["status"], "ok", "{exported}");
    let fresh = executor(&dir.join("fresh-user"));
    assert!(fresh.open_document(initial, None, false).is_ok());
    let outcome = tt_mcp::script::run(&fresh, &autonomous(&dir), tt_mcp::script::load(&target).unwrap(), None).unwrap();
    assert!(outcome.ok, "{:?}", outcome.envelope);
    assert_eq!(canonical(&fresh), canonical(&h), "replay reproduces every canonical entry");
    assert_eq!(zones(&fresh), zones(&h), "and the same derived planar graph");
    assert_eq!(fresh.snapshot().unwrap().unwrap().document_rev(), h.snapshot().unwrap().unwrap().document_rev());
    c.close().unwrap();
    drop(h);
    drop(fresh);
    let _ = std::fs::remove_dir_all(&dir);
}
