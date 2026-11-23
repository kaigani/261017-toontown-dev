#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use tt_executor::{Cx, ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions, Origin};
use tt_registry::{CommandError, Registry};

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
static CALLS: AtomicUsize = AtomicUsize::new(0);
#[tt_registry::command(id = "colour.palette.create", kind = action, label = "Panic probe",
    scopes = [edit], undo = "Panic probe", needs_project,
    example(title = "Probe", params = "{}"))]
fn probe(cx: &mut Cx, _p: Value) -> Result<Value, CommandError> {
    CALLS.fetch_add(1, Ordering::SeqCst);
    let id = cx.resolve_as("scene:#1/element:\"Bob\"", tt_model::ObjectType::Element)?;
    cx.patch_objects(vec![(
        id,
        vec![tt_model::change::PatchOp {
            op: "replace".into(),
            from: None,
            path: "/name".into(),
            range: None,
            value: Some(json!("Half")),
            old_value: None,
            elided: false,
        }],
    )])?;
    panic!("review: staged handler panic");
}
fn injected() -> ExecutorHandle {
    let mut commands: Vec<_> = tt_executor::registry()
        .unwrap()
        .entries()
        .filter(|e| e.id() != "colour.palette.create")
        .filter_map(|e| e.command)
        .collect();
    commands.push(&PROBE);
    let registry = Box::leak(Box::new(Registry::assemble(&commands).unwrap()));
    let h =
        ExecutorHandle::spawn_with_registry(ExecutorConfig { budget: std::time::Duration::from_secs(60) }, registry)
            .unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "doc.state.get", json!({}));
    h
}
fn state(h: &ExecutorHandle) -> Value {
    json!({"rev":rev(h), "history":ok(h,"edit.history.get",json!({})),
        "selection":ok(h,"cell.selection.get",json!({})),
        "rows":ok(h,"timeline.rows.list",json!({})),
        "bob":ok(h,"element.get",json!({"elementId":"scene:#1/element:\"Bob\""}))})
}
#[test]
fn e2_atomic_second_step_and_expect_rev_and_retry() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let h = injected();
    for expected in [false, true] {
        let before = state(&h);
        let mut req = json!({"label":"Panic batch","atomic":true,"steps":[
            {"id":"element.timing.set","params":{"elementIds":["scene:#1/element:\"Bob-P\""],"startDelta":2}},
            {"id":"colour.palette.create","params":{}},
            {"id":"timeline.currentFrame.set","params":{"frame":9}}]});
        if expected {
            req["expectRev"] = json!({"scene:#1":obj_rev(&h,&first(&h,"scene"))});
        }
        let n = CALLS.load(Ordering::SeqCst);
        let out = h.batch(serde_json::from_value(req).unwrap(), Origin::User);
        assert_eq!(out.error.as_ref().unwrap().details["error"]["code"], "internal", "{out:?}");
        assert_eq!(out.steps.as_ref().unwrap().len(), 2);
        assert_eq!(CALLS.load(Ordering::SeqCst), n + 1);
        assert_eq!(state(&h), before, "E-29: first step rolled back and third not run");
    }
}
#[test]
fn e2_requested_retry_returns_the_original_internal_answer_without_running_again() {
    // Ruling E-34: a repeated `requestId` returns the original result instead of executing again (spec/11 lines
    // 300-301), so a retry after a contained panic answers the same `internal` and the handler does not run a second time.
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let h = injected();
    let before = state(&h);
    let options = InvokeOptions { request_id: Some("r2-panic-retry".into()), ..Default::default() };
    let call = || {
        h.invoke(Invocation {
            command: "colour.palette.create".into(),
            params: json!({}),
            options: options.clone(),
            origin: Origin::User,
        })
    };
    let n = CALLS.load(Ordering::SeqCst);
    let first = call().error.unwrap();
    assert_eq!(first.details["reason"], "handlerPanicked");
    assert_eq!(CALLS.load(Ordering::SeqCst), n + 1, "the first call runs the handler");
    assert_eq!(state(&h), before);
    let retry = call().error.unwrap();
    assert_eq!(
        (retry.code.as_str(), &retry.message, &retry.details),
        (first.code.as_str(), &first.message, &first.details)
    );
    assert_eq!(CALLS.load(Ordering::SeqCst), n + 1, "the retry does not run the handler again");
    assert_eq!(state(&h), before);
    for _ in 0..20 {
        ok(&h, "timeline.rows.list", json!({}));
    }
}
