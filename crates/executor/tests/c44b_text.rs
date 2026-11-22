//! M4 C4.4b, the Text tool (02 §7.6 lines 664-681, §12.1 lines 1064-1069; design note 5; SI-191 to SI-195): the
//! gesture rows, the edit session, the one input path and the typing run (one "Typing" step per run, no timers).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::text_edit::Input;
use tt_executor::{ExecutorHandle, Origin, Status};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

/// The headless Drawing view's default mapping: drawing unit -> view pixel.
fn v(x: f64, y: f64) -> (f64, f64) {
    (x + 960.0, 540.0 - y)
}

fn drag(points: &[(f64, f64)], mods: &[&str]) -> Vec<Value> {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let ty = if i == 0 {
                "down"
            } else if i + 1 == n {
                "up"
            } else {
                "move"
            };
            json!({ "type": ty, "x": x, "y": y, "modifiers": mods })
        })
        .collect()
}

fn click(p: (f64, f64)) -> Vec<Value> {
    drag(&[p, p], &[])
}

fn labels(h: &ExecutorHandle) -> Vec<String> {
    ok(h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["label"].as_str().unwrap().to_owned())
        .collect()
}

/// A text object's `text`, `runs` and `paragraphs`.
fn content(h: &ExecutorHandle, id: &str) -> Value {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap();
    json!({ "text": n["text"], "runs": n["runs"], "paragraphs": n["paragraphs"] })
}

fn pointer(h: &ExecutorHandle, events: Vec<Value>) -> tt_executor::Envelope {
    h.pointer(Origin::User, json!({ "view": "drawing", "tool": "text", "events": events }))
}

fn input(h: &ExecutorHandle, i: Input) -> tt_executor::Envelope {
    h.text_input(Origin::User, i)
}

fn session(h: &ExecutorHandle) -> Value {
    h.text_session().unwrap().unwrap_or(Value::Null)
}

fn text_of(h: &ExecutorHandle, id: &str) -> String {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    d["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .map_or_else(String::new, |n| n["text"].as_str().unwrap_or_default().to_owned())
}

/// A click on empty canvas with the Text tool: `draw.text.add` at the point with the Text panel defaults and the current
/// colour, then a session at caret 0 (02 line 669; design note 5 §2).
fn new_text(h: &ExecutorHandle, at: (f64, f64)) -> String {
    let env = pointer(h, click(v(at.0, at.1)));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    let add = commands.iter().find(|c| c["commandId"] == "draw.text.add").expect("draw.text.add");
    add["envelope"]["result"]["textId"].as_str().unwrap().to_owned()
}

#[test]
fn a_click_on_empty_canvas_adds_text_with_the_panel_defaults_and_opens_a_session() {
    let h = ready();
    ok(&h, "tool.text.options.set", json!({ "size": 48, "bold": true }));
    let env = pointer(&h, click(v(-700.0, -400.0)));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    let add = &commands[1];
    assert_eq!(add["commandId"], "draw.text.add");
    assert_eq!(add["params"]["position"], json!({ "x": -700.0, "y": -400.0 }));
    assert_eq!(add["params"]["text"], "");
    assert_eq!(add["params"]["format"]["size"].as_f64(), Some(48.0));
    assert_eq!(add["params"]["format"]["bold"], true);
    let id = add["envelope"]["result"]["textId"].as_str().unwrap().to_owned();
    let s = session(&h);
    assert_eq!((s["textId"].as_str(), s["caret"].as_u64()), (Some(id.as_str()), Some(0)));
    assert_eq!(labels(&h).last().unwrap(), "New Text Object", "02 line 1066");
}

#[test]
fn five_keystrokes_are_one_typing_step_and_equal_one_insert() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    for c in ["H", "e", "l", "l", "o"] {
        let env = input(&h, Input::Text(c.into()));
        assert_eq!(env.status, Status::Ok, "{env:?}");
    }
    assert_eq!(text_of(&h, &id), "Hello");
    let l = labels(&h);
    assert_eq!(&l[l.len() - 2..], ["New Text Object", "Typing"], "one step for the run (02 lines 1066-1067)");
    // Undo empties the object in one step; redo restores it.
    ok(&h, "edit.undo", json!({}));
    assert_eq!(text_of(&h, &id), "");
    ok(&h, "edit.redo", json!({}));
    assert_eq!(text_of(&h, &id), "Hello");
    // The same as one insert of five characters (an agent's type_text), document and labels.
    let g = ready();
    let gid = new_text(&g, (-700.0, -400.0));
    assert_eq!(input(&g, Input::Text("Hello".into())).status, Status::Ok);
    assert_eq!(text_of(&g, &gid), "Hello");
    // (Revisions differ: five commits stamp five; the content is the same.)
    let k = ready();
    let kid = new_text(&k, (-700.0, -400.0));
    for c in ["H", "e", "l", "l", "o"] {
        input(&k, Input::Text(c.into()));
    }
    assert_eq!(content(&g, &gid), content(&k, &kid), "one insert of five characters equals five inserts");
    let gl = labels(&g);
    assert_eq!(&gl[gl.len() - 2..], ["New Text Object", "Typing"]);
}

