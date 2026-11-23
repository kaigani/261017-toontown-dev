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

#[test]
fn f2_pixel_zone_matrix() {
    let h=fixture();
    let id=ok(&h,"element.create",json!({"type":"peg","name":"PixelProbe"}))["elementIds"][0].clone();
    ok(&h,"element.timing.set",json!({"elementIds":[id],"start":5,"duration":20}));
    let r=row(&h,"PixelProbe");
    // Kernel note 5.1 widths; no glyph overlaps on this new peg.
    // End has an observed extra pixel (6 vs barHandleWidth 5), pinned as O3.
    for (x,y,zone) in [
        (58.,16.,"cell"),(59.,16.,"start"),(69.,16.,"start"),(70.,16.,"body"),
        (377.,16.,"body"),(378.,16.,"end"),(379.,16.,"end"),(389.,16.,"end"),(390.,16.,"end"),(391.,16.,"cell"),
        (377.,5.,"body"),(378.,5.,"loop"),(390.,5.,"loop"),(391.,5.,"cell"),
    ] {
        let out=h.timing_replay(tt_executor::Origin::User,layout("tracks"),
            json!([sample(r,x,y),sample(r,x+384.,y)]),false).unwrap();
        let cmd=&out["plan"]["commands"][0];
        let params=&cmd[1];
        match zone {
            "start"=>assert_eq!(params["anchor"],"end","x={x} {out}"),
            "end"=>assert_eq!(params["anchor"],"start","x={x} {out}"),
            "body"=>assert!(params.get("startDelta").is_some(),"x={x} {out}"),
            "loop"=>assert!(params.get("loops").is_some(),"x={x} {out}"),
            "cell"=>assert_ne!(cmd[0],"element.timing.set","x={x} {out}"),
            _=>unreachable!(),
        }
    }
    ok(&h,"anim.key.add",json!({"nodeIds":[id],"frame":12}));
    for (x,key) in [(178.,false),(179.,true),(189.,true),(190.,false)] {
        let out=h.timing_replay(tt_executor::Origin::User,layout("tracks"),
            json!([sample(r,x,11.),sample(r,x+64.,11.)]),false).unwrap();
        assert_eq!(out["plan"]["commands"][0][0]=="anim.key.move",key,"x={x} {out}");
    }
    // Deliberate key/handle overlap: design note has no priority ruling.
    ok(&h,"anim.key.add",json!({"nodeIds":[id],"frame":5}));
    let out=h.timing_replay(tt_executor::Origin::User,layout("tracks"),
        json!([sample(r,69.,11.),sample(r,133.,11.)]),false).unwrap();
    eprintln!("O3 overlap observation: {out}");
}
