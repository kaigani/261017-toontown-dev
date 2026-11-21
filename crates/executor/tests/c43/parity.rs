//! The parity gate, kernel half (M4 C4.3b; parity note 9, rulings D-25, D-30, D-31, D-35, D-36). The checks live in
//! `tests/common/parity_gate.rs`, shared with the reviewers' test files; these tests run them on the committed
//! manifest and tables, and each check against a fixture built to break it. The freshness of the manifest's keymap,
//! toolbar and gesture sections, and the gesture table against the driver both ways, are tested here.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, Origin};
use tt_registry::Kind;

use crate::common::parity_gate::{
    DEFERRED, GESTURES, READS, TOOLBAR, WINDOW_GESTURES, actions, catalogue, deferred_entries, manifest, owner_problem,
    pa2, pa3, pa3_entries, pa3_raw, reads_problems, section_problems, states, window_gesture_problems,
};
use crate::common::*;

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
    let (problems, counts) = pa3_raw(&m, DEFERRED);
    eprintln!("PA3 states: {counts:?}");
    for (state, ids) in states(&m, DEFERRED) {
        eprintln!("{state} ({}): {}", ids.len(), ids.join(", "));
    }
    assert_eq!(problems, Vec::<String>::new());
    // The check catches what it is for: a command in no state, in two, a passed owner, an entry without a section.
    let mut d2 = d.clone();
    let deferred = d2["deferred"].as_object_mut().unwrap();
    let (gone, _) = deferred.iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap();
    deferred.remove(&gone);
    deferred.insert("edit.undo".into(), json!({ "owner": "M4 C4.4", "section": "01 §6.1", "location": "x" }));
    deferred.insert("tool.list".into(), json!({ "owner": "M4 C4.2", "section": "02 §7", "location": "x" }));
    deferred.insert("app.info.get".into(), json!({ "owner": "M5 C5.1", "section": "", "location": "x" }));
    let mut m2 = m.clone();
    m2["menu"].as_array_mut().unwrap().push(json!({ "id": "x", "surface": "menu", "commands": ["keymap.get"] }));
    let (bad, _) = pa3(&m2, &d2);
    assert!(bad.iter().any(|p| p.contains("`keymap.get` is in several states")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains(&format!("`{gone}` is in no state"))), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("`edit.undo` is in several states")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("owner `M4 C4.2` has passed")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("`app.info.get`: no spec section")), "{bad:?}");
}

/// D-35: a mention in the source binds nothing. A mutating command whose only action is removed, and a query whose
/// declared read and call site are removed, fall into no state although `uses` still names them.
#[test]
fn d35_a_mention_binds_nothing() {
    let m = manifest();
    let entries = deferred_entries(DEFERRED).unwrap();
    // `keymap.export`: its one action is the shortcut editor's button.
    let mut m2 = m.clone();
    m2["dom"]
        .as_array_mut()
        .unwrap()
        .retain(|a| !a["commands"].as_array().unwrap().iter().any(|c| c == "keymap.export"));
    assert!(
        m2["uses"].as_array().unwrap().iter().any(|u| u["command"] == "keymap.export"),
        "the fixture keeps the mention"
    );
    let (bad, _) = pa3_entries(&m2, &entries, READS);
    assert!(bad.iter().any(|p| p.contains("`keymap.export` is in no state")), "{bad:?}");
    // `tool.list`: read by the Drawing tools palette (in every window that renders it); without a declared read on any
    // surface the mention binds nothing.
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    let mut declared = 0;
    for (_, s) in reads["surfaces"].as_object_mut().unwrap() {
        declared += usize::from(s["reads"].as_object_mut().unwrap().remove("tool.list").is_some());
    }
    assert!(declared > 0, "the fixture removes a declared read");
    let (bad, _) = pa3_entries(&m, &entries, &reads.to_string());
    assert!(bad.iter().any(|p| p.contains("`tool.list` is in no state")), "{bad:?}");
}

