//! M4 C4.4b, the Contour Editor's gestures (design note 3 §2 and §4; SI-184, SI-185): every row of the gesture table
//! through the headless `pointer` gives the command an agent sends for the same edit, value for value, and the same
//! canonical bytes and undo steps as that command sent directly (02 §13.3, ruling D-4, D-32).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin, Status};

const D1: &str = "dr_01K742SG0KKMCAP9X1E6Z5P4SW";
const BOB: &str = "scene:#1/element:\"Bob\"";
const TOOL: &str = "contourEditor";

fn ready() -> ExecutorHandle {
    let h = fixture();
    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 1 }));
    h
}

/// The headless Drawing view's default mapping (SI-063, SI-069): drawing unit -> view pixel.
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

fn click(p: (f64, f64), mods: &[&str]) -> Vec<Value> {
    drag(&[p, p], mods)
}

fn canonical(h: &ExecutorHandle) -> Vec<(String, String)> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .into_iter()
        .map(|k| {
            let v = doc.entry_raw_json(&k).unwrap();
            (format!("{k:?}"), tt_model::canonical::to_canonical_string(&v).unwrap())
        })
        .collect()
}

fn history(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "edit.history.get", json!({}))["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| json!({ "label": e["label"], "rev": e["rev"] }))
        .collect()
}

fn selection(h: &ExecutorHandle) -> Value {
    let s = ok(h, "draw.selection.get", json!({ "drawingId": D1 }));
    json!({ "ids": s["selectedIds"], "points": s["selectedPoints"] })
}

/// A 100 x 100 square stroke at drawing (300, 300)-(400, 400) on a new layer, selected: its id and anchor positions.
fn square(h: &ExecutorHandle) -> (String, Vec<(f64, f64)>) {
    let r = ok(
        h,
        "draw.rectangle.add",
        json!({ "drawingId": D1, "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 400}, "layer": "new" }),
    );
    let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
    ok(h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
    (sid.clone(), anchors(h, &sid))
}

fn anchors(h: &ExecutorHandle, sid: &str) -> Vec<(f64, f64)> {
    let d = ok(h, "draw.drawing.get", json!({ "drawingId": D1, "flatten": true }));
    let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == sid).cloned().unwrap();
    let b = n["bezier"].as_array().unwrap();
    (0..b.len().div_ceil(3)).map(|k| (b[k * 3]["x"].as_f64().unwrap(), b[k * 3]["y"].as_f64().unwrap())).collect()
}

/// The gesture on one executor and the expected commands sent directly on another, from the same state: the same
/// commands and parameters, the same canonical bytes, the same history and selection. Returns the gesture's envelopes.
fn equivalent(setup: &dyn Fn(&ExecutorHandle), events: Vec<Value>, expected: &[(&str, Value)]) -> Vec<Value> {
    let a = ready();
    setup(&a);
    let env = a.pointer(Origin::User, json!({ "view": "drawing", "tool": TOOL, "events": events }));
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    assert_eq!(commands[0]["commandId"], "tool.active.set");
    let got: Vec<(String, Value)> =
        commands[1..].iter().map(|c| (c["commandId"].as_str().unwrap().to_owned(), c["params"].clone())).collect();
    let want: Vec<(String, Value)> = expected.iter().map(|(i, p)| ((*i).to_owned(), p.clone())).collect();
    assert_eq!(got, want, "the agent's commands, value for value (D-4)");
    let b = ready();
    setup(&b);
    ok(&b, "tool.active.set", json!({ "toolId": TOOL }));
    for (id, p) in expected {
        ok(&b, id, p.clone());
    }
    assert_eq!(canonical(&a), canonical(&b), "the same canonical saved bytes and ids");
    assert_eq!(history(&a), history(&b), "the same undo steps and labels");
    assert_eq!(selection(&a), selection(&b), "the same selection");
    commands[1..].iter().map(|c| c["envelope"].clone()).collect()
}

fn with_square(f: impl Fn(&ExecutorHandle, &str) + 'static) -> impl Fn(&ExecutorHandle) {
    move |h| {
        let (sid, _) = square(h);
        f(h, &sid);
    }
}

fn sid_of(setup: &dyn Fn(&ExecutorHandle)) -> (String, Vec<(f64, f64)>) {
    let h = ready();
    setup(&h);
    let s = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    let sid = s["selectedIds"][0]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| s["selectedPoints"][0]["strokeId"].as_str().unwrap().to_owned());
    let a = anchors(&h, &sid);
    (sid, a)
}

#[test]
fn clicking_an_anchor_selects_it_and_constrain_adds_it() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    let envs = equivalent(
        &setup,
        click(v(a[1].0 + 2.0, a[1].1 - 2.0), &[]),
        &[(
            "draw.pointSelection.set",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}], "mode": "replace" }),
        )],
    );
    assert_eq!(envs[0]["result"]["selectedPoints"], json!([{"strokeId": sid, "anchor": 1}]));
    let pre = with_square(|h, sid| {
        ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
    });
    equivalent(
        &pre,
        click(v(a[2].0, a[2].1), &["shift"]),
        &[(
            "draw.pointSelection.set",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 2}], "mode": "add" }),
        )],
    );
}

