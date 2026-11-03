//! M3 C3.1 round 1 (builder): the coverage each round-1 ruling asks for beyond the reviewer's reproductions —
//! session rollback (C31-R1-02), AG-02 equivalence (-03), self-contained clipboards (-04, -13), notes (-05), subtree
//! duplication (-06), overlapping transfers (-07), name planning (-08), target limits (-09), every-column commands
//! (-15), lock availability (-16) and charged selection (-14).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions, Status};

const INTRO: &str = "scene:\"Intro\"";
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const NOTES: &str = "scene:\"Intro\"/element:\"Notes\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const D1: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";

fn id(h: &ExecutorHandle, r: &str) -> String {
    get(h, r)["id"].as_str().unwrap().to_owned()
}

fn labels(h: &ExecutorHandle, element: &str) -> String {
    let e = get(h, element);
    let mut out = Vec::new();
    for r in e["exposure"].as_array().cloned().unwrap_or_default() {
        let label = r[0].as_str().map_or(".".to_owned(), |d| get(h, d)["name"].as_str().unwrap().to_owned());
        for _ in 0..r[1].as_u64().unwrap() {
            out.push(label.clone());
        }
    }
    out.join(" ")
}

fn content(h: &ExecutorHandle, d: &str) -> Value {
    ok(h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"].clone()
}

fn other_project(h: &ExecutorHandle, seed: u64) {
    let doc = tt_model::fixture::sample_document(seed).unwrap();
    let e = h.open_document_with(
        doc,
        tt_executor::OpenOptions { on_unsaved: tt_executor::UnsavedPolicy::Discard, ..Default::default() },
        tt_executor::Origin::User,
    );
    assert!(e.is_ok(), "{e:?}");
}

// ----- C31-R1-02 ------------------------------------------------------------------------------------------------------

#[test]
fn r02_timeline_state_follows_dry_runs_and_failed_groups() {
    let h = fixture();
    let solo_before = ok(&h, "app.state.get", json!({}));
    for (cmd, params) in [
        ("element.solo.set", json!({ "enabled": true, "elementIds": [BOB] })),
        ("element.typeColour.set", json!({ "category": "drawings", "colour": "#010203" })),
    ] {
        let env = with(&h, cmd, params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
        assert_eq!(env.status, Status::Ok, "{cmd}");
        let failed = batch(
            &h,
            json!({ "label": "rollback", "atomic": true, "steps": [
                { "id": cmd, "params": params },
                { "id": "element.rename", "params": { "elementId": BOB, "name": "bad.name" } }
            ] }),
        );
        assert_ne!(failed.status, Status::Ok);
    }
    assert_eq!(ok(&h, "element.typeColour.get", json!({}))["colours"]["drawings"], "#9DC3E6FF");
    assert_eq!(ok(&h, "app.state.get", json!({})), solo_before);
    // The real calls apply.
    ok(&h, "element.typeColour.set", json!({ "category": "drawings", "colour": "#010203" }));
    assert_eq!(ok(&h, "element.typeColour.get", json!({}))["colours"]["drawings"], "#010203FF");
}

// ----- C31-R1-03 / -16 ------------------------------------------------------------------------------------------------

#[test]
fn r03_patch_ops_and_props_match_the_owning_commands() {
    for (path, value, event) in [
        ("/name", json!("Robert"), "element.state.changed"),
        ("/note", json!("n"), "element.state.changed"),
        ("/markerColour", json!("#aabbcc"), "element.state.changed"),
        ("/visibleInScene", json!(false), "element.state.changed"),
        ("/locked", json!(true), "element.state.changed"),
    ] {
        let h = fixture();
        let rx = h.subscribe();
        ok(
            &h,
            "doc.object.patch",
            json!({ "targets": [BOB], "ops": [{ "op": "replace", "path": path, "value": value }] }),
        );
        let n = rx.try_iter().filter(|e| e.name == event).count();
        assert_eq!(n, 1, "{path}: exactly one owning-command event");
    }
    // Colour through ops is normalised like the command.
    let h = fixture();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "replace", "path": "/markerColour", "value": "#aabbcc" }] }),
    );
    assert_eq!(get(&h, BOB)["markerColour"], "#AABBCCFF");
    // Removing a note through ops clears it.
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "ops": [{ "op": "remove", "path": "/note" }] }));
    assert!(get(&h, BOB).get("note").is_none());
    // Drawing rename through ops and props: the command's codes and the bank event.
    let rx = h.subscribe();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [D1], "ops": [{ "op": "replace", "path": "/name", "value": "one" }] }),
    );
    assert_eq!(rx.try_iter().filter(|e| e.name == "element.drawing.changed").count(), 1);
    let one = "scene:\"Intro\"/element:\"Bob\"/drawing:\"one\"";
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [one], "props": { "name": "10" } })).code,
        "drawing.nameDuplicate"
    );
    // Properties whose commands are later checkpoints still name their owner.
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "replace", "path": "/transform/rotation", "value": 5 }] }),
    );
    assert_eq!(
        (e.code.as_str(), e.details["commandId"].as_str()),
        ("command.notImplemented", Some("anim.transform.set"))
    );
    // A content edit of a locked drawing is still refused; unlocking is the lock command's.
    let d2 = "scene:\"Intro\"/element:\"Bob\"/drawing:\"2\"";
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [d2], "props": { "pivot": { "x": 1, "y": 1 } } })).code,
        "drawing.locked"
    );
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [d2], "ops": [{ "op": "replace", "path": "/locked", "value": false }] }),
    );
    // 04 §13.20 through a delegated write: image drawings cannot be locked.
    let img = get(&h, BG)["exposure"][0][0].as_str().unwrap().to_owned();
    let r = rev(&h);
    assert_eq!(
        err(&h, "doc.object.patch", json!({ "targets": [img], "props": { "locked": true } })).code,
        "cell.notApplicable"
    );
    assert_eq!(rev(&h), r);
}

