//! M3 C3.1 round 2 (builder): the coverage each round-2 ruling asks for beyond the reviewer's reproductions — ordered
//! patch interpretation (C31-R2-01), delegated `copy`/`move` (-02), one column-type check for every cell paste
//! (-03), notes through generic cut/delete and the frame-domain refusal (-04), bounded derived names (-05) and
//! collision-safe binary entries (-06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use c13_support::{job_ok, tmp};
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions, Status};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const NOTES: &str = "scene:\"Intro\"/element:\"Notes\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
const D1: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";

fn id(h: &ExecutorHandle, r: &str) -> String {
    get(h, r)["id"].as_str().unwrap().to_owned()
}

fn cell_notes(h: &ExecutorHandle, e: &str) -> Value {
    get(h, e)["cellNotes"].clone()
}

fn clear_notes(h: &ExecutorHandle, e: &str) {
    let frames: Vec<u64> =
        cell_notes(h, e).as_object().map(|m| m.keys().filter_map(|k| k.parse().ok()).collect()).unwrap_or_default();
    for f in frames {
        ok(h, "cell.note.set", json!({ "elementId": e, "frame": f, "text": "" }));
    }
}

// ----- C31-R2-01 ------------------------------------------------------------------------------------------------------

#[test]
fn r01_props_then_ops_in_order_and_a_late_test_rolls_everything_back() {
    let h = fixture();
    ok(&h, "element.note.set", json!({ "elementId": BOB, "text": "old" }));
    // `props` come first: an `ops` test sees the delegated write the merge made.
    let env = run(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "props": { "note": "mid" }, "ops": [
            { "op": "test", "path": "/note", "value": "mid" },
            { "op": "add", "path": "/meta", "value": { "k": 1 } },
            { "op": "replace", "path": "/note", "value": "new" }
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert_eq!(get(&h, BOB)["note"], "new");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, BOB)["note"], "old", "one undo step");
    // A failing test after delegated and ordinary writes rolls back both, with no event.
    let before = get(&h, BOB);
    let revision = rev(&h);
    let rx = h.subscribe();
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [
            { "op": "replace", "path": "/note", "value": "x" },
            { "op": "replace", "path": "/markerColour", "value": "#FF0000" },
            { "op": "add", "path": "/meta/z", "value": 2 },
            { "op": "test", "path": "/note", "value": "not x" }
        ] }),
    );
    assert_eq!(e.code, "doc.patchTestFailed");
    assert_eq!(get(&h, BOB), before);
    assert_eq!(rev(&h), revision);
    assert!(!rx.try_iter().any(|e| e.name == "element.state.changed"), "a refused patch announces nothing");
}

#[test]
fn r01_test_between_delegated_writes_sees_the_staged_value_across_targets() {
    let h = fixture();
    let env = run(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB, NOTES], "ops": [
            { "op": "replace", "path": "/locked", "value": true },
            { "op": "test", "path": "/locked", "value": true },
            { "op": "replace", "path": "/locked", "value": false }
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert_eq!(get(&h, BOB)["locked"], false);
    assert_eq!(get(&h, NOTES)["locked"], false);
}

// ----- C31-R2-02 ------------------------------------------------------------------------------------------------------

#[test]
fn r02_move_and_copy_go_through_the_owning_handlers() {
    let h = fixture();
    // `move` from a delegated property: its removal is `element.note.set ""`, the destination an ordinary write.
    ok(&h, "element.note.set", json!({ "elementId": BOB, "text": "moved" }));
    let rx = h.subscribe();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "move", "from": "/note", "path": "/meta/n" }] }),
    );
    let bob = get(&h, BOB);
    assert!(bob.get("note").is_none_or(|n| n == "" || n.is_null()), "{bob}");
    assert_eq!(bob["meta"]["n"], "moved");
    assert_eq!(rx.try_iter().filter(|e| e.name == "element.state.changed").count(), 1);
    // `move` into a delegated property: the ordinary source is removed, the write is `element.rename`'s (refusing an
    // invalid name like the command, with nothing changed).
    ok(&h, "doc.object.patch", json!({ "targets": [BOB], "props": { "meta": { "name": "bad.name" } } }));
    let revision = rev(&h);
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "move", "from": "/meta/name", "path": "/name" }] }),
    );
    let direct = err(&h, "element.rename", json!({ "elementId": BOB, "name": "bad.name" }));
    assert_eq!(e.code, direct.code);
    assert_eq!(rev(&h), revision);
    assert_eq!(get(&h, BOB)["meta"]["name"], "bad.name", "the source stays when the write is refused");
    // `copy` onto a drawing lock: `element.drawingLock.set`, its event once.
    ok(&h, "doc.object.patch", json!({ "targets": [D1], "props": { "meta": { "lock": true } } }));
    let rx = h.subscribe();
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [D1], "ops": [{ "op": "copy", "from": "/meta/lock", "path": "/locked" }] }),
    );
    assert_eq!(get(&h, D1)["locked"], true);
    assert_eq!(rx.try_iter().filter(|e| e.name == "element.drawing.changed").count(), 1);
    // A copy between ordinary properties keeps the model's own semantics.
    ok(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "copy", "from": "/meta/n", "path": "/meta/m" }] }),
    );
    assert_eq!(get(&h, BOB)["meta"]["m"], "moved");
    // A missing source is a typed refusal.
    let e = err(
        &h,
        "doc.object.patch",
        json!({ "targets": [BOB], "ops": [{ "op": "copy", "from": "/meta/none", "path": "/note" }] }),
    );
    assert_eq!(e.code, "arg.invalid");
}