#[test]
fn dragging_an_unselected_anchor_moves_only_it_and_a_selected_one_moves_the_selection() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    let (x, y) = v(a[0].0, a[0].1);
    equivalent(
        &setup,
        drag(&[(x, y), (x + 5.0, y), (x + 10.0, y - 4.0)], &[]),
        &[(
            "draw.point.move",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}], "delta": {"x": 10.0, "y": 4.0}, "snap": false }),
        )],
    );
    let pre = with_square(|h, sid| {
        ok(
            h,
            "draw.pointSelection.set",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}, {"strokeId": sid, "anchor": 2}] }),
        );
    });
    equivalent(
        &pre,
        drag(&[(x, y), (x + 10.0, y - 4.0)], &[]),
        &[(
            "draw.point.move",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}, {"strokeId": sid, "anchor": 2}],
                    "delta": {"x": 10.0, "y": 4.0}, "snap": false }),
        )],
    );
}

#[test]
fn snap_to_contour_is_made_explicit() {
    let setup = with_square(|h, _| {
        ok(h, "tool.snapToContour.set", json!({ "enabled": true }));
    });
    let (sid, a) = sid_of(&setup);
    let (x, y) = v(a[0].0, a[0].1);
    equivalent(
        &setup,
        drag(&[(x, y), (x + 10.0, y)], &[]),
        &[(
            "draw.point.move",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}], "delta": {"x": 10.0, "y": 0.0}, "snap": true }),
        )],
    );
}

#[test]
fn dragging_a_segment_bends_it_and_alternate_moves_the_selection() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    let mid = v((a[0].0 + a[1].0) / 2.0, (a[0].1 + a[1].1) / 2.0);
    let envs = equivalent(
        &setup,
        drag(&[mid, (mid.0, mid.1 - 20.0)], &[]),
        &[(
            "draw.segment.move",
            json!({ "drawingId": D1, "strokeId": sid, "segment": 0, "t": 0.5, "delta": {"x": 0.0, "y": 20.0} }),
        )],
    );
    assert_eq!(envs[0]["status"], "ok");
    let pre = with_square(|h, sid| {
        ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 3}] }));
    });
    equivalent(
        &pre,
        drag(&[mid, (mid.0 + 6.0, mid.1)], &["alt"]),
        &[(
            "draw.point.move",
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 3}], "delta": {"x": 6.0, "y": 0.0}, "snap": false }),
        )],
    );
}

#[test]
fn temporary_select_click_on_a_segment_inserts_an_anchor_and_on_nothing_does_nothing() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    let mid = v((a[1].0 + a[2].0) / 2.0, (a[1].1 + a[2].1) / 2.0);
    let envs = equivalent(
        &setup,
        click(mid, &["ctrl"]),
        &[("draw.point.add", json!({ "drawingId": D1, "strokeId": sid, "segment": 1, "t": 0.5 }))],
    );
    assert_eq!(envs[0]["result"]["anchor"], 2);
    // Far from every stroke: no command after `tool.active.set`.
    equivalent(&setup, click(v(-900.0, -500.0), &["ctrl"]), &[]);
}

