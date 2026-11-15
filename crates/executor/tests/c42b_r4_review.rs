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
