//! Independent round-2 data and mixed-operation probes. Finding reproductions deliberately remain failing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorHandle, InvokeOptions, Status};
const B: &str = "scene:\"Intro\"/element:\"Bob\"";
const N: &str = "scene:\"Intro\"/element:\"Notes\"";
const P: &str = "scene:\"Intro\"/element:\"Bob-P\"";
const D: &str = "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"";
const BG: &str = "scene:\"Intro\"/element:\"Background\"";
fn seeded() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(0x0031_2007).unwrap(), None, false).is_ok());
    h
}
fn notes(h: &ExecutorHandle) -> Value {
    get(h, B)["cellNotes"].clone()
}
fn names(h: &ExecutorHandle, e: &str) -> Vec<String> {
    get(h, e)["exposure"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| {
            let name = r[0].as_str().map(|d| get(h, d)["name"].as_str().unwrap().to_owned()).unwrap_or("_".into());
            vec![name; r[1].as_u64().unwrap() as usize]
        })
        .collect()
}
fn scrub(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("createdAt");
    }
    v
}

#[test]
fn r02_solo_additive_dry_run_and_failed_batch_preserve_existing_session() {
    let h = seeded();
    ok(&h, "element.solo.set", json!({"enabled":true,"elementIds":[N]}));
    let before = ok(&h, "app.state.get", json!({}));
    let colours = ok(&h, "element.typeColour.get", json!({}));
    for enabled in [false, true] {
        let env = with(
            &h,
            "element.solo.set",
            json!({"enabled":enabled,"elementIds":[B,P]}),
            InvokeOptions { dry_run: true, ..Default::default() },
        );
        assert_eq!(env.status, Status::Ok);
        assert_eq!(ok(&h, "app.state.get", json!({})), before);
    }
    let e = batch(
        &h,
        json!({"label":"Round two rollback","atomic":true,"steps":[
            {"id":"element.solo.set","params":{"enabled":false}},
            {"id":"element.typeColour.set","params":{"category":"skeletons","colour":"#7193a1"}},
            {"id":"cell.note.set","params":{"elementId":B,"frame":0,"text":"refuse"}}
        ]}),
    );
    assert_eq!(e.status, Status::Error);
    assert_eq!(ok(&h, "app.state.get", json!({})), before);
    assert_eq!(ok(&h, "element.typeColour.get", json!({})), colours);
    assert_eq!(ok(&h, "xsheet.lightTable.list", json!({}))["items"], json!([]));
}

#[test]
fn r03_all_delegated_families_match_objects_selection_and_events_in_both_forms() {
    for (target, property, value, command, params) in [
        (B, "name", json!("Round Two"), "element.rename", json!({"elementId":B,"name":"Round Two"})),
        (B, "note", json!("new variant 731"), "element.note.set", json!({"elementId":B,"text":"new variant 731"})),
        (B, "markerColour", json!("#7153a2"), "element.colour.set", json!({"elementIds":[B],"marker":"#7153a2"})),
        (
            B,
            "backgroundColour",
            json!("#153272"),
            "element.colour.set",
            json!({"elementIds":[B],"background":"#153272"}),
        ),
        (
            B,
            "visibleInXsheet",
            json!(false),
            "element.visibility.set",
            json!({"elementIds":[B],"scope":"xsheet","visible":false}),
        ),
        (
            B,
            "visibleInScene",
            json!(false),
            "element.visibility.set",
            json!({"elementIds":[B],"scope":"scene","visible":false}),
        ),
        (B, "locked", json!(true), "element.lock.set", json!({"elementIds":[B],"locked":true})),
        (D, "name", json!("Variant731"), "element.drawing.rename", json!({"drawingId":D,"name":"Variant731"})),
        (D, "locked", json!(true), "element.drawingLock.set", json!({"drawingIds":[D],"locked":true})),
    ] {
        for ops in [false, true] {
            let a = seeded();
            let b = seeded();
            for h in [&a, &b] {
                ok(h, "element.current.set", json!({"elementId":B}));
            }
            let key = get(&a, target)["id"].as_str().unwrap().to_owned();
            let ra = a.subscribe();
            let rb = b.subscribe();
            ok(&a, command, params.clone());
            let patch = if ops {
                json!({"targets":[target],"ops":[{"op":"replace","path":format!("/{property}"),"value":value}]})
            } else {
                json!({"targets":[target],"props":{property:value}})
            };
            ok(&b, "doc.object.patch", patch);
            assert_eq!(scrub(get(&a, &key)), scrub(get(&b, &key)), "{property}, ops={ops}");
            assert_eq!(ok(&a, "cell.selection.get", json!({})), ok(&b, "cell.selection.get", json!({})));
            let events = |rx: std::sync::mpsc::Receiver<tt_executor::Event>| -> Vec<Value> {
                rx.try_iter()
                    .filter(|e| e.name.starts_with("element.") || e.name == "cell.selection.changed")
                    .map(|e| json!({"name":e.name,"payload":e.payload}))
                    .collect()
            };
            assert_eq!(events(ra), events(rb), "{property}, ops={ops}");
        }
    }
}

