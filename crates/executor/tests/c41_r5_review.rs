#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review assertions")]
// Independent C4.1 closure probes: budgets and state refusals outside the builder oracle.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use tt_executor::{ExecutorHandle, Invocation};

fn gui_refs() -> Vec<(String, Map<String, Value>)> {
    let mut refs: BTreeSet<(String, String)> = BTreeSet::new();
    let registry = tt_executor::registry().unwrap();
    for e in registry.entries() {
        if let Some(c) = e.command
            && c.decl.agent_only.is_none()
            && matches!(e.status(), tt_executor::DeclarationStatus::Implemented)
        {
            refs.insert((e.id().to_owned(), "{}".to_owned()));
        }
    }
    fn walk(v: &Value, out: &mut BTreeSet<(String, String)>) {
        match v {
            Value::Object(m) => {
                if let Some(id) = m.get("command").and_then(Value::as_str) {
                    let p = m.get("params").cloned().unwrap_or_else(|| json!({}));
                    out.insert((id.to_owned(), p.to_string()));
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let menu: Value = serde_json::from_str(include_str!("../../../desktop/src/ui-bindings/menu.json")).unwrap();
    walk(&menu, &mut refs);
    let keymap: Value = serde_json::from_str(include_str!("../src/keymap-default.json")).unwrap();
    for b in keymap["bindings"].as_array().unwrap() {
        let p = b.get("params").cloned().unwrap_or_else(|| json!({}));
        refs.insert((b["commandId"].as_str().unwrap().to_owned(), p.to_string()));
    }
    refs.into_iter()
        .filter(|(id, _)| {
            registry.get(id).is_some_and(|e| matches!(e.status(), tt_executor::DeclarationStatus::Implemented))
        })
        .map(|(id, p)| (id, serde_json::from_str::<Value>(&p).unwrap().as_object().cloned().unwrap_or_default()))
        .collect()
}

fn ok(h: &ExecutorHandle, id: &str, p: Value) {
    let e = h.invoke(Invocation::user(id, p.clone()));
    assert!(e.error.is_none(), "{id} {p}: {e:?}");
}

#[test]
fn independent_large_state_budgets() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    ok(&h, "project.create", json!({"name":"Review"}));
    let refs = gui_refs();
    let best = || {
        (0..5)
            .map(|_| {
                let t = Instant::now();
                h.availability(refs.clone()).unwrap();
                t.elapsed()
            })
            .min()
            .unwrap()
    };
    let empty = best();
    let built = Instant::now();
    for _ in 0..102 {
        ok(&h, "element.create", json!({"type":"peg","count":99}));
    }
    eprintln!("build {:?}", built.elapsed());
    let large = best();
    let list = h.invoke(Invocation::user("element.list", json!({}))).result.unwrap();
    eprintln!("element list sample {}", list["elements"][0]);
    let ids: Vec<Value> = list["elements"].as_array().unwrap().iter().filter_map(|e| e.get("id").cloned()).collect();
    assert!(ids.len() >= 10000);
    eprintln!("empty {empty:?} large {large:?} objects {}", ids.len());
    assert!(large < Duration::from_millis(50) && large <= empty + Duration::from_millis(20));
    ok(&h, "edit.selection.set", json!({"context":"timeline","ids":ids}));
    let selected = best();
    ok(&h, "edit.copy", json!({"context":"timeline"}));
    let clipboard = best();
    let mut hold = Duration::ZERO;
    for _ in 0..5 {
        for chunk in refs.chunks(32) {
            hold = hold.max(h.availability_chunk_hold(chunk.to_vec()).unwrap());
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let (h, r, s) = (h.clone(), refs.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                h.availability(r.clone()).unwrap();
            }
        })
    };
    let mut latency = Duration::ZERO;
    let mut seed = 1234567u64;
    for _ in 0..200 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = Instant::now();
        while offset.elapsed() < Duration::from_micros(seed % 2000) {
            std::thread::yield_now();
        }
        let t = Instant::now();
        ok(&h, "app.frame.set", json!({"frame":1}));
        latency = latency.max(t.elapsed());
    }
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    eprintln!("selected {selected:?} clipboard {clipboard:?} hold {hold:?} latency200 {latency:?}");
    assert!(selected < Duration::from_millis(50) && clipboard < Duration::from_millis(50));
    assert!(hold <= Duration::from_millis(20));
    assert!(latency < Duration::from_millis(100));
}

#[test]
fn current_drawing_empty_selection_parity() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "app.context.set", json!({"elementId":"scene:\"Intro\"/element:\"Bob\"","frame":1}));
    for id in ["draw.object.delete"] {
        let a = h.availability(vec![(id.into(), Map::new())]).unwrap().remove(0);
        if a.dialog {
            continue;
        }
        let mut i = Invocation::user(id, a.params.clone());
        i.options.dry_run = true;
        let e = h.invoke(i);
        eprintln!("{id}: {a:?} {e:?}");
        assert_eq!(a.enabled, e.is_ok(), "{id} enabled but refuses {:?}", e.error);
    }
}

#[test]
fn locked_current_drawing_parity() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    ok(&h, "app.context.set", json!({"elementId":"scene:\"Intro\"/element:\"Bob\"","frame":1}));
    ok(&h, "element.drawingLock.set", json!({"locked":true}));
    let id = "element.drawingContent.delete";
    let a = h.availability(vec![(id.into(), Map::new())]).unwrap().remove(0);
    let mut i = Invocation::user(id, a.params.clone());
    i.options.dry_run = true;
    let e = h.invoke(i);
    assert_eq!(a.enabled, e.is_ok(), "{id} available {a:?} refuses {:?}", e.error);
}
