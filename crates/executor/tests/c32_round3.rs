//! M3 C3.2 round 3 (builder; C32-R3-01): acceptance evidence for animation history and the session/UI boundary.
//!
//! - Running undo and redo of an animation step on a large rig (24 nested pegs, 1 000-key channels) are cancelled by
//!   their caller's budget: `command.timeout`, the document, revision, history position and session state as before
//!   (replay is atomic), no `doc.changed` or `anim.changed`, and the next undo or redo works. A queued undo behind a
//!   running evaluation keeps its own deadline and is either applied whole or refused whole.
//! - `anim.*` mutations pass the same acceptance rules as any command, from the user and from an agent session: stale
//!   `expectRev` (`rev.conflict`), scopes, locked targets addressed by id (05 §6.6: they still apply), work limits,
//!   cancellation (C23-R3-01: a refused call restores the session state in full).
//! - Delivery: `anim.changed` is a session event (05 §19.4; 11 §9.2: events are the `app://events` stream, every
//!   subscriber receives them, with the call's origin); the issuing request's own answer is its envelope.
//!
//! The work hooks are process-global, so the tests take turns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};
use tt_registry::Scope;

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const INTRO: &str = "scene:\"Intro\"";

static TURN: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Spin;

impl Spin {
    fn on(us: u64) -> Self {
        tt_vector::work::hooks::set_spin_micros(us);
        Self
    }
}

impl Drop for Spin {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}

/// 24 nested pegs over 100 000 frames, each with a 1 000-key rotation channel (the shape of `c32_budgets`).
fn rig(h: &ExecutorHandle) -> Vec<String> {
    let mut parent: Option<String> = None;
    let mut nodes = Vec::new();
    for i in 0..24 {
        let mut p = json!({ "type": "peg", "name": format!("Deep{i}"), "sceneId": INTRO });
        if let Some(q) = &parent {
            p["parentId"] = json!(q);
        }
        let id = ok(h, "element.create", p)["elementIds"][0].as_str().unwrap().to_owned();
        ok(h, "element.timing.set", json!({ "elementIds": [id], "duration": 100_000 }));
        let keys: Vec<Value> = (0..1_000)
            .map(|k| json!({ "frame": 1 + k * 100, "value": f64::from(k % 7), "preset": "easeInOut" }))
            .collect();
        ok(h, "anim.function.set", json!({ "nodeId": id, "channel": "rotation", "keys": keys }));
        parent = Some(id.clone());
        nodes.push(id);
    }
    nodes
}

/// Everything a cancelled replay must leave alone: the rig's objects, the revision, the history and the app state.
fn snapshot(h: &ExecutorHandle, nodes: &[String]) -> Value {
    let objects: Vec<Value> = nodes.iter().map(|n| get(h, n)).collect();
    json!({
        "objects": objects,
        "rev": rev(h),
        "history": ok(h, "edit.history.get", json!({})),
        "app": ok(h, "app.state.get", json!({})),
    })
}

fn anim_events(rx: &std::sync::mpsc::Receiver<tt_executor::Event>) -> Vec<(String, Value)> {
    rx.try_iter().map(|e| (e.name, e.payload)).collect()
}

fn code(env: &tt_executor::Envelope) -> Option<&str> {
    env.error.as_ref().map(|e| e.code.as_str())
}

// ----- (a) running and queued history replay ---------------------------------------------------------------------

