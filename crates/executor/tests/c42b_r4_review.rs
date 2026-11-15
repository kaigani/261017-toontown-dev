//! Scoped closure checks at the host boundary.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tt_executor::{AttachedIo, CapturedPng, CommandError, Envelope, Origin};

#[derive(Debug, Default)]
struct Io(AtomicUsize);

#[test]
fn cutter_free_form_and_rectangular_paths_keep_cutter_label() {
    for points in [
        vec![(-170., -80.), (-80., -70.), (-85., 80.), (-170., 80.), (-170., -80.)],
        vec![(-170., -80.), (-90., -80.), (-90., 80.), (-170., 80.), (-170., -80.)],
    ] {
        let h = fixture();
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
        ok(&h, "draw.rectangle.add", json!({"from":{"x":-150,"y":-60},"to":{"x":-30,"y":60}}));
        let before = ok(&h, "edit.history.get", json!({}))["position"].as_u64().unwrap();
        let events: Vec<_> = points.iter().enumerate().map(|(i,(x,y))| json!({"type":if i==0 {"down"} else if i+1==points.len() {"up"} else {"move"},"x":x,"y":y})).collect();
        let reply =
            h.pointer(Origin::User, json!({"view":"drawing","space":"drawing","tool":"cutter","events":events}));
        assert!(reply.is_ok(), "{reply:?}");
        let history = ok(&h, "edit.history.get", json!({}));
        let position = history["position"].as_u64().unwrap();
        assert_eq!(position, before + 1);
        assert_eq!(history["entries"][position as usize - 1]["label"], "Cutter Tool");
    }
}

#[test]
fn user_window_and_agent_headless_have_identical_commands_and_canonical_content() {
    let a = fixture();
    let b = fixture();
    for h in [&a, &b] {
        ok(h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    }
    let request = json!({"view":"drawing","tool":"rectangle","events":[{"type":"down","x":1200,"y":400},{"type":"up","x":1250,"y":450}]});
    let user = a.pointer_in_window(Origin::User, request.clone());
    let simulated = b.pointer(agent(&tt_registry::Scope::ALL), request);
    assert!(user.is_ok() && simulated.is_ok());
    assert_eq!(user.result.unwrap()["commands"], simulated.result.unwrap()["commands"]);
    let canonical = |h: &tt_executor::ExecutorHandle| {
        let doc = h.snapshot().unwrap().unwrap();
        doc.entry_keys()
            .into_iter()
            .map(|key| tt_model::canonical::to_canonical_string(&doc.entry_raw_json(&key).unwrap()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(canonical(&a), canonical(&b));
}
impl AttachedIo for Io {
    fn route_pointer(&self, _: &str, _: &Origin, _: &Value, _: Duration) -> Option<Envelope> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Envelope::ok(json!({"routed":true}), None))
    }
    fn capture(&self, _: &str, _: Option<[f64; 4]>, _: u32, _: Duration) -> Result<CapturedPng, CommandError> {
        Err(CommandError::new("view.notFound", "test has no window"))
    }
}

#[test]
fn user_commit_bypasses_priority_and_agent_tool_request_waits_before_routing() {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    let io = Arc::new(Io::default());
    h.set_attached_io(Some(io.clone()));
    h.set_wait_for_user(Duration::from_millis(80));
    h.report_shell(serde_json::from_value(json!({"modal":{"kind":"gesture","title":"drawing"}})).unwrap());
    let request = json!({"view":"drawing","tool":"rectangle","events":[{"type":"down","x":1200,"y":400},{"type":"up","x":1250,"y":450}]});
    assert!(h.pointer_in_window(Origin::User, request.clone()).is_ok());
    assert_eq!(io.0.load(Ordering::SeqCst), 0);
    let before = ok(&h, "tool.active.get", json!({}));
    let start = Instant::now();
    let mut agent_request = request.clone();
    agent_request["tool"] = json!("pencil");
    let reply = h.pointer(agent(&tt_registry::Scope::ALL), agent_request);
    assert_eq!(reply.error.unwrap().code, "ui.busy");
    assert!(start.elapsed() >= Duration::from_millis(70));
    assert_eq!(io.0.load(Ordering::SeqCst), 0);
    assert_eq!(ok(&h, "tool.active.get", json!({})), before);
    h.report_shell(serde_json::from_value(json!({"modal":null})).unwrap());
    assert!(h.pointer(agent(&tt_registry::Scope::ALL), request).is_ok());
    assert_eq!(io.0.load(Ordering::SeqCst), 1);
}
