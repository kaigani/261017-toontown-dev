//! M5 C5.2a, ruling E-56: the window's thumbnails never block commands (04 §17.1 item 10). They are rendered off the
//! document thread from an immutable snapshot of the drawing at a revision into a bounded cache; the window reads them
//! through `ExecutorHandle::thumbnails_read` (a host read, not a command) and learns of finished ones from the host-only
//! notice `shell.thumbnailsReady`. With a dev-only delay on the renderer: 200 thumbnails pending while ordinary
//! commands answer at their normal speed; a drawing edited while its thumbnail renders never shows the old image (the
//! revision decides); the cache holds its bound; a read replaces the queue (a scroll drops what is out of view).
#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::thumbnails::ThumbnailRequest;
use tt_executor::{Event, ExecutorConfig, ExecutorHandle};

/// The hooks are process-wide: one test at a time.
static SERIAL: Mutex<()> = Mutex::new(());

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";

fn handle() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(30) }).unwrap();
    fork(&h);
    h
}

/// `n` new drawings of Bob (each with a stroke, so each renders something), their IDs.
fn drawings(h: &ExecutorHandle, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let d = ok(h, "element.drawing.create", json!({ "elementId": BOB, "name": format!("t{i}") }))["drawingId"]
                .as_str()
                .unwrap()
                .to_owned();
            ok(
                h,
                "draw.stroke.add",
                json!({ "drawingId": d, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": 100 + i, "y": 50 }] }),
            );
            d
        })
        .collect()
}

fn ask(ids: &[String], size: u32) -> Vec<ThumbnailRequest> {
    ids.iter().map(|d| ThumbnailRequest { drawing_id: d.clone(), size }).collect()
}

/// Waits (bounded) until a `shell.thumbnailsReady` notice says nothing is pending; returns the keys announced.
fn until_done(rx: &Receiver<Event>, bound: Duration) -> Vec<Value> {
    let start = Instant::now();
    let mut keys = Vec::new();
    while start.elapsed() < bound {
        if let Ok(e) = rx.recv_timeout(Duration::from_millis(100))
            && e.name == "shell.thumbnailsReady"
        {
            keys.extend(e.payload["items"].as_array().cloned().unwrap_or_default());
            if e.payload["pending"] == 0 {
                return keys;
            }
        }
    }
    panic!("the thumbnails were not done within {bound:?}: {} announced", keys.len());
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn timed(h: &ExecutorHandle, n: usize) -> Vec<Duration> {
    (0..n)
        .map(|_| {
            let t = Instant::now();
            ok(h, "doc.state.get", json!({}));
            t.elapsed()
        })
        .collect()
}

#[test]
fn two_hundred_pending_thumbnails_never_slow_ordinary_commands() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let ids = drawings(&h, 200);
    let before = median(timed(&h, 30));
    tt_executor::hooks::set_thumbnail_delay_ms(15);
    let rx = h.subscribe();
    let t = Instant::now();
    let read = h.thumbnails_read(ask(&ids, 64)).unwrap();
    let read_took = t.elapsed();
    assert_eq!((read.ready.len(), read.pending.len()), (0, 200), "all pending at first");
    // The work is out (about 3 s with the delay): commands answer as before.
    let during = median(timed(&h, 30));
    let pending_now = h.thumbnails_usage().1;
    tt_executor::hooks::set_thumbnail_delay_ms(0);
    assert!(pending_now < 200, "the work was still out while the commands ran ({pending_now} rendered)");
    assert!(
        during <= before * 4 + Duration::from_millis(5),
        "ordinary commands slowed while thumbnails rendered: median {during:?} against {before:?}"
    );
    assert!(read_took < Duration::from_secs(1), "the read itself is short: {read_took:?}");
    until_done(&rx, Duration::from_secs(60));
    let again = h.thumbnails_read(ask(&ids, 64)).unwrap();
    assert_eq!((again.ready.len(), again.pending.len()), (200, 0), "all ready from the cache");
    assert!(again.ready.iter().all(|r| r.png.len() > 20));
}

#[test]
fn an_edited_drawing_never_shows_its_old_image() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let ids = drawings(&h, 1);
    let rx = h.subscribe();
    tt_executor::hooks::set_thumbnail_delay_ms(300);
    let first = h.thumbnails_read(ask(&ids, 64)).unwrap();
    let old_rev = first.pending[0].rev;
    // Edited while its thumbnail renders: a new revision.
    ok(
        &h,
        "draw.stroke.add",
        json!({ "drawingId": ids[0], "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": -80, "y": 40 }] }),
    );
    let done = until_done(&rx, Duration::from_secs(10));
    tt_executor::hooks::set_thumbnail_delay_ms(0);
    assert_eq!(done[0]["rev"], old_rev, "the old revision's thumbnail finished");
    let now = h.thumbnails_read(ask(&ids, 64)).unwrap();
    assert!(now.ready.iter().all(|r| r.key.rev != old_rev), "the old image was answered for the new revision");
    assert_eq!(now.pending.len(), 1);
    assert!(now.pending[0].rev > old_rev);
    until_done(&rx, Duration::from_secs(10));
    let last = h.thumbnails_read(ask(&ids, 64)).unwrap();
    assert_eq!(last.ready.len(), 1);
    assert_eq!(last.ready[0].key.rev, now.pending[0].rev);
}

