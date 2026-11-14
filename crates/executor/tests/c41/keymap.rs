//! M4 C4.1: the keymap module (01 §8): canonical sequences per platform, defaults conflict-free, mode precedence,
//! overrides round trip. Builder-owned.

use serde_json::json;
use tt_executor::keymap::*;
#[test]
fn sequences_are_canonical_and_platform_aware() {
    let w = Platform::Windows;
    let m = Platform::Macos;
    assert_eq!(parse("shift+primary+z", w).unwrap().canonical(), "Primary+Shift+Z");
    assert_eq!(parse("Ctrl+Z", w).unwrap().canonical(), "Primary+Z");
    assert_eq!(parse("Ctrl+Z", m).unwrap().canonical(), "Ctrl+Z");
    assert_eq!(parse("Cmd+Z", m).unwrap().canonical(), "Primary+Z");
    assert_eq!(parse("Primary+?", m).unwrap().canonical(), "Primary+Shift+/");
    assert_eq!(parse("Primary+Plus", w).unwrap().canonical(), "Primary+Shift+=");
    assert_eq!(parse("esc", w).unwrap().canonical(), "Escape");
    assert_eq!(parse("f11", w).unwrap().canonical(), "F11");
    assert_eq!(parse("Win+E", w).unwrap_err().code, codes::RESERVED_KEY);
    assert_eq!(parse("Shift", w).unwrap_err().code, codes::RESERVED_KEY);
    assert_eq!(parse("Primary+Primary+Z", w).unwrap_err().code, codes::INVALID_SEQUENCE);
    assert_eq!(parse("Shift+?", w).unwrap_err().code, codes::INVALID_SEQUENCE);
    assert_eq!(parse("Primary++", w).unwrap_err().code, codes::INVALID_SEQUENCE);
    assert_eq!(parse("Hyper+Z", w).unwrap_err().code, codes::INVALID_SEQUENCE);
    assert_eq!(parse("Primary+Shift+Z", m).unwrap().native(m), "\u{21E7}\u{2318}Z");
    assert_eq!(parse("Primary+Shift+Z", w).unwrap().native(w), "Ctrl+Shift+Z");
}

#[test]
fn defaults_parse_and_are_conflict_free_on_every_platform() {
    for p in [Platform::Windows, Platform::Macos, Platform::Linux] {
        assert_eq!(default_problems(p), Vec::<String>::new(), "{p:?}");
        assert!(!defaults(p).is_empty());
    }
}

#[test]
fn mode_context_wins_over_global() {
    let b = |c: &str, k: &str, ctx| Binding {
        command_id: c.into(),
        params: None,
        keys: k.into(),
        context: ctx,
        is_default: false,
    };
    let list = vec![b("a", "X", Context::Global), b("d", "X", Context::Drawing)];
    assert_eq!(resolve(&list, "X", Some(Context::Drawing)).unwrap().command_id, "d");
    assert_eq!(resolve(&list, "X", Some(Context::Staging)).unwrap().command_id, "a");
    assert_eq!(resolve(&list, "X", None).unwrap().command_id, "a");
    assert_eq!(conflicts(&list, &b("e", "X", Context::Staging)).len(), 1);
}

#[test]
fn overrides_round_trip_through_effective() {
    let p = Platform::Windows;
    let mut desired = effective(&Overrides::default(), p);
    desired.retain(|b| b.command_id != "project.save");
    desired.push(Binding {
        command_id: "project.save".into(),
        params: None,
        keys: "F2".into(),
        context: Context::Global,
        is_default: false,
    });
    let o = overrides_for(&desired, p);
    assert_eq!(o.removed.len(), 1);
    assert_eq!(o.added.len(), 1);
    let back = Overrides::from_stored(&o.to_stored());
    assert_eq!(back, o);
    let mut e = effective(&back, p);
    desired.sort_by(|a, b| (&a.command_id, a.context, &a.keys).cmp(&(&b.command_id, b.context, &b.keys)));
    e.sort_by(|a, b| (&a.command_id, a.context, &a.keys).cmp(&(&b.command_id, b.context, &b.keys)));
    assert_eq!(e, desired);
}

/// C41-R2-21: the length bounds of command ids, key sequences and looked-up names are the domain's, so an over-long
/// value answers the typed domain error, never a schema `arg.invalid` that pre-empts it; through `keymap.set`,
/// `keymap.remove`, `keymap.get`, `keymap.reset`, the import path, `ui.help.get`, `workspace.export` and
/// `inspect.ui.tree`.
#[test]
fn over_long_values_answer_their_domain_errors() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let code = |id: &str, p: serde_json::Value| h.invoke(tt_executor::Invocation::user(id, p)).error.unwrap().code;
    let long_id = "a".repeat(257);
    let long_keys = "x".repeat(65);
    assert_eq!(code("keymap.set", json!({ "commandId": long_id, "keys": "F2" })), "keymap.unknownCommand");
    assert_eq!(code("keymap.set", json!({ "commandId": "project.save", "keys": long_keys })), "keymap.invalidSequence");
    assert_eq!(code("keymap.remove", json!({ "commandId": long_id })), "keymap.unknownCommand");
    assert_eq!(
        code("keymap.remove", json!({ "commandId": "project.save", "keys": long_keys })),
        "keymap.invalidSequence"
    );
    assert_eq!(code("keymap.get", json!({ "commandId": long_id })), "keymap.unknownCommand");
    assert_eq!(code("keymap.get", json!({ "keys": long_keys })), "keymap.invalidSequence");
    assert_eq!(code("keymap.reset", json!({ "commandId": long_id })), "keymap.unknownCommand");
    let file = std::env::temp_dir().join(format!("tt-c41-long-{}.json", tt_executor::random_token(6)));
    for (command, keys, expected) in [
        (long_id.as_str(), "F2", "keymap.unknownCommand"),
        ("project.save", long_keys.as_str(), "keymap.invalidSequence"),
    ] {
        let data = json!({ "format": "toontown-keymap", "formatVersion": "1.0",
            "bindings": [{ "commandId": command, "keys": keys, "context": "global" }] });
        std::fs::write(&file, data.to_string()).unwrap();
        let e = h
            .invoke(tt_executor::Invocation::user("keymap.import", json!({ "path": file, "mode": "merge" })))
            .error
            .unwrap();
        assert_eq!((e.code.as_str(), e.details["index"].as_u64()), (expected, Some(0)));
    }
    let _ = std::fs::remove_file(&file);
    let long = "m".repeat(300);
    assert_eq!(code("ui.help.get", json!({ "messageId": long })), "ui.unknownMessage");
    assert_eq!(code("ui.help.get", json!({ "commandId": long })), "command.unknown");
    assert_eq!(code("ui.help.get", json!({ "panelId": long })), "window.panelNotFound");
    assert_eq!(code("ui.help.get", json!({ "preferenceKey": long })), "prefs.unknownKey");
    let target = std::env::temp_dir().join("tt-c41-long-workspace.json");
    assert_eq!(code("workspace.export", json!({ "name": long, "path": target })), "workspace.notFound");
    assert_eq!(code("inspect.ui.tree", json!({ "target": long })), "window.panelNotFound");
}
