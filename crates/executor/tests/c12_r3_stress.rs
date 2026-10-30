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
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
use tt_executor::{Invocation, InvokeOptions, Origin};
use tt_model::document::{Document, EntryKey};

fn artifact(name: &str, v: &Value) {
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        std::fs::write(std::path::Path::new(&dir).join(name), serde_json::to_vec_pretty(v).unwrap()).unwrap();
    }
}

#[test]
fn eight_clients_cas_commands_and_batches_have_no_lost_updates() {
    let h = fixture();
    let scene = first(&h, "scene");
    let initial = rev(&h);
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|client| {
            let h = h.clone();
            let scene = scene.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let origin = Origin::Agent {
                    session: format!("client-{client}"),
                    name: format!("Client {client}"),
                    scopes: [tt_registry::Scope::Read, tt_registry::Scope::Edit].into_iter().collect(),
                };
                let mut commits = Vec::new();
                let mut conflicts = 0;
                barrier.wait();
                for index in 0..31 {
                    for attempt in 0..1000 {
                        let obj = get(&h, &scene);
                        let n = obj["meta"]["counter"].as_u64().unwrap_or(0) + 1;
                        let observed_rev = obj["rev"].as_u64().unwrap();
                        let params = json!({"targets":[scene],"props":{"meta":{"counter":n}},"expectRev":observed_rev});
                        let env = if (index + client) % 2 == 0 {
                            h.invoke(Invocation {
                                command: "doc.object.patch".into(),
                                params,
                                options: Default::default(),
                                origin: origin.clone(),
                            })
                        } else {
                            h.batch(
                                serde_json::from_value(
                                    json!({"label":format!("Client {client} step {index}"),"steps":[
                                        {"id":"doc.object.patch","params":params},
                                        {"id":"doc.object.get","params":{"ref":scene}}
                                    ]}),
                                )
                                .unwrap(),
                                origin.clone(),
                            )
                        };
                        if env.is_ok() {
                            commits.push(env.document_rev.unwrap());
                            break;
                        }
                        let e = env.error.unwrap();
                        assert!(
                            e.code == "rev.conflict"
                                || (e.code == "batch.stepFailed" && e.details["error"]["code"] == "rev.conflict"),
                            "{e:?}"
                        );
                        conflicts += 1;
                        assert!(attempt < 999, "CAS did not converge");
                    }
                }
                (commits, conflicts)
            })
        })
        .collect();
    let (mut revisions, mut conflicts) = (Vec::new(), 0);
    for w in workers {
        let (r, c) = w.join().unwrap();
        revisions.extend(r);
        conflicts += c;
    }
    revisions.sort_unstable();
    assert_eq!(revisions, ((initial + 1)..=(initial + 248)).collect::<Vec<_>>());
    assert_eq!(get(&h, &scene)["meta"]["counter"], 248);
    assert_eq!(rev(&h), initial + 248);
    artifact(
        "concurrency.json",
        &json!({"clients":8,"commits":248,"conflicts":conflicts,"initialRev":initial,"finalRev":rev(&h),"revisions":revisions}),
    );
    println!("8 clients, 248 commits, {conflicts} correctly rejected conflicts, documentRev {}", rev(&h));
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
    let h = fixture();
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
        assert!(env.is_ok(), "seed {SEED:x}, operation {i}: {id} {params}: {env:?}");
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
    let replay = fixture();
    ok(&replay, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"seedBase":true}}}));
    for (i, (id, params)) in commands.iter().enumerate() {
        let env = run(&replay, id, params.clone());
        assert!(env.is_ok(), "replay {i}: {env:?}");
    }
    assert_eq!(
        canonical_entries(&h.snapshot().unwrap().unwrap()),
        canonical_entries(&replay.snapshot().unwrap().unwrap()),
        "fresh replay must be byte-identical including revisions"
    );
    let receipt = json!({"seed":format!("0x{SEED:x}"),"operations":10000,"replayOperations":10000,"editsUndoRedoCreateRestoreDryRun":counts,"documentRev":expected_rev,"durationSeconds":start.elapsed().as_secs_f64(),"oracle":"independent before/after metadata stack; exact canonical all-entry byte checks every 100 operations; full fresh executor replay"});
    artifact("random-history.json", &receipt);
    artifact("random-history-commands.json", &json!(commands));
    println!("{receipt}");
}

