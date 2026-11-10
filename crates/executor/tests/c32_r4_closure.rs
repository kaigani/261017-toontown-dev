//! Independent closure oracles: canonical whole-document rollback and replay.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, InvokeOptions, Origin, Status};

struct Spin;
impl Spin {
    fn start(us: u64) -> Self {
        tt_vector::work::hooks::set_spin_micros(us);
        Self
    }
}
impl Drop for Spin {
    fn drop(&mut self) {
        tt_vector::work::hooks::set_spin_micros(0);
    }
}
fn bytes(h: &ExecutorHandle) -> Vec<u8> {
    let d = h.snapshot().unwrap().unwrap();
    let entries: Vec<_> = d.entry_keys().iter().map(|k| d.entry_raw_json(k).unwrap()).collect();
    tt_model::canonical::to_canonical_string(&json!(entries)).unwrap().into_bytes()
}
fn state(h: &ExecutorHandle) -> (Vec<u8>, u64, Value, Value) {
    (bytes(h), rev(h), ok(h, "edit.history.get", json!({})), ok(h, "app.state.get", json!({})))
}
fn events(rx: &std::sync::mpsc::Receiver<tt_executor::Event>) -> Vec<Value> {
    rx.try_iter().filter(|e| e.name == "anim.changed").map(|e| e.payload).collect()
}
fn poses(h: &ExecutorHandle, nodes: &[String]) -> Value {
    ok(h, "anim.evaluate", json!({"nodeIds":nodes,"frames":[1,5050,99901],"include":["world"]}))
}
#[test]
fn canonical_rollback_running_replays_queued_deadline_and_acceptance() {
    let budget = Duration::from_secs(8);
    let h = fixture_with(ExecutorConfig { budget });
    let mut nodes = Vec::new();
    for i in 0..24 {
        let mut p = json!({"type":"peg","name":format!("Closure{i}")});
        if let Some(parent) = nodes.last() {
            p["parentId"] = json!(parent);
        }
        let id = ok(&h, "element.create", p)["elementIds"][0].as_str().unwrap().to_owned();
        ok(&h, "element.timing.set", json!({"elementIds":[id],"duration":100_000}));
        let keys: Vec<_> = (0..1000).map(|k| json!({"frame":1+k*100,"value":k%9,"preset":"easeInOut"})).collect();
        ok(&h, "anim.function.set", json!({"nodeId":id,"channel":"rotation","keys":keys}));
        nodes.push(id);
    }
    ok(&h, "element.select", json!({"elementIds":[nodes[7]]}));
    ok(&h, "app.frame.set", json!({"frame":17}));
    let original = state(&h);
    let original_pose = poses(&h, &nodes);
    let rx = h.subscribe();
    ok(&h, "anim.transform.set", json!({"nodeIds":nodes,"frame":5050,"rotation":37}));
    let direct = events(&rx);
    assert_eq!(direct.len(), 1);
    let applied = state(&h);
    let applied_pose = poses(&h, &nodes);
    assert_ne!(original_pose, applied_pose);
    for cmd in ["edit.undo", "edit.redo"] {
        let before = state(&h);
        let rx = h.subscribe();
        let start = Instant::now();
        let result = {
            let _spin = Spin::start(500_000);
            run(&h, cmd, json!({}))
        };
        assert_eq!(result.error.as_ref().map(|e| e.code.as_str()), Some("command.timeout"), "{cmd}: {result:?}");
        assert!(start.elapsed() >= budget, "must actually exhaust request budget");
        assert!(start.elapsed() < budget * 3);
        assert_eq!(state(&h), before, "canonical entries, revision, cursor, selection and frame");
        assert!(
            !rx.try_iter().any(|e| matches!(e.name.as_str(), "anim.changed" | "doc.changed" | "edit.history.changed"))
        );
        let rx = h.subscribe();
        ok(&h, cmd, json!({}));
        assert_eq!(events(&rx), direct);
        let after = state(&h);
        assert_eq!(after.1, before.1 + 1);
        assert_eq!(after.3, before.3);
        assert_eq!(
            after.2["position"],
            if cmd == "edit.undo" { original.2["position"].clone() } else { applied.2["position"].clone() }
        );
        assert_eq!(poses(&h, &nodes), if cmd == "edit.undo" { original_pose.clone() } else { applied_pose.clone() });
    }
    let before = state(&h);
    let spin = Spin::start(2_000);
    let worker = h.clone();
    let deepest = nodes[23].clone();
    let running = std::thread::spawn(move || {
        let frames: Vec<_> = (0..20_000).map(|i| 1.0 + f64::from(i) * 4.5).collect();
        run(&worker, "anim.evaluate", json!({"nodeIds":[deepest],"frames":frames}))
    });
    std::thread::sleep(Duration::from_millis(150));
    let rx = h.subscribe();
    let start = Instant::now();
    let queued = run(&h, "edit.undo", json!({}));
    let elapsed = start.elapsed();
    let evaluation = running.join().unwrap();
    drop(spin);
    assert_eq!(evaluation.error.unwrap().code, "command.timeout");
    assert!(elapsed < budget + Duration::from_secs(2), "queued request deadline: {elapsed:?}");
    if queued.status == Status::Ok {
        assert_eq!(events(&rx), direct);
        assert_eq!(state(&h).2["position"], original.2["position"]);
        assert_eq!(rev(&h), before.1 + 1);
        assert_eq!(state(&h).3, before.3);
        assert_eq!(poses(&h, &nodes), original_pose);
        // Redo must recover every keyed object, not merely report success.
        ok(&h, "edit.redo", json!({}));
        for id in &nodes {
            let sampled = ok(
                &h,
                "anim.function.sample",
                json!({"nodeId":id,"channel":"rotation","from":5050,"to":5050,"step":1}),
            );
            assert_eq!(sampled["samples"][0]["value"], 37.0);
        }
    } else {
        assert_eq!(queued.error.unwrap().code, "command.timeout");
        assert_eq!(state(&h), before);
        assert!(!rx.try_iter().any(|e| matches!(e.name.as_str(), "anim.changed" | "doc.changed")));
    }
    for origin in [Origin::User, agent(&[tt_registry::Scope::Read, tt_registry::Scope::Edit, tt_registry::Scope::Ui])] {
        let id = &nodes[0];
        let object = get(&h, id);
        let before = state(&h);
        let stale = h.invoke(Invocation {
            command: "anim.transform.set".into(),
            params: json!({"nodeIds":[id],"target":"static","rotation":42}),
            origin: origin.clone(),
            options: InvokeOptions {
                expect_rev: [(id.clone(), object["rev"].as_u64().unwrap() + 1)].into_iter().collect(),
                ..Default::default()
            },
        });
        assert_eq!(stale.error.unwrap().code, "rev.conflict");
        assert_eq!(state(&h), before);
        ok(&h, "element.lock.set", json!({"elementIds":[id],"locked":true}));
        let changed = h.invoke(Invocation {
            command: "anim.transform.set".into(),
            params: json!({"nodeIds":[id],"target":"static","rotation":42}),
            origin: origin.clone(),
            options: Default::default(),
        });
        assert_eq!(changed.status, Status::Ok);
        assert_eq!(get(&h, id)["transform"]["rotation"], 42.0);
        let before = state(&h);
        let frames: Vec<_> = (1..=100_000).collect();
        let oversized = h.invoke(Invocation {
            command: "anim.evaluate".into(),
            params: json!({"nodeIds":vec![id;11],"frames":frames}),
            origin,
            options: Default::default(),
        });
        assert_eq!(oversized.error.unwrap().code, "limit.exceeded");
        assert_eq!(state(&h), before);
    }
    let before = state(&h);
    let rx = h.subscribe();
    let cancelled = {
        let _spin = Spin::start(500_000);
        batch(
            &h,
            json!({"label":"closure rollback","atomic":true,"steps":[
                {"id":"app.frame.set","params":{"frame":89}},
                {"id":"element.select","params":{"elementIds":[nodes[12]]}},
                {"id":"anim.transform.set","params":{"nodeIds":nodes,"frame":999,"rotation":11}}
            ]}),
        )
    };
    assert_eq!(cancelled.error.unwrap().code, "command.timeout");
    assert_eq!(state(&h), before);
    assert!(!rx.try_iter().any(|e| matches!(e.name.as_str(), "anim.changed" | "doc.changed")));
}
