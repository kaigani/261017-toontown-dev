//! Clipboard edits through the executor (01 §6.2–§6.3, §12.4; C12-R1-14): cut, copy, paste, delete, cut special,
//! paste special and paste as new element over the object kinds of the M1 model.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]

mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const BOB_P: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const BOB_1: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";

fn cell(element: &str, frame: u32) -> String {
    format!("{element}/cell:#{frame}")
}

/// The frames of an element's exposure, one drawing ID (or null) per frame.
fn frames(h: &ExecutorHandle, element: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for run in get(h, element)["exposure"].as_array().unwrap() {
        for _ in 0..run[1].as_u64().unwrap() {
            out.push(run[0].clone());
        }
    }
    out
}

fn names(h: &ExecutorHandle) -> Vec<String> {
    let r = ok(h, "doc.object.find", json!({ "type": "element", "limit": 1000 }));
    r["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| get(h, i["id"].as_str().unwrap())["name"].as_str().unwrap().to_owned())
        .collect()
}

fn clipboard(h: &ExecutorHandle) -> Value {
    ok(h, "edit.clipboard.get", json!({ "includeData": true }))
}

#[test]
fn copy_and_paste_elements_makes_renamed_clones_sharing_drawings() {
    let h = fixture();
    let bob = get(&h, BOB);
    let before = rev(&h);
    let r = ok(&h, "edit.copy", json!({ "context": "staging", "objectIds": [BOB] }));
    assert_eq!(r["clipboard"], json!({ "kind": "elements", "itemCount": 1 }));
    assert_eq!(rev(&h), before, "copy does not change the document");
    let r = ok(&h, "edit.paste", json!({ "context": "staging" }));
    let id = r["createdIds"][0].as_str().unwrap();
    let clone = get(&h, id);
    assert_eq!(clone["name"], "Bob-2");
    assert_eq!(clone["drawingBankId"], bob["drawingBankId"]);
    assert_eq!(clone["exposure"], bob["exposure"]);
    assert_ne!(clone["id"], bob["id"]);
    assert_eq!(rev(&h), before + 1);
    ok(&h, "edit.undo", json!({}));
    assert!(run(&h, "doc.object.get", json!({ "ref": id })).is_error());
    // The clipboard is unchanged by pasting: a second paste makes another clone.
    ok(&h, "edit.paste", json!({ "context": "staging" }));
    ok(&h, "edit.paste", json!({ "context": "staging" }));
    let n = names(&h);
    assert!(n.contains(&"Bob-2".to_owned()) && n.contains(&"Bob-3".to_owned()), "{n:?}");
}

#[test]
fn cut_takes_descendants_paste_restores_the_forest_and_delete_reattaches() {
    // 05 §15: cut/copy/paste of nodes act on the nodes with all their descendants; 04 §8.3: delete re-attaches the
    // children to the deleted element's parent.
    let h = fixture();
    let scene = get(&h, "scene:\"Intro\"");
    let peg = get(&h, BOB_P)["id"].clone();
    let count = names(&h).len();
    let r = ok(&h, "edit.cut", json!({ "context": "staging", "objectIds": [BOB_P] }));
    let taken = r["clipboard"]["itemCount"].as_u64().unwrap() as usize;
    assert!(taken >= 2, "the peg and its child Bob: {r}");
    assert!(run(&h, "doc.object.get", json!({ "ref": BOB_P })).is_error());
    assert!(run(&h, "doc.object.get", json!({ "ref": BOB })).is_error(), "descendants are cut too");
    assert_eq!(names(&h).len(), count - taken);
    let r = ok(&h, "edit.paste", json!({ "context": "staging" }));
    assert_eq!(r["createdIds"].as_array().unwrap().len(), taken);
    let pasted_peg = get(&h, BOB_P);
    assert_eq!(pasted_peg["elementType"], "peg");
    assert_ne!(pasted_peg["id"], peg, "a paste creates new objects");
    let bob = ok(&h, "doc.object.get", json!({ "ref": BOB, "include": ["derived"] }))["object"].clone();
    assert_eq!(bob["parentId"], pasted_peg["id"], "the copied forest keeps its parentage");
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, "scene:\"Intro\"")["tree"], scene["tree"], "undo restores the tree");
    // Delete: Bob stays and hangs where the peg was.
    ok(&h, "edit.delete", json!({ "context": "staging", "objectIds": [BOB_P] }));
    assert!(run(&h, "doc.object.get", json!({ "ref": BOB })).is_ok());
    assert_eq!(names(&h).len(), count - 1);
}

