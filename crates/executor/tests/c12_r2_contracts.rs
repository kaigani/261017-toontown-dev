//! Independent round-2 adversarial inputs, specification assertions, no product edits.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
mod common;
use common::*;
use serde_json::{Value, json};
use std::sync::mpsc;
use std::time::Duration;
use tt_executor::{ExecutorHandle, Invocation, InvokeOptions, Origin};

const BOB: &str = "scene:\"Intro\"/element:\"Bob\"";
const PEG: &str = "scene:\"Intro\"/element:\"Bob-P\"";

fn canonical(h: &ExecutorHandle) -> Vec<String> {
    let doc = h.snapshot().unwrap().unwrap();
    doc.entry_keys()
        .into_iter()
        .map(|k| tt_model::canonical::to_canonical_string(&doc.entry_raw_json(&k).unwrap()).unwrap())
        .collect()
}

fn locked_fixture() -> ExecutorHandle {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let i = parts.elements.iter().position(|e| e.name() == "Bob").unwrap();
    let mut v = serde_json::to_value(&parts.elements[i]).unwrap();
    v["locked"] = json!(true);
    parts.elements[i] = tt_model::object::from_json(&v).unwrap();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(tt_model::document::Document::from_parts(parts).unwrap(), None, false).is_ok());
    h
}

fn held(h: &ExecutorHandle) -> (String, mpsc::Sender<()>) {
    let (release, wait) = mpsc::channel();
    let (started, ready) = mpsc::channel();
    let id = h
        .jobs()
        .submit("review.r2", "user", move |_| {
            started.send(()).unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(30));
            Ok(json!({"r2": 42}))
        })
        .unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    (id, release)
}

fn normal(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        for k in ["rev", "documentRev", "dryRun", "undo", "createdAt", "time"] {
            m.shift_remove(k);
        }
        for x in m.values_mut() {
            *x = normal(x.take());
        }
    } else if let Some(a) = v.as_array_mut() {
        for x in a {
            *x = normal(x.take());
        }
    }
    v
}

#[test]
fn failed_batch_rolls_back_all_five_effect_classes_together() {
    let h = fixture();
    let (job, release) = held(&h);
    let before = ok(&h, "app.state.get", json!({}));
    let original = canonical(&h);
    let rx = h.subscribe();
    let env = batch(
        &h,
        json!({"label":"R2 composite rollback","steps":[
            {"id":"job.cancel","params":{"jobId":job}},
            {"id":"edit.copy","params":{"context":"staging","objectIds":[BOB]}},
            {"id":"checkpoint.create","params":{"name":"r2-rollback"}},
            {"id":"workspace.select","params":{"name":"Focused drawing"}},
            {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"r2":917}}}},
            {"id":"unknown.r2","params":{}}
        ]}),
    );
    assert!(env.is_error(), "{env:?}");
    assert_eq!(env.error.as_ref().unwrap().details["stepIndex"], 6, "all five effects must reach the final failure");
    assert_eq!(canonical(&h), original);
    assert_eq!(ok(&h, "app.state.get", json!({})), before);
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
    assert_eq!(ok(&h, "edit.clipboard.get", json!({})), Value::Null);
    h.snapshot().unwrap();
    assert!(rx.try_iter().all(|e| e.name.starts_with("job.")));
    release.send(()).unwrap();
    assert_eq!(h.jobs().wait(&job, Duration::from_secs(5)).unwrap().0.state, tt_executor::JobState::Succeeded);
}

#[test]
fn composite_failed_batch_preview_matches_real_envelope_and_commits_no_effects() {
    let h = fixture();
    let (job, release) = held(&h);
    let state = ok(&h, "app.state.get", json!({}));
    let doc = canonical(&h);
    let events = h.subscribe();
    let req = json!({"label":"R2 composite prediction","dryRun":true,"steps":[
        {"id":"job.cancel","params":{"jobId":job}},
        {"id":"edit.copy","params":{"context":"staging","objectIds":[BOB]}},
        {"id":"checkpoint.create","params":{"name":"r2-preview"}},
        {"id":"workspace.select","params":{"name":"Focused drawing"}},
        {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"prediction":9473}}}},
        {"id":"unknown.r2","params":{}}
    ]});
    let preview = batch(&h, req.clone());
    assert_eq!(preview.error.as_ref().unwrap().details["stepIndex"], 6);
    assert_eq!(canonical(&h), doc);
    assert_eq!(ok(&h, "app.state.get", json!({})), state);
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
    assert_eq!(ok(&h, "edit.clipboard.get", json!({})), Value::Null);
    h.snapshot().unwrap();
    assert!(events.try_iter().all(|e| e.name.starts_with("job.")));
    let mut req = req;
    req["dryRun"] = json!(false);
    let real = batch(&h, req);
    let mut preview = serde_json::to_value(preview).unwrap();
    // Only the documented preview flag differs for this rolled-back batch; retain revisions, IDs and all results.
    preview.as_object_mut().unwrap().shift_remove("dryRun");
    if let Some(steps) = preview["steps"].as_array_mut() {
        for step in steps {
            step.as_object_mut().unwrap().shift_remove("dryRun");
        }
    }
    assert_eq!(serde_json::to_vec(&preview).unwrap(), serde_json::to_vec(&real).unwrap());
    release.send(()).unwrap();
    assert_eq!(h.jobs().wait(&job, Duration::from_secs(5)).unwrap().0.state, tt_executor::JobState::Succeeded);
}

