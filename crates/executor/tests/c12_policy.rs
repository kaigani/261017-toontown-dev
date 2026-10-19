//! C1.2 rulings: the whole-object and generic-patch policy (10 AG-02), special references (11 §5.1), derived
//! projections (AG-04), persisted/parameter naming rulings (SI-003, SI-004), permissions, argument validation and
//! command availability without a project (01 §1.3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Value, json};
use tt_executor::{ExecutorConfig, ExecutorHandle, Invocation, Status};
use tt_registry::Scope;

fn patch_err(h: &ExecutorHandle, target: &str, ops: Value) -> tt_registry::CommandError {
    err(h, "doc.object.patch", json!({ "targets": [target], "ops": ops }))
}

#[test]
fn whole_object_and_structural_patches_are_refused() {
    let h = fixture();
    let scene = first(&h, "scene");
    let element = first(&h, "element");
    let r0 = rev(&h);
    for (op, path) in [("replace", ""), ("remove", ""), ("add", "")] {
        let e = patch_err(&h, &scene, json!([{ "op": op, "path": path, "value": {} }]));
        assert_eq!(e.code, "doc.structuralChange", "{op} {path:?}");
        assert_eq!(e.details["reason"], "wholeObject");
    }
    let e = patch_err(&h, &scene, json!([{ "op": "replace", "path": "/elements", "value": [] }]));
    assert_eq!(e.code, "doc.structuralChange");
    assert_eq!(e.details["property"], "elements");
    assert!(e.details["command"].as_str().unwrap().contains("element"));
    let e = patch_err(&h, &element, json!([{ "op": "remove", "path": "/channels" }]));
    assert_eq!(e.code, "doc.structuralChange");
    let e = patch_err(&h, &scene, json!([{ "op": "move", "from": "/elements/0", "path": "/meta/x" }]));
    assert_eq!(e.code, "doc.structuralChange");
    assert_eq!(rev(&h), r0, "nothing changed");
}

#[test]
fn read_only_derived_transient_and_command_properties_are_refused() {
    let h = fixture();
    let scene = first(&h, "scene");
    let element = first(&h, "element");
    for (target, path, code) in [
        (&scene, "/id", "doc.readOnlyProperty"),
        (&scene, "/rev", "doc.readOnlyProperty"),
        (&scene, "/type", "doc.readOnlyProperty"),
        (&element, "/createdAt", "doc.readOnlyProperty"),
        (&element, "/elementType", "doc.readOnlyProperty"),
        (&scene, "/length", "doc.readOnlyProperty"),
        (&scene, "/path", "doc.readOnlyProperty"),
        (&element, "/parentId", "doc.readOnlyProperty"),
        (&scene, "/noSuchProperty", "doc.schemaViolation"),
    ] {
        let e = patch_err(&h, target, json!([{ "op": "replace", "path": path, "value": 1 }]));
        assert_eq!(e.code, code, "{path}");
    }
    // x-command properties follow their command; until it exists the patch names it and its owner.
    let e = err(&h, "doc.object.patch", json!({ "targets": [element], "props": { "name": "Bob" } }));
    assert_eq!(e.code, "command.notImplemented");
    assert_eq!(e.details["commandId"], "element.rename");
    assert_eq!(e.details["milestone"], "M3 C3.1");
    let e = patch_err(&h, &element, json!([{ "op": "replace", "path": "/transform/offset/x", "value": 1 }]));
    assert_eq!(e.details["commandId"], "anim.transform.set");
    // Replacing a value object that contains x-command properties is the same.
    let t = get(&h, &element)["transform"].clone();
    let e = patch_err(&h, &element, json!([{ "op": "replace", "path": "/transform", "value": t }]));
    assert_eq!(e.details["commandId"], "anim.transform.set");
    // `test` reads anything.
    let id = get(&h, &scene)["id"].clone();
    assert!(run(&h, "doc.object.patch", json!({ "targets": [scene], "ops": [{ "op": "test", "path": "/id", "value": id }, { "op": "add", "path": "/meta", "value": { "ok": true } }] })).is_ok());
    let e = patch_err(&h, &scene, json!([{ "op": "test", "path": "/meta/ok", "value": false }]));
    assert_eq!(e.code, "doc.patchTestFailed");
}