#[test]
fn r03_rfc_test_must_observe_the_preceding_delegated_write() {
    let h = seeded();
    ok(&h, "element.note.set", json!({"elementId":B,"text":"before731"}));
    let before = get(&h, B);
    let revision = rev(&h);
    let env = run(
        &h,
        "doc.object.patch",
        json!({"targets":[B],"ops":[
            {"op":"test","path":"/note","value":"before731"},
            {"op":"replace","path":"/note","value":"after731"}
        ]}),
    );
    assert_eq!(env.status, Status::Ok, "RFC 6902 executes test before replace: {env:?}");
    assert_eq!(get(&h, B)["note"], "after731");
    ok(&h, "edit.undo", json!({}));
    assert_eq!(get(&h, B)["note"], before["note"]);
    assert!(rev(&h) > revision);
}

#[test]
fn r03_rfc_copy_to_visibility_has_the_owning_command_consequence() {
    let h = seeded();
    ok(&h, "element.current.set", json!({"elementId":B}));
    ok(&h, "element.visibility.set", json!({"elementIds":[B],"scope":"scene","visible":false}));
    let original = get(&h, B)["id"].clone();
    let rx = h.subscribe();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[B],"ops":[{"op":"copy","from":"/visibleInScene","path":"/visibleInXsheet"}]}),
    );
    assert_eq!(get(&h, B)["visibleInXsheet"], false);
    assert_ne!(ok(&h, "cell.selection.get", json!({}))["currentElementId"], original, "AG-02 also applies to RFC copy");
    assert_eq!(rx.try_iter().filter(|e| e.name == "element.state.changed").count(), 1);
}

#[test]
fn r03_bank_events_announced_once_to_each_clone_user_and_bank() {
    let h = seeded();
    let bc = ok(&h, "element.clone", json!({"elementId":B}))["elementId"].clone();
    let nc = ok(&h, "element.clone", json!({"elementId":N}))["elementId"].clone();
    let rx = h.subscribe();
    ok(&h, "cell.range.add", json!({"elementIds":[B,N],"frame":2,"count":3,"named":true,"position":"before"}));
    let events: Vec<_> = rx.try_iter().filter(|e| e.name == "element.drawing.changed").collect();
    assert_eq!(events.len(), 4);
    for e in [json!(B), json!(N), bc, nc] {
        let object = get(&h, e.as_str().unwrap());
        let bank = ok(&h, "element.drawing.list", json!({"elementId":e}))["drawings"].as_array().unwrap().clone();
        let matching: Vec<_> = events.iter().filter(|v| v.payload["elementId"] == object["id"]).collect();
        assert_eq!(matching.len(), 1);
        let drawings = matching[0].payload["drawingIds"].as_array().unwrap();
        assert_eq!(drawings.len(), 3);
        for d in drawings {
            assert!(bank.iter().any(|b| b["drawingId"] == *d || b["id"] == *d));
        }
    }
}

