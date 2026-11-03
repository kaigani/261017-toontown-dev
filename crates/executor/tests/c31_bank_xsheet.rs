//! M3 C3.1: drawing banks (04 §13.11–§13.14, §14) and the exposure sheet as JSON (04 §17.2) through the executor:
//! usage and unused drawings, create/rename/duplicate/delete with the `needs_confirmation` path, clear and lock,
//! and `xsheet.exposures.get/set` (windows, `continues`, validation codes with column/run pointers, atomicity,
//! `expectRev`, `dryRun`, created drawings).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Status};

const INTRO: &str = "scene:\"Intro\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const NOTES: &str = "scene:\"Intro\"/element:\"Notes\"";
const D1: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
const D2: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"2\"";
const D10: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"10\"";

fn exposure(h: &ExecutorHandle, e: &str) -> Value {
    get(h, e)["exposure"].clone()
}

#[test]
fn drawing_list_usage_filters_and_thumbnails() {
    let h = fixture();
    let l = ok(&h, "element.drawing.list", json!({ "elementId": BOB }))["drawings"].clone();
    let names: Vec<&str> = l.as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["1", "2", "10"], "natural name order");
    assert_eq!(l[0]["exposures"], json!([{ "from": 1, "to": 6 }, { "from": 21, "to": 44 }]));
    assert_eq!(l[0]["useCount"], 30);
    assert_eq!(l[1]["locked"], true);
    // `10` is exposed nowhere but used by the Dialogue lip-sync mapping: not unused (10 §7.2).
    let unused = ok(&h, "element.drawing.list", json!({ "elementId": BOB, "filter": "unused" }))["drawings"].clone();
    assert_eq!(unused, json!([]));
    let t = ok(&h, "element.drawing.list", json!({ "elementId": BOB, "thumbnails": 16 }))["drawings"].clone();
    assert!(t[0]["thumbnail"].as_str().unwrap().len() > 20, "a synchronous PNG thumbnail");
    assert_eq!(
        err(&h, "element.drawing.list", json!({ "elementId": "scene:\"Intro\"/element:\"Bob-P\"" })).code,
        "element.notCellBased"
    );
}

#[test]
fn unused_drawings_of_a_scene() {
    let h = fixture();
    ok(&h, "element.drawing.create", json!({ "elementId": BOB, "name": "spare" }));
    let items = ok(&h, "element.unusedDrawing.list", json!({ "sceneId": INTRO }))["items"].clone();
    let names: Vec<&str> = items.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["spare"]);
}