#[test]
fn each_ending_of_the_run_gives_a_new_step() {
    // (name, what ends the run between two keystrokes)
    type Ending = fn(&ExecutorHandle);
    let endings: Vec<(&str, Ending)> = vec![
        ("caret move", |h| {
            input(h, Input::Key("Left".into()));
        }),
        ("highlight", |h| {
            input(h, Input::Key("Shift+Left".into()));
            input(h, Input::Key("Right".into()));
        }),
        ("undo then redo", |h| {
            ok(h, "edit.undo", json!({}));
            ok(h, "edit.redo", json!({}));
        }),
        ("an interleaved command", |h| {
            ok(h, "draw.pivot.set", json!({ "drawingId": D1, "point": { "x": 1, "y": 1 } }));
        }),
        ("select all", |h| {
            input(h, Input::Key("Primary+A".into()));
            input(h, Input::Key("End".into()));
        }),
    ];
    for (name, f) in endings {
        let h = ready();
        let _ = new_text(&h, (-700.0, -400.0));
        input(&h, Input::Text("ab".into()));
        let before = labels(&h).len();
        f(&h);
        let mid = labels(&h).len();
        input(&h, Input::Text("c".into()));
        assert_eq!(labels(&h).len(), mid + 1, "{name}: the keystroke after it is a new step (before {before})");
        assert_eq!(labels(&h).last().unwrap(), "Typing", "{name}");
    }
}

#[test]
fn commands_are_always_their_own_steps() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    let before = labels(&h).len();
    ok(&h, "draw.text.set", json!({ "drawingId": D1, "textId": id, "insert": { "at": 0, "text": "a" } }));
    ok(&h, "draw.text.set", json!({ "drawingId": D1, "textId": id, "insert": { "at": 1, "text": "b" } }));
    assert_eq!(labels(&h).len(), before + 2, "02 lines 1064-1065: run_command never extends a run");
}

#[test]
fn editing_keys_move_the_caret_and_edit_at_it() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("abc".into()));
    input(&h, Input::Key("Left".into()));
    input(&h, Input::Key("Backspace".into()));
    assert_eq!(text_of(&h, &id), "ac");
    assert_eq!(session(&h)["caret"], 1);
    input(&h, Input::Key("Delete".into()));
    assert_eq!(text_of(&h, &id), "a");
    input(&h, Input::Key("Enter".into()));
    input(&h, Input::Text("z".into()));
    assert_eq!(text_of(&h, &id), "a\nz");
    input(&h, Input::Key("Up".into()));
    assert_eq!(session(&h)["caret"], 1, "up from after `z`: after `a`");
    input(&h, Input::Key("Home".into()));
    assert_eq!(session(&h)["caret"], 0);
    input(&h, Input::Key("Shift+End".into()));
    assert_eq!((session(&h)["anchor"].as_u64(), session(&h)["caret"].as_u64()), (Some(0), Some(1)));
    // Typing over the highlight replaces it in one call.
    input(&h, Input::Text("Q".into()));
    assert_eq!(text_of(&h, &id), "Q\nz");
    // Tab inserts nothing.
    input(&h, Input::Key("Tab".into()));
    assert_eq!(text_of(&h, &id), "Q\nz");
}