#[test]
fn alternate_click_on_an_anchor_resets_its_handles() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    equivalent(
        &setup,
        click(v(a[2].0, a[2].1), &["alt"]),
        &[("draw.point.set", json!({ "drawingId": D1, "strokeId": sid, "anchor": 2, "handleMode": "reset" }))],
    );
}

#[test]
fn alternate_anchor_drag_pulls_both_handles_broken() {
    let setup = with_square(|_, _| {});
    let (sid, a) = sid_of(&setup);
    let (x, y) = v(a[1].0, a[1].1);
    equivalent(
        &setup,
        drag(&[(x, y), (x + 8.0, y - 6.0)], &["alt"]),
        &[(
            "draw.point.set",
            json!({ "drawingId": D1, "strokeId": sid, "anchor": 1, "inHandle": {"x": 8.0, "y": 6.0},
                    "outHandle": {"x": 8.0, "y": 6.0}, "handleMode": "broken" }),
        )],
    );
}

#[test]
fn dragging_a_handle_of_a_selected_anchor_is_symmetric_and_alternate_breaks_it() {
    // Give anchor 1 handles first (an alternate drag), then select it.
    let setup = with_square(|h, sid| {
        ok(
            h,
            "draw.point.set",
            json!({ "drawingId": D1, "strokeId": sid, "anchor": 1, "inHandle": {"x": 0.0, "y": -20.0},
                    "outHandle": {"x": 0.0, "y": 20.0}, "handleMode": "broken" }),
        );
        ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}] }));
    });
    let (sid, a) = sid_of(&setup);
    let (x, y) = v(a[1].0, a[1].1 + 20.0);
    equivalent(
        &setup,
        drag(&[(x, y), (x + 5.0, y)], &[]),
        &[(
            "draw.point.set",
            json!({ "drawingId": D1, "strokeId": sid, "anchor": 1, "handleMode": "symmetric", "outHandle": {"x": 5.0, "y": 20.0} }),
        )],
    );
    let (x, y) = v(a[1].0, a[1].1 - 20.0);
    equivalent(
        &setup,
        drag(&[(x, y), (x - 5.0, y)], &["alt"]),
        &[(
            "draw.point.set",
            json!({ "drawingId": D1, "strokeId": sid, "anchor": 1, "handleMode": "broken", "inHandle": {"x": -5.0, "y": -20.0} }),
        )],
    );
}

#[test]
fn dragging_on_nothing_selects_anchors_in_the_rectangle() {
    let setup = with_square(|_, _| {});
    let (sid, _) = sid_of(&setup);
    let envs = equivalent(
        &setup,
        drag(&[v(290.0, 410.0), v(350.0, 350.0)], &[]),
        &[(
            "draw.pointSelection.set",
            json!({ "drawingId": D1, "rect": {"x": 290.0, "y": 350.0, "width": 60.0, "height": 60.0}, "mode": "replace" }),
        )],
    );
    // Only the corner at (300, 400) is inside.
    let pts = envs[0]["result"]["selectedPoints"].as_array().unwrap();
    assert_eq!(pts.len(), 1, "{pts:?}");
    assert_eq!(pts[0]["strokeId"], sid.as_str());
}

#[test]
fn clicking_nothing_clears_both_selections_and_constrain_keeps_the_points() {
    let setup = with_square(|h, sid| {
        ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
    });
    let far = (40.0, 40.0);
    let p = json!({ "x": far.0 - 960.0, "y": 540.0 - far.1 });
    equivalent(
        &setup,
        click(far, &[]),
        &[
            ("draw.selection.set", json!({ "by": "point", "point": p, "mode": "replace" })),
            ("draw.pointSelection.set", json!({ "drawingId": D1, "points": [], "mode": "replace" })),
        ],
    );
    let h = ready();
    setup(&h);
    let before = selection(&h)["points"].clone();
    equivalent(
        &setup,
        click(far, &["shift"]),
        &[("draw.selection.set", json!({ "by": "point", "point": p, "mode": "add" }))],
    );
    let env = h.pointer(Origin::User, json!({ "view": "drawing", "tool": TOOL, "events": click(far, &["shift"]) }));
    assert_eq!(env.status, Status::Ok);
    assert_eq!(selection(&h)["points"], before, "constrain keeps the point selection");
}

