//! M4 C4.1 round 1: settings files that cannot be used or saved are told to the user, saying what happened (C41-R1-04,
//! C41-R1-08, rulings O-12 and O-13). Builder-owned.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, Status, UserDirs};

fn root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tt-c41-settings-{tag}-{}", tt_executor::random_token(6)))
}

fn over(root: &Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions { user_dirs: Some(UserDirs::under(root)), ..Default::default() },
        ..Default::default()
    })
    .unwrap()
}

/// The `ui.message.shown` payloads a fresh executor emits at its first call.
fn shown_at_start(h: &ExecutorHandle) -> Vec<Value> {
    let events = h.subscribe();
    assert_eq!(h.invoke(Invocation::user("app.state.get", json!({}))).status, Status::Ok);
    events.try_iter().filter(|e| e.name == "ui.message.shown").map(|e| e.payload).collect()
}

fn settings_file(dir: &Path, kind: &str, version: &str, data: &Value) {
    std::fs::create_dir_all(dir).unwrap();
    let v = json!({ "format": "toontown-settings", "kind": kind, "formatVersion": version, "data": data });
    std::fs::write(dir.join(format!("{kind}.json")), v.to_string()).unwrap();
}

#[test]
fn an_unusable_file_is_reported_with_its_reason_and_where_it_was_kept() {
    let r = root("aside");
    let dirs = UserDirs::under(&r);
    settings_file(&dirs.settings, "keymap", "2.0", &json!({}));
    settings_file(&dirs.settings, "state", "1.0", &json!({ "suppressedMessages": "not a list" }));
    settings_file(&dirs.settings, "preferences", "1.0", &json!({ "interface.theme": "dark" }));
    let h = over(&r);
    let shown = shown_at_start(&h);
    let corrupt: Vec<&Value> = shown.iter().filter(|m| m["messageId"] == "prefs.corrupt").collect();
    assert_eq!(corrupt.len(), 2, "{shown:?}");
    let by = |kind: &str| corrupt.iter().find(|m| m["details"]["kind"] == kind).copied().unwrap();
    assert_eq!(by("keymap")["details"]["reason"], "unsupportedVersion");
    assert_eq!(by("state")["details"]["reason"], "wrongShape");
    for m in &corrupt {
        let kept = m["details"]["keptAt"].as_str().unwrap();
        assert!(Path::new(kept).exists(), "the reported copy exists: {kept}");
    }
    // The good file is unaffected.
    let prefs = h.invoke(Invocation::user("prefs.get", json!({ "keys": ["interface.theme"] })));
    assert_eq!(prefs.result.unwrap()["values"]["interface.theme"], "dark");
    // A second start reports nothing more: the files were kept aside.
    drop(h);
    let h = over(&r);
    assert!(shown_at_start(&h).iter().all(|m| m["messageId"] != "prefs.corrupt"));
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn a_failed_settings_write_is_told_once_per_file_and_the_change_stays_in_memory() {
    let r = root("unwritable");
    std::fs::create_dir_all(&r).unwrap();
    // The settings folder's place is taken by a file: no folder and no settings file can be created there.
    std::fs::write(r.join("Settings"), b"not a folder").unwrap();
    let h = over(&r);
    let events = h.subscribe();
    let first = h.invoke(Invocation::user("prefs.set", json!({ "values": { "interface.theme": "dark" } })));
    assert_eq!(first.status, Status::Ok, "the command still succeeds in memory: {:?}", first.error);
    let second = h.invoke(Invocation::user("prefs.set", json!({ "values": { "interface.theme": "light" } })));
    assert_eq!(second.status, Status::Ok);
    let shown: Vec<Value> = events.try_iter().filter(|e| e.name == "ui.message.shown").map(|e| e.payload).collect();
    let failed: Vec<&Value> = shown
        .iter()
        .filter(|m| m["messageId"] == "prefs.saveFailed" && m["details"]["kind"] == "preferences")
        .collect();
    assert_eq!(failed.len(), 1, "once per file per session: {shown:?}");
    assert!(failed[0]["details"]["path"].as_str().unwrap().ends_with("preferences.json"));
    assert!(!failed[0]["details"]["message"].as_str().unwrap().is_empty(), "the OS reason is given");
    let now = h.invoke(Invocation::user("prefs.get", json!({ "keys": ["interface.theme"] })));
    assert_eq!(now.result.unwrap()["values"]["interface.theme"], "light");
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}