// ----- C31-R2-03 ------------------------------------------------------------------------------------------------------

#[test]
fn r03_every_cell_paste_checks_column_types_in_both_directions() {
    let h = fixture();
    let image = ok(&h, "element.create", json!({ "type": "image", "name": "Img" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    // Vector cells into an image column, through every entry point.
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 1, "to": 2 }));
    let revision = rev(&h);
    for (cmd, p) in [
        ("cell.clipboard.apply", json!({ "elementId": image, "frame": 1 })),
        ("edit.paste", json!({ "context": "xsheet", "destination": { "elementId": image, "frame": 1 } })),
        (
            "edit.pasteSpecial",
            json!({ "context": "xsheet", "destination": { "elementId": image, "frame": 1 }, "options": { "overwriteDrawings": false } }),
        ),
    ] {
        assert_eq!(err(&h, cmd, p).code, "cell.typeMismatch", "{cmd}");
        assert_eq!(rev(&h), revision, "{cmd}: nothing changed");
    }
    // Image cells into a vector column after the source column is deleted: the copy's recorded type decides.
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BG], "from": 1, "to": 1 }));
    ok(&h, "element.delete", json!({ "elementIds": [BG] }));
    let revision = rev(&h);
    let before = get(&h, BOB);
    for (cmd, p) in [
        ("cell.clipboard.apply", json!({ "elementId": BOB, "frame": 20 })),
        ("edit.paste", json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 20 } })),
        (
            "edit.pasteSpecial",
            json!({ "context": "xsheet", "destination": { "elementId": BOB, "frame": 20 }, "options": {} }),
        ),
    ] {
        assert_eq!(err(&h, cmd, p).code, "cell.typeMismatch", "{cmd}");
        assert_eq!(rev(&h), revision);
        assert_eq!(get(&h, BOB), before);
    }
    // The compatible direction still pastes.
    ok(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": image, "frame": 5 } }));
}

#[test]
fn r03_cross_project_paste_uses_the_copied_column_type() {
    // The same seed gives the same IDs: the copy's column type, not the destination's same-ID element, decides.
    let h = fixture();
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BG], "from": 1, "to": 1 }));
    let bg = id(&h, BG);
    ok(&h, "element.delete", json!({ "elementIds": [BG] }));
    let made = ok(&h, "element.create", json!({ "type": "drawing", "name": "Background" }))["elementIds"][0].clone();
    assert_ne!(made, json!(bg));
    let e = err(&h, "edit.paste", json!({ "context": "xsheet", "destination": { "elementId": made, "frame": 1 } }));
    assert_eq!(e.code, "cell.typeMismatch");
}

// ----- C31-R2-04 ------------------------------------------------------------------------------------------------------