#[test]
fn delete_clears_cells_and_cut_special_removes_frames() {
    let h = fixture();
    let all = frames(&h, BOB);
    assert!(all.len() >= 3 && all[0].is_string(), "{all:?}");
    let r = ok(&h, "edit.delete", json!({ "context": "xsheet", "objectIds": [cell(BOB, 1)] }));
    assert_eq!(r["deletedIds"].as_array().unwrap().len(), 1);
    let now = frames(&h, BOB);
    assert_eq!(now.len(), all.len());
    assert_eq!(now[0], Value::Null, "Delete clears the cell (01 §6.2)");
    assert_eq!(now[1..], all[1..]);
    ok(&h, "edit.undo", json!({}));
    let r = ok(
        &h,
        "edit.cutSpecial",
        json!({ "context": "xsheet", "objectIds": [cell(BOB, 1)], "options": { "createDeleteExposure": true } }),
    );
    assert_eq!(r["clipboard"], json!({ "kind": "cells", "itemCount": 1 }));
    let now = frames(&h, BOB);
    assert_eq!(now.len(), all.len() - 1, "create/delete exposure removes the frame");
    assert_eq!(now[..], all[1..]);
    let clip = clipboard(&h);
    assert_eq!(clip["items"][0]["drawingRef"], all[0]);
    assert_eq!(clip["options"]["createDeleteExposure"], true);
    // Without create/delete exposure, cut special blanks the cell.
    ok(&h, "edit.undo", json!({}));
    ok(
        &h,
        "edit.cutSpecial",
        json!({ "context": "xsheet", "objectIds": [cell(BOB, 1)], "options": { "createDeleteExposure": false } }),
    );
    assert_eq!(frames(&h, BOB).len(), all.len());
}

#[test]
fn paste_and_paste_special_cells() {
    let h = fixture();
    let all = frames(&h, BOB);
    ok(&h, "edit.copy", json!({ "context": "xsheet", "objectIds": [cell(BOB, 1), cell(BOB, 2)] }));
    let r = ok(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 60 } }));
    assert_eq!(r["modifiedIds"][0], get(&h, BOB)["id"]);
    let now = frames(&h, BOB);
    assert_eq!(now.len(), 61);
    assert_eq!(now[59], all[0]);
    assert_eq!(now[60], all[1]);
    assert!(now[all.len()..59].iter().all(Value::is_null), "paste beyond the end leaves blank frames");
    ok(&h, "edit.undo", json!({}));
    // Paste special with create/delete exposure inserts.
    let r = ok(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 1 }, "options": { "createDeleteExposure": true } }),
    );
    assert_eq!(r["warnings"], json!([]));
    let now = frames(&h, BOB);
    assert_eq!(now.len(), all.len() + 2);
    assert_eq!(now[2..], all[..]);
    // Paste special again repeats with the remembered options.
    ok(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 1 }, "useLast": true }),
    );
    assert_eq!(frames(&h, BOB).len(), all.len() + 4);
    // Extend exposure fills the gap with the last drawing.
    ok(&h, "edit.undo", json!({}));
    ok(&h, "edit.undo", json!({}));
    ok(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 60 }, "options": { "extendExposures": true, "createDeleteExposure": false } }),
    );
    let now = frames(&h, BOB);
    let last = all.iter().rev().find(|f| f.is_string()).unwrap();
    assert!(now[all.len()..59].iter().all(|f| f == last || all.last().unwrap().is_null()), "{now:?}");
}

#[test]
fn paste_special_again_needs_an_earlier_paste_special() {
    let h = fixture();
    ok(&h, "edit.copy", json!({ "context": "xsheet", "objectIds": [cell(BOB, 1)] }));
    let e = err(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "xsheet", "destination": { "elementId": BOB }, "useLast": true }),
    );
    assert_eq!(e.code, "edit.noPreviousPasteSpecial");
}