/// D-35: the declared reads and the query call sites of product code agree both ways.
#[test]
fn d35_declared_reads_and_call_sites_agree_both_ways() {
    let m = manifest();
    assert_eq!(reads_problems(&m, READS), Vec::<String>::new());
    // A query a surface sends without declaring it fails (C43B-R3S-01), and a call site no surface declares fails.
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    reads["surfaces"]["main"]["reads"].as_object_mut().unwrap().remove("view.list");
    let bad = reads_problems(&m, &reads.to_string());
    assert!(
        bad.iter().any(|p| p.contains("surface `main` sends `view.list` when rendered but declares no read of it")),
        "{bad:?}"
    );
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    for (_, s) in reads["surfaces"].as_object_mut().unwrap() {
        s["reads"].as_object_mut().unwrap().remove("view.list");
    }
    let bad = reads_problems(&m, &reads.to_string());
    assert!(bad.iter().any(|p| p.contains("`view.list` is read in desktop/src/panels/PanelBody.tsx")), "{bad:?}");
    // A declared read without a call site fails, as does a declared mutating command.
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    reads["surfaces"]["main"]["reads"]["job.list"] = json!(["desktop/src/panels/PanelBody.tsx"]);
    reads["surfaces"]["main"]["reads"]["edit.undo"] = json!(["desktop/src/panels/PanelBody.tsx"]);
    let bad = reads_problems(&m, &reads.to_string());
    assert!(bad.iter().any(|p| p.contains("declares `job.list` in desktop/src/panels/PanelBody.tsx")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("`edit.undo` is not a query")), "{bad:?}");
    // C43B-R2S-01, C43B-R2-01: a surface key the collector does not render fails, with or without reads; so does a real
    // surface renamed (its reads moved to an invented key).
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    reads["surfaces"]["dialog:Nowhere"] = json!({ "description": "x", "reads": {} });
    let bad = reads_problems(&m, &reads.to_string());
    assert!(bad.iter().any(|p| p.contains("`dialog:Nowhere` is not a surface the collector renders")), "{bad:?}");
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    let moved = reads["surfaces"].as_object_mut().unwrap().remove("dialog:Help").unwrap();
    reads["surfaces"]["helpWindow"] = moved;
    let bad = reads_problems(&m, &reads.to_string());
    assert!(
        bad.iter().any(|p| p == "reads.json: surface `helpWindow` is not a surface the collector renders"),
        "{bad:?}"
    );
    // C43B-R3S-01 (ruling D-48): a surface that declares a read it does not send fails, even when another surface in the
    // same file sends it (About declaring Project Info's query, both in infoDialogs.tsx); and a query a surface sends
    // without declaring it fails.
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    reads["surfaces"]["dialog:About Toontown"]["reads"]["project.info.get"] =
        json!(["desktop/src/shell/infoDialogs.tsx"]);
    let bad = reads_problems(&m, &reads.to_string());
    assert_eq!(
        bad,
        vec![
            "reads.json: surface `dialog:About Toontown` declares `project.info.get` (desktop/src/shell/infoDialogs.tsx), which it does not send when rendered"
                .to_owned()
        ]
    );
    let mut reads: Value = serde_json::from_str(READS).unwrap();
    reads["surfaces"]["dialog:Project Info"]["reads"].as_object_mut().unwrap().remove("project.info.get");
    let bad = reads_problems(&m, &reads.to_string());
    assert!(
        bad.iter().any(|p| p == "surface `dialog:Project Info` sends `project.info.get` when rendered but declares no read of it (reads.json)"),
        "{bad:?}"
    );
    // A call site removed from the manifest (an inert mention left behind) leaves its declared read without a call.
    let mut m2 = m.clone();
    m2["callSites"].as_array_mut().unwrap().retain(|c| c["command"] != "tool.list");
    let bad = reads_problems(&m2, READS);
    assert!(bad.iter().any(|p| p.contains("declares `tool.list`")), "{bad:?}");
}

/// The window gestures are called where their rows say; a row whose command no file calls fails.
#[test]
fn window_gestures_have_their_call_sites() {
    let m = manifest();
    assert_eq!(window_gesture_problems(&m, WINDOW_GESTURES), Vec::<String>::new());
    let mut rows: Value = serde_json::from_str(WINDOW_GESTURES).unwrap();
    rows["rows"][0]["files"] = json!(["desktop/src/App.tsx"]);
    let bad = window_gesture_problems(&m, &rows.to_string());
    assert!(bad.iter().any(|p| p.contains("dock:drop: no file it names calls `window.panel.move`")), "{bad:?}");
}

