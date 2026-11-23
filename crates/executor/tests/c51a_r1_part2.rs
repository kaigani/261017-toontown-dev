#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn c1_scene_and_project_lifecycle() {
    let h = fixture();
    ok(&h, "playback.loop.set", json!({"enabled":true}));
    ok(&h, "playback.start", json!({}));
    ok(&h, "app.context.set", json!({"sceneId":"scene:#2"}));
    assert_eq!(ok(&h,"playback.state.get",json!({}))["playing"],false,"09:210 scene switch");
    let dir = std::env::temp_dir().join(format!("tt-c51-part2-{}",tt_executor::random_token(8)));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Second.animpkg");
    let second = fixture();
    let job = ok(&second,"project.saveAs",json!({"path":path.to_string_lossy()}))["jobId"].clone();
    ok(&second,"job.wait",json!({"jobId":job,"timeoutMs":60000}));
    drop(second);
    ok(&h,"playback.start",json!({}));
    ok(&h,"project.close",json!({"onUnsaved":"discard"}));
    let job = ok(&h,"project.open",json!({"path":path.to_string_lossy(),"onUnsaved":"discard"}))["jobId"].clone();
    ok(&h,"job.wait",json!({"jobId":job,"timeoutMs":60000}));
    let rx = h.subscribe();
    let before = ok(&h,"playback.state.get",json!({}));
    assert_eq!(before["playing"],false);
    let end = Instant::now()+Duration::from_secs(2);
    while Instant::now()<end {
        if let Ok(e)=rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
            assert_ne!(e.name,"timeline.currentFrame.changed","stale tick: {e:?}");
        }
    }
    assert_eq!(ok(&h,"playback.state.get",json!({}))["frame"],before["frame"]);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn c2_throttle_and_final_frame() {
    let h=fixture();
    ok(&h,"element.timing.set",json!({"elementIds":["scene:\"Intro\"/element:\"Bob-P\""],"duration":96}));
    ok(&h,"timeline.playbackRange.set",json!({"start":1,"end":96}));
    ok(&h,"timeline.currentFrame.set",json!({"frame":1}));
    ok(&h,"playback.rate.set",json!({"fps":24}));
    ok(&h,"playback.loop.set",json!({"enabled":false}));
    let rx=h.subscribe();
    ok(&h,"playback.start",json!({}));
    let deadline=Instant::now()+Duration::from_secs(60);
    let mut frames=Vec::new();
    loop {
        let e=rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
        if e.name=="timeline.currentFrame.changed" { frames.push(e.payload["frame"].clone()); }
        if e.name=="playback.stopped" { assert_eq!(e.payload["frame"],96); break; }
    }
    assert_eq!(ok(&h,"playback.state.get",json!({}))["playing"],false);
    assert_eq!(frames.last(),Some(&json!(96)),"04:1466 final event");
    assert!(!frames.is_empty() && frames.len()<=45,"04:1466 nominal four seconds: {} events",frames.len());
    println!("C2 frame events: {} final: 96",frames.len());
}

use serde_json::Value;
use tt_executor::{ExecutorHandle, Origin, BatchRequest};
/// The fixture's visible rows (Timeline order; no row is collapsed in the sample document): `(id, name)`.
fn rows(h: &ExecutorHandle) -> Vec<(String, String)> {
    ok(h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["elementId"].as_str().unwrap().to_owned(), r["name"].as_str().unwrap().to_owned()))
        .collect()
}

fn row(h: &ExecutorHandle, name: &str) -> (usize, String) {
    let r = rows(h);
    let i = r.iter().position(|(_, n)| n == name).unwrap();
    (i, r[i].0.clone())
}

fn layout(surface: &str) -> Value {
    json!({"surface": surface, "zoomLevel": 6, "firstFrame": 1, "scrollTop": 0, "platform": "windows"})
}

fn s(x: f64, y: f64) -> Value {
    json!({"x": x, "y": y, "modifiers": []})
}

/// The state compared between the window's path and the direct command.
fn state(h: &ExecutorHandle) -> Value {
    json!({
        "rows": ok(h, "timeline.rows.list", json!({})),
        "app": ok(h, "app.state.get", json!({})),
        "history": ok(h, "edit.history.get", json!({}))["entries"].as_array().map(|e| e.len()),
        "rev": rev(h),
    })
}

