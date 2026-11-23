//! M5 C5.1a round 1 (builder; the observation on host notices): `shell.playbackFrame` and `shell.timelineView`, the
//! window's host-only notices (SI-207, SI-210, D-57), are emitted on the executor's stream during real playback and a
//! view change, and never arrive on a real MCP stream: not in `app://events`, not as a notification of any kind.
#![allow(clippy::unwrap_used, missing_docs, reason = "test")]
mod support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::*;

#[test]
fn playback_and_view_notices_are_emitted_but_never_reach_an_mcp_client() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    c.request("resources/subscribe", json!({"uri": "app://events"}));
    c.request("logging/setLevel", json!({"level": "debug"}));
    let rx = h.subscribe();
    assert_eq!(c.run("playback.loop.set", json!({"enabled": true}))["status"], "ok");
    assert_eq!(c.run("playback.start", json!({}))["status"], "ok");
    // Wait (bounded) until the clock has presented at least two frames.
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let s = c.run("playback.state.get", json!({}));
        if s["result"]["frame"].as_u64().unwrap_or(1) > 2 {
            break;
        }
        assert!(Instant::now() < end, "playback did not advance: {s}");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(c.run("timeline.view.set", json!({"zoomLevel": 8, "firstFrame": 5}))["status"], "ok");
    assert_eq!(c.run("playback.stop", json!({}))["status"], "ok");
    c.drain(Duration::from_millis(400));
    // Emitted on the executor's stream (the window's) ...
    let names: Vec<String> = rx.try_iter().map(|e| e.name).collect();
    for n in ["shell.playbackFrame", "shell.timelineView"] {
        assert!(names.iter().any(|x| x == n), "{n} not emitted: {names:?}");
    }
    // ... and absent from everything the client received.
    let resource = c.request("resources/read", json!({"uri": "app://events?after=0"}));
    let body: Value = serde_json::from_str(resource["contents"][0]["text"].as_str().unwrap()).unwrap();
    let types: Vec<&str> = body["events"].as_array().unwrap().iter().filter_map(|e| e["type"].as_str()).collect();
    assert!(types.contains(&"playback.started") && types.contains(&"playback.stopped"), "{types:?}");
    assert!(types.iter().all(|t| !t.starts_with("shell.")), "{types:?}");
    let everything = serde_json::to_string(&c.notes).unwrap();
    for n in ["shell.playbackFrame", "shell.timelineView"] {
        assert!(!everything.contains(n), "{n} reached the client: {everything}");
    }
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
