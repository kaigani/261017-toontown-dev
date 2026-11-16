//! Ruling D-38 (C43B-R1-06; 11 §7.5 line 339) and C43B-R2S-03: the window's UI tree always carries the menu bar as the
//! person sees it. The host makes one platform-aware model of its native menu at every refresh and sets the native items
//! from it (`desktop/src-tauri/src/menu_model.rs`, whose tests fix the labels, enabled and check states: a missing recent
//! project, the current workspace, the macOS predefined roles); the kernel's tree carries that model exactly as
//! reported, as the window's last child, whether or not a menu is open. Headless there is no menu bar; an attached
//! window whose host has not reported its menus yet says so.

use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

use crate::common::*;

fn bar(h: &ExecutorHandle) -> Value {
    let tree = ok(h, "inspect.ui.tree", json!({}));
    let children = tree["root"]["children"].as_array().unwrap();
    let last = children.last().cloned().unwrap();
    assert_eq!(last["role"], "menuBar", "the menu bar is the window's last child: {last}");
    assert_eq!(children.iter().filter(|c| c["role"] == "menuBar").count(), 1);
    last
}

/// A model as the host reports it: a File menu with a disabled command, Open Recent with a missing project, and the
/// macOS application menu's Hide Others.
fn model() -> Value {
    json!([
        { "role": "menu", "id": "menu.app", "label": "Toontown", "enabled": true, "children": [
            { "role": "menuitem", "id": "menu:Toontown/Hide Others", "label": "Hide Others", "enabled": true, "predefined": "HideOthers" }
        ] },
        { "role": "menu", "id": "menu.file", "label": "File", "enabled": true, "children": [
            { "role": "menuitem", "id": "menu:File/Save", "label": "Save", "enabled": false, "reason": "no project is open",
              "shortcut": "Primary+S", "commandId": "project.save", "command": { "commandId": "project.save", "params": {} } },
            { "role": "menu", "id": "menu:File/Open Recent", "label": "Open Recent", "enabled": true, "children": [
                { "role": "menuitem", "id": "dyn:recent:0", "label": "Gone (missing)", "enabled": true, "commandId": "project.open",
                  "command": { "commandId": "project.open", "params": { "path": "/p/Gone.animpkg" } } }
            ] }
        ] }
    ])
}

#[test]
fn d38_headless_has_no_menu_bar_and_says_so() {
    let h = fixture();
    let b = bar(&h);
    assert_eq!(b["present"], false);
    assert!(b["reason"].as_str().unwrap().contains("headless"), "{b}");
    assert_eq!(b["children"], json!([]));
}

#[test]
fn d38_an_attached_window_without_a_report_says_so() {
    let h = fixture();
    h.report_shell(serde_json::from_value(json!({ "attached": true })).unwrap());
    let b = bar(&h);
    assert_eq!(b["present"], false);
    assert!(b["reason"].as_str().unwrap().contains("not reported"), "{b}");
}

#[test]
fn c43b_r2s_03_the_tree_carries_the_hosts_model_exactly() {
    let h = fixture();
    h.report_shell(serde_json::from_value(json!({ "menuBar": model() })).unwrap());
    let b = bar(&h);
    assert_eq!(b["present"], true);
    assert_eq!(b["children"], model(), "the menus as reported, nothing re-derived");
    // A later refresh replaces them: the tree follows the native menu's state, not the kernel's own reading.
    let mut next = model();
    next[1]["children"][0]["enabled"] = json!(true);
    next[1]["children"][0].as_object_mut().unwrap().remove("reason");
    h.report_shell(serde_json::from_value(json!({ "menuBar": next.clone() })).unwrap());
    assert_eq!(bar(&h)["children"], next);
}