#[test]
fn value_collections_are_patchable_and_validated() {
    let h = fixture();
    let scene = first(&h, "scene");
    let env = run(
        &h,
        "doc.object.patch",
        json!({ "targets": [scene], "props": { "meta": { "list": [1, 2, 3], "nested": { "a": null } } } }),
    );
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(get(&h, &scene)["meta"], json!({ "list": [1, 2, 3], "nested": {} }), "RFC 7386 drops nulls");
    assert!(run(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "list": null } } })).is_ok());
    assert!(get(&h, &scene)["meta"].get("list").is_none(), "null removes");
    // Schema violations are refused before anything changes.
    let drawing = first(&h, "drawing");
    let e = err(&h, "doc.object.patch", json!({ "targets": [drawing], "props": { "mouthShape": "Q" } }));
    assert_eq!(e.code, "doc.schemaViolation");
    // Several targets change atomically in one step.
    let palette = first(&h, "palette");
    let env =
        run(&h, "doc.object.patch", json!({ "targets": [scene, palette], "props": { "meta": { "both": true } } }));
    assert_eq!(env.changes.len(), 2);
    assert_eq!(ok(&h, "edit.history.get", json!({}))["total"], 3);
}

#[test]
fn locked_drawings_refuse_content_patches() {
    let mut parts = tt_model::fixture::sample_parts(1).unwrap();
    let mut v = serde_json::to_value(&parts.drawings[0]).unwrap();
    v["locked"] = json!(true);
    parts.drawings[0] = tt_model::object::from_json(&v).unwrap();
    let doc = tt_model::document::Document::from_parts(parts).unwrap();
    let h = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    assert!(h.open_document(doc.clone(), None, false).is_ok());
    let d = doc.drawings().find(|d| *d.locked()).unwrap().id().as_str().to_owned();
    let e = err(&h, "doc.object.patch", json!({ "targets": [d], "props": { "mouthShape": "A" } }));
    assert_eq!(e.code, "drawing.locked");
}

#[test]
fn naming_rulings_hold_in_patches() {
    let h = fixture();
    let pen = first(&h, "pen");
    // SI-004: persisted names `tip`, `tipAngle`, `kind`; the command-parameter names are not properties.
    assert!(
        run(&h, "doc.object.patch", json!({ "targets": [pen], "props": { "tip": "square", "tipAngle": 30 } })).is_ok()
    );
    let p = get(&h, &pen);
    assert_eq!((p["tip"].as_str(), p["tipAngle"].as_f64()), (Some("square"), Some(30.0)));
    for wrong in ["tipShape", "angle"] {
        let e = err(&h, "doc.object.patch", json!({ "targets": [pen], "props": { wrong: 1 } }));
        assert_eq!(e.code, "doc.schemaViolation", "{wrong}");
    }
    let e = err(&h, "doc.object.patch", json!({ "targets": [pen], "props": { "type": "eraser" } }));
    assert_eq!(e.code, "doc.readOnlyProperty", "`type` is the envelope's");
    // SI-003: the persisted role enum is the superset.
    let element = first(&h, "element");
    for role in ["normal", "foreground", "background", "envelope"] {
        assert!(
            run(&h, "doc.object.patch", json!({ "targets": [element], "props": { "role": role } })).is_ok(),
            "{role}"
        );
    }
    // The declared examples carry the same rulings.
    let d = h.describe_command("doc.object.patch").unwrap();
    let examples = serde_json::to_string(&d["examples"]).unwrap();
    assert!(examples.contains("tipAngle") && examples.contains("foreground") && !examples.contains("tipShape"));
}

