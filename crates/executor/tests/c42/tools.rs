//! Tools, options, pens and the global drawing settings (02 §5, §6, §12; design note 3 §1, invariant G7; rulings D-7d
//! and D-7e).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, ExecutorOptions, Invocation, PersistOptions, UserDirs};

use crate::common::*;
use crate::equivalence::{history, ready};

fn root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tt-c42-tools-{tag}-{}", tt_executor::random_token(6)))
}

fn over(root: &Path) -> ExecutorHandle {
    let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions { user_dirs: Some(UserDirs::under(root)), ..Default::default() },
        ..Default::default()
    })
    .unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    h
}

#[test]
fn g7_tool_selection_and_options_are_not_undoable_and_pens_are() {
    let h = ready();
    let steps = history(&h).len();
    ok(&h, "tool.active.set", json!({ "toolId": "brush" }));
    ok(&h, "tool.polyline.options.set", json!({ "snap": false, "snapRadius": 20 }));
    ok(&h, "tool.placeBehind.set", json!({ "enabled": true }));
    assert_eq!(history(&h).len(), steps, "no undo step for tool state (02 §12.1)");
    let created = ok(&h, "tool.pen.create", json!({ "name": "Wide", "settings": { "maxSize": 40 } }));
    let hist = history(&h);
    assert_eq!(hist.len(), steps + 1);
    assert_eq!(hist.last().unwrap()["label"], "New Pen");
    let pens = ok(&h, "tool.pen.list", json!({}));
    assert_eq!(pens["currentBrushPenId"], created["penId"], "a new pen becomes current");
    let wide = pens["pens"].as_array().unwrap().iter().find(|p| p["name"] == "Wide").unwrap().clone();
    assert_eq!((wide["maxSize"].as_f64(), wide["type"].as_str()), (Some(40.0), Some("brush")));
    ok(&h, "edit.undo", json!({}));
    let names: Vec<Value> =
        ok(&h, "tool.pen.list", json!({}))["pens"].as_array().unwrap().iter().map(|p| p["name"].clone()).collect();
    assert!(!names.contains(&json!("Wide")), "{names:?}");
    // Undo leaves the tool state.
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "brush");
    assert_eq!(
        ok(&h, "tool.options.get", json!({ "toolId": "polyline" }))["options"]["snapRadius"].as_f64(),
        Some(20.0)
    );
}

#[test]
fn group_select_shows_the_last_used_tool_and_cycles_when_active() {
    let h = ready();
    assert_eq!(ok(&h, "tool.group.select", json!({ "group": "brush" }))["toolId"], "brush");
    assert_eq!(ok(&h, "tool.group.select", json!({ "group": "brush" }))["toolId"], "pencil", "cycling");
    assert_eq!(ok(&h, "tool.group.select", json!({ "group": "eraser" }))["toolId"], "eraser");
    assert_eq!(ok(&h, "tool.group.select", json!({ "group": "brush" }))["toolId"], "pencil", "the last used");
    for _ in 0..6 {
        ok(&h, "tool.group.select", json!({ "group": "brush" }));
    }
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "pencil", "six tools wrap around");
    let list = ok(&h, "tool.list", json!({}));
    assert_eq!(list["tools"].as_array().unwrap().len(), 23);
    assert_eq!(list["activeToolId"], "pencil");
    assert_eq!(err(&h, "tool.active.set", json!({ "toolId": "nope" })).code, "arg.invalid");
}

