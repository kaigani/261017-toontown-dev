//! M4 C4.2 builder tests of `tt-mcp`, one test binary for the checkpoint (orchestrator ruling D-2): the `pointer` tool and
//! `key` with `view` through the real protocol and serialisation (plan §5 "wire level"). Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod support;

use serde_json::{Value, json};
use support::*;

/// A connected agent with a new project holding one drawing element, current at frame 1.
fn ready() -> (std::path::PathBuf, tt_executor::ExecutorHandle, Client) {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    let (mut c, _) = Client::connect(&h, opts(&dir), json!({}));
    let env = c.run("project.create", json!({ "name": "P", "path": path(&dir.join("P.animpkg")) }));
    assert_eq!(env["status"], "ok", "{env}");
    let e = c.run("element.create", json!({ "type": "drawing" }));
    let id = e["result"]["elementIds"][0].as_str().unwrap().to_owned();
    assert_eq!(c.run("app.context.set", json!({ "elementId": id, "frame": 1 }))["status"], "ok");
    (dir, h, c)
}

#[test]
fn pointer_answers_the_commands_its_gestures_produced() {
    let (dir, _h, mut c) = ready();
    let r = c.call(
        "pointer",
        json!({ "view": "drawing", "tool": "brush", "events": [
            { "type": "down", "x": 1000, "y": 500, "pressure": 0.5 },
            { "type": "move", "x": 1010, "y": 490, "pressure": null },
            { "type": "up", "x": 1020, "y": 480 } ] }),
    );
    let sc = &r["structuredContent"];
    assert_eq!(sc["status"], "ok", "{r}");
    let commands = sc["result"]["commands"].as_array().unwrap();
    assert_eq!(commands[0]["commandId"], "tool.active.set");
    assert_eq!(commands[1]["commandId"], "draw.stroke.add");
    let pts = commands[1]["params"]["points"].as_array().unwrap();
    let pressures: Vec<Value> = pts.iter().map(|p| p["pressure"].clone()).collect();
    assert_eq!(pressures, vec![json!(0.5), json!(1.0), json!(1.0)], "absent and null pressure are the default 1");
    assert_eq!(pts[0], json!({ "x": 40.0, "y": 40.0, "pressure": 0.5 }));
    assert!(commands[1]["result"]["strokeIds"].as_array().is_some_and(|s| !s.is_empty()), "{r}");
    assert_eq!(sc["result"]["hover"]["drawing"], json!({ "x": 60.0, "y": 60.0 }));
    // The activity log names the command the call produced.
    let undo = c.call("history", json!({}));
    let labels: Vec<Value> =
        undo["structuredContent"]["result"]["entries"].as_array().unwrap().iter().map(|e| e["label"].clone()).collect();
    assert!(labels.contains(&json!("Brush Tool")), "{labels:?}");
    // A refusal is the tool's error, with the gesture it came from.
    let r = c.call("pointer", json!({ "view": "drawing", "tool": "text", "events": [
        { "type": "down", "x": 1, "y": 1 }, { "type": "up", "x": 1, "y": 1 } ] }));
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["error"]["details"]["gestureCommandId"], "tool:text");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn key_with_a_view_aims_view_commands_at_it_without_moving_the_active_view() {
    let (dir, _h, mut c) = ready();
    let active = c.run("app.state.get", json!({}))["result"]["activeView"]["viewId"].clone();
    let r = c.call("key", json!({ "keys": "primary+=", "view": "drawing" }));
    assert_eq!(r["structuredContent"]["status"], "ok", "{r}");
    let zoom = |c: &mut Client, v: &str| c.run("view.transform.get", json!({ "viewId": v }))["result"]["zoom"].clone();
    assert_eq!(zoom(&mut c, "drawingView"), 1.5);
    if active != "drawingView" {
        assert_eq!(zoom(&mut c, active.as_str().unwrap()), 1.0, "the active view did not move");
    }
    assert_eq!(c.run("app.state.get", json!({}))["result"]["activeView"]["viewId"], active);
    let r = c.call("key", json!({ "keys": "primary+=", "view": "nowhere" }));
    assert_eq!(r["structuredContent"]["error"]["code"], "view.notFound", "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}
