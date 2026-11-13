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
mod part_b_splitter {
    use super::*;
    #[test]
    fn all_builtin_tabs_resize_only_area_and_noop_is_silent() {
        let r = root();
        let h = over(&r);
        let names = ok(&h, "workspace.list", json!({}));
        let mut checked = 0;
        for entry in names.as_array().unwrap().iter().filter(|w| w["builtIn"] == true) {
            ok(&h, "workspace.select", json!({"name":entry["name"]}));
            for area in ["left", "right", "top", "bottom"] {
                let layout = ok(&h, "workspace.get", json!({}));
                let Some(groups) = layout["docks"][area].as_array() else { continue };
                let panels: Vec<_> =
                    groups.iter().flat_map(|g| g["tabs"].as_array().unwrap().iter().cloned()).collect();
                for panel in panels {
                    ok(&h, "workspace.save", json!({}));
                    let path = r.join("Settings/workspaces.json");
                    let disk = std::fs::read(&path).ok();
                    let modified = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                    let before = ok(&h, "workspace.get", json!({}));
                    let ev = h.subscribe();
                    let mut p = json!({"panelId":panel,"area":area,"relativeTo":panel,"position":"tab"});
                    ok(&h, "window.panel.move", p.clone());
                    assert_eq!(ok(&h, "workspace.get", json!({})), before);
                    assert!(ev.try_iter().all(|e| e.name != "workspace.changed"));
                    assert_eq!(std::fs::read(&path).ok(), disk);
                    assert_eq!(std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()), modified);
                    for size in [1, 32767, 333] {
                        p["size"] = json!(size);
                        let old = ok(&h, "workspace.get", json!({}));
                        ok(&h, "window.panel.move", p.clone());
                        let mut expected = old.clone();
                        expected["docks"][area][0]["size"] = json!(size);
                        assert_eq!(ok(&h, "workspace.get", json!({})), expected, "{} {area} {panel}", entry["name"]);
                        let count = ev.try_iter().filter(|e| e.name == "workspace.changed").count();
                        assert_eq!(count, usize::from(old != expected));
                        ok(&h, "window.panel.move", p.clone());
                        assert!(ev.try_iter().all(|e| e.name != "workspace.changed"));
                    }
                    for size in [0, 32768] {
                        p["size"] = json!(size);
                        assert!(h.invoke(Invocation::user("window.panel.move", p.clone())).error.is_some());
                    }
                    p.as_object_mut().unwrap().remove("size");
                    for position in ["before", "after", "above", "below"] {
                        p["position"] = json!(position);
                        let e = h.invoke(Invocation::user("window.panel.move", p.clone())).error.unwrap();
                        assert_eq!(e.code, "arg.invalid");
                    }
                    p["position"] = json!("tab");
                    p["area"] = json!(if area == "left" { "right" } else { "left" });
                    assert_eq!(h.invoke(Invocation::user("window.panel.move", p)).error.unwrap().code, "arg.invalid");
                    checked += 1;
                }
            }
        }
        assert!(checked >= 6);
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn second_group_resize_and_saved_restore() {
        let r = root();
        let h = over(&r);
        let mut layout = ok(&h, "workspace.get", json!({}));
        layout["docks"]["left"] = json!([{"tabs":["history"],"current":"history","size":200},{"tabs":["colour","library"],"current":"library","size":100}]);
        // Remove these panels from their original places before applying the fixture.
        for area in ["right", "top", "bottom"] {
            if let Some(groups) = layout["docks"][area].as_array_mut() {
                groups.retain_mut(|g| {
                    g["tabs"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|p| ![json!("history"), json!("colour"), json!("library")].contains(p));
                    if g["tabs"].as_array().unwrap().is_empty() {
                        return false;
                    }
                    if !g["tabs"].as_array().unwrap().contains(&g["current"]) {
                        g["current"] = g["tabs"][0].clone();
                    }
                    true
                });
            }
        }
        ok(&h, "workspace.layout.set", json!({"layout":layout}));
        for panel in ["colour", "library"] {
            let before = ok(&h, "workspace.get", json!({}));
            ok(
                &h,
                "window.panel.move",
                json!({"panelId":panel,"relativeTo":panel,"position":"tab","area":"left","size":444}),
            );
            let mut expected = before;
            expected["docks"]["left"][0]["size"] = json!(444);
            assert_eq!(ok(&h, "workspace.get", json!({})), expected);
        }
        let current = ok(&h, "workspace.get", json!({}));
        let list = ok(&h, "workspace.list", json!({}));
        assert!(list.as_array().unwrap().iter().any(|e| e["modified"] == true));
        // Workspace UI changes do not add document undo steps.
        assert!(h.invoke(Invocation::user("edit.undo", json!({}))).error.is_some());
        assert!(h.invoke(Invocation::user("edit.redo", json!({}))).error.is_some());
        assert_eq!(ok(&h, "workspace.get", json!({})), current);
        ok(&h, "workspace.save", json!({}));
        drop(h);
        let h = over(&r);
        assert_eq!(ok(&h, "workspace.get", json!({})), current);
        drop(h);
        std::fs::remove_dir_all(r).unwrap();
    }
    #[test]
    fn self_relative_special_panels_remain_refused() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        for (panel, area) in [("cameraView", "central"), ("no-such-panel", "left")] {
            assert!(
                h.invoke(Invocation::user(
                    "window.panel.move",
                    json!({"panelId":panel,"relativeTo":panel,"position":"tab","area":area,"size":333})
                ))
                .error
                .is_some()
            );
        }
        ok(&h, "window.panel.set", json!({"panelId":"history","floating":true}));
        assert_eq!(
            h.invoke(Invocation::user(
                "window.panel.move",
                json!({"panelId":"history","relativeTo":"history","position":"tab","area":"left","size":333})
            ))
            .error
            .unwrap()
            .code,
            "arg.invalid"
        );
        ok(&h, "window.panel.set", json!({"panelId":"history","visible":false}));
        assert!(
            h.invoke(Invocation::user(
                "window.panel.move",
                json!({"panelId":"history","relativeTo":"history","position":"tab","area":"left","size":333})
            ))
            .error
            .is_some()
        );
    }
}