#[test]
fn tool_keys_reach_the_group_commands_in_the_drawing_view() {
    let h = ready();
    ok(&h, "view.active.set", json!({ "viewId": "drawingView" }));
    match h.resolve_key("b").unwrap() {
        tt_executor::host::KeyResolution::Command { command_id, params, .. } => {
            assert_eq!((command_id.as_str(), params), ("tool.group.select", json!({ "group": "brush" })));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn options_are_checked_all_or_nothing_and_announced() {
    let h = ready();
    let rx = h.subscribe();
    let e = err(&h, "tool.options.set", json!({ "toolId": "polyline", "options": { "snapRadius": 12, "nope": 1 } }));
    assert_eq!((e.code.as_str(), &e.details["pointer"]), ("tool.notApplicable", &json!("/options/nope")));
    let e = err(&h, "tool.options.set", json!({ "toolId": "polyline", "options": { "snapRadius": 99 } }));
    assert_eq!(e.code, "tool.valueOutOfRange");
    assert_eq!(ok(&h, "tool.options.get", json!({ "toolId": "polyline" }))["options"]["snapRadius"], 8, "unchanged");
    ok(&h, "tool.options.set", json!({ "toolId": "polyline", "options": { "snapRadius": 12 } }));
    let events: Vec<Value> = rx.try_iter().filter(|e| e.name == "tool.options.changed").map(|e| e.payload).collect();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["toolId"], "polyline");
    // The thickness source is shared by the Pencil and the shape tools (02 §13.2).
    ok(&h, "tool.pencil.options.set", json!({ "thicknessSource": "fixed", "thickness": 2.5 }));
    for t in ["pencil", "rectangle", "ellipse", "line", "polyline"] {
        let o = ok(&h, "tool.options.get", json!({ "toolId": t }))["options"].clone();
        assert_eq!((o["thicknessSource"].as_str(), o["thickness"].as_f64()), (Some("fixed"), Some(2.5)), "{t}");
    }
}

#[test]
fn d7e_absent_null_and_a_value_differ_through_real_serialisation() {
    let h = ready();
    let set =
        |v: Value| h.invoke(Invocation::user("tool.text.options.set", serde_json::from_str(&v.to_string()).unwrap()));
    assert!(set(json!({ "outlineColourId": "c_red" })).is_ok());
    let get = || ok(&h, "tool.options.get", json!({ "toolId": "text" }))["options"]["outlineColourId"].clone();
    assert_eq!(get(), "c_red");
    assert!(set(json!({ "size": 40 })).is_ok());
    assert_eq!(get(), "c_red", "absent leaves it");
    assert!(set(json!({ "outlineColourId": null })).is_ok());
    assert_eq!(get(), Value::Null, "null clears it");
}

#[test]
fn pens_keep_their_rules() {
    let h = ready();
    let e = err(&h, "tool.pen.create", json!({ "name": "Pen 1" }));
    assert_eq!(e.code, "pen.nameExists");
    let e1 = ok(&h, "tool.pen.list", json!({ "type": "eraser" }))["pens"][0]["penId"].clone();
    assert_eq!(err(&h, "tool.pen.delete", json!({ "penId": e1 })).code, "pen.lastPen", "the last eraser pen");
    assert_eq!(err(&h, "tool.brush.options.set", json!({ "penId": e1 })).code, "tool.notApplicable");
    let p1 = ok(&h, "tool.pen.list", json!({ "type": "brush" }))["pens"][0]["penId"].clone();
    // Raising the minimum above the maximum pushes the maximum up (02 §6.1).
    let pen = ok(&h, "tool.pen.set", json!({ "penId": p1, "minSize": 30 }))["pen"].clone();
    assert_eq!((pen["minSize"].as_f64(), pen["maxSize"].as_f64()), (Some(30.0), Some(30.0)), "{pen}");
    assert_eq!(history(&h).last().unwrap()["label"], "Set Pen Style");
    ok(&h, "tool.pen.rename", json!({ "penId": p1, "name": "Fine" }));
    assert_eq!(history(&h).last().unwrap()["label"], "Rename Pen");
    let rx = h.subscribe();
    let removed = ok(&h, "tool.pen.delete", json!({ "penId": p1 }));
    assert_eq!(removed["removedPenId"], p1);
    let change: Vec<Value> = rx.try_iter().filter(|e| e.name == "tool.pen.changed").map(|e| e.payload).collect();
    assert_eq!(change, vec![json!({ "penId": p1, "change": "deleted" })]);
}

#[test]
fn d7d_drawing_settings_persist_per_user_and_a_corrupt_file_is_told() {
    let r = root("persist");
    {
        let h = over(&r);
        ok(&h, "tool.active.set", json!({ "toolId": "ellipse" }));
        ok(&h, "tool.symmetry.set", json!({ "mode": "mirror", "angle": 15 }));
        ok(&h, "drawAid.grid.set", json!({ "visible": true, "type": "field12" }));
        ok(&h, "app.state.get", json!({}));
    }
    let h = over(&r);
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "ellipse");
    let s = ok(&h, "tool.settings.get", json!({}));
    assert_eq!(s["symmetry"], json!({ "mode": "mirror", "angle": 15.0, "mirrors": 6 }), "{s}");
    assert_eq!(ok(&h, "drawAid.state.get", json!({}))["grid"]["type"], "field12");
    drop(h);
    // A corrupt file: told once with its reason, and the defaults are used.
    let dirs = UserDirs::under(&r);
    let file = dirs.settings.join("drawing.json");
    std::fs::write(&file, b"{ not json").unwrap();
    let h = ExecutorHandle::spawn_with_options(ExecutorOptions {
        persist: PersistOptions { user_dirs: Some(UserDirs::under(&r)), ..Default::default() },
        ..Default::default()
    })
    .unwrap();
    let rx = h.subscribe();
    assert!(h.open_document(tt_model::fixture::sample_document(1).unwrap(), None, false).is_ok());
    let _ = ok(&h, "app.state.get", json!({}));
    let shown: Vec<Value> = rx.try_iter().filter(|e| e.name == "ui.message.shown").map(|e| e.payload).collect();
    assert!(shown.iter().any(|m| m.to_string().contains("drawing")), "{shown:?}");
    assert_eq!(ok(&h, "tool.active.get", json!({}))["toolId"], "select", "defaults");
    drop(h);
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn symmetry_is_a_global_drawing_setting_read_by_strokes() {
    let h = ready();
    ok(&h, "tool.symmetry.set", json!({ "mode": "kaleidoscope", "angle": 0, "mirrors": 4 }));
    let s = ok(&h, "tool.settings.get", json!({}));
    assert_eq!(s["symmetry"]["mode"], "kaleidoscope");
    assert_eq!(s["gapClosing"], "off");
    let r = ok(
        &h,
        "draw.stroke.add",
        json!({ "kind": "pencil", "points": [{"x": 10, "y": 10}, {"x": 40, "y": 30}], "symmetry": true }),
    );
    assert_eq!(r["strokeIds"].as_array().unwrap().len(), 8, "a kaleidoscope of 4 mirrors draws 8 copies: {r}");
}