#[test]
fn r03_position_events_come_once_from_any_cause() {
    let h = fixture();
    ok(&h, "element.current.set", json!({ "elementId": BOB }));
    for (cmd, params, expected) in [
        ("element.current.set", json!({ "elementId": BOB }), 0),
        ("element.current.set", json!({ "elementId": BG }), 1),
        ("cell.select", json!({ "elementIds": [BOB], "from": 1, "to": 3 }), 1),
        ("cell.select", json!({ "elementIds": [BOB], "from": 1, "to": 3 }), 0),
        ("app.frame.set", json!({ "frame": 4 }), 1),
        ("element.select", json!({ "elementIds": [PEG] }), 1),
        ("element.delete", json!({ "elementIds": [BG] }), 1),
    ] {
        let rx = h.subscribe();
        ok(&h, cmd, params);
        let n = rx.try_iter().filter(|e| e.name == "cell.selection.changed").count();
        assert_eq!(n, expected, "{cmd}");
    }
}

// ----- C31-R1-04 / -13 -------------------------------------------------------------------------------------------------

#[test]
fn r04_copies_are_self_contained_across_projects_and_deletion() {
    // Generic edit.copy / edit.paste of cells into another project: the artwork comes from the copy.
    let h = fixture();
    let src_content = content(&h, D1);
    ok(&h, "edit.copy", json!({ "context": "xsheet", "objectIds": ["scene:\"Intro\"/element:\"Bob\"/cell:#1"] }));
    other_project(&h, 4242);
    let notes_id = id(&h, NOTES);
    ok(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": notes_id, "frame": 1 } }));
    let pasted = get(&h, NOTES)["exposure"][0][0].as_str().unwrap().to_owned();
    assert_eq!(content(&h, &pasted), src_content, "drawing-local IDs, fills and owner records verbatim");
    // Whole elements: banks travel with them.
    let h = fixture();
    let bob_drawings =
        ok(&h, "element.drawing.list", json!({ "elementId": BOB }))["drawings"].as_array().unwrap().len();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "wholeElements": true }));
    other_project(&h, 4343);
    let r = ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 1 }));
    let copy = r["elementIds"][0].as_str().unwrap().to_owned();
    let list = ok(&h, "element.drawing.list", json!({ "elementId": copy }))["drawings"].clone();
    assert_eq!(list.as_array().unwrap().len(), bob_drawings);
    assert_ne!(get(&h, &copy)["drawingBankId"], get(&h, BOB)["drawingBankId"]);
    // Image cells into another project's image column.
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BG], "from": 1, "to": 2 }));
    other_project(&h, 4444);
    ok(&h, "cell.clipboard.apply", json!({ "elementId": BG, "frame": 49 }));
    // Deleting the source after copying, then pasting as new drawings.
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 9, "to": 10 }));
    ok(&h, "element.delete", json!({ "elementIds": [BOB] }));
    let r = ok(&h, "cell.clipboard.apply", json!({ "elementId": NOTES, "frame": 1, "mode": "newDrawings" }));
    assert_eq!(r["createdDrawingIds"].as_array().unwrap().len(), 1);
}