#[test]
fn checkpoint_is_visible_to_later_atomic_query() {
    let h = fixture();
    let e = batch(
        &h,
        json!({"label":"R2 checkpoint visibility","steps":[
            {"id":"checkpoint.create","params":{"name":"r2-visible"}},
            {"id":"checkpoint.list","params":{}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let steps = e.steps.unwrap();
    assert_eq!(steps[1].result.as_ref().unwrap()[0]["name"], "r2-visible", "11 §7.3 later queries see staged state");
}

#[test]
fn checkpoint_create_then_delete_in_atomic_batch_succeeds() {
    let h = fixture();
    let e = batch(
        &h,
        json!({"label":"R2 checkpoint lifecycle","steps":[
            {"id":"checkpoint.create","params":{"name":"r2-temporary"}},
            {"id":"checkpoint.delete","params":{"name":"r2-temporary"}}
        ]}),
    );
    assert!(e.is_ok(), "11 §7.3: {e:?}");
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
}

#[test]
fn sequential_dry_run_simulates_checkpoints() {
    let h = fixture();
    let req = json!({"label":"R2 simulation","atomic":false,"dryRun":true,"steps":[
        {"id":"checkpoint.create","params":{"name":"r2-sequential"}},
        {"id":"checkpoint.list","params":{}}
    ]});
    let dry = batch(&h, req.clone());
    assert_eq!(ok(&h, "checkpoint.list", json!({})), json!([]));
    let mut actual = req;
    actual["dryRun"] = json!(false);
    let real = batch(&h, actual);
    assert_eq!(
        normal(serde_json::to_value(dry).unwrap()),
        normal(serde_json::to_value(real).unwrap()),
        "11 §7.2 prediction"
    );
}

#[test]
fn checkpoint_before_edit_predicts_same_result_in_dry_run() {
    let h = fixture();
    let req = json!({"label":"R2 checkpoint prediction","dryRun":true,"steps":[
        {"id":"checkpoint.create","params":{"name":"r2-first"}},
        {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"n":946}}}}
    ]});
    let dry = batch(&h, req.clone());
    let mut actual = req;
    actual["dryRun"] = json!(false);
    let real = batch(&h, actual);
    assert_eq!(
        dry.steps.unwrap()[0].result,
        real.steps.unwrap()[0].result,
        "checkpoint result revision is not an envelope dry-run field"
    );
}

#[test]
fn paste_dry_run_predicts_created_ids_exactly() {
    let h = fixture();
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":[PEG]}));
    let dry =
        with(&h, "edit.paste", json!({"context":"staging"}), InvokeOptions { dry_run: true, ..Default::default() });
    let real = run(&h, "edit.paste", json!({"context":"staging"}));
    assert!(dry.is_ok() && real.is_ok(), "{dry:?} {real:?}");
    assert_eq!(dry.result, real.result, "R1-02/03: predicted IDs must match the real result");
}

#[test]
fn identical_document_and_paste_sequence_produce_identical_ids() {
    let a = fixture();
    let b = fixture();
    let mut outputs = Vec::new();
    for h in [&a, &b] {
        ok(h, "edit.copy", json!({"context":"staging","objectIds":[PEG]}));
        outputs.push(ok(h, "edit.paste", json!({"context":"staging"})));
    }
    assert_eq!(outputs[0], outputs[1], "11 P7 / 10 AG-06 deterministic ID allocation");
}

#[test]
fn element_delete_preserves_opaque_metadata_and_extension_strings() {
    let h = fixture();
    let victim = get(&h, PEG)["id"].clone();
    let opaque = json!({"literal":victim,"list":[{"text":victim}],"nested":{"id":victim}});
    ok(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"meta":opaque,"ext":{"org.review":opaque}}}));
    let before = get(&h, BOB);
    ok(&h, "edit.delete", json!({"context":"staging","objectIds":[PEG]}));
    let after = get(&h, BOB);
    assert_eq!(after["meta"], before["meta"], "10 §2.2 metadata is never interpreted");
    assert_eq!(after["ext"], before["ext"], "unknown namespaces preserved verbatim");
}