/// D-36: owners in their exact syntax from the plan's catalogue and not passed; sections that exist; commands that are
/// implemented and not events; duplicates refused while parsing. Each rule fails on its fixture.
#[test]
fn d36_deferred_entries_are_validated_in_full() {
    let cat = catalogue();
    for owner in ["M4 C4.4", "M5 C5.1", "M6 C6.2", "M10 C10.2", "M11 C11.1"] {
        assert_eq!(owner_problem(owner, &cat), None, "{owner}");
    }
    for owner in
        ["M5 C5.1.trailing", "M5 C6.1", "M5 C5.9", "M99 C99.0", "M5  C5.1", "M5 C5.", "M4 C4.3", "M4 C4.2", "M0 C0.1"]
    {
        assert!(owner_problem(owner, &cat).is_some(), "owner `{owner}` accepted");
    }
    let m = manifest();
    let base = deferred_entries(DEFERRED).unwrap();
    let (id, entry) = base[0].clone();
    let with = |f: &dyn Fn(&mut Value)| {
        let mut e = base.clone();
        f(&mut e[0].1);
        pa3_entries(&m, &e, READS).0
    };
    for owner in ["5 C5.1", "nonsense", "M5 C5.1.trailing"] {
        let bad = with(&|v| v["owner"] = json!(owner));
        assert!(bad.iter().any(|p| p.contains(&format!("owner `{owner}`"))), "{owner}: {bad:?}");
    }
    let bad = with(&|v| v["section"] = json!("02 §99.9 (nothing)"));
    assert!(bad.iter().any(|p| p.contains("has no heading §99.9")), "{bad:?}");
    let bad = with(&|v| v["section"] = json!("a section without a sign"));
    assert!(bad.iter().any(|p| p.contains("no spec section cited")), "{bad:?}");
    assert!(section_problems("x", "§7.6 (no file number)").iter().any(|p| p.contains("names no spec file")));
    // A command that is an event, and one that is not registered.
    let mut e = base.clone();
    e.push(("job.finished".into(), entry.clone()));
    e.push(("review.missing".into(), entry.clone()));
    let bad = pa3_entries(&m, &e, READS).0;
    assert!(bad.iter().any(|p| p.contains("deferred `job.finished` is an event")), "{bad:?}");
    assert!(bad.iter().any(|p| p.contains("deferred `review.missing` is not registered")), "{bad:?}");
    // A duplicate entry in the raw file.
    let at = DEFERRED.find("\"deferred\": {").unwrap() + "\"deferred\": {".len();
    let mut raw = DEFERRED.to_owned();
    raw.insert_str(at, &format!("\n    {}: {},", serde_json::to_string(&id).unwrap(), entry));
    let (bad, _) = pa3_raw(&m, &raw);
    assert!(bad.iter().any(|p| p.contains(&format!("deferred `{id}` is listed twice"))), "{bad:?}");
    // Ruling on C43B-R2-02: a key with an escape sequence or a non-ASCII character, at any depth, is refused by the raw
    // scan before anything is decoded (an escaped duplicate, an escaped root, an escaped entry field, a non-ASCII key),
    // without a panic; an escape in a value is not a key and stays allowed.
    for raw in [
        r#"{"deferred":{"anim.camera.set":{"owner":"M4 C4.4"},"anim.camera.\u0073et":{"owner":"M4 C4.4"}}}"#,
        r#"{"defer\u0072ed":{"anim.camera.set":{},"anim.camera.set":{}}}"#,
        r#"{"deferred":{"anim.camera.set":{"own\u0065r":"M4 C4.4"}}}"#,
        "{\"deferred\":{\"anim.camera.s\u{0435}t\":{}}}",
    ] {
        let err = deferred_entries(raw).expect_err(raw);
        assert!(err.contains("keys are plain ASCII"), "{raw}: {err}");
    }
    let ok = deferred_entries(r#"{"deferred":{"anim.camera.set":{"owner":"M4 C\u0034.4"}}}"#).unwrap();
    assert_eq!(ok[0].1["owner"], "M4 C4.4");
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
    named.extend(named_commands(include_str!("../../src/contour_gesture.rs")));
    named.extend(named_commands(include_str!("../../src/envelope_gesture.rs")));
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
    // The Contour Editor (C4.4b; design note 3 §2): every row of its gesture table in both views, and the Delete key the
    // kernel's key dispatch routes to it while it is active (SI-184).
    for (view, req) in [("drawing", "drawingView"), ("camera", "cameraView")] {
        let fresh = || {
            let h = ready();
            let r = ok(
                &h,
                "draw.rectangle.add",
                json!({ "from": {"x": 300, "y": 300}, "to": {"x": 400, "y": 400}, "layer": "new" }),
            );
            let sid = r["strokeIds"][0].as_str().unwrap().to_owned();
            ok(&h, "draw.selection.set", json!({ "by": "ids", "ids": [sid], "mode": "replace" }));
            ok(&h, "draw.pointSelection.set", json!({ "points": [{ "strokeId": sid, "anchor": 1 }] }));
            ok(&h, "tool.active.set", json!({ "toolId": "contourEditor" }));
            let d = ok(&h, "draw.drawing.get", json!({ "flatten": true }));
            let n = d["nodes"].as_array().unwrap().iter().find(|n| n["id"] == sid.as_str()).cloned().unwrap();
            let b = n["bezier"].as_array().unwrap().clone();
            let a: Vec<(f64, f64)> =
                (0..4).map(|k| (b[k * 3]["x"].as_f64().unwrap(), b[k * 3]["y"].as_f64().unwrap())).collect();
            let m = if view == "camera" { bob_to_view(&h) } else { [1.0, 0.0, 0.0, -1.0, 960.0, 540.0] };
            let v = move |x: f64, y: f64| crate::camera::apply(&m, x, y);
            (h, a, v)
        };
        type ToView<'a> = &'a dyn Fn(f64, f64) -> (f64, f64);
        type Row = Box<dyn Fn(&[(f64, f64)], ToView) -> Vec<Value>>;
        let gestures: Vec<Row> = vec![
            // A click off every target: the Select click, then the point selection cleared.
            Box::new(|_, v| click(v(-900.0, -900.0), &[])),
            // An anchor drag, a segment drag, a temporary-select click on a segment, an alternate click on an anchor.
            Box::new(|a, v| {
                let p = v(a[0].0, a[0].1);
                drag(&[p, (p.0 + 12.0, p.1 + 5.0)], &[])
            }),
            Box::new(|a, v| {
                let p = v((a[2].0 + a[3].0) / 2.0, (a[2].1 + a[3].1) / 2.0);
                drag(&[p, (p.0, p.1 + 12.0)], &[])
            }),
            Box::new(|a, v| click(v((a[2].0 + a[3].0) / 2.0, (a[2].1 + a[3].1) / 2.0), &["ctrl"])),
            Box::new(|a, v| click(v(a[2].0, a[2].1), &["alt"])),
        ];
        for g in &gestures {
            let (h, a, v) = fresh();
            put(
                view,
                "contourEditor",
                answered(&h, json!({ "view": req, "tool": "contourEditor", "events": g(&a, &v) })),
            );
        }
        // Perspective (design note 4 §3): a click selects, a corner drag maps the selection; Reposition All (§4): a drag
        // repositions every drawing.
        let (h, _, v) = fresh();
        ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
        // The corner of the box the scene answer publishes (the handles' box; a stroke's anchor sits off it by half
        // its thickness).
        let tb = serde_json::to_value(h.canvas_scene(req).unwrap()).unwrap()["toolBox"]["box"].clone();
        let p = v(tb[2].as_f64().unwrap(), tb[3].as_f64().unwrap());
        put(
            view,
            "perspective",
            answered(
                &h,
                json!({ "view": req, "tool": "perspective", "events": drag(&[p, (p.0 - 15.0, p.1 + 4.0)], &[]) }),
            ),
        );
        let (h, _, v) = fresh();
        ok(&h, "tool.active.set", json!({ "toolId": "perspective" }));
        put(
            view,
            "perspective",
            answered(&h, json!({ "view": req, "tool": "perspective", "events": click(v(-900.0, -900.0), &[]) })),
        );
        let (h, a, v) = fresh();
        let p = v((a[0].0 + a[2].0) / 2.0, (a[0].1 + a[2].1) / 2.0);
        put(
            view,
            "repositionAll",
            answered(
                &h,
                json!({ "view": req, "tool": "repositionAll", "events": drag(&[p, (p.0 + 20.0, p.1 + 9.0)], &[]) }),
            ),
        );
        let (h, _, _) = fresh();
        if view == "camera" {
            ok(&h, "app.mode.set", json!({ "mode": "staging" }));
        }
        match h.resolve_key("Delete").unwrap() {
            tt_executor::host::KeyResolution::Command { command_id, params, .. } => {
                ok(&h, &command_id, params);
                put(view, "contourEditor", vec![command_id]);
            }
            other => panic!("Delete with the Contour Editor: {other:?}"),
        }
    }
    out
}