#[test]
fn r04_hundred_drawing_copy_survives_delete_close_and_open_another_project() {
    for generic in [false, true] {
        let h = seeded();
        let cols: Vec<Value> = (1..=100).map(|i| json!({"drawing":format!("R2{i:03}"),"from":i,"to":i})).collect();
        ok(
            &h,
            "xsheet.exposures.set",
            json!({"createMissingDrawings":true,"columns":[{"elementId":B,"from":1,"to":100,"exposures":cols}]}),
        );
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":100,"text":"last731"}));
        let d = get(&h, B)["exposure"][0][0].as_str().unwrap().to_owned();
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[d],"props":{"ext":{"review.round2":{"opaque":[731,17]}},"meta":{"variant":"731"}}}),
        );
        let copied = get(&h, &d);
        if generic {
            ok(&h, "cell.select", json!({"elementIds":[B],"from":1,"to":100}));
            ok(&h, "edit.copy", json!({"context":"xsheet"}));
        } else {
            ok(&h, "cell.clipboard.set", json!({"elementIds":[B],"from":1,"to":100}));
        }
        ok(&h, "element.delete", json!({"elementIds":[B]}));
        ok(&h, "project.close", json!({"onUnsaved":"discard"}));
        assert!(h.open_document(tt_model::fixture::sample_document(0x0031_2019).unwrap(), None, false).is_ok());
        if generic {
            ok(&h, "edit.paste", json!({"context":"xsheet","destination":{"elementId":N,"frame":30}}));
        } else {
            ok(&h, "cell.clipboard.apply", json!({"elementId":N,"frame":30}));
        }
        let e = get(&h, N);
        let mut f = 1;
        let mut first = None;
        for r in e["exposure"].as_array().unwrap() {
            let len = r[1].as_u64().unwrap();
            if f <= 30 && f + len > 30 {
                first = r[0].as_str().map(str::to_owned);
            }
            f += len;
        }
        let pasted = get(&h, &first.unwrap());
        assert_ne!(pasted["id"], copied["id"]);
        assert_eq!(pasted["ext"], copied["ext"]);
        assert_eq!(e["cellNotes"]["129"], "last731");
        assert_eq!(names(&h, N)[29..129], (1..=100).map(|i| format!("R2{i:03}")).collect::<Vec<_>>());
    }
}

#[test]
fn r05_notes_at_both_ends_matrix_and_undo_redo() {
    let cases = [
        ("cell.exposure.add", json!({"elementIds":[B],"from":1,"to":1,"frames":3}), 12, 15),
        ("cell.exposure.remove", json!({"elementIds":[B],"from":1,"to":1,"frames":2}), 7, 10),
        ("cell.exposure.set", json!({"elementIds":[B],"from":1,"to":1,"length":3}), 6, 9),
        ("cell.blank.add", json!({"elementIds":[B],"frame":1,"count":3}), 12, 15),
        ("cell.blank.remove", json!({"elementIds":[B],"frame":7,"count":2}), 7, 10),
        ("cell.range.add", json!({"elementIds":[B],"frame":3,"position":"before","count":4}), 13, 16),
        ("cell.range.delete", json!({"elementIds":[B],"from":2,"to":4,"shift":true}), 6, 9),
        ("cell.cycle.create", json!({"elementId":B,"from":1,"to":2,"cycles":3}), 13, 16),
        (
            "cell.columnate.apply",
            json!({"elementId":B,"frame":1,"firstDrawing":"1","lastDrawing":"2","exposeEach":2,"placement":"insert"}),
            13,
            16,
        ),
        ("cell.range.move", json!({"sourceElementIds":[B],"from":1,"to":3,"targetFrame":30,"mode":"move"}), 6, 9),
        ("cell.clipboard.set", json!({"elementIds":[B],"from":1,"to":3,"cut":true}), 6, 9),
        ("cell.clipboard.apply", json!({"elementId":B,"frame":2}), 12, 15),
    ];
    for (cmd, p, a, b) in cases {
        let h = seeded();
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":9,"text":"left731"}));
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":12,"text":"right731"}));
        ok(&h, "cell.clipboard.set", json!({"elementIds":[B],"from":1,"to":3}));
        let before = notes(&h);
        ok(&h, cmd, p);
        let after = notes(&h);
        assert_eq!(after[a.to_string()], "left731", "{cmd}: {after}");
        assert_eq!(after[b.to_string()], "right731", "{cmd}: {after}");
        ok(&h, "edit.undo", json!({}));
        assert_eq!(notes(&h), before, "undo {cmd}");
        ok(&h, "edit.redo", json!({}));
        assert_eq!(notes(&h), after, "redo {cmd}");
    }
}