#[test]
fn the_cache_holds_its_bound() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let ids = drawings(&h, 12);
    let rx = h.subscribe();
    // One thumbnail's size, from a first render.
    h.thumbnails_read(ask(&ids[..1], 128)).unwrap();
    until_done(&rx, Duration::from_secs(10));
    let one = h.thumbnails_usage().0;
    assert!(one > 0);
    tt_executor::hooks::set_thumbnail_cache_bound((one * 3) as u64);
    h.thumbnails_read(ask(&ids, 128)).unwrap();
    until_done(&rx, Duration::from_secs(30));
    let (bytes, count) = h.thumbnails_usage();
    tt_executor::hooks::set_thumbnail_cache_bound(0);
    assert!(bytes <= one * 3 + one / 2, "the cache holds {bytes} bytes in {count} thumbnails past its bound");
    assert!(count < 12, "older thumbnails were dropped: {count}");
}

#[test]
fn a_read_replaces_the_queue() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let ids = drawings(&h, 6);
    let rx = h.subscribe();
    tt_executor::hooks::set_thumbnail_delay_ms(150);
    h.thumbnails_read(ask(&ids[..5], 64)).unwrap();
    // A scroll: now only the last drawing is in view.
    let read = h.thumbnails_read(ask(&ids[5..], 64)).unwrap();
    assert_eq!(read.pending.len(), 1);
    let done = until_done(&rx, Duration::from_secs(10));
    tt_executor::hooks::set_thumbnail_delay_ms(0);
    let rendered: Vec<&str> = done.iter().filter_map(|k| k["drawingId"].as_str()).collect();
    assert!(rendered.contains(&ids[5].as_str()));
    // At most the one already rendering when the scroll came finishes of the first five.
    assert!(rendered.iter().filter(|d| ids[..5].iter().any(|x| x == *d)).count() <= 1, "{rendered:?}");
}

#[test]
fn missing_drawings_and_no_project() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let read = h.thumbnails_read(ask(&["dr_00000000000000000000000000".to_owned()], 64)).unwrap();
    assert_eq!(read.missing.len(), 1);
    let none = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    assert_eq!(none.thumbnails_read(Vec::new()).unwrap_err().code, "command.disabled");
}

/// A new drawing of Bob with one stroke ending at `x`; its ID.
fn drawing_with(h: &ExecutorHandle, name: &str, x: f64) -> String {
    let d = ok(h, "element.drawing.create", json!({ "elementId": BOB, "name": name }))["drawingId"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(
        h,
        "draw.stroke.add",
        json!({ "drawingId": d, "kind": "pencil", "points": [{ "x": 0, "y": 0 }, { "x": x, "y": 50 }] }),
    );
    d
}

/// Replaces the open project by a fresh copy of the sample document (a second fork of the same document).
fn fork(h: &ExecutorHandle) {
    let options = tt_executor::OpenOptions {
        on_unsaved: tt_executor::UnsavedPolicy::Discard,
        ..tt_executor::OpenOptions::default()
    };
    let opened =
        h.open_document_with(tt_model::fixture::sample_document(1).unwrap(), options, tt_executor::Origin::User);
    assert!(opened.is_ok(), "{:?}", opened.error);
}

/// The thumbnail once ready (bounded).
fn ready(h: &ExecutorHandle, d: &str) -> tt_executor::thumbnails::ThumbnailReady {
    let start = Instant::now();
    loop {
        let r = h.thumbnails_read(ask(&[d.to_owned()], 64)).unwrap();
        if let Some(t) = r.ready.into_iter().next() {
            return t;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "no thumbnail of {d}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Rulings E-70 (C52A-R1S-02) and E-71: two projects forked from one document hold the same drawing at the same
/// revision with different content; after replacing the first by the second, its image is never answered.
#[test]
fn a_forked_project_never_shows_the_other_forks_image() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let a = drawing_with(&h, "fork", 100.0);
    let rev_a = ok(&h, "doc.object.get", json!({ "ref": a }))["object"]["rev"].clone();
    let image_a = ready(&h, &a);
    // The second fork: the same document, the same calls, other content.
    fork(&h);
    let b = drawing_with(&h, "fork", 400.0);
    assert_eq!(b, a, "the same drawing ID in both forks");
    assert_eq!(ok(&h, "doc.object.get", json!({ "ref": b }))["object"]["rev"], rev_a, "at the same revision");
    let first = h.thumbnails_read(ask(std::slice::from_ref(&b), 64)).unwrap();
    assert!(first.ready.is_empty(), "the other fork's image answered from the cache");
    let image_b = ready(&h, &b);
    assert_ne!(image_b.png, image_a.png, "the second fork's own image");
}

/// E-71 (C52A-R1-01): a render of the replaced project still in flight is neither kept nor announced. One worker
/// renders in order, so the new project's thumbnail is announced only after the old render has finished.
#[test]
fn a_render_in_flight_across_a_replacement_is_dropped() {
    let _guard = SERIAL.lock().unwrap();
    let h = handle();
    let a = drawing_with(&h, "inflight", 100.0);
    tt_executor::hooks::set_thumbnail_delay_ms(300);
    assert_eq!(h.thumbnails_read(ask(std::slice::from_ref(&a), 64)).unwrap().pending.len(), 1);
    let rx = h.subscribe();
    fork(&h);
    // Another drawing of the new project (an element first, so its ID is not the old drawing's).
    ok(&h, "element.create", json!({ "type": "drawing", "name": "Shift" }));
    let c = drawing_with(&h, "fresh", 250.0);
    assert_ne!(c, a);
    assert_eq!(h.thumbnails_read(ask(std::slice::from_ref(&c), 64)).unwrap().pending.len(), 1);
    let announced = until_done(&rx, Duration::from_secs(30));
    tt_executor::hooks::set_thumbnail_delay_ms(0);
    assert!(
        announced.iter().all(|k| k["drawingId"] != a.as_str()),
        "the old project's render announced: {announced:?}"
    );
    assert!(announced.iter().any(|k| k["drawingId"] == c.as_str()));
    assert_eq!(h.thumbnails_usage().1, 1, "only the new project's thumbnail is kept");
}
