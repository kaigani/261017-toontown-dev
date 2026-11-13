//! Independent round-two desktop shell review, part B.
#![allow(clippy::unwrap_used, reason = "review assertions")]
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, UserDirs};
fn root() -> PathBuf {
    std::env::temp_dir().join(format!("tt-part-b-{}", tt_executor::random_token(8)))
}
fn over(r: &Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions { user_dirs: Some(UserDirs::under(r)), ..Default::default() },
        ..Default::default()
    })
    .unwrap()
}
fn ok(h: &ExecutorHandle, cmd: &str, p: Value) -> Value {
    let e = h.invoke(Invocation::user(cmd, p));
    assert!(e.error.is_none(), "{cmd}: {e:?}");
    e.result.unwrap_or(Value::Null)
}
mod part_b_preferences {
    use super::*;
    #[test]
    fn stale_policy_member_does_not_poison_other_members() {
        let r = root();
        let d = UserDirs::under(&r).settings;
        tt_persist::settings::write(
            &d,
            "preferences",
            &json!({"agent.policy.defaults":{"maxJobs":99,"maxBatchSteps":42}}),
        )
        .unwrap();
        let h = over(&r);
        let v = ok(&h, "prefs.get", json!({"keys":["agent.policy.defaults"]}));
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
        assert_eq!(v["values"]["agent.policy.defaults"]["maxJobs"], 2);
        assert_eq!(v["values"]["agent.policy.defaults"]["maxBatchSteps"], 42);
    }
    #[test]
    fn empty_arguments_never_panic_or_create_empty_bindings() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        let registry = tt_executor::registry().unwrap();
        for entry in registry
            .entries()
            .filter(|e| ["keymap.", "prefs.", "workspace.", "window."].iter().any(|p| e.id().starts_with(p)))
        {
            for params in [
                json!([]),
                json!({}),
                json!({"commandId":""}),
                json!({"keys":""}),
                json!({"keys":[]}),
                json!({"values":{}}),
            ] {
                let mut inv = Invocation::user(entry.id(), params);
                inv.options.dry_run = true;
                let e = h.invoke(inv);
                if let Some(error) = e.error {
                    assert_ne!(error.code, "internal", "{}: {error:?}", entry.id());
                }
            }
        }
        let map = ok(&h, "keymap.list", json!({}));
        for b in map.as_array().unwrap() {
            assert!(!b["commandId"].as_str().unwrap().is_empty());
            assert!(!b["keys"].as_str().unwrap().is_empty());
        }
    }
    #[test]
    fn partial_policy_preserves_stored_members() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        ok(&h, "prefs.set", json!({"values":{"agent.policy.defaults":{"maxJobs":7}}}));
        ok(&h, "prefs.set", json!({"values":{"agent.policy.defaults":{"maxBatchSteps":42}}}));
        let v = ok(&h, "prefs.get", json!({"keys":["agent.policy.defaults"]}));
        assert_eq!(v["values"]["agent.policy.defaults"]["maxJobs"], 7, "partial policy must retain the stored choice");
    }
    #[test]
    fn policy_bounds_both_paths() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        for (member, min, max) in [
            ("maxJobs", 1i64, 8i64),
            ("maxBatchSteps", 1, 100000),
            ("maxInlineImagePx", 256, 8192),
            ("maxInlineBytes", 1, 1073741824),
        ] {
            for n in [min - 1, min, max, max + 1] {
                for cmd in ["prefs.set", "agent.policy.defaults.set"] {
                    let obj = json!({member:n});
                    let p = if cmd == "prefs.set" { json!({"values":{"agent.policy.defaults":obj}}) } else { obj };
                    let e = h.invoke(Invocation::user(cmd, p));
                    assert_eq!(e.error.is_none(), (min..=max).contains(&n), "{cmd} {member} {n}: {e:?}");
                }
            }
        }
    }
    #[test]
    fn prefs_noops_write_nothing_and_agent_source() {
        let r = root();
        let h = over(&r);
        let ev = h.subscribe();
        ok(&h, "prefs.set", json!({"values":{"interface.theme":"system"}}));
        ok(&h, "prefs.reset", json!({"keys":["interface.theme"]}));
        assert!(!r.join("Settings/preferences.json").exists());
        assert!(ev.try_iter().all(|e| e.name != "prefs.changed"));
        let mut inv = Invocation::user(
            "prefs.set",
            json!({"values":{"interface.theme":"dark","files.backups.count":4,"interface.statusBar.visible":true}}),
        );
        inv.origin = tt_executor::Origin::Agent {
            session: "review".into(),
            name: "review".into(),
            scopes: [tt_registry::Scope::Prefs].into_iter().collect(),
        };
        assert!(h.invoke(inv).error.is_none());
        let changes: Vec<_> = ev.try_iter().filter(|e| e.name == "prefs.changed").collect();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].payload["source"], "agent");
        let mut keys = changes[0].payload["keys"].as_array().unwrap().clone();
        keys.sort_by_key(Value::to_string);
        assert_eq!(keys, vec![json!("files.backups.count"), json!("interface.theme")]);
        let p = r.join("Settings/preferences.json");
        let before = std::fs::read(&p).unwrap();
        let modified = std::fs::metadata(&p).unwrap().modified().unwrap();
        ok(&h, "prefs.set", json!({"values":{"interface.theme":"dark"}}));
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), modified);
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn import_errors_are_indexed_atomic_and_identical_import_is_silent() {
        let r = root();
        let h = over(&r);
        let p = r.join("keys.json");
        ok(&h, "keymap.set", json!({"commandId":"project.save","keys":"F2"}));
        let disk = std::fs::read(r.join("Settings/keymap.json")).unwrap();
        let before = ok(&h, "keymap.list", json!({}));
        for mode in ["merge", "replace"] {
            for (command, keys, code) in
                [("", "F3", "keymap.unknownCommand"), ("project.save", "", "keymap.invalidSequence")]
            {
                let data = json!({"format":"toontown-keymap","formatVersion":"1.0","bindings":[{"commandId":"project.save","keys":"F4","context":"global"},{"commandId":command,"keys":keys,"context":"global"}]});
                std::fs::write(&p, data.to_string()).unwrap();
                let e = h.invoke(Invocation::user("keymap.import", json!({"path":p,"mode":mode}))).error.unwrap();
                assert_eq!(e.code, code);
                assert_eq!(e.details["index"], 1);
                assert_eq!(ok(&h, "keymap.list", json!({})), before);
                assert_eq!(std::fs::read(r.join("Settings/keymap.json")).unwrap(), disk);
            }
        }
        ok(&h, "keymap.export", json!({"path":p,"onExists":"overwrite"}));
        let ev = h.subscribe();
        ok(&h, "keymap.import", json!({"path":p,"mode":"replace"}));
        assert!(ev.try_iter().all(|e| e.name != "keymap.changed"));
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
}
mod part_b_settings {
    use super::*;
    #[test]
    fn versions_and_once_notices() {
        for version in [Some(json!("1.0")), Some(json!("1.99")), Some(json!("0.9")), Some(json!(1)), None] {
            let r = root();
            let d = UserDirs::under(&r).settings;
            std::fs::create_dir_all(&d).unwrap();
            let mut v = json!({"format":"toontown-settings","kind":"preferences","data":{"interface.theme":"dark","future.extra":42}});
            if let Some(x) = &version {
                v["formatVersion"] = x.clone();
            }
            std::fs::write(d.join("preferences.json"), v.to_string()).unwrap();
            let h = over(&r);
            let ev = h.subscribe();
            let prefs = ok(&h, "prefs.get", json!({"keys":["interface.theme"]}));
            let valid = version.as_ref().and_then(Value::as_str).is_some_and(|s| s.starts_with("1."));
            assert_eq!(prefs["values"]["interface.theme"], if valid { "dark" } else { "system" });
            let notices: Vec<_> = ev.try_iter().filter(|e| e.payload["messageId"] == "prefs.corrupt").collect();
            assert_eq!(notices.len(), usize::from(!valid));
            for n in notices {
                assert!(Path::new(n.payload["details"]["keptAt"].as_str().unwrap()).exists());
            }
            drop(h);
            let h = over(&r);
            let ev = h.subscribe();
            ok(&h, "app.state.get", json!({}));
            assert!(ev.try_iter().all(|e| e.payload["messageId"] != "prefs.corrupt"));
            drop(h);
            std::fs::remove_dir_all(r).unwrap();
        }
    }
    #[test]
    fn cleanup_lock_is_nonblocking_and_released() {
        let r = root();
        std::fs::create_dir_all(&r).unwrap();
        let lock =
            std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(r.join(".settings.lock")).unwrap();
        lock.lock_shared().unwrap();
        let p = r.join(".preferences.json.abcd.tmp");
        std::fs::write(&p, b"live").unwrap();
        let start = std::time::Instant::now();
        assert_eq!(tt_persist::settings::clean_temporaries(&r), 0);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        assert!(p.exists());
        drop(lock);
        assert_eq!(tt_persist::settings::clean_temporaries(&r), 1);
        assert!(r.join(".settings.lock").exists());
        let mut copies = Vec::new();
        for text in ["first", "second"] {
            std::fs::write(r.join("preferences.json"), text).unwrap();
            copies.push(tt_persist::settings::set_aside(&r.join("preferences.json")).unwrap());
        }
        assert_ne!(copies[0], copies[1]);
        assert_eq!(std::fs::read_to_string(&copies[0]).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(&copies[1]).unwrap(), "second");
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn failed_write_retries_after_folder_repair() {
        let r = root();
        std::fs::create_dir_all(&r).unwrap();
        std::fs::write(r.join("Settings"), b"blocked").unwrap();
        let h = over(&r);
        let ev = h.subscribe();
        for theme in ["dark", "light"] {
            ok(&h, "prefs.set", json!({"values":{"interface.theme":theme}}));
        }
        let notices: Vec<_> = ev.try_iter().filter(|e| e.payload["messageId"] == "prefs.saveFailed").collect();
        let failed: Vec<_> = notices.iter().filter(|e| e.payload["details"]["kind"] == "preferences").collect();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].payload["details"]["path"].as_str().unwrap().ends_with("preferences.json"));
        assert!(!failed[0].payload["details"]["message"].as_str().unwrap().is_empty());
        std::fs::remove_file(r.join("Settings")).unwrap();
        ok(&h, "prefs.set", json!({"values":{"interface.theme":"dark"}}));
        let disk: Value = serde_json::from_slice(&std::fs::read(r.join("Settings/preferences.json")).unwrap()).unwrap();
        assert_eq!(disk["data"]["interface.theme"], "dark");
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
}
