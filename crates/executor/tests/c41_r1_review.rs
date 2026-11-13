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
