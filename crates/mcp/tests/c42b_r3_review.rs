//! Round-three independent wire review.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod support;
use serde_json::{Value, json};
use support::*;
#[test]
fn gesture_scope_on_both_mcp_surfaces() {
    for input in [false, true] {
        let dir = tmp();
        let h = executor(&dir.join("user"));
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let record = json!({"origin":"user","tool":"brush","view":"drawingView","events":[{"pressure":0.4,"tiltX":3,"dt":12,"modifiers":["shift"]}],"commands":[{"commandId":"draw.stroke.add","params":{"points":[{"pressure":0.4}]}}]});
        h.report_shell(tt_executor::host::ShellReport { attached: Some(true), ..Default::default() });
        h.report_shell(tt_executor::host::ShellReport{ui_tree:Some(json!({"id":"main","role":"window","children":[{"id":"drawingView.canvas","role":"canvas","value":json!({"backend":"webgl2","lastGesture":record}).to_string()}]})),rendered_seq:Some(h.events_broadcast()),..Default::default()});
        let mut o = opts(&dir);
        let mut scopes = vec![tt_registry::Scope::Read];
        if input {
            scopes.push(tt_registry::Scope::Input);
        }
        o.scopes = Some(scopes.into_iter().collect());
        let (mut c, _) = Client::connect(&h, o, json!({}));
        for (tool, args) in [
            ("inspect_ui", json!({"target":"main"})),
            ("run_command", json!({"id":"inspect.ui.tree","params":{"target":"main"}})),
        ] {
            let r = c.call(tool, args);
            let s = &r["structuredContent"];
            assert_eq!(s["status"], "ok", "{r}");
            let text = s.to_string();
            for word in ["pressure", "tiltX", "modifiers", "dt"] {
                assert_eq!(text.contains(word), input, "{tool}: {text}");
            }
            let v: Value = serde_json::from_str(s["result"]["root"]["children"][0]["value"].as_str().unwrap()).unwrap();
            assert_eq!(v["backend"], "webgl2");
            assert_eq!(v["lastGesture"]["commands"][0]["commandId"], "draw.stroke.add");
        }
        drop(c);
        drop(h);
    }
}