#[test]
fn a_stroke_whose_anchors_are_not_shown_is_not_a_drag_target() {
    // The square is deselected: a drag at its corner is a rectangle selection, not a move.
    let setup = with_square(|h, _| {
        ok(h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [], "mode": "replace" }));
    });
    let h = ready();
    setup(&h);
    let s = ok(&h, "draw.selection.get", json!({ "drawingId": D1 }));
    assert_eq!(s["selectedIds"], json!([]));
    let (x, y) = v(300.0, 300.0);
    let env = h.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": TOOL, "events": drag(&[(x, y), (x + 10.0, y)], &[]) }),
    );
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    assert_eq!(commands[1]["commandId"], "draw.pointSelection.set", "{commands:#?}");
}

#[test]
fn hit_radii_are_view_pixels_at_any_zoom() {
    // At 200 % an anchor 4 du away is 8 view px away: outside the 6 px radius, so the press is on the segment.
    let setup = with_square(|h, _| {
        ok(h, "view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2.0 }));
    });
    let h = ready();
    setup(&h);
    let (sid, a) = sid_of(&setup);
    let to_view = |x: f64, y: f64| {
        let r = ok(
            &h,
            "view.coordinates.convert",
            json!({ "viewId": "drawingView", "point": {"x": x, "y": y}, "from": "drawing", "to": "view" }),
        );
        (r["point"]["x"].as_f64().unwrap(), r["point"]["y"].as_f64().unwrap())
    };
    let (dx, dy) = (a[1].0 - a[0].0, a[1].1 - a[0].1);
    let len = dx.hypot(dy);
    let p = to_view(a[0].0 + 4.0 * dx / len, a[0].1 + 4.0 * dy / len);
    // At 100 % the same press is 4 view px from the anchor: on the anchor.
    let q = ready();
    with_square(|_, _| {})(&q);
    let near = v(a[0].0 + 4.0 * dx / len, a[0].1 + 4.0 * dy / len);
    let env = q.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": TOOL, "events": drag(&[near, (near.0, near.1 + 10.0)], &[]) }),
    );
    assert_eq!(env.result.as_ref().unwrap()["commands"][1]["commandId"], "draw.point.move", "{env:?}");
    let env = h.pointer(
        Origin::User,
        json!({ "view": "drawing", "tool": TOOL, "events": drag(&[p, (p.0, p.1 + 10.0)], &[]) }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    assert_eq!(commands[1]["commandId"], "draw.segment.move", "{commands:#?}");
    assert_eq!(commands[1]["params"]["strokeId"], sid.as_str());
}

fn resolve(h: &ExecutorHandle, keys: &str) -> (String, Value) {
    match h.resolve_key(keys).unwrap() {
        tt_executor::host::KeyResolution::Command { command_id, params, .. } => (command_id, params),
        other => panic!("{keys}: {other:?}"),
    }
}

#[test]
fn delete_deletes_the_selected_anchors_with_the_contour_editor_and_the_selection_otherwise() {
    let h = ready();
    ok(&h, "app.mode.set", json!({ "mode": "drawing" }));
    let (sid, _) = square(&h);
    ok(&h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}] }));
    ok(&h, "tool.active.set", json!({ "toolId": TOOL }));
    for keys in ["Delete", "Backspace"] {
        assert_eq!(resolve(&h, keys).0, "draw.point.delete", "{keys} with the Contour Editor (SI-184)");
    }
    let before = history(&h).len();
    let (id, params) = resolve(&h, "Delete");
    let r = ok(&h, &id, params);
    assert_eq!(r["modifiedIds"], json!([sid]), "{r}");
    let hist = history(&h);
    assert_eq!(hist.len(), before + 1);
    assert_eq!(hist.last().unwrap()["label"], "Contour Editing");
    assert_eq!(anchors(&h, &sid).len(), 3);
    // Nothing selected: the key answers the command's own refusal (consumed silently, C41-R2-09).
    let e = h.resolve_key("Delete").unwrap_err();
    assert_eq!(e.code, "command.disabled", "{e:?}");
    assert_eq!(e.details["commandId"], "draw.point.delete");
    // With Select the binding is unchanged.
    ok(&h, "tool.active.set", json!({ "toolId": "select" }));
    ok(&h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
    // (`edit.delete` reads its own context, so it may answer its refusal here: the command it names is what counts.)
    let named = match h.resolve_key("Delete") {
        Ok(tt_executor::host::KeyResolution::Command { command_id, .. }) => command_id,
        Err(e) => e.details["commandId"].as_str().unwrap().to_owned(),
        Ok(other) => panic!("{other:?}"),
    };
    assert_eq!(named, "edit.delete");
    // In the staging mode (the Camera view) the Contour Editor's Delete is the same (SI-184: while it is active).
    ok(&h, "app.mode.set", json!({ "mode": "staging" }));
    ok(&h, "tool.active.set", json!({ "toolId": TOOL }));
    ok(&h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
    assert_eq!(resolve(&h, "Delete").0, "draw.point.delete");
}

#[test]
fn a_locked_drawing_refuses_every_edit_row_and_keeps_document_and_history() {
    let setup = with_square(|h, sid| {
        ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}] }));
        ok(h, "element.drawingLock.set", json!({ "drawingIds": [D1], "locked": true }));
    });
    let (_, a) = sid_of(&setup);
    let (x, y) = v(a[0].0, a[0].1);
    let mid = v((a[0].0 + a[1].0) / 2.0, (a[0].1 + a[1].1) / 2.0);
    let rows: Vec<(&str, Vec<Value>)> = vec![
        ("anchor drag", drag(&[(x, y), (x + 10.0, y)], &[])),
        ("segment drag", drag(&[mid, (mid.0, mid.1 + 10.0)], &[])),
        ("insert", click(mid, &["ctrl"])),
        ("reset", click((x, y), &["alt"])),
        ("handles", drag(&[(x, y), (x + 10.0, y)], &["alt"])),
    ];
    for (name, events) in rows {
        let h = ready();
        setup(&h);
        let (bytes, hist) = (canonical(&h), history(&h));
        let env = h.pointer(Origin::User, json!({ "view": "drawing", "tool": TOOL, "events": events }));
        assert_eq!(env.status, Status::Error, "{name}: {env:?}");
        assert_eq!(env.error.as_ref().unwrap().code, "drawing.locked", "{name}: {env:?}");
        assert_eq!(canonical(&h), bytes, "{name}: document unchanged");
        assert_eq!(history(&h), hist, "{name}: history unchanged");
    }
}