#[test]
fn drawing_copy_preserves_object_shaped_opaque_metadata() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap().to_owned();
    let opaque = json!({"shape":{"id":"opaque-literal","type":"stroke","rev":432,"text":"keep"}});
    ok(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":opaque}}));
    ok(&h, "edit.copy", json!({"context":"drawing","objectIds":[stroke]}));
    let r = ok(&h, "edit.paste", json!({"context":"drawing","destination":{"drawingId":drawing}}));
    assert_eq!(
        get(&h, r["createdIds"][0].as_str().unwrap())["meta"],
        opaque,
        "10 §2.2 opaque data cannot be traversed as model objects"
    );
}

#[test]
fn locked_elements_still_allow_exposure_edits_per_04_10_4() {
    let h = locked_fixture();
    let e = run(&h, "edit.delete", json!({"context":"xsheet","objectIds":[format!("{BOB}/cell:#1")]}));
    assert!(e.is_ok(), "04 §10.4 explicitly permits exposure editing: {e:?}");
}

#[test]
fn explicit_key_edit_on_locked_element_is_allowed_per_05_6_6() {
    let h = locked_fixture();
    let e = run(
        &h,
        "edit.delete",
        json!({"context":"functionEditor","objectIds":[format!("{BOB}/channel:\"rotation\"/key:\"12\"")]}),
    );
    assert!(e.is_ok(), "05 §6.6 ID-addressed commands still act: {e:?}");
}

#[test]
fn clipboard_element_properties_validates_values_not_only_names() {
    let h = fixture();
    let e = run(
        &h,
        "edit.clipboard.set",
        json!({"content":{"kind":"elementProperties","itemCount":1,"items":[{"columnWidth":"not-an-integer"}]}}),
    );
    assert!(e.is_error(), "01 §12.1 clipboard kind schema must validate values: {e:?}");
}

#[test]
fn json04_materialisation_does_not_invent_opaque_nested_containers() {
    let h = fixture();
    ok(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"meta":{"custom":{}}}}));
    let before = canonical(&h);
    let e = run(
        &h,
        "doc.object.patch",
        json!({"targets":[BOB],"ops":[{"op":"add","path":"/meta/custom/ext/literal","value":751}]}),
    );
    assert!(e.is_error(), "RFC6902 missing parent is not an omitted model member: {e:?}");
    assert_eq!(canonical(&h), before);
}

#[test]
fn omitted_meta_materialisation_keeps_canonical_undo_exact() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let before = normal(get(&h, &stroke));
    ok(&h, "doc.object.patch", json!({"targets":[stroke],"ops":[{"op":"add","path":"/meta/round2","value":723}]}));
    ok(&h, "edit.undo", json!({}));
    assert_eq!(normal(get(&h, &stroke)), before);
}

#[test]
fn diff_reports_drawing_content_revision_invalidation() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let drawing = stroke.split('/').next().unwrap();
    let before = rev(&h);
    ok(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":{"r2":726}}}));
    let diff = ok(&h, "doc.diff", json!({"sinceRev":before}));
    assert!(
        diff["changes"].as_array().unwrap().iter().any(|c| c["id"] == drawing),
        "10 REV-02 / AG-09 catch-up omits changed drawing revision: {diff}"
    );
}

#[test]
fn all_edit_commands_require_project() {
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    for id in [
        "edit.cut",
        "edit.copy",
        "edit.paste",
        "edit.delete",
        "edit.cutSpecial",
        "edit.pasteSpecial",
        "edit.pasteAsNewElement",
    ] {
        assert_eq!(err(&h, id, json!({})).code, "project.none", "{id}");
    }
}