#[test]
fn r05_overlap_move_carries_end_notes_and_clears_overwritten_notes() {
    let h = seeded();
    for f in [3, 12] {
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":f,"text":""}));
    }
    for (f, t) in [(2, "start731"), (4, "end731"), (5, "gone731")] {
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":f,"text":t}));
    }
    ok(
        &h,
        "cell.range.move",
        json!({"sourceElementIds":[B],"from":2,"to":4,"targetFrame":3,"mode":"move","options":{"exposure":false}}),
    );
    assert_eq!(notes(&h), json!({"3":"start731","5":"end731"}));
}

#[test]
fn r06_nested_clip_skeleton_particle_and_user_children_are_independent_subtrees() {
    let h = seeded();
    let outer = ok(&h, "element.create", json!({"type":"clipping","name":"Outer731"}))["elementIds"][0]
        .as_str()
        .unwrap()
        .to_owned();
    for child in
        ["scene:\"Intro\"/element:\"Rig\"", "scene:\"Intro\"/element:\"Snow\"", P, "scene:\"Intro\"/element:\"Clip\""]
    {
        ok(&h, "element.parent.set", json!({"elementIds":[child],"parentId":outer}));
    }
    let source =
        ok(&h, "element.list", json!({"parentId":outer,"recursive":true}))["elements"].as_array().unwrap().clone();
    let copied =
        ok(&h, "element.duplicate", json!({"elementIds":[outer]}))["elementIds"][0].as_str().unwrap().to_owned();
    let target =
        ok(&h, "element.list", json!({"parentId":copied,"recursive":true}))["elements"].as_array().unwrap().clone();
    assert_eq!(source.len(), target.len());
    assert!(source.len() > 10);
    let originals: Vec<String> =
        source.iter().map(|s| s["id"].as_str().unwrap().to_owned()).chain([outer.clone()]).collect();
    fn check_refs(v: &Value, originals: &[String]) {
        match v {
            Value::String(s) => assert!(!originals.contains(s), "reference still names source subtree: {s}"),
            Value::Array(a) => {
                for x in a {
                    check_refs(x, originals);
                }
            }
            Value::Object(o) => {
                for (k, x) in o {
                    if k != "ext" && k != "meta" {
                        check_refs(x, originals);
                    }
                }
            }
            _ => {}
        }
    }
    for (s, t) in source.iter().zip(target.iter()) {
        assert_ne!(s["id"], t["id"]);
        assert_eq!(s["elementType"], t["elementType"]);
        let a = get(&h, s["id"].as_str().unwrap());
        let b = get(&h, t["id"].as_str().unwrap());
        check_refs(&b, &originals);
        if a["drawingBankId"].is_string() {
            assert_ne!(a["drawingBankId"], b["drawingBankId"]);
        }
    }
}

