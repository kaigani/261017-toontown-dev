//! Independent C4.2a round 1 review probes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::json;
fn ready() -> tt_executor::ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"", "frame":1}));
    h
}
mod views {
    use super::*;
    #[test]
    fn navigation_is_session_only_and_isolated() {
        let h = ready();
        let r = rev(&h);
        let history = ok(&h, "edit.history.get", json!({}));
        for id in ["drawingView", "cameraView", "topView", "sideView"] {
            ok(&h, "view.zoom.set", json!({"viewId":id,"zoom":2}));
            ok(&h, "view.pan.set", json!({"viewId":id,"centre":{"x":42,"y":-9}}));
            ok(&h, "view.rotation.set", json!({"viewId":id,"degrees":37.5}));
        }
        assert_eq!(rev(&h), r);
        assert_eq!(ok(&h, "edit.history.get", json!({})), history);
        ok(&h, "view.type.set", json!({"viewId":"drawingView","type":"camera"}));
        assert_eq!(ok(&h, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 1.0);
        ok(&h, "view.type.set", json!({"viewId":"drawingView","type":"previous"}));
        assert_eq!(ok(&h, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 2.0);
        let fresh = ready();
        assert_eq!(ok(&fresh, "view.transform.get", json!({"viewId":"drawingView"}))["zoom"], 1.0);
    }
    #[test]
    fn navigation_rejects_invalid_external_values() {
        let h = ready();
        for z in [0.0, -1.0, 0.0624, 64.01] {
            assert_eq!(err(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z})).code, "arg.invalid");
        }
        for p in [json!({"x":1e308,"y":0}), json!({"x":null,"y":0})] {
            assert_eq!(err(&h, "view.pan.set", json!({"viewId":"drawingView","centre":p})).code, "arg.invalid");
        }
        assert_eq!(err(&h, "view.transform.get", json!({"viewId":"missing"})).code, "view.notFound");
        for z in [0.0625, 64.0] {
            ok(&h, "view.zoom.set", json!({"viewId":"drawingView","zoom":z}));
        }
    }
}