#[test]
fn create_rename_duplicate() {
    let h = fixture();
    let r = ok(&h, "element.drawing.create", json!({ "elementId": BOB }));
    assert_eq!(r["name"], "11", "next after the highest numeric name");
    let r =
        ok(&h, "element.drawing.create", json!({ "elementId": BOB, "name": "A", "exposeFrom": 45, "exposeTo": 46 }));
    assert_eq!(exposure(&h, BOB).as_array().unwrap().last().unwrap()[1], 2);
    let a = r["drawingId"].as_str().unwrap().to_owned();
    assert_eq!(
        err(&h, "element.drawing.create", json!({ "elementId": BOB, "name": "A" })).code,
        "drawing.nameDuplicate"
    );
    assert_eq!(err(&h, "element.drawing.create", json!({ "elementId": BOB, "name": "" })).code, "drawing.nameEmpty");
    assert_eq!(
        err(&h, "element.drawing.create", json!({ "elementId": BOB, "name": "a:b" })).code,
        "drawing.nameInvalid"
    );
    assert_eq!(
        err(&h, "element.drawing.create", json!({ "elementId": "scene:\"Intro\"/element:\"Background\"" })).code,
        "cell.notApplicable",
        "image drawings hold exactly one bitmap"
    );
    ok(&h, "element.drawing.rename", json!({ "drawingId": a, "name": "B" }));
    assert_eq!(get(&h, &a)["name"], "B");
    assert_eq!(err(&h, "element.drawing.rename", json!({ "drawingId": a, "name": "1" })).code, "drawing.nameDuplicate");
    ok(&h, "element.drawing.rename", json!({ "drawingId": a, "name": "B" }));
    let r = ok(&h, "element.drawing.duplicate", json!({ "drawingId": D1 }));
    assert_eq!(r["name"], "12");
    let content = |d: &str| ok(&h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"].clone();
    assert_eq!(content(r["drawingId"].as_str().unwrap()), content(D1));
    // A clone sees the renamed drawing.
    let clone = ok(&h, "element.clone", json!({ "elementId": BOB }))["elementId"].as_str().unwrap().to_owned();
    let l = ok(&h, "element.drawing.list", json!({ "elementId": clone }))["drawings"].clone();
    assert!(l.as_array().unwrap().iter().any(|d| d["name"] == "B"));
}

#[test]
fn delete_needs_confirmation_when_exposed() {
    let h = fixture();
    let before = exposure(&h, BOB);
    let env = run(&h, "element.drawing.delete", json!({ "drawingIds": [D1] }));
    assert_eq!(env.status, Status::NeedsConfirmation, "{env:?}");
    let res = env.result.unwrap();
    assert_eq!(res["code"], "drawing.inUse");
    assert_eq!(res["retryWith"], json!({ "ifInUse": "clearCells" }));
    assert_eq!(res["consequences"].as_array().unwrap().len(), 2, "two exposures of drawing 1");
    assert_eq!(exposure(&h, BOB), before, "nothing changed");
    let r = ok(&h, "element.drawing.delete", json!({ "drawingIds": [D1], "ifInUse": "clearCells" }));
    assert_eq!(r["clearedCells"][0]["from"], 1);
    assert_eq!(get(&h, BOB)["exposure"][0][0], Value::Null);
    ok(&h, "edit.undo", json!({}));
    assert_eq!(exposure(&h, BOB), before, "undo restores the drawing and its cells");
    // Deleting a drawing a lip-sync mapping uses clears the mapping entry.
    let r = ok(&h, "element.drawing.delete", json!({ "drawingIds": [D10] }));
    assert_eq!(r["clearedCells"], json!([]));
    let sound = get(&h, "scene:\"Intro\"/element:\"Dialogue\"");
    assert_eq!(sound["lipSyncMappings"][0]["mapping"]["X"], Value::Null);
}

#[test]
fn clear_and_lock() {
    let h = fixture();
    assert_eq!(err(&h, "element.drawingContent.delete", json!({ "drawingIds": [D2] })).code, "drawing.locked");
    ok(&h, "element.drawingContent.delete", json!({ "drawingIds": [D1] }));
    assert_eq!(get(&h, D1)["nodes"], json!([]));
    assert_eq!(get(&h, BOB)["exposure"][0][1], 6, "cells keep their label");
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [D2], "locked": false }));
    assert_eq!(get(&h, D2)["locked"], false);
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [D1], "locked": true }));
    // A locked drawing refuses content commands (M2 draw.* honours the same flag).
    let e = err(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": { "x": 0, "y": 0 }, "to": { "x": 10, "y": 10 } }),
    );
    assert_eq!(e.code, "drawing.locked");
}

#[test]
fn exposure_sheet_get_window_and_parts() {
    let h = fixture();
    let s = ok(
        &h,
        "xsheet.exposures.get",
        json!({ "sceneId": INTRO, "from": 3, "to": 24, "include": ["notes", "mouths"] }),
    );
    assert_eq!(s["sceneLength"], 48);
    let cols = s["columns"].as_array().unwrap();
    let names: Vec<&str> = cols.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Bob", "Background", "Notes", "Dialogue", "Reference"]);
    let bob = &cols[0];
    assert_eq!(
        bob["exposures"][0],
        json!({ "drawing": "1", "drawingId": get(&h, D1)["id"], "from": 3, "to": 6, "continues": "before" })
    );
    assert_eq!(bob["exposures"][2]["continues"], "after");
    assert_eq!(bob["notes"], json!([{ "frame": 3, "text": "fix hand" }, { "frame": 12, "text": "hold" }]));
    assert_eq!(cols[2]["visible"], false, "hidden columns are included");
    assert_eq!(cols[3]["clips"][0]["from"], 3);
    assert!(!cols[3]["mouths"].as_array().unwrap().is_empty());
    assert_eq!(cols[4]["exposures"][0]["to"], 24);
}

