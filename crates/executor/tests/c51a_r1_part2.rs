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
