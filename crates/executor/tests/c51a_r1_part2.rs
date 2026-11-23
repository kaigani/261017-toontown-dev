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
