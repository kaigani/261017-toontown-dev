//! M3 C3.1: the cell and exposure commands of 04 §13 and §17.4 through the executor — every command's effect on the
//! fixture's Bob column (`1`×6, blank×2, `2`×12, `1`×24; bank `1`, `2` (locked), `10`), the 04 §17.6 error codes,
//! one undo step per call, explicit targets versus the selection, events, and the 13.20 availability rules.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const NOTES: &str = "scene:\"Intro\"/element:\"Notes\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const D2: &str = "dr_01K742SG0M5J3BAJSW8241WA87";
const D10: &str = "dr_01K742SG0NG5HTFZRD7238FZXJ";

/// The cells of a column as labels (`.` = blank), frame 1 up to its last exposure.
fn cells(h: &ExecutorHandle, element: &str) -> String {
    let e = get(h, element);
    let mut out = Vec::new();
    for r in e["exposure"].as_array().cloned().unwrap_or_default() {
        let label = match r[0].as_str() {
            None => ".".to_owned(),
            Some(d) => get(h, d)["name"].as_str().unwrap().to_owned(),
        };
        for _ in 0..r[1].as_u64().unwrap() {
            out.push(label.clone());
        }
    }
    out.join(" ")
}

/// Run-length form of `cells`: `1x6 .x2 2x12 1x24`.
fn runs(h: &ExecutorHandle, element: &str) -> String {
    let e = get(h, element);
    e["exposure"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let label = r[0].as_str().map_or(".".to_owned(), |d| get(h, d)["name"].as_str().unwrap().to_owned());
            format!("{label}x{}", r[1])
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn undo_restores(h: &ExecutorHandle, element: &str, before: &str) {
    let r = rev(h);
    ok(h, "edit.undo", json!({}));
    assert_eq!(runs(h, element), before, "one undo step restores the column");
    assert!(rev(h) > r);
}

#[test]
fn fixture_column_is_as_documented() {
    let h = fixture();
    assert_eq!(runs(&h, BOB), "1x6 .x2 2x12 1x24");
}

#[test]
fn add_remove_and_set_exposure_are_one_step_each() {
    let h = fixture();
    let before = runs(&h, BOB);
    let r = ok(&h, "cell.exposure.add", json!({ "elementIds": [BOB], "from": 1, "to": 1, "frames": 2 }));
    assert_eq!(runs(&h, BOB), "1x8 .x2 2x12 1x24");
    assert_eq!(r["changed"][0]["from"], 7);
    undo_restores(&h, BOB, &before);

    ok(&h, "cell.exposure.remove", json!({ "elementIds": [BOB], "from": 9, "to": 9, "frames": 5 }));
    assert_eq!(runs(&h, BOB), "1x6 .x2 2x7 1x24");
    undo_restores(&h, BOB, &before);

    // Multi-cell: every distinct exposure intersecting the range (blank run included) — 13.1.
    ok(&h, "cell.exposure.add", json!({ "elementIds": [BOB], "from": 6, "to": 9 }));
    assert_eq!(runs(&h, BOB), "1x7 .x3 2x13 1x24");
    undo_restores(&h, BOB, &before);

    // On twos: exposures re-timed, blank runs keep their length (04 §18 #11).
    ok(&h, "cell.exposure.set", json!({ "elementIds": [BOB], "from": 1, "to": 30, "length": 2 }));
    assert_eq!(runs(&h, BOB), "1x2 .x2 2x2 1x2");
    undo_restores(&h, BOB, &before);
}

#[test]
fn fill_overwrites_and_reports_the_minimum() {
    let h = fixture();
    let before = runs(&h, BOB);
    ok(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": 10 }));
    assert_eq!(runs(&h, BOB), "1x10 2x10 1x24", "overwritten, not inserted");
    undo_restores(&h, BOB, &before);
    let e = err(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 9, "toFrame": 3 }));
    assert_eq!(e.code, "cell.frameBeforeSelection");
    assert_eq!(e.details["minimum"], 9);
    let e = err(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 7, "toFrame": 9 }));
    assert_eq!(e.code, "cell.notApplicable", "a blank cell has nothing to extend");
    let e = err(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 0, "toFrame": 9 }));
    assert_eq!(e.code, "cell.frameOutOfRange");
    assert_eq!(runs(&h, BOB), before, "failed commands change nothing");
}

