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

#[test]
fn a_floating_panel_survives_a_restart_with_its_geometry() {
    let r = root("float");
    let h = over(&r);
    let env = h.invoke(Invocation::user(
        "window.panel.set",
        json!({ "panelId": "history", "floating": true, "geometry": { "x": 60, "y": 60, "width": 420, "height": 300 } }),
    ));
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    // The current workspace is stored when it is saved, at quit with autosave on, or before a switch (01 §1.2, §5.5).
    assert_eq!(h.invoke(Invocation::user("workspace.save", json!({}))).status, Status::Ok);
    drop(h);
    let h = over(&r);
    let panels = h.invoke(Invocation::user("window.panel.list", json!({}))).result.unwrap();
    let history = panels.as_array().unwrap().iter().find(|p| p["panelId"] == "history").cloned().unwrap();
    assert_eq!((history["floating"].clone(), history["visible"].clone()), (json!(true), json!(true)), "{history}");
    assert_eq!(history["geometry"], json!({ "x": 60, "y": 60, "width": 420, "height": 300 }), "{history}");
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn quitting_saves_a_modified_workspace_with_its_floats() {
    let r = root("quit");
    let h = over(&r);
    let float =
        json!({ "panelId": "colour", "floating": true, "geometry": { "x": 70, "y": 80, "width": 300, "height": 400 } });
    assert_eq!(h.invoke(Invocation::user("window.panel.set", float)).status, Status::Ok);
    let quit = h.invoke(Invocation::user("app.quit", json!({ "onUnsaved": "discard" })));
    assert_eq!(quit.status, Status::Ok, "{:?}", quit.error);
    drop(h);
    let h = over(&r);
    let panels = h.invoke(Invocation::user("window.panel.list", json!({}))).result.unwrap();
    let colour = panels.as_array().unwrap().iter().find(|p| p["panelId"] == "colour").cloned().unwrap();
    assert_eq!(colour["floating"], true, "{colour}");
    assert_eq!(colour["geometry"]["width"], 300, "{colour}");
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn moving_a_float_is_announced() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let float = json!({ "panelId": "history", "floating": true, "geometry": { "x": 60, "y": 60, "width": 420, "height": 300 } });
    assert_eq!(h.invoke(Invocation::user("window.panel.set", float)).status, Status::Ok);
    let events = h.subscribe();
    let moved = json!({ "panelId": "history", "geometry": { "x": 140, "y": 90, "width": 380, "height": 280 } });
    assert_eq!(h.invoke(Invocation::user("window.panel.set", moved)).status, Status::Ok);
    let names: Vec<String> = events.try_iter().map(|e| e.name).collect();
    assert!(names.iter().any(|n| n == "workspace.changed"), "a float move is announced (ruling O-2): {names:?}");
}

#[test]
fn a_float_is_clamped_to_the_reported_screens() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    h.report_shell(
        serde_json::from_value(json!({ "screens": [{ "x": 0, "y": 0, "width": 1024, "height": 768, "scale": 1.0 }] }))
            .unwrap(),
    );
    let off = json!({ "panelId": "history", "floating": true, "geometry": { "x": 5000, "y": 4000, "width": 3000, "height": 300 } });
    let entry = h.invoke(Invocation::user("window.panel.set", off)).result.unwrap();
    let g = &entry["geometry"];
    assert!(g["width"].as_u64().unwrap() <= 1024, "{g}");
    let (x, y) = (g["x"].as_i64().unwrap(), g["y"].as_i64().unwrap());
    assert!((0..1024).contains(&x) && (0..768).contains(&y), "on a screen: {g}");
}

fn pref(h: &ExecutorHandle, key: &str) -> Value {
    h.invoke(Invocation::user("prefs.get", json!({ "keys": [key] }))).result.unwrap()["values"][key].clone()
}