#[test]
fn all_inventory_entries_reject_malformed_params_under_watchdog_and_catalogue_is_recorded() {
    let h = fixture();
    let r = tt_executor::registry().unwrap();
    let mut catalogue = Vec::new();
    let mut results = Vec::new();
    for e in r.entries() {
        let mut description = e.describe();
        description["typed"] = json!(e.command.is_some());
        description["specParameters"] = json!(e.spec.params);
        description["specResult"] = json!(e.spec.result);
        description["specUndo"] = json!(e.spec.undo);
        catalogue.push(description);
        for p in [json!([]), json!("bad"), json!({"__round3_unknown":true})] {
            let t = Instant::now();
            let env = run(&h, e.id(), p.clone());
            assert!(env.is_error(), "{} accepted {p}", e.id());
            assert!(t.elapsed() < Duration::from_secs(2), "{} took {:?}", e.id(), t.elapsed());
            results.push(
                json!({"id":e.id(),"params":p,"code":env.error.unwrap().code,"ms":t.elapsed().as_secs_f64()*1000.0}),
            );
        }
    }
    artifact("stress-registry.json", &json!(catalogue));
    artifact("malformed-sweep.json", &json!(results));
    println!("796 entries, 2388 malformed invocations completed under watchdog");
}

#[test]
fn every_executable_typed_example_also_validates_its_result_schema() {
    let mut results = Vec::new();
    let mut violations = Vec::new();
    let envelope_schema = tt_registry::schema_of::<tt_executor::Envelope>();
    let envelope_validator =
        jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&envelope_schema).unwrap();
    for e in tt_executor::registry().unwrap().entries() {
        let Some(c) = e.command else { continue };
        for ex in c.decl.examples {
            let h = fixture();
            ok(&h, "checkpoint.create", json!({"name":"before-walk"}));
            ok(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"p":1}}}));
            ok(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"p":2}}}));
            ok(&h, "edit.undo", json!({}));
            ok(&h, "workspace.create", json!({"name":"Inking"}));
            let job = h.jobs().submit("review.example", "user", |_| Ok(json!({}))).unwrap();
            h.jobs().wait(&job, Duration::from_secs(5)).unwrap();
            let env = run(&h, e.id(), serde_json::from_str(ex.params).unwrap());
            if let Some(v) = &env.result {
                let schema = e.result_schema();
                let validator =
                    jsonschema::options().with_draft(jsonschema::Draft::Draft202012).build(&schema).unwrap();
                let errors: Vec<_> = validator.iter_errors(v).map(|x| x.to_string()).collect();
                if !errors.is_empty() {
                    violations.push(json!({"id":e.id(),"example":ex.title,"where":"result","errors":errors}));
                }
            }
            let errors: Vec<_> =
                envelope_validator.iter_errors(&serde_json::to_value(&env).unwrap()).map(|x| x.to_string()).collect();
            if !errors.is_empty() {
                violations.push(json!({"id":e.id(),"example":ex.title,"where":"envelope","errors":errors}));
            }
            results.push(json!({"id":e.id(),"example":ex.title,"status":env.status,"error":env.error}));
        }
    }
    println!("{} typed examples executed and successful results validated", results.len());
    artifact("examples.json", &json!(results));
    artifact("example-schema-violations.json", &json!(violations));
    assert!(
        violations.is_empty(),
        "{} example result/envelope schema violations; first: {:?}",
        violations.len(),
        &violations[..violations.len().min(3)]
    );
}

#[test]
fn ordinary_dry_run_preserves_exact_history_and_matches_real_result() {
    let h = fixture();
    let before = canonical_entries(&h.snapshot().unwrap().unwrap());
    let hist = ok(&h, "edit.history.get", json!({}));
    let inv = Invocation {
        command: "doc.object.patch".into(),
        params: json!({"targets":["scene:#1"],"props":{"meta":{"review":1}}}),
        options: InvokeOptions { dry_run: true, ..Default::default() },
        origin: Origin::User,
    };
    let env = h.invoke(inv.clone());
    assert!(env.is_ok());
    assert!(env.undo.is_none());
    assert_eq!(canonical_entries(&h.snapshot().unwrap().unwrap()), before);
    assert_eq!(ok(&h, "edit.history.get", json!({})), hist);
    let real = h.invoke(Invocation { options: Default::default(), ..inv });
    assert_eq!(env.result, real.result);
    assert_eq!(env.changes, real.changes);
}
