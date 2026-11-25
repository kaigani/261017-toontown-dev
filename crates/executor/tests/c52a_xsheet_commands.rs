//! M5 C5.2a: the seven `xsheet.*` commands (04 §12.2, §12.4, §15; spec A): the display options, the panels, the
//! column widths, and the static light table with its two command families in one state (ruling E-57), saved with the
//! scene in the project's `session.json` (04 §15.3; the defect of the earlier milestone, merge note §5), which is not a
//! document change.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod c13_support;
mod common;

use std::path::Path;

use c13_support::*;
use common::*;
use serde_json::{Value, json};
use tt_executor::ExecutorHandle;

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A saved project with a drawing element `Bob` exposing drawings 1 to 3 on frames 1 to 3, and a peg; returns the
/// handle, the target, Bob, Bob's drawings and the peg.
fn project(dir: &Path) -> (ExecutorHandle, std::path::PathBuf, String, Vec<String>, String) {
    let h = spawn(&dir.join("user"));
    ok(&h, "project.create", json!({ "name": "Sheet" }));
    let target = dir.join("Sheet.animpkg");
    job_ok(&h, "project.saveAs", json!({ "path": path(&target) }));
    let bob = ok(&h, "element.create", json!({ "type": "drawing", "name": "Bob" }))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let drawings = (1..=3)
        .map(|i| {
            ok(
                &h,
                "element.drawing.create",
                json!({ "elementId": bob, "name": format!("{i}"), "exposeFrom": i, "exposeTo": i }),
            )["drawingId"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let peg =
        ok(&h, "element.create", json!({ "type": "peg", "name": "Peg" }))["elementIds"][0].as_str().unwrap().to_owned();
    job_ok(&h, "project.save", json!({}));
    (h, target, bob, drawings, peg)
}

fn items(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "xsheet.lightTable.list", json!({}))["items"].as_array().unwrap().clone()
}

fn entries(h: &ExecutorHandle) -> Vec<Value> {
    ok(h, "drawAid.staticLightTable.list", json!({}))["entries"].as_array().unwrap().clone()
}

#[test]
fn display_options_and_panels_answer_their_state_and_announce_a_change_once() {
    let dir = tmp();
    let (h, _, _, _, _) = project(&dir);
    let rx = h.subscribe();
    let d = ok(&h, "xsheet.display.set", json!({}));
    assert_eq!(d, json!({ "thumbnails": false, "exposureText": false, "elementNames": false, "notes": true }));
    let d = ok(&h, "xsheet.display.set", json!({ "thumbnails": true, "elementNames": true }));
    assert_eq!(d, json!({ "thumbnails": true, "exposureText": false, "elementNames": true, "notes": true }));
    ok(&h, "xsheet.display.set", json!({ "thumbnails": true }));
    let views: Vec<_> = rx.try_iter().filter(|e| e.name == "shell.xsheetView").collect();
    assert_eq!(views.len(), 1, "one notice for the one effective change: {views:?}");
    assert_eq!(views[0].payload["thumbnails"], true);

    let p = ok(&h, "xsheet.panel.set", json!({}));
    assert_eq!(p, json!({ "elementList": false, "lightTable": false, "toolbar": true }));
    let p = ok(&h, "xsheet.panel.set", json!({ "elementList": true, "toolbar": false }));
    assert_eq!(p, json!({ "elementList": true, "lightTable": false, "toolbar": false }));
    // Not a document change: no revision, not dirty.
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false);
}

#[test]
fn the_light_table_panel_is_one_state_with_the_drawing_aid() {
    let dir = tmp();
    let (h, _, _, _, _) = project(&dir);
    let rx = h.subscribe();
    ok(&h, "xsheet.panel.set", json!({ "lightTable": true }));
    // The aid's own state changed, announced as the aid command announces it.
    assert!(rx.try_iter().any(|e| e.name == "drawAid.changed"
        && e.payload["aid"] == "staticLightTablePanel"
        && e.payload["state"] == true));
    ok(&h, "drawAid.staticLightTablePanel.set", json!({ "visible": false }));
    assert_eq!(ok(&h, "xsheet.panel.set", json!({}))["lightTable"], false);
}

#[test]
fn column_widths_are_set_per_column_and_reset_together() {
    let dir = tmp();
    let (h, _, bob, _, peg) = project(&dir);
    let w = ok(&h, "xsheet.columnWidth.set", json!({ "elementId": bob, "width": 120 }));
    assert_eq!(w["widths"], json!({ bob.clone(): 120 }));
    // The current element (Bob was made current by its creation, then the peg): set the peg's? A peg is no column.
    let e = err(&h, "xsheet.columnWidth.set", json!({ "elementId": peg, "width": 80 }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("arg.invalid", Some("/elementId")));
    assert_eq!(err(&h, "xsheet.columnWidth.set", json!({ "elementId": bob })).code, "arg.invalid", "a width or reset");
    assert_eq!(err(&h, "xsheet.columnWidth.set", json!({ "reset": true, "width": 40 })).code, "arg.invalid");
    assert_eq!(err(&h, "xsheet.columnWidth.set", json!({ "elementId": bob, "width": 10 })).code, "arg.invalid");
    let w = ok(&h, "xsheet.columnWidth.set", json!({ "reset": true }));
    assert_eq!(w["widths"], json!({}));
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false, "not a document change");
}

#[test]
fn the_two_light_table_families_are_one_state() {
    let dir = tmp();
    let (h, _, bob, drawings, _) = project(&dir);
    let rx = h.subscribe();
    // The sheet's family adds by cell and by drawing; the aid family lists them, and the reverse.
    let added = ok(
        &h,
        "xsheet.lightTable.add",
        json!({ "items": [{ "elementId": bob, "frame": 2 }, { "elementId": bob, "drawingId": drawings[0] }] }),
    )["itemIds"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(added.len(), 2);
    let events: Vec<_> = rx.try_iter().collect();
    assert!(events.iter().any(|e| e.name == "xsheet.lightTable.changed" && e.payload["itemIds"] == json!(added)));
    assert!(events.iter().any(|e| e.name == "drawAid.changed" && e.payload["aid"] == "staticLightTable"));
    let listed = items(&h);
    assert_eq!(
        listed.iter().map(|i| i["drawingId"].clone()).collect::<Vec<_>>(),
        vec![json!(drawings[1]), json!(drawings[0])]
    );
    assert_eq!(listed[0]["label"], "2 in Bob");
    assert_eq!(entries(&h).iter().map(|e| e["entryId"].clone()).collect::<Vec<_>>(), added);
    let via_aid =
        ok(&h, "drawAid.staticLightTable.add", json!({ "drawingId": drawings[2], "elementId": bob }))["entryId"]
            .clone();
    assert_eq!(items(&h).last().unwrap()["itemId"], via_aid);

    // Move, set (others and all) and remove, each read back through the other family.
    let order = ok(&h, "xsheet.lightTable.move", json!({ "itemId": via_aid, "index": 0 }))["order"].clone();
    assert_eq!(order[0], via_aid);
    assert_eq!(entries(&h)[0]["entryId"], via_aid);
    assert_eq!(err(&h, "xsheet.lightTable.move", json!({ "itemId": via_aid, "index": 3 })).code, "arg.invalid");
    assert_eq!(err(&h, "xsheet.lightTable.move", json!({ "itemId": "lt99", "index": 0 })).code, "ref.notFound");
    let changed = ok(&h, "xsheet.lightTable.set", json!({ "itemIds": [via_aid], "apply": "others", "visible": false }))
        ["changedIds"]
        .clone();
    assert_eq!(changed, json!(added), "Hide All Others hides the two others");
    assert!(entries(&h).iter().all(|e| e["visible"] == (e["entryId"] == via_aid)));
    let changed = ok(&h, "xsheet.lightTable.set", json!({ "apply": "all", "placement": "over" }))["changedIds"].clone();
    assert_eq!(changed.as_array().unwrap().len(), 3);
    assert!(entries(&h).iter().all(|e| e["placement"] == "overlay"));
    let again = ok(&h, "xsheet.lightTable.set", json!({ "apply": "all", "placement": "over" }))["changedIds"].clone();
    assert_eq!(again, json!([]), "only effective changes are listed");
    assert_eq!(err(&h, "xsheet.lightTable.set", json!({ "visible": true })).code, "arg.invalid", "items or apply all");
    let removed = ok(&h, "xsheet.lightTable.remove", json!({ "itemIds": [added[0]] }))["removedIds"].clone();
    assert_eq!(removed, json!([added[0]]));
    assert_eq!(entries(&h).len(), 2);
    ok(&h, "xsheet.lightTable.remove", json!({ "all": true }));
    assert!(items(&h).is_empty() && entries(&h).is_empty());
    // The drawings themselves stay (04 §15.3).
    assert_eq!(ok(&h, "element.drawing.list", json!({ "elementId": bob }))["drawings"].as_array().unwrap().len(), 3);
}

#[test]
fn sending_the_selected_cells_and_the_refusals() {
    let dir = tmp();
    let (h, _, bob, drawings, peg) = project(&dir);
    ok(&h, "cell.select", json!({ "elementIds": [bob], "from": 1, "to": 3 }));
    let added = ok(&h, "xsheet.lightTable.add", json!({}))["itemIds"].as_array().unwrap().len();
    assert_eq!(added, 3, "the three drawings of the selection");
    assert_eq!(
        items(&h).iter().map(|i| i["drawingId"].clone()).collect::<Vec<_>>(),
        drawings.iter().map(|d| json!(d)).collect::<Vec<_>>()
    );
    let e = err(&h, "xsheet.lightTable.add", json!({ "items": [{ "elementId": peg }] }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("cell.notApplicable", Some("/items/0/elementId")));
    let e = err(&h, "xsheet.lightTable.add", json!({ "items": [{ "elementId": bob, "frame": 9 }] }));
    assert_eq!(e.code, "drawing.noTarget");
    let e = err(
        &h,
        "xsheet.lightTable.add",
        json!({ "items": [{ "elementId": bob, "frame": 1, "drawingId": drawings[0] }] }),
    );
    assert_eq!(e.code, "arg.invalid");
    // An empty selection exposes nothing: the bare command is refused before it runs.
    ok(&h, "cell.select", json!({ "elementIds": [bob], "from": 9, "to": 9 }));
    assert_eq!(err(&h, "xsheet.lightTable.add", json!({})).code, "drawing.noTarget");
}

#[test]
fn the_light_table_and_column_widths_are_saved_with_the_scene_and_are_no_document_change() {
    let dir = tmp();
    let (h, target, bob, drawings, _) = project(&dir);
    let rev0 = rev(&h);
    ok(
        &h,
        "xsheet.lightTable.add",
        json!({ "items": [{ "elementId": bob, "frame": 1 }, { "elementId": bob, "frame": 3 }] }),
    );
    ok(&h, "xsheet.lightTable.set", json!({ "apply": "all", "placement": "over" }));
    ok(&h, "xsheet.columnWidth.set", json!({ "elementId": bob, "width": 150 }));
    assert_eq!(rev(&h), rev0, "no document revision");
    assert_eq!(ok(&h, "doc.state.get", json!({}))["dirty"], false, "not dirty");
    let before = items(&h);
    // Saved with the project's session state at its next save (a light-table change alone does not make the project
    // modified, as the playback ranges, SI-204), then read back on open.
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "edit": "with the light table" } } }));
    job_ok(&h, "project.save", json!({}));
    ok(&h, "project.close", json!({}));
    job_ok(&h, "project.open", json!({ "path": path(&target) }));
    assert_eq!(items(&h), before, "the light table is the scene's, saved with it");
    assert_eq!(
        ok(&h, "xsheet.columnWidth.set", json!({ "elementId": bob, "width": 150 }))["widths"],
        json!({ bob.clone(): 150 })
    );

    // An item whose drawing is gone is dropped (on the list, then on the next open).
    let gone = drawings[2].clone();
    ok(&h, "element.drawing.delete", json!({ "drawingIds": [gone], "ifInUse": "clearCells" }));
    assert_eq!(items(&h).len(), 1);
    job_ok(&h, "project.save", json!({}));
    ok(&h, "project.close", json!({}));
    job_ok(&h, "project.open", json!({ "path": path(&target) }));
    assert_eq!(items(&h).len(), 1);
    let saved = std::fs::read_to_string(target.join("session.json")).unwrap_or_default();
    assert!(!saved.contains(&gone), "the deleted drawing's item is not saved");
}