#[test]
fn special_references_resolve_against_the_context() {
    let h = fixture();
    let st = ok(&h, "app.state.get", json!({}));
    let element = st["element"]["id"].as_str().unwrap().to_owned();
    let scene = st["scene"]["id"].as_str().unwrap().to_owned();
    let items = ok(
        &h,
        "doc.object.resolve",
        json!({ "refs": ["@current", "@activeView", "@selection", "@activeCamera"] }),
    )["items"]
        .clone();
    assert_eq!(items[0]["id"], json!(element), "@current is the current element");
    assert_eq!(items[1]["id"], json!(scene), "@activeView of the camera view is the scene");
    assert_eq!(items[2]["error"]["code"], "ref.notFound", "nothing selected");
    let camera = items[3].clone();
    // Selection.
    ok(&h, "edit.selection.set", json!({ "context": "staging", "ids": [element] }));
    let items = ok(&h, "doc.object.resolve", json!({ "refs": ["@selection"] }))["items"].clone();
    assert_eq!(items[0]["id"], json!(element));
    let got = ok(&h, "doc.object.get", json!({ "refs": ["@selection", "@current"] }));
    assert_eq!(got["objects"].as_array().unwrap().len(), 1, "duplicates collapse");
    // A typed parameter's bare @current is the current object of that type.
    ok(&h, "app.context.set", json!({ "sceneId": "@current", "elementId": "@current" }));
    if camera.get("id").is_some() {
        assert_eq!(camera["type"], "element");
    }
    // Paths may start at @current.
    let drawings = ok(&h, "doc.object.find", json!({ "type": "drawing", "within": "@current" }));
    let _ = drawings;
    let st = ok(&h, "app.state.get", json!({}));
    if let Some(d) = st["drawing"]["name"].as_str() {
        let r = ok(&h, "doc.object.resolve", json!({ "refs": [format!("@current/drawing:\"{d}\"")] }));
        assert_eq!(r["items"][0]["id"], st["drawing"]["id"]);
    }
}

#[test]
fn derived_properties_are_projected_on_request() {
    let h = fixture();
    let element = first(&h, "element");
    let plain = get(&h, &element);
    assert!(plain.get("parentId").is_none(), "derived data only on request");
    let env = run(&h, "doc.object.get", json!({ "ref": element, "include": ["derived"] }));
    let o = env.result.as_ref().unwrap()["object"].clone();
    for k in [
        "parentId",
        "childIds",
        "depth",
        "timelineIndex",
        "xsheetIndex",
        "cloneGroupId",
        "cloneIds",
        "drawingCount",
        "hiddenByParent",
    ] {
        assert!(o.get(k).is_some(), "element.{k}");
    }
    assert!(o.get("effectiveMarkerColour").is_none());
    let w = env.warnings.iter().find(|w| w.code == "doc.derivedUnavailable").unwrap();
    assert!(w.message.contains("effectiveMarkerColour (M4 C4.1"));
    let scene = first(&h, "scene");
    let s = ok(&h, "doc.object.get", json!({ "ref": scene, "include": ["derived"] }))["object"].clone();
    assert!(s["length"].as_u64().unwrap() >= 1);
    let drawing = first(&h, "drawing");
    let d = ok(&h, "doc.object.get", json!({ "ref": drawing, "include": ["derived", "stats"] }));
    assert!(d["object"]["usage"].is_array() && d["object"]["isEmpty"].is_boolean());
    assert!(d["stats"][0]["jsonBytes"].as_u64().unwrap() > 0);
    // Every derived projection validates against its type schema (DM-1 plus the query-only properties).
    for (r, ty) in [(&element, tt_model::ObjectType::Element), (&scene, tt_model::ObjectType::Scene)] {
        let o = ok(&h, "doc.object.get", json!({ "ref": r, "include": ["derived"], "depth": -1 }))["object"].clone();
        let v = tt_model::schema::validate(ty, &o, 20).unwrap();
        assert!(v.is_empty(), "{ty}: {v:?}");
    }
}

#[test]
fn queries_find_resolve_and_page() {
    let h = fixture();
    let all = ok(&h, "doc.object.find", json!({ "type": "element", "limit": 1000 }));
    let total = all["total"].as_u64().unwrap();
    assert!(total >= 2);
    let page = ok(&h, "doc.object.find", json!({ "type": "element", "limit": 1, "offset": 1 }));
    assert_eq!(page["items"][0], all["items"][1]);
    assert_eq!(page["nextOffset"], if total > 2 { json!(2) } else { Value::Null });
    let drawings = ok(&h, "doc.object.find", json!({ "type": "element", "where": { "elementType": "drawing" } }));
    assert!(drawings["items"].as_array().unwrap().iter().all(|i| i["type"] == "element"));
    let none = ok(
        &h,
        "doc.object.find",
        json!({ "type": "element", "where": { "anyOf": [{ "elementType": "nope" }, { "locked": true }] } }),
    );
    assert_eq!(none["total"], 0);
    let re =
        ok(&h, "doc.object.find", json!({ "type": "scene", "where": { "name": { "op": "regex", "value": "^.+$" } } }));
    assert!(re["total"].as_u64().unwrap() >= 1);
    let e = err(&h, "doc.object.find", json!({ "where": { "name": { "op": "near", "value": 1 } } }));
    assert_eq!(e.code, "arg.invalid");
    let stroke = first(&h, "stroke");
    let drawing = stroke.split_once('/').unwrap().0;
    let paged = ok(
        &h,
        "doc.object.get",
        json!({ "ref": drawing, "depth": -1, "page": { "pointer": "/nodes", "offset": 0, "limit": 1 } }),
    );
    assert_eq!(paged["object"]["nodes"].as_array().unwrap().len(), 1);
    assert!(paged["page"][0]["total"].as_u64().unwrap() >= 1);
}