#[test]
fn r04_generic_delete_and_cut_follow_notes_in_one_undo_step() {
    let h = fixture();
    clear_notes(&h, BOB);
    for (f, t) in [(2, "two"), (4, "four"), (6, "six")] {
        ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": f, "text": t }));
    }
    let before = cell_notes(&h, BOB);
    // Generic delete of cells clears their notes (no shift).
    ok(&h, "cell.select", json!({ "elementIds": [BOB], "from": 2, "to": 2 }));
    ok(&h, "edit.delete", json!({ "context": "xsheet" }));
    assert_eq!(cell_notes(&h, BOB), json!({ "4": "four", "6": "six" }));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(cell_notes(&h, BOB), before, "notes and exposure undo together");
    // Shifting cut-special over two separate frames: removed notes go, later ones follow both removals.
    ok(
        &h,
        "edit.cutSpecial",
        json!({ "context": "xsheet", "objectIds": [format!("{BOB}/cell:#2"), format!("{BOB}/cell:#4")], "options": { "createDeleteExposure": true } }),
    );
    assert_eq!(cell_notes(&h, BOB), json!({ "4": "six" }));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(cell_notes(&h, BOB), before);
    ok(&h, "edit.redo", json!({}));
    assert_eq!(cell_notes(&h, BOB), json!({ "4": "six" }));
}

#[test]
fn r04_every_shift_past_the_frame_domain_refuses_and_keeps_the_note() {
    let h = fixture();
    clear_notes(&h, BOB);
    ok(&h, "cell.note.set", json!({ "elementId": BOB, "frame": 99_999, "text": "edge" }));
    let before = get(&h, BOB);
    let revision = rev(&h);
    for (cmd, p) in [
        ("cell.range.add", json!({ "elementIds": [BOB], "frame": 1, "count": 2, "position": "before" })),
        ("cell.exposure.add", json!({ "elementIds": [BOB], "from": 1, "to": 1, "frames": 2 })),
    ] {
        let e = err(&h, cmd, p);
        assert_eq!(e.code, "cell.frameOutOfRange", "{cmd}: {e:?}");
        assert_eq!(rev(&h), revision, "{cmd}");
        assert_eq!(get(&h, BOB), before, "{cmd}");
    }
    // A shift that stays inside the domain moves the note to the last frame.
    ok(&h, "cell.range.add", json!({ "elementIds": [BOB], "frame": 1, "count": 1, "position": "before" }));
    assert_eq!(cell_notes(&h, BOB)["100000"], "edge");
}

// ----- C31-R2-05 ------------------------------------------------------------------------------------------------------

fn chars(v: &Value) -> usize {
    v.as_str().unwrap().chars().count()
}

#[test]
fn r05_derived_names_fit_after_truncation_and_collisions() {
    let h = fixture();
    // Two skeletons whose 64-character names share the first 59 characters: the companion rows keep their meaningful
    // suffix, fit, and the second set is numbered after shortening.
    let a = format!("{}A", "骨".repeat(63));
    let b = format!("{}B", "骨".repeat(63));
    let mut rows = Vec::new();
    for n in [&a, &b] {
        let id = ok(&h, "element.create", json!({ "type": "skeleton", "name": n }))["elementIds"][0]
            .as_str()
            .unwrap()
            .to_owned();
        let kids = ok(&h, "element.list", json!({ "parentId": id, "recursive": false }))["elements"].clone();
        for k in kids.as_array().unwrap() {
            let name = k["name"].clone();
            assert!(chars(&name) <= 64, "{name}");
            rows.push(name.as_str().unwrap().to_owned());
        }
    }
    assert!(rows.iter().any(|r| r.ends_with("-bones")) && rows.iter().any(|r| r.ends_with("-skin")), "{rows:?}");
    assert!(rows.iter().any(|r| r.ends_with("-bones_1")), "collision after shortening is numbered: {rows:?}");
    let distinct: std::collections::BTreeSet<_> = rows.iter().collect();
    assert_eq!(distinct.len(), rows.len());
    // The parent peg of a 64-character element keeps `-P`.
    let long = ok(&h, "element.create", json!({ "type": "drawing", "name": "柱".repeat(64) }))["elementIds"][0].clone();
    let name = ok(&h, "element.parentPeg.create", json!({ "elementIds": [long] }))["name"].clone();
    assert_eq!(chars(&name), 64);
    assert!(name.as_str().unwrap().ends_with("-P"), "{name}");
    // Generic dashed copy names cross a digit width inside 64 characters.
    let base = "描".repeat(64);
    ok(&h, "element.drawing.create", json!({ "elementId": BOB, "name": base, "exposeFrom": 41 }));
    for n in 2..=9 {
        let taken = format!("{}-{n}", "描".repeat(62));
        ok(&h, "element.drawing.create", json!({ "elementId": NOTES, "name": taken, "exposeFrom": 40 + n }));
    }
    ok(&h, "element.drawing.create", json!({ "elementId": NOTES, "name": base, "exposeFrom": 60 }));
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BOB], "from": 41, "to": 41 }));
    ok(
        &h,
        "edit.pasteSpecial",
        json!({ "context": "xsheet", "destination": { "elementId": NOTES, "frame": 70 }, "options": { "overwriteDrawings": false } }),
    );
    let list = ok(&h, "element.drawing.list", json!({ "elementId": NOTES }))["drawings"].clone();
    let want = format!("{}-10", "描".repeat(61));
    assert!(list.as_array().unwrap().iter().any(|x| x["name"] == want.as_str()), "{list}");
}