#[test]
fn paste_as_new_element_copies_the_cells_into_a_new_bank() {
    let h = fixture();
    let all = frames(&h, BOB);
    ok(&h, "edit.copy", json!({ "context": "xsheet", "objectIds": [cell(BOB, 1), cell(BOB, 2)] }));
    let r = ok(&h, "edit.pasteAsNewElement", json!({ "frame": 5 }));
    let el = get(&h, r["elementId"].as_str().unwrap());
    assert_eq!(el["elementType"], "drawing");
    assert_eq!(el["name"], "Bob-2");
    assert_ne!(el["drawingBankId"], get(&h, BOB)["drawingBankId"]);
    let now = frames(&h, r["elementId"].as_str().unwrap());
    assert_eq!(now.len(), 6);
    assert!(now[..4].iter().all(Value::is_null));
    assert!(now[4].is_string() && now[4] != all[0], "a copy of the drawing, not the original");
    let copy = get(&h, now[4].as_str().unwrap());
    let original = get(&h, all[0].as_str().unwrap());
    assert_eq!(copy["name"], original["name"]);
    ok(&h, "edit.undo", json!({}));
    assert!(run(&h, "doc.object.get", json!({ "ref": r["elementId"] })).is_error());
}

#[test]
fn drawing_objects_cut_copy_paste_and_new_element() {
    let h = fixture();
    let body = format!("{BOB_1}/drawingNode:\"Body\"");
    let strokes = get(&h, &body)["strokes"].as_array().unwrap().len();
    let stroke = ok(&h, "doc.object.find", json!({ "type": "stroke", "limit": 1 }))["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = ok(&h, "edit.copy", json!({ "context": "drawing", "objectIds": [stroke] }));
    assert_eq!(r["clipboard"], json!({ "kind": "drawingObjects", "itemCount": 1 }));
    assert_eq!(clipboard(&h)["items"][0]["type"], "stroke", "full vector content (01 §12.4)");
    let r = ok(&h, "edit.paste", json!({ "context": "drawing", "destination": { "drawingId": BOB_1 } }));
    assert_eq!(r["createdIds"].as_array().unwrap().len(), 1);
    assert_eq!(get(&h, &body)["strokes"].as_array().unwrap().len(), strokes + 1);
    // Cutting the layer cuts its strokes with it.
    ok(&h, "edit.cut", json!({ "context": "drawing", "objectIds": [body.clone(), stroke.clone()] }));
    assert!(run(&h, "doc.object.get", json!({ "ref": body })).is_error());
    assert_eq!(clipboard(&h)["itemCount"], 1);
    let r = ok(&h, "edit.pasteAsNewElement", json!({ "frame": 1 }));
    let el = get(&h, r["elementId"].as_str().unwrap());
    assert_eq!(el["elementType"], "drawing");
    let d = frames(&h, r["elementId"].as_str().unwrap())[0].as_str().unwrap().to_owned();
    let drawing = get(&h, &d);
    assert_eq!(drawing["nodes"].as_array().unwrap().len(), 1);
}

#[test]
fn locked_drawings_refuse_cut_and_paste() {
    let h = fixture();
    let locked = "scene:\"Intro\"/element:\"Bob\"/drawing:\"2\"";
    let node = get(&h, locked)["nodes"][0]["id"].as_str().unwrap().to_owned();
    let key = format!("{}/{node}", get(&h, locked)["id"].as_str().unwrap());
    let before = rev(&h);
    assert_eq!(err(&h, "edit.delete", json!({ "context": "drawing", "objectIds": [key] })).code, "drawing.locked");
    let stroke = ok(&h, "doc.object.find", json!({ "type": "stroke", "limit": 1 }))["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "edit.copy", json!({ "context": "drawing", "objectIds": [stroke] }));
    assert_eq!(
        err(&h, "edit.paste", json!({ "context": "drawing", "destination": { "drawingId": locked } })).code,
        "drawing.locked"
    );
    assert_eq!(rev(&h), before);
}

#[test]
fn keys_paste_at_a_frame_and_cut_special_filters_key_groups() {
    let h = fixture();
    let rotation = format!("{BOB}/channel:\"rotation\"");
    let scale = format!("{BOB}/channel:\"scale.x\"");
    let k12 = format!("{rotation}/key:\"12\"");
    let k24 = format!("{rotation}/key:\"24\"");
    let r = ok(&h, "edit.copy", json!({ "context": "functionEditor", "objectIds": [k12, k24] }));
    assert_eq!(r["clipboard"], json!({ "kind": "keyframes", "itemCount": 2 }));
    let r = ok(
        &h,
        "edit.paste",
        json!({ "context": "functionEditor", "destination": { "functionId": scale, "frame": 30 } }),
    );
    assert_eq!(r["createdIds"].as_array().unwrap().len(), 2);
    let keys = get(&h, &scale)["keys"].clone();
    let frames: Vec<u64> = keys
        .as_array()
        .unwrap()
        .iter()
        .map(|k| get(&h, &format!("{scale}/key:@{}", k["id"].as_str().unwrap()))["frame"].as_u64().unwrap())
        .collect();
    assert_eq!(frames, vec![1, 30, 42], "keys keep their spacing from the paste frame");
    // Cut special with rotation keys off leaves rotation keys in place.
    let scale_key = format!("{scale}/key:\"1\"");
    ok(
        &h,
        "edit.cutSpecial",
        json!({ "context": "functionEditor", "objectIds": [k12, scale_key], "options": { "rotationKeys": false } }),
    );
    assert!(run(&h, "doc.object.get", json!({ "ref": k12 })).is_ok());
    assert!(run(&h, "doc.object.get", json!({ "ref": scale_key })).is_error());
    assert_eq!(clipboard(&h)["itemCount"], 1);
}

#[test]
fn motion_point_paste_special_pastes_selected_values() {
    let h = fixture();
    let points = ok(&h, "doc.object.find", json!({ "type": "motionPoint", "limit": 3 }))["items"].clone();
    let first = points[0]["id"].as_str().unwrap().to_owned();
    let last = points[2]["id"].as_str().unwrap().to_owned();
    let (x0, x2) = (get(&h, &first)["position"]["x"].clone(), get(&h, &last)["position"]["x"].clone());
    assert_ne!(x0, x2);
    let r = ok(&h, "edit.copy", json!({ "context": "functionEditor", "objectIds": [first] }));
    assert_eq!(r["clipboard"]["kind"], "motionPoint");
    let r = ok(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "functionEditor", "destination": { "objectIds": [last] }, "options": { "x": true, "y": false } }),
    );
    assert_eq!(get(&h, &last)["position"]["x"], x0);
    assert_eq!(r["modifiedIds"].as_array().unwrap().len(), 1);
    assert_eq!(r["warnings"][0]["code"], "edit.optionNotApplied", "inapplicable options are listed (01 §12.4)");
}

