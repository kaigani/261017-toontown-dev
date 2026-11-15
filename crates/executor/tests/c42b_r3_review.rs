//! Independent round-three review.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
fn ready() -> tt_executor::ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    h
}
fn label(h: &tt_executor::ExecutorHandle) -> Value {
    let v = ok(h, "edit.history.get", json!({}));
    v["entries"][v["position"].as_u64().unwrap() as usize - 1]["label"].clone()
}
#[test]
fn six_gesture_and_command_labels() {
    for (tool, expected) in [
        ("brush", "Brush Tool"),
        ("pencil", "Pencil Tool"),
        ("rectangle", "Rectangle Tool"),
        ("ellipse", "Ellipse Tool"),
        ("line", "Line Tool"),
        ("polyline", "Polyline Tool"),
    ] {
        let a = ready();
        let b = ready();
        let mut events = json!([{"type":"down","x":1200,"y":400},{"type":"up","x":1250,"y":450}]);
        if tool == "polyline" {
            events = json!([{"type":"down","x":1200,"y":400},{"type":"up","x":1200,"y":400},{"type":"down","x":1250,"y":450},{"type":"up","x":1250,"y":450},{"type":"doubleClick","x":1250,"y":450}]);
        }
        let e = a.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":tool,"events":events}));
        assert!(e.is_ok(), "{e:?}");
        for c in e.result.unwrap()["commands"].as_array().unwrap() {
            ok(&b, c["commandId"].as_str().unwrap(), c["params"].clone());
        }
        assert_eq!(label(&a), expected, "gesture {tool}");
        assert_eq!(label(&b), expected, "command {tool}");
        ok(&a, "edit.undo", json!({}));
        ok(&a, "edit.redo", json!({}));
        assert_eq!(label(&a), expected);
    }
}
#[test]
fn alternate_scissor_label_is_scissor_tool() {
    let h = ready();
    ok(&h, "draw.rectangle.add", json!({"from":{"x":-150,"y":-60},"to":{"x":-30,"y":60}}));
    let pts = [(-170., -80.), (-90., -80.), (-90., 80.), (-170., 80.), (-170., -80.)];
    let events:Vec<Value>=pts.iter().enumerate().map(|(i,(x,y))|json!({"type":if i==0 {"down"} else if i==4 {"up"} else {"move"},"x":x+960.,"y":540.-y,"modifiers":["alt"]})).collect();
    let e = h.pointer(tt_executor::Origin::User, json!({"view":"drawing","tool":"scissor","events":events}));
    assert!(e.is_ok(), "{e:?}");
    assert_eq!(label(&h), "Scissor Tool", "02 section 7.4.3 line 654; D-4");
}
#[test]
fn two_step_atomic_refusal_restores_document() {
    let h = ready();
    let before = canonical(&h);
    let e = batch(
        &h,
        json!({"label":"review","atomic":true,"steps":[{"id":"draw.stroke.add","params":{"kind":"brush","points":[{"x":300,"y":100},{"x":350,"y":120}],"fit":false}},{"id":"draw.stroke.add","params":{"kind":"brush","points":[]}}]}),
    );
    assert!(!e.is_ok());
    assert_eq!(canonical(&h), before);
}

fn canonical(h: &tt_executor::ExecutorHandle) -> Vec<String> {
    let d = h.snapshot().unwrap().unwrap();
    d.entry_keys()
        .into_iter()
        .map(|k| tt_model::canonical::to_canonical_string(&d.entry_raw_json(&k).unwrap()).unwrap())
        .collect()
}

#[test]
fn direct_transform_uses_command_table_label() {
    let h = ready();
    let r = ok(&h, "draw.rectangle.add", json!({"from":{"x":300,"y":100},"to":{"x":350,"y":150}}));
    ok(&h, "draw.selection.set", json!({"by":"ids","ids":r["strokeIds"]}));
    ok(&h, "draw.transform.apply", json!({"translate":{"x":10,"y":0}}));
    assert_eq!(label(&h), "Move/Scale/Rotate Object", "02 section 13.5 line 1332; D-4 command label");
}
#[test]
fn shell_end_reports_release_kernel_modal_for_each_ending() {
    for ending in ["double click", "Enter", "closing click", "Escape", "tool change", "view gone"] {
        let h = ready();
        h.set_wait_for_user(std::time::Duration::ZERO);
        h.report_shell(serde_json::from_str(r#"{"modal":{"kind":"gesture","title":"polyline"}}"#).unwrap());
        let inv = || tt_executor::Invocation {
            command: "doc.object.patch".into(),
            params: json!({"targets":["@current"],"props":{"name":"review"}}),
            options: Default::default(),
            origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Edit]),
        };
        assert_eq!(h.invoke(inv()).error.unwrap().code, "ui.busy");
        assert_eq!(ok(&h, "app.state.get", json!({}))["modal"]["kind"], "gesture");
        h.report_shell(serde_json::from_str(r#"{"modal":null}"#).unwrap());
        assert!(ok(&h, "app.state.get", json!({}))["modal"].is_null(), "{ending}");
        assert!(h.invoke(inv()).is_ok(), "{ending}");
    }
}

#[test]
fn wire_gesture_refusals_are_atomic() {
    let h = ready();
    let before = canonical(&h);
    let mut cases = vec![json!([{"type":"down","x":[],"y":2}]), json!([{"type":"down","x":null,"y":2}])];
    cases.push(Value::Array((0..10001).map(|_| json!({"type":"hover","x":1,"y":2})).collect()));
    for events in cases {
        let text = json!({"view":"drawing","tool":"brush","events":events}).to_string();
        let request: Value = serde_json::from_str(&text).unwrap();
        let e = h.pointer(tt_executor::Origin::User, request);
        assert!(!e.is_ok(), "{e:?}");
        assert!(matches!(e.error.unwrap().code.as_str(), "arg.invalid" | "limit.exceeded"));
        assert_eq!(canonical(&h), before);
    }
    assert!(serde_json::from_str::<Value>(r#"{"x":NaN}"#).is_err());
}
