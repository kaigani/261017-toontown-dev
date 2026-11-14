//! Independent checkpoint review, part B.
#![allow(clippy::unwrap_used, reason = "review assertions")]
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, UserDirs};
mod part_b_availability {
    use super::*;
    #[test]
    fn selection_and_empty_state_parity() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        let mut mismatches = Vec::new();
        for state in 0..3 {
            if state == 1 {
                ok(&h, "project.create", json!({"name":"Empty"}));
            }
            if state == 2 {
                assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
                ok(&h, "edit.selection.set", json!({"context":"staging","ids":["scene:\"Intro\"/element:\"Bob\""]}));
            }
            for id in [
                "edit.copy",
                "edit.cut",
                "edit.cutSpecial",
                "edit.delete",
                "edit.paste",
                "edit.selectAll",
                "edit.undo",
                "edit.redo",
                "project.save",
                "project.close",
            ] {
                let params = if id.starts_with("project.") || ["edit.undo", "edit.redo"].contains(&id) {
                    json!({})
                } else {
                    json!({"context":"staging"})
                };
                let a = h.availability(vec![(id.into(), params.as_object().unwrap().clone())]).unwrap().remove(0);
                if a.dialog {
                    continue;
                }
                let mut inv = Invocation::user(id, a.params.clone());
                inv.options.dry_run = true;
                let e = h.invoke(inv);
                let form_refusal = e
                    .error
                    .as_ref()
                    .is_some_and(|e| ["project.pathRequired", "project.unsavedChanges"].contains(&e.code.as_str()));
                if a.enabled != e.is_ok() && !form_refusal {
                    mismatches.push(format!("state {state} {id}: {a:?} {e:?}"));
                }
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }
}
fn ok(h: &ExecutorHandle, id: &str, p: Value) -> Value {
    let e = h.invoke(Invocation::user(id, p));
    assert!(e.error.is_none(), "{id}: {e:?}");
    e.result.unwrap_or(Value::Null)
}
mod part_b_preferences {
    use super::*;
    #[test]
    fn composites_atomic_roundtrip_and_stale_members() {
        for (key, a, b, valid) in [
            ("edit.pasteSpecial.timeline", "overwriteDrawings", "extendExposures", json!(true)),
            ("edit.cutSpecial.timeline", "overwriteDrawings", "extendExposures", json!(true)),
            ("edit.dragDrop.timeline", "overwriteDrawings", "extendExposures", json!(true)),
            ("edit.pasteSpecial.motionPoint", "x", "y", json!(false)),
            ("agent.policy.defaults", "maxJobs", "maxBatchSteps", json!(42)),
        ] {
            let root = std::env::temp_dir().join(format!("tt-r3-{}", tt_executor::random_token(8)));
            let dirs = UserDirs::under(&root);
            let spawn = || {
                ExecutorHandle::spawn_with_options(ExecutorOptions {
                    persist: PersistOptions { user_dirs: Some(dirs.clone()), ..Default::default() },
                    ..Default::default()
                })
                .unwrap()
            };
            let h = spawn();
            let get = |h: &ExecutorHandle| ok(h, "prefs.get", json!({"keys":[key]}))["values"][key].clone();
            let defaults = get(&h);
            ok(&h, "prefs.set", json!({"values":{key:{b:valid}}}));
            let current = get(&h);
            let path = dirs.settings.join("preferences.json");
            let bytes = std::fs::read(&path).unwrap();
            for bad in [Value::Null, json!("invalid")] {
                assert!(
                    h.invoke(Invocation::user(
                        "prefs.set",
                        json!({"values":{key:{a:bad,b:defaults[b]},"interface.theme":"dark"}})
                    ))
                    .error
                    .is_some()
                );
                assert_eq!(get(&h), current);
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
            }
            assert!(h.invoke(Invocation::user("prefs.set", json!({"values":{key:{"unknown":true}}}))).error.is_some());
            let events = h.subscribe();
            ok(&h, "prefs.set", json!({"values":{key:{b:valid}}}));
            assert!(events.try_iter().all(|e| e.name != "prefs.changed"));
            drop(h);
            let h = spawn();
            assert_eq!(get(&h), current);
            ok(&h, "prefs.set", json!({"values":{key:{b:defaults[b]}}}));
            assert_eq!(get(&h), defaults);
            ok(&h, "prefs.set", json!({"values":{key:{b:valid}}}));
            ok(&h, "prefs.reset", json!({"keys":[key]}));
            assert_eq!(get(&h), defaults);
            drop(h);
            tt_persist::settings::write(&dirs.settings, "preferences", &json!({key:{a:null,b:valid,"unknown":true}}))
                .unwrap();
            let h = spawn();
            let v = get(&h);
            assert_eq!(v[a], defaults[a]);
            assert_eq!(v[b], valid);
            assert!(v.get("unknown").is_none());
            drop(h);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