#[test]
fn running_undo_and_redo_of_a_large_animation_step_are_cancelled_atomically() {
    let _t = turn();
    let budget = Duration::from_secs(2);
    let h = fixture_with(ExecutorConfig { budget });
    let nodes = rig(&h);
    ok(&h, "element.select", json!({ "elementIds": [nodes[3]] }));
    ok(&h, "app.frame.set", json!({ "frame": 9 }));
    // One step that keys all 24 pegs: its undo and redo replay 24 entries of 1 000 keys.
    let rx = h.subscribe();
    ok(&h, "anim.transform.set", json!({ "nodeIds": nodes, "frame": 5_050, "rotation": 3 }));
    let direct: Vec<Value> =
        anim_events(&rx).into_iter().filter(|(n, _)| n == "anim.changed").map(|(_, p)| p).collect();
    assert_eq!(direct.len(), 1);
    for (cmd, label) in [("edit.undo", "undo"), ("edit.redo", "redo")] {
        let before = snapshot(&h, &nodes);
        let rx = h.subscribe();
        // Each replayed entry spins 150 ms: 24 entries overrun the 2 s budget while the replay runs.
        let t = Instant::now();
        let env = {
            let _spin = Spin::on(150_000);
            run(&h, cmd, json!({}))
        };
        let answered = t.elapsed();
        assert_eq!(code(&env), Some("command.timeout"), "{label}: {env:?}");
        assert!(answered < budget * 3, "{label}: answered in {answered:?}");
        // Nothing commits late.
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(snapshot(&h, &nodes), before, "{label}: document, revision, history and session unchanged");
        let events = anim_events(&rx);
        assert!(
            !events.iter().any(|(n, _)| n == "anim.changed" || n == "doc.changed" || n == "edit.history.changed"),
            "{label}: a cancelled replay announces nothing: {events:?}"
        );
        // The same replay without the hook applies whole, with the step's event.
        let rx = h.subscribe();
        let env = run(&h, cmd, json!({}));
        assert_eq!(env.status, Status::Ok, "{label} after cancellation: {env:?}");
        let replayed: Vec<Value> =
            anim_events(&rx).into_iter().filter(|(n, _)| n == "anim.changed").map(|(_, p)| p).collect();
        assert_eq!(replayed, direct, "{label}: the step's scope");
        assert_eq!(rev(&h), before["rev"].as_u64().unwrap() + 1, "{label}: one revision");
    }
}

#[test]
fn a_queued_undo_behind_a_running_evaluation_keeps_its_own_deadline_and_is_atomic() {
    let _t = turn();
    let budget = Duration::from_secs(2);
    let h = fixture_with(ExecutorConfig { budget });
    let nodes = rig(&h);
    let rx = h.subscribe();
    ok(&h, "anim.transform.set", json!({ "nodeIds": nodes, "frame": 5_050, "rotation": 3 }));
    let direct: Vec<Value> =
        anim_events(&rx).into_iter().filter(|(n, _)| n == "anim.changed").map(|(_, p)| p).collect();
    let before = snapshot(&h, &nodes);
    let position = before["history"]["position"].as_u64().unwrap();
    let spin = Spin::on(2_000);
    let a = h.clone();
    let deepest = nodes.last().unwrap().clone();
    let frames: Vec<f64> = (0..20_000).map(|i| 1.0 + f64::from(i) * 4.5).collect();
    let running =
        std::thread::spawn(move || run(&a, "anim.evaluate", json!({ "nodeIds": [deepest], "frames": frames })));
    std::thread::sleep(Duration::from_millis(150));
    let rx = h.subscribe();
    let t = Instant::now();
    let queued = run(&h, "edit.undo", json!({}));
    let waited = t.elapsed();
    let running = running.join().unwrap();
    drop(spin);
    assert_eq!(code(&running), Some("command.timeout"), "the evaluation overran: {running:?}");
    assert!(waited < budget * 2, "the queued undo answered within its own budget: {waited:?}");
    let events = anim_events(&rx);
    let changed: Vec<&Value> = events.iter().filter(|(n, _)| n == "anim.changed").map(|(_, p)| p).collect();
    let history = ok(&h, "edit.history.get", json!({}));
    if queued.status == Status::Ok {
        // Applied whole: one step back, the step's event once.
        assert_eq!(history["position"].as_u64().unwrap(), position - 1);
        assert_eq!(changed, direct.iter().collect::<Vec<_>>());
    } else {
        // Refused whole: nothing moved, nothing announced.
        assert_eq!(code(&queued), Some("command.timeout"), "{queued:?}");
        assert_eq!(snapshot(&h, &nodes), before);
        assert!(changed.is_empty(), "{events:?}");
        assert!(run(&h, "edit.undo", json!({})).is_ok(), "the next undo works");
    }
}