#[test]
fn blank_cells_insert_and_delete() {
    let h = fixture();
    let before = runs(&h, BOB);
    ok(&h, "cell.blank.add", json!({ "elementIds": [BOB], "frame": 3, "count": 2 }));
    assert_eq!(runs(&h, BOB), "1x2 .x2 1x4 .x2 2x12 1x24");
    undo_restores(&h, BOB, &before);
    let e = err(&h, "cell.blank.remove", json!({ "elementIds": [BOB], "frame": 3 }));
    assert_eq!(e.code, "cell.notBlank");
    ok(&h, "cell.blank.remove", json!({ "elementIds": [BOB], "frame": 7, "count": 2 }));
    assert_eq!(runs(&h, BOB), "1x6 2x12 1x24");
}

#[test]
fn insert_named_cells_and_cells_in_all_columns() {
    let h = fixture();
    let r = ok(&h, "cell.range.add", json!({ "elementIds": [BOB], "frame": 6, "count": 2, "named": true }));
    assert_eq!((r["from"].as_u64(), r["to"].as_u64()), (Some(7), Some(8)));
    assert_eq!(r["drawingIds"].as_array().unwrap().len(), 2);
    // After the highest numeric name `10`: `11`, `12` (04 §14.2).
    assert_eq!(runs(&h, BOB), "1x6 11x1 12x1 .x2 2x12 1x24");
    let e = err(&h, "cell.range.add", json!({ "allElements": true, "named": true, "frame": 1 }));
    assert_eq!(e.code, "arg.invalid");
    // In all columns: Bob and Background shift; the sound clip after the insertion point moves too; pegs do not.
    let h = fixture();
    let peg_before = get(&h, "scene:\"Intro\"/element:\"Bob-P\"");
    ok(&h, "cell.range.add", json!({ "allElements": true, "frame": 1, "count": 3, "position": "before", "elementIds": [BOB] }));
    assert_eq!(runs(&h, BOB), ".x3 1x6 .x2 2x12 1x24");
    assert_eq!(runs(&h, BG).split(' ').next(), Some(".x3"));
    let sound = get(&h, "scene:\"Intro\"/element:\"Dialogue\"");
    let clip = get(&h, sound["placements"][0]["id"].as_str().unwrap());
    assert_eq!(clip["startFrame"], 4);
    assert_eq!(clip["stopFrame"], 11);
    assert_eq!(clip["mouthTrack"]["producedFor"]["stopFrame"], 11, "the mouths move with their clip");
    assert_eq!(get(&h, "scene:\"Intro\"/element:\"Bob-P\"")["start"], peg_before["start"]);
}

#[test]
fn delete_range_clears_or_shifts() {
    let h = fixture();
    ok(&h, "cell.range.delete", json!({ "elementIds": [BOB], "from": 1, "to": 3 }));
    assert_eq!(runs(&h, BOB), ".x3 1x3 .x2 2x12 1x24");
    let h = fixture();
    ok(&h, "cell.range.delete", json!({ "elementIds": [BOB], "from": 1, "to": 3, "shift": true }));
    assert_eq!(runs(&h, BOB), "1x3 .x2 2x12 1x24");
}