#[test]
fn r05_planner_is_the_one_source_of_generated_names() {
    use tt_anim::exposure::{MAX_NAME_CHARS, derived_name, plan_names};
    let base = "x".repeat(64);
    assert_eq!(derived_name(&base, "-mask").chars().count(), MAX_NAME_CHARS);
    assert!(derived_name(&base, "-mask").ends_with("-mask"));
    // A caller's name is used as given (never shortened silently).
    let long = "y".repeat(70);
    assert_eq!(plan_names(&long, "", "_", 1, &|_| false, 1, false), vec![long.clone()]);
    // Collisions are tested after shortening and after the counter gains a digit.
    let taken: Vec<String> = (1..=9).map(|k| format!("{}_{k}", "z".repeat(62))).chain(["z".repeat(64)]).collect();
    let got = plan_names(&"z".repeat(64), "", "_", 1, &|n| taken.iter().any(|t| t == n), 2, false);
    assert_eq!(got, vec![format!("{}_10", "z".repeat(61)), format!("{}_11", "z".repeat(61))]);
    let got = plan_names("m", "-mask", "_", 1, &|n| n == "m-mask", 1, false);
    assert_eq!(got, vec!["m-mask_1".to_owned()]);
}

// ----- C31-R2-06 ------------------------------------------------------------------------------------------------------

fn image_package(dir: &std::path::Path, seed: u64, colour: [u8; 4]) -> (std::path::PathBuf, String) {
    let mut parts = tt_model::fixture::sample_parts(seed).unwrap();
    let element = parts.elements.iter().find(|e| e.name() == "Background").unwrap();
    let bank = parts.banks.iter().find(|b| Some(b.id()) == element.drawing_bank_id().as_ref()).unwrap();
    let did = bank.drawings().ids()[0].clone();
    let at = parts.drawings.iter().position(|d| *d.id() == did).unwrap();
    let png = tt_render::encode_png(&colour, 1, 1).unwrap();
    let mut d = serde_json::to_value(&parts.drawings[at]).unwrap();
    let entry = d["nodes"][0]["image"]["entry"].as_str().unwrap().to_owned();
    d["nodes"][0]["width"] = json!(1);
    d["nodes"][0]["height"] = json!(1);
    d["nodes"][0]["image"] =
        json!({ "entry": entry, "mediaType": "image/png", "size": png.len(), "sha256": tt_persist::fsx::sha256(&png) });
    parts.drawings[at] = tt_model::object::from_json(&d).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let mut blobs = tt_persist::container::Blobs::default();
    blobs.insert(entry.clone(), tt_persist::container::BlobData::Bytes(std::sync::Arc::new(png)));
    let preserved = tt_persist::load::Preserved::default();
    let input = tt_persist::save::SaveInput {
        doc: &doc,
        preserved: &preserved,
        blobs: &blobs,
        session: None,
        saved_at: "2026-11-03T15:00:00Z".into(),
    };
    let path = dir.join(format!("Image-{seed}-{}.animpkg", colour[0]));
    tt_persist::save::save_package(&path, &input, None).unwrap();
    (path, entry)
}