#[test]
fn an_omitted_snap_follows_the_setting() {
    // Two strokes of one layer: the square, and a line whose end lies 4.24 du from where the corner is moved.
    let run = |setting: bool, snap: Option<bool>| {
        let h = ready();
        let (sid, a) = square(&h);
        ok(&h, "tool.snapToContour.set", json!({ "enabled": setting }));
        let c = a[0];
        ok(
            &h,
            "draw.line.add",
            json!({ "drawingId": D1, "from": {"x": c.0 - 60.0, "y": c.1 - 60.0}, "to": {"x": c.0 - 20.0, "y": c.1 - 20.0}, "layer": "top" }),
        );
        let mut p =
            json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}], "delta": {"x": -17.0, "y": -17.0} });
        if let Some(b) = snap {
            p["snap"] = json!(b);
        }
        ok(&h, "draw.point.move", p);
        canonical(&h)
    };
    assert_eq!(run(false, None), run(false, Some(false)), "omitted with the setting off: no snap");
    assert_eq!(run(true, None), run(true, Some(true)), "omitted with the setting on: snap (SI-187)");
    assert_ne!(run(false, Some(false)), run(false, Some(true)), "the fixture snaps when asked");
    assert_eq!(run(true, Some(false)), run(false, Some(false)), "an explicit value wins over the setting");
}

