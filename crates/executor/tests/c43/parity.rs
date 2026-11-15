//! The parity gate, kernel half (M4 C4.3b; parity note 9, rulings D-25, D-30, D-31). PA2: every command a GUI action
//! of the generated manifest sends is registered, not an event, and its fixed parameters validate. PA3: every
//! implemented command is exactly one of bound (the manifest sends or uses it), agent-only (its declaration says so,
//! with a justification) or deferred (`deferred.json`: an owner checkpoint that has not passed and the spec section).
//! Freshness: the manifest's keymap, toolbar and gesture sections are the kernel's data, both ways; the gesture table
//! is the driver's, both ways. Each check fails on a fixture built to break it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tt_executor::host::KeyResolution;
use tt_executor::{ExecutorHandle, Origin};
use tt_registry::{Kind, Status, validate_params};

use crate::common::*;

const MANIFEST: &str = include_str!("../../../../desktop/src/ui-bindings/parity.json");
const DEFERRED: &str = include_str!("../../../../desktop/src/ui-bindings/deferred.json");
const GESTURES: &str = include_str!("../../src/gesture-table.json");
const TOOLBAR: &str = include_str!("../../src/toolbar-default.json");

/// The sections of GUI actions (each entry: `{id, surface, commands, params?}`; the gestures: `{view, tool, commands}`).
const SECTIONS: &[&str] = &["dom", "menu", "keymap", "toolbar", "gestures"];

/// The checkpoints that have passed when this checkpoint hands off: a `deferred` owner must come after them (D-31).
/// M4 C4.3b's own handoff makes C4.3b passed too, so C4.4 is the first M4 checkpoint a GUI location may wait for.
fn passed(owner: &str) -> bool {
    let Some((m, c)) = owner.split_once(' ') else { return true };
    let (Ok(m), Some(c)) = (m.trim_start_matches('M').parse::<u32>(), c.strip_prefix('C')) else { return true };
    let mut parts = c.split('.').map(str::parse::<u32>);
    let (Some(Ok(cm)), Some(Ok(ck))) = (parts.next(), parts.next()) else { return true };
    cm != m || m < 4 || (m == 4 && ck <= 3)
}

fn manifest() -> Value {
    serde_json::from_str(MANIFEST).unwrap()
}

/// Every (command, params) a GUI action of `m` sends.
fn actions(m: &Value) -> Vec<(String, Option<Value>, String)> {
    let mut out = Vec::new();
    for sec in SECTIONS {
        for a in m[*sec].as_array().unwrap() {
            let at = format!("{sec}:{}", a["id"].as_str().or(a["tool"].as_str()).unwrap_or("?"));
            for c in a["commands"].as_array().unwrap() {
                out.push((c.as_str().unwrap().to_owned(), a.get("params").cloned(), at.clone()));
            }
        }
    }
    out
}

/// PA2 over a manifest: the problems (an unregistered command, an event, parameters that do not validate). Each
/// action's parameters are completed the way the GUI runs them (`complete_reference`: a toggle the reference leaves
/// open gets its next state), in a fixture with a project open; when required parameters are still missing the GUI
/// collects them with a dialog (01 §0), so the parameters given are checked against the schema without `required`.
fn pa2(m: &Value) -> Vec<String> {
    let r = tt_executor::registry().unwrap();
    let h = fixture();
    let mut out = Vec::new();
    for (id, params, at) in actions(m) {
        let Some(e) = r.get(&id) else {
            out.push(format!("{at}: `{id}` is not registered"));
            continue;
        };
        if e.spec.kind == Kind::Event {
            out.push(format!("{at}: `{id}` is an event"));
            continue;
        }
        // A gesture's parameters are the driver's (its samples): the battery of the gesture table runs them.
        if at.starts_with("gestures:") {
            continue;
        }
        let given = params.and_then(|p| p.as_object().cloned()).unwrap_or_default();
        let (params, dialog) = match h.complete_reference(&id, given) {
            Ok(KeyResolution::Command { params, dialog, .. }) => (params, dialog),
            other => {
                out.push(format!("{at}: `{id}` does not complete: {other:?}"));
                continue;
            }
        };
        let mut schema = e.params_schema();
        if dialog && let Some(o) = schema.as_object_mut() {
            o.remove("required");
        }
        if let Err(err) = validate_params(&id, &schema, &params) {
            out.push(format!("{at}: `{id}` {params}: {}", err.message));
        }
    }
    out
}

/// The commands a GUI action of the manifest sends.
fn sent(m: &Value) -> BTreeSet<String> {
    actions(m).into_iter().map(|a| a.0).collect()
}