#[test]
fn templates_copy_between_catalogs() {
    let h = fixture();
    let walk = "library:local/libraryFolder:\"Props\"/libraryItem:\"Walk\"";
    let r = ok(&h, "edit.copy", json!({ "context": "library", "objectIds": [walk] }));
    assert_eq!(r["clipboard"], json!({ "kind": "templates", "itemCount": 1 }));
    let r = ok(&h, "edit.paste", json!({ "context": "library", "destination": { "catalogId": "library:local" } }));
    let id = r["createdIds"][0].as_str().unwrap();
    let item = get(&h, id);
    assert_eq!(item["name"], "Walk");
    assert_eq!(item["path"], "library:local/libraryItem:\"Walk\"");
    // Pasting into the same folder again gives a unique name.
    ok(
        &h,
        "edit.paste",
        json!({ "context": "library", "destination": { "catalogId": "library:local/libraryFolder:\"Props\"" } }),
    );
    let folder = get(&h, "library:local/libraryFolder:\"Props\"");
    assert_eq!(folder["children"].as_array().unwrap().len(), 2);
    assert!(
        run(&h, "doc.object.get", json!({ "ref": "library:local/libraryFolder:\"Props\"/libraryItem:\"Walk-2\"" }))
            .is_ok()
    );
}

#[test]
fn element_properties_paste_onto_elements() {
    let h = fixture();
    ok(
        &h,
        "edit.clipboard.set",
        json!({ "content": { "kind": "elementProperties", "itemCount": 1, "items": [{ "columnWidth": 120 }] } }),
    );
    let r = ok(&h, "edit.paste", json!({ "context": "staging", "destination": { "objectIds": [BOB] } }));
    assert_eq!(r["modifiedIds"].as_array().unwrap().len(), 1);
    assert_eq!(get(&h, BOB)["columnWidth"], 120);
    let e = err(
        &h,
        "edit.clipboard.set",
        json!({ "content": { "kind": "elementProperties", "itemCount": 1, "items": [{ "noSuchProperty": 1 }] } }),
    );
    assert_eq!(e.code, "edit.clipboardInvalid");
}