#[test]
fn mixed_expect_rev_guards_cover_paths_compound_ids_and_unmodified_project() {
    let h = fixture();
    let stroke = first(&h, "stroke");
    let project = first(&h, "project");
    let initial = rev(&h);
    ok(&h, "doc.object.patch", json!({"targets":[stroke],"props":{"meta":{"r2":18}}}));
    let sr = obj_rev(&h, &stroke);
    let pr = obj_rev(&h, &project);
    assert_eq!(pr, initial, "project own rev differs from document revision after child edit");
    let opts = InvokeOptions {
        expect_rev: [(project, pr), (stroke.clone(), sr), ("scene:#1".into(), initial)].into_iter().collect(),
        ..Default::default()
    };
    assert!(
        with(
            &h,
            "doc.object.patch",
            json!({"targets":[BOB],"props":{"meta":{"guarded":728}},"expectRev":{"palette:\"Main\"":initial}}),
            opts.clone()
        )
        .is_ok()
    );
    let mut stale = opts;
    stale.expect_rev.insert(stroke.clone(), sr - 1);
    let e =
        with(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"meta":{"guarded":729}}}), stale).error.unwrap();
    assert_eq!(e.code, "rev.conflict");
    assert_eq!(e.details["currentRev"], sr);
    assert_eq!(e.details["id"], stroke);
    assert!(e.details.get("current").is_some());
    assert_eq!(get(&h, BOB)["meta"]["guarded"], 728);
}

#[test]
fn edit_object_kinds_clipboard_and_history_roundtrip() {
    let cases = [
        ("staging", PEG),
        ("drawing", "scene:\"Intro\"/element:\"Bob\"/drawing:\"1\"/layer:#1/stroke:#1"),
        ("xsheet", "scene:\"Intro\"/element:\"Bob\"/cell:#2"),
        ("functionEditor", "scene:\"Intro\"/element:\"Bob\"/channel:\"rotation\"/key:\"24\""),
        ("library", "library:local/libraryFolder:\"Props\"/libraryItem:\"Walk\""),
    ];
    for (context, target) in cases {
        let h = fixture();
        let before: Vec<_> = canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect();
        ok(&h, "edit.cut", json!({"context":context,"objectIds":[target]}));
        let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
        ok(&h, "edit.clipboard.set", json!({"content":clip}));
        assert_eq!(ok(&h, "edit.clipboard.get", json!({"includeData":true})), clip);
        let after: Vec<_> = canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect();
        ok(&h, "edit.undo", json!({}));
        assert_eq!(
            canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>(),
            before,
            "{context}"
        );
        ok(&h, "edit.redo", json!({}));
        assert_eq!(
            canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>(),
            after,
            "{context}"
        );
    }
    // Fresh destinations/options also cover key, template, motion-point and property pastes, and cut special.
    for case in 0..5 {
        let h = fixture();
        let (command, params) = match case {
            0 => {
                ok(
                    &h,
                    "edit.copy",
                    json!({"context":"functionEditor","objectIds":[format!("{BOB}/channel:\"rotation\"/key:\"24\"")]}),
                );
                (
                    "edit.paste",
                    json!({"context":"functionEditor","destination":{"functionId":format!("{BOB}/channel:\"rotation\""),"frame":37}}),
                )
            }
            1 => {
                ok(
                    &h,
                    "edit.copy",
                    json!({"context":"library","objectIds":["library:local/libraryFolder:\"Props\"/libraryItem:\"Walk\""]}),
                );
                (
                    "edit.paste",
                    json!({"context":"library","destination":{"catalogId":"library:local/libraryFolder:\"Props\""}}),
                )
            }
            2 => {
                let points = ok(&h, "doc.object.find", json!({"type":"motionPoint","limit":3}))["items"].clone();
                ok(&h, "edit.copy", json!({"context":"functionEditor","objectIds":[points[2]["id"]]}));
                (
                    "edit.pasteSpecial",
                    json!({"context":"functionEditor","destination":{"objectIds":[points[0]["id"]]},"options":{"x":true,"y":false,"z":false}}),
                )
            }
            3 => {
                ok(
                    &h,
                    "edit.clipboard.set",
                    json!({"content":{"kind":"elementProperties","itemCount":1,"items":[{"columnWidth":137}]}}),
                );
                ("edit.paste", json!({"context":"staging","destination":{"objectIds":[BOB]}}))
            }
            _ => (
                "edit.cutSpecial",
                json!({"context":"functionEditor","objectIds":[format!("{BOB}/channel:\"rotation\"/key:\"24\"")],"options":{"rotationKeys":true,"scaleKeys":false}}),
            ),
        };
        if case < 4 {
            let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
            ok(&h, "edit.clipboard.set", json!({"content":clip}));
            assert_eq!(ok(&h, "edit.clipboard.get", json!({"includeData":true})), clip);
        }
        let before: Vec<_> = canonical(&h).iter().map(|s| normal(serde_json::from_str(s).unwrap())).collect();
        ok(&h, command, params);
        let after: Vec<_> = canonical(&h).iter().map(|s| normal(serde_json::from_str(s).unwrap())).collect();
        assert_ne!(after, before, "paste/cut-special case {case} must edit content");
        ok(&h, "edit.undo", json!({}));
        assert_eq!(canonical(&h).iter().map(|s| normal(serde_json::from_str(s).unwrap())).collect::<Vec<_>>(), before);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(canonical(&h).iter().map(|s| normal(serde_json::from_str(s).unwrap())).collect::<Vec<_>>(), after);
    }
}