/// 11 §6.4 (line 241), ruling D-12: none of the seven changes focus, so an agent's call runs while the person has a
/// dialog open (only focus-changing `ui` commands answer `ui.modalOpen`), and waits while the person is mid-gesture,
/// then answers `ui.busy` with nothing changed.
#[test]
fn an_agents_call_runs_under_the_persons_dialog_and_waits_out_a_gesture() {
    use tt_executor::host::ShellReport;
    use tt_executor::{Invocation, InvokeOptions, Status};
    use tt_registry::Scope;
    let modal = |kind: &str| -> ShellReport {
        serde_json::from_str(&format!(r#"{{"modal":{{"kind":"{kind}","title":"Preferences"}}}}"#)).unwrap()
    };
    let inv = |command: &str, params: Value| Invocation {
        command: command.into(),
        params,
        options: InvokeOptions::default(),
        origin: agent(&[Scope::Read, Scope::Edit, Scope::Ui]),
    };
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let calls = [
        ("xsheet.display.set", json!({ "thumbnails": true })),
        ("xsheet.panel.set", json!({ "elementList": true })),
        ("xsheet.columnWidth.set", json!({ "elementId": bob, "width": 120 })),
        (
            "xsheet.lightTable.add",
            json!({ "items": [{ "elementId": bob, "frame": 1 }, { "elementId": bob, "frame": 1 }] }),
        ),
        ("xsheet.lightTable.move", json!({ "itemId": "lt2", "index": 0 })),
        ("xsheet.lightTable.set", json!({ "apply": "all", "placement": "over" })),
        ("xsheet.lightTable.remove", json!({ "all": true })),
    ];
    let h = fixture();
    h.set_wait_for_user(std::time::Duration::ZERO);
    for (command, params) in &calls {
        h.report_shell(modal("dialog"));
        let env = h.invoke(inv(command, params.clone()));
        assert_eq!(env.status, Status::Ok, "{command} under a dialog: {:?}", env.error);
        h.report_shell(modal("gesture"));
        let before = (ok(&h, "xsheet.lightTable.list", json!({})), ok(&h, "xsheet.exposures.get", json!({})));
        let env = h.invoke(inv(command, params.clone()));
        assert_eq!(env.error.map(|e| e.code), Some("ui.busy".to_owned()), "{command} under a gesture");
        let after = (ok(&h, "xsheet.lightTable.list", json!({})), ok(&h, "xsheet.exposures.get", json!({})));
        assert_eq!(before, after, "{command}: nothing changed under the gesture");
        h.report_shell(serde_json::from_str(r#"{"modal":null}"#).unwrap());
    }
}

/// Ruling E-70 (C52A-R1S-01; spec A line 504): one state, one set of events. Every effective change through either
/// light-table family raises both `drawAid.changed` and `xsheet.lightTable.changed`, the latter with the items changed.
#[test]
fn every_light_table_change_raises_both_families_events() {
    let h = fixture();
    let bob = "scene:\"Intro\"/element:\"Bob\"";
    let run_and_events = |command: &str, params: Value| -> (Value, Vec<tt_executor::Event>) {
        let rx = h.subscribe();
        let r = ok(&h, command, params);
        (r, rx.try_iter().collect())
    };
    let check = |label: &str, events: &[tt_executor::Event], ids: &[String]| {
        assert!(events.iter().any(|e| e.name == "drawAid.changed"), "{label}: no drawAid.changed");
        let sheet = events.iter().find(|e| e.name == "xsheet.lightTable.changed");
        let listed: Vec<String> =
            sheet.unwrap_or_else(|| panic!("{label}: no xsheet.lightTable.changed")).payload["itemIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
        for id in ids {
            assert!(listed.contains(id), "{label}: {id} not in {listed:?}");
        }
    };
    let ids = |v: &Value, key: &str| -> Vec<String> {
        v[key].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect()
    };
    // The drawing-aid family.
    let (r, ev) = run_and_events("drawAid.staticLightTable.add", json!({ "elementId": bob, "frame": 1 }));
    let a = r["entryId"].as_str().unwrap().to_owned();
    check("drawAid add", &ev, std::slice::from_ref(&a));
    let (_, ev) = run_and_events("drawAid.staticLightTable.set", json!({ "entryIds": [a], "visible": false }));
    check("drawAid set", &ev, std::slice::from_ref(&a));
    // The sheet's family.
    let (r, ev) = run_and_events("xsheet.lightTable.add", json!({ "items": [{ "elementId": bob, "frame": 3 }] }));
    let added = ids(&r, "itemIds");
    check("xsheet add", &ev, &added);
    let (_, ev) = run_and_events("xsheet.lightTable.move", json!({ "itemId": added[0], "index": 0 }));
    check("xsheet move", &ev, &added);
    let (_, ev) =
        run_and_events("xsheet.lightTable.set", json!({ "itemIds": [a], "apply": "given", "placement": "over" }));
    check("xsheet set", &ev, std::slice::from_ref(&a));
    let (_, ev) = run_and_events("xsheet.lightTable.remove", json!({ "itemIds": [added[0]] }));
    check("xsheet remove", &ev, &added);
    let (_, ev) = run_and_events("drawAid.staticLightTable.remove", json!({ "entryIds": [a] }));
    check("drawAid remove", &ev, std::slice::from_ref(&a));
    // No effective change, no event of either family.
    let (_, ev) = run_and_events("xsheet.lightTable.remove", json!({ "all": true }));
    assert!(!ev.iter().any(|e| e.name == "drawAid.changed" || e.name == "xsheet.lightTable.changed"), "{ev:?}");
}
