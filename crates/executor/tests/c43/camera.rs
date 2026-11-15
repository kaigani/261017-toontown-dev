//! The Camera view's composition (camera-view note §2-§4: CV1-CV4; M3 merge note §8 item 3, camera half).

use serde_json::{Value, json};

use crate::common::*;

pub const BOB: &str = "scene:#1/element:\"Bob\"";

#[test]
fn probe_the_camera_scene() {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    let list = ok(&h, "view.list", json!({}));
    eprintln!("{list:#}");
    let s = h.canvas_scene("cameraView");
    eprintln!("{:#}", serde_json::to_value(&s.as_ref().map_err(|e| e.message.clone())).unwrap());
    let _ = Value::Null;
}