#[test]
fn selection_is_the_default_target() {
    let h = fixture();
    let e = err(&h, "edit.copy", json!({ "context": "staging" }));
    assert_eq!(e.code, "command.disabled", "nothing selected");
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [BOB] }));
    let r = ok(&h, "edit.copy", json!({ "context": "staging" }));
    assert_eq!(r["clipboard"]["itemCount"], 1);
    ok(&h, "edit.selection.set", json!({ "context": "xsheet", "ids": [cell(BOB, 1), cell(BOB, 2)] }));
    let r = ok(&h, "edit.copy", json!({ "context": "xsheet" }));
    assert_eq!(r["clipboard"], json!({ "kind": "cells", "itemCount": 2 }));
}

#[test]
fn paste_errors() {
    let h = fixture();
    assert_eq!(err(&h, "edit.paste", json!({ "context": "staging" })).code, "edit.clipboardEmpty");
    ok(&h, "edit.copy", json!({ "context": "xsheet", "objectIds": [cell(BOB, 1)] }));
    assert_eq!(err(&h, "edit.paste", json!({ "context": "library" })).code, "edit.pasteNotAllowed");
    // A peg has no exposure: pasting cells into it is the wrong element type (01 §6.2).
    assert_eq!(
        err(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": BOB_P, "frame": 1 } })).code,
        "edit.wrongElementType"
    );
    ok(
        &h,
        "edit.clipboard.set",
        json!({ "content": { "kind": "image", "itemCount": 1, "items": [{ "mediaType": "image/png", "width": 1, "height": 1, "base64": "" }] } }),
    );
    assert_eq!(err(&h, "edit.paste", json!({ "context": "drawing" })).code, "command.notImplemented");
}

#[test]
fn dry_run_cut_changes_nothing() {
    let h = fixture();
    let before = rev(&h);
    let env = with(
        &h,
        "edit.cut",
        json!({ "context": "staging", "objectIds": [BOB] }),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert!(env.is_ok(), "{env:?}");
    assert!(env.dry_run);
    assert_eq!(rev(&h), before);
    assert_eq!(ok(&h, "edit.clipboard.get", json!({})), Value::Null);
    assert!(run(&h, "doc.object.get", json!({ "ref": BOB })).is_ok());
}

#[test]
fn cut_and_paste_in_one_atomic_batch_is_one_undo_step() {
    let h = fixture();
    let original = get(&h, BOB)["id"].clone();
    let before = rev(&h);
    let env = batch(
        &h,
        json!({ "label": "Move Bob", "steps": [
            { "id": "edit.cut", "params": { "context": "staging", "objectIds": [BOB] } },
            { "id": "edit.paste", "params": { "context": "staging" } }
        ] }),
    );
    assert!(env.is_ok(), "{env:?}");
    assert_eq!(rev(&h), before + 1);
    assert!(names(&h).contains(&"Bob".to_owned()));
    assert_ne!(get(&h, BOB)["id"], original, "the pasted element is a new object");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(rev(&h), before + 2);
    assert_eq!(get(&h, BOB)["id"], original, "one undo step restores the original");
}