#[test]
fn malformed_paste_destination_is_rejected_without_fallback() {
    let h = fixture();
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":[PEG]}));
    let e = run(&h, "edit.paste", json!({"context":"staging","destination":{"sceneId":false}}));
    assert!(e.is_error(), "11 §8 wrong-typed explicit target silently fell back to current scene: {e:?}");
}

#[test]
fn edit_envelopes_report_explicit_or_selection_targets() {
    let h = fixture();
    ok(&h, "edit.selection.set", json!({"context":"staging","ids":[PEG]}));
    let e = run(&h, "edit.copy", json!({"context":"staging"}));
    assert!(e.is_ok(), "{e:?}");
    assert!(e.targets.is_some(), "11 P4: selection-derived actual targets missing from envelope");
}

#[test]
fn pending_job_wait_reserves_request_id_against_other_commands() {
    let h = fixture();
    let (job, release) = held(&h);
    let options = InvokeOptions { request_id: Some("r2-pending".into()), ..Default::default() };
    let waiter = h.clone();
    let wait_opts = options.clone();
    let t = std::thread::spawn(move || with(&waiter, "job.wait", json!({"jobId":job,"timeoutMs":5000}), wait_opts));
    // Allow WaitBegin to finish; repeated barriers do not depend on job completion.
    std::thread::sleep(Duration::from_millis(150));
    h.snapshot().unwrap();
    let collision = with(&h, "app.frame.set", json!({"frame":37}), options);
    release.send(()).unwrap();
    let waited = t.join().unwrap();
    let conflict = |e: &tt_executor::Envelope| e.error.as_ref().is_some_and(|e| e.code == "request.conflict");
    // Scheduling may let either invocation claim the ID first. Exactly one may succeed.
    assert!(
        (waited.is_ok() && conflict(&collision)) || (collision.is_ok() && conflict(&waited)),
        "two different invocations must not share one request ID: wait={waited:?}; collision={collision:?}"
    );
}

#[test]
fn request_ids_are_session_scoped_and_survive_undo() {
    let h = fixture();
    let scene = first(&h, "scene");
    let options = InvokeOptions { request_id: Some("r2-same-id".into()), ..Default::default() };
    let call = Invocation {
        command: "doc.object.patch".into(),
        params: json!({"targets":[scene],"props":{"meta":{"r2":31}}}),
        options: options.clone(),
        origin: Origin::User,
    };
    let first = h.invoke(call.clone());
    assert!(first.is_ok());
    ok(&h, "edit.undo", json!({}));
    let before = rev(&h);
    assert_eq!(serde_json::to_value(h.invoke(call.clone())).unwrap(), serde_json::to_value(first).unwrap());
    assert_eq!(rev(&h), before);
    let mut other = call;
    other.origin = agent(&[tt_registry::Scope::Edit]);
    assert!(h.invoke(other).is_ok());
    assert_eq!(rev(&h), before + 1);
}

#[test]
fn eviction_at_byte_limit_never_reexecutes_oldest_request() {
    let h = fixture();
    let options = InvokeOptions { request_id: Some("r2-oldest".into()), ..Default::default() };
    let call =
        Invocation { command: "app.frame.set".into(), params: json!({"frame":13}), options, origin: Origin::User };
    assert!(h.invoke(call.clone()).is_ok());
    ok(&h, "app.frame.set", json!({"frame":47}));
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({"targets":[scene],"props":{"meta":{"payload":"r".repeat(1_048_576)}}}));
    for i in 0..70 {
        assert!(
            with(
                &h,
                "doc.object.get",
                json!({"ref":scene}),
                InvokeOptions { request_id: Some(format!("r2-fill-{i}")), ..Default::default() }
            )
            .is_ok()
        );
    }
    let retry = h.invoke(call);
    assert_eq!(retry.error.unwrap().code, "request.unknownId");
    assert_eq!(ok(&h, "app.state.get", json!({}))["frame"], 47);
}

#[test]
fn export_registry_for_independent_audit() {
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        let entries: Vec<_> = tt_executor::registry().unwrap().entries().map(tt_registry::Entry::describe).collect();
        std::fs::write(std::path::Path::new(&dir).join("registry.json"), serde_json::to_vec_pretty(&entries).unwrap())
            .unwrap();
    }
}