#[test]
fn r13_both_bridges_overwrite_the_full_drawing() {
    for generic in [true, false] {
        let h = fixture();
        ok(
            &h,
            "doc.object.patch",
            json!({ "targets": [D1], "props": { "ext": { "review.example": { "opaque": [1] } }, "meta": { "k": "v" } } }),
        );
        let target =
            ok(&h, "element.drawing.create", json!({ "elementId": NOTES, "name": "1", "exposeFrom": 1 }))["drawingId"]
                .as_str()
                .unwrap()
                .to_owned();
        let target_rev = get(&h, &target)["createdAt"].clone();
        ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
        if generic {
            ok(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": NOTES, "frame": 1 } }));
        } else {
            ok(
                &h,
                "cell.clipboard.apply",
                json!({ "elementId": NOTES, "frame": 1, "options": { "overwriteDrawings": true, "exposure": false } }),
            );
        }
        let t = get(&h, &target);
        let s = get(&h, D1);
        assert_eq!((t["ext"].clone(), t["meta"].clone()), (s["ext"].clone(), s["meta"].clone()), "generic={generic}");
        assert_eq!(t["colourRecovery"], s["colourRecovery"]);
        assert_eq!(t["createdAt"], target_rev, "the destination keeps its identity");
        assert_eq!(content(&h, &target), content(&h, D1));
    }
}

// ----- C31-R1-05 ------------------------------------------------------------------------------------------------------

#[test]
fn r05_note_overwrite_and_clear_rules() {
    let h = fixture();
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 1, "text": "copied" }));
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 21, "text": "replaced" }));
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 2 }));
    // Overwrite paste: the copied note replaces the destination's, frame 22 (no copied note) is cleared.
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 22, "text": "gone" }));
    ok(&h, "cell.clipboard.apply", json!({ "elementId": BOB, "frame": 21, "options": { "exposure": false } }));
    let notes = get(&h, BOB)["cellNotes"].clone();
    assert_eq!((notes["21"].as_str(), notes.get("22")), (Some("copied"), None));
    // Clearing cells clears their notes; overwriting edits (fill, label, sheet writes) keep notes in place.
    ok(&h, "cell.range.delete", json!({ "elementIds": [BOB], "from": 1, "to": 1 }));
    assert!(get(&h, BOB)["cellNotes"].get("1").is_none());
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 30, "text": "stays" }));
    ok(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 21, "toFrame": 40 }));
    ok(&h, "cell.label.set", json!({ "elementIds": [BOB], "from": 30, "to": 30, "drawing": "10" }));
    ok(&h, "xsheet.exposures.set", json!({ "columns": [{ "elementId": BOB, "from": 25, "to": 35, "exposures": [] }] }));
    assert_eq!(get(&h, BOB)["cellNotes"]["30"], "stays");
    // Moving a note's cell carries the note; the edit is one undo step.
    let before = get(&h, BOB)["cellNotes"].clone();
    ok(
        &h,
        "cell.range.move",
        json!({ "sourceElementIds": [BOB], "from": 30, "to": 30, "targetFrame": 50, "mode": "move" }),
    );
    assert_eq!(get(&h, BOB)["cellNotes"]["49"], "stays");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, BOB)["cellNotes"], before);
}

// ----- C31-R1-06 ------------------------------------------------------------------------------------------------------