#[test]
fn cycle_and_columnate() {
    let h = fixture();
    let r = ok(&h, "cell.cycle.create", json!({ "elementId": BOB, "from": 1, "to": 8, "cycles": 3 }));
    assert_eq!((r["from"].as_u64(), r["to"].as_u64()), (Some(1), Some(24)));
    assert_eq!(runs(&h, BOB), "1x6 .x2 1x6 .x2 1x6 .x2 2x12 1x24");
    let h = fixture();
    let r = ok(
        &h,
        "cell.columnate.apply",
        json!({ "elementId": BOB, "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "loopMode": "forwardBackward", "loops": 2 }),
    );
    assert_eq!(r["totalFrames"], 8);
    let seq: Vec<&str> = r["sequence"].as_array().unwrap().iter().map(|s| s["drawing"].as_str().unwrap()).collect();
    assert_eq!(seq, ["1", "2", "10", "2", "1", "2", "10", "2"], "bank order is natural name order");
    assert_eq!(cells(&h, BOB).split(' ').take(9).collect::<Vec<_>>().join(" "), "1 2 10 2 1 2 10 2 2");
    // Reversed when last precedes first; unknown names refused.
    let h = fixture();
    let r = ok(&h, "cell.columnate.apply", json!({ "elementId": BOB, "frame": 1, "firstDrawing": "10", "lastDrawing": "1", "placement": "insert" }));
    let seq: Vec<&str> = r["sequence"].as_array().unwrap().iter().map(|s| s["drawing"].as_str().unwrap()).collect();
    assert_eq!(seq, ["10", "2", "1"]);
    assert_eq!(runs(&h, BOB), "10x1 2x1 1x7 .x2 2x12 1x24", "insert shifts the column");
    let e = err(&h, "cell.columnate.apply", json!({ "elementId": BOB, "firstDrawing": "1", "lastDrawing": "99" }));
    assert_eq!(e.code, "cell.drawingNotFound");
    // dryRun computes and changes nothing.
    let before = runs(&h, BOB);
    let r = ok(&h, "cell.columnate.apply", json!({ "elementId": BOB, "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "loopMode": "continuous", "totalFrames": 10, "dryRun": true }));
    assert_eq!(r["totalFrames"], 10);
    assert_eq!(runs(&h, BOB), before);
}

#[test]
fn labels_by_name_id_and_step() {
    let h = fixture();
    ok(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 2, "drawing": "10" }));
    assert_eq!(runs(&h, BOB), "10x2 1x4 .x2 2x12 1x24");
    ok(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 7, "to": 8, "drawingId": D2 }));
    assert_eq!(runs(&h, BOB), "10x2 1x4 2x14 1x24");
    let r = ok(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 3, "to": 3, "step": 1 }));
    assert_eq!(r["name"], "2", "1 → 2 in natural order");
    let e = err(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 1, "drawing": "nope" }));
    assert_eq!(e.code, "cell.drawingNotFound");
    let r = ok(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 1, "drawing": "nope", "createMissing": true }));
    assert_eq!(r["name"], "nope");
    let e = err(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 1, "step": 99 }));
    assert_eq!(e.code, "cell.drawingNotFound");
    let e = err(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    assert_eq!(e.code, "arg.invalid", "exactly one of drawing, drawingId, step");
    // Image elements never create drawings (13.13).
    let e = err(&h, "cell.label.set", json!({ "elementIds": [BG], "from": 1, "to": 1, "drawing": "new", "createMissing": true }));
    assert_eq!(e.code, "cell.drawingNotFound");
    // A drawing of another bank is not found.
    let e = err(&h, "cell.label.set", json!({ "elementIds": [NOTES], "from": 1, "to": 1, "drawingId": D1 }));
    assert_eq!(e.code, "cell.drawingNotFound");
}

#[test]
fn duplicate_drawing_replaces_only_the_range() {
    let h = fixture();
    let r = ok(&h, "cell.drawing.duplicate", json!({ "elementId": BOB, "from": 3, "to": 4 }));
    assert_eq!(r["name"], "11");
    assert_eq!(runs(&h, BOB), "1x2 11x2 1x2 .x2 2x12 1x24");
    let copy = get(&h, r["drawingId"].as_str().unwrap());
    let orig = get(&h, D1);
    assert_eq!(copy["nodes"], orig["nodes"], "content and drawing-local IDs are kept (C24-T1 owner resolution)");
    let e = err(&h, "cell.drawing.duplicate", json!({ "elementId": BOB, "from": 7 }));
    assert_eq!(e.code, "cell.notApplicable");
    let e = err(&h, "cell.drawing.duplicate", json!({ "elementId": BG, "from": 1 }));
    assert_eq!(e.code, "cell.notApplicable", "13.20: no duplicate drawing on image columns");
}

#[test]
fn notes_set_update_remove() {
    let h = fixture();
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 9, "text": "ease" }));
    assert_eq!(get(&h, BOB)["cellNotes"]["9"], "ease");
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 3, "text": "" }));
    assert!(get(&h, BOB)["cellNotes"].get("3").is_none());
    let e = err(&h, "cell.note.set", json!({ "elementId": "scene:\"Intro\"/element:\"Bob-P\"", "frame": 1, "text": "x" }));
    assert_eq!(e.code, "cell.notApplicable");
}