/// The interpreter's plan equals the expectation written from the spec (D-23); the window's press and release commit
/// it; the direct commands give the same state exactly (D-32).
fn three_way(surface: &str, samples: Value, expected: Value) {
    let (hw, hd) = (fixture(), fixture());
    let plan = hw.timing_replay(Origin::User, layout(surface), samples.clone(), false).unwrap()["plan"].clone();
    assert_eq!(plan, expected, "the interpreter's plan");
    let press = hw
        .timing_gesture(Origin::User, json!({"phase": "press", "layout": layout(surface), "sample": samples[0]}))
        .unwrap();
    let out = hw
        .timing_gesture(Origin::User, json!({"phase": "release", "grabId": press["grabId"], "samples": samples}))
        .unwrap();
    assert_eq!(out["plan"], expected, "the window's commit: {out}");
    for env in out["envelopes"].as_array().unwrap() {
        assert_eq!(env["status"], "ok", "{env}");
    }
    match expected["kind"].as_str().unwrap() {
        "commands" => {
            for c in expected["commands"].as_array().unwrap() {
                ok(&hd, c[0].as_str().unwrap(), c[1].clone());
            }
        }
        "batch" => {
            let steps: Vec<Value> =
                expected["steps"].as_array().unwrap().iter().map(|c| json!({"id": c[0], "params": c[1]})).collect();
            let req: BatchRequest =
                serde_json::from_value(json!({"label": expected["label"], "atomic": true, "steps": steps})).unwrap();
            let env = hd.batch(req, Origin::User);
            assert!(env.is_ok(), "{env:?}");
        }
        _ => {}
    }
    assert_eq!(state(&hw), state(&hd), "window and direct commands agree exactly (D-32)");
}

fn one(command: &str, params: Value) -> Value {
    json!({"kind": "commands", "commands": [[command, params]]})
}

const RH: f64 = 22.0;
const CW: f64 = 16.0;

#[test]
fn d23_loop_handle_and_stretch_from_start() {
    let h = fixture();
    let (fade, id) = row(&h, "Fade");
    // 04 line 720: the loop handle (the upper half of a loopable bar's end) adjusts the loops: the bar's 48-frame cycle
    // dragged to 96 frames is two loops.
    let y = fade as f64 * RH + 5.0;
    three_way(
        "tracks",
        json!([s(48.0 * CW, y), s(96.0 * CW, y)]),
        one("element.timing.set", json!({"elementIds": [id], "loops": 2})),
    );
    // 04 line 719: the start dragged changes start and duration keeping the end (keyframes scaled).
    let y = fade as f64 * RH + 16.0;
    three_way(
        "tracks",
        json!([s(2.0, y), s(2.0 + 4.0 * CW, y)]),
        one("element.timing.set", json!({"elementIds": [id], "duration": 44, "anchor": "end", "keyframes": "scale"})),
    );
}

#[test]
fn d23_keyframe_drag_moves_the_keys_of_that_frame() {
    let h = fixture();
    let (peg, id) = row(&h, "Bob-P");
    // 04 line 621, 05 §10.1 "Move keyframe": the peg's key at frame 48 dragged ten frames earlier.
    let y = peg as f64 * RH + 11.0;
    let x = 47.0 * CW + CW / 2.0;
    three_way(
        "tracks",
        json!([s(x, y), s(x - 10.0 * CW, y)]),
        one("anim.key.move", json!({"nodeId": id, "fromFrame": 48, "toFrame": 38})),
    );
}

#[test]
fn d23_row_drops_reorder_attach_and_change_level() {
    let h = fixture();
    let r = rows(&h);
    let (bg, bg_id) = row(&h, "Background");
    let (peg, peg_id) = row(&h, "Bob-P");
    let (bob, bob_id) = row(&h, "Bob");
    let root: Vec<String> = ok(&h, "timeline.rows.list", json!({}))["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["depth"] == 0)
        .map(|r| r["elementId"].as_str().unwrap().to_owned())
        .collect();
    let press = s(40.0, bg as f64 * RH + 11.0);
    // 04 line 478: a row dropped between two rows of its level reorders the level (the upper quarter of Bob-P's row
    // drops before it).
    let mut order: Vec<String> = root.iter().filter(|x| **x != bg_id).cloned().collect();
    let at = order.iter().position(|x| *x == peg_id).unwrap();
    order.insert(at, bg_id.clone());
    three_way(
        "layers",
        json!([press, s(40.0, peg as f64 * RH + 2.0)]),
        one("element.order.set", json!({"order": "timeline", "elementIds": order})),
    );
    // 04 §9.2 line 487: dropped onto a peg row (its middle half), the row is attached.
    three_way(
        "layers",
        json!([press, s(40.0, peg as f64 * RH + 11.0)]),
        one("element.parent.set", json!({"elementIds": [bg_id], "parentId": peg_id})),
    );
    // Between rows of another level (before Bob, Bob-P's first child): attached at that index.
    three_way(
        "layers",
        json!([press, s(40.0, bob as f64 * RH + 2.0)]),
        one("element.parent.set", json!({"elementIds": [bg_id], "parentId": peg_id, "index": 0})),
    );
    // A child dropped below the last row goes to the end of the root level: detached, then the root order (one step).
    let mut root_after: Vec<String> = root.clone();
    root_after.push(bob_id.clone());
    three_way(
        "layers",
        json!([s(40.0, bob as f64 * RH + 11.0), s(40.0, r.len() as f64 * RH + 5.0)]),
        json!({"kind": "batch", "label": "Move Rows", "steps": [
            ["element.parent.remove", {"elementIds": [bob_id]}],
            ["element.order.set", {"order": "timeline", "elementIds": root_after}],
        ]}),
    );
}


#[test]
fn d1_exact_threshold_and_between_rows() {
    // layout dragThreshold=4, cellWidth=16: start just before a cell boundary.
    let h=fixture(); let (i,id)=row(&h,"Bob-P"); let y=i as f64*RH+16.0;
    three_way("tracks",json!([s(28.0,y),s(32.0,y)]),one("element.timing.set",json!({"elementIds":[id],"startDelta":1})));
    let out=h.timing_replay(Origin::User,layout("tracks"),json!([s(28.0,y),s(31.0,y)]),true).unwrap();
    assert_eq!(out["plan"]["commands"][0][0],"cell.select","04:687; below threshold is click: {out}");
    let (bg,bg_id)=row(&h,"Background"); let (peg,peg_id)=row(&h,"Bob-P");
    let root:Vec<String>=ok(&h,"timeline.rows.list",json!({}))["rows"].as_array().unwrap().iter().filter(|r|r["depth"]==0).map(|r|r["elementId"].as_str().unwrap().to_owned()).collect();
    let mut order:Vec<String>=root.into_iter().filter(|r|r!=&bg_id).collect(); let at=order.iter().position(|r|r==&peg_id).unwrap();order.insert(at,bg_id);
    three_way("layers",json!([s(40.0,bg as f64*RH+11.0),s(40.0,peg as f64*RH)]),one("element.order.set",json!({"order":"timeline","elementIds":order})));
}

#[test]
fn d2_cell_exposure_change_cancels() {
 let h=fixture(); let (i,id)=row(&h,"Bob");
 ok(&h,"cell.select",json!({"elementIds":[id],"from":1,"to":6}));
 let samples=json!([s(24.0,i as f64*RH+16.0),s(184.0,i as f64*RH+16.0)]);
 let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout("tracks"),"sample":samples[0]})).unwrap();
 ok(&h,"cell.label.set",json!({"elementIds":[id],"from":1,"to":6,"drawing":"2"}));
 let before=state(&h);
 let out=h.timing_gesture(Origin::User,json!({"phase":"release","grabId":p["grabId"],"samples":samples})).unwrap();
 assert_eq!(out["error"]["code"],"timeline.targetChanged","E-10: {out}");
 assert_eq!(state(&h),before,"cancel changes nothing");
}