// ----- (b) the session/UI acceptance boundary ----------------------------------------------------------------------

fn as_origin(
    h: &ExecutorHandle,
    origin: &Origin,
    id: &str,
    params: Value,
    options: InvokeOptions,
) -> tt_executor::Envelope {
    h.invoke(Invocation { command: id.to_owned(), params, options, origin: origin.clone() })
}

#[test]
fn animation_mutations_pass_the_same_acceptance_rules_as_any_command_for_user_and_agent() {
    let _t = turn();
    let editor = agent(&[Scope::Read, Scope::Edit, Scope::Ui]);
    let reader = agent(&[Scope::Read]);
    for origin in [Origin::User, editor] {
        let h = fixture();
        ok(&h, "element.select", json!({ "elementIds": [PEG] }));
        ok(&h, "app.frame.set", json!({ "frame": 7 }));
        let app = ok(&h, "app.state.get", json!({}));
        let rev0 = rev(&h);
        let peg = get(&h, PEG);
        let stale = InvokeOptions {
            expect_rev: [(peg["id"].as_str().unwrap().to_owned(), peg["rev"].as_u64().unwrap() + 5)]
                .into_iter()
                .collect(),
            ..InvokeOptions::default()
        };
        // Stale `expectRev`: the same refusal as the generic patch of the same property, nothing changed or announced.
        let rx = h.subscribe();
        let anim = as_origin(
            &h,
            &origin,
            "anim.transform.set",
            json!({ "nodeIds": [PEG], "target": "static", "rotation": 5 }),
            stale.clone(),
        );
        let generic = as_origin(
            &h,
            &origin,
            "doc.object.patch",
            json!({ "targets": [PEG], "props": { "transform": { "rotation": 5 } } }),
            stale,
        );
        assert_eq!(code(&anim), Some("rev.conflict"), "{origin:?}: {anim:?}");
        assert_eq!(code(&anim), code(&generic));
        assert_eq!((rev(&h), ok(&h, "app.state.get", json!({}))), (rev0, app.clone()));
        assert!(!rx.try_iter().any(|e| e.name == "anim.changed"));
        // A locked element addressed by id still takes the command (05 §6.6), like any id-addressed command.
        ok(&h, "element.lock.set", json!({ "elementIds": [PEG], "locked": true }));
        let rx = h.subscribe();
        let anim = as_origin(
            &h,
            &origin,
            "anim.transform.set",
            json!({ "nodeIds": [PEG], "target": "static", "rotation": 6 }),
            InvokeOptions::default(),
        );
        let generic = as_origin(
            &h,
            &origin,
            "doc.object.patch",
            json!({ "targets": [PEG], "props": { "transform": { "skew": 2 } } }),
            InvokeOptions::default(),
        );
        assert_eq!((anim.status, generic.status), (Status::Ok, Status::Ok), "{anim:?} {generic:?}");
        assert_eq!(rx.try_iter().filter(|e| e.name == "anim.changed").count(), 2);
        // Work limits: an oversized evaluation is refused before the watchdog, the session untouched.
        let app = ok(&h, "app.state.get", json!({}));
        let frames: Vec<f64> = (0..100_000).map(|i| 1.0 + f64::from(i)).collect();
        let big = as_origin(
            &h,
            &origin,
            "anim.evaluate",
            json!({ "nodeIds": vec![BOB; 11], "frames": frames }),
            InvokeOptions::default(),
        );
        assert_eq!(code(&big), Some("limit.exceeded"), "{big:?}");
        assert_eq!(ok(&h, "app.state.get", json!({})), app);
    }
    // Scopes: a read-only agent is refused the edit exactly as it is refused the generic patch.
    let h = fixture();
    let rev0 = rev(&h);
    let anim = as_origin(
        &h,
        &reader,
        "anim.transform.set",
        json!({ "nodeIds": [PEG], "frame": 3, "rotation": 1 }),
        InvokeOptions::default(),
    );
    let generic = as_origin(
        &h,
        &reader,
        "doc.object.patch",
        json!({ "targets": [PEG], "props": { "transform": { "rotation": 1 } } }),
        InvokeOptions::default(),
    );
    assert_eq!(anim.status, Status::Error, "{anim:?}");
    assert_eq!(code(&anim), code(&generic));
    assert_eq!(rev(&h), rev0);
    // A read is allowed.
    assert!(
        as_origin(&h, &reader, "anim.evaluate", json!({ "nodeIds": [PEG], "frames": [1] }), InvokeOptions::default())
            .is_ok()
    );
}

