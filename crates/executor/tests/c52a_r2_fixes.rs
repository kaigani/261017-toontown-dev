#![cfg(feature = "test-hooks")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, dead_code, reason = "test")]
mod common;
mod c13_support;
use common::*;
use serde_json::{Value,json};
use tt_executor::{ExecutorHandle,ExecutorConfig,Event};
use tt_executor::thumbnails::ThumbnailRequest;
use std::sync::{Mutex,mpsc::Receiver};
use std::time::{Duration,Instant};
static SERIAL: Mutex<()> = Mutex::new(());
const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
fn handle() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(ExecutorConfig { budget: Duration::from_secs(30) }).unwrap();
    fork(&h);
    h
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

#[test]
fn q4_saved_forks_same_id_revision_different_image() {
 let _guard=SERIAL.lock().unwrap();
 let h=fixture();
 let dir=c13_support::tmp();
 let base=dir.join("base.animpkg").to_string_lossy().into_owned();
 let b=dir.join("b.animpkg").to_string_lossy().into_owned();
 let a=dir.join("a.animpkg").to_string_lossy().into_owned();
 c13_support::job_ok(&h,"project.saveAs",json!({"path":base}));
 let da=drawing_with(&h,"fork",100.0);
 let ra=get(&h,&da)["rev"].clone();
 c13_support::job_ok(&h,"project.saveAs",json!({"path":a}));
 ok(&h,"project.close",json!({}));
 c13_support::job_ok(&h,"project.open",json!({"path":base}));
 let db=drawing_with(&h,"fork",400.0);
 assert_eq!(da,db);
 assert_eq!(ra,get(&h,&db)["rev"]);
 c13_support::job_ok(&h,"project.saveAs",json!({"path":b}));
 ok(&h,"project.close",json!({}));
 c13_support::job_ok(&h,"project.open",json!({"path":a}));
 let ia=ready(&h,&da);
 ok(&h,"project.close",json!({}));
 c13_support::job_ok(&h,"project.open",json!({"path":b}));
 let ib=ready(&h,&db);
 assert_ne!(ia.png,ib.png);
}
