#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value,json};
const BOB:&str="scene:\"Intro\"/element:\"Bob\"";
const PEG:&str="scene:\"Intro\"/element:\"Bob-P\"";
#[test]
fn f1_drawing_step_cross_product() {
    let h=fixture();
    let n=ok(&h,"timeline.rows.list",json!({}))["sceneLength"].as_i64().unwrap();
    ok(&h,"timeline.playbackRange.set",json!({"start":4,"end":n-4}));
    for cmd in ["timeline.currentFrame.move","app.frame.move"] {
        for direction in ["next","previous"] {
            for count in [1,2,1000] {
                for within in [false,true] {
                    for at in [1,2,4,8,n-4,n-1,n] {
                        ok(&h,"xsheet.currentCell.set",json!({"elementId":BOB,"frame":at}));
                        let mut p=json!({"direction":direction,"unit":"drawing","count":count,"withinRange":within});
                        if cmd.starts_with("timeline.") { p["elementId"]=json!(BOB); }
                        else {
                            let unsupported=run(&h,cmd,p.clone());
                            assert!(unsupported.error.is_some_and(|e|e.code=="arg.unknown"));
                            p.as_object_mut().unwrap().remove("withinRange");
                        }
                        let r=ok(&h,cmd,p);
                        let (lo,hi)=if within && cmd.starts_with("timeline.") {(4,n-4)} else {(1,n)};
                        if (direction=="previous" && at<=lo) || (direction=="next" && at>=hi) {
                            assert_eq!(r["frame"],at,"09:135 {cmd} {direction} {count} {within}");
                        }
                        ok(&h,"timeline.rows.list",json!({}));
                    }
                }
            }
        }
    }
}
#[test]
fn f1_window_and_label_boundaries() {
    let h=fixture();
    for cmd in ["timeline.rows.list","xsheet.exposures.get"] {
        for p in [json!({"from":20,"to":3}),json!({"from":3,"to":3}),
            json!({"from":10000,"to":10001}),json!({"from":i64::MAX,"to":i64::MAX}),
            json!({"from":u64::MAX,"to":u64::MAX}),json!({"from":0,"to":0})] {
            let r=run(&h,cmd,p);
            assert!(r.is_ok()||r.error.as_ref().is_some_and(|e|e.code!="internal"),"{r:?}");
            ok(&h,"timeline.rows.list",json!({}));
        }
    }
    for frames in [json!([]),json!((1..=10000).collect::<Vec<_>>()),json!([i64::MAX,u64::MAX])] {
        let r=run(&h,"timeline.frameLabel.get",json!({"frames":frames}));
        assert!(r.is_ok()||r.error.as_ref().is_some_and(|e|e.code!="internal"),"{r:?}");
        ok(&h,"timeline.rows.list",json!({}));
    }
}
fn row(h:&tt_executor::ExecutorHandle,name:&str)->usize {
    ok(h,"timeline.rows.list",json!({}))["rows"].as_array().unwrap().iter()
        .filter(|r|r["shownInTimeline"]==true).position(|r|r["name"]==name).unwrap()
}
fn layout(surface:&str)->Value { json!({"surface":surface,"zoomLevel":6,"firstFrame":1,"scrollTop":0,"platform":"windows"}) }
fn sample(r:usize,x:f64,y:f64)->Value {json!({"x":x,"y":r as f64*22.0+y,"modifiers":[]})}
