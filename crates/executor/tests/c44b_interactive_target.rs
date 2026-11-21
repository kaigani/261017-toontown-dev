//! Ruling D-79 (findings C44B-R1-02 and C44B-R1-03): one rule in the gesture path for every drawing tool and view. A
//! gesture never produces an editing command for a drawing the person cannot edit interactively there:
//!
//! - a locked drawing (02 line 414): refused in both views, `drawing.locked` (Reposition All excepted: SI-189, its
//!   precheck refuses only when every drawing is locked);
//! - a locked element (05 §6.6, 04 §10.4; ruling D-80): refused in the scene views (the Camera view here),
//!   `element.locked`; "Locked elements can still be edited in the Drawing View" (04 line 572);
//! - an element hidden in the scene (02 §1.2, 04 §10.1): refused in the Camera view, `element.hidden`, and the
//!   drawing tools' overlays are not in its scene answer; the Drawing view edits it.
//!
//! The table runs every tool that edits, in both views, unlocked (the gesture commits an edit), then under each
//! condition (the refusal's code, nothing committed). An agent addressing a locked element by id still edits it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin, Status};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";

/// Bob current at frame 1, a rectangle on its own layer (300..400 by 300..360 du), selected.
fn ready() -> (ExecutorHandle, String) {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    let r = ok(
        &h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": { "x": 300, "y": 300 }, "to": { "x": 400, "y": 360 }, "layer": "new" }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
    (h, sid)
}

fn drag(a: (f64, f64), b: (f64, f64)) -> Vec<Value> {
    vec![
        json!({ "type": "down", "x": a.0, "y": a.1, "modifiers": [] }),
        json!({ "type": "move", "x": (a.0 + b.0) / 2.0, "y": (a.1 + b.1) / 2.0, "modifiers": [] }),
        json!({ "type": "up", "x": b.0, "y": b.1, "modifiers": [] }),
    ]
}

fn click(a: (f64, f64)) -> Vec<Value> {
    vec![
        json!({ "type": "down", "x": a.0, "y": a.1, "modifiers": [] }),
        json!({ "type": "up", "x": a.0, "y": a.1, "modifiers": [] }),
    ]
}

/// Each tool that edits and a gesture that edits the fixture, in drawing units.
fn gestures(h: &ExecutorHandle) -> Vec<(&'static str, Vec<Value>)> {
    let b = ok(h, "draw.selection.get", json!({ "drawingId": D1 }))["bounds"].clone();
    let (bx, by) = (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap());
    let (bw, bh) = (b["width"].as_f64().unwrap(), b["height"].as_f64().unwrap());
    vec![
        ("brush", drag((0.0, 0.0), (60.0, 40.0))),
        ("pencil", drag((0.0, 0.0), (60.0, 40.0))),
        ("rectangle", drag((0.0, 0.0), (60.0, 40.0))),
        ("ellipse", drag((0.0, 0.0), (60.0, 40.0))),
        ("line", drag((0.0, 0.0), (60.0, 40.0))),
        ("eraser", drag((290.0, 330.0), (310.0, 330.0))),
        ("paint", click((350.0, 330.0))),
        ("select", drag((350.0, 330.0), (370.0, 340.0))),
        ("contourEditor", drag((300.0, 300.0), (290.0, 290.0))),
        ("perspective", drag((bx + bw, by + bh), (bx + bw - 20.0, by + bh))),
        ("repositionAll", drag((350.0, 330.0), (370.0, 340.0))),
        ("text", click((-700.0, -400.0))),
    ]
}

fn position(h: &ExecutorHandle) -> u64 {
    ok(h, "edit.history.get", json!({}))["position"].as_u64().unwrap()
}

fn gesture(h: &ExecutorHandle, view: &str, tool: &str, events: &[Value]) -> tt_executor::Envelope {
    h.pointer(Origin::User, json!({ "view": view, "space": "drawing", "tool": tool, "events": events }))
}

/// The condition a case applies before the gesture.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Condition {
    Unlocked,
    DrawingLocked,
    ElementLocked,
    ElementHidden,
}