#[test]
fn escape_on_an_empty_object_deletes_it_as_delete_text_object() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Key("Escape".into()));
    assert_eq!(session(&h), Value::Null);
    assert_eq!(labels(&h).last().unwrap(), "Delete Text Object", "02 line 681; SI-194");
    assert!(
        ok(&h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }))["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["id"] != id.as_str())
    );
    // A non-empty object stays, and an agent's deletion of a non-empty text keeps "Delete Object".
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("x".into()));
    input(&h, Input::Key("Escape".into()));
    assert_eq!(text_of(&h, &id), "x");
    ok(&h, "draw.object.delete", json!({ "drawingId": D1, "ids": [id] }));
    assert_eq!(labels(&h).last().unwrap(), "Delete Object");
}

#[test]
fn switching_tools_ends_the_session_and_removes_an_empty_object() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    ok(&h, "tool.active.set", json!({ "toolId": "select" }));
    assert_eq!(session(&h), Value::Null);
    assert_eq!(text_of(&h, &id), "");
    assert_eq!(labels(&h).last().unwrap(), "Delete Text Object");
}

#[test]
fn a_click_elsewhere_ends_the_session_and_acts_there() {
    let h = ready();
    let first = new_text(&h, (-700.0, -400.0));
    let env = pointer(&h, click(v(-600.0, -300.0)));
    let ids: Vec<String> = env.result.as_ref().unwrap()["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["commandId"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["tool.active.set", "draw.object.delete", "draw.text.add"], "the empty first object goes");
    assert_ne!(session(&h)["textId"].as_str(), Some(first.as_str()));
}

#[test]
fn pending_format_applies_to_the_next_keystroke_without_a_step() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("ab".into()));
    let before = labels(&h).len();
    ok(&h, "draw.textFormat.set", json!({ "drawingId": D1, "textId": id, "start": 2, "length": 0, "bold": true }));
    assert_eq!(labels(&h).len(), before, "SI-195: no step");
    input(&h, Input::Text("c".into()));
    let d = ok(&h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id.as_str()).cloned().unwrap();
    let runs = n["runs"].as_array().unwrap();
    assert_eq!(runs.last().unwrap()["bold"], true, "{runs:?}");
    assert_eq!(runs.last().unwrap()["length"], 1);
}

#[test]
fn a_pending_format_ends_the_typing_run() {
    // SI-191 (finding C44B-R1S-01): "A, bold, B" is two "Typing" steps; undo removes B only.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("A".into()));
    ok(&h, "draw.textFormat.set", json!({ "drawingId": D1, "textId": id, "start": 1, "length": 0, "bold": true }));
    input(&h, Input::Text("B".into()));
    assert_eq!(text_of(&h, &id), "AB");
    let l = labels(&h);
    assert_eq!(&l[l.len() - 3..], ["New Text Object", "Typing", "Typing"], "{l:?}");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(text_of(&h, &id), "A", "the undo removes B only");
}