mod part_c1_freshness {
    use super::*;
    use std::time::{Duration, Instant};
    use tt_executor::host::ShellReport;
    #[test]
    fn report_order_and_wait_bounds() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        ok(&h, "prefs.set", json!({"values":{"interface.theme":"dark"}}));
        let seq = h.events_broadcast();
        let report = |label: &str, n| {
            h.report_shell(ShellReport {
                attached: Some(true),
                ui_tree: Some(json!({"id":"main","label":label})),
                rendered_seq: n,
                ..Default::default()
            })
        };
        for (label, n, want, fresh) in [
            ("old", Some(0), "old", false),
            ("new", Some(seq), "new", true),
            ("older", Some(0), "new", true),
            ("equal", Some(seq), "equal", true),
            ("future", Some(seq + 1), "equal", true),
            ("none", None, "none", false),
        ] {
            report(label, n);
            assert_eq!(ok(&h, "inspect.ui.tree", json!({}))["root"]["label"], want);
            assert_eq!(h.await_rendered(Duration::ZERO), fresh);
        }
        let start = Instant::now();
        assert!(!h.await_rendered(Duration::from_secs(2)));
        assert!((Duration::from_millis(1950)..Duration::from_secs(3)).contains(&start.elapsed()));
        let other = h.clone();
        let waiter = std::thread::spawn(move || other.await_rendered(Duration::from_secs(2)));
        assert!(h.invoke(Invocation::user("app.state.get", json!({}))).is_ok());
        let release = Instant::now();
        report("fresh", Some(seq));
        assert!(waiter.join().unwrap());
        assert!(release.elapsed() < Duration::from_millis(500));
    }
    #[test]
    fn tree_exposes_its_vouching_sequence() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        let seq = h.events_broadcast();
        h.report_shell(ShellReport {
            attached: Some(true),
            ui_tree: Some(json!({"id":"main"})),
            rendered_seq: Some(seq),
            ..Default::default()
        });
        let tree = ok(&h, "inspect.ui.tree", json!({}));
        assert_eq!(tree["renderedSeq"], seq, "the agent must know which events this tree reflects: {tree}");
    }
}

