//! The headless `pointer` path (design note 4: P1, P3, P5; design note 3: G3, G6; SI-129, SI-130).

use serde_json::{Value, json};
use tt_executor::{Origin, Status};
use tt_registry::Scope;

use crate::common::*;
use crate::equivalence::{canonical, d, drag, ev, history, pointer, ready};

fn selection(h: &tt_executor::ExecutorHandle) -> Value {
    ok(h, "draw.selection.get", json!({}))
}

#[test]
fn p3_the_same_events_give_the_same_commands_on_every_run_whatever_dt() {
    let events = || {
        let mut e = drag(&[(1000.0, 500.0), (1010.0, 490.0), (1030.0, 470.0)]);
        for (i, x) in e.iter_mut().enumerate() {
            x["dt"] = json!(16.0 * i as f64);
        }
        e
    };
    let (a, b) = (ready(), ready());
    let ea = pointer(&a, "pencil", events());
    let eb = pointer(&b, "pencil", events());
    assert_eq!(ea.result, eb.result);
    assert_eq!(canonical(&a), canonical(&b));
    // Simulated time does not change the command: the same samples with other dt values.
    let c = ready();
    let mut other = events();
    for x in &mut other {
        x["dt"] = json!(1000.0);
    }
    assert_eq!(pointer(&c, "pencil", other).result.unwrap()["commands"], ea.result.unwrap()["commands"]);
}

#[test]
fn g6_a_gesture_without_its_release_commits_nothing() {
    let h = ready();
    let (before, steps, sel) = (canonical(&h), history(&h), selection(&h));
    let env = pointer(&h, "brush", vec![ev("down", 1000.0, 500.0), ev("move", 1020.0, 520.0)]);
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.unwrap()["commands"].clone();
    assert_eq!(commands.as_array().unwrap().len(), 1, "only tool.active.set: {commands}");
    assert_eq!((canonical(&h), history(&h), selection(&h)), (before, steps, sel), "byte-identical (G6)");
}

#[test]
fn g3_navigation_inside_a_gesture_is_refused_and_nothing_is_committed() {
    let h = ready();
    let before = canonical(&h);
    let mut events = vec![ev("down", 1000.0, 500.0)];
    let mut wheel = ev("wheel", 1000.0, 500.0);
    wheel["wheelDelta"] = json!(-120);
    events.push(wheel);
    events.push(ev("up", 1100.0, 400.0));
    let e = pointer(&h, "rectangle", events).error.unwrap();
    assert_eq!(e.code, "arg.invalid");
    assert_eq!(canonical(&h), before);
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 1.0);
    // Outside a gesture the wheel zooms by the table about the pointer.
    let mut w = ev("wheel", 1200.0, 300.0);
    w["wheelDelta"] = json!(-120);
    let env = h.pointer(Origin::User, json!({ "view": "drawing", "events": [w] }));
    assert_eq!(env.result.unwrap()["commands"][0]["commandId"], "view.zoom.set");
    assert_eq!(ok(&h, "view.transform.get", json!({ "viewId": "drawingView" }))["zoom"], 2.0, "the 02 §4.3 ladder");
}

#[test]
fn p5_a_refused_gesture_answers_its_command_s_refusal_and_ends_the_call() {
    let h = ready();
    ok(&h, "element.drawingLock.set", json!({ "locked": true }));
    let before = canonical(&h);
    let mut events = drag(&[(1000.0, 500.0), (1100.0, 400.0)]);
    events.extend(drag(&[(900.0, 500.0), (950.0, 450.0)]));
    let e = pointer(&h, "rectangle", events).error.unwrap();
    assert_eq!(e.code, "drawing.locked");
    assert_eq!(
        (e.details["gesture"].as_u64(), e.details["gestureCommandId"].as_str()),
        (Some(1), Some("draw.rectangle.add"))
    );
    // The receipts of the commands that succeeded before the refusal (C42A-R1-22): here `tool.active.set`.
    let done = e.details["completed"].as_array().unwrap();
    assert_eq!(done.len(), 1, "{done:?}");
    assert_eq!(
        (done[0]["commandId"].as_str(), done[0]["envelope"]["status"].as_str()),
        (Some("tool.active.set"), Some("ok"))
    );
    assert_eq!(canonical(&h), before);
}

#[test]
fn tools_without_built_gestures_say_so_and_drawing_space_events_map_back() {
    let h = ready();
    let e = pointer(&h, "text", drag(&[(1000.0, 500.0), (1000.0, 500.0)])).error.unwrap();
    assert_eq!((e.code.as_str(), &e.details["owner"]), ("command.notImplemented", &json!("M4 C4.3")));
    // `space: "drawing"` events are converted to view pixels at the press: the same command as view events.
    let a = ready();
    let view = a.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": "line", "events": drag(&[(1000.0, 500.0), (1100.0, 400.0)]) }),
    );
    let b = ready();
    let drawing = b.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": "line", "space": "drawing",
                "events": [{ "type": "down", "x": 40, "y": 40 }, { "type": "up", "x": 140, "y": 140 }] }),
    );
    assert_eq!(view.result.unwrap()["commands"][1]["params"], drawing.result.clone().unwrap()["commands"][1]["params"]);
    assert_eq!(drawing.result.unwrap()["hover"]["drawing"], d(1100.0, 400.0));
    let e = b.pointer(Origin::User, json!({ "view": "drawing", "space": "camera", "events": [] })).error.unwrap();
    assert_eq!(e.code, "arg.invalid", "SI-125");
}

