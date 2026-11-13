//! Independent C4.1 round 1 sensor wire contract regressions.
use serde_json::json;
use std::error::Error;
use tt_executor::host::ShellReport;
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin};

#[test]
fn closing_dialog_over_json_clears_modal_and_shortcuts() -> Result<(), Box<dyn Error>> {
    let handle = ExecutorHandle::spawn(Default::default())?;
    handle
        .report_shell(serde_json::from_value::<ShellReport>(json!({"modal":{"kind":"dialog","title":"Preferences"}}))?);
    let state = || {
        handle.invoke(Invocation {
            command: "app.state.get".into(),
            params: json!({}),
            options: InvokeOptions::default(),
            origin: Origin::User,
        })
    };
    assert_eq!(state().result.as_ref().and_then(|v| v.pointer("/modal/kind")), Some(&json!("dialog")));
    handle.report_shell(serde_json::from_value::<ShellReport>(json!({"modal":null}))?);
    let after = state();
    assert_eq!(
        after.result.as_ref().and_then(|v| v.get("modal")),
        Some(&serde_json::Value::Null),
        "closing the actual JSON IPC dialog must release user priority: {after:?}"
    );
    Ok(())
}

#[test]
fn unknown_settings_version_is_quarantined() -> Result<(), Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("tt-c41-review-{}", tt_executor::random_token(8)));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("preferences.json"),
        serde_json::to_vec(&json!({
            "format":"toontown-settings", "kind":"preferences", "formatVersion":"999.0", "data":{"interface.theme":"dark"}
        }))?,
    )?;
    let outcome = tt_persist::settings::read(&dir, "preferences");
    std::fs::remove_dir_all(&dir)?;
    assert!(matches!(outcome, tt_persist::settings::Read::Corrupt(_)), "unsupported version was accepted: {outcome:?}");
    Ok(())
}

mod part_c_priority {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};
    use tt_registry::Scope;

    fn agent(command: &str, params: serde_json::Value) -> Invocation {
        Invocation {
            command: command.into(),
            params,
            options: Default::default(),
            origin: Origin::Agent {
                session: "review".into(),
                name: "review".into(),
                scopes: [Scope::Read, Scope::Edit, Scope::Ui].into_iter().collect(),
            },
        }
    }
    fn modal(h: &ExecutorHandle, kind: &str) {
        h.report_shell(ShellReport {
            modal: Some(Some(tt_executor::shell::ModalReport { kind: kind.into(), title: "review".into() })),
            ..Default::default()
        });
    }
    #[test]
    fn silent_busy_report_has_bounded_calls_and_user_remains_free() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        for kind in ["gesture", "dialog"] {
            modal(&h, kind);
            h.set_wait_for_user(Duration::from_millis(80));
            let start = Instant::now();
            let e = h
                .invoke(agent("doc.object.patch", json!({"targets":["@current"],"props":{"name":"x"}})))
                .error
                .ok_or("missing busy error")?;
            let elapsed = start.elapsed();
            assert_eq!(e.code, "ui.busy");
            assert!(elapsed >= Duration::from_millis(80) && elapsed < Duration::from_secs(2), "{elapsed:?}");
            eprintln!("{kind} busy wait {elapsed:?}");
            let user_start = Instant::now();
            let env = h.invoke(Invocation::user("window.panel.set", json!({"panelId":"history","visible":true})));
            assert!(env.error.is_none(), "{env:?}");
            assert!(user_start.elapsed() < Duration::from_secs(2));
            h.set_wait_for_user(Duration::ZERO);
            let start = Instant::now();
            assert_eq!(
                h.invoke(agent("doc.object.patch", json!({"targets":["@current"],"props":{"name":"x"}})))
                    .error
                    .ok_or("missing error")?
                    .code,
                "ui.busy"
            );
            assert!(start.elapsed() < Duration::from_secs(1));
        }
        Ok(())
    }
    #[test]
    fn user_command_passes_waiting_agent_and_new_modal_is_observed() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        modal(&h, "gesture");
        h.set_wait_for_user(Duration::from_secs(5));
        let waiting = Arc::new(AtomicBool::new(false));
        let w = waiting.clone();
        let h2 = h.clone();
        let caller = std::thread::spawn(move || {
            h2.invoke_served_until(
                agent("doc.object.patch", json!({"targets":["@current"],"props":{"name":"x"}})),
                &|| {
                    w.store(true, Ordering::Release);
                    false
                },
            )
            .0
        });
        let start = Instant::now();
        while !waiting.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(2) {
            std::thread::yield_now();
        }
        let observed = waiting.load(Ordering::Acquire);
        modal(&h, "dialog");
        let user_start = Instant::now();
        let env = h.invoke(Invocation::user("window.panel.set", json!({"panelId":"history","visible":true})));
        let elapsed = user_start.elapsed();
        let blocked = h.invoke(agent("window.panel.set", json!({"panelId":"history","visible":false})));
        h.report_shell(ShellReport { modal: Some(None), ..Default::default() });
        let _finished = caller.join().map_err(|_| "caller panicked")?;
        assert!(observed);
        assert!(env.error.is_none(), "{env:?}");
        assert!(elapsed < Duration::from_secs(2), "user waited {elapsed:?}");
        assert_eq!(blocked.error.ok_or("missing focus refusal")?.code, "ui.modalOpen");
        Ok(())
    }
}