#[test]
fn external_snapshot_staging_and_revision_rewind_cannot_replace_dirty_live_document() {
    let h = fixture();
    let snapshot = h.snapshot().unwrap().unwrap();
    let object = first(&h, "scene");
    let (entry, _, _) = snapshot.object_pointer(&object).unwrap();
    let ops = vec![tt_model::change::PatchOp {
        op: "add",
        path: "/meta/r2-external".into(),
        from: None,
        range: None,
        value: Some(json!(462)),
        old_value: None,
        elided: false,
    }];
    let replacement = snapshot.with_patched_entries(&[(entry, ops)]).unwrap().0.at_earlier_revision(0, &[]).unwrap();
    ok(&h, "doc.object.patch", json!({"targets":[object],"props":{"meta":{"live-r2":381}}}));
    let before = canonical(&h);
    assert_eq!(
        h.open_document(replacement.clone(), Some("not-a-file.animpkg".into()), false).error.unwrap().code,
        "project.unsavedChanges"
    );
    let e = h.open_document_with(
        replacement,
        tt_executor::OpenOptions { path: None, read_only: false, on_unsaved: tt_executor::UnsavedPolicy::Discard },
        agent(&[tt_registry::Scope::Edit, tt_registry::Scope::FilesRead]),
    );
    assert_eq!(e.error.unwrap().code, "permission.denied");
    assert_eq!(canonical(&h), before);
}

#[test]
fn fresh_job_cross_session_list_wait_cancel_are_isolated() {
    let h = fixture();
    let (job, release) = held(&h);
    for (id, params) in [
        ("job.list", json!({"all":true})),
        ("job.wait", json!({"jobId":job,"timeoutMs":0})),
        ("job.cancel", json!({"jobId":job})),
    ] {
        let e = h.invoke(Invocation {
            command: id.into(),
            params,
            options: Default::default(),
            origin: agent(&[tt_registry::Scope::Read, tt_registry::Scope::Ui]),
        });
        assert!(e.is_error(), "{id}: {e:?}");
    }
    release.send(()).unwrap();
    assert_eq!(h.jobs().wait(&job, Duration::from_secs(5)).unwrap().0.state, tt_executor::JobState::Succeeded);
}

#[test]
fn fresh_mode_workspace_and_renamed_builtin_reset_are_coherent() {
    let h = fixture();
    ok(&h, "app.mode.set", json!({"mode":"staging","viewId":"sideView"}));
    assert_eq!(ok(&h, "app.state.get", json!({}))["activeView"]["type"], "side");
    let layout = ok(&h, "workspace.get", json!({}));
    ok(&h, "workspace.select", json!({"name":"Focused drawing"}));
    assert_eq!(ok(&h, "app.state.get", json!({}))["mode"], "drawing");
    ok(&h, "workspace.select", json!({"name":"Default"}));
    assert_eq!(ok(&h, "workspace.get", json!({})), layout);
    ok(&h, "workspace.rename", json!({"name":"Default","newName":"R2 Workspace"}));
    ok(&h, "workspace.reset", json!({"name":"R2 Workspace","select":true}));
    assert_eq!(ok(&h, "app.state.get", json!({}))["activeView"]["type"], "camera");
}

#[test]
fn revision_max_blocks_fresh_paste_and_keeps_queries_alive() {
    let doc = tt_model::fixture::sample_document(42).unwrap().with_revisions(u64::MAX, &[]).unwrap();
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    assert!(h.open_document(doc, None, false).is_ok());
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":[PEG]}));
    let before = canonical(&h);
    assert_eq!(err(&h, "edit.paste", json!({"context":"staging"})).code, "doc.limitExceeded");
    assert_eq!(canonical(&h), before);
    assert_eq!(rev(&h), u64::MAX);
}

#[test]
fn standalone_m1_element_types_roundtrip_undo_redo() {
    let all = fixture();
    let ids = ok(&all, "doc.object.find", json!({"type":"element","limit":1000}))["items"].as_array().unwrap().clone();
    for item in ids {
        let h = fixture();
        let id = item["id"].as_str().unwrap();
        // Effect parameters require their effect parent; the complete forest is checked separately.
        if get(&h, id)["elementType"] == "effectParameter" {
            continue;
        }
        let before = canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>();
        ok(&h, "edit.copy", json!({"context":"staging","objectIds":[id]}));
        let pasted = ok(&h, "edit.paste", json!({"context":"staging"}));
        let created = pasted["createdIds"][0].as_str().unwrap();
        assert_eq!(get(&h, created)["elementType"], get(&h, id)["elementType"]);
        let after = canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>();
        ok(&h, "edit.undo", json!({}));
        assert_eq!(canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>(), before);
        ok(&h, "edit.redo", json!({}));
        assert_eq!(canonical(&h).iter().map(|x| normal(serde_json::from_str(x).unwrap())).collect::<Vec<_>>(), after);
    }
}