#[test]
fn the_caret_follows_the_text_after_an_undo_and_after_another_origins_edit() {
    // Finding C44B-R1S-02: the session's indices are reconciled with the text after every change.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("ABC".into()));
    assert_eq!(session(&h)["caret"].as_u64(), Some(3));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(text_of(&h, &id), "");
    assert_eq!(session(&h)["caret"].as_u64(), Some(0), "clamped after the undo");
    let e = input(&h, Input::Text("X".into()));
    assert_eq!(e.status, Status::Ok, "{e:?}");
    assert_eq!(text_of(&h, &id), "X");
    // Another origin shortens the text under the caret.
    input(&h, Input::Text("YZ".into()));
    assert_eq!(session(&h)["caret"].as_u64(), Some(3));
    let agent = Origin::Agent {
        session: "s1".into(),
        name: "Test".into(),
        scopes: [tt_registry::Scope::Read, tt_registry::Scope::Edit].into_iter().collect(),
    };
    let env = h.invoke(tt_executor::Invocation {
        command: "draw.text.set".into(),
        params: json!({ "drawingId": D1, "textId": id, "delete": { "start": 1, "length": 2 } }),
        options: Default::default(),
        origin: agent,
    });
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert_eq!(text_of(&h, &id), "X");
    let s = session(&h);
    assert_eq!((s["caret"].as_u64(), s["anchor"].as_u64()), (Some(1), Some(1)), "{s}");
    let e = input(&h, Input::Text("W".into()));
    assert_eq!(e.status, Status::Ok, "{e:?}");
    assert_eq!(text_of(&h, &id), "XW");
}

#[test]
fn typed_text_takes_the_current_colour_in_an_existing_object() {
    // 02 §7.6, design note 5 §6 (finding C44B-R1S-03): text typed into an object whose run is another colour takes the
    // current colour.
    let h = ready();
    let d = h.snapshot().unwrap().unwrap();
    let cs: Vec<String> = d.palettes().flat_map(|p| p.swatches()).map(|s| s.colour_id().as_str().to_owned()).collect();
    let (red, blue) = (cs[0].clone(), cs[1].clone());
    ok(&h, "colour.current.set", json!({ "colourId": red }));
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("ab".into()));
    ok(&h, "colour.current.set", json!({ "colourId": blue }));
    input(&h, Input::Text("X".into()));
    assert_eq!(text_of(&h, &id), "abX");
    let d = ok(&h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id.as_str()).cloned().unwrap();
    let runs = n["runs"].as_array().unwrap();
    assert_eq!(runs.first().unwrap()["colourId"], json!(red), "{runs:?}");
    assert_eq!(runs.last().unwrap()["colourId"], json!(blue), "the typed X is blue: {runs:?}");
    assert_eq!(runs.last().unwrap()["length"], 1, "{runs:?}");
}

#[test]
fn in_the_canvas_text_editor_undo_is_the_documents() {
    // SI-193 (finding C44B-R1S-04): with the canvas's text editor focused, Primary+Z resolves to the document's undo;
    // in an ordinary text field it stays the field's.
    use tt_executor::host::{KeyResolution, ShellReport};
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("ABC".into()));
    h.report_shell(ShellReport {
        text_focus: Some(true),
        text_field_kind: Some("canvasText".into()),
        ..Default::default()
    });
    match h.resolve_key("Primary+Z").unwrap() {
        KeyResolution::Command { command_id, .. } => assert_eq!(command_id, "edit.undo"),
        other => panic!("Primary+Z in the canvas's text editor: {other:?}"),
    }
    ok(&h, "edit.undo", json!({}));
    assert_eq!(text_of(&h, &id), "", "the typing run is undone");
    h.report_shell(ShellReport { text_field_kind: Some("ordinary".into()), ..Default::default() });
    assert!(
        matches!(h.resolve_key("Primary+Z").unwrap(), KeyResolution::Suspended { ref reason, .. } if reason == "textField"),
        "an ordinary field keeps its own undo"
    );
}

