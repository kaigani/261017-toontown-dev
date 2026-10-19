//! Round-3 independent public-API regressions; seed 0xc123_7e61.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review regressions")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::Duration;
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions};

fn fresh() -> ExecutorHandle {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::fixture::sample_document(0xc123_7e61).unwrap(), None, false).is_ok());
    h
}

fn held(h: &ExecutorHandle) -> (String, mpsc::Sender<()>) {
    let (release, wait) = mpsc::channel();
    let (started, ready) = mpsc::channel();
    let id = h
        .jobs()
        .submit("review.r3", "user", move |_| {
            started.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(10));
            Ok(json!({"fresh": 0xc123}))
        })
        .unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    (id, release)
}

#[test]
fn overlapping_identical_waits_retain_one_original_envelope() {
    let h = fresh();
    let (id, release) = held(&h);
    let call = Invocation {
        command: "job.wait".into(),
        params: json!({"jobId":id,"timeoutMs":800}),
        options: InvokeOptions { request_id: Some("r3-overlap".into()), ..Default::default() },
        origin: tt_executor::Origin::User,
    };
    let first_h = h.clone();
    let first_call = call.clone();
    let first = std::thread::spawn(move || first_h.invoke(first_call));
    std::thread::sleep(Duration::from_millis(100));
    // The first request must actually reserve the ID before starting the duplicate.
    for _ in 0..100 {
        let probe = with(
            &h,
            "app.frame.set",
            json!({"frame":19}),
            InvokeOptions { request_id: Some("r3-overlap".into()), ..Default::default() },
        );
        if probe.error.as_ref().is_some_and(|e| e.details["inProgress"] == true) {
            break;
        }
        // Avoid winning the ID with the probe: give the spawned waiter a scheduling turn before this loop.
        assert!(!probe.is_ok(), "test scheduling: first waiter did not reserve the id");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(250));
    let second_h = h.clone();
    let second_call = call.clone();
    let second = std::thread::spawn(move || second_h.invoke(second_call));
    let original = first.join().unwrap();
    assert!(original.is_ok(), "{original:?}");
    assert_eq!(original.result.as_ref().unwrap()["timedOut"], true);
    release.send(()).unwrap();
    let duplicate = second.join().unwrap();
    assert_eq!(
        serde_json::to_value(&duplicate).unwrap(),
        serde_json::to_value(&original).unwrap(),
        "11 §7.2: overlapping identical retries must return the original envelope"
    );
    assert_eq!(serde_json::to_value(h.invoke(call)).unwrap(), serde_json::to_value(original).unwrap());
}