mod part_d_settings {
    use super::*;

    #[test]
    fn malformed_shapes_are_quarantined_independently() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("tt-c41-shapes-{}", tt_executor::random_token(8)));
        let dirs = tt_executor::UserDirs::under(&root);
        std::fs::create_dir_all(&dirs.settings)?;
        for kind in ["preferences", "keymap", "state", "recent", "workspaces"] {
            let bad = if kind == "recent" { json!({"wrong": true}) } else { json!(42) };
            tt_persist::settings::write(&dirs.settings, kind, &bad)?;
        }
        let h = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
            persist: tt_executor::PersistOptions { user_dirs: Some(dirs.clone()), ..Default::default() },
            ..Default::default()
        })?;
        let retained: Vec<_> = ["preferences", "keymap", "state", "recent", "workspaces"]
            .into_iter()
            .filter(|k| dirs.settings.join(format!("{k}.json")).exists())
            .collect();
        drop(h);
        std::fs::remove_dir_all(root)?;
        assert!(retained.is_empty(), "invalid shapes silently retained: {retained:?}");
        Ok(())
    }

    #[test]
    fn abandoned_settings_temporary_is_cleaned_on_load() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("tt-c41-temp-{}", tt_executor::random_token(8)));
        let dirs = tt_executor::UserDirs::under(&root);
        std::fs::create_dir_all(&dirs.settings)?;
        tt_persist::settings::write(&dirs.settings, "preferences", &json!({"interface.theme":"dark"}))?;
        let abandoned = dirs.settings.join(".preferences.json.0123456789ab.tmp");
        std::fs::write(&abandoned, b"interrupted replacement")?;
        let h = ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
            persist: tt_executor::PersistOptions { user_dirs: Some(dirs), ..Default::default() },
            ..Default::default()
        })?;
        let remains = abandoned.exists();
        drop(h);
        std::fs::remove_dir_all(root)?;
        assert!(!remains, "interrupted settings write leaves an orphan forever");
        Ok(())
    }
}

mod part_d_preferences {
    use super::*;
    use serde_json::Value;
    use tt_executor::{ExecutorOptions, PersistOptions, Status};

    fn call(h: &ExecutorHandle, command: &str, params: Value) -> tt_executor::Envelope {
        h.invoke(Invocation { command: command.into(), params, options: Default::default(), origin: Origin::User })
    }

