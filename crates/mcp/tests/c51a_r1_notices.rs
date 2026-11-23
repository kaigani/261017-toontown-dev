#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use support::*;
use serde_json::{Value,json};
use std::time::{Duration,Instant};
#[test]
fn c3_playback_notices_are_isolated() {
 let dir=tmp(); let h=executor(&dir.join("user"));
 assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(),None,false).is_ok());
 let mut o=opts(&dir); o.policy=Some(tt_executor::agents::Policy::Autonomous);
 let (mut c,_)=Client::connect(&h,o,json!({}));
 c.request("resources/subscribe",json!({"uri":"app://events"}));
 c.request("logging/setLevel",json!({"level":"debug"}));
 for (id,p) in [("playback.loop.set",json!({"enabled":true})),("playback.start",json!({}))] {assert_eq!(c.run(id,p)["status"],"ok");}
 let end=Instant::now()+Duration::from_secs(60);
 loop {let s=c.run("playback.state.get",json!({})); if s["result"]["frame"].as_u64().unwrap_or(1)>1 {break;} assert!(Instant::now()<end,"did not advance: {s}"); std::thread::sleep(Duration::from_millis(20));}
 assert_eq!(c.run("timeline.view.set",json!({"zoomLevel":7,"firstFrame":2}))["status"],"ok");
 assert_eq!(c.run("playback.stop",json!({}))["status"],"ok");
 c.drain(Duration::from_millis(400));
 let r=c.request("resources/read",json!({"uri":"app://events?after=0"}));
 let body:Value=serde_json::from_str(r["contents"][0]["text"].as_str().unwrap()).unwrap();
 let names:Vec<&str>=body["events"].as_array().unwrap().iter().filter_map(|e|e["type"].as_str()).collect();
 for n in ["playback.started","playback.stopped","timeline.currentFrame.changed"] {assert!(names.contains(&n),"{names:?}");}
 assert!(names.iter().all(|n|!n.starts_with("shell.")),"{names:?}");
 let notes=serde_json::to_string(&c.notes).unwrap(); assert!(!notes.contains("shell."),"{notes}");
 c.close().unwrap(); drop(h); std::fs::remove_dir_all(dir).unwrap();
}