/// Ruling O-16: an object supplied for an object-typed preference is merged member by member over the present value,
/// through `prefs.set` and `agent.policy.defaults.set` alike; `prefs.changed` names the key only when the merged value
/// differs; `prefs.reset` returns the members to their defaults.
#[test]
fn a_partial_object_is_merged_over_the_present_value() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let key = "edit.pasteSpecial.timeline";
    let set = |members: Value| {
        let e = h.invoke(Invocation::user("prefs.set", json!({ "values": { key: members } })));
        assert_eq!(e.status, Status::Ok, "{e:?}");
    };
    set(json!({ "overwriteDrawings": false }));
    set(json!({ "extendExposures": true }));
    let v = pref(&h, key);
    assert_eq!((v["overwriteDrawings"].clone(), v["extendExposures"].clone()), (json!(false), json!(true)));
    let events = h.subscribe();
    set(json!({ "overwriteDrawings": false }));
    assert!(events.try_iter().all(|e| e.name != "prefs.changed"), "an unchanged merged value is not a change");
    // A member of the wrong type refuses the whole call and changes nothing.
    let e = h.invoke(Invocation::user("prefs.set", json!({ "values": { key: { "updateColours": 3 } } })));
    assert_eq!(e.error.unwrap().code, "prefs.invalidValue");
    assert_eq!(pref(&h, key), v);
    // The agent policy setter merges the same way.
    let policy = |p: Value| assert_eq!(h.invoke(Invocation::user("agent.policy.defaults.set", p)).status, Status::Ok);
    policy(json!({ "maxJobs": 7 }));
    policy(json!({ "maxBatchSteps": 42 }));
    let p = pref(&h, "agent.policy.defaults");
    assert_eq!((p["maxJobs"].clone(), p["maxBatchSteps"].clone()), (json!(7), json!(42)));
    assert_eq!(h.invoke(Invocation::user("prefs.reset", json!({ "keys": [key] }))).status, Status::Ok);
    assert_eq!(pref(&h, key)["overwriteDrawings"], true);
}

/// Ruling O-17: a stored object-typed preference falls back per member: an invalid member takes its default, valid
/// members are kept, unknown members are ignored; for the timeline and motion-point composites and the agent policy.
#[test]
fn a_stale_member_falls_back_alone() {
    let r = root("members");
    let dirs = UserDirs::under(&r);
    settings_file(
        &dirs.settings,
        "preferences",
        "1.0",
        &json!({
            "edit.pasteSpecial.timeline": { "overwriteDrawings": "yes", "extendExposures": true, "gone": 1 },
            "edit.cutSpecial.timeline": { "boundaryKeys": true, "pivotKeys": 0 },
            "edit.dragDrop.timeline": "not an object",
            "edit.pasteSpecial.motionPoint": { "x": false, "y": null },
            "agent.policy.defaults": { "policy": "reckless", "maxJobs": 5, "maxInlineImagePx": 99 }
        }),
    );
    let h = over(&r);
    let paste = pref(&h, "edit.pasteSpecial.timeline");
    assert_eq!((paste["overwriteDrawings"].clone(), paste["extendExposures"].clone()), (json!(true), json!(true)));
    assert!(paste.get("gone").is_none());
    let cut = pref(&h, "edit.cutSpecial.timeline");
    assert_eq!((cut["boundaryKeys"].clone(), cut["pivotKeys"].clone()), (json!(true), json!(true)));
    assert_eq!(pref(&h, "edit.dragDrop.timeline")["overwriteDrawings"], true);
    let motion = pref(&h, "edit.pasteSpecial.motionPoint");
    assert_eq!((motion["x"].clone(), motion["y"].clone()), (json!(false), json!(true)));
    let policy = pref(&h, "agent.policy.defaults");
    assert_eq!(policy["policy"], "assist");
    assert_eq!(policy["maxJobs"], 5);
    assert_eq!(policy["maxInlineImagePx"], 2048);
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}