    #[test]
    fn catalogue_types_ranges_and_nonfinite_wire_values() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        let catalogue: Value = serde_json::from_str(include_str!("../src/prefs.json"))?;
        for d in catalogue["prefs"].as_array().ok_or("catalogue array")? {
            let key = d["key"].as_str().ok_or("key")?;
            let e = call(&h, "prefs.set", json!({"values":{key: null}}));
            assert_ne!(e.status, Status::Ok, "null accepted for {key}");
            for (edge, delta) in [("min", -1.0), ("max", 1.0)] {
                if let Some(n) = d[edge].as_f64() {
                    let outside = if d["type"] == "int" { json!((n + delta) as i64) } else { json!(n + delta) };
                    assert_ne!(
                        call(&h, "prefs.set", json!({"values":{key: outside}})).status,
                        Status::Ok,
                        "{key} {edge}"
                    );
                }
            }
        }
        assert_ne!(call(&h, "prefs.set", json!({"values":{"unknown.review":true}})).status, Status::Ok);
        // JSON cannot carry nonfinite numbers. The typed Value conversion becomes null and must be refused too.
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_ne!(call(&h, "prefs.set", json!({"values":{"project.defaults.frameRate":n}})).status, Status::Ok);
        }
        for literal in ["NaN", "Infinity", "-Infinity", "1e999"] {
            assert!(
                serde_json::from_str::<Value>(&format!("{{\"values\":{{\"project.defaults.frameRate\":{literal}}}}}"))
                    .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn accepted_calls_emit_only_actual_changed_keys() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        let events = h.subscribe();
        assert_eq!(
            call(&h, "prefs.set", json!({"values":{"interface.theme":"dark","files.backups.count":3}})).status,
            Status::Ok
        );
        let changes: Vec<_> = events.try_iter().filter(|e| e.name == "prefs.changed").collect();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].payload["keys"], json!(["interface.theme"]), "unchanged defaults are not changes");
        Ok(())
    }

    #[test]
    fn atomic_disk_defaults_reload_access_and_reset() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("tt-c41-prefs-{}", tt_executor::random_token(8)));
        let dirs = tt_executor::UserDirs::under(&root);
        let over = || {
            ExecutorHandle::spawn_with_options(ExecutorOptions {
                persist: PersistOptions { user_dirs: Some(dirs.clone()), ..Default::default() },
                ..Default::default()
            })
        };
        let h = over()?;
        assert_eq!(call(&h, "prefs.set", json!({"values":{"interface.theme":"dark"}})).status, Status::Ok);
        let path = dirs.settings.join("preferences.json");
        let before = std::fs::read(&path)?;
        assert_ne!(
            call(&h, "prefs.set", json!({"values":{"interface.theme":"light","unknown.review":1}})).status,
            Status::Ok
        );
        assert_eq!(std::fs::read(&path)?, before);
        assert_eq!(
            call(&h, "prefs.get", json!({"keys":["interface.theme"]})).result.ok_or("get")?["values"]["interface.theme"],
            "dark"
        );
        assert_eq!(call(&h, "prefs.set", json!({"values":{"interface.theme":"system"}})).status, Status::Ok);
        let stored: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        assert!(stored["data"].get("interface.theme").is_none());
        let mut inv = Invocation {
            command: "prefs.set".into(),
            params: json!({"values":{"agent.server.enabled":true}}),
            options: Default::default(),
            origin: Origin::Agent {
                session: "review".into(),
                name: "review".into(),
                scopes: [tt_registry::Scope::Prefs, tt_registry::Scope::App].into_iter().collect(),
            },
        };
        assert_ne!(h.invoke(inv.clone()).status, Status::Ok);
        inv.origin = Origin::User;
        assert_eq!(h.invoke(inv).status, Status::Ok);
        for params in [
            json!({"keys":["agent.server.enabled"]}),
            json!({"prefix":"interface."}),
            json!({"prefix":""}),
            json!({"keys":[]}),
            json!({"prefix":"unknown."}),
        ] {
            assert_eq!(call(&h, "prefs.reset", params).status, Status::Ok);
        }
        assert_ne!(call(&h, "prefs.reset", json!({})).status, Status::Ok);
        assert_ne!(call(&h, "prefs.reset", json!({"keys":["unknown.review"]})).status, Status::Ok);
        drop(h);
        tt_persist::settings::write(
            &dirs.settings,
            "preferences",
            &json!({"files.backups.count":99,"interface.theme":"dark"}),
        )?;
        let h = over()?;
        let values =
            call(&h, "prefs.get", json!({"keys":["files.backups.count","interface.theme"]})).result.ok_or("get")?;
        assert_eq!(values["values"]["files.backups.count"], 3);
        assert_eq!(values["values"]["interface.theme"], "dark");
        drop(h);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}

