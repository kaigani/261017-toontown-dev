//! Independent checkpoint review, part B.
#![allow(clippy::unwrap_used, reason = "review assertions")]
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, UserDirs};
mod part_b_recovery {
    use super::*;
    #[test]
    fn unrelated_live_pid_is_conservatively_kept() {
        let root = std::env::temp_dir().join(format!("tt-r3-pid-{}", tt_executor::random_token(8)));
        let folder = root.join("Recovery/rc_abcd0003");
        std::fs::create_dir_all(&folder).unwrap();
        #[cfg(windows)]
        let mut child = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "$null = [Console]::ReadLine()"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        #[cfg(not(windows))]
        let mut child = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::fs::write(folder.join("engine"), format!("en_other\n{}\n{}", child.id(), tt_persist::host::host()))
            .unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(folder.join("engine")).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH)).unwrap();
        drop(file);
        let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
            persist: PersistOptions { user_dirs: Some(UserDirs::under(&root)), ..Default::default() },
            ..Default::default()
        })
        .unwrap();
        let kept = folder.exists();
        drop(h);
        drop(child.stdin.take());
        child.wait().unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(kept, "a live PID is conservatively kept even when its record is stale");
    }
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    #[test]
    fn release_before_write_begins_preserves_both_owners() {
        let root = std::env::temp_dir().join(format!("tt-r3-fence-{}", tt_executor::random_token(8)));
        let doc = tt_model::fixture::sample_document(1).unwrap();
        let preserved = Default::default();
        let blobs = Default::default();
        let input = tt_persist::save::SaveInput {
            doc: &doc,
            preserved: &preserved,
            blobs: &blobs,
            session: None,
            saved_at: "2026-11-13T00:00:00Z".into(),
        };
        let mut info: tt_persist::recovery::RecoveryInfo = serde_json::from_value(json!({"recoveryId":"rc_abcd0001","projectId":"pr_test","projectName":"Review","originalPath":null,"savedAt":"2026-11-13T00:00:00Z","documentRev":1,"appVersion":"0","processId":std::process::id(),"host":tt_persist::host::host(),"heartbeatAt":"2026-11-13T00:00:00Z"})).unwrap();
        let fence = AtomicBool::new(false);
        assert!(tt_persist::recovery::write_owned(&root, &info, &input, "en_old", &fence).unwrap().is_some());
        fence.store(true, Ordering::Release);
        assert_eq!(tt_persist::recovery::release_engine(&root, "en_old", None), 1);
        let old_path = root.join(&info.recovery_id).join("recovery.json");
        let old = std::fs::read(&old_path).unwrap();
        info.recovery_id = "rc_abcd0002".into();
        assert!(
            tt_persist::recovery::write_owned(&root, &info, &input, "en_new", &AtomicBool::new(false))
                .unwrap()
                .is_some()
        );
        let new_path = root.join(&info.recovery_id).join("recovery.json");
        let new = std::fs::read(&new_path).unwrap();
        let reached = Arc::new(AtomicUsize::new(0));
        let count = reached.clone();
        tt_persist::lock::hooks::set(Some(Arc::new(move |boundary, _| {
            if boundary == tt_persist::lock::Boundary::RecoveryWriteChecked {
                count.fetch_add(1, Ordering::SeqCst);
            }
        })));
        info.recovery_id = "rc_abcd0001".into();
        let result = tt_persist::recovery::write_owned(&root, &info, &input, "en_old", &fence);
        tt_persist::lock::hooks::set(None);
        assert!(result.unwrap().is_none());
        assert_eq!(reached.load(Ordering::SeqCst), 0);
        assert_eq!(std::fs::read(old_path).unwrap(), old);
        assert_eq!(std::fs::read(new_path).unwrap(), new);
        std::fs::remove_dir_all(root).unwrap();
    }
}
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
