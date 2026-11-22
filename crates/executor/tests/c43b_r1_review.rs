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

// Ruling D-37 (C43B-R1-01, -02): the gate is the product's shared one, not a copy.
use common::parity_gate::{manifest, pa2, pa3, pa3_raw};

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
    assert!(!pa3_raw(&m, &raw).0.is_empty(), "D-31 exact gate accepts duplicate deferred command {id}");
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