#[test]
fn a_cancelled_animation_edit_restores_the_session_state_in_full() {
    let _t = turn();
    let budget = Duration::from_millis(400);
    let h = fixture_with(ExecutorConfig { budget });
    ok(&h, "element.select", json!({ "elementIds": [BOB] }));
    ok(&h, "app.frame.set", json!({ "frame": 5 }));
    let app = ok(&h, "app.state.get", json!({}));
    let rev0 = rev(&h);
    let history = ok(&h, "edit.history.get", json!({}));
    let rx = h.subscribe();
    // An atomic batch that also moves the current frame and the selection, then overruns while keying.
    let env = {
        let _spin = Spin::on(300_000);
        batch(
            &h,
            json!({ "label": "cancelled pose", "atomic": true, "steps": [
                { "id": "app.frame.set", "params": { "frame": 12 } },
                { "id": "element.select", "params": { "elementIds": [PEG] } },
                { "id": "anim.transform.set", "params": { "nodeIds": [BOB, PEG], "frame": 12, "rotation": 1 } } ] }),
        )
    };
    assert_eq!(code(&env), Some("command.timeout"), "{env:?}");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(ok(&h, "app.state.get", json!({})), app, "selection and frame restored");
    assert_eq!(rev(&h), rev0);
    assert_eq!(ok(&h, "edit.history.get", json!({})), history);
    let events: Vec<String> = rx.try_iter().map(|e| e.name).collect();
    assert!(!events.iter().any(|n| n == "anim.changed" || n == "doc.changed"), "{events:?}");
}

// ----- delivery ---------------------------------------------------------------------------------------------------

#[test]
fn animation_events_reach_every_subscriber_whoever_issued_the_call() {
    let _t = turn();
    let h = fixture();
    let editor = agent(&[Scope::Read, Scope::Edit]);
    for origin in [Origin::User, editor] {
        let issuer = h.subscribe();
        let observer = h.subscribe();
        let env = as_origin(
            &h,
            &origin,
            "anim.transform.set",
            json!({ "nodeIds": [BOB], "frame": 18, "rotation": 2 }),
            InvokeOptions::default(),
        );
        assert!(env.is_ok(), "{env:?}");
        let a: Vec<Value> = issuer.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload).collect();
        let b: Vec<Value> = observer.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload).collect();
        assert_eq!(a.len(), 1, "{origin:?}");
        assert_eq!(a, b, "the same event for every subscriber");
        assert_eq!(a[0]["frames"], json!({ "from": 12.0, "to": 24.0 }));
        // The issuing request's answer is its envelope; the event is not part of it.
        let answer = serde_json::to_value(env.result.clone()).unwrap();
        assert!(answer.get("nodeIds").is_some() && answer.get("channels").is_none(), "{answer}");
        ok(&h, "edit.undo", json!({}));
    }
}
