//! M5 C5.1b (builder): the Play menu's toggles and radio items show the kernel's state (04 line 708; ruling O-10): Loop
//! and Force Frame Rate complete to their next state and are checked when on; a Playback Rate item is checked when it
//! names the rate in effect; a Playback Range item when it names the current scene's mode.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::ExecutorHandle;

fn facts(h: &ExecutorHandle, command: &str, params: Value) -> tt_executor::host::Availability {
    let p: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    h.availability(vec![(command.to_owned(), p)]).unwrap().remove(0)
}

#[test]
fn loop_and_force_rate_complete_to_their_next_state_and_show_it() {
    let h = fixture();
    for (command, on) in [("playback.loop.set", "enabled"), ("playback.forceRate.set", "enabled")] {
        let off = facts(&h, command, json!({}));
        assert_eq!(off.checked, Some(false), "{command}");
        assert_eq!(off.params[on], true, "{command}: the next state");
        ok(&h, command, json!({ on: true }));
        let now = facts(&h, command, json!({}));
        assert_eq!(now.checked, Some(true), "{command}");
        assert_eq!(now.params[on], false, "{command}: the next state");
    }
}

#[test]
fn a_rate_item_is_checked_when_it_names_the_rate_in_effect() {
    let h = fixture();
    assert_eq!(facts(&h, "playback.rate.set", json!({"fps": "project"})).checked, Some(true));
    assert_eq!(facts(&h, "playback.rate.set", json!({"fps": 12})).checked, Some(false));
    ok(&h, "playback.rate.set", json!({"fps": 12}));
    assert_eq!(facts(&h, "playback.rate.set", json!({"fps": 12})).checked, Some(true));
    assert_eq!(facts(&h, "playback.rate.set", json!({"fps": 24})).checked, Some(false));
    assert_eq!(facts(&h, "playback.rate.set", json!({"fps": "project"})).checked, Some(false));
}

#[test]
fn a_range_item_is_checked_when_it_names_the_scenes_mode() {
    let h = fixture();
    assert_eq!(facts(&h, "playback.range.mode.set", json!({"mode": "auto"})).checked, Some(true));
    assert_eq!(facts(&h, "playback.range.mode.set", json!({"mode": "free"})).checked, Some(false));
    ok(&h, "playback.range.mode.set", json!({"mode": "free"}));
    assert_eq!(facts(&h, "playback.range.mode.set", json!({"mode": "free"})).checked, Some(true));
    assert_eq!(facts(&h, "playback.range.mode.set", json!({"mode": "auto"})).checked, Some(false));
}
