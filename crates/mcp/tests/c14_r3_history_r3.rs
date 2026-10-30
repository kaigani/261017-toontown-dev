//! R3 rerun of the retained R1 independent oracle with a fresh seed and 248 CAS commits.
//! Independent stress/oracle tests, fixed fresh seed 0xc123_2026_1019_b683.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    reason = "review tests and evidence"
)]

use serde_json::{Value, json};
use std::time::Instant;
use tt_model::document::{Document, EntryKey};

fn artifact(name: &str, v: &Value) {
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        std::fs::write(std::path::Path::new(&dir).join(name), serde_json::to_vec_pretty(v).unwrap()).unwrap();
    }
}

fn canonical_entries(doc: &Document) -> Vec<(EntryKey, String)> {
    doc.entry_keys()
        .into_iter()
        .map(|key| {
            let text = tt_model::canonical::to_canonical_string(&doc.entry_raw_json(&key).unwrap()).unwrap();
            (key, text)
        })
        .collect()
}

#[test]
fn ten_thousand_history_operations_match_independent_content_oracle_and_replay() {
    const SEED: u64 = 0xc123_2026_1019_b683;
    let start = Instant::now();
    let h = saved_fixture();
    ok(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"seedBase":true}}}));
    let baseline = h.snapshot().unwrap().unwrap();
    let scene = first(&h, "scene");
    let (scene_entry, _, _) = baseline.object_pointer(&scene).unwrap();
    let scene_base = baseline.entry_raw_json(&scene_entry).unwrap();
    let mut current = scene_base["meta"].clone();
    let mut history: Vec<(Value, Value)> = Vec::new();
    let mut position = 0;
    let mut expected_rev = baseline.document_rev();
    let mut scene_rev = scene_base["rev"].as_u64().unwrap();
    let mut seed = SEED;
    let mut commands: Vec<(String, Value)> = Vec::new();
    let mut checkpoint = current.clone();
    let mut have_checkpoint = false;
    let mut counts = [0_u64; 6];
    for i in 0..10_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let choice = seed % 100;
        let (id, params) = if choice < 55 {
            // Opaque metadata with numeric keys, nested arrays, nulls and object-looking payloads.
            let v = json!({"0":{"name":"e\u{301}","rev":seed%7,"type":"stroke"},"array":[seed%11,null,{"value":i}],"review":seed});
            if v != current {
                history.truncate(position);
                history.push((current.clone(), v.clone()));
                position = history.len();
                if history.len() > 1000 {
                    history.remove(0);
                    position -= 1;
                }
                current = v.clone();
                expected_rev += 1;
                scene_rev = expected_rev;
            }
            counts[0] += 1;
            ("doc.object.patch", json!({"targets":[scene],"ops":[{"op":"replace","path":"/meta","value":v}]}))
        } else if choice < 75 && position > 0 {
            let n = ((seed >> 8) % 3 + 1).min(position as u64) as usize;
            for _ in 0..n {
                position -= 1;
                current = history[position].0.clone();
                expected_rev += 1;
                scene_rev = expected_rev;
            }
            counts[1] += n as u64;
            ("edit.undo", json!({"steps":n}))
        } else if choice < 88 && position < history.len() {
            let n = ((seed >> 8) % 3 + 1).min((history.len() - position) as u64) as usize;
            for _ in 0..n {
                current = history[position].1.clone();
                position += 1;
                expected_rev += 1;
                scene_rev = expected_rev;
            }
            counts[2] += n as u64;
            ("edit.redo", json!({"steps":n}))
        } else if choice < 91 && !have_checkpoint {
            checkpoint = current.clone();
            have_checkpoint = true;
            counts[3] += 1;
            ("checkpoint.create", json!({"name":"random-oracle"}))
        } else if choice < 95 && have_checkpoint {
            if checkpoint != current {
                history.truncate(position);
                history.push((current.clone(), checkpoint.clone()));
                position = history.len();
                if history.len() > 1000 {
                    history.remove(0);
                    position -= 1;
                }
                current = checkpoint.clone();
                expected_rev += 1;
                scene_rev = expected_rev;
            }
            counts[4] += 1;
            ("checkpoint.restore", json!({"name":"random-oracle"}))
        } else {
            counts[5] += 1;
            ("doc.object.patch", json!({"targets":[scene],"props":{"meta":{"dry":i}},"dryRun":true}))
        };
        let env = run(&h, id, params.clone());
        assert!(env["status"] == "ok", "seed {SEED:x}, operation {i}: {id} {params}: {env:?}");
        commands.push((id.into(), params));
        assert_eq!(rev(&h), expected_rev, "seed {SEED:x}, operation {i}");
        if i % 100 == 0 || i == 9999 {
            let snapshot = h.snapshot().unwrap().unwrap();
            for key in baseline.entry_keys() {
                let mut want = baseline.entry_raw_json(&key).unwrap();
                if key == scene_entry {
                    want["meta"] = current.clone();
                    want["rev"] = json!(scene_rev);
                }
                assert_eq!(
                    tt_model::canonical::to_canonical_string(&snapshot.entry_raw_json(&key).unwrap()).unwrap(),
                    tt_model::canonical::to_canonical_string(&want).unwrap(),
                    "seed {SEED:x}, operation {i}, {key:?}"
                );
            }
        }
    }
    let replay = saved_fixture();
    ok(&replay, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"seedBase":true}}}));
    for (i, (id, params)) in commands.iter().enumerate() {
        let env = run(&replay, id, params.clone());
        assert!(env["status"] == "ok", "replay {i}: {env:?}");
    }
    assert_eq!(
        canonical_entries(&h.snapshot().unwrap().unwrap()),
        canonical_entries(&replay.snapshot().unwrap().unwrap()),
        "fresh replay must be byte-identical including revisions"
    );
    let receipt = json!({"seed":format!("0x{SEED:x}"),"operations":10000,"replayOperations":10000,"editsUndoRedoCreateRestoreDryRun":counts,"documentRev":expected_rev,"durationSeconds":start.elapsed().as_secs_f64(),"oracle":"independent before/after metadata stack; exact canonical all-entry byte checks every 100 operations; full fresh executor replay"});
    artifact("mcp-history-r3.json", &receipt);
    artifact("mcp-history-r3-commands.json", &json!(commands));
    println!("{receipt}");
}