/// The commands the front end's or the host's source names by id (the reads behind a surface, the host's own calls).
fn read(m: &Value) -> BTreeSet<String> {
    m["uses"].as_array().unwrap().iter().map(|u| u["command"].as_str().unwrap().to_owned()).collect()
}

/// PA3 over a manifest and a deferred table: the commands in none or several of the three states, and the deferred
/// entries whose owner has passed or that lack a section. Bound: a GUI action sends it, or the source reads it and it
/// is not declared agent-only. A command declared agent-only that the source names is agent-only: the read is the
/// GUI's plumbing (its state stores, the host's tree and geometry reads), not a location the user acts at; one that a
/// GUI action sends is in two states (parity note 9, PA3).
fn pa3(m: &Value, deferred: &Value) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    let r = tt_executor::registry().unwrap();
    let (sent, read) = (sent(m), read(m));
    let d = deferred["deferred"].as_object().unwrap();
    let mut problems = Vec::new();
    let mut counts = BTreeMap::new();
    for e in r.entries() {
        if e.spec.kind == Kind::Event || !matches!(e.status(), Status::Implemented) {
            continue;
        }
        let id = e.id();
        let agent = e.command.and_then(|c| c.decl.agent_only).filter(|j| j.len() > 10);
        let bound = sent.contains(id) || (agent.is_none() && read.contains(id));
        let states: Vec<&'static str> =
            [bound.then_some("bound"), agent.map(|_| "agent-only"), d.contains_key(id).then_some("deferred")]
                .into_iter()
                .flatten()
                .collect();
        match states.as_slice() {
            [one] => *counts.entry(*one).or_insert(0) += 1,
            [] => problems.push(format!("`{id}` is in no state: bind it, declare it agent-only, or defer it")),
            many => problems.push(format!("`{id}` is in several states: {many:?}")),
        }
    }
    for (id, v) in d {
        let owner = v["owner"].as_str().unwrap_or("");
        if passed(owner) {
            problems.push(format!("deferred `{id}`: owner `{owner}` has passed (D-31)"));
        }
        if v["section"].as_str().is_none_or(str::is_empty) {
            problems.push(format!("deferred `{id}`: no spec section (D-31)"));
        }
        if r.get(id).is_none() {
            problems.push(format!("deferred `{id}` is not registered"));
        }
    }
    (problems, counts)
}

#[test]
fn pa2_every_manifest_command_is_registered_and_its_fixed_parameters_validate() {
    let m = manifest();
    assert!(actions(&m).len() > 150, "the manifest holds the GUI's actions");
    assert_eq!(pa2(&m), Vec::<String>::new());
    // The check catches what it is for: an unregistered command, an event, parameters that do not validate.
    let mut bad = m.clone();
    bad["menu"].as_array_mut().unwrap().push(json!({ "id": "x", "surface": "menu", "commands": ["no.such.command"] }));
    bad["menu"].as_array_mut().unwrap().push(json!({ "id": "y", "surface": "menu", "commands": ["job.finished"] }));
    bad["menu"].as_array_mut().unwrap().push(
        json!({ "id": "z", "surface": "menu", "commands": ["draw.flip.apply"], "params": { "axis": "diagonal" } }),
    );
    assert_eq!(pa2(&bad).len(), 3, "{:?}", pa2(&bad));
}

#[test]
fn pa3_every_implemented_command_is_exactly_one_of_bound_agent_only_or_deferred() {
    let (m, d) = (manifest(), serde_json::from_str::<Value>(DEFERRED).unwrap());
    let (problems, counts) = pa3(&m, &d);
    eprintln!("PA3 states: {counts:?}");
    assert_eq!(problems, Vec::<String>::new());
    // The check catches what it is for: a command in no state, in two, a passed owner, an entry without a section.
    let mut d2 = d.clone();
    let deferred = d2["deferred"].as_object_mut().unwrap();
    let (gone, _) = deferred.iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap();
    deferred.remove(&gone);
    deferred.insert("edit.undo".into(), json!({ "owner": "M4 C4.4", "section": "01", "location": "x" }));
    deferred.insert("tool.list".into(), json!({ "owner": "M4 C4.2", "section": "02 §1", "location": "x" }));
    deferred.insert("app.state.get".into(), json!({ "owner": "M5 C5.1", "section": "", "location": "x" }));
    let mut m2 = m.clone();
    m2["menu"].as_array_mut().unwrap().push(json!({ "id": "x", "surface": "menu", "commands": ["keymap.get"] }));
    let (bad, _) = pa3(&m2, &d2);
    assert!(bad.iter().any(|p| p.contains("`keymap.get` is in several states")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains(&format!("`{gone}` is in no state"))), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("`edit.undo` is in several states")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("owner `M4 C4.2` has passed")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("`app.state.get`: no spec section")), "{bad:?}");
}