fn apply(h: &ExecutorHandle, c: Condition, tool: &str) {
    match c {
        Condition::Unlocked => {}
        Condition::DrawingLocked => {
            // Reposition All moves every unlocked drawing of the element: its refusal needs every one locked.
            let ids: Vec<String> = if tool == "repositionAll" {
                ok(h, "element.drawing.list", json!({ "elementId": BOB }))["drawings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["id"].as_str().unwrap().to_owned())
                    .collect()
            } else {
                vec![D1.to_owned()]
            };
            ok(h, "element.drawingLock.set", json!({ "drawingIds": ids, "locked": true }));
        }
        Condition::ElementLocked => {
            ok(h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true }));
        }
        Condition::ElementHidden => {
            ok(h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
        }
    }
}

/// The refusal a condition gives in a view, or `None` when the gesture edits.
fn expected(c: Condition, view: &str, tool: &str) -> Option<&'static str> {
    match (c, view) {
        (Condition::Unlocked, _) => None,
        (Condition::DrawingLocked, _) if tool == "repositionAll" => Some("any"),
        (Condition::DrawingLocked, _) => Some("drawing.locked"),
        (Condition::ElementLocked, "cameraView") => Some("element.locked"),
        (Condition::ElementHidden, "cameraView") => Some("element.hidden"),
        _ => None,
    }
}

#[test]
fn no_gesture_edits_a_locked_or_hidden_target_in_any_view() {
    let mut rows = Vec::new();
    for view in ["drawingView", "cameraView"] {
        for c in [Condition::Unlocked, Condition::DrawingLocked, Condition::ElementLocked, Condition::ElementHidden] {
            let tools: Vec<&str> = {
                let (h, _) = ready();
                gestures(&h).into_iter().map(|(t, _)| t).collect()
            };
            for tool in tools {
                // In the Camera view "select" is the scene Select (C4.3's scene tools, their own picking rules): the
                // drawing tools' rule does not apply to it; its picking of a locked element is checked below.
                if view == "cameraView" && tool == "select" {
                    continue;
                }
                let (h, _) = ready();
                let events = gestures(&h).into_iter().find(|(t, _)| *t == tool).unwrap().1;
                apply(&h, c, tool);
                let before = position(&h);
                let env = gesture(&h, view, tool, &events);
                let after = position(&h);
                let code = env.error.as_ref().map(|e| e.code.clone());
                let row = format!("{view} {c:?} {tool}: {:?} {code:?}", env.status);
                match expected(c, view, tool) {
                    None => {
                        assert_eq!(env.status, Status::Ok, "{row}: {env:?}");
                        assert!(after > before, "{row}: the gesture commits an edit");
                    }
                    Some(code) => {
                        assert_eq!(env.status, Status::Error, "{row}: {env:?}");
                        if code != "any" {
                            assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some(code), "{row}");
                        }
                        assert_eq!(after, before, "{row}: nothing committed");
                    }
                }
                rows.push(row);
            }
        }
    }
    assert_eq!(rows.len(), 4 * 12 + 4 * 11, "{rows:#?}");
}

#[test]
fn a_hidden_element_shows_no_drawing_tool_overlay_in_the_camera_view() {
    let (h, _) = ready();
    ok(&h, "tool.active.set", json!({ "toolId": "contourEditor" }));
    let shown = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    assert!(shown["contours"].is_object(), "the Contour Editor's overlay is drawn while the element is visible");
    ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
    let hidden = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    assert!(hidden["contours"].is_null(), "no overlay, no hit targets for a hidden element: {}", hidden["contours"]);
    for tool in ["perspective", "repositionAll", "text"] {
        ok(&h, "tool.active.set", json!({ "toolId": tool }));
        let s = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
        assert!(s["toolBox"].is_null() && s["textBoxes"].is_null(), "{tool}: {s}");
    }
    // The Drawing view shows the current drawing whatever the scene's visibility (02 §1.2).
    ok(&h, "tool.active.set", json!({ "toolId": "contourEditor" }));
    let drawing = serde_json::to_value(h.canvas_scene("drawingView").unwrap()).unwrap();
    assert!(drawing["contours"].is_object());
}

