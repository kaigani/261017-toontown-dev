//! Independent dynamic gate fixtures for D-25 and D-31.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    dead_code,
    unused_imports,
    reason = "review fixtures"
)]
mod common;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tt_executor::host::KeyResolution;
use tt_executor::{ExecutorHandle, Origin};
use tt_registry::{Kind, Status, validate_params};

use common::*;

const MANIFEST: &str = include_str!("../../../desktop/src/ui-bindings/parity.json");
const DEFERRED: &str = include_str!("../../../desktop/src/ui-bindings/deferred.json");
const GESTURES: &str = include_str!("../src/gesture-table.json");
const TOOLBAR: &str = include_str!("../src/toolbar-default.json");

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
/// collects them with a dialog (01 ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§0), so the parameters given are checked against the schema without `required`.
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
fn case1_gate_negative_matrix() {
    let m = manifest();
    let d: Value = serde_json::from_str(DEFERRED).unwrap();
    assert!(pa2(&m).is_empty());
    assert!(pa3(&m, &d).0.is_empty());
    let id = d["deferred"].as_object().unwrap().keys().next().unwrap().clone();
    let mut accepted = Vec::new();
    for owner in ["M4 C4.3", "M4 C4.2", "nonsense", "M5 C5.1.trailing", "5 C5.1"] {
        let mut bad = d.clone();
        bad["deferred"][&id]["owner"] = json!(owner);
        let errors = pa3(&m, &bad).0;
        eprintln!("owner={owner:?} errors={errors:?}");
        if errors.is_empty() {
            accepted.push(owner);
        }
    }
    assert!(accepted.is_empty(), "D-31 malformed/passed owners accepted: {accepted:?}");
}
#[test]
fn case1_duplicate_deferred_entry_rejected() {
    let m = manifest();
    let d: Value = serde_json::from_str(DEFERRED).unwrap();
    let (id, value) = d["deferred"].as_object().unwrap().iter().next().unwrap();
    let mut raw = DEFERRED.to_owned();
    let at = raw.find('{').unwrap() + 1;
    // Duplicate a command inside the deferred object, using valid raw JSON.
    let at = raw[at..].find('{').unwrap() + at + 1;
    raw.insert_str(at, &format!("{}:{},", serde_json::to_string(id).unwrap(), value));
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert!(!pa3(&m, &parsed).0.is_empty(), "D-31 exact gate accepts duplicate deferred command {id}");
}

#[test]
fn case1_other_negative_fixtures() {
    let m = manifest();
    let d: Value = serde_json::from_str(DEFERRED).unwrap();
    for (id, params) in
        [("review.missing", json!({})), ("job.finished", json!({})), ("draw.flip.apply", json!({"axis":"diagonal"}))]
    {
        let mut bad = m.clone();
        bad["menu"].as_array_mut().unwrap().push(json!({"id":"review","commands":[id],"params":params}));
        assert!(!pa2(&bad).is_empty(), "PA2 accepted {id}");
    }
    let id = d["deferred"].as_object().unwrap().keys().next().unwrap().clone();
    for mode in ["missing", "bound-deferred", "agent-menu", "unknown", "empty-section"] {
        let (mut bm, mut bd) = (m.clone(), d.clone());
        match mode {
            "missing" => {
                bd["deferred"].as_object_mut().unwrap().remove(&id);
            }
            "bound-deferred" => {
                bd["deferred"]["edit.undo"] = json!({"owner":"M4 C4.4","section":"01"});
            }
            "agent-menu" => {
                bm["menu"].as_array_mut().unwrap().push(json!({"id":"review","commands":["keymap.get"]}));
            }
            "unknown" => {
                bd["deferred"]["review.missing"] = json!({"owner":"M4 C4.4","section":"01"});
            }
            _ => {
                bd["deferred"][&id]["section"] = json!("");
            }
        }
        let errors = pa3(&bm, &bd).0;
        eprintln!("fixture={mode} errors={errors:?}");
        assert!(!errors.is_empty(), "PA3 accepted {mode}");
    }
}

