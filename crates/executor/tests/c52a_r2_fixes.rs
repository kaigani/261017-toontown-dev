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

mod q5 {
use crate::common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};

const ROW: f64 = 20.0;
const COL: f64 = 80.0;

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "scrollLeft": 0, "platform": "windows"})
}

fn s(x: f64, y: f64) -> Value {
    json!({"x": x, "y": y, "modifiers": []})
}

fn s_with(x: f64, y: f64, modifiers: &[&str]) -> Value {
    json!({"x": x, "y": y, "modifiers": modifiers})
}

/// The point in the middle of a cell (column `c`, frame `f`).
fn cell(c: usize, f: u32) -> (f64, f64) {
    (c as f64 * COL + COL / 2.0, f64::from(f - 1) * ROW + ROW / 2.0)
}

/// The sheet's columns (element IDs, left to right) and each one's exposures (`[drawingId, from, to]`).
/// A column: its element and its exposures (`[drawingId, from, to]`).
type Column = (String, Vec<(String, u32, u32)>);

fn columns(h: &ExecutorHandle) -> Vec<Column> {
    ok(h, "xsheet.exposures.get", json!({}))["columns"]
        .as_array()
        .unwrap()
        .iter()
        // Every column the sheet shows (a sound column has clips, no exposures: its runs are empty).
        .filter(|c| c["visible"] == true)
        .map(|c| {
            let runs = c["exposures"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    (
                        r["drawingId"].as_str().unwrap().to_owned(),
                        r["from"].as_u64().unwrap() as u32,
                        r["to"].as_u64().unwrap() as u32,
                    )
                })
                .collect();
            (c["elementId"].as_str().unwrap().to_owned(), runs)
        })
        .collect()
}

fn state(h: &ExecutorHandle) -> Value {
    json!({
        "sheet": ok(h, "xsheet.exposures.get", json!({})),
        "selection": ok(h, "cell.selection.get", json!({})),
        "lightTable": ok(h, "xsheet.lightTable.list", json!({})),
        "app": ok(h, "app.state.get", json!({})),
        "rev": rev(h),
    })
}

/// The plan equals the expectation; the window's press and release commit it; the direct commands give the same state.
fn three_way(setup: &dyn Fn(&ExecutorHandle), surface: &str, samples: Value, expected: Value) {
    three_way_in(setup, layout(surface), samples, expected);
}

/// [`three_way`] with a given layout.
fn three_way_in(setup: &dyn Fn(&ExecutorHandle), layout: Value, samples: Value, expected: Value) {
    let (hw, hd) = (fixture(), fixture());
    setup(&hw);
    setup(&hd);
    let plan = hw.timing_replay(Origin::User, layout.clone(), samples.clone(), false).unwrap()["plan"].clone();
    assert_eq!(plan, expected, "the interpreter's plan");
    let press =
        hw.timing_gesture(Origin::User, json!({"phase": "press", "layout": layout, "sample": samples[0]})).unwrap();
    let out = hw
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["plan"], expected, "the window's commit: {out}");
    for env in out["envelopes"].as_array().unwrap() {
        assert_eq!(env["status"], "ok", "{env}");
    }
    for c in expected["commands"].as_array().unwrap() {
        ok(&hd, c[0].as_str().unwrap(), c[1].clone());
    }
    assert_eq!(state(&hw), state(&hd), "the window's path and the direct commands");
}

fn none(_: &ExecutorHandle) {}

#[test]
fn an_edge_drag_whose_exposure_changed_is_cancelled() {
    let h = fixture();
    let cols = columns(&h);
    let bob = cols[0].0.clone();
    let (_, _, to) = cols[0].1[0].clone();
    let other = cols[0].1.iter().map(|r| r.0.clone()).find(|d| *d != cols[0].1[0].0).expect("a second drawing of Bob");
    let x = COL / 2.0;
    let edge = f64::from(to) * ROW - 1.0;
    let samples = json!([s(x, edge), s(x, edge + 2.0 * ROW)]);
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("xsheetGrid"), "sample": samples[0]}))
        .unwrap();
    assert_eq!(press["target"]["kind"], "exposureEdge", "{press}");
    // The last frame of the grabbed exposure now shows another drawing.
    ok(&h, "cell.label.set", json!({"elementIds": [bob], "from": to, "to": to, "drawingId": other}));
    let changed = rev(&h);
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["cancelled"], true, "{out}");
    assert_eq!(out["error"]["code"], "timeline.targetChanged");
    assert_eq!(rev(&h), changed, "nothing of the gesture was committed");
    // A change of another column under the same drag leaves the grabbed exposure and its outcome as they were.
    let h = fixture();
    let press = h
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout("xsheetGrid"), "sample": samples[0]}))
        .unwrap();
    ok(&h, "cell.blank.add", json!({"elementIds": [cols[1].0], "frame": 1}));
    let out = h
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert!(out["cancelled"] != true, "{out}");
}


#[test]
fn huge_layouts_refuse() {
 for field in ["scrollLeft","lightTable"] {
 let h=fixture();
 let mut l=layout("xsheetGrid");
 l[field]=if field=="scrollLeft" {json!(1e300)} else {json!({"x":1e300,"y":1e300,"width":100,"height":100})};
 let e=h.timing_gesture(Origin::User,json!({"phase":"press","layout":l,"sample":s(40.0,20.0)})).unwrap_err();
 assert_eq!(e.code,"arg.invalid");
 ok(&h,"doc.state.get",json!({}));
 }
}

}