#[test]
fn copying_a_peg_includes_descendants_per_scene_edit_rules() {
    let h = fixture();
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":[PEG]}));
    let clip = ok(&h, "edit.clipboard.get", json!({"includeData":true}));
    let bob = get(&h, BOB)["id"].clone();
    assert!(clip["items"].as_array().unwrap().iter().any(|i| i["id"] == bob), "05 section 15: {clip}");
}

#[test]
fn cutting_a_peg_removes_descendants_per_scene_edit_rules() {
    let h = fixture();
    ok(&h, "edit.cut", json!({"context":"staging","objectIds":[PEG]}));
    assert!(run(&h, "doc.object.get", json!({"ref":BOB})).is_error(), "05 section 15: descendants are cut too");
}

#[test]
fn copying_complete_scene_forest_preserves_effect_parentage_on_paste() {
    let h = fixture();
    let ids: Vec<Value> = ok(&h, "doc.object.find", json!({"type":"element","limit":1000}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone())
        .collect();
    ok(&h, "edit.copy", json!({"context":"staging","objectIds":ids}));
    let pasted = run(&h, "edit.paste", json!({"context":"staging"}));
    assert!(pasted.is_ok(), "05 section 15: selected forest already contains every required effect parent: {pasted:?}");
}

#[test]
fn sequential_batch_job_wait_honours_wait_timeout_contract() {
    let h = fixture();
    let (job, release) = held(&h);
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        release.send(()).unwrap();
    });
    let e = batch(
        &h,
        json!({"label":"R2 wait contract","atomic":false,"steps":[
            {"id":"job.wait","params":{"jobId":job,"timeoutMs":2000}}
        ]}),
    );
    worker.join().unwrap();
    assert!(e.is_ok(), "{e:?}");
    assert_eq!(e.steps.unwrap()[0].result.as_ref().unwrap()["state"], "succeeded", "a batched wait must wait too");
}

#[test]
fn checkpoint_between_edits_captures_its_staged_content() {
    let h = fixture();
    let e = batch(
        &h,
        json!({"label":"R2 checkpoint midway","steps":[
            {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"phase":460}}}},
            {"id":"checkpoint.create","params":{"name":"r2-middle"}},
            {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"phase":461}}}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    assert_eq!(get(&h, BOB)["meta"]["phase"], 461);
    ok(&h, "checkpoint.restore", json!({"name":"r2-middle"}));
    assert_eq!(get(&h, BOB)["meta"]["phase"], 460, "R1-10 binding: checkpoint stores staged content at its step");
}

#[test]
fn cross_bank_cell_paste_cannot_overwrite_a_locked_drawing() {
    let h = fixture();
    ok(&h, "edit.copy", json!({"context":"xsheet","objectIds":[format!("{BOB}/cell:#1")]}));
    let clone = ok(&h, "edit.pasteAsNewElement", json!({"frame":1}))["elementId"].as_str().unwrap().to_owned();
    let drawing = get(&h, &format!("{BOB}/drawing:\"1\""))["id"].as_str().unwrap().to_owned();
    ok(
        &h,
        "doc.object.patch",
        json!({"targets":[format!("{BOB}/drawing:\"1\"/layer:#1/stroke:#1")],"props":{"meta":{"keepLocked":987}}}),
    );
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
    let clip = json!({"kind":"cells","itemCount":1,"items":[{"elementId":clone,"frameOffset":0,"drawingRef":get(&h,&clone)["exposure"][0][0]}]});
    ok(&h, "edit.clipboard.set", json!({"content":clip}));
    let e = run(&h, "edit.paste", json!({"context":"xsheet","destination":{"elementId":BOB,"frame":1}}));
    assert_eq!(e.error.as_ref().map(|e| e.code.as_str()), Some("drawing.locked"), "04 section 13.16: {e:?}");
}

#[test]
fn json04_materialised_parent_is_removed_by_raw_patch_inverse() {
    let before = get(&fixture(), BOB);
    assert!(before.get("meta").is_none());
    let (after, inverse) = tt_model::change::apply_with_inverse(
        &before,
        &[tt_model::change::PatchOp {
            op: "add",
            path: "/meta/r2".into(),
            from: None,
            range: None,
            value: Some(json!(463)),
            old_value: None,
            elided: false,
        }],
    )
    .unwrap();
    assert_eq!(
        tt_model::change::apply(&after, &inverse).unwrap(),
        before,
        "C1.1: raw apply_with_inverse restores exactly"
    );
}