#[test]
fn type_text_needs_a_session_and_another_origin_waits() {
    let h = ready();
    let e = input(&h, Input::Text("x".into()));
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("arg.missing"));
    let _ = new_text(&h, (-700.0, -400.0));
    let agent = Origin::Agent {
        session: "s1".into(),
        name: "Test".into(),
        scopes: [tt_registry::Scope::Read, tt_registry::Scope::Edit, tt_registry::Scope::Input].into_iter().collect(),
    };
    let e = h.text_input(agent, Input::Text("x".into()));
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("ui.busy"), "{e:?}");
}

#[test]
fn drags_set_the_width_and_move_the_box() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("Hello world".into()));
    let s = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    let te = s["textEdit"].clone();
    let right = (te["handles"][1][0].as_f64().unwrap(), te["handles"][1][1].as_f64().unwrap());
    let env = pointer(&h, drag(&[v(right.0, right.1), v(right.0 - 40.0, right.1)], &[]));
    let c = env.result.as_ref().unwrap()["commands"].as_array().unwrap().last().cloned().unwrap();
    assert_eq!(c["commandId"], "draw.text.set");
    assert!(c["params"]["width"].as_f64().is_some(), "{c}");
    assert_eq!(labels(&h).last().unwrap(), "Set Text Width");
    // The frame: the box's top edge.
    let s = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    let q = s["textEdit"]["quad"].clone();
    let top = ((q[0][0].as_f64().unwrap() + q[1][0].as_f64().unwrap()) / 2.0, q[0][1].as_f64().unwrap());
    let env = pointer(&h, drag(&[v(top.0, top.1), v(top.0 + 25.0, top.1 + 10.0)], &[]));
    let c = env.result.as_ref().unwrap()["commands"].as_array().unwrap().last().cloned().unwrap();
    assert_eq!(c["commandId"], "draw.text.set", "{env:?}");
    assert_eq!(c["params"]["position"], json!({ "x": -700.0 + 25.0, "y": -400.0 + 10.0 }));
    assert_eq!(labels(&h).last().unwrap(), "Move Text Object");
    assert_eq!(session(&h)["textId"].as_str(), Some(id.as_str()), "the session stays on its object");
}

#[test]
fn a_click_inside_a_box_places_the_caret_and_a_double_click_selects_the_word() {
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("one two".into()));
    let s = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    let q = s["textEdit"]["quad"].clone();
    let (x0, x1) = (q[0][0].as_f64().unwrap(), q[1][0].as_f64().unwrap());
    let (y0, y1) = (q[0][1].as_f64().unwrap(), q[3][1].as_f64().unwrap());
    let mid = (x0 + 0.85 * (x1 - x0), (y0 + y1) / 2.0);
    let env = pointer(&h, click(v(mid.0, mid.1)));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let caret = session(&h)["caret"].as_u64().unwrap();
    assert!(caret >= 5, "near the end of `two`: {caret}");
    let mut events = click(v(mid.0, mid.1));
    events.push(json!({ "type": "doubleClick", "x": v(mid.0, mid.1).0, "y": v(mid.0, mid.1).1 }));
    let _ = pointer(&h, events);
    let s = session(&h);
    assert_eq!(s["textId"].as_str(), Some(id.as_str()));
}

// ----- Round 1 (D-81): double click, a locked drawing, word keys ---------------------------------------------------------

/// The text object's box (drawing units): `(x, y, width, height)` with y up.
fn text_box(h: &ExecutorHandle, id: &str) -> (f64, f64, f64, f64) {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).cloned().unwrap();
    let b = &n["bounds"];
    (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap(), b["width"].as_f64().unwrap(), b["height"].as_f64().unwrap())
}