#[test]
fn a_locked_or_hidden_element_cannot_be_picked_in_the_camera_view_and_can_in_the_drawing_view() {
    // 05 §6.6 ("cannot be picked"), ruling D-80: a selection click is refused in the Camera view and nothing is sent;
    // the Drawing view picks a locked element's content (04 line 572). A hidden element offers no hit target (D-79).
    for (condition, code) in [("locked", "element.locked"), ("hidden", "element.hidden")] {
        for (tool, events) in [("select", click((300.0, 330.0))), ("contourEditor", click((300.0, 300.0)))] {
            // The scene Select (the Camera view's "select") answers with an empty pick; the drawing tools are refused.
            let (h, _) = ready();
            ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [], "mode": "replace" }));
            if condition == "locked" {
                ok(&h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true }));
            } else {
                ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
            }
            let env = gesture(&h, "cameraView", tool, &events);
            if tool == "select" {
                let picked = env.result.as_ref().and_then(|r| r["commands"].as_array().cloned()).unwrap_or_default();
                let ids: Vec<Value> = picked
                    .iter()
                    .filter(|c| c["commandId"] == "edit.selection.set")
                    .flat_map(|c| c["params"]["ids"].as_array().cloned().unwrap_or_default())
                    .collect();
                assert!(ids.is_empty(), "{condition}: the scene Select picks nothing: {env:?}");
            } else {
                assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some(code), "{condition} {tool}: {env:?}");
            }
            let sel = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
            assert_eq!(sel["selectedIds"], json!([]), "{condition} {tool}: nothing picked");
            let env = gesture(&h, "drawingView", tool, &events);
            assert_eq!(env.status, Status::Ok, "{condition} {tool} in the Drawing view: {env:?}");
        }
    }
}

#[test]
fn an_agent_addressing_a_locked_element_by_id_still_edits_it() {
    // 05 §6.6: "Commands addressing it by id still act on it (locking is an interaction aid)."
    let (h, sid) = ready();
    ok(&h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true }));
    let before = position(&h);
    ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{ "strokeId": sid, "anchor": 0 }], "delta": { "x": -5.0, "y": 0.0 } }),
    );
    assert!(position(&h) > before);
}