#[test]
fn permissions_validation_and_availability() {
    let h = fixture();
    let scene = first(&h, "scene");
    let reader = agent(&[Scope::Read]);
    let env = h.invoke(Invocation {
        command: "doc.object.patch".into(),
        params: json!({ "targets": [scene], "props": { "meta": {} } }),
        options: Default::default(),
        origin: reader.clone(),
    });
    let e = env.error.unwrap();
    assert_eq!(e.code, "permission.denied");
    assert_eq!(e.details["missingScopes"], json!(["edit"]));
    let env = h.invoke(Invocation {
        command: "app.state.get".into(),
        params: json!({}),
        options: Default::default(),
        origin: reader.clone(),
    });
    assert!(env.is_ok());
    let env = h.invoke(Invocation {
        command: "app.quit".into(),
        params: json!({}),
        options: Default::default(),
        origin: agent(&[Scope::Read, Scope::Edit, Scope::Ui]),
    });
    assert_eq!(env.error.unwrap().code, "permission.denied", "`app` scope is never an agent's");
    let listed = h.list_commands(reader).unwrap();
    let patch = listed.iter().find(|c| c["id"] == "doc.object.patch").unwrap();
    assert_eq!(patch["enabled"], false);
    assert!(patch["disabledReason"].as_str().unwrap().contains("edit"));
    // Arguments.
    let e = err(&h, "nope.command", json!({}));
    assert_eq!(e.code, "command.unknown");
    let e = err(&h, "app.frame.set", json!({ "frame": 1, "extra": 1 }));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("arg.unknown", Some("/extra")));
    let e = err(&h, "app.frame.set", json!({}));
    assert_eq!((e.code.as_str(), e.details["pointer"].as_str()), ("arg.missing", Some("/frame")));
    let e = err(&h, "app.frame.set", json!({ "frame": 0 }));
    assert_eq!(e.code, "arg.invalid");
    assert_eq!(e.details["allowed"]["minimum"], 1);
    let e = err(&h, "edit.undo", json!([1]));
    assert_eq!(e.code, "arg.invalid");
    let e = err(&h, "app.state.changed", json!({}));
    assert_eq!(e.code, "command.disabled", "events are not invocable");
    let e = err(&h, "draw.stroke.add", json!({}));
    assert_eq!(e.code, "command.notImplemented");
    assert_eq!(e.details["milestone"], "M2 C2.3");
    // Without a project (01 §1.3).
    let empty = ExecutorHandle::spawn(ExecutorConfig::default()).unwrap();
    assert_eq!(err(&empty, "doc.object.get", json!({ "ref": "scene:#1" })).code, "project.none");
    assert_eq!(err(&empty, "edit.undo", json!({})).code, "project.none");
    assert!(run(&empty, "window.panel.list", json!({})).is_ok());
    assert!(run(&empty, "workspace.select", json!({ "name": "Drawing" })).is_ok());
    let st = ok(&empty, "app.state.get", json!({}));
    assert_eq!((st["project"].clone(), st["startScreenVisible"].clone()), (Value::Null, json!(true)));
    assert_eq!(run(&empty, "app.quit", json!({})).status, Status::Ok);
}

