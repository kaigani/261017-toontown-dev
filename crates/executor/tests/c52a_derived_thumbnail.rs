//! M5 C5.2a: a drawing's derived `thumbnail` (10 line 493: a blob; the C5.2 plan §7; ruling E-56: the agents' form is
//! synchronous). `doc.object.get {include: ["derived"]}` answers a blob naming the derived-cache entry of the drawing at
//! its revision (`cache/thumbnails/<drawing>-<revision>-64.png`, 10 line 743), which `doc.blob.get {entry}` reads while
//! the drawing is at that revision. One renderer: the bytes are those of `element.drawing.list {thumbnails: 64}`, and
//! the window's background read finds the same image in the shared cache. A drawing the call has changed but not
//! committed has no thumbnail yet (its revision is stamped at commit): it is named in `doc.derivedUnavailable`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::thumbnails::ThumbnailRequest;

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

/// A new drawing of Bob with one stroke; its ID.
fn drawing(h: &tt_executor::ExecutorHandle, name: &str) -> String {
    let d = ok(h, "element.drawing.create", json!({ "elementId": BOB, "name": name }))["drawingId"]
        .as_str()
        .unwrap()
        .to_owned();
    stroke(h, &d, 100.0);
    d
}

fn stroke(h: &tt_executor::ExecutorHandle, d: &str, x: f64) {
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": d, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": x, "y": 50 }] }),
    );
}

fn derived(h: &tt_executor::ExecutorHandle, d: &str) -> Value {
    ok(h, "doc.object.get", json!({ "ref": d, "include": ["derived"] }))["object"].clone()
}

/// The PNG's width and height from its header.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let be = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (be(16), be(20))
}

#[test]
fn a_drawings_derived_thumbnail_is_a_blob_that_doc_blob_get_reads() {
    let h = fixture();
    let d = drawing(&h, "thumb");
    let o = derived(&h, &d);
    let t = &o["thumbnail"];
    let entry = t["entry"].as_str().unwrap();
    let rev = o["rev"].as_u64().unwrap();
    assert_eq!(entry, format!("cache/thumbnails/{d}-{rev}-64.png"));
    assert_eq!(t["mediaType"], "image/png");
    assert_eq!(t["sha256"].as_str().unwrap().len(), 64);
    // The projection validates against the drawing's type schema (the derived property is a blob).
    let v = tt_model::schema::validate(tt_model::ObjectType::Drawing, &o, 20).unwrap();
    assert!(v.is_empty(), "{v:?}");
    let blob = ok(&h, "doc.blob.get", json!({ "entry": entry }));
    assert_eq!((&blob["size"], &blob["sha256"], &blob["mediaType"]), (&t["size"], &t["sha256"], &t["mediaType"]));
    let bytes = tt_persist::b64::decode(blob["base64"].as_str().unwrap()).unwrap();
    assert_eq!(png_size(&bytes), (64, 64));
    // One renderer: the agents' listing at the same size gives the same bytes.
    let listed = ok(&h, "element.drawing.list", json!({ "elementId": BOB, "thumbnails": 64 }));
    let row = listed["drawings"].as_array().unwrap().iter().find(|r| r["id"] == d.as_str()).unwrap().clone();
    assert_eq!(row["thumbnail"], blob["base64"]);
    // Read again: the same blob (from the cache, at the same revision).
    assert_eq!(derived(&h, &d)["thumbnail"], *t);
}

#[test]
fn the_entry_follows_the_drawings_revision() {
    let h = fixture();
    let d = drawing(&h, "edited");
    let before = derived(&h, &d)["thumbnail"].clone();
    stroke(&h, &d, 400.0);
    let after = derived(&h, &d)["thumbnail"].clone();
    assert_ne!(after["entry"], before["entry"]);
    assert_ne!(after["sha256"], before["sha256"], "the edited drawing's thumbnail is a new image");
    // The old revision's entry is gone, as a derived cache's entry may be; the new one reads.
    let gone = run(&h, "doc.blob.get", json!({ "entry": before["entry"] }));
    assert_eq!(gone.error.unwrap().code, "project.mediaMissing");
    assert_eq!(ok(&h, "doc.blob.get", json!({ "entry": after["entry"] }))["sha256"], after["sha256"]);
    // An entry of the folder's form for a drawing that does not exist, and one of another form.
    let none = run(&h, "doc.blob.get", json!({ "entry": "cache/thumbnails/dr_00000000000000000000000000-1-64.png" }));
    assert_eq!(none.error.unwrap().code, "project.mediaMissing");
    let other = run(&h, "doc.blob.get", json!({ "entry": "cache/thumbnails/nothing.png" }));
    assert_eq!(other.error.unwrap().code, "project.mediaMissing");
}

#[test]
fn the_windows_read_finds_the_agents_thumbnail_in_the_shared_cache() {
    let h = fixture();
    let d = drawing(&h, "shared");
    let t = derived(&h, &d)["thumbnail"].clone();
    let blob = ok(&h, "doc.blob.get", json!({ "entry": t["entry"] }));
    let read = h.thumbnails_read(vec![ThumbnailRequest { drawing_id: d.clone(), size: 64 }]).unwrap();
    assert!(read.pending.is_empty(), "rendered once already: {read:?}");
    assert_eq!(read.ready.len(), 1);
    assert_eq!(read.ready[0].png, blob["base64"].as_str().unwrap());
}

#[test]
fn a_drawing_the_call_changed_has_no_thumbnail_until_it_commits() {
    let h = fixture();
    let d = drawing(&h, "staged");
    let env = batch(
        &h,
        json!({ "label": "Edit and read", "steps": [
            { "id": "draw.stroke.add", "params": { "drawingId": d, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": 300, "y": 10 }] } },
            { "id": "doc.object.get", "params": { "ref": d, "include": ["derived"] } }
        ] }),
    );
    assert!(env.is_ok(), "{:?}", env.error);
    let step = &env.steps.as_ref().unwrap()[1];
    let o = &step.result.as_ref().unwrap()["object"];
    assert!(o.get("thumbnail").is_none(), "a thumbnail of uncommitted content: {o}");
    let w = step.warnings.iter().find(|w| w.code == "doc.derivedUnavailable").unwrap();
    assert!(w.message.contains("drawing.thumbnail (the drawing has changes this call has not committed)"), "{w:?}");
    // Committed, the drawing has its thumbnail at the new revision.
    let o = derived(&h, &d);
    assert_eq!(o["thumbnail"]["entry"], format!("cache/thumbnails/{d}-{}-64.png", o["rev"]));
    // Untouched drawings in the same batch keep theirs.
    let other = drawing(&h, "untouched");
    let env = batch(
        &h,
        json!({ "label": "Edit one, read the other", "steps": [
            { "id": "draw.stroke.add", "params": { "drawingId": d, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": 20, "y": 90 }] } },
            { "id": "doc.object.get", "params": { "ref": other, "include": ["derived"] } }
        ] }),
    );
    assert!(env.steps.as_ref().unwrap()[1].result.as_ref().unwrap()["object"]["thumbnail"].is_object());
}

#[test]
fn fields_without_the_thumbnail_render_none() {
    let h = fixture();
    let d = drawing(&h, "fields");
    let o =
        ok(&h, "doc.object.get", json!({ "ref": d, "include": ["derived"], "fields": ["usage"] }))["object"].clone();
    assert!(o.get("thumbnail").is_none() && o.get("usage").is_some());
    let read = h.thumbnails_read(vec![ThumbnailRequest { drawing_id: d, size: 64 }]).unwrap();
    assert_eq!(read.pending.len(), 1, "nothing was rendered for a read that did not ask for it");
}
