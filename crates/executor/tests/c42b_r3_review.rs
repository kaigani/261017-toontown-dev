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
#[ignore = "orchestrator ruling D-20: draw.transform.apply's table label names the operation performed"]
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

#[test]
fn window_and_headless_commit_match_and_agent_keeps_hover() {
    let a = ready();
    let b = ready();
    let q = json!({"view":"drawing","tool":"rectangle","events":[{"type":"down","x":1200,"y":400},{"type":"up","x":1250,"y":450}]});
    let wa = a.pointer_in_window(tt_executor::Origin::User, q.clone());
    let hb = b.pointer(tt_executor::Origin::User, q);
    assert!(wa.is_ok() && hb.is_ok());
    let wr = wa.result.unwrap();
    let hr = hb.result.unwrap();
    assert_eq!(wr["commands"], hr["commands"]);
    assert_eq!(canonical(&a), canonical(&b));
    assert!(wr["hover"].get("objects").is_none());
    assert!(hr["hover"]["objects"].is_array());
    let who =
        agent(&[tt_registry::Scope::Read, tt_registry::Scope::Ui, tt_registry::Scope::Edit, tt_registry::Scope::Input]);
    let e = a.pointer_in_window(who, json!({"view":"drawing","events":[{"type":"hover","x":1225,"y":425}]}));
    assert!(e.is_ok());
    assert!(e.result.unwrap()["hover"]["objects"].is_array());
}

#[test]
fn serialized_field_absent_null_present_matrix() {
    use tt_executor::gesture::{PointerEvent, PointerRequest};
    let decode_event = |v: Value| serde_json::from_str::<PointerEvent>(&v.to_string());
    let base = json!({"type":"hover","x":1,"y":2});
    for (field, present) in [
        ("pressure", json!(0.5)),
        ("tiltX", json!(12.0)),
        ("tiltY", json!(-3.0)),
        ("button", json!("left")),
        ("dt", json!(5.0)),
        ("wheelDelta", json!(-1.0)),
    ] {
        let absent = serde_json::to_value(decode_event(base.clone()).unwrap()).unwrap();
        let mut n = base.clone();
        n[field] = Value::Null;
        assert_eq!(absent, serde_json::to_value(decode_event(n).unwrap()).unwrap(), "{field}");
        let mut v = base.clone();
        v[field] = present.clone();
        assert_eq!(serde_json::to_value(decode_event(v).unwrap()).unwrap()[field], present);
    }
    for field in ["type", "x", "y"] {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(decode_event(v.clone()).is_err());
        let mut v = base.clone();
        v[field] = Value::Null;
        assert!(decode_event(v.clone()).is_err());
    }
    let mut v = base;
    v["modifiers"] = Value::Null;
    assert!(decode_event(v.clone()).is_err());
    v["modifiers"] = json!(["shift"]);
    assert!(decode_event(v).is_ok());
    let base = json!({"view":"drawing","events":[]});
    let decode = |v: Value| serde_json::from_str::<PointerRequest>(&v.to_string());
    for (field, present) in [("space", json!("view")), ("pointerType", json!("pen")), ("tool", json!("brush"))] {
        assert!(decode(base.clone()).is_ok());
        let mut v = base.clone();
        v[field] = Value::Null;
        assert_eq!(decode(v).is_ok(), field == "tool");
        let mut v = base.clone();
        v[field] = present;
        assert!(decode(v).is_ok());
    }
    for field in ["view", "events"] {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(decode(v).is_err());
        let mut v = base.clone();
        v[field] = Value::Null;
        assert!(decode(v).is_err());
    }
}