#[test]
fn staged_drawing_only_diff_keeps_content_invalidation() {
    let h = fresh();
    let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap();
    let before = rev(&h);
    let e = batch(
        &h,
        json!({"label":"R3 filtered drawing invalidation", "steps":[
            {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"r3":7321}}}},
            {"id":"doc.diff","params":{"sinceRev":before,"types":["drawing"],"within":drawing}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let steps = e.steps.unwrap();
    let changes = steps[1].result.as_ref().unwrap()["changes"].as_array().unwrap();
    assert!(
        changes.iter().any(|c| c["id"] == drawing && c["rev"] == before + 1),
        "REV-02 and R2-12: drawing filter must not erase staged content invalidation: {changes:?}"
    );
}

#[test]
fn checkpoint_after_stroke_edit_has_no_diff_at_same_committed_revision() {
    let h = fresh();
    let stroke = first(&h, "stroke");
    let e = batch(
        &h,
        json!({"label":"R3 checkpoint stamps", "steps":[
            {"id":"doc.object.patch","params":{"targets":[stroke],"props":{"meta":{"r3":527}}}},
            {"id":"checkpoint.create","params":{"name":"r3-after-stroke"}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let diff = ok(&h, "doc.diff", json!({"sinceCheckpoint":"r3-after-stroke"}));
    assert_eq!(diff["fromRev"], diff["toRev"]);
    assert_eq!(diff["changes"], json!([]), "R2-03: checkpoint must carry the staged object's revision stamps");
}

#[test]
fn checkpoint_create_delete_and_failure_are_transactional_on_fresh_content() {
    let h = fresh();
    let original = get(&h, "scene:#1");
    let e = batch(
        &h,
        json!({"label":"R3 checkpoint lifecycle", "steps":[
            {"id":"checkpoint.create","params":{"name":"r3-base"}},
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"r3":713}}}},
            {"id":"checkpoint.create","params":{"name":"r3-mid"}},
            {"id":"checkpoint.list","params":{}},
            {"id":"checkpoint.delete","params":{"name":"r3-base"}},
            {"id":"r3.fail","params":{}}
        ]}),
    );
    assert!(e.is_error());
    assert_eq!(get(&h, "scene:#1"), original);
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
    let steps = e.steps.unwrap();
    assert_eq!(steps[3].result.as_ref().unwrap().as_array().unwrap().len(), 2);
}

#[test]
fn public_staging_rewind_and_serde_candidates_cannot_cross_load_integrity_gate() {
    let doc = tt_model::fixture::sample_document(0xc123_6e12).unwrap();
    let candidate = doc.at_earlier_revision(0, &[]).unwrap();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(candidate, None, false).is_error());
    assert!(h.snapshot().unwrap().is_none());
    let mut parts = tt_model::fixture::sample_parts(0xc123_6e12).unwrap();
    let mut v = serde_json::to_value(&parts.elements[0]).unwrap();
    v["rev"] = json!(u64::MAX);
    parts.elements[0] = tt_model::object::from_json(&v).unwrap();
    let candidate = tt_model::document::Document::assemble(parts);
    assert!(h.open_document(candidate, None, false).is_error());
    assert!(h.snapshot().unwrap().is_none());
    assert!(h.open_document(doc, None, false).is_ok());
    let snapshot = h.snapshot().unwrap().unwrap();
    ok(&h, "doc.object.patch", json!({"targets":["scene:#1"],"props":{"meta":{"live":831}}}));
    assert!(h.open_document(snapshot.with_revisions(99, &[]).unwrap(), None, false).is_error());
    assert_eq!(get(&h, "scene:#1")["meta"]["live"], 831);
}

#[test]
fn malformed_paste_destination_variants_never_fall_back() {
    for bad in [json!({"sceneId":19}), json!({"objectIds":[false]}), json!({"frame":0}), json!({"drawingId":{}})] {
        let h = fresh();
        ok(&h, "edit.copy", json!({"context":"staging","objectIds":["scene:#1/element:\"Bob-P\""]}));
        let before = rev(&h);
        assert!(run(&h, "edit.paste", json!({"context":"staging","destination":bad})).is_error());
        assert_eq!(rev(&h), before);
    }
}

#[test]
fn dry_run_ids_match_for_all_implemented_paste_creation_routes() {
    let scenarios = [
        ("edit.paste", json!({"context":"staging"}), "staging", "scene:#1/element:\"Bob-P\""),
        (
            "edit.paste",
            json!({"context":"drawing","destination":{"drawingId":"scene:#1/element:\"Bob\"/drawing:#1"}}),
            "drawing",
            "scene:#1/element:\"Bob\"/drawing:#1/drawingNode:#1",
        ),
        (
            "edit.pasteSpecial",
            json!({"context":"xsheet","destination":{"elementId":"scene:#1/element:\"Bob\""},"options":{"overwriteDrawings":false}}),
            "xsheet",
            "scene:#1/element:\"Bob\"/cell:#1",
        ),
        ("edit.pasteAsNewElement", json!({"frame":11}), "xsheet", "scene:#1/element:\"Bob\"/cell:#1"),
    ];
    for (id, params, context, source) in scenarios {
        let h = fresh();
        ok(&h, "edit.copy", json!({"context":context,"objectIds":[source]}));
        let preview = with(&h, id, params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
        let actual = run(&h, id, params);
        assert!(preview.is_ok() && actual.is_ok(), "{id}: {preview:?} {actual:?}");
        assert_eq!(preview.result, actual.result, "{id}: exact ID prediction");
    }
}

#[test]
fn sequential_batch_wait_uses_elapsed_time_and_reaches_completion() {
    let h = fresh();
    let (job, release) = held(&h);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1250));
        release.send(()).unwrap();
    });
    let start = std::time::Instant::now();
    let e = batch(
        &h,
        json!({"label":"R3 blocking wait CPU probe","atomic":false,"steps":[
            {"id":"job.wait","params":{"jobId":job,"timeoutMs":4000}},
            {"id":"app.frame.set","params":{"frame":37}}
        ]}),
    );
    release.join().unwrap();
    assert!(e.is_ok(), "{e:?}");
    assert!(start.elapsed() >= Duration::from_millis(1000));
    assert_eq!(e.steps.unwrap()[0].result.as_ref().unwrap()["state"], "succeeded");
    assert_eq!(ok(&h, "app.state.get", json!({}))["frame"], 37);
}