#[test]
fn r07_three_way_rotation_snapshot_all_modes_shared_and_distinct_banks() {
    for shared in [false, true] {
        for mode in ["copy", "copyNew", "move", "link"] {
            for exposure in [false, true] {
                if mode == "link" && !shared {
                    continue;
                }
                let h = seeded();
                let a = ok(&h, "element.create", json!({"type":"drawing","name":"Rot731"}))["elementIds"][0].clone();
                let mut cols = vec![a.clone()];
                for i in 1..3 {
                    cols.push(if shared {
                        ok(&h, "element.clone", json!({"elementId":a}))["elementId"].clone()
                    } else {
                        ok(&h, "element.create", json!({"type":"drawing","name":format!("Rot731{i}")}))["elementIds"][0]
                            .clone()
                    });
                }
                let all = ok(&h, "element.list", json!({"sceneId":"scene:\"Intro\"","order":"xsheet"}))["elements"]
                    .as_array()
                    .unwrap()
                    .clone();
                cols.sort_by_key(|c| all.iter().position(|e| e["id"] == *c).unwrap());
                for (i, e) in cols.iter().enumerate() {
                    let r = ok(
                        &h,
                        "cell.label.set",
                        json!({"elementIds":[e],"from":1,"to":2,"drawing":format!("Seed{i}"),"createMissing":true}),
                    );
                    ok(
                        &h,
                        "doc.object.patch",
                        json!({"targets":[r["drawingId"]],"props":{"ext":{"review.round2":{"rotationSource":i}}}}),
                    );
                }
                let before: Vec<Value> = cols.iter().map(|e| get(&h, e.as_str().unwrap())).collect();
                let src = vec![cols[2].clone(), cols[0].clone(), cols[1].clone()];
                ok(
                    &h,
                    "cell.range.move",
                    json!({"sourceElementIds":src,"from":1,"to":2,"targetElementId":cols[0],"targetFrame":1,"mode":mode,"options":{"exposure":exposure}}),
                );
                for (i, e) in cols.iter().enumerate() {
                    let d = get(&h, e.as_str().unwrap())["exposure"][0][0].as_str().unwrap().to_owned();
                    let old = before[(i + 2) % 3]["exposure"][0][0].as_str().unwrap();
                    assert_eq!(
                        get(&h, &d)["nodes"],
                        get(&h, old)["nodes"],
                        "{mode} shared={shared} exposure={exposure}"
                    );
                    assert_eq!(get(&h, &d)["ext"]["review.round2"]["rotationSource"], json!((i + 2) % 3));
                    if mode != "copyNew" {
                        assert_eq!(get(&h, &d)["name"], format!("Seed{}", (i + 2) % 3));
                    }
                }
            }
        }
    }
}