#[test]
fn batches_resolve_references_and_refuse_jobs_and_history() {
    let h = fixture();
    let scene = first(&h, "scene");
    let env = batch(
        &h,
        json!({ "label": "refs", "steps": [
        { "id": "doc.object.find", "params": { "type": "scene", "limit": 1 }, "as": "s" },
        { "id": "doc.object.patch", "params": { "targets": ["$s.result.items.0.id"], "props": { "meta": { "via": "name" } } } },
        { "id": "doc.object.get", "params": { "ref": "$1.result.items.0.id" } }
    ] }),
    );
    assert!(env.is_ok(), "{:?}", env.error);
    assert_eq!(env.steps.as_ref().unwrap()[2].result.as_ref().unwrap()["object"]["meta"]["via"], "name");
    let e =
        batch(&h, json!({ "label": "bad", "steps": [{ "id": "doc.object.get", "params": { "ref": "$9.result.x" } }] }))
            .error
            .unwrap();
    assert_eq!(e.code, "batch.stepFailed");
    assert_eq!(e.details["error"]["code"], "arg.invalid");
    let e = batch(&h, json!({ "label": "job", "steps": [{ "id": "project.save", "params": {} }] })).error.unwrap();
    assert_eq!((e.code.as_str(), e.details["stepIndex"].as_u64()), ("batch.jobNotAllowed", Some(1)));
    let e = batch(&h, json!({ "label": "undo", "steps": [{ "id": "edit.undo", "params": {} }] })).error.unwrap();
    assert_eq!(e.details["error"]["code"], "command.disabled");
    // Non-atomic batches commit step by step and may continue after an error.
    let r = rev(&h);
    let env = batch(
        &h,
        json!({ "label": "seq", "atomic": false, "stopOnError": false, "steps": [
        { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "s": 1 } } } },
        { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "id": "x" } } },
        { "id": "doc.object.patch", "params": { "targets": [scene], "props": { "meta": { "s": 2 } } } }
    ] }),
    );
    assert_eq!(env.status, Status::Error);
    assert_eq!(env.steps.as_ref().unwrap().len(), 3);
    assert_eq!(rev(&h), r + 2);
}

#[test]
fn the_load_transaction_is_guarded() {
    // ADR-0013 §16 (C12-R1-01): scopes of project.open, the unsaved-changes rule of 01 §11.2, the events.
    use tt_executor::{OpenOptions, Origin, UnsavedPolicy};
    let h = fixture();
    let doc = || tt_model::fixture::sample_document(2).unwrap();
    let opts = |on_unsaved| OpenOptions { path: None, read_only: false, on_unsaved };
    let scene = first(&h, "scene");
    ok(&h, "doc.object.patch", json!({ "targets": [scene], "props": { "meta": { "dirty": 1 } } }));
    let before = rev(&h);
    // An agent needs the scopes of project.open.
    let e =
        h.open_document_with(doc(), opts(UnsavedPolicy::Discard), agent(&[Scope::Read, Scope::Edit])).error.unwrap();
    assert_eq!((e.code.as_str(), e.details["missingScopes"].clone()), ("permission.denied", json!(["files.read"])));
    // Unsaved changes: fail by default; save arrives with project.save (C1.3); discarding needs the app scope.
    let user = || Origin::User;
    assert_eq!(
        h.open_document_with(doc(), opts(UnsavedPolicy::Fail), user()).error.unwrap().code,
        "project.unsavedChanges"
    );
    assert_eq!(
        h.open_document_with(doc(), opts(UnsavedPolicy::Save), user()).error.unwrap().code,
        "command.notImplemented"
    );
    let e = h
        .open_document_with(doc(), opts(UnsavedPolicy::Discard), agent(&[Scope::Read, Scope::Edit, Scope::FilesRead]))
        .error
        .unwrap();
    assert_eq!((e.code.as_str(), e.details["missingScopes"].clone()), ("permission.denied", json!(["app"])));
    assert_eq!(rev(&h), before, "nothing replaced");
    // The user may discard: the replacement is announced and history starts afresh.
    let rx = h.subscribe();
    assert!(h.open_document_with(doc(), opts(UnsavedPolicy::Discard), user()).is_ok());
    h.snapshot().unwrap();
    let names: Vec<String> = rx.try_iter().map(|e| e.name).collect();
    assert!(names.contains(&"project.closed".to_owned()) && names.contains(&"project.opened".to_owned()), "{names:?}");
    assert_eq!(ok(&h, "edit.history.get", json!({}))["entries"], json!([]));
}