#[test]
fn safe_revision_reconstruction_cannot_install_future_object_revisions() {
    let original = tt_model::fixture::sample_document(0xc122_9471).unwrap();
    let candidate = match original.at_earlier_revision(0, &[]) {
        Ok(candidate) => candidate,
        Err(_) => return, // Rejecting the invalid reconstruction is also correct.
    };
    let h = ExecutorHandle::spawn(Default::default()).unwrap();
    let loaded = h.open_document(candidate, None, false);
    if loaded.is_error() {
        return;
    }
    assert!(
        get(&h, BOB)["rev"].as_u64().unwrap() <= rev(&h),
        "safe public composition installed object rev=1 above documentRev=0"
    );
}

#[test]
fn fresh_historical_diff_replays_and_agrees_with_checkpoint_across_batch_steps() {
    let h = fixture();
    ok(&h, "checkpoint.create", json!({"name":"r2-history-base"}));
    let before = rev(&h);
    let e = batch(
        &h,
        json!({"label":"R2 repeated entry","steps":[
            {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"serial":811}}}},
            {"id":"doc.diff","params":{"sinceRev":before}},
            {"id":"doc.object.patch","params":{"targets":[BOB],"props":{"meta":{"serial":812}}}},
            {"id":"doc.diff","params":{"sinceRev":before}}
        ]}),
    );
    assert!(e.is_ok(), "{e:?}");
    let steps = e.steps.unwrap();
    assert!(steps[1].result.as_ref().unwrap()["changes"].to_string().contains("811"));
    assert!(steps[3].result.as_ref().unwrap()["changes"].to_string().contains("812"));
    for n in 813..1086 {
        ok(&h, "doc.object.patch", json!({"targets":[BOB],"props":{"meta":{"serial":n}}}));
    }
    let a = ok(&h, "doc.diff", json!({"sinceRev":before}));
    let b = ok(&h, "doc.diff", json!({"sinceCheckpoint":"r2-history-base"}));
    assert_eq!(a, b, "back-patch reconstruction beyond 256 snapshots agrees with retained checkpoint");
    let old = ok(&h, "doc.diff", json!({"sinceRev":before,"untilRev":before+1}));
    assert!(old["changes"].to_string().contains("812"));
    assert!(old["changes"].as_array().unwrap().iter().all(|c| c["rev"] == before + 1));
}

#[test]
fn eight_fresh_clients_mix_object_guards_and_atomic_batches() {
    use std::sync::{Arc, Barrier};
    let h = fixture();
    let barrier = Arc::new(Barrier::new(8));
    let initial = rev(&h);
    let threads: Vec<_> = (0..8)
        .map(|client| {
            let h = h.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let origin = Origin::Agent {
                    session: format!("r2-cas-{client}"),
                    name: format!("R2 {client}"),
                    scopes: [tt_registry::Scope::Read, tt_registry::Scope::Edit].into_iter().collect(),
                };
                barrier.wait();
                let mut committed = Vec::new();
                for i in 0..37 {
                    let mut complete = false;
                    for _ in 0..2000 {
                        let obj = get(&h, BOB);
                        let n = obj["meta"]["counter"].as_u64().unwrap_or(9461) + 1;
                        let p = json!({"targets":[BOB],"props":{"meta":{"counter":n}},"expectRev":{BOB:obj["rev"]}});
                        let e = if (client + i) % 3 == 0 {
                            h.batch(
                                serde_json::from_value(
                                    json!({"label":"R2 CAS","steps":[{"id":"doc.object.patch","params":p}]}),
                                )
                                .unwrap(),
                                origin.clone(),
                            )
                        } else {
                            h.invoke(Invocation {
                                command: "doc.object.patch".into(),
                                params: p,
                                options: Default::default(),
                                origin: origin.clone(),
                            })
                        };
                        if e.is_ok() {
                            committed.push(e.document_rev.unwrap());
                            complete = true;
                            break;
                        }
                        let error = e.error.unwrap();
                        assert!(
                            error.code == "rev.conflict"
                                || (error.code == "batch.stepFailed"
                                    && error.details["error"]["code"] == "rev.conflict"),
                            "{error:?}"
                        );
                    }
                    assert!(complete, "CAS retry exhausted");
                }
                committed
            })
        })
        .collect();
    let mut committed: Vec<u64> = threads.into_iter().flat_map(|t| t.join().unwrap()).collect();
    committed.sort_unstable();
    assert_eq!(committed, ((initial + 1)..=(initial + 296)).collect::<Vec<_>>());
    assert_eq!(get(&h, BOB)["meta"]["counter"], 9757);
}