mod support;
use std::cell::RefCell;
struct Protocol {
    handle: tt_executor::ExecutorHandle,
    client: RefCell<Option<support::Client>>,
}
impl std::ops::Deref for Protocol {
    type Target = tt_executor::ExecutorHandle;
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}
impl Drop for Protocol {
    fn drop(&mut self) {
        if let Some(c) = self.client.get_mut().take() {
            c.close().unwrap();
        }
    }
}
fn run(h: &Protocol, id: &str, mut params: Value) -> Value {
    let dry = params.as_object_mut().and_then(|p| p.remove("dryRun")).unwrap_or(json!(false));
    h.client.borrow_mut().as_mut().unwrap().call("run_command", json!({"id":id,"params":params,"dryRun":dry}))["structuredContent"].clone()
}
fn ok(h: &Protocol, id: &str, params: Value) -> Value {
    let e = run(h, id, params);
    assert_eq!(e["status"], "ok", "{e}");
    e["result"].clone()
}
fn first(h: &Protocol, ty: &str) -> String {
    ok(h, "doc.object.find", json!({"type":ty,"limit":1}))["items"][0]["id"].as_str().unwrap().to_owned()
}
fn rev(h: &Protocol) -> u64 {
    h.snapshot().unwrap().unwrap().document_rev()
}
fn saved_fixture() -> Protocol {
    static SAVED: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let path = SAVED.get_or_init(|| {
        let dir = support::tmp();
        let h = support::executor(&dir.join("initial-user"));
        assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
        let path = dir.join("Oracle.animpkg");
        let e = h.invoke(tt_executor::Invocation::user("project.saveAs", json!({"path":path})));
        assert!(e.is_ok(), "{e:?}");
        let j = e.result.unwrap()["jobId"].clone();
        let done = h.invoke(tt_executor::Invocation::user("job.wait", json!({"jobId":j,"timeoutMs":60000})));
        assert_eq!(done.result.unwrap()["state"], "succeeded");
        assert!(h.invoke(tt_executor::Invocation::user("project.close", json!({}))).is_ok());
        path
    });
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    let mut opts = support::opts(path.parent().unwrap());
    opts.policy = Some(tt_executor::agents::Policy::Autonomous);
    let (c, _) = support::Client::connect(&h, opts, json!({}));
    let p = Protocol { handle: h, client: RefCell::new(Some(c)) };
    let e = ok(&p, "project.open", json!({"path":path,"readOnly":true}));
    let done = ok(&p, "job.wait", json!({"jobId":e["jobId"],"timeoutMs":60000}));
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(ok(&p, "doc.state.get", json!({}))["dirty"], false);
    p
}