#[test]
fn d2_deleted_destination_parent_cancels() {
 let h=fixture(); let (i,id)=row(&h,"Background"); let (dest,parent)=row(&h,"Bob-P");
 let samples=json!([s(40.0,i as f64*RH+11.0),s(40.0,dest as f64*RH+11.0)]);
 let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout("layers"),"sample":samples[0]})).unwrap();
 ok(&h,"element.delete",json!({"elementIds":[parent]}));
 let before=rev(&h); let sheet=ok(&h,"timeline.rows.list",json!({}));
 let out=h.timing_gesture(Origin::User,json!({"phase":"release","grabId":p["grabId"],"samples":samples})).unwrap();
 assert_eq!(out["error"]["code"],"timeline.targetChanged","E-10: {id}: {out}");
 assert_eq!(rev(&h),before); assert_eq!(ok(&h,"timeline.rows.list",json!({})),sheet);
}

#[test]
fn d2_deleted_key_cancels() {
 let h=fixture(); let (i,id)=row(&h,"Bob-P");
 let samples=json!([s(760.0,i as f64*RH+11.0),s(600.0,i as f64*RH+11.0)]);
 let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout("tracks"),"sample":samples[0]})).unwrap();
 ok(&h,"anim.key.remove",json!({"nodeIds":[id],"frame":48}));
 let before=state(&h);
 let out=h.timing_gesture(Origin::User,json!({"phase":"release","grabId":p["grabId"],"samples":samples})).unwrap();
 assert_eq!(out["error"]["code"],"timeline.targetChanged","E-10: {out}");assert_eq!(state(&h),before);
}

#[test]
fn d2_row_insert_above_bar_commits_by_id() {
 let h=fixture(); let (i,id)=row(&h,"Bob-P");
 let samples=json!([s(152.0,i as f64*RH+16.0),s(200.0,i as f64*RH+16.0)]);
 let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout("tracks"),"sample":samples[0]})).unwrap();
 let created=ok(&h,"element.create",json!({"type":"peg","name":"Inserted"}));
 println!("inserted: {created}");
 let all=rows(&h);let new=all.iter().find(|(_,n)|n=="Inserted").unwrap().0.clone();
 let mut root:Vec<String>=ok(&h,"timeline.rows.list",json!({}))["rows"].as_array().unwrap().iter().filter(|r|r["depth"]==0).map(|r|r["elementId"].as_str().unwrap().to_owned()).filter(|r|r!=&new).collect();root.insert(0,new);
 ok(&h,"element.order.set",json!({"order":"timeline","elementIds":root}));
 let out=h.timing_gesture(Origin::User,json!({"phase":"release","grabId":p["grabId"],"samples":samples})).unwrap();
 assert!(out.get("error").is_none(),"E-10 target {id} still exists: {out}");
 assert_eq!(ok(&h,"timeline.rows.list",json!({}))["rows"].as_array().unwrap().iter().find(|r|r["elementId"]==id).unwrap()["bar"]["start"],4);
}
