//! M4 C4.1: the keymap module (01 §8): canonical sequences per platform, defaults conflict-free, mode precedence,
//! overrides round trip. Builder-owned.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

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