// spec/11_agent_interface_mcp.md section 14 E6:586-587; busy note section 5 E1; SI-157.
#[test]
fn case2_mutation_waits_for_commit_cancel_and_refusal() {
    use std::{sync::mpsc, thread, time::Duration};
    for ending in ["commit", "cancel", "refuse"] {
        let h = fixture();
        ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
        h.set_wait_for_user(Duration::from_secs(10));
        h.report_shell(serde_json::from_value(json!({"modal":{"kind":"gesture","title":"brush"}})).unwrap());
        let (tx, rx) = mpsc::channel();
        let hh = h.clone();
        let pending = thread::spawn(move || {
            let env = hh.invoke(tt_executor::Invocation {
                command: "draw.rectangle.add".into(),
                params: json!({"from":{"x":200.,"y":200.},"to":{"x":230.,"y":230.}}),
                options: Default::default(),
                origin: common::agent(&tt_registry::Scope::ALL),
            });
            tx.send(env).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "agent answered during {ending}");
        assert!(run(&h, "doc.state.get", json!({})).is_ok(), "reads are allowed while held");
        let before = rev(&h);
        if ending != "cancel" {
            let request = if ending == "commit" {
                json!({"view":"drawingView","tool":"brush","events":[{"type":"down","x":700.,"y":300.},{"type":"up","x":760.,"y":330.}]})
            } else {
                json!({"view":"drawingView","tool":"notATool","events":[]})
            };
            let env = h.pointer_in_window(Origin::User, request);
            assert_eq!(env.is_ok(), ending == "commit", "{env:?}");
        }
        let committed = rev(&h);
        assert_eq!(committed > before, ending == "commit");
        h.report_shell(serde_json::from_value(json!({"modal":null})).unwrap());
        let env = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        assert!(env.is_ok(), "agent lost after {ending}: {env:?}");
        pending.join().unwrap();
        assert!(rev(&h) > committed, "agent is after {ending}");
    }
}

// Preview note section 3/PV3: whole-object filtering must partition geometry.
#[test]
fn case2_selection_parts_empty_whole_group_text_and_generation() {
    use tt_executor::canvas::{Part, PayloadOptions};
    use tt_render::canvas::{Record, decode};
    let h = fixture();
    ok(&h, "app.context.set", json!({"elementId":"scene:#1/element:\"Bob\"","frame":1}));
    let mut ids = Vec::new();
    for x in [-380., 380.] {
        let made =
            ok(&h, "draw.rectangle.add", json!({"from":{"x":x,"y":-250.},"to":{"x":x+60.,"y":-200.},"layer":"new"}));
        ids.push(made["strokeIds"][0].clone());
    }
    let group = ok(&h, "draw.group.create", json!({"ids":ids}))["groupId"].clone();
    let text = ok(&h, "draw.text.add", json!({"position":{"x":0.,"y":-400.},"text":"Review"}))["textId"].clone();
    let triangles = |part, generation| {
        let d = decode(
            &h.canvas_drawing(None, generation, 0, PayloadOptions { part, ..Default::default() }).unwrap().bytes,
        )
        .unwrap();
        assert_eq!(d.identity.generation, generation);
        let mut points = Vec::new();
        for r in d.records {
            match r {
                Record::Solid { vertices, indices } | Record::Gradient { vertices, indices, .. } => {
                    for i in indices {
                        let v = vertices[i as usize];
                        points.push((v.x, v.y));
                    }
                }
                Record::Bitmap { quad, .. } => {
                    for p in quad.as_chunks::<2>().0 {
                        points.push((p[0], p[1]));
                    }
                }
            }
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        points
    };
    for selected in
        [json!({"by":"none"}), json!({"by":"all"}), json!({"by":"ids","ids":[group]}), json!({"by":"ids","ids":[text]})]
    {
        let selection = ok(&h, "draw.selection.set", selected.clone());
        eprintln!("selection answer={selection}");
        if selected["by"] == "ids" {
            assert!(selection["bounds"].is_object(), "fixture must select visible nonempty geometry: {selection}");
        }
        for generation in [1, 7] {
            let all = triangles(Part::All, generation);
            let mut split = triangles(Part::Selection, generation);
            let selected_count = split.len();
            split.extend(triangles(Part::Rest, generation));
            split.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            assert_eq!(all.len(), split.len(), "{selected}");
            for (a, b) in all.iter().zip(&split) {
                assert!((a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4, "{selected}: {a:?} {b:?}");
            }
            if selected["by"] == "none" {
                assert_eq!(selected_count, 0);
            }
            if selected["by"] == "all" {
                assert_eq!(selected_count, all.len());
            }
            if selected["by"] == "ids" {
                assert!(selected_count > 0, "PV3 selected nonempty group/text has no geometry: {selected}");
            }
            eprintln!("selection={selected} generation={generation} vertices={} selected={selected_count}", all.len());
        }
    }
}

// SI-148: Camera scene ids choose anim.tool.set; Drawing always chooses drawing tools.
#[test]
fn case2_tool_choice_table() {
    let h = fixture();
    let scene = [
        "select",
        "transform",
        "record",
        "rotate",
        "skew",
        "scale",
        "motion",
        "boneCreate",
        "boneSetup",
        "boneManipulate",
        "boneInverse",
        "boneForward",
        "boneInfluence",
        "boneParent",
    ];
    let drawing = [
        "select",
        "contourEditor",
        "perspective",
        "repositionAll",
        "brush",
        "pencil",
        "rectangle",
        "ellipse",
        "polyline",
        "line",
        "paint",
        "unpaint",
        "paintUnpainted",
        "dropper",
        "closeGap",
        "stroke",
        "editTexture",
        "eraser",
        "cutter",
        "scissor",
        "zoom",
        "grabber",
        "text",
    ];
    let tools: BTreeSet<_> = scene.iter().chain(&drawing).copied().chain(["invalidReviewTool"]).collect();
    for view in ["cameraView", "drawingView", "camera", "drawing"] {
        for tool in &tools {
            let expected = if view.starts_with("camera") && scene.contains(tool) {
                ("anim.tool.set", json!({"tool":tool}))
            } else {
                ("tool.active.set", json!({"toolId":tool}))
            };
            assert_eq!(
                h.gesture_tool_choice(view, tool).unwrap(),
                (expected.0.to_owned(), expected.1),
                "SI-148 {view} {tool}"
            );
        }
    }
}
