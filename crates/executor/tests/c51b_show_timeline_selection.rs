//! M5 C5.1b (builder): Show Timeline Selection (04 line 581; plan §6.5), the Element menu item and the default key `/`:
//! both send `timeline.view.set {revealElementId: null}`, which reveals the current element's row in the kernel, so a
//! person's key, the menu item and an agent's `key` reveal the same row; `{}` still only reads the view.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::host::KeyResolution;

fn view(h: &tt_executor::ExecutorHandle) -> Value {
    ok(h, "timeline.view.set", json!({}))
}

#[test]
fn the_key_and_the_menu_reveal_the_current_elements_row() {
    let h = fixture();
    // An element exposed far right of the view (frames 300-310), made current; the view starts at frame 1.
    let id = ok(&h, "element.create", json!({"type": "drawing", "name": "Far"}))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "element.drawing.create", json!({"elementId": id, "name": "1", "exposeFrom": 300, "exposeTo": 310}));
    ok(&h, "app.context.set", json!({"elementId": id, "frame": 1}));
    ok(&h, "timeline.view.set", json!({"firstFrame": 1}));
    assert_eq!(view(&h)["firstFrame"], 1, "{{}} only reads the view");

    // The default key resolves to the menu item's reference.
    match h.resolve_key("/").unwrap() {
        KeyResolution::Command { command_id, params, .. } => {
            assert_eq!(command_id, "timeline.view.set");
            assert_eq!(params, json!({"revealElementId": null}));
        }
        other => panic!("`/` resolves to {other:?}"),
    }
    let revealed = ok(&h, "timeline.view.set", json!({"revealElementId": null}));
    assert_eq!(revealed["firstFrame"], 300, "null reveals the current element's row: {revealed}");

    // The same as naming the element.
    ok(&h, "timeline.view.set", json!({"firstFrame": 1}));
    assert_eq!(ok(&h, "timeline.view.set", json!({"revealElementId": id}))["firstFrame"], 300);
}