/// Availability equals execution for the six commands of the Contour Editor's gestures over the states a gesture can
/// meet (design note 3 §8; the O-24 rule, prechecks equal handlers): a precheck dropped or weaker than its handler
/// fails a cell.
#[test]
fn availability_equals_execution_for_the_contour_commands() {
    #[derive(Clone, Copy, Debug)]
    enum State {
        Unlocked,
        Locked,
        NoDrawing,
        EmptySelection,
        Stale,
        OutOfRange,
    }
    let commands = |sid: &str, st: State| -> Vec<(&'static str, Value)> {
        let (anchor, segment, t) = match st {
            State::Stale => (99, 99, 0.5),
            State::OutOfRange => (0, 0, 1.0),
            _ => (1, 0, 0.5),
        };
        let pts = if matches!(st, State::EmptySelection) {
            None
        } else {
            Some(json!([{ "strokeId": sid, "anchor": anchor }]))
        };
        let with_points = |mut v: Value| {
            if let Some(p) = &pts {
                v["points"] = p.clone();
            }
            v
        };
        vec![
            ("draw.pointSelection.set", with_points(json!({ "mode": "replace" }))),
            ("draw.point.move", with_points(json!({ "delta": {"x": 3.0, "y": 0.0} }))),
            ("draw.point.delete", with_points(json!({}))),
            (
                "draw.segment.move",
                json!({ "strokeId": sid, "segment": segment, "t": t, "delta": {"x": 0.0, "y": 5.0} }),
            ),
            ("draw.point.add", json!({ "strokeId": sid, "segment": segment, "t": t })),
            ("draw.point.set", json!({ "strokeId": sid, "anchor": anchor, "handleMode": "reset" })),
        ]
    };
    let states =
        [State::Unlocked, State::Locked, State::NoDrawing, State::EmptySelection, State::Stale, State::OutOfRange];
    let mut mismatches = Vec::new();
    let mut cells = 0;
    for st in states {
        for k in 0..6 {
            let h = ready();
            let (sid, _) = square(&h);
            match st {
                State::Locked => {
                    ok(&h, "element.drawingLock.set", json!({ "drawingIds": [D1], "locked": true }));
                }
                State::NoDrawing => {
                    ok(&h, "app.context.set", json!({ "elementId": BOB, "frame": 999 }));
                }
                _ => {}
            }
            let (id, params) = commands(&sid, st)[k].clone();
            let a = h.availability(vec![(id.to_owned(), params.as_object().unwrap().clone())]).unwrap();
            let r = run(&h, id, params.clone());
            cells += 1;
            if a[0].enabled != (r.status == Status::Ok) {
                mismatches.push(format!("{st:?} {id} {params}: availability {:?}, handler {:?}", a[0], r.error));
            }
        }
    }
    assert_eq!(cells, 36);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

// ----- the hit corpus (design note 3 §4: one rule, two ports) ----------------------------------------------------------

fn affine(v: &Value) -> [f64; 6] {
    let a = v.as_array().unwrap();
    std::array::from_fn(|i| a[i].as_f64().unwrap())
}

fn map(m: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// The square with anchor 1 given handles and selected (so its handles are targets), the stroke selected.
fn corpus_state(h: &ExecutorHandle) -> String {
    let (sid, _) = square(h);
    ok(
        h,
        "draw.point.set",
        json!({ "drawingId": D1, "strokeId": sid, "anchor": 1, "inHandle": {"x": -18.0, "y": 6.0},
                "outHandle": {"x": 14.0, "y": 22.0}, "handleMode": "broken" }),
    );
    ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}] }));
    ok(h, "tool.active.set", json!({ "toolId": TOOL }));
    sid
}

