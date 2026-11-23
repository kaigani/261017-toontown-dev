#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{json,Value};
use tt_executor::Origin;
use std::time::{Duration,Instant};
const PEG:&str="scene:\"Intro\"/element:\"Bob-P\"";
fn layout()->Value {json!({"surface":"tracks","zoomLevel":6,"firstFrame":1,"scrollTop":0,"platform":"windows"})}
fn samples(h:&tt_executor::ExecutorHandle)->Value {
 let rows=ok(h,"timeline.rows.list",json!({}));
 let i=rows["rows"].as_array().unwrap().iter().position(|r|r["name"]=="Bob-P").unwrap();
 json!([{"x":152,"y":i*22+16,"modifiers":[]},{"x":200,"y":i*22+16,"modifiers":[]}])
}
#[test]
fn b1_loop_reads_navigation_and_stop() {
 let h=fixture();
 ok(&h,"timeline.playbackRange.set",json!({"start":2,"end":4}));
 ok(&h,"playback.rate.set",json!({"fps":120}));
 ok(&h,"playback.loop.set",json!({"enabled":true}));
 ok(&h,"playback.start",json!({}));
 assert_eq!(ok(&h,"playback.start",json!({}))["playing"],true);
 for _ in 0..200 {assert_eq!(ok(&h,"playback.state.get",json!({}))["playing"],true);}
 let guard=Instant::now()+Duration::from_secs(60);
 let mut prev=0; let mut wrapped=false;
 while Instant::now()<guard {
  let f=ok(&h,"playback.state.get",json!({}))["frame"].as_u64().unwrap();
  if f<prev {wrapped=true;break;} prev=f;
  std::thread::sleep(Duration::from_millis(2));
 }
 assert!(wrapped,"09:115 loop wraps");
 ok(&h,"playback.forceRate.set",json!({"enabled":true}));
 let state=ok(&h,"playback.state.get",json!({}));
 assert_eq!(state["forceRate"],true); assert_eq!(state["rate"]["fps"],120.0);
 ok(&h,"timeline.currentFrame.move",json!({"direction":"first"}));
 assert_eq!(ok(&h,"playback.state.get",json!({}))["playing"],false,"09:138");
 ok(&h,"playback.stop",json!({})); ok(&h,"playback.stop",json!({}));
}
#[test]
fn b2_final_playback_event() {
 let h=fixture();
 ok(&h,"timeline.playbackRange.set",json!({"start":2,"end":30}));
 ok(&h,"playback.rate.set",json!({"fps":120}));
 let rx=h.subscribe(); ok(&h,"playback.start",json!({}));
 let guard=Instant::now()+Duration::from_secs(60); let mut frames=Vec::new();
 loop {
  let left=guard.saturating_duration_since(Instant::now()); assert!(!left.is_zero());
  let e=rx.recv_timeout(left).unwrap();
  if e.name=="timeline.currentFrame.changed" && e.payload["source"]=="playback" {frames.push(e.payload["frame"].clone());}
  if e.name=="playback.stopped" {assert_eq!(e.payload["frame"],30);break;}
 }
 assert_eq!(frames.last(),Some(&json!(30)),"04:1466 final frame");
}
#[test]
fn b3_abort_and_bar_replay_undo() {
 let h=fixture(); let s=samples(&h); let before=ok(&h,"timeline.rows.list",json!({}));
 let hist=ok(&h,"edit.history.get",json!({}));
 let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout(),"sample":s[0]})).unwrap();
 let rx=h.subscribe();
 h.timing_gesture(Origin::User,json!({"phase":"abort","grabId":p["grabId"]})).unwrap();
 assert_eq!(ok(&h,"edit.history.get",json!({})),hist);
 assert!(!rx.try_iter().any(|e|e.name=="document.changed"));
 let d=fixture(); ok(&d,"element.timing.set",json!({"elementIds":[PEG],"startDelta":3}));
 h.timing_replay(Origin::User,layout(),s,true).unwrap();
 assert_eq!(ok(&h,"timeline.rows.list",json!({})),ok(&d,"timeline.rows.list",json!({})),"E-7 replay/direct");
 ok(&h,"edit.undo",json!({})); assert_eq!(ok(&h,"timeline.rows.list",json!({})),before);
}
#[test]
fn b4_target_changes_between_press_release() {
 for change in ["rename","move","selection","delete"] {
  let h=fixture(); let s=samples(&h);
  let p=h.timing_gesture(Origin::User,json!({"phase":"press","layout":layout(),"sample":s[0]})).unwrap();
  match change {
   "rename"=>{ok(&h,"element.rename",json!({"elementId":"scene:\"Intro\"/element:\"Bob\"","name":"Renamed"}));},
   "move"=>{ok(&h,"element.timing.set",json!({"elementIds":[PEG],"startDelta":30}));},
   "selection"=>{ok(&h,"element.select",json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"mode":"replace"}));},
   _=>{ok(&h,"element.delete",json!({"elementIds":[PEG]}));}
  }
  let before=rev(&h);
  let out=h.timing_gesture(Origin::User,json!({"phase":"release","grabId":p["grabId"],"samples":s})).unwrap();
  if change=="rename" || change=="selection" {assert!(out.get("cancelled").is_none(),"E-10 {change}: {out}");}
  else {assert_eq!(out["error"]["code"],"timeline.targetChanged","{change}: {out}");assert_eq!(rev(&h),before);}
 }
}