fn pasted_image(h: &ExecutorHandle, created: &Value) -> Value {
    let d = created["createdDrawingIds"][0].as_str().unwrap();
    ok(h, "draw.drawing.get", json!({ "drawingId": d }))["nodes"][0]["image"].clone()
}

fn with_bytes(clip: &Value, entry: &str, png: &[u8]) -> Value {
    let mut c = clip.clone();
    c["payload"]["blobs"][entry] = json!(tt_persist::b64::encode(png));
    c
}

#[test]
fn r06_entries_are_remapped_on_collision_shared_when_equal_and_staged_with_the_call() {
    let dir = tmp();
    let (source, entry) = image_package(&dir, 0x0031_2061, [200, 10, 10, 255]);
    let a = tt_render::encode_png(&[200, 10, 10, 255], 1, 1).unwrap();
    let b = tt_render::encode_png(&[10, 10, 200, 255], 1, 1).unwrap();
    let h = fixture();
    job_ok(&h, "project.open", json!({ "path": source, "onUnsaved": "discard" }));
    ok(&h, "cell.clipboard.set", json!({ "elementIds": [BG], "from": 1, "to": 1 }));
    let clip_a = ok(&h, "edit.clipboard.get", json!({ "includeData": true }));
    assert!(clip_a["payload"]["blobs"][&entry].is_string(), "the copy carries its bytes");
    // A destination without the entry.
    assert!(h.open_document(tt_model::fixture::sample_document(0x0031_2062).unwrap(), None, false).is_ok());
    let target = ok(&h, "element.create", json!({ "type": "image", "name": "Target" }))["elementIds"][0].clone();
    ok(&h, "edit.clipboard.set", json!({ "content": clip_a }));
    // A dry run and a refused atomic group stage the entry with the call only.
    let env = with(
        &h,
        "cell.clipboard.apply",
        json!({ "elementId": target, "frame": 1 }),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let failed = batch(
        &h,
        json!({ "label": "rollback", "atomic": true, "steps": [
            { "id": "cell.clipboard.apply", "params": { "elementId": target, "frame": 1 } },
            { "id": "element.rename", "params": { "elementId": BOB, "name": "bad.name" } }
        ] }),
    );
    assert_ne!(failed.status, Status::Ok);
    // Had either left bytes A under the entry, bytes B would now collide and be renamed.
    ok(&h, "edit.clipboard.set", json!({ "content": with_bytes(&clip_a, &entry, &b) }));
    let first = pasted_image(&h, &ok(&h, "cell.clipboard.apply", json!({ "elementId": target, "frame": 1 })));
    assert_eq!(first["entry"], entry.as_str(), "nothing leaked from the dry run or the refused group");
    // Bytes A now collide: a fresh deterministic entry, the reference rewritten, the hash kept.
    ok(&h, "edit.clipboard.set", json!({ "content": clip_a }));
    let second = pasted_image(&h, &ok(&h, "cell.clipboard.apply", json!({ "elementId": target, "frame": 3 })));
    let fresh = second["entry"].as_str().unwrap().to_owned();
    assert_ne!(fresh, entry);
    assert!(fresh.contains(&tt_persist::fsx::sha256(&a)[..16]), "{fresh}");
    assert_eq!(second["sha256"], tt_persist::fsx::sha256(&a));
    // The same bytes again share that entry (dedupe).
    let third = pasted_image(&h, &ok(&h, "cell.clipboard.apply", json!({ "elementId": target, "frame": 5 })));
    assert_eq!(third["entry"], fresh.as_str());
    // Both survive save and open with their own bytes.
    let out = dir.join("both.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": out }));
    let loaded = tt_persist::load::load(&out, &Default::default()).unwrap();
    assert_eq!(loaded.blobs.read(&entry, 1 << 20).unwrap(), b);
    assert_eq!(loaded.blobs.read(&fresh, 1 << 20).unwrap(), a);
    ok(&h, "project.close", json!({ "onUnsaved": "discard" }));
    job_ok(&h, "project.open", json!({ "path": out }));
    let reread = ok(
        &h,
        "draw.drawing.get",
        json!({ "drawingId": get(&h, "scene:\"Intro\"/element:\"Target\"")["exposure"][2][0] }),
    );
    assert_eq!(reread["nodes"][0]["image"]["entry"], fresh.as_str());
}
