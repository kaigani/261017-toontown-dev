//! The generated declarations (`scripts/gen-command-declarations.py`; C12-R1-12) are current with spec A and
//! complete: every schema compiles and constrains, every example validates, every entry names its owner, its scopes
//! and its GUI location or agent-only justification.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test")]

use serde_json::{Value, json};
use tt_registry::declared::DECLARED;
use tt_registry::inventory::INVENTORY;
use tt_registry::{Kind, Scope, Status, declared_schema, owner_of, validate_params};

#[test]
fn declarations_follow_the_current_spec_columns() {
    assert!(DECLARED.len() > 600, "{}", DECLARED.len());
    for (i, c) in DECLARED.iter().enumerate() {
        let spec =
            INVENTORY.iter().find(|e| e.id == c.decl.id).unwrap_or_else(|| panic!("{} not in spec A", c.decl.id));
        assert_eq!(spec.kind, c.decl.kind, "{}", c.decl.id);
        // Regenerate after a spec change: the schemas carry the spec text they were derived from.
        assert_eq!(declared_schema(i, false)["x-spec"], json!(spec.params), "{}: params column changed", c.decl.id);
        assert_eq!(declared_schema(i, true)["x-spec"], json!(spec.result), "{}: result column changed", c.decl.id);
    }
}

#[test]
fn schemas_compile_reject_unknown_parameters_and_accept_the_examples() {
    for (i, c) in DECLARED.iter().enumerate() {
        let id = c.decl.id;
        let params = declared_schema(i, false);
        let result = declared_schema(i, true);
        for s in [&params, &result] {
            assert_eq!(s["$schema"], "https://json-schema.org/draft/2020-12/schema", "{id}");
            tt_registry::schema_violations(s, &Value::Null, 1).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        }
        assert_eq!(params["type"], "object", "{id}");
        assert_eq!(params["additionalProperties"], false, "{id}: unknown parameters are rejected (11 §8)");
        assert!(!c.decl.examples.is_empty(), "{id}");
        let mut with_unknown: Value = serde_json::from_str(c.decl.examples[0].params).unwrap();
        with_unknown["__unknown"] = json!(1);
        let unknown = validate_params(id, &params, &with_unknown).unwrap_err();
        assert_eq!(unknown.code, "arg.unknown", "{id}");
        for ex in c.decl.examples {
            let v: Value = serde_json::from_str(ex.params).unwrap();
            validate_params(id, &params, &v).unwrap_or_else(|e| panic!("{id}: example does not validate: {e:?}"));
        }
        if let Some(job) = result.get("x-jobResult") {
            assert_eq!(c.decl.kind, Kind::Job, "{id}");
            let mut s = job.clone();
            s["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
            tt_registry::schema_violations(&s, &Value::Null, 1).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        }
    }
}

#[test]
fn declarations_carry_owner_scopes_project_rule_undo_and_gui() {
    for c in DECLARED {
        let d = &c.decl;
        let Status::NotImplemented { owner } = d.status else { panic!("{}: generated declarations are stubs", d.id) };
        assert_eq!(owner, owner_of(d.id), "{}", d.id);
        assert!(owner.contains(" C"), "{}: owner names a checkpoint: {owner}", d.id);
        assert!(!d.scopes.is_empty(), "{}", d.id);
        let spec = INVENTORY.iter().find(|e| e.id == d.id).unwrap();
        assert_eq!(d.undo.is_some(), spec.undo == "yes", "{}: undo column", d.id);
        if d.kind == Kind::Event {
            assert!(d.agent_only.is_some() && !d.needs_project, "{}", d.id);
        } else {
            assert!(d.gui.is_some_and(|g| g.len() > 10), "{}: GUI location", d.id);
        }
    }
    let get = |id: &str| DECLARED.iter().find(|c| c.decl.id == id).unwrap().decl;
    assert!(get("export.movie.run").scopes.contains(&Scope::FilesWrite));
    assert!(get("import.file.run").scopes.contains(&Scope::FilesRead));
    assert!(get("prefs.set").scopes.contains(&Scope::Prefs));
    assert!(get("export.publish.run").scopes.contains(&Scope::Network));
    assert!(get("print.run").scopes.contains(&Scope::Print));
    assert!(!get("prefs.set").needs_project, "01 §1.3: preferences work without a project");
    assert!(get("element.create").needs_project);
    assert_eq!(owner_of("import.movie.run"), "M8 C8.3");
    assert_eq!(owner_of("anim.key.add"), "M3 C3.2");
}

#[test]
fn parameter_constraints_come_from_the_spec() {
    let at = |id: &str| declared_schema(DECLARED.iter().position(|c| c.decl.id == id).unwrap(), false);
    let p = at("cell.select");
    assert_eq!(p["properties"]["mode"]["enum"], json!(["replace", "add"]));
    assert_eq!(p["properties"]["mode"]["default"], "replace");
    assert_eq!(p["required"], json!(["elementIds", "from", "to"]));
    assert_eq!(p["properties"]["elementIds"]["type"], "array");
    let e =
        validate_params("cell.select", &p, &json!({ "elementIds": [], "from": 1, "to": 2, "mode": "x" })).unwrap_err();
    assert_eq!(e.code, "arg.invalid");
    let p = at("lipsync.settings.set");
    assert_eq!(p["properties"]["minHoldFrames"]["minimum"], 0);
    assert_eq!(p["properties"]["minHoldFrames"]["maximum"], 6);
    assert!(validate_params("lipsync.settings.set", &p, &json!({ "minHoldFrames": 7 })).is_err());
    let p = at("timeline.currentFrame.set");
    assert_eq!(p["properties"]["frame"]["minimum"], 1);
    let p = at("anim.camera.create");
    assert_eq!(p["properties"]["fov"]["minimum"], 1);
    assert_eq!(p["properties"]["fov"]["maximum"], 179);
    assert_eq!(p["properties"]["fov"]["default"], 41.1121);
}