#[test]
fn b5_seeded_timing_save_and_history_roundtrip() {
 let h=fixture();
 ok(&h,"element.clone",json!({"elementId":"scene:\"Intro\"/element:\"Bob\"","name":"ReviewClone"}));
 let rows0=ok(&h,"timeline.rows.list",json!({}));
 let sheet0=ok(&h,"xsheet.exposures.get",json!({}));
 let hist0=ok(&h,"edit.history.get",json!({}))["position"].as_u64().unwrap();
 let mut seed=0x51a_u64;
 for i in 0..300 {
  seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
  match seed%3 {
   0=>{ok(&h,"element.timing.set",json!({"elementIds":[PEG],"loops":1+(seed>>32)%4}));},
   1=>{ok(&h,"element.timing.set",json!({"elementIds":[PEG],"duration":24+(seed>>32)%40,"anchor":"start","keyframes":"scale"}));},
   _=>{
    let rows=ok(&h,"timeline.rows.list",json!({}));
    let all=rows["rows"].as_array().unwrap();let ri=all.iter().position(|r|r["name"]=="Bob-P").unwrap();
    let start=all[ri]["bar"]["start"].as_u64().unwrap();
    let x=(start+8) as f64*16.0;
    let s=json!([{"x":x,"y":ri*22+16,"modifiers":[]},{"x":x+16.0,"y":ri*22+16,"modifiers":[]}]);
    let out=h.timing_replay(Origin::User,layout(),s,true).unwrap();
    assert!(out.get("error").is_none(),"step {i}: {out}");
   }
  }
 }
 let rows=ok(&h,"timeline.rows.list",json!({}));let sheet=ok(&h,"xsheet.exposures.get",json!({}));
 let hist=ok(&h,"edit.history.get",json!({}))["position"].as_u64().unwrap();
 for _ in hist0..hist {ok(&h,"edit.undo",json!({}));}
 assert_eq!(ok(&h,"timeline.rows.list",json!({})),rows0,"04:1309 undo all");
 assert_eq!(ok(&h,"xsheet.exposures.get",json!({})),sheet0);
 for _ in hist0..hist {ok(&h,"edit.redo",json!({}));}
 assert_eq!(ok(&h,"timeline.rows.list",json!({})),rows);
 assert_eq!(ok(&h,"xsheet.exposures.get",json!({})),sheet);
 let dir=std::env::temp_dir().join(format!("tt-review-{}",tt_executor::random_token(8)));std::fs::create_dir_all(&dir).unwrap();
 let path=dir.join("Timing.animpkg");
 let j=ok(&h,"project.saveAs",json!({"path":path.to_string_lossy()}))["jobId"].clone();
 ok(&h,"job.wait",json!({"jobId":j,"timeoutMs":60000}));
 ok(&h,"project.close",json!({"onUnsaved":"discard"}));
 let j=ok(&h,"project.open",json!({"path":path.to_string_lossy(),"onUnsaved":"discard"}))["jobId"].clone();
 ok(&h,"job.wait",json!({"jobId":j,"timeoutMs":60000}));
 assert_eq!(ok(&h,"timeline.rows.list",json!({})),rows);
 assert_eq!(ok(&h,"xsheet.exposures.get",json!({})),sheet);
 drop(h);std::fs::remove_dir_all(dir).unwrap();
}