#[test]
fn the_manifests_keymap_toolbar_and_gestures_are_the_kernels_data() {
    let m = manifest();
    // The keymap: the kernel's default bindings on every platform, as commands.
    let mut kernel: BTreeSet<String> = BTreeSet::new();
    for p in [tt_executor::keymap::Platform::Windows, tt_executor::keymap::Platform::Macos] {
        kernel.extend(tt_executor::keymap::defaults(p).into_iter().map(|b| b.command_id));
    }
    let listed: BTreeSet<String> =
        m["keymap"].as_array().unwrap().iter().map(|a| a["commands"][0].as_str().unwrap().to_owned()).collect();
    assert_eq!(
        listed, kernel,
        "the keymap section is the kernel's defaults (regenerate: pnpm --dir desktop parity:gen)"
    );
    // The toolbar: every command item of the default toolbar the kernel uses is in the section, and only those.
    let items: Vec<String> = serde_json::from_str(TOOLBAR).unwrap();
    let ids: BTreeSet<String> =
        m["toolbar"].as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap().to_owned()).collect();
    for item in items.iter().filter(|i| i.contains('.')) {
        assert!(ids.contains(&format!("toolbar:{item}")), "toolbar item {item}");
    }
    assert!(ids.iter().all(|i| items.iter().any(|x| i == &format!("toolbar:{x}"))), "{ids:?}");
    // The gestures: the table, verbatim.
    let table: Value = serde_json::from_str(GESTURES).unwrap();
    assert_eq!(m["gestures"], table["rows"], "the gesture section is gesture-table.json");
}

#[test]
fn the_gesture_table_is_the_drivers_both_ways() {
    let table: Value = serde_json::from_str(GESTURES).unwrap();
    let rows: BTreeMap<(String, String), BTreeSet<String>> = table["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let cmds = r["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()).collect();
            ((r["view"].as_str().unwrap().to_owned(), r["tool"].as_str().unwrap().to_owned()), cmds)
        })
        .collect();
    let listed: BTreeSet<String> = rows.values().flatten().cloned().collect();
    // The driver can end only in the commands its source names: each is in the table.
    let mut named = named_commands(include_str!("../../src/gesture.rs"));
    named.extend(named_commands(include_str!("../../src/scene_gesture.rs")));
    let missing: Vec<&String> = named.difference(&listed).collect();
    assert!(missing.is_empty(), "the driver names commands the table lacks: {missing:?}");
    // Each row is what its gestures produce, exactly: a battery over every tool in both views.
    let produced = battery();
    let rows_seen: BTreeSet<&(String, String)> = produced.keys().collect();
    let rows_listed: BTreeSet<&(String, String)> = rows.keys().collect();
    assert_eq!(rows_seen, rows_listed, "the battery covers the table's rows");
    for (row, cmds) in &rows {
        assert_eq!(&produced[row], cmds, "row {row:?}: the commands its gestures produced");
    }
}

/// The command ids a source file names that are registered commands.
fn named_commands(text: &str) -> BTreeSet<String> {
    let r = tt_executor::registry().unwrap();
    let mut out = BTreeSet::new();
    for part in text.split('"').skip(1).step_by(2) {
        if part.contains('.') && r.get(part).is_some_and(|e| e.spec.kind != Kind::Event) {
            out.insert(part.to_owned());
        }
    }
    out
}

