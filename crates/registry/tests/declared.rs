//! The generated declarations (`scripts/gen-command-declarations.py`; C12-R1-12) are current with spec A and
//! complete: every schema compiles and constrains, every example validates, every entry names its owner, its scopes
//! and its GUI location or agent-only justification.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test")]

use serde_json::{Value, json};
use tt_registry::declared::DECLARED;
use tt_registry::inventory::INVENTORY;
use tt_registry::{Kind, Scope, Status, declared_sample, declared_schema, owner_of, validate_params};

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
            if let Some(defs) = result.get("$defs") {
                s["$defs"] = defs.clone();
            }
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

/// Scalar leaves of a value, as JSON Pointers.
fn leaves(v: &Value, at: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            m.iter().for_each(|(k, x)| leaves(x, &format!("{at}/{}", k.replace('~', "~0").replace('/', "~1")), out))
        }
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| leaves(x, &format!("{at}/{i}"), out)),
        _ => out.push(at.to_owned()),
    }
}

/// Object positions of a value, as JSON Pointers.
fn objects(v: &Value, at: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            out.push(at.to_owned());
            m.iter().for_each(|(k, x)| objects(x, &format!("{at}/{}", k.replace('~', "~0").replace('/', "~1")), out));
        }
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| objects(x, &format!("{at}/{i}"), out)),
        _ => {}
    }
}

/// Every generated schema is closed and fully typed (C12-R2-02): its full sample validates; replacing any scalar leaf
/// by an object, adding an unknown member at any object level, or giving `null` is rejected.
#[test]
fn generated_schemas_reject_every_mutated_leaf_and_unknown_member() {
    let mut checked = (0usize, 0usize, 0usize);
    for (i, c) in DECLARED.iter().enumerate() {
        let id = c.decl.id;
        let result = declared_schema(i, true);
        let mut parts = vec![("params", declared_schema(i, false), declared_sample(i, "params"))];
        parts.push(("result", result.clone(), declared_sample(i, "result")));
        if let Some(job) = result.get("x-jobResult") {
            let mut s = job.clone();
            s["$schema"] = result["$schema"].clone();
            if let Some(defs) = result.get("$defs") {
                s["$defs"] = defs.clone();
            }
            parts.push(("jobResult", s, declared_sample(i, "jobResult")));
        }
        for (part, schema, sample) in parts {
            let rejects = |v: &Value| !tt_registry::schema_violations(&schema, v, 1).unwrap().is_empty();
            assert!(rejects(&Value::Null), "{id} {part}: accepts null");
            if sample.get("x-unsampleable").is_some() {
                // A required model object (Element, Drawing, Palette): the model schema is closed.
                assert!(rejects(&json!({ "__unknown": 1 })), "{id} {part}: accepts an unknown member");
                checked.2 += 1;
                continue;
            }
            assert!(!rejects(&sample), "{id} {part}: its sample does not validate: {sample}");
            let mut at = Vec::new();
            leaves(&sample, "", &mut at);
            for p in at {
                let mut m = sample.clone();
                *m.pointer_mut(&p).unwrap() = json!({ "__mutated": true });
                assert!(rejects(&m), "{id} {part}: leaf {p} accepts an object");
                checked.0 += 1;
            }
            let mut at = Vec::new();
            objects(&sample, "", &mut at);
            for p in at {
                let mut m = sample.clone();
                if let Some(o) = m.pointer_mut(&p).and_then(Value::as_object_mut) {
                    // An object value: refused by a closed object, and by a map whose values are typed scalars.
                    o.insert("__unknown".into(), json!({ "__mutated": true }));
                }
                assert!(rejects(&m), "{id} {part}: object {p} accepts an unknown member");
                checked.1 += 1;
            }
        }
    }
    println!("mutated leaves {}, unknown members {}, model-typed parts {}", checked.0, checked.1, checked.2);
    assert!(checked.0 > 2000 && checked.1 > 1000, "{checked:?}");
}

/// Every placeholder (a position the owning section leaves undefined) is strict and belongs to a recorded spec issue.
#[test]
fn placeholders_are_strict_and_recorded() {
    let issues =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plan/spec-issues.md"))
            .unwrap();
    let mut families = std::collections::BTreeSet::new();
    for (i, c) in DECLARED.iter().enumerate() {
        if !tt_registry::declared_placeholders(i).is_empty() {
            families.insert(c.decl.id.split('.').next().unwrap().to_owned());
        }
    }
    for f in &families {
        assert!(issues.contains(&format!("`{f}.*`")), "no spec issue records the placeholders of `{f}.*`");
    }
}