#[test]
fn opaque_payloads_survive_every_edit_handler_and_property_patch() {
    let bob = "scene:#1/element:\"Bob\"";
    let peg = "scene:#1/element:\"Bob-P\"";
    for command in ["edit.copy", "edit.cut", "edit.cutSpecial", "edit.delete"] {
        let h = fresh();
        let victim = get(&h, peg)["id"].clone();
        let opaque = json!({"0":{"type":"element","id":victim,"rev":77,"ext":{"literal":null}},"ids":[victim,"a/b~c"]});
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[bob],"ops":[{"op":"add","path":"/meta","value":opaque},{"op":"add","path":"/ext","value":{"review.r3":opaque}}]}),
        );
        let sibling = first(&h, "scene");
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[sibling],"ops":[{"op":"add","path":"/meta","value":opaque},{"op":"add","path":"/ext","value":{"review.r3":opaque}}]}),
        );
        let e = run(&h, command, json!({"context":"staging","objectIds":[peg]}));
        assert!(e.is_ok(), "{command}: {e:?}");
        assert_eq!(get(&h, &sibling)["meta"], opaque, "{command}: sibling opaque strings");
        assert_eq!(get(&h, &sibling)["ext"]["review.r3"], opaque, "{command}: sibling extensions");
        if command != "edit.delete" {
            let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
            let child = clip["items"].as_array().unwrap().iter().find(|v| v["name"] == "Bob").unwrap();
            assert_eq!(child["meta"], opaque, "{command}: copied descendant metadata");
            assert_eq!(child["ext"]["review.r3"], opaque);
        }
        if command != "edit.copy" {
            ok(&h, "edit.undo", json!({}));
        }
        assert_eq!(get(&h, bob)["meta"], opaque);
    }
    for command in ["edit.paste", "edit.pasteSpecial", "edit.pasteAsNewElement"] {
        let h = fresh();
        let stroke = first(&h, "stroke");
        let opaque = json!({"type":"stroke","id":stroke,"rev":73,"meta":{"ext":{"literal":[1,null,"e\u{301}"]}}});
        ok(
            &h,
            "doc.object.patch",
            json!({"targets":[stroke],"ops":[{"op":"add","path":"/meta","value":opaque},{"op":"add","path":"/ext","value":{"review.r3":opaque}}]}),
        );
        ok(&h, "edit.copy", json!({"context":"drawing","objectIds":[stroke]}));
        let params = if command == "edit.pasteAsNewElement" {
            json!({"frame":3})
        } else {
            json!({"context":"drawing","destination":{"drawingId":stroke.split('/').next().unwrap()}})
        };
        let e = run(&h, command, params);
        assert!(e.is_ok(), "{command}: {e:?}");
        let document = h.snapshot().unwrap().unwrap();
        let preserved = document
            .locations()
            .filter(|l| l.ty == tt_model::ObjectType::Stroke)
            .filter(|l| document.object_raw_json(&l.key).unwrap().1["meta"] == opaque)
            .count();
        assert_eq!(preserved, 2, "{command}: copied stroke and source retain opaque model-shaped values");
    }
}

#[test]
fn json04_multi_operation_inverse_restores_raw_bytes_and_opaque_boundaries() {
    let original = get(&fresh(), "scene:#1/element:\"Bob\"");
    let add = |path: &str, value: Value| tt_model::change::PatchOp {
        op: "add",
        path: path.into(),
        value: Some(value),
        from: None,
        range: None,
        old_value: None,
        elided: false,
    };
    let ops = vec![
        add("/meta/0", json!({"type":"stroke","rev":19,"ext":{}})),
        add("/meta/0/ext/a~1b~0c", json!([1, null, {}])),
        add("/ext/review.r3", json!({"id":"not-a-reference"})),
    ];
    let (after, inverse) = tt_model::change::apply_with_inverse(&original, &ops).unwrap();
    let restored = tt_model::change::apply(&after, &inverse).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&original).unwrap());
    let bad = vec![add("/meta/0/meta/invented", json!(1))];
    assert!(tt_model::change::apply(&after, &bad).is_err());
}

#[test]
fn fresh_clipboard_property_values_are_checked_before_storage() {
    let h = fresh();
    for value in [json!({"columnWidth":1.25}), json!({"visible":"true"}), json!({"rev":999}), json!({"id":"el_wrong"})]
    {
        let e = run(
            &h,
            "edit.clipboard.set",
            json!({"content":{"kind":"elementProperties","itemCount":1,"items":[value]}}),
        );
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("edit.clipboardInvalid"), "{e:?}");
        assert_eq!(ok(&h, "edit.clipboard.get", json!({})), Value::Null);
    }
}