#[test]
fn a_double_click_selects_the_word_in_both_views_without_a_step() {
    // 02 §7.6 (finding C44B-R1-41): a double click inside a word selects it; an agent's pointer double click the same.
    for view in ["drawingView", "cameraView"] {
        let h = ready();
        let id = new_text(&h, (-700.0, -400.0));
        input(&h, Input::Text("hello world".into()));
        ok(&h, "tool.active.set", json!({ "toolId": "text" }));
        let (x, y, w, hh) = text_box(&h, &id);
        let p = (x + w * 0.85, y + hh / 2.0);
        let before = labels(&h).len();
        let events = vec![
            json!({ "type": "down", "x": p.0, "y": p.1 }),
            json!({ "type": "up", "x": p.0, "y": p.1 }),
            json!({ "type": "down", "x": p.0, "y": p.1 }),
            json!({ "type": "up", "x": p.0, "y": p.1 }),
            json!({ "type": "doubleClick", "x": p.0, "y": p.1 }),
        ];
        let env =
            h.pointer(Origin::User, json!({ "view": view, "space": "drawing", "tool": "text", "events": events }));
        assert_eq!(env.status, Status::Ok, "{view}: {env:?}");
        let s = session(&h);
        assert_eq!((s["anchor"].as_u64(), s["caret"].as_u64()), (Some(6), Some(11)), "{view}: the word: {s}");
        assert_eq!(labels(&h).len(), before, "{view}: no undo step");
    }
}

#[test]
fn no_session_opens_on_a_locked_drawing() {
    // Finding C44B-R1-42: after the drawing is locked a press inside its text is refused and no editor opens.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("locked".into()));
    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [D1], "locked": true }));
    assert!(session(&h).is_null(), "locking ends the session");
    let (x, y, w, hh) = text_box(&h, &id);
    let p = (x + w / 2.0, y + hh / 2.0);
    let env = h.pointer(
        Origin::User,
        json!({ "view": "drawingView", "space": "drawing", "tool": "text",
                "events": [{ "type": "down", "x": p.0, "y": p.1 }, { "type": "up", "x": p.0, "y": p.1 }] }),
    );
    assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("drawing.locked"), "{env:?}");
    assert!(session(&h).is_null(), "no session on a locked drawing");
}

#[test]
fn word_keys_move_extend_and_delete_by_words() {
    // 01 §8.4 (D-81 item 4b): Ctrl (Option on macOS) with the arrows, Backspace and Delete works by words.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("one two, three".into()));
    let caret = |h: &ExecutorHandle| session(h)["caret"].as_u64().unwrap();
    input(&h, Input::Key("Primary+Left".into()));
    assert_eq!(caret(&h), 9, "to the start of `three`");
    input(&h, Input::Key("Alt+Left".into()));
    assert_eq!(caret(&h), 7, "the comma is a word of its own");
    input(&h, Input::Key("Primary+Left".into()));
    assert_eq!(caret(&h), 4, "to the start of `two`");
    input(&h, Input::Key("Primary+Shift+Right".into()));
    let s = session(&h);
    assert_eq!((s["anchor"].as_u64(), s["caret"].as_u64()), (Some(4), Some(7)), "extended over `two`");
    input(&h, Input::Key("Right".into()));
    input(&h, Input::Key("Primary+Delete".into()));
    assert_eq!(text_of(&h, &id), "one two three", "the comma deleted to the word's end");
    input(&h, Input::Key("End".into()));
    input(&h, Input::Key("Alt+Backspace".into()));
    assert_eq!(text_of(&h, &id), "one two ", "the last word deleted");
}