#[test]
fn r06_duplicate_copies_whole_subtrees() {
    let h = fixture();
    for (name, rows) in [("Rig", 3), ("Snow", 1), ("Clip", 1)] {
        let r = format!("scene:\"Intro\"/element:\"{name}\"");
        let orig = ok(&h, "element.list", json!({ "parentId": r }))["elements"].as_array().unwrap().len();
        let copy = ok(&h, "element.duplicate", json!({ "elementIds": [r] }))["elementIds"][0].clone();
        let copied = ok(&h, "element.list", json!({ "parentId": copy }))["elements"].clone();
        assert_eq!(copied.as_array().unwrap().len(), orig, "{name}: every row of the subtree");
        assert!(orig >= rows);
        let orig_ids: Vec<Value> = ok(&h, "element.list", json!({ "parentId": r }))["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].clone())
            .collect();
        assert!(copied.as_array().unwrap().iter().all(|e| !orig_ids.contains(&e["id"])), "{name}: new IDs");
    }
    // Ordinary children are duplicated with their parent (SI-094); the copy of a child is not its original.
    let copy = ok(&h, "element.duplicate", json!({ "elementIds": [PEG] }))["elementIds"][0].clone();
    let kids = ok(&h, "element.list", json!({ "parentId": copy, "recursive": false }))["elements"].clone();
    assert_eq!(kids.as_array().unwrap().len(), 1);
    assert_ne!(kids[0]["id"], json!(id(&h, BOB)));
    // A parameter row alone is refused; given with its effect it is copied once.
    assert_eq!(
        err(&h, "element.duplicate", json!({ "elementIds": ["scene:\"Intro\"/element:\"Clip-mask\""] })).code,
        "arg.invalid"
    );
    let r = ok(
        &h,
        "element.duplicate",
        json!({ "elementIds": ["scene:\"Intro\"/element:\"Clip\"", "scene:\"Intro\"/element:\"Clip-mask\""] }),
    );
    assert_eq!(r["elementIds"].as_array().unwrap().len(), 1);
}

// ----- C31-R1-07 ------------------------------------------------------------------------------------------------------

#[test]
fn r07_overlapping_transfers_read_the_original_cells() {
    for (mode, backward) in [("copy", false), ("copy", true), ("copyNew", false), ("move", false), ("link", false)] {
        let h = fixture();
        let made = ok(&h, "element.create", json!({ "type": "drawing", "count": 3, "name": "Col", "sceneId": INTRO }))
            ["elementIds"]
            .as_array()
            .unwrap()
            .clone();
        // Link needs one bank: use clones of the first column.
        let cols: Vec<Value> = if mode == "link" {
            let a = made[0].clone();
            let b = ok(&h, "element.clone", json!({ "elementId": a }))["elementId"].clone();
            let c = ok(&h, "element.clone", json!({ "elementId": a }))["elementId"].clone();
            vec![a, b, c]
        } else {
            made
        };
        for (i, e) in cols.iter().enumerate() {
            ok(
                &h,
                "cell.label.set",
                json!({ "elementIds": [e], "from": 1, "to": 1, "drawing": format!("D{i}"), "createMissing": true }),
            );
        }
        let order: Vec<String> = ok(&h, "element.list", json!({ "sceneId": INTRO, "order": "xsheet" }))["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect();
        let pos = |e: &Value| order.iter().position(|o| o == e.as_str().unwrap()).unwrap();
        let mut sorted = cols.clone();
        sorted.sort_by_key(|e| pos(e));
        let names_before: Vec<String> = sorted.iter().map(|e| labels(&h, e.as_str().unwrap())).collect();
        let (sources, target) = if backward {
            (vec![sorted[1].clone(), sorted[2].clone()], sorted[0].clone())
        } else {
            (vec![sorted[0].clone(), sorted[1].clone()], sorted[1].clone())
        };
        ok(
            &h,
            "cell.range.move",
            json!({ "sourceElementIds": sources, "from": 1, "to": 1, "targetElementId": target, "targetFrame": 1, "mode": mode, "options": { "exposure": false } }),
        );
        let after: Vec<String> = sorted.iter().map(|e| labels(&h, e.as_str().unwrap())).collect();
        let (first, second) = if backward { (0, 1) } else { (1, 2) };
        let expect_first = names_before[if backward { 1 } else { 0 }].clone();
        let expect_second = names_before[if backward { 2 } else { 1 }].clone();
        if mode == "copyNew" {
            assert_ne!(after[first], "", "{mode}");
        } else {
            assert_eq!(after[first], expect_first, "{mode} backward={backward}: {after:?}");
            assert_eq!(after[second], expect_second, "{mode} backward={backward}: {after:?}");
        }
    }
}

// ----- C31-R1-08 ------------------------------------------------------------------------------------------------------

#[test]
fn r08_names_normalise_and_fit() {
    // NFC-equivalent names in reverse column order create one drawing.
    let h = fixture();
    let clone = ok(&h, "element.clone", json!({ "elementId": BOB }))["elementId"].clone();
    let r = ok(
        &h,
        "xsheet.exposures.set",
        json!({ "createMissingDrawings": true, "columns": [
            { "elementId": clone, "from": 1, "to": 1, "exposures": [{ "drawing": "e\u{301}t\u{e9}", "from": 1, "to": 1 }] },
            { "elementId": BOB, "from": 1, "to": 1, "exposures": [{ "drawing": " \u{e9}t\u{e9} ", "from": 1, "to": 1 }] }
        ] }),
    );
    assert_eq!(r["createdDrawings"].as_array().unwrap().len(), 1);
    // Default copy names stay within 64 characters, after collisions too.
    let h = fixture();
    let long = "N".repeat(64);
    let src =
        ok(&h, "element.create", json!({ "type": "drawing", "name": long, "sceneId": INTRO }))["elementIds"][0].clone();
    ok(&h, "element.create", json!({ "type": "drawing", "name": format!("{}_1", "N".repeat(62)), "sceneId": INTRO }));
    let c = ok(&h, "element.clone", json!({ "elementId": src }))["name"].as_str().unwrap().to_owned();
    assert_eq!(c, format!("{}_2", "N".repeat(62)));
    let d = ok(&h, "element.duplicate", json!({ "elementIds": [src] }))["names"][0].as_str().unwrap().to_owned();
    assert_eq!(d, format!("{}_3", "N".repeat(62)));
    assert!(d.chars().count() <= 64);
}

// ----- C31-R1-09 / -15 ------------------------------------------------------------------------------------------------

#[test]
fn r09_target_limits_and_r15_every_selected_column() {
    let h = fixture();
    ok(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": 9999 }));
    assert_eq!(
        err(&h, "cell.fill.apply", json!({ "elementIds": [BOB], "frame": 1, "toFrame": 10_000 })).code,
        "cell.frameOutOfRange"
    );
    assert_eq!(
        err(&h, "element.childExposure.apply", json!({ "pegId": PEG, "toFrame": 10_000 })).code,
        "cell.frameOutOfRange"
    );
    // Columnate and drawing duplication over a two-column selection.
    let h = fixture();
    ok(
        &h,
        "cell.label.set",
        json!({ "elementIds": [NOTES], "from": 1, "to": 2, "drawing": "1", "createMissing": true }),
    );
    ok(
        &h,
        "cell.label.set",
        json!({ "elementIds": [NOTES], "from": 3, "to": 3, "drawing": "2", "createMissing": true }),
    );
    ok(&h, "cell.select", json!({ "elementIds": [BOB, NOTES], "from": 1, "to": 2 }));
    let r = ok(&h, "cell.columnate.apply", json!({ "firstDrawing": "1", "lastDrawing": "2", "placement": "insert" }));
    assert_eq!(r["totalFrames"], 2);
    assert!(labels(&h, NOTES).starts_with("1 2 1 1 2"), "{}", labels(&h, NOTES));
    assert!(labels(&h, BOB).starts_with("1 2 1"), "{}", labels(&h, BOB));
    ok(&h, "cell.select", json!({ "elementIds": [BOB, NOTES], "from": 1, "to": 1 }));
    let r = ok(&h, "cell.drawing.duplicate", json!({}));
    assert!(r["drawingId"].is_string());
    assert!(!labels(&h, BOB).starts_with("1 "), "{}", labels(&h, BOB));
    assert!(!labels(&h, NOTES).starts_with("1 "), "{}", labels(&h, NOTES));
    // Columnate total frames: a single pass is never extended (SI-095).
    assert_eq!(
        err(
            &h,
            "cell.columnate.apply",
            json!({ "elementId": BOB, "frame": 1, "firstDrawing": "1", "lastDrawing": "10", "totalFrames": 50 })
        )
        .code,
        "arg.invalid"
    );
}

// ----- C31-R1-14 ------------------------------------------------------------------------------------------------------

#[test]
fn r14_selection_is_set_based_and_ordered() {
    let h = fixture();
    ok(&h, "cell.select", json!({ "elementIds": [BOB, NOTES], "from": 1, "to": 3 }));
    let r = ok(&h, "cell.select", json!({ "elementIds": [BOB], "from": 2, "to": 5, "mode": "add" }));
    assert_eq!((r["from"].as_u64(), r["to"].as_u64()), (Some(1), Some(5)));
    let ids = ok(&h, "edit.selection.get", json!({ "context": "xsheet" }))["ids"].as_array().unwrap().len();
    assert_eq!(ids, 3 + 3 + 2, "duplicates are kept once, in selection order");
    // A whole 100 000-frame column is selected, and later calls stay fast.
    ok(&h, "cell.select", json!({ "elementIds": [BOB], "from": 1, "to": 100_000 }));
    let t = std::time::Instant::now();
    for _ in 0..20 {
        ok(&h, "app.frame.set", json!({ "frame": 2 }));
    }
    assert!(t.elapsed() < std::time::Duration::from_secs(10), "{:?}", t.elapsed());
    assert_eq!(
        err(&h, "cell.select", json!({ "elementIds": [BOB, NOTES, BG], "from": 1, "to": 400_000 })).code,
        "cell.frameOutOfRange"
    );
}