#[test]
fn move_copy_link_and_type_rules() {
    let h = fixture();
    ok(&h, "cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 6, "targetFrame": 45 }));
    assert_eq!(runs(&h, BOB), ".x2 2x12 1x30", "moved out (shift) and in after the end");
    // Copy into the annotation column: drawings copied into its bank.
    let h = fixture();
    let r = ok(&h, "cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 6, "targetElementId": NOTES, "targetFrame": 1, "mode": "copy" }));
    assert_eq!(r["createdDrawingIds"].as_array().unwrap().len(), 1);
    assert_eq!(runs(&h, NOTES), "1x6");
    assert_eq!(runs(&h, BOB), "1x6 .x2 2x12 1x24", "copy leaves the source");
    let e = err(&h, "cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 6, "targetElementId": NOTES, "targetFrame": 1, "mode": "link" }));
    assert_eq!(e.code, "cell.linkNotPossible");
    let e = err(&h, "cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 6, "targetElementId": BG, "targetFrame": 1, "mode": "copy" }));
    assert_eq!(e.code, "cell.typeMismatch");
    // Overwriting cells that show a locked drawing is refused.
    let e = err(&h, "cell.range.move", json!({ "sourceElementIds": [BOB], "from": 1, "to": 2, "targetFrame": 10, "mode": "copy", "options": { "exposure": false } }));
    assert_eq!(e.code, "drawing.locked");
}

#[test]
fn clipboard_copy_cut_paste() {
    let h = fixture();
    let r = ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 8 }));
    assert_eq!((r["contentType"].as_str(), r["frames"].as_u64()), (Some("drawingCells"), Some(8)));
    let before = runs(&h, BOB);
    ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 45 }));
    assert_eq!(runs(&h, BOB), "1x6 .x2 2x12 1x30", "same bank: the same drawings, appended");
    undo_restores(&h, BOB, &before);
    let r = ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 45, "mode": "newDrawings" }));
    assert_eq!(r["createdDrawingIds"].as_array().unwrap().len(), 1);
    // Cut removes and shifts (cut-special default `exposure: true`), as one undo step.
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 6, "cut": true }));
    assert_eq!(runs(&h, BOB), ".x2 2x12 1x24");
    let e = err(&h, "cell.clipboard.apply", json!({ "elementId": BG, "frame": 1 }));
    assert_eq!(e.code, "cell.typeMismatch");
    let h = fixture();
    let e = err(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 1 }));
    assert_eq!(e.code, "cell.clipboardEmpty");
}

#[test]
fn explicit_targets_leave_the_selection_and_frame_alone() {
    let h = fixture();
    ok(&h, "app.frame.set", json!({ "frame": 5 }));
    ok(&h, "cell.select", json!({ "elementIds": [BOB], "from": 9, "to": 10 }));
    let before = ok(&h, "cell.selection.get", json!({}));
    ok(&h, "cell.exposure.add", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    assert_eq!(ok(&h, "cell.selection.get", json!({})), before);
    // Without a target the selection is used: frames 9-10, now the blank run's end and drawing 2's start.
    ok(&h, "cell.exposure.add", json!({}));
    assert_eq!(runs(&h, BOB), "1x7 .x3 2x13 1x24");
    assert_eq!(before["from"], 9);
    assert_eq!(before["currentFrame"], 5);
}

#[test]
fn no_target_and_availability() {
    let h = fixture();
    // The fixture's current element is the camera: not cell-based.
    let e = err(&h, "cell.exposure.set", json!({ "length": 2 }));
    assert!(e.code == "cell.notApplicable" || e.code == "cell.noTarget", "{e:?}");
    let e = err(&h, "cell.exposure.set", json!({ "elementIds": [], "length": 2 }));
    assert_eq!(e.code, "cell.noTarget");
    // 13.20: set exposure is not available on sound columns at all; blank cells are, from M8 (SI-088).
    let e = err(&h, "cell.exposure.set", json!({ "elementIds": ["scene:\"Intro\"/element:\"Dialogue\""], "length": 2 }));
    assert_eq!(e.code, "cell.notApplicable");
    assert!(e.details.get("owner").is_none());
    let e = err(&h, "cell.blank.add", json!({ "elementIds": ["scene:\"Intro\"/element:\"Dialogue\""], "frame": 1 }));
    assert_eq!(e.code, "cell.notApplicable");
    assert_eq!(e.details["owner"], "M8 C8.1");
    let e = err(&h, "cell.exposure.set", json!({ "elementIds": ["scene:\"Intro\"/element:\"Nobody\""], "length": 2 }));
    assert_eq!(e.code, "element.notFound");
}

#[test]
fn oversize_results_refuse_before_staging() {
    let h = fixture();
    let before = rev(&h);
    let e = err(&h, "cell.cycle.create", json!({ "elementId": BOB, "from": 1, "to": 9999, "cycles": 999 }));
    assert_eq!(e.code, "doc.limitExceeded");
    assert_eq!(rev(&h), before);
    // Adding 9999 frames is within the scene limit.
    ok(&h, "cell.exposure.add", json!({ "elementIds": [BOB], "from": 1, "to": 1, "frames": 9999 }));
}

#[test]
fn exposure_events_fire_once_per_element() {
    let h = fixture();
    let rx = h.subscribe();
    ok(&h, "cell.exposure.set", json!({ "elementIds": [BOB, NOTES, BG], "from": 1, "to": 6, "length": 1 }));
    let ev: Vec<Value> = rx
        .try_iter()
        .filter(|e| e.name == "cell.exposures.changed")
        .map(|e| e.payload)
        .collect();
    // Notes is empty (unchanged); Bob and the Background change.
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert!(ev.iter().all(|e| e["rev"].as_u64() == Some(rev(&h))));
    let _ = D10;
}