mod part_d_agent_policy {
    use super::*;

    #[test]
    fn generic_preferences_enforce_agent_policy_limits() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        for bad in [
            json!({"policy":"unknown"}),
            json!({"maxJobs":0}),
            json!({"maxJobs":9}),
            json!({"maxBatchSteps":0}),
            json!({"maxBatchSteps":100001}),
            json!({"maxInlineImagePx":255}),
            json!({"maxInlineImagePx":8193}),
        ] {
            let env = h.invoke(Invocation {
                command: "prefs.set".into(),
                params: json!({"values":{"agent.policy.defaults":bad}}),
                options: Default::default(),
                origin: Origin::User,
            });
            assert_ne!(
                env.status,
                tt_executor::Status::Ok,
                "invalid policy accepted through generic preferences: {bad}"
            );
        }
        Ok(())
    }
}

mod part_e_keymap {
    use super::*;
    use tt_executor::Status;
    fn call(h: &ExecutorHandle, id: &str, p: serde_json::Value) -> tt_executor::Envelope {
        h.invoke(Invocation::user(id, p))
    }
    #[test]
    fn import_preserves_typed_binding_errors() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("tt-key-errors-{}", tt_executor::random_token(8)));
        std::fs::create_dir_all(&root)?;
        let h = ExecutorHandle::spawn(Default::default())?;
        let path = root.join("exchange.json");
        let mut errors = Vec::new();
        for mode in ["merge", "replace"] {
            for (command, keys, expected) in
                [("project.save", "Escape", "keymap.reservedKey"), ("missing.review", "F2", "keymap.unknownCommand")]
            {
                std::fs::write(
                    &path,
                    serde_json::to_vec(
                        &json!({"format":"toontown-keymap","bindings":[{"commandId":command,"keys":keys,"context":"global"}]}),
                    )?,
                )?;
                let e = call(&h, "keymap.import", json!({"path":path,"mode":mode}));
                if e.error.as_ref().map(|e| e.code.as_str()) != Some(expected) {
                    errors.push(format!("{mode}/{expected}: {:?}", e.error));
                }
            }
        }
        std::fs::remove_dir_all(root)?;
        assert!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    #[test]
    fn empty_sequence_has_keymap_error() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        let e = call(&h, "keymap.set", json!({"commandId":"project.save","keys":""}));
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("keymap.invalidSequence"));
        Ok(())
    }
    #[test]
    fn canonical_conflicts_override_reset_and_defaults() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        for p in [tt_executor::keymap::Platform::Windows, tt_executor::keymap::Platform::Macos] {
            assert!(tt_executor::keymap::default_problems(p).is_empty());
            for b in tt_executor::keymap::defaults(p) {
                assert!(tt_executor::registry().map_err(|e| e.0)?.get(&b.command_id).is_some());
            }
        }
        for keys in ["Primary+Shift+F2", "shift+primary+f2"] {
            assert_eq!(call(&h, "keymap.set", json!({"commandId":"project.save","keys":keys})).status, Status::Ok);
        }
        let conflict =
            call(&h, "keymap.set", json!({"commandId":"project.open","keys":"shift+primary+f2","context":"drawing"}));
        assert_eq!(conflict.error.as_ref().map(|e| e.code.as_str()), Some("keymap.conflict"));
        assert_eq!(
            call(
                &h,
                "keymap.set",
                json!({"commandId":"project.open","keys":"Primary+Shift+F2","onConflict":"override"})
            )
            .status,
            Status::Ok
        );
        let save = call(&h, "keymap.list", json!({"commandId":"project.save"})).result.ok_or("list")?;
        assert!(!save.to_string().contains("F2"));
        assert_eq!(call(&h, "keymap.reset", json!({"commandId":"project.save"})).status, Status::Ok);
        assert_eq!(call(&h, "keymap.reset", json!({"all":true})).status, Status::Ok);
        Ok(())
    }
}
mod part_e_freshness {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn older_tree_cannot_inherit_newer_acknowledgement() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        let _ = h.invoke(Invocation::user("prefs.set", json!({"values":{"interface.theme":"dark"}})));
        let seq = h.events_broadcast();
        assert!(seq > 0);
        h.report_shell(ShellReport {
            attached: Some(true),
            ui_tree: Some(json!({"id":"window","label":"new"})),
            rendered_seq: Some(seq),
            ..Default::default()
        });
        h.report_shell(ShellReport {
            ui_tree: Some(json!({"id":"window","label":"old"})),
            rendered_seq: Some(0),
            ..Default::default()
        });
        let tree = h.invoke(Invocation::user("inspect.ui.tree", json!({}))).result.ok_or("tree")?;
        assert!(
            !h.await_rendered(Duration::ZERO) || tree["root"]["label"] != "old",
            "old tree was accepted as fresh: {tree}"
        );
        Ok(())
    }
    #[test]
    fn future_acknowledgement_cannot_make_tree_fresh() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        h.report_shell(ShellReport {
            attached: Some(true),
            ui_tree: Some(json!({"id":"window"})),
            rendered_seq: Some(u64::MAX),
            ..Default::default()
        });
        let _ = h.invoke(Invocation::user("prefs.set", json!({"values":{"interface.theme":"dark"}})));
        assert!(!h.await_rendered(Duration::ZERO), "future acknowledgement blessed an older tree");
        Ok(())
    }
    #[test]
    fn bounded_concurrent_waits_leave_document_thread_free() -> Result<(), Box<dyn Error>> {
        let h = ExecutorHandle::spawn(Default::default())?;
        assert!(h.await_rendered(Duration::ZERO));
        let tree = h.invoke(Invocation::user("inspect.ui.tree", json!({}))).result.ok_or("tree")?;
        assert_eq!(tree["source"], "virtual");
        h.report_shell(ShellReport { attached: Some(true), ..Default::default() });
        let _ = h.invoke(Invocation::user("prefs.set", json!({"values":{"interface.theme":"dark"}})));
        let start = Instant::now();
        let joins: Vec<_> = (0..16)
            .map(|_| {
                let h = h.clone();
                std::thread::spawn(move || h.await_rendered(Duration::from_secs(2)))
            })
            .collect();
        let user = Instant::now();
        assert!(h.invoke(Invocation::user("app.state.get", json!({}))).is_ok());
        assert!(user.elapsed() < Duration::from_secs(1));
        for j in joins {
            assert!(!j.join().map_err(|_| "wait panic")?);
        }
        assert!(start.elapsed() < Duration::from_secs(4));
        Ok(())
    }
}

mod part_e_restart {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn release_does_not_steal_another_live_engines_recovery() -> Result<(), Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("tt-live-recovery-{}", tt_executor::random_token(8)));
        let over = || {
            ExecutorHandle::spawn_with_options(tt_executor::ExecutorOptions {
                persist: tt_executor::PersistOptions {
                    user_dirs: Some(tt_executor::UserDirs::under(&root)),
                    maintenance: tt_executor::Maintenance {
                        tick: Duration::ZERO,
                        autosave: Some(Duration::ZERO),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
        };
        let first = over()?;
        assert!(first.invoke(Invocation::user("project.create", json!({"name":"Still live"}))).is_ok());
        first.run_maintenance(true)?;
        let start = Instant::now();
        while !std::fs::read_dir(root.join("Recovery"))
            .map(|rd| rd.filter_map(Result::ok).any(|e| e.path().join("recovery.json").exists()))
            .unwrap_or(false)
        {
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::yield_now();
        }
        let second = over()?;
        let released = second.release_abandoned_recovery()?;
        drop(second);
        drop(first);
        std::fs::remove_dir_all(root)?;
        assert_eq!(released, 0, "another live engine still owns its recovery");
        Ok(())
    }
}