/// The target the kernel's driver chose for a press at `p`, read from the command of a drag from it (then undone).
fn kernel_target(h: &ExecutorHandle, view: &str, sid: &str, p: (f64, f64)) -> Value {
    let env = h.pointer(
        Origin::User,
        json!({ "view": view, "tool": TOOL, "events": drag(&[p, (p.0 + 10.0, p.1 + 7.0)], &[]) }),
    );
    assert_eq!(env.status, Status::Ok, "{env:?}");
    let commands = env.result.as_ref().unwrap()["commands"].as_array().unwrap().clone();
    let c = &commands[1];
    let params = &c["params"];
    let target = match c["commandId"].as_str().unwrap() {
        "draw.point.move" => {
            let pts = params["points"].as_array().unwrap();
            assert_eq!(pts.len(), 1, "{params}");
            json!({ "kind": "anchor", "stroke": pts[0]["strokeId"], "anchor": pts[0]["anchor"] })
        }
        "draw.point.set" => json!({ "kind": "handle", "stroke": params["strokeId"], "anchor": params["anchor"],
                                    "out": params.get("outHandle").is_some() }),
        "draw.segment.move" => json!({ "kind": "segment", "stroke": params["strokeId"], "segment": params["segment"],
                                       "t": params["t"] }),
        "draw.pointSelection.set" => Value::Null,
        other => panic!("{other}: {params}"),
    };
    // Back to the corpus state: the edit undone, the selections restored.
    if !target.is_null() {
        ok(h, "edit.undo", json!({}));
    }
    ok(h, "draw.selection.set", json!({ "drawingId": D1, "by": "ids", "ids": [sid], "mode": "replace" }));
    ok(h, "draw.pointSelection.set", json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 1}] }));
    target
}

