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
