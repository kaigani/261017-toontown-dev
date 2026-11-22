//! Drawing aids (02 §10, §13.5): user-scope settings, never undoable, announced as `drawAid.changed`; the static
//! light table per scene (SI-085, SI-126).

use serde_json::{Value, json};

use crate::common::*;
use crate::equivalence::{BOB, D1, history, ready};

#[test]
fn aids_have_their_defaults_change_without_undo_steps_and_are_announced() {
    let h = ready();
    let s = ok(&h, "drawAid.state.get", json!({}));
    assert_eq!(s["onionSkin"], json!({ "enabled": false, "previous": 1, "next": 0 }), "{s}");
    assert_eq!(s["lightTableShade"], json!({ "auto": true, "static": true }));
    assert_eq!(s["background"]["colour"], "#F0F0F0FF");
    assert_eq!(s["staticLightTable"], json!([]));
    let steps = history(&h).len();
    let rx = h.subscribe();
    assert_eq!(
        ok(&h, "drawAid.grid.set", json!({ "visible": true, "type": "field16" })),
        json!({ "visible": true, "type": "field16", "overlay": false })
    );
    ok(&h, "drawAid.grid.set", json!({ "visible": true }));
    let events: Vec<Value> = rx.try_iter().filter(|e| e.name == "drawAid.changed").map(|e| e.payload).collect();
    assert_eq!(events.len(), 1, "an unchanged value is not announced: {events:?}");
    assert_eq!(events[0]["aid"], "grid");
    assert_eq!(history(&h).len(), steps, "aids are not undoable (02 §12.1)");
    assert_eq!(err(&h, "drawAid.onionSkin.set", json!({ "previous": 4 })).code, "arg.invalid", "the schema's range");
    assert_eq!(err(&h, "drawAid.background.set", json!({ "colour": "red" })).code, "arg.invalid");
    let bg = ok(&h, "drawAid.background.set", json!({ "colour": "#112233" }));
    assert_eq!(bg["colour"], "#112233FF", "{bg}");
}

#[test]
fn onion_skin_cycles_zero_to_three_and_wraps() {
    let h = ready();
    let seen: Vec<i64> = (0..4)
        .map(|_| ok(&h, "drawAid.onionSkinCycle.run", json!({ "direction": "previous" }))["previous"].as_i64().unwrap())
        .collect();
    assert_eq!(seen, vec![2, 3, 0, 1]);
    assert_eq!(ok(&h, "drawAid.onionSkinCycle.run", json!({ "direction": "next" }))["next"], 1);
}

#[test]
fn the_static_light_table_holds_references_per_scene_and_drops_deleted_sources() {
    let h = ready();
    let a = ok(&h, "drawAid.staticLightTable.add", json!({}))["entryId"].clone();
    let b = ok(&h, "drawAid.staticLightTable.add", json!({ "elementId": BOB, "frame": 1, "placement": "overlay" }))
        ["entryId"]
        .clone();
    assert_ne!(a, b);
    let list = ok(&h, "drawAid.staticLightTable.list", json!({}))["entries"].clone();
    assert_eq!(list.as_array().unwrap().len(), 2, "{list}");
    assert_eq!(list[0]["sourceId"], D1);
    assert_eq!((list[0]["placement"].as_str(), list[1]["placement"].as_str()), (Some("underlay"), Some("overlay")));
    // Hide all others; then overlay all.
    let set = ok(&h, "drawAid.staticLightTable.set", json!({ "onlyVisibleEntryId": b }))["entries"].clone();
    assert_eq!((set[0]["visible"].as_bool(), set[1]["visible"].as_bool()), (Some(false), Some(true)), "{set}");
    let set = ok(&h, "drawAid.staticLightTable.set", json!({ "all": true, "placement": "overlay" }))["entries"].clone();
    assert!(set.as_array().unwrap().iter().all(|e| e["placement"] == "overlay"));
    // The Exposure Sheet's light table is the same list (SI-085).
    assert_eq!(ok(&h, "xsheet.lightTable.list", json!({}))["items"].as_array().unwrap().len(), 2);
    let removed = ok(&h, "drawAid.staticLightTable.remove", json!({ "entryIds": [a] }))["removedEntryIds"].clone();
    assert_eq!(removed, json!([a]));
    assert_eq!(err(&h, "drawAid.staticLightTable.remove", json!({ "entryIds": ["lt99"] })).code, "ref.notFound");
    // Deleting the source removes its entry (02 §10.4).
    ok(&h, "element.delete", json!({ "elementIds": [BOB] }));
    assert_eq!(ok(&h, "drawAid.staticLightTable.list", json!({}))["entries"], json!([]));
    assert_eq!(ok(&h, "drawAid.state.get", json!({}))["staticLightTable"], json!([]));
}

#[test]
fn light_table_edits_announce_only_effective_changes() {
    let h = ready();
    let a = ok(&h, "drawAid.staticLightTable.add", json!({}))["entryId"].clone();
    let rx = h.subscribe();
    ok(&h, "drawAid.staticLightTable.set", json!({ "entryIds": [a], "visible": true }));
    ok(&h, "drawAid.staticLightTable.set", json!({ "all": true, "placement": "underlay" }));
    assert_eq!(rx.try_iter().filter(|e| e.name == "drawAid.changed").count(), 0, "nothing changed");
    ok(&h, "drawAid.staticLightTable.set", json!({ "all": true, "visible": false }));
    assert_eq!(rx.try_iter().filter(|e| e.name == "drawAid.changed").count(), 1);
}