#[test]
fn limits_and_agent_scopes_are_kept() {
    let h = ready();
    let many: Vec<Value> = (0..10_001).map(|i| ev("hover", f64::from(i % 1000), 10.0)).collect();
    let e = h.pointer(Origin::User, json!({ "view": "drawing", "events": many })).error.unwrap();
    assert_eq!((e.code.as_str(), &e.details["limit"]), ("limit.exceeded", &json!("pointerEvents")));
    let e = h.pointer(Origin::User, json!({ "view": "drawing", "events": [ev("down", 1e9, 0.0)] })).error.unwrap();
    assert_eq!(e.code, "arg.invalid");
    let e = h.pointer(Origin::User, json!({ "view": "nowhere", "events": [] })).error.unwrap();
    assert_eq!(e.code, "view.notFound");
    // An agent without the edit scope cannot draw through `pointer` either.
    let a = agent(&[Scope::Ui, Scope::Read]);
    let before = canonical(&h);
    let e = h
        .pointer(a, json!({ "view": "drawing", "tool": "brush", "events": drag(&[(1000.0, 500.0), (1010.0, 490.0)]) }))
        .error
        .unwrap();
    assert_eq!(e.code, "permission.denied", "{e:?}");
    assert_eq!(canonical(&h), before);
}

#[test]
fn p1_one_call_is_one_document_thread_request() {
    // Two threads each draw a 3-click polyline through `pointer`; each call's clicks stay one gesture (headless calls
    // never interleave).
    let h = std::sync::Arc::new(ready());
    let mut threads = Vec::new();
    for k in 0..2 {
        let h = h.clone();
        threads.push(std::thread::spawn(move || {
            let x = 900.0 + 300.0 * f64::from(k);
            let mut events = Vec::new();
            for (dx, dy) in [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)] {
                events.extend(drag(&[(x + dx, 500.0 + dy), (x + dx, 500.0 + dy)]));
            }
            events.push(ev("doubleClick", x + 50.0, 550.0));
            pointer(&h, "polyline", events)
        }));
    }
    for t in threads {
        let env = t.join().unwrap();
        let c = env.result.unwrap()["commands"][1].clone();
        let pts = c["params"]["points"].as_array().unwrap().clone();
        assert_eq!(pts.len(), 3, "{c}");
        let x0 = pts[0]["x"].as_f64().unwrap();
        assert!(pts.iter().all(|p| (p["x"].as_f64().unwrap() - x0).abs() <= 50.0), "one call's clicks only: {c}");
    }
}

#[test]
fn p4_kernel_rule_agent_writes_wait_for_a_user_gesture_then_answer_busy() {
    let h = ready();
    h.set_wait_for_user(std::time::Duration::from_millis(30));
    h.report_shell(serde_json::from_value(json!({ "modal": { "kind": "gesture", "title": "Brush" } })).unwrap());
    let a = agent(&[Scope::Ui, Scope::Read, Scope::Edit]);
    for (id, p) in [
        ("tool.active.set", json!({ "toolId": "brush" })),
        ("drawAid.grid.set", json!({ "visible": true })),
        ("view.zoom.set", json!({ "viewId": "drawingView", "steps": 1 })),
    ] {
        let e = h.invoke(tt_executor::Invocation {
            command: id.into(),
            params: p,
            options: Default::default(),
            origin: a.clone(),
        });
        assert_eq!(e.error.map(|x| x.code), Some("ui.busy".to_owned()), "{id}");
    }
    let e = h.pointer(
        a.clone(),
        json!({ "view": "drawing", "tool": "brush", "events": drag(&[(1000.0, 500.0), (1010.0, 490.0)]) }),
    );
    assert_eq!(e.error.map(|x| x.code), Some("ui.busy".to_owned()));
    let q = h.invoke(tt_executor::Invocation {
        command: "tool.active.get".into(),
        params: json!({}),
        options: Default::default(),
        origin: a,
    });
    assert!(q.is_ok(), "queries still run");
}

#[test]
fn hover_names_the_objects_under_it_and_tilt_is_carried_but_changes_no_stroke() {
    let h = ready();
    crate::equivalence::painted_rect(&h);
    let env = h.pointer(Origin::User, json!({ "view": "drawing", "events": [ev("hover", 1310.0, 210.0)] }));
    let objects = env.result.unwrap()["hover"]["objects"].clone();
    assert!(objects.as_array().is_some_and(|o| !o.is_empty()), "{objects}");
    // Tilt changes no stroke (SI-125): the same command with and without it.
    let tilted = |t: f64| {
        let mut e = drag(&[(1000.0, 500.0), (1010.0, 490.0)]);
        for x in &mut e {
            x["tiltX"] = json!(t);
            x["tiltY"] = json!(-t);
        }
        pointer(&ready(), "brush", e).result.unwrap()["commands"][1]["params"].clone()
    };
    assert_eq!(tilted(0.0), tilted(40.0));
    let mut e = drag(&[(1000.0, 500.0), (1010.0, 490.0)]);
    e[0]["tiltX"] = json!(91);
    assert_eq!(pointer(&h, "brush", e).error.unwrap().code, "arg.invalid");
}
