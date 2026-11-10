//! Independent multi-step history replay and branch-discard oracle.
mod c13_support;
mod common;
use common::*;
use serde_json::{Value, json};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
fn call(h: &tt_executor::ExecutorHandle, id: &str, params: Value) -> Vec<Value> {
    let rx = h.subscribe();
    ok(h, id, params);
    let events: Vec<_> = rx.try_iter().collect();
    let anim: Vec<_> = events.iter().enumerate().filter(|(_, e)| e.name == "anim.changed").collect();
    assert_eq!(anim.len(), 1, "{id}: {events:?}");
    assert!(events[..anim[0].0].iter().any(|e| e.name == "doc.changed"));
    let Some(doc) = events[..anim[0].0].iter().rev().find(|e| e.name == "doc.changed") else { unreachable!() };
    assert_eq!(doc.payload["documentRev"], json!(rev(h)));
    assert!(doc.payload["undoStepId"].as_str().is_some_and(|s| !s.is_empty()));
    anim.into_iter().map(|(_, e)| e.payload.clone()).collect()
}
#[test]
fn several_steps_replay_their_own_scope_and_discarded_branch_never_returns() {
    let h = fixture();
    let mut expected = Vec::new();
    for frame in [15, 18, 21] {
        expected.push(call(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":frame,"rotation":frame})));
    }
    for ev in expected.iter().rev() {
        assert_eq!(&call(&h, "edit.undo", json!({})), ev);
    }
    for ev in &expected {
        assert_eq!(&call(&h, "edit.redo", json!({})), ev);
    }
    call(&h, "edit.undo", json!({}));
    let replacement = call(&h, "anim.transform.set", json!({"nodeIds":[B],"target":"static","skew":7}));
    let rx = h.subscribe();
    let refused = run(&h, "edit.redo", json!({}));
    assert_eq!(refused.status, tt_executor::Status::Error);
    assert_eq!(rx.try_iter().filter(|e| e.name == "anim.changed").count(), 0);
    assert_eq!(call(&h, "edit.undo", json!({})), replacement);
    assert_eq!(call(&h, "edit.redo", json!({})), replacement);
}
#[test]
fn loop_scope_and_fractional_samples_survive_history() {
    let h = fixture();
    let id = ok(&h, "element.create", json!({"type":"peg","name":"R3 loop"}))["elementIds"][0].clone();
    ok(&h, "element.timing.set", json!({"elementIds":[id],"duration":10}));
    ok(
        &h,
        "anim.function.set",
        json!({"nodeId":id,"channel":"rotation","keys":[{"frame":1,"value":0},{"frame":10,"value":90}]}),
    );
    let changed = call(&h, "anim.loop.set", json!({"nodeId":id,"loops":3}));
    let params = json!({"nodeId":id,"channel":"rotation","from":9.5,"to":21.5,"step":0.5});
    let samples = ok(&h, "anim.function.sample", params.clone());
    let Some(values) = samples["samples"].as_array() else { unreachable!() };
    for value in values {
        let Some(frame) = value["frame"].as_f64() else { unreachable!() };
        let Some(actual) = value["value"].as_f64() else { unreachable!() };
        let local = ((frame - 1.0) % 10.0 + 1.0).min(10.0);
        assert!((actual - (local - 1.0) * 10.0).abs() < 1e-8, "{value}");
    }
    assert_eq!(call(&h, "edit.undo", json!({})), changed);
    assert_eq!(call(&h, "edit.redo", json!({})), changed);
    assert_eq!(ok(&h, "anim.function.sample", params), samples);
    let path = ok(&h, "anim.path.get", json!({"nodeId":id}));
    eprintln!("R3 passage payload: {path}");
}
#[test]
fn camera_whole_block_and_hidden_annotation_replay_keep_evaluation() {
    let h = fixture();
    let camera = "scene:\"Intro\"/element:\"Camera\"";
    let block = json!({"targets":[camera],"props":{"camera":{"fov":60,"fovMode":"static"}}});
    let direct = call(&h, "doc.object.patch", block);
    let params = json!({"nodeIds":[B,camera],"frames":[1,12.5]});
    let evaluated = ok(&h, "anim.evaluate", params.clone());
    assert_eq!(call(&h, "edit.undo", json!({})), direct);
    assert_eq!(call(&h, "edit.redo", json!({})), direct);
    assert_eq!(ok(&h, "anim.evaluate", params), evaluated);
    for (id, params) in [
        ("element.visibility.set", json!({"elementIds":[B],"scope":"scene","visible":false})),
        ("element.annotation.set", json!({"elementId":B,"annotation":true})),
    ] {
        ok(&h, id, params);
        let query = json!({"nodeIds":[B,camera],"frames":[1,12.5]});
        let after = ok(&h, "anim.evaluate", query.clone());
        ok(&h, "edit.undo", json!({}));
        ok(&h, "edit.redo", json!({}));
        assert_eq!(ok(&h, "anim.evaluate", query), after);
    }
}
#[test]
fn non_horizontal_passage_payload_is_repeatable_and_recorded() {
    let h = fixture();
    let id = ok(&h, "element.create", json!({"type":"peg","name":"R3 passage"}))["elementIds"][0].clone();
    ok(
        &h,
        "anim.path.set",
        json!({"nodeId":id,"points":[
      {"kind":"key","frame":1,"x":0,"y":0,"z":0},
      {"kind":"control","x":2,"y":3,"z":1},
      {"kind":"key","frame":24,"x":6,"y":4,"z":2}]}),
    );
    let params = json!({"nodeId":id});
    let a = ok(&h, "anim.path.get", params.clone());
    let b = ok(&h, "anim.path.get", params);
    assert_eq!(a, b);
    eprintln!("R3 non-horizontal passage payload: {a}");
}
#[test]
fn open_has_no_preopen_undo_or_replay_event() {
    let h = fixture();
    call(&h, "anim.transform.set", json!({"nodeIds":[B],"frame":18,"rotation":37}));
    let dir = c13_support::tmp();
    let path = dir.join("r3-fresh.animpkg");
    c13_support::job_ok(&h, "project.saveAs", json!({"path":path}));
    ok(&h, "project.close", json!({"onUnsaved":"discard"}));
    c13_support::job_ok(&h, "project.open", json!({"path":path}));
    let rx = h.subscribe();
    let refused = run(&h, "edit.undo", json!({}));
    assert_eq!(refused.status, tt_executor::Status::Error);
    assert!(!rx.try_iter().any(|e| e.name == "anim.changed"));
}