#[test]
fn exposure_sheet_set_validation_is_atomic() {
    let h = fixture();
    let before = (exposure(&h, BOB), exposure(&h, NOTES));
    let cases = [
        (json!([{ "drawing": "1", "from": 5, "to": 3 }]), "xsheet.runInvalid", "/columns/1/exposures/0"),
        (
            json!([{ "drawing": "1", "from": 1, "to": 4 }, { "drawing": "1", "from": 4, "to": 6 }]),
            "xsheet.runOverlap",
            "/columns/1/exposures/1",
        ),
        (json!([{ "drawing": "1", "from": 1, "to": 30 }]), "xsheet.runOutOfRange", "/columns/1/exposures/0"),
        (json!([{ "drawing": "nope", "from": 1, "to": 2 }]), "cell.drawingNotFound", "/columns/1/exposures/0"),
    ];
    for (runs, code, pointer) in cases {
        let e = err(
            &h,
            "xsheet.exposures.set",
            json!({ "columns": [
                { "elementId": BOB, "from": 1, "to": 4, "exposures": [{ "drawing": "1", "from": 1, "to": 4 }] },
                { "elementId": BOB, "from": 1, "to": 24, "exposures": runs }
            ] }),
        );
        assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), (code, Some(pointer)), "{e:?}");
        assert_eq!((exposure(&h, BOB), exposure(&h, NOTES)), before, "either every column or none");
    }
    assert_eq!(
        err(
            &h,
            "xsheet.exposures.set",
            json!({ "columns": [{ "elementId": "scene:\"Intro\"/element:\"Dialogue\"", "exposures": [] }] })
        )
        .code,
        "xsheet.columnReadOnly"
    );
    assert_eq!(
        err(
            &h,
            "xsheet.exposures.set",
            json!({ "columns": [{ "elementId": "scene:\"Intro\"/element:\"Bob-P\"", "exposures": [] }] })
        )
        .code,
        "xsheet.columnReadOnly"
    );
}

#[test]
fn exposure_sheet_set_writes_windows_with_rev_and_dry_run() {
    let h = fixture();
    let rev0 = ok(&h, "xsheet.exposures.get", json!({ "sceneId": INTRO }))["rev"].as_u64().unwrap();
    let e = err(
        &h,
        "xsheet.exposures.set",
        json!({ "expectRev": rev0 + 7, "columns": [{ "elementId": BOB, "exposures": [] }] }),
    );
    assert_eq!(e.code, "xsheet.revisionMismatch");
    // dryRun: the diff, nothing applied.
    let before = exposure(&h, BOB);
    let r = ok(
        &h,
        "xsheet.exposures.set",
        json!({ "dryRun": true, "columns": [{ "elementId": BOB, "from": 1, "to": 4, "exposures": [{ "drawing": "10", "from": 1, "to": 4 }] }] }),
    );
    assert_eq!(r["changed"][0], json!({ "elementId": get(&h, BOB)["id"], "from": 1, "to": 4 }));
    assert_eq!(exposure(&h, BOB), before);
    // The 17.2 example: frames not covered become blank; `4` is created.
    let r = ok(
        &h,
        "xsheet.exposures.set",
        json!({ "sceneId": INTRO, "expectRev": rev0, "createMissingDrawings": true, "columns": [{ "elementId": BOB, "from": 1, "to": 24,
            "exposures": [{ "drawing": "1", "from": 1, "to": 6 }, { "drawing": "2", "from": 7, "to": 12 }, { "drawing": "4", "from": 13, "to": 24 }] }] }),
    );
    assert_eq!(r["createdDrawings"][0]["name"], "4");
    let s = ok(&h, "xsheet.exposures.get", json!({ "elementIds": [BOB], "from": 1, "to": 30 }));
    let runs: Vec<(String, u64, u64)> = s["columns"][0]["exposures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["drawing"].as_str().unwrap().to_owned(), r["from"].as_u64().unwrap(), r["to"].as_u64().unwrap()))
        .collect();
    assert_eq!(runs, [("1".into(), 1, 6), ("2".into(), 7, 12), ("4".into(), 13, 24), ("1".into(), 25, 30)]);
    assert_eq!(s["rev"], r["rev"]);
    assert!(r["rev"].as_u64().unwrap() > rev0);
    // One undo step.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(exposure(&h, BOB), before);
    // Without `to`, the tail is replaced: frames after the last run become blank.
    ok(
        &h,
        "xsheet.exposures.set",
        json!({ "columns": [{ "elementId": BOB, "exposures": [{ "drawing": "1", "from": 1, "to": 2 }] }] }),
    );
    assert_eq!(exposure(&h, BOB), json!([[get(&h, D1)["id"], 2]]));
}

#[test]
fn light_table_list_tracks_sources() {
    let h = fixture();
    let r = ok(&h, "xsheet.lightTable.list", json!({}));
    assert_eq!(r["items"], json!([]));
}