mod part_c1_restart {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn released_engine_cannot_refresh_and_third_cannot_steal() {
        let r = root();
        let first = over(&r);
        let c = ok(&first, "project.create", json!({"name":"Recovery fence"}));
        ok(&first, "doc.object.patch", json!({"targets":[c["sceneId"]],"props":{"meta":{"value":"Before"}}}));
        first.run_maintenance(true).unwrap();
        let start = Instant::now();
        let file = loop {
            if let Some(p) = std::fs::read_dir(r.join("Recovery"))
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.path().join("recovery.json"))
                .find(|p| p.exists())
            {
                break p;
            }
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::yield_now();
        };
        let second = over(&r);
        let owner = first.abandon();
        assert_eq!(second.release_recovery_of(&owner).unwrap(), 1);
        let before = std::fs::read(&file).unwrap();
        ok(&first, "doc.object.patch", json!({"targets":[c["sceneId"]],"props":{"meta":{"value":"After"}}}));
        first.run_maintenance(true).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let list = ok(&second, "project.recovery.list", json!({}));
        let started = ok(&second, "project.recovery.open", json!({"recoveryId":list[0]["recoveryId"]}));
        ok(&second, "job.wait", json!({"jobId":started["jobId"],"timeoutMs":60000}));
        let claimed = std::fs::read(&file).unwrap();
        let third = over(&r);
        assert_eq!(third.release_recovery_of(&owner).unwrap(), 0);
        assert_eq!(std::fs::read(file).unwrap(), claimed);
        drop(third);
        drop(second);
        drop(first);
        std::fs::remove_dir_all(r).unwrap();
    }
}

mod part_c1_availability {
    use super::*;
    #[test]
    fn disabled_key_has_typed_refusal() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        assert!(h.resolve_key("Primary+Z").is_err(), "disabled Undo must give a typed refusal");
    }
    #[test]
    fn availability_matches_dry_run_in_three_states() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        let registry = tt_executor::registry().unwrap();
        let mut mismatches = Vec::new();
        for state in 0..3 {
            if state == 1 {
                ok(&h, "project.create", json!({"name":"Availability"}));
            }
            if state == 2 {
                ok(&h, "project.properties.set", json!({"frameRate":12}));
            }
            for entry in registry.entries() {
                let a = h.availability(vec![(entry.id().to_owned(), Default::default())]).unwrap().remove(0);
                if !a.enabled || !a.dialog {
                    let mut inv = Invocation::user(entry.id(), a.params.clone());
                    inv.options.dry_run = true;
                    let e = h.invoke(inv);
                    if !a.enabled && e.is_ok() {
                        mismatches.push(format!("state {state} {} disabled but succeeds", entry.id()));
                    }
                    if a.enabled
                        && let Some(err) = e.error
                        && ["command.disabled", "project.notOpen", "command.notImplemented"]
                            .contains(&err.code.as_str())
                    {
                        mismatches.push(format!("state {state} {} enabled but {}", entry.id(), err.code));
                    }
                }
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }
    #[test]
    fn undo_redo_labels_and_toggle_follow_state() {
        let h = ExecutorHandle::spawn(Default::default()).unwrap();
        let facts = || {
            h.availability(vec![
                ("edit.undo".into(), Default::default()),
                ("edit.redo".into(), Default::default()),
                ("ui.statusBar.set".into(), Default::default()),
            ])
            .unwrap()
        };
        assert!(!facts()[0].enabled);
        ok(&h, "project.create", json!({"name":"History"}));
        assert!(!facts()[0].enabled);
        ok(&h, "project.properties.set", json!({"frameRate":12}));
        assert!(facts()[0].enabled);
        assert!(facts()[0].label.as_ref().unwrap().starts_with("Undo "));
        ok(&h, "edit.undo", json!({}));
        assert!(facts()[1].enabled);
        assert!(facts()[1].label.as_ref().unwrap().starts_with("Redo "));
        ok(&h, "ui.statusBar.set", json!({"visible":false}));
        assert_eq!(facts()[2].checked, Some(false));
    }
}