/// `desktop/src/canvas/testdata/contour-hits.json`: presses against the corpus state at zoom 0.5, 1 and 2, a rotated
/// Drawing view and two Camera views, each with the scene answer's `contours`, the view's drawing mapping and the
/// target the kernel's driver chose. Vitest's `contour.test.ts` asks `contourHitAt` for the same presses (same
/// target, same `t` within 1e-9). `TT_UPDATE_CANVAS_CORPUS=1` rewrites it.
#[test]
fn the_hit_corpus_is_the_kernels_driver() {
    type Config<'a> = (&'a str, &'a str, Vec<(&'a str, Value)>);
    let configs: Vec<Config> = vec![
        ("drawing-zoom-0.5", "drawingView", vec![("view.zoom.set", json!({ "viewId": "drawingView", "zoom": 0.5 }))]),
        ("drawing-zoom-1", "drawingView", vec![]),
        ("drawing-zoom-2", "drawingView", vec![("view.zoom.set", json!({ "viewId": "drawingView", "zoom": 2.0 }))]),
        (
            "drawing-rotated",
            "drawingView",
            vec![
                ("view.zoom.set", json!({ "viewId": "drawingView", "zoom": 1.5 })),
                ("view.rotation.set", json!({ "viewId": "drawingView", "degrees": 30.0 })),
            ],
        ),
        ("camera", "cameraView", vec![]),
        (
            "camera-zoom-rotated",
            "cameraView",
            vec![
                ("view.zoom.set", json!({ "viewId": "cameraView", "zoom": 2.0 })),
                ("view.rotation.set", json!({ "viewId": "cameraView", "degrees": -30.0 })),
            ],
        ),
    ];
    let mut out = Vec::new();
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for (name, view, steps) in configs {
        let h = ready();
        let sid = corpus_state(&h);
        for (id, p) in steps {
            ok(&h, id, p);
        }
        let scene = serde_json::to_value(h.canvas_scene(view).unwrap()).unwrap();
        let contours = scene["contours"].clone();
        assert!(contours.is_object(), "{name}: the scene answer carries the overlay");
        let m = affine(&ok(&h, "draw.viewMapping.get", json!({ "view": view }))["drawingToView"]);
        let anchors = anchors(&h, &sid);
        // Presses near every anchor, the two handle ends and points of every segment, in view pixels.
        let mut sites: Vec<(f64, f64)> = anchors.iter().map(|a| map(&m, a.0, a.1)).collect();
        let a1 = anchors[1];
        sites.push(map(&m, a1.0 - 18.0, a1.1 + 6.0));
        sites.push(map(&m, a1.0 + 14.0, a1.1 + 22.0));
        for k in 0..anchors.len() {
            let (a, b) = (anchors[k], anchors[(k + 1) % anchors.len()]);
            for t in [0.08, 0.3, 0.5] {
                sites.push(map(&m, a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
        }
        let offsets = [(0.0, 0.0), (3.0, 0.0), (0.0, -4.0), (4.5, 4.5), (-5.5, 0.0), (0.0, 6.5), (-7.0, 2.0)];
        let mut presses = Vec::new();
        for s in &sites {
            for o in offsets {
                let p = (s.0 + o.0, s.1 + o.1);
                let target = kernel_target(&h, view, &sid, p);
                *kinds.entry(target.get("kind").and_then(Value::as_str).unwrap_or("none").to_owned()).or_default() += 1;
                presses.push(json!({ "p": [p.0, p.1], "target": target }));
            }
        }
        out.push(json!({ "name": name, "view": view, "drawingToView": m, "contours": contours, "presses": presses }));
    }
    // Every kind of target occurs, misses included.
    for k in ["anchor", "handle", "segment", "none"] {
        assert!(kinds.get(k).copied().unwrap_or(0) >= 5, "{k}: {kinds:?}");
    }
    let text = serde_json::to_string_pretty(&json!({ "configs": out })).unwrap() + "\n";
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/src/canvas/testdata/contour-hits.json");
    if std::env::var_os("TT_UPDATE_CANVAS_CORPUS").is_some() {
        std::fs::write(&path, &text).unwrap();
    } else {
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("contour-hits.json: {e} (TT_UPDATE_CANVAS_CORPUS=1 writes it)"));
        assert!(committed.replace("\r\n", "\n") == text, "contour-hits.json differs from the kernel's driver");
    }
}

// ----- Snap to Contour's candidates (SI-070 as decided; ruling D-76 item 1 withdrew D-75 item 1) ----------------------

fn polyline(h: &ExecutorHandle, pts: &[(f64, f64)]) -> String {
    let p: Vec<Value> = pts.iter().map(|q| json!({ "x": q.0, "y": q.1 })).collect();
    let r = ok(h, "draw.polyline.add", json!({ "drawingId": D1, "points": p, "layer": "new", "snap": false }));
    r["strokeIds"][0].as_str().unwrap().to_owned()
}

#[test]
fn an_open_strokes_end_does_not_snap_to_its_own_start() {
    let h = ready();
    let sid = polyline(&h, &[(300.0, 300.0), (400.0, 300.0), (400.0, 400.0), (300.0, 400.0), (305.0, 330.0)]);
    // The end moves to 5 du from its own start: the stroke has a moved anchor, so none of its points is a candidate
    // (SI-070); the end lands where the delta puts it and no zone is made.
    let r = ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 4}], "delta": {"x": -2.0, "y": -26.0}, "snap": true }),
    );
    let pts = anchors(&h, &sid);
    assert_eq!(pts.last().copied(), Some((303.0, 304.0)), "{pts:?}");
    assert_eq!(pts.first().copied(), Some((300.0, 300.0)));
    let zones =
        r["createdIds"].as_array().unwrap().iter().filter(|z| z.as_str().is_some_and(|z| z.starts_with("zn_"))).count();
    assert_eq!(zones, 0, "no closed shape: {r}");
}

#[test]
fn no_anchor_of_a_moved_stroke_is_a_snap_candidate() {
    let h = ready();
    let sid = polyline(&h, &[(300.0, 300.0), (350.0, 300.0), (400.0, 300.0)]);
    // Anchors 0 and 1 move by (45, 3): anchor 0 lands 5.8 du from anchor 1's old place, anchor 1 lands 5.8 du from
    // anchor 2, which does not move; the stroke has moved anchors, so neither snaps (SI-070).
    ok(
        &h,
        "draw.point.move",
        json!({ "drawingId": D1, "points": [{"strokeId": sid, "anchor": 0}, {"strokeId": sid, "anchor": 1}],
                "delta": {"x": 45.0, "y": 3.0}, "snap": true }),
    );
    let pts = anchors(&h, &sid);
    assert_eq!(pts[0], (345.0, 303.0), "{pts:?}");
    assert_eq!(pts[1], (395.0, 303.0), "{pts:?}");
}
