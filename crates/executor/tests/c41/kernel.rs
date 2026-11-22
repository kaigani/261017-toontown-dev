//! M4 C4.1 kernel: preferences (01 §7), the keymap (01 §8), `ui.*` (01 §2, §5.8, §9), the native-window commands as
//! staged shell effects (ADR-0021) and settings persistence across an executor restart over the same user folder
//! (01 §11). Builder-owned.

use std::path::PathBuf;
use std::time::Duration;

use crate::common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, ExecutorOptions, Invocation, InvokeOptions, Status};
use tt_registry::Scope;

/// A fresh user folder for one test.
fn user_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tt-c41-{tag}-{}-{}", std::process::id(), nanos()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

/// An executor over a user folder, no project.
fn over(dir: &std::path::Path) -> ExecutorHandle {
    ExecutorHandle::spawn_with_options(ExecutorOptions {
        config: ExecutorConfig { budget: Duration::from_secs(60) },
        persist: tt_executor::PersistOptions {
            user_dirs: Some(tt_executor::UserDirs::under(dir)),
            maintenance: tt_executor::Maintenance { tick: Duration::ZERO, ..Default::default() },
            ..Default::default()
        },
    })
    .unwrap()
}

fn settings_file(dir: &std::path::Path, kind: &str) -> Value {
    let text = std::fs::read_to_string(dir.join("Settings").join(format!("{kind}.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// Preferences
// ---------------------------------------------------------------------------------------------------------------

/// Every key of the 01 §7.3 catalogue tables is listed with its type, default and range (01 §12.1 rule 15).
#[test]
fn the_catalogue_covers_01_section_7_3() {
    let spec =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/01_application_shell.md")).unwrap();
    let start = spec.find("### 7.3 Preference catalogue").unwrap();
    let end = spec[start..].find("## 8.").map(|e| start + e).unwrap();
    let mut spec_keys: Vec<(String, String, String)> = Vec::new();
    for line in spec[start..end].lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() > 4 && cells[1].starts_with('`') && cells[1].ends_with('`') {
            spec_keys.push((cells[1].trim_matches('`').to_owned(), cells[2].to_owned(), cells[3].to_owned()));
        }
    }
    assert!(spec_keys.len() >= 60, "{}", spec_keys.len());
    let h = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    let list = ok(&h, "prefs.list", json!({}));
    let list = list.as_array().unwrap();
    for (key, ty, default) in &spec_keys {
        let e = list.iter().find(|e| e["key"] == json!(key)).unwrap_or_else(|| panic!("{key} missing"));
        assert_eq!(e["type"], json!(ty), "{key}");
        // Defaults as the spec writes them (numbers, booleans, quoted strings, colours, enum names).
        let d = &e["default"];
        let written = default.trim_matches('`').trim_matches('"');
        match ty.as_str() {
            "bool" => assert_eq!(d, &json!(written == "true"), "{key}"),
            "int" | "number" => assert!((d.as_f64().unwrap() - written.parse::<f64>().unwrap()).abs() < 1e-12, "{key}"),
            "colour" if written != "theme default" => assert_eq!(d, &json!(written), "{key}"),
            "enum" if !written.contains(' ') => assert_eq!(d, &json!(written), "{key}"),
            _ => {}
        }
    }
}

#[test]
fn prefs_set_is_atomic_and_stores_only_differences() {
    let dir = user_dir("atomic");
    let h = over(&dir);
    // One invalid value: nothing changes.
    let e = err(&h, "prefs.set", json!({ "values": { "interface.theme": "dark", "files.backups.count": 99 } }));
    assert_eq!(e.code, "prefs.outOfRange");
    assert_eq!(e.details["pointer"], "/values/files.backups.count");
    assert_eq!(ok(&h, "prefs.get", json!({ "keys": ["interface.theme"] }))["values"]["interface.theme"], "system");
    assert!(!dir.join("Settings/preferences.json").exists(), "a refused call writes nothing");
    // Valid: applied; the restart key is reported; only non-default values are stored.
    let r = ok(
        &h,
        "prefs.set",
        json!({ "values": { "interface.theme": "dark", "display.fastPencilRendering": true, "files.backups.count": 3 } }),
    );
    assert_eq!(r["restartRequired"], json!(["display.fastPencilRendering"]));
    let stored = settings_file(&dir, "preferences");
    assert_eq!(stored["data"], json!({ "interface.theme": "dark", "display.fastPencilRendering": true }));
    // Typed refusals.
    assert_eq!(err(&h, "prefs.set", json!({ "values": { "nope.nope": 1 } })).code, "prefs.unknownKey");
    assert_eq!(err(&h, "prefs.set", json!({ "values": { "interface.theme": "neon" } })).code, "prefs.invalidValue");
    assert_eq!(
        err(&h, "prefs.set", json!({ "values": { "files.autosave.intervalMinutes": 2.5 } })).code,
        "prefs.invalidValue"
    );
    assert_eq!(err(&h, "prefs.get", json!({ "keys": ["nope"] })).code, "prefs.unknownKey");
    // Reset by prefix.
    let r = ok(&h, "prefs.reset", json!({ "prefix": "interface.theme" }));
    assert_eq!(r["reset"], json!(["interface.theme"]));
    assert_eq!(settings_file(&dir, "preferences")["data"], json!({ "display.fastPencilRendering": true }));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dry_run_and_a_failed_batch_write_no_settings() {
    let dir = user_dir("dry");
    let h = over(&dir);
    let env = with(
        &h,
        "prefs.set",
        json!({ "values": { "interface.theme": "dark" } }),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(env.status, Status::Ok);
    let env = batch(
        &h,
        json!({ "atomic": true, "label": "Settings", "steps": [
            { "id": "prefs.set", "params": { "values": { "interface.theme": "dark" } } },
            { "id": "keymap.set", "params": { "commandId": "project.save", "keys": "F2" } },
            { "id": "prefs.get", "params": { "keys": ["interface.theme"] } },
            { "id": "keymap.get", "params": { "keys": "F2" } },
            { "id": "prefs.set", "params": { "values": { "files.backups.count": -1 } } }
        ] }),
    );
    assert_eq!(env.status, Status::Error);
    assert!(!dir.join("Settings/preferences.json").exists());
    assert!(!dir.join("Settings/keymap.json").exists());
    // The same batch without the bad step: later steps see the staged values, and both files are written.
    let env = batch(
        &h,
        json!({ "atomic": true, "label": "Settings", "steps": [
            { "id": "prefs.set", "params": { "values": { "interface.theme": "dark" } } },
            { "id": "keymap.set", "params": { "commandId": "project.save", "keys": "F2" } },
            { "id": "prefs.get", "params": { "keys": ["interface.theme"] } },
            { "id": "keymap.get", "params": { "keys": "F2" } }
        ] }),
    );
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    let steps = serde_json::to_value(&env).unwrap()["steps"].clone();
    assert_eq!(steps[2]["result"]["values"]["interface.theme"], "dark", "{steps}");
    assert_eq!(steps[3]["result"]["effectiveCommand"]["commandId"], "project.save", "{steps}");
    assert!(dir.join("Settings/preferences.json").exists() && dir.join("Settings/keymap.json").exists());
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn agents_cannot_change_their_own_access() {
    let h = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    let inv = |cmd: &str, params: Value| Invocation {
        command: cmd.into(),
        params,
        options: InvokeOptions::default(),
        origin: agent(&[Scope::Read, Scope::Prefs, Scope::Ui]),
    };
    let env = h.invoke(inv("prefs.set", json!({ "values": { "agent.server.enabled": true } })));
    assert_eq!(env.error.unwrap().code, "permission.denied");
    let env = h.invoke(inv("prefs.set", json!({ "values": { "interface.theme": "dark" } })));
    assert_eq!(env.status, Status::Ok, "{:?}", env.error);
    // A full reset by an agent leaves the agent keys alone.
    let env = h.invoke(inv("prefs.reset", json!({ "prefix": "" })));
    let reset = env.result.unwrap()["reset"].as_array().unwrap().clone();
    assert!(!reset.iter().any(|k| k.as_str().unwrap().starts_with("agent.")));
    let env = h.invoke(inv("prefs.reset", json!({ "keys": ["agent.server.port"] })));
    assert_eq!(env.error.unwrap().code, "permission.denied");
}

// ---------------------------------------------------------------------------------------------------------------
// Keymap
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn keymap_set_conflicts_override_and_reserved_keys() {
    let dir = user_dir("keys");
    let h = over(&dir);
    // Primary+S is Save's on every platform (written in any case): a conflict without override, moved with it. (Ctrl
    // is the primary modifier on Windows only; on macOS it is its own key, so `ctrl+s` would not conflict there.)
    let e = err(&h, "keymap.set", json!({ "commandId": "project.saveAs", "keys": "primary+s" }));
    assert_eq!(e.code, "keymap.conflict", "{e:?}");
    assert_eq!(e.details["conflicts"][0]["commandId"], "project.save");
    let r =
        ok(&h, "keymap.set", json!({ "commandId": "project.saveAs", "keys": "Primary+S", "onConflict": "override" }));
    assert_eq!(r["removed"][0]["commandId"], "project.save");
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "Primary+S" }))["effectiveCommand"]["commandId"], "project.saveAs");
    // Mode contexts shadow global: binding a global key in the drawing context conflicts.
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "edit.copy", "keys": "Primary+Z", "context": "drawing" })).code,
        "keymap.conflict"
    );
    // Reserved and malformed sequences, unknown commands, events.
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "project.save", "keys": "Escape" })).code,
        "keymap.reservedKey"
    );
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "project.save", "keys": "Shift" })).code,
        "keymap.reservedKey"
    );
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "project.save", "keys": "Primary+" })).code,
        "keymap.invalidSequence"
    );
    assert_eq!(err(&h, "keymap.set", json!({ "commandId": "nope.nope", "keys": "F2" })).code, "keymap.unknownCommand");
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "prefs.changed", "keys": "F2" })).code,
        "keymap.unknownCommand"
    );
    assert_eq!(
        err(&h, "keymap.set", json!({ "commandId": "view.zoom.set", "params": { "steps": "x" }, "keys": "F2" })).code,
        "arg.invalid"
    );
    // Overrides are stored as deltas against the defaults (01 §8.1).
    let stored = settings_file(&dir, "keymap")["data"].clone();
    assert_eq!(stored["baseSet"], "default");
    assert_eq!(stored["added"].as_array().unwrap().len(), 1, "{stored}");
    assert_eq!(stored["removed"].as_array().unwrap().len(), 1, "{stored}");
    // Reset one command restores its default and moves the other off the sequence.
    let r = ok(&h, "keymap.reset", json!({ "commandId": "project.save" }));
    assert!(r["reset"].as_u64().unwrap() >= 1);
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "Primary+S" }))["effectiveCommand"]["commandId"], "project.save");
    assert!(
        ok(&h, "keymap.list", json!({ "onlyModified": true })).as_array().unwrap().is_empty(),
        "the move was undone"
    );
    ok(&h, "keymap.set", json!({ "commandId": "project.open", "keys": "F3" }));
    ok(&h, "keymap.remove", json!({ "commandId": "print.run" }));
    let r = ok(&h, "keymap.reset", json!({ "all": true }));
    assert_eq!(r["reset"], 2);
    assert!(ok(&h, "keymap.list", json!({ "onlyModified": true })).as_array().unwrap().is_empty());
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "Primary+P" }))["effectiveCommand"]["commandId"], "print.run");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keymap_export_import_merge_and_replace() {
    let dir = user_dir("exchange");
    let h = over(&dir);
    ok(&h, "keymap.set", json!({ "commandId": "project.save", "keys": "F2" }));
    let file = dir.join("keys.json");
    ok(&h, "keymap.export", json!({ "path": file.to_string_lossy() }));
    assert_eq!(err(&h, "keymap.export", json!({ "path": file.to_string_lossy() })).code, "fs.exists");
    ok(&h, "keymap.reset", json!({ "all": true }));
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "F2" }))["effectiveCommand"], Value::Null);
    // Merge brings F2 back; the defaults already present are not duplicated.
    let r = ok(&h, "keymap.import", json!({ "path": file.to_string_lossy() }));
    assert!(r["imported"].as_u64().unwrap() > 10);
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "F2" }))["effectiveCommand"]["commandId"], "project.save");
    // A conflicting file: refused whole with `fail`, applied with `override`.
    let other = dir.join("other.json");
    std::fs::write(
        &other,
        serde_json::to_vec(&json!({ "format": "toontown-keymap", "formatVersion": "1.0",
            "bindings": [{ "commandId": "project.open", "keys": "F2", "context": "global" }] }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(err(&h, "keymap.import", json!({ "path": other.to_string_lossy() })).code, "keymap.conflict");
    let r = ok(&h, "keymap.import", json!({ "path": other.to_string_lossy(), "onConflict": "override" }));
    assert_eq!(r["conflicts"][0]["commandId"], "project.save");
    // Replace: the file is the whole keymap.
    ok(&h, "keymap.import", json!({ "path": other.to_string_lossy(), "mode": "replace" }));
    assert_eq!(ok(&h, "keymap.list", json!({})).as_array().unwrap().len(), 1);
    // A malformed file changes nothing.
    std::fs::write(&other, b"{\"format\": \"toontown-keymap\", \"bindings\": [{\"commandId\": \"project.open\"}]}")
        .unwrap();
    assert_eq!(err(&h, "keymap.import", json!({ "path": other.to_string_lossy() })).code, "keymap.invalidFile");
    assert_eq!(ok(&h, "keymap.list", json!({})).as_array().unwrap().len(), 1);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// ui.*
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn tips_status_bar_start_screen_and_help() {
    let dir = user_dir("ui");
    let h = over(&dir);
    assert_eq!(ok(&h, "ui.tips.list", json!({})).as_array().unwrap().len(), 5);
    ok(&h, "ui.tips.set", json!({ "messageId": "drawing.strokeHidden", "suppressed": true }));
    assert_eq!(settings_file(&dir, "state")["data"]["suppressedMessages"], json!(["drawing.strokeHidden"]));
    assert_eq!(err(&h, "ui.tips.set", json!({ "messageId": "nope", "suppressed": true })).code, "ui.unknownMessage");
    assert_eq!(ok(&h, "ui.tips.reset", json!({}))["reenabled"], 1);
    // The status bar's visibility is the preference.
    ok(&h, "ui.statusBar.set", json!({ "visible": false }));
    assert_eq!(
        ok(&h, "prefs.get", json!({ "keys": ["interface.statusBar.visible"] }))["values"]["interface.statusBar.visible"],
        false
    );
    assert_eq!(ok(&h, "ui.statusBar.get", json!({}))["visible"], false);
    // Start screen: shown without a project by default, closable, follows the start-up preference.
    assert_eq!(ok(&h, "app.state.get", json!({}))["startScreenVisible"], true);
    ok(&h, "ui.startScreen.set", json!({ "visible": false }));
    assert_eq!(ok(&h, "app.state.get", json!({}))["startScreenVisible"], false);
    ok(&h, "prefs.set", json!({ "values": { "general.startup.behaviour": "newDefaultProject" } }));
    ok(&h, "ui.startScreen.set", json!({ "visible": true }));
    assert_eq!(ok(&h, "app.state.get", json!({}))["startScreenVisible"], true);
    // Help for every kind of target, typed refusals.
    for p in [
        json!({ "commandId": "edit.undo" }),
        json!({ "panelId": "history" }),
        json!({ "panelId": "drawingTools" }),
        json!({ "preferenceKey": "interface.theme" }),
        json!({ "messageId": "prefs.corrupt" }),
        json!({ "messageId": "drawing.strokeHidden" }),
    ] {
        let r = ok(&h, "ui.help.get", p.clone());
        assert!(!r["title"].as_str().unwrap().is_empty() && !r["text"].as_str().unwrap().is_empty(), "{p}");
    }
    assert_eq!(err(&h, "ui.help.get", json!({})).code, "arg.missing");
    assert_eq!(err(&h, "ui.help.get", json!({ "panelId": "x", "messageId": "y" })).code, "arg.invalid");
    assert_eq!(err(&h, "ui.help.get", json!({ "panelId": "nope" })).code, "window.panelNotFound");
    assert_eq!(err(&h, "ui.help.get", json!({ "commandId": "nope.nope" })).code, "command.unknown");
    assert_eq!(err(&h, "ui.help.get", json!({ "messageId": "nope" })).code, "ui.unknownMessage");
    assert_eq!(err(&h, "ui.preferences.open", json!({ "category": "nope" })).code, "arg.invalid");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Help exists for every registered command, panel, preference and listed message (01 §9.3).
#[test]
fn help_exists_for_every_command_panel_preference_and_message() {
    let h = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    for e in tt_executor::registry().unwrap().entries() {
        ok(&h, "ui.help.get", json!({ "commandId": e.id() }));
    }
    for p in ["cameraView", "drawingView", "timeline", "xsheet", "properties", "history", "capture", "soundTools"] {
        ok(&h, "ui.help.get", json!({ "panelId": p }));
    }
    for e in ok(&h, "prefs.list", json!({})).as_array().unwrap() {
        ok(&h, "ui.help.get", json!({ "preferenceKey": e["key"] }));
    }
    for code in ["project.unsavedChanges", "keymap.conflict", "workspace.last", "name.invalidCharacter"] {
        ok(&h, "ui.help.get", json!({ "messageId": code }));
    }
}

#[test]
fn user_themes_are_validated_and_selectable() {
    let dir = user_dir("themes");
    std::fs::create_dir_all(dir.join("Settings/Themes")).unwrap();
    std::fs::write(
        dir.join("Settings/Themes/night.json"),
        br##"{"format":"toontown-theme","formatVersion":"1.0","themeId":"night","name":"Night","dark":true,"colours":{"background":"#101010FF"}}"##,
    )
    .unwrap();
    std::fs::write(dir.join("Settings/Themes/broken.json"), b"{nope").unwrap();
    let h = over(&dir);
    let themes = ok(&h, "ui.theme.list", json!({}));
    let ids: Vec<&str> = themes.as_array().unwrap().iter().map(|t| t["themeId"].as_str().unwrap()).collect();
    assert_eq!(ids, ["system", "light", "dark", "night"], "the invalid file is ignored");
    assert_eq!(themes[3]["colours"]["background"], "#101010FF");
    ok(&h, "prefs.set", json!({ "values": { "interface.theme": "night" } }));
    let list = ok(&h, "prefs.list", json!({ "prefix": "interface.theme" }));
    assert!(list[0]["values"].as_array().unwrap().contains(&json!("night")));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// Native window commands and shell events
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn window_commands_are_staged_shell_effects() {
    let dir = user_dir("window");
    let h = over(&dir);
    let events = h.subscribe();
    let g = ok(&h, "window.geometry.get", json!({}));
    assert_eq!(g["width"], 1280);
    // A dry run publishes nothing and stores nothing.
    let env = with(
        &h,
        "window.geometry.set",
        json!({ "width": 1000 }),
        InvokeOptions { dry_run: true, ..Default::default() },
    );
    assert_eq!(env.status, Status::Ok);
    assert!(events.try_iter().all(|e| e.name != "shell.window"));
    assert!(!dir.join("Settings/state.json").exists());
    // Accepted: the host event and the stored geometry.
    ok(&h, "window.geometry.set", json!({ "width": 1000, "height": 700 }));
    let shell: Vec<_> = events.try_iter().filter(|e| e.name == "shell.window").collect();
    assert_eq!(shell.len(), 1);
    assert_eq!(shell[0].payload["width"], 1000);
    assert_eq!(settings_file(&dir, "state")["data"]["window"]["height"], 700);
    // Equal values publish nothing (no echo loop with the host).
    ok(&h, "window.geometry.set", json!({ "width": 1000 }));
    assert!(events.try_iter().all(|e| e.name != "shell.window"));
    // Off-screen geometry is clamped back onto the screen (01 §5.4).
    let g = ok(&h, "window.geometry.set", json!({ "x": -50_000, "y": 90_000 }));
    assert!(g["x"].as_i64().unwrap() >= 0 && g["y"].as_i64().unwrap() >= 0, "{g}");
    ok(&h, "window.minimized.set", json!({ "minimized": true }));
    ok(&h, "window.raiseAll.run", json!({}));
    let names: Vec<String> = events.try_iter().map(|e| e.name).collect();
    assert!(names.contains(&"shell.raiseAll".to_owned()), "{names:?}");
    ok(&h, "window.fullScreen.set", json!({ "enabled": true }));
    assert!(events.try_iter().any(|e| e.name == "shell.window" && e.payload["fullScreen"] == json!(true)));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspaces_export_and_import() {
    let dir = user_dir("ws");
    let h = over(&dir);
    let file = dir.join("mine.ttworkspace");
    ok(&h, "workspace.export", json!({ "name": "Drawing", "path": file.to_string_lossy() }));
    // Fail, rename and overwrite.
    assert_eq!(err(&h, "workspace.import", json!({ "path": file.to_string_lossy() })).code, "workspace.exists");
    assert_eq!(
        ok(&h, "workspace.import", json!({ "path": file.to_string_lossy(), "onExists": "rename" }))["name"],
        "Drawing 2"
    );
    assert_eq!(
        ok(&h, "workspace.import", json!({ "path": file.to_string_lossy(), "onExists": "overwrite" }))["name"],
        "Drawing"
    );
    let names: Vec<String> = ok(&h, "workspace.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"Drawing 2".to_owned()));
    std::fs::write(&file, b"{\"format\": \"other\"}").unwrap();
    assert_eq!(err(&h, "workspace.import", json!({ "path": file.to_string_lossy() })).code, "workspace.invalidFile");
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// Settings persistence across a restart (01 §11)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn settings_survive_a_restart_over_the_same_folder() {
    let dir = user_dir("restart");
    {
        let h = over(&dir);
        ok(&h, "prefs.set", json!({ "values": { "interface.theme": "dark", "general.recentProjects.maxCount": 4 } }));
        ok(&h, "keymap.set", json!({ "commandId": "project.save", "keys": "F2" }));
        ok(&h, "ui.tips.set", json!({ "messageId": "import.layersHint", "suppressed": true }));
        ok(&h, "workspace.create", json!({ "name": "Mine" }));
        ok(&h, "window.geometry.set", json!({ "x": 200, "y": 150, "width": 1100, "height": 720 }));
    }
    let h = over(&dir);
    assert_eq!(ok(&h, "prefs.get", json!({ "keys": ["interface.theme"] }))["values"]["interface.theme"], "dark");
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "F2" }))["effectiveCommand"]["commandId"], "project.save");
    let tips = ok(&h, "ui.tips.list", json!({}));
    assert!(tips.as_array().unwrap().iter().any(|t| t["messageId"] == "import.layersHint" && t["suppressed"] == true));
    let names: Vec<String> = ok(&h, "workspace.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"Mine".to_owned()), "{names:?}");
    let g = ok(&h, "window.geometry.get", json!({}));
    assert_eq!((g["x"].as_i64(), g["width"].as_u64()), (Some(200), Some(1100)));
    // Every file is a versioned settings file (01 §11).
    for kind in ["preferences", "keymap", "state", "workspaces"] {
        let f = settings_file(&dir, kind);
        assert_eq!(
            (f["format"].as_str(), f["formatVersion"].as_str()),
            (Some("toontown-settings"), Some("1.0")),
            "{kind}"
        );
    }
    drop(h);
    // A corrupt keymap file is kept aside and the defaults apply to it alone.
    std::fs::write(dir.join("Settings/keymap.json"), b"{broken").unwrap();
    let h = over(&dir);
    assert_eq!(ok(&h, "keymap.get", json!({ "keys": "F2" }))["effectiveCommand"], Value::Null);
    assert_eq!(ok(&h, "prefs.get", json!({ "keys": ["interface.theme"] }))["values"]["interface.theme"], "dark");
    let aside = std::fs::read_dir(dir.join("Settings"))
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with("keymap.corrupt-"));
    assert!(aside, "the corrupt file is kept aside");
    // A hand-edited invalid value falls back to the default for that key only.
    drop(h);
    let prefs = dir.join("Settings/preferences.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&prefs).unwrap()).unwrap();
    v["data"]["files.backups.count"] = json!(500);
    std::fs::write(&prefs, serde_json::to_vec(&v).unwrap()).unwrap();
    let h = over(&dir);
    let r = ok(&h, "prefs.get", json!({ "keys": ["files.backups.count", "interface.theme"] }));
    assert_eq!(r["values"], json!({ "files.backups.count": 3, "interface.theme": "dark" }));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same-place `window.panel.move` the GUI sends at the end of a splitter drag (SI-118, interim ruling O-3a): a
/// panel tabbed with itself in its own side area. In every built-in workspace and every side area the arrangement stays
/// byte-equal except that area's size; without `size` nothing changes and nothing is announced; any other placement
/// relative to itself is still refused.
#[test]
fn a_splitter_drag_is_recorded_by_a_same_place_panel_move() {
    let h = fixture();
    let names: Vec<String> = ok(&h, "workspace.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["builtIn"] == true)
        .map(|w| w["name"].as_str().unwrap().to_owned())
        .collect();
    let mut checked = 0;
    for name in names {
        ok(&h, "workspace.select", json!({ "name": name }));
        for area in ["left", "right", "top", "bottom"] {
            let before = ok(&h, "workspace.get", json!({}));
            let Some(group) = before["docks"][area].as_array().and_then(|g| g.first()).cloned() else { continue };
            let panel = group["current"].as_str().unwrap().to_owned();
            // Without `size`: a no-op, not announced.
            let events = h.subscribe();
            ok(
                &h,
                "window.panel.move",
                json!({ "panelId": panel, "area": area, "relativeTo": panel, "position": "tab" }),
            );
            assert_eq!(ok(&h, "workspace.get", json!({})), before, "{name} {area}: no-op");
            assert!(events.try_iter().all(|e| e.name != "workspace.changed"), "{name} {area}: nothing announced");
            // With `size`: only the area's size changes.
            let p = json!({ "panelId": panel, "area": area, "relativeTo": panel, "position": "tab", "size": 333 });
            ok(&h, "window.panel.move", p.clone());
            let after = ok(&h, "workspace.get", json!({}));
            let mut expected = before.clone();
            expected["docks"][area][0]["size"] = json!(333);
            assert_eq!(after.to_string(), expected.to_string(), "{name} {area}: {p}");
            // Any other self-relative placement, or another area, is still refused.
            let e = h.invoke(tt_executor::Invocation::user(
                "window.panel.move",
                json!({ "panelId": panel, "area": area, "relativeTo": panel, "position": "after" }),
            ));
            assert_eq!(e.error.map(|e| e.code).as_deref(), Some("arg.invalid"), "{name} {area}");
            checked += 1;
        }
    }
    assert!(checked >= 6, "every side area of every built-in workspace ({checked})");
}
