//! Ruling D-38 (C43B-R1-06; 11 §7.5 line 339): the window's UI tree always carries the menu bar, built from the menus the
//! host reports (the menu manifest for its platform) and the live availability: each item's role, label, enabled
//! state (with the reason when disabled), check state where it has one, shortcut and the command it invokes with the
//! parameters a click runs. It does not depend on a menu being open. Headless there is no menu bar and the tree says
//! so.

use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle};

use crate::common::*;

const MENU: &str = include_str!("../../../../desktop/src/ui-bindings/menu.json");

fn report_menus(h: &ExecutorHandle) {
    let menus = serde_json::from_str::<Value>(MENU).unwrap()["menus"].clone();
    h.report_shell(serde_json::from_value(json!({ "menuBar": menus })).unwrap());
}

fn bar(h: &ExecutorHandle) -> Value {
    let tree = ok(h, "inspect.ui.tree", json!({}));
    let children = tree["root"]["children"].as_array().unwrap();
    let last = children.last().cloned().unwrap();
    assert_eq!(last["role"], "menuBar", "the menu bar is the window's last child: {last}");
    assert_eq!(children.iter().filter(|c| c["role"] == "menuBar").count(), 1);
    last
}

fn items(n: &Value, out: &mut Vec<Value>) {
    if n["role"] == "menuitem" {
        out.push(n.clone());
    }
    for c in n["children"].as_array().into_iter().flatten() {
        items(c, out);
    }
}

fn item(bar: &Value, command: &str, params: &Value) -> Value {
    let mut all = Vec::new();
    items(bar, &mut all);
    all.into_iter()
        .find(|i| {
            i["commandId"] == command && {
                let p = &i["command"]["params"];
                params.as_object().unwrap().iter().all(|(k, v)| &p[k] == v)
            }
        })
        .unwrap_or_else(|| panic!("no menu item for {command} {params}"))
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
fn d38_the_menu_bar_carries_every_item_with_live_state() {
    let h = fixture();
    report_menus(&h);
    let b = bar(&h);
    assert_eq!(b["present"], true);
    // Every command item of the manifest is in the tree with its command.
    let mut want = 0;
    fn count(items: &[Value], n: &mut usize) {
        for i in items {
            if i["kind"] == "command" {
                *n += 1;
            }
            if let Some(sub) = i["items"].as_array() {
                count(sub, n);
            }
        }
    }
    for m in serde_json::from_str::<Value>(MENU).unwrap()["menus"].as_array().unwrap() {
        count(m["items"].as_array().unwrap(), &mut want);
    }
    let mut all = Vec::new();
    items(&b, &mut all);
    let with_command = all.iter().filter(|i| i["command"]["commandId"].is_string()).count();
    assert!(with_command >= want, "{with_command} command items in the tree, {want} in the manifest");
    // Undo: disabled with its reason while there is nothing to undo; its shortcut; then enabled with the step's label.
    let undo = item(&b, "edit.undo", &json!({}));
    assert_eq!(undo["enabled"], false, "{undo}");
    assert!(undo["reason"].is_string(), "{undo}");
    assert_eq!(undo["shortcut"], "Primary+Z");
    ok(&h, "app.context.set", json!({ "elementId": crate::camera::BOB, "frame": 1 }));
    ok(&h, "draw.rectangle.add", json!({ "from": { "x": -50, "y": -50 }, "to": { "x": 50, "y": 50 } }));
    let undo = item(&bar(&h), "edit.undo", &json!({}));
    assert_eq!(undo["enabled"], true, "{undo}");
    assert_eq!(undo["label"], "Undo Rectangle Tool");
    // A check item: the status bar shown, and the parameters a click runs (its next state).
    let status = item(&bar(&h), "ui.statusBar.set", &json!({}));
    assert_eq!(status["checked"], true, "{status}");
    assert_eq!(status["command"]["params"], json!({ "visible": false }));
    ok(&h, "ui.statusBar.set", json!({ "visible": false }));
    let status = item(&bar(&h), "ui.statusBar.set", &json!({}));
    assert_eq!(status["checked"], false, "{status}");
    assert_eq!(status["command"]["params"], json!({ "visible": true }));
    // A panel's check item.
    let panel = item(&bar(&h), "window.panel.set", &json!({ "panelId": "drawingView" }));
    assert!(panel["checked"].is_boolean(), "{panel}");
}

#[test]
fn d38_without_a_project_the_project_items_are_disabled() {
    let h = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    report_menus(&h);
    let b = bar(&h);
    let save = item(&b, "project.save", &json!({}));
    assert_eq!(save["enabled"], false, "{save}");
    assert!(save["reason"].is_string(), "{save}");
    let open = item(&b, "project.open", &json!({}));
    assert_eq!(open["enabled"], true, "{open}");
}