/// One gesture's answered commands: the tool selection (`tool.active.set`, `anim.tool.set`) goes to the view's `*`
/// row with the wheel's zoom; the rest to the tool's row.
fn answered(h: &ExecutorHandle, req: Value) -> Vec<String> {
    let env = h.pointer(Origin::User, req.clone());
    assert!(env.is_ok(), "{req}: {env:?}");
    env.result.unwrap()["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["commandId"].as_str().unwrap().to_owned())
        .collect()
}

/// The gesture table's rows as produced: every drawing tool's G10 gesture in both views, the drawing Select's move
/// and pivot drags, the navigation tools, the wheel, and the scene tools' click, drag and alternate drag.
fn battery() -> BTreeMap<(String, String), BTreeSet<String>> {
    use crate::scene_tools::{
        G10Row, Shape, bob_to_view, centre, click, drag, furniture, g10_rows, gesture, painted, ready,
    };
    let mut out: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut put = |view: &str, tool: &str, cmds: Vec<String>| {
        for c in cmds {
            let row = if matches!(c.as_str(), "tool.active.set" | "anim.tool.set") { "*" } else { tool };
            out.entry((view.to_owned(), row.to_owned())).or_default().insert(c);
        }
    };
    let selected: fn(&ExecutorHandle) = |h| {
        painted(h);
        ok(h, "draw.selection.set", json!({ "by": "all" }));
    };
    let pivot = |h: &ExecutorHandle| {
        let p = ok(h, "draw.drawing.get", json!({ "drawingId": "dr_01K742SG0KKMCAP9X1E6Z5P4SW" }));
        (p["pivot"]["x"].as_f64().unwrap_or(0.0), p["pivot"]["y"].as_f64().unwrap_or(0.0))
    };
    let mut rows: Vec<G10Row> = g10_rows();
    // The drawing Select's move of a selection (from inside it) and its pivot drag (from the drawing pivot).
    rows.push(G10Row {
        tool: "select",
        shape: Shape::Drag,
        pts: vec![(350.0, 330.0), (370.0, 320.0)],
        setup: selected,
        ..g10_rows().pop().unwrap()
    });
    rows.push(G10Row {
        tool: "zoom",
        shape: Shape::Drag,
        pts: vec![(-100.0, 100.0), (100.0, -100.0)],
        ..g10_rows().pop().unwrap()
    });
    rows.push(G10Row {
        tool: "grabber",
        shape: Shape::Drag,
        pts: vec![(0.0, 0.0), (40.0, 20.0)],
        ..g10_rows().pop().unwrap()
    });
    for row in &rows {
        let (cam, drw) = (ready(), ready());
        (row.setup)(&cam);
        (row.setup)(&drw);
        let dv: Vec<(f64, f64)> = row.pts.iter().map(|p| (p.0 + 960.0, 540.0 - p.1)).collect();
        let gd = gesture(row.shape, &dv);
        put("drawing", row.tool, answered(&drw, json!({ "view": "drawingView", "tool": row.tool, "events": gd })));
        let m = bob_to_view(&cam);
        let cv: Vec<(f64, f64)> = row.pts.iter().map(|p| crate::camera::apply(&m, p.0, p.1)).collect();
        let gc = gesture(row.shape, &cv);
        // `select` names the scene Select in a Camera view (SI-148): the drawing Select is chosen first.
        let req = if row.tool == "select" {
            put("camera", "*", answered(&cam, json!({ "view": "cameraView", "tool": "select", "events": [] })));
            ok(&cam, "tool.active.set", json!({ "toolId": "select" }));
            json!({ "view": "cameraView", "events": gc })
        } else {
            json!({ "view": "cameraView", "tool": row.tool, "events": gc })
        };
        put("camera", row.tool, answered(&cam, req));
    }
    // The pivot drag, from the drawing pivot's handle in each view.
    let (drw, cam) = (ready(), ready());
    let (px, py) = pivot(&drw);
    put(
        "drawing",
        "select",
        answered(
            &drw,
            json!({ "view": "drawingView", "tool": "select",
        "events": drag(&[(px + 960.0 + 1.0, 540.0 - py + 1.0), (1000.0, 500.0)], &[]) }),
        ),
    );
    let m = bob_to_view(&cam);
    let (vx, vy) = crate::camera::apply(&m, px, py);
    ok(&cam, "tool.active.set", json!({ "toolId": "select" }));
    put(
        "camera",
        "select",
        answered(
            &cam,
            json!({ "view": "cameraView",
        "events": drag(&[(vx + 1.0, vy + 1.0), (vx + 40.0, vy + 30.0)], &[]) }),
        ),
    );
    // The wheel zooms in either view, whatever the tool.
    for (view, req) in [("drawing", "drawingView"), ("camera", "cameraView")] {
        let h = ready();
        put(
            view,
            "*",
            answered(
                &h,
                json!({ "view": req, "events": [{ "type": "wheel", "x": 400.0, "y": 300.0, "wheelDelta": -1.0 }] }),
            ),
        );
    }
    // The scene tools: a click selects, a drag moves; the alternate modifier makes the Transform tool the Skew tool.
    let scene = [
        ("scene:select", "select", &[][..]),
        ("scene:transform", "transform", &[][..]),
        ("scene:skew", "transform", &["alt"][..]),
    ];
    for (row, tool, mods) in scene {
        let h = ready();
        ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [crate::camera::BOB] }));
        let (bx, _) = furniture(&h);
        let c = centre(&bx);
        ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [] }));
        put("camera", row, answered(&h, json!({ "view": "cameraView", "tool": tool, "events": click(c, mods) })));
        // A move drags from inside the selection; the Skew tool shears from a side's midpoint (SI-151).
        let from = if row == "scene:skew" { (c.0, bx[1]) } else { c };
        let events = drag(&[from, (from.0 + 30.0, from.1 + 10.0)], mods);
        put("camera", row, answered(&h, json!({ "view": "cameraView", "events": events })));
    }
    out
}
