//! Drawing-event subscription through actual MCP transport.
#![allow(clippy::unwrap_used, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
use std::time::Duration;
use support::*;

#[test]
fn drawing_events_reach_mcp_event_resource_subscribers() {
    let dir = tmp();
    let h = executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let mut options = opts(&dir);
    options.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (mut c, _) = Client::connect(&h, options, json!({}));
    c.request("resources/subscribe", json!({"uri":"app://events"}));
    let r = c.run("draw.pivot.set", json!({"drawingId":"dr_01K742SG0KKMCAP9X1E6Z5P4SW","point":{"x":71,"y":28}}));
    assert_eq!(r["status"], "ok", "{r}");
    c.drain(Duration::from_millis(300));
    assert!(c.notes_of("notifications/resources/updated").iter().any(|n| n["params"]["uri"] == "app://events"));
    let resource = c.request("resources/read", json!({"uri":"app://events?after=0"}));
    let body: Value = serde_json::from_str(resource["contents"][0]["text"].as_str().unwrap()).unwrap();
    let events: Vec<_> =
        body["events"].as_array().unwrap().iter().filter(|e| e["type"] == "draw.drawing.changed").collect();
    assert_eq!(events.len(), 1, "{body}");
    let p = &events[0]["payload"];
    assert_eq!(p["drawingId"], r["result"]["drawingId"]);
    assert_eq!(p["rev"], r["result"]["rev"]);
    assert_eq!(p["commandId"], "draw.pivot.set");
    c.close().unwrap();
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