#[test]
fn fresh_complete_forest_paste_preserves_derived_parent_links() {
    let h = fresh();
    let before = h.snapshot().unwrap().unwrap();
    let ids: Vec<Value> = ok(&h, "doc.object.find", json!({"type":"element","limit":1000}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone())
        .collect();
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":ids}));
    let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    let result = ok(&h, "edit.paste", json!({"context":"staging"}));
    let old = clip["items"].as_array().unwrap();
    let new = result["createdIds"].as_array().unwrap();
    assert_eq!(old.len(), new.len());
    for (item, id) in old.iter().zip(new) {
        let object = ok(&h, "doc.object.get", json!({"ref":id,"include":["derived"]}));
        let expected = old.iter().position(|v| v["id"] == item["parentId"]).map_or(Value::Null, |i| new[i].clone());
        assert_eq!(object["object"]["parentId"], expected);
    }
    assert!(h.snapshot().unwrap().unwrap().issues().iter().all(|i| i.severity != tt_model::document::Severity::Error));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(h.snapshot().unwrap().unwrap().entry_keys().len(), before.entry_keys().len());
}

#[test]
fn key_and_template_creation_predict_ids_and_stay_opaque() {
    for kind in ["key", "libraryItem"] {
        let h = fresh();
        let source = first(&h, kind);
        let context = if kind == "key" { "functionEditor" } else { "library" };
        let opaque = json!({"type":kind,"id":source,"rev":111,"ext":{"unchanged":null}});
        ok(&h, "doc.object.patch", json!({"targets":[source],"ops":[{"op":"add","path":"/meta","value":opaque}]}));
        ok(&h, "edit.copy", json!({"context":context,"objectIds":[source]}));
        let dest = if kind == "key" {
            let doc = h.snapshot().unwrap().unwrap();
            json!({"functionId":doc.locate(&source).unwrap().parent,"frame":35})
        } else {
            json!({})
        };
        for command in ["edit.paste", "edit.pasteSpecial"] {
            let params = json!({"context":context,"destination":dest});
            let dry = with(&h, command, params.clone(), InvokeOptions { dry_run: true, ..Default::default() });
            let actual = run(&h, command, params);
            assert!(dry.is_ok() && actual.is_ok(), "{kind} {command}: {dry:?} {actual:?}");
            assert_eq!(dry.result, actual.result);
            let new = actual.result.as_ref().unwrap()["createdIds"][0].as_str().unwrap();
            assert_eq!(get(&h, new)["meta"], opaque, "{kind} {command}");
            ok(&h, "edit.undo", json!({}));
        }
    }
}

#[test]
fn maximum_revision_rolls_back_checkpoints_and_keeps_executor_alive() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let mut parts = tt_model::fixture::sample_parts(0xc123_ffff).unwrap();
    parts.document_rev = u64::MAX;
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    let e = batch(
        &h,
        json!({"label":"R3 exhaustion", "steps":[
            {"id":"checkpoint.create","params":{"name":"r3-exhausted"}},
            {"id":"doc.object.patch","params":{"targets":["scene:#1"],"props":{"meta":{"overflow":true}}}}
        ]}),
    );
    assert!(e.is_error());
    assert_eq!(e.error.as_ref().unwrap().details["error"]["code"], "doc.limitExceeded");
    assert_eq!(rev(&h), u64::MAX);
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
    assert!(get(&h, "scene:#1")["meta"].get("overflow").is_none());
}

#[test]
fn fresh_cell_paste_checks_target_type_and_locked_drawing_before_commit() {
    let h = fresh();
    let bob = "scene:#1/element:\"Bob\"";
    ok(&h, "edit.copy", json!({"context":"xsheet","objectIds":[format!("{bob}/cell:#1")]}));
    let wrong = run(
        &h,
        "edit.paste",
        json!({"context":"xsheet","destination":{"elementId":"scene:#1/element:\"Bob-P\"","frame":1}}),
    );
    assert_eq!(wrong.error.as_ref().map(|e| e.code.as_str()), Some("edit.wrongElementType"));
    let new_element = ok(&h, "edit.pasteAsNewElement", json!({"frame":1}))["elementId"].as_str().unwrap().to_owned();
    let drawing = get(&h, &format!("{bob}/drawing:#1"))["id"].as_str().unwrap().to_owned();
    let snapshot = h.snapshot().unwrap().unwrap();
    let (entry, _, _) = snapshot.object_pointer(&drawing).unwrap();
    let locked = snapshot
        .with_patched_entries(&[(
            entry,
            vec![tt_model::change::PatchOp {
                op: "replace",
                path: "/locked".into(),
                from: None,
                range: None,
                value: Some(json!(true)),
                old_value: None,
                elided: false,
            }],
        )])
        .unwrap()
        .0;
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(locked, None, false).is_ok());
    let before = get(&h, &drawing);
    let before_rev = rev(&h);
    let clip = json!({"kind":"cells","itemCount":1,"items":[{"elementId":new_element,"frameOffset":0,"drawingRef":get(&h,&new_element)["exposure"][0][0]}]});
    ok(&h, "edit.clipboard.set", json!({"content":clip}));
    for command in ["edit.paste", "edit.pasteSpecial"] {
        let e = run(&h, command, json!({"context":"xsheet","destination":{"elementId":bob,"frame":1}}));
        assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("drawing.locked"), "{command}: {e:?}");
        assert_eq!(get(&h, &drawing), before);
        assert_eq!(rev(&h), before_rev);
    }
}
