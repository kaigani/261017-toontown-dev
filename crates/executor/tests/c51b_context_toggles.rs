//! M5 C5.1b round 1, C51B-R1S-04 (04 line 708: "Toggle commands MUST present their current state"): the Timeline
//! contexts' toggles and radio items, as the kernel reports and completes them (ruling O-10). The ruler's Automatic Fit
//! and Free show which mode is current; Show Effect Parameters on one effect, Sound Waveforms and Toggle Constant Z
//! report their state and complete to the next one, so each turns the setting off as well as on.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::ExecutorHandle;

fn facts(h: &ExecutorHandle, command: &str, params: Value) -> tt_executor::host::Availability {
    let p: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    h.availability(vec![(command.to_owned(), p)]).unwrap().remove(0)
}

/// Runs the toggle's completed reference twice: the state goes on, off, and the check mark follows it.
fn round_trip(h: &ExecutorHandle, command: &str, reference: Value, key: &str, starts_on: bool) {
    let first = facts(h, command, reference.clone());
    assert!(first.enabled, "{command}: {:?}", first.reason);
    assert_eq!(first.checked, Some(starts_on), "{command}: the state shown");
    assert_eq!(first.params[key], !starts_on, "{command}: the next state");
    ok(h, command, first.params.clone());
    let second = facts(h, command, reference.clone());
    assert_eq!(second.checked, Some(!starts_on), "{command}: the state shown after the first run");
    assert_eq!(second.params[key], starts_on, "{command}: the next state after the first run");
    ok(h, command, second.params.clone());
    assert_eq!(facts(h, command, reference).checked, Some(starts_on), "{command}: back");
}

fn bob(h: &ExecutorHandle) -> String {
    ok(h, "element.list", json!({}))["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Bob")
        .and_then(|e| e["id"].as_str())
        .unwrap()
        .to_owned()
}

#[test]
fn the_ruler_shows_the_current_range_mode() {
    let h = fixture();
    let checked = |mode: &str| facts(&h, "timeline.playbackRange.set", json!({ "mode": mode })).checked;
    assert_eq!((checked("auto"), checked("free")), (Some(true), Some(false)));
    ok(&h, "timeline.playbackRange.set", json!({ "mode": "free", "start": 2, "end": 5 }));
    assert_eq!((checked("auto"), checked("free")), (Some(false), Some(true)));
    // A reference that sets the range's ends is not a radio item: no check mark.
    assert_eq!(facts(&h, "timeline.playbackRange.set", json!({ "start": 3 })).checked, None);
}

#[test]
fn show_effect_parameters_turns_one_effects_rows_off_and_on() {
    let h = fixture();
    let clip = ok(&h, "element.create", json!({ "type": "clipping" }))["elementIds"][0].as_str().unwrap().to_owned();
    round_trip(&h, "timeline.effectParameterRows.set", json!({ "elementId": clip }), "visible", true);
    // Hidden for every effect, the one effect's item shows it hidden and turns it on.
    ok(&h, "timeline.effectParameterRows.set", json!({ "visible": false }));
    round_trip(&h, "timeline.effectParameterRows.set", json!({ "elementId": clip }), "visible", false);
}

#[test]
fn sound_waveforms_is_a_toggle() {
    let h = fixture();
    round_trip(&h, "timeline.soundWaveforms.set", json!({}), "enabled", true);
}

#[test]
fn toggle_constant_z_shows_and_completes_the_segments_state() {
    let h = fixture();
    let bob = bob(&h);
    ok(&h, "anim.key.add", json!({ "nodeIds": [bob], "frame": 1 }));
    ok(&h, "anim.key.add", json!({ "nodeIds": [bob], "frame": 5 }));
    round_trip(&h, "anim.path.lockDepth.set", json!({ "nodeId": bob, "frame": 1 }), "enabled", false);
    // No motion keyframe at the frame: no state to show, the reference is not completed.
    let none = facts(&h, "anim.path.lockDepth.set", json!({ "nodeId": bob, "frame": 3 }));
    assert_eq!(none.checked, None);
    assert!(none.params.get("enabled").is_none(), "{:?}", none.params);
}