#[test]
fn the_caret_moves_and_cuts_by_grapheme_clusters() {
    // Ruling D-82 item 2: the caret keys, Shift selection, Backspace and Delete work by extended grapheme clusters; the
    // commands stay code point indexed. "aq\u{301}b " then woman-ZWJ-laptop: code points a(0) q(1) U+0301(2) b(3)
    // space(4) U+1F469(5) ZWJ(6) U+1F4BB(7); clusters end at 1, 3, 4, 5 and 8.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("aq\u{301}b \u{1F469}\u{200D}\u{1F4BB}".into()));
    let caret = |h: &ExecutorHandle| session(h)["caret"].as_u64().unwrap();
    assert_eq!(caret(&h), 8);
    for want in [5, 4, 3, 1, 0] {
        input(&h, Input::Key("Left".into()));
        assert_eq!(caret(&h), want, "Left steps over one cluster");
    }
    for want in [1, 3] {
        input(&h, Input::Key("Right".into()));
        assert_eq!(caret(&h), want, "Right steps over one cluster");
    }
    input(&h, Input::Key("Shift+Left".into()));
    let s = session(&h);
    assert_eq!((s["anchor"].as_u64(), s["caret"].as_u64()), (Some(3), Some(1)), "Shift selects the whole cluster");
    input(&h, Input::Key("Right".into()));
    assert_eq!(caret(&h), 3, "Right collapses the highlight at its end");
    input(&h, Input::Key("Backspace".into()));
    assert_eq!(text_of(&h, &id), "ab \u{1F469}\u{200D}\u{1F4BB}", "Backspace removes q and its mark together");
    assert_eq!(caret(&h), 1);
    input(&h, Input::Key("End".into()));
    input(&h, Input::Key("Backspace".into()));
    assert_eq!(text_of(&h, &id), "ab ", "Backspace removes the joined emoji whole");
    input(&h, Input::Text("\u{1F1EF}\u{1F1F5}".into()));
    input(&h, Input::Key("Left".into()));
    assert_eq!(caret(&h), 3, "a flag is one step");
    input(&h, Input::Key("Delete".into()));
    assert_eq!(text_of(&h, &id), "ab ", "Delete removes the flag whole");

    // A base letter typed before a lone combining mark joins it: the caret goes after the cluster.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("\u{301}b".into()));
    input(&h, Input::Key("Home".into()));
    input(&h, Input::Text("q".into()));
    assert_eq!(text_of(&h, &id), "q\u{301}b");
    assert_eq!(session(&h)["caret"].as_u64(), Some(2), "not between q and its mark");

    // Another origin's edit that puts the caret inside a cluster: it moves to the cluster's end. An agent still
    // addresses any code point.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("qb".into()));
    input(&h, Input::Key("Left".into()));
    assert_eq!(session(&h)["caret"].as_u64(), Some(1));
    let agent = Origin::Agent {
        session: "s1".into(),
        name: "Test".into(),
        scopes: [tt_registry::Scope::Read, tt_registry::Scope::Edit].into_iter().collect(),
    };
    let env = h.invoke(tt_executor::Invocation {
        command: "draw.text.set".into(),
        params: json!({ "drawingId": D1, "textId": id, "insert": { "at": 1, "text": "\u{301}" } }),
        options: Default::default(),
        origin: agent,
    });
    assert_eq!(env.status, Status::Ok, "{env:?}");
    assert_eq!(text_of(&h, &id), "q\u{301}b");
    let e = input(&h, Input::Text("x".into()));
    assert_eq!(e.status, Status::Ok, "{e:?}");
    assert_eq!(text_of(&h, &id), "q\u{301}xb", "the next keystroke lands after the cluster, not inside it");
}

#[test]
fn the_scene_answer_carries_the_session_for_a_surface_that_mounts_later() {
    // Ruling D-85 item 2 (finding C44B-R2-02): the Text panel opened after the session began reads the session from
    // the scene answer (`textEdit.drawingId`, `textEdit.format`, the notice's values); a canvas reads the current colour.
    let h = ready();
    let id = new_text(&h, (-700.0, -400.0));
    input(&h, Input::Text("A".into()));
    let scene = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    let edit = &scene["textEdit"];
    assert_eq!(edit["drawingId"], json!(D1), "{edit}");
    assert_eq!(edit["textId"], json!(id));
    assert_eq!(edit["format"]["bold"], json!(false), "the values the panel shows at the caret: {edit}");
    assert!(scene["currentColour"].is_string(), "the current colour: {}", scene["currentColour"]);
    let camera = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    assert_eq!(camera["textEdit"]["drawingId"], json!(D1), "every view's answer carries it");
}