#[test]
fn the_camera_view_edits_a_visible_element_with_no_ink() {
    // Round 1 live probes l3b 7 and l1b 8: visibility is the element's, not its ink's. A new element's empty drawing
    // takes a text box in the Camera view, and a drawing of invisible strokes only takes the Contour Editor's click.
    let h = fixture();
    let eid = ok(&h, "element.create", json!({ "type": "drawing", "name": "Text" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "element.drawing.create", json!({ "elementId": eid, "name": "1", "exposeFrom": 1, "exposeTo": 24 }));
    ok(&h, "app.context.set", json!({ "elementId": eid, "frame": 1 }));
    ok(&h, "tool.active.set", json!({ "toolId": "text" }));
    let env = gesture(&h, "cameraView", "text", &click((-100.0, 80.0)));
    assert_eq!(env.status, Status::Ok, "a text box in an empty drawing: {env:?}");
    let scene = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    assert!(scene["textEdit"].is_object(), "the session opened: {}", scene["textEdit"]);

    // The same element with one stroke made invisible.
    ok(&h, "tool.active.set", json!({ "toolId": "contourEditor" }));
    let points = json!([{ "x": -80, "y": 0 }, { "x": 0, "y": 40 }, { "x": 80, "y": 0 }]);
    let added = ok(&h, "draw.polyline.add", json!({ "points": points, "closed": false, "snap": false }));
    let sid = added["strokeIds"][0].as_str().unwrap().to_owned();
    let did = ok(&h, "draw.drawing.get", json!({}))["drawingId"].as_str().unwrap().to_owned();
    ok(&h, "draw.stroke.set", json!({ "strokeId": format!("{did}/{sid}"), "kind": "invisible" }));
    ok(&h, "draw.selection.set", json!({ "by": "ids", "ids": [sid], "mode": "replace" }));
    let env = gesture(&h, "cameraView", "contourEditor", &click((120.0, 100.0)));
    assert_eq!(env.status, Status::Ok, "a click on nothing deselects: {env:?}");
    let sel = ok(&h, "draw.selection.get", json!({}));
    assert_eq!(sel["selectedIds"], json!([]));
    let scene = serde_json::to_value(h.canvas_scene("cameraView").unwrap()).unwrap();
    assert!(scene["contours"].is_object(), "the overlay is drawn for an element of invisible strokes");
}

/// The person's call from the window (ruling D-83 item 3): what the desktop host sends for a Tool options control, a
/// menu or a context-menu item.
fn person(h: &ExecutorHandle, command: &str, params: Value) -> tt_executor::Envelope {
    h.invoke(tt_executor::Invocation {
        command: command.into(),
        params,
        options: tt_executor::InvokeOptions { person: true, ..Default::default() },
        origin: Origin::User,
    })
}

#[test]
fn a_persons_key_panel_or_menu_edit_follows_the_rule_and_an_agents_direct_call_does_not() {
    // Finding C44B-R2S-03: the rule is the person's, whatever the input. Each case: the element locked or hidden by an
    // agent while an anchor and a line are selected in the Camera view; the person's Delete key (the Contour Editor's
    // `draw.point.delete`), the Tool options line thickness (`draw.stroke.set`) and the context menu's Delete
    // (`edit.delete` in the drawing context) are refused with the condition's code and change nothing; in the Drawing
    // view the element condition does not hold (D-80); an agent's direct call still edits.
    for (condition, code) in [("locked", "element.locked"), ("hidden", "element.hidden")] {
        let (h, _) = ready();
        let line = json!([{ "x": -80, "y": 0 }, { "x": 0, "y": 40 }, { "x": 80, "y": 0 }]);
        let params = json!({ "drawingId": D1, "points": line, "layer": "new", "snap": false });
        let added = ok(&h, "draw.polyline.add", params);
        let sid = added["strokeIds"][0].as_str().unwrap().to_owned();
        ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
        ok(&h, "tool.active.set", json!({ "toolId": "contourEditor" }));
        ok(&h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{ "strokeId": sid, "anchor": 1 }] }));
        ok(&h, "view.active.set", json!({ "viewId": "cameraView" }));
        if condition == "locked" {
            ok(&h, "element.lock.set", json!({ "elementIds": [BOB], "locked": true }));
        } else {
            ok(&h, "element.visibility.set", json!({ "elementIds": [BOB], "scope": "scene", "visible": false }));
        }
        let before = position(&h);
        let content = ok(&h, "draw.drawing.get", json!({ "drawingId": D1 }));
        let key = h.resolve_key("Delete");
        let refused = key.as_ref().err().map(|e| (e.code.clone(), e.details["refusedCommandId"].clone()));
        assert_eq!(refused, Some((code.to_owned(), json!("draw.point.delete"))), "{condition}: Delete: {key:?}");
        let stroke = format!("{D1}/{sid}");
        for (command, params) in [
            ("draw.stroke.set", json!({ "strokeId": stroke, "thickness": 5 })),
            ("draw.point.delete", json!({})),
            ("edit.delete", json!({ "context": "drawing" })),
        ] {
            let env = person(&h, command, params);
            assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some(code), "{condition} {command}: {env:?}");
            assert_eq!(env.error.as_ref().map(|e| e.details["refusedCommandId"].clone()), Some(json!(command)));
        }
        assert_eq!(position(&h), before, "{condition}: nothing committed");
        assert_eq!(ok(&h, "draw.drawing.get", json!({ "drawingId": D1 })), content, "{condition}: nothing changed");
        // The Drawing view: the element condition does not hold there.
        ok(&h, "view.active.set", json!({ "viewId": "drawingView" }));
        let env = person(&h, "draw.stroke.set", json!({ "strokeId": stroke, "thickness": 5 }));
        assert_eq!(env.status, Status::Ok, "{condition}: the person edits in the Drawing view: {env:?}");
        // An agent's direct call in the Camera view still edits (05 §6.6: commands addressing it by id act on it).
        ok(&h, "view.active.set", json!({ "viewId": "cameraView" }));
        let env = run(&h, "draw.stroke.set", json!({ "strokeId": stroke, "thickness": 6 }));
        assert_eq!(env.status, Status::Ok, "{condition}: a direct call: {env:?}");
    }
    // A drawing lock refuses the person's edit in either view, and the person can still unlock it.
    let (h, sid) = ready();
    ok(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": true }));
    for view in ["drawingView", "cameraView"] {
        ok(&h, "view.active.set", json!({ "viewId": view }));
        let env = person(&h, "edit.delete", json!({ "context": "drawing" }));
        assert_eq!(env.error.as_ref().map(|e| e.code.as_str()), Some("drawing.locked"), "{view}: {env:?}");
    }
    let env = person(&h, "draw.lock.set", json!({ "drawingId": D1, "locked": false }));
    assert_eq!(env.status, Status::Ok, "the lock itself is not refused: {env:?}");
    let pick = json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" });
    let env = person(&h, "draw.selection.set", pick);
    assert_eq!(env.status, Status::Ok, "{env:?}");
}