#[test]
fn r08_nfc_names_three_clones_and_suffix_ten() {
    let h = seeded();
    let mut cols = vec![json!(B)];
    for _ in 0..2 {
        cols.push(ok(&h, "element.clone", json!({"elementId":B}))["elementId"].clone());
    }
    let spellings = [" A\u{30a}ngstro\u{308}m ", "Ångström", "Ångstro\u{308}m"];
    let columns: Vec<Value> = cols
        .iter()
        .zip(spellings)
        .map(|(e, s)| json!({"elementId":e,"from":17,"to":18,"exposures":[{"drawing":s,"from":17,"to":18}]}))
        .collect();
    assert_eq!(
        ok(&h, "xsheet.exposures.set", json!({"columns":columns,"createMissingDrawings":true}))["createdDrawings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let base = "海".repeat(64);
    let src = ok(&h, "element.create", json!({"type":"drawing","name":base}))["elementIds"][0].clone();
    for i in 1..=9 {
        ok(&h, "element.create", json!({"type":"drawing","name":format!("{}_{}","海".repeat(62),i)}));
    }
    let c = ok(&h, "element.clone", json!({"elementId":src}));
    assert_eq!(c["name"], format!("{}_10", "海".repeat(61)));
}

#[test]
fn r08_max_length_effect_name_keeps_required_derived_rows_valid() {
    for kind in ["clipping", "skeleton", "particleNature"] {
        let h = seeded();
        let name = "界".repeat(64);
        let result = run(&h, "element.create", json!({"type":kind,"name":name}));
        assert_eq!(result.status, Status::Ok, "valid 64-character {kind} requires valid parameter names: {result:?}");
    }
}

#[test]
fn r08_generic_new_drawing_copy_name_fits_64_characters() {
    let h = seeded();
    let name = "描".repeat(64);
    ok(&h, "element.drawing.create", json!({"elementId":B,"name":name,"exposeFrom":41}));
    ok(&h, "element.drawing.create", json!({"elementId":N,"name":name,"exposeFrom":41}));
    ok(&h, "cell.clipboard.set", json!({"elementIds":[B],"from":41,"to":41}));
    let env = run(
        &h,
        "edit.pasteSpecial",
        json!({"context":"xsheet","destination":{"elementId":N,"frame":50},"options":{"overwriteDrawings":false}}),
    );
    assert_eq!(env.status, Status::Ok, "copy needs a legal collision suffix: {env:?}");
}

#[test]
fn r09_frame_position_boundaries() {
    let h = seeded();
    let cases = [
        ("cell.note.set", json!({"elementId":B,"frame":100001,"text":"x"})),
        ("cell.select", json!({"elementIds":[B],"from":0,"to":1})),
        ("cell.range.move", json!({"sourceElementIds":[B],"from":1,"to":1,"targetFrame":100001})),
        ("element.drawing.create", json!({"elementId":B,"exposeFrom":100001})),
        ("element.drawing.create", json!({"elementId":B,"exposeFrom":1,"exposeTo":100001})),
        ("xsheet.exposures.get", json!({"elementIds":[B],"from":1,"to":100001})),
        ("cell.fill.apply", json!({"elementIds":[B],"frame":1,"toFrame":10000})),
        ("element.childExposure.apply", json!({"pegId":P,"toFrame":10000})),
    ];
    for (cmd, p) in cases {
        let revision = rev(&h);
        let e = run(&h, cmd, p);
        assert_eq!(e.status, Status::Error, "{cmd}: {e:?}");
        assert_eq!(rev(&h), revision);
    }
    ok(&h, "cell.note.set", json!({"elementId":B,"frame":100000,"text":"edge731"}));
    ok(&h, "cell.fill.apply", json!({"elementIds":[B],"frame":1,"toFrame":9999}));
    ok(&h, "element.childExposure.apply", json!({"pegId":P,"toFrame":9999}));
}

#[test]
fn r13_generic_overwrite_clone_group_preserves_full_drawing_and_locked_refusal() {
    let h = seeded();
    let clone = ok(&h, "element.clone", json!({"elementId":N}))["elementId"].clone();
    let target = ok(&h, "element.drawing.create", json!({"elementId":N,"name":"1","exposeFrom":13}))["drawingId"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(&h, "doc.object.patch", json!({"targets":[D],"props":{"ext":{"review.round2":{"v":[731]}},"meta":{"r2":true}}}));
    ok(&h, "cell.clipboard.set", json!({"elementIds":[B],"from":1,"to":1}));
    ok(&h, "element.drawingLock.set", json!({"drawingIds":[target],"locked":true}));
    let before = get(&h, &target);
    let revision = rev(&h);
    assert_eq!(
        err(&h, "edit.paste", json!({"context":"xsheet","destination":{"elementId":clone,"frame":13}})).code,
        "drawing.locked"
    );
    assert_eq!(get(&h, &target), before);
    assert_eq!(rev(&h), revision);
    ok(&h, "element.drawingLock.set", json!({"drawingIds":[target],"locked":false}));
    ok(&h, "edit.paste", json!({"context":"xsheet","destination":{"elementId":clone,"frame":13}}));
    let a = get(&h, D);
    let b = get(&h, &target);
    for k in ["nodes", "colourRecovery", "ext", "meta", "pivot", "mouthShape"] {
        assert_eq!(a[k], b[k], "{k}");
    }
    assert_eq!(
        ok(&h, "draw.drawing.get", json!({"drawingId":D}))["nodes"],
        ok(&h, "draw.drawing.get", json!({"drawingId":target}))["nodes"]
    );
}

#[test]
fn r15_three_selected_columns_different_lengths_one_undo() {
    for cmd in ["cell.cycle.create", "cell.columnate.apply", "cell.drawing.duplicate"] {
        let h = seeded();
        let cols = ok(&h, "element.create", json!({"type":"drawing","name":"Tri731","count":3}))["elementIds"]
            .as_array()
            .unwrap()
            .clone();
        for (i, e) in cols.iter().enumerate() {
            ok(
                &h,
                "cell.label.set",
                json!({"elementIds":[e],"from":1,"to":(i+1)*3,"drawing":"K731","createMissing":true}),
            );
        }
        ok(&h, "cell.select", json!({"elementIds":cols,"from":1,"to":2}));
        let before: Vec<Value> = cols.iter().map(|e| get(&h, e.as_str().unwrap())["exposure"].clone()).collect();
        let params = match cmd {
            "cell.cycle.create" => json!({"cycles":3}),
            "cell.columnate.apply" => {
                json!({"firstDrawing":"K731","lastDrawing":"K731","exposeEach":4,"exposeLast":4,"placement":"insert"})
            }
            _ => json!({}),
        };
        ok(&h, cmd, params);
        for (e, original) in cols.iter().zip(&before) {
            assert_ne!(&get(&h, e.as_str().unwrap())["exposure"], original, "{cmd}");
        }
        ok(&h, "edit.undo", json!({}));
        for (e, original) in cols.iter().zip(&before) {
            assert_eq!(&get(&h, e.as_str().unwrap())["exposure"], original, "one undo {cmd}");
        }
    }
}

#[test]
fn r16_image_lock_patch_ops_and_props_no_change_or_event() {
    for ops in [false, true] {
        let h = seeded();
        let image = get(&h, BG)["exposure"][0][0].clone();
        let revision = rev(&h);
        let rx = h.subscribe();
        let p = if ops {
            json!({"targets":[image],"ops":[{"op":"replace","path":"/locked","value":true}]})
        } else {
            json!({"targets":[image],"props":{"locked":true}})
        };
        assert_eq!(err(&h, "doc.object.patch", p).code, "cell.notApplicable");
        assert_eq!(rev(&h), revision);
        assert!(!rx.try_iter().any(|e| e.name == "element.drawing.changed"));
    }
}

#[test]
fn r04_generic_bridge_refuses_image_cells_into_vector_column() {
    let h = seeded();
    ok(&h, "cell.clipboard.set", json!({"elementIds":[BG],"from":1,"to":2}));
    let before = get(&h, B);
    let revision = rev(&h);
    let e = run(&h, "edit.paste", json!({"context":"xsheet","destination":{"elementId":B,"frame":20}}));
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("cell.typeMismatch"), "04 §13.16 both bridges: {e:?}");
    assert_eq!(get(&h, B), before);
    assert_eq!(rev(&h), revision);
}

fn assert_generic_cut_notes(special: bool) {
    let h = seeded();
    for f in [3, 12] {
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":f,"text":""}));
    }
    for (f, text) in [(1, "cut731"), (3, "tail731")] {
        ok(&h, "cell.note.set", json!({"elementId":B,"frame":f,"text":text}));
    }
    ok(&h, "cell.select", json!({"elementIds":[B],"from":1,"to":2}));
    if special {
        ok(&h, "edit.cutSpecial", json!({"context":"xsheet","options":{"createDeleteExposure":true}}));
    } else {
        ok(&h, "edit.cut", json!({"context":"xsheet"}));
    }
    let expected = if special { json!({"1":"tail731"}) } else { json!({"3":"tail731"}) };
    assert_eq!(notes(&h), expected, "SI-093 generic cut, special={special}");
}

#[test]
fn r05_generic_cut_clears_notes_and_retimes_remaining_notes() {
    assert_generic_cut_notes(false);
}

#[test]
fn r05_generic_cut_special_retimes_remaining_notes() {
    assert_generic_cut_notes(true);
}
