//! Attached mode's routes (M4 C4.2b; design note 2 §6, design note 5 §2, D-6): an agent's `pointer` call for a shown
//! view goes to the window (P1, P2), the user's never does, and a view no window shows keeps the headless path; an
//! attached `inspect.screenshot` is the host's capture, in the headless answer's shape (S3), with the caps (S2).
//! The host's own half (the window's queue and the platform captures) is checked in the running application.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tt_executor::{AttachedIo, CapturedPng, CommandError, Envelope, Origin};

use crate::common::*;
use crate::equivalence::{drag, ready};

#[derive(Debug, Default)]
struct FakeIo {
    /// Views the fake "window" shows.
    shown: Vec<String>,
    routed: Mutex<Vec<(String, Value)>>,
    captures: Mutex<Vec<(String, Option<[f64; 4]>)>>,
    png_side: u32,
    fail: Option<String>,
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let px = vec![200u8; (w * h * 4) as usize];
    tt_render::encode_png(&px, w, h).unwrap()
}

impl AttachedIo for FakeIo {
    fn route_pointer(&self, view: &str, _origin: &Origin, request: &Value, _budget: Duration) -> Option<Envelope> {
        if !self.shown.iter().any(|v| v == view) {
            return None;
        }
        self.routed.lock().unwrap().push((view.to_owned(), request.clone()));
        Some(Envelope::ok(json!({ "routed": view }), None))
    }

    fn capture(
        &self,
        target: &str,
        region: Option<[f64; 4]>,
        max_side: u32,
        _b: Duration,
    ) -> Result<CapturedPng, CommandError> {
        self.captures.lock().unwrap().push((target.to_owned(), region));
        if let Some(code) = &self.fail {
            return Err(CommandError::new(code.clone(), "refused by the fake host"));
        }
        if self.png_side > max_side {
            return Err(CommandError::new("render.inlineTooLarge", "over the side"));
        }
        let side = self.png_side.max(1);
        Ok(CapturedPng { png: png(side, side), width: side, height: side, view_id: "drawingView".into() })
    }
}

fn agent() -> Origin {
    Origin::Agent { session: "s1".into(), name: "test".into(), scopes: tt_registry::Scope::ALL.into_iter().collect() }
}

#[test]
fn p1_an_agents_gesture_on_a_shown_view_is_routed_to_the_window_and_the_users_never_is() {
    let h = ready();
    let io = Arc::new(FakeIo { shown: vec!["drawingView".into()], ..FakeIo::default() });
    h.set_attached_io(Some(io.clone()));
    let req = json!({ "view": "drawing", "tool": "brush", "events": drag(&[(700.0, 300.0), (760.0, 320.0)]) });
    let before = ok(&h, "edit.history.get", json!({}))["entries"].as_array().unwrap().len();
    let env = h.pointer(agent(), req.clone());
    assert_eq!(env.result, Some(json!({ "routed": "drawingView" })), "{env:?}");
    assert_eq!(io.routed.lock().unwrap().len(), 1, "routed once, with the request as given");
    assert_eq!(io.routed.lock().unwrap()[0].1, req);
    assert_eq!(
        ok(&h, "edit.history.get", json!({}))["entries"].as_array().unwrap().len(),
        before,
        "nothing ran headless"
    );
    // The user's own gestures and a window's commit run here, never routed.
    let env = h.pointer(Origin::User, req.clone());
    assert!(env.is_ok() && env.result.as_ref().is_some_and(|r| r.get("commands").is_some()), "{env:?}");
    let env = h.pointer_in_window(agent(), req);
    assert!(env.is_ok() && env.result.as_ref().is_some_and(|r| r.get("commands").is_some()), "{env:?}");
    assert_eq!(io.routed.lock().unwrap().len(), 1);
}

#[test]
fn a_view_no_window_shows_keeps_the_headless_path() {
    let h = ready();
    let io = Arc::new(FakeIo::default());
    h.set_attached_io(Some(io.clone()));
    let env = h.pointer(
        agent(),
        json!({ "view": "drawing", "tool": "brush", "events": drag(&[(700.0, 300.0), (760.0, 320.0)]) }),
    );
    assert!(env.is_ok() && env.result.as_ref().is_some_and(|r| r.get("commands").is_some()), "{env:?}");
    assert!(io.routed.lock().unwrap().is_empty());
}

#[test]
fn s3_an_attached_screenshot_is_the_hosts_capture_in_the_headless_shape() {
    let h = ready();
    let headless = ok(&h, "inspect.screenshot", json!({}));
    let io = Arc::new(FakeIo { png_side: 40, ..FakeIo::default() });
    h.set_attached_io(Some(io.clone()));
    let r = ok(
        &h,
        "inspect.screenshot",
        json!({ "target": "drawingView", "region": { "x": 1, "y": 2, "w": 30, "h": 40 } }),
    );
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
    let mut headless_keys = keys(&headless);
    headless_keys.remove("jobId");
    assert_eq!(keys(&r), headless_keys, "the same fields as the headless answer");
    assert_eq!((r["source"].clone(), r["width"].clone(), r["height"].clone()), (json!("gui"), json!(40), json!(40)));
    let bytes = tt_persist::b64::decode(r["png"].as_str().unwrap()).unwrap();
    assert_eq!(r["sha256"], tt_persist::fsx::sha256(&bytes), "S1: the sha256 is the PNG's");
    assert_eq!(io.captures.lock().unwrap()[0], ("drawingView".to_owned(), Some([1.0, 2.0, 30.0, 40.0])));
    // Malformed parameters refuse before any capture; the host's refusals pass through.
    assert_eq!(
        err(&h, "inspect.screenshot", json!({ "region": { "x": 0, "y": 0, "w": 0, "h": 5 } })).code,
        "arg.invalid"
    );
    assert_eq!(err(&h, "inspect.screenshot", json!({ "target": 5 })).code, "arg.invalid");
    assert_eq!(io.captures.lock().unwrap().len(), 1);
}

#[test]
fn s2_the_attached_caps_hold_before_and_after_the_capture() {
    let h = ready();
    h.set_attached_io(Some(Arc::new(FakeIo { png_side: 4096, ..FakeIo::default() })));
    assert_eq!(err(&h, "inspect.screenshot", json!({})).code, "render.inlineTooLarge");
    h.set_attached_io(Some(Arc::new(FakeIo { png_side: 8, fail: Some("view.notFound".into()), ..FakeIo::default() })));
    assert_eq!(err(&h, "inspect.screenshot", json!({ "target": "nowhere" })).code, "view.notFound");
    h.set_attached_io(None);
    assert_eq!(ok(&h, "inspect.screenshot", json!({}))["source"], "virtual", "without the host: the virtual view");
}
