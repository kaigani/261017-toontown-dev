//! C1.2 registry gates: the assembled registry covers spec A, every M1 command has a typed declaration with schemas
//! and examples, stubs name their owners, the UI-binding manifest's ids exist (11 §14.1 parity, both directions for
//! typed declarations), and the drawing-command delegation ruling holds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

use serde_json::{Value, json};
use tt_registry::{DELEGATES, Kind, Status, validate_params};

const M1_NAMESPACES: &[&str] = &["edit", "doc", "project", "app", "agent", "inspect", "checkpoint", "job"];

fn registry() -> &'static tt_registry::Registry {
    tt_executor::registry().expect("registry assembles")
}

#[test]
fn registry_covers_spec_a_and_types_every_m1_command() {
    let r = registry();
    assert_eq!(r.len(), 796, "every spec A entry is registered");
    assert_eq!(tt_registry::REGISTRY_VERSION, "0.1.0");
    let mut typed = 0;
    let mut missing = Vec::new();
    for e in r.entries() {
        if e.command.is_some() {
            typed += 1;
        }
        if M1_NAMESPACES.contains(&e.spec.namespace) && e.command.is_none() {
            missing.push(e.id());
        }
    }
    assert!(missing.is_empty(), "M1 commands without a typed declaration: {missing:?}");
    for ns in ["window", "workspace"] {
        let untyped: Vec<&str> =
            r.entries().filter(|e| e.spec.namespace == ns && e.command.is_none()).map(|e| e.id()).collect();
        assert!(untyped.is_empty(), "kernel-level {ns} commands without a declaration: {untyped:?}");
    }
    println!("typed declarations: {typed}");
    assert_eq!(typed, 796, "every entry is declared (C12-R1-12)");
}

#[test]
fn every_declaration_has_schemas_and_valid_examples() {
    for e in registry().entries() {
        // Every entry has a declaration (hand-written or generated; C12-R1-12): nothing is skipped.
        let c = e.command.unwrap_or_else(|| panic!("{}: no declaration", e.id()));
        let params = e.params_schema();
        let result = e.result_schema();
        assert_eq!(
            params.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "{}",
            e.id()
        );
        assert!(result.is_object(), "{}: result schema", e.id());
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&result)
            .unwrap_or_else(|err| panic!("{}: result schema does not compile: {err}", e.id()));
        assert!(!c.decl.examples.is_empty(), "{}: needs an example (11 §14.2)", e.id());
        for ex in c.decl.examples {
            let v: Value = serde_json::from_str(ex.params)
                .unwrap_or_else(|err| panic!("{}: example `{}` is not JSON: {err}", e.id(), ex.title));
            validate_params(e.id(), &params, &v)
                .unwrap_or_else(|err| panic!("{}: example `{}` does not validate: {err:?}", e.id(), ex.title));
        }
        assert_eq!(c.decl.kind, e.spec.kind, "{}", e.id());
        if c.decl.kind == Kind::Action && e.spec.undo == "yes" {
            assert!(c.decl.undo.is_some(), "{}: undoable action without an undo label", e.id());
        }
    }
}

#[test]
fn stubs_and_declarations_name_their_owners() {
    let owner = regex::Regex::new(r"^M(1[0-1]|[0-9])( C[0-9]+\.[0-9]+)?( \(.*\))?$").unwrap();
    // Declarations whose checkpoint differs from the namespace default, by brief: app.info.get is C1.4 (readiness
    // bullet 6).
    let exceptions = ["app.info.get"];
    for e in registry().entries() {
        if let Status::NotImplemented { owner: o } = e.status() {
            assert!(owner.is_match(o), "{}: owner `{o}`", e.id());
            let d = e.describe();
            assert_eq!(d["status"], "notImplemented", "{}", e.id());
            assert_eq!(d["milestone"], json!(o), "{}", e.id());
            if e.command.is_some()
                && !exceptions.contains(&e.id())
                && !e.id().starts_with("doc.")
                && !e.id().starts_with("project.")
                && !e.id().starts_with("agent.")
                && !e.id().starts_with("inspect.")
            {
                assert_eq!(o, tt_registry::owner_of(e.id()), "{}: declared owner differs from owner_of", e.id());
            }
        }
    }
    assert_eq!(registry().get("app.info.get").unwrap().status(), Status::NotImplemented { owner: "M1 C1.4" });
    // C1.3 implemented every command it owns.
    assert_eq!(registry().get("doc.validate").unwrap().status(), Status::Implemented);
    let c13: Vec<&str> = registry()
        .entries()
        .filter(|e| e.status() == Status::NotImplemented { owner: "M1 C1.3" })
        .map(|e| e.id())
        .collect();
    assert!(c13.is_empty(), "C1.3 stubs left: {c13:?}");
    assert_eq!(
        registry().get("draw.stroke.add").map(|e| e.status()),
        Some(Status::NotImplemented { owner: "M2 C2.3" })
    );
}

/// Command ids bound in the UI-binding manifest (desktop/src/ui-bindings/menu.json).
fn manifest_items() -> Vec<Value> {
    let m: Value = serde_json::from_str(include_str!("../../../desktop/src/ui-bindings/menu.json")).unwrap();
    fn walk(items: &[Value], out: &mut Vec<Value>) {
        for i in items {
            if i["kind"] == "command" {
                out.push(i.clone());
            }
            if let Some(sub) = i["items"].as_array() {
                walk(sub, out);
            }
        }
    }
    let mut out = Vec::new();
    for menu in m["menus"].as_array().unwrap() {
        walk(menu["items"].as_array().unwrap(), &mut out);
    }
    out
}

#[test]
fn ui_binding_manifest_parity() {
    let r = registry();
    let items = manifest_items();
    assert!(items.len() >= 18, "manifest command items: {}", items.len());
    for i in &items {
        let id = i["command"].as_str().unwrap();
        let e = r.get(id).unwrap_or_else(|| panic!("manifest binds unregistered command `{id}`"));
        assert_ne!(e.spec.kind, Kind::Event, "{id}: events cannot be bound");
        // 01 §1.3: the manifest's availability without a project agrees with the declaration.
        if let Some(c) = e.command {
            assert_eq!(
                c.decl.needs_project,
                !i["enabledWithoutProject"].as_bool().unwrap(),
                "{id}: needs_project vs enabledWithoutProject"
            );
        }
    }
    // Reverse direction for every entry (nothing skipped; C12-R1-12): reachable from the manifest, a declared GUI
    // location, or agentOnly with a justification (11 §14.1).
    let bound: Vec<&str> = items.iter().map(|i| i["command"].as_str().unwrap()).collect();
    let mut unreachable = Vec::new();
    for e in r.entries() {
        let c = e.command.unwrap_or_else(|| panic!("{}: no declaration", e.id()));
        let ok = bound.contains(&e.id())
            || c.decl.gui.is_some_and(|g| !g.is_empty())
            || c.decl.agent_only.is_some_and(|j| j.len() > 10);
        if !ok {
            unreachable.push(e.id());
        }
    }
    assert!(
        unreachable.is_empty(),
        "typed commands without a GUI location or agentOnly justification: {unreachable:?}"
    );
}

#[test]
fn drawing_command_delegation_ruling() {
    let r = registry();
    for (from, to, _) in DELEGATES {
        let a = r.get(from).unwrap();
        let b = r.get(to).unwrap();
        assert_eq!(a.spec.kind, b.spec.kind, "{from} and {to} are the same kind of command");
        assert_eq!(a.spec.undo, b.spec.undo);
        assert_eq!(a.describe()["delegatesTo"]["commandId"], json!(to));
    }
    // The drawing schema's x-command annotations name the 04 commands the 02 commands delegate to.
    let name = tt_model::schema::property_schema(tt_model::ObjectType::Drawing, "/name").unwrap();
    let locked = tt_model::schema::property_schema(tt_model::ObjectType::Drawing, "/locked").unwrap();
    assert_eq!(name["x-command"], "element.drawing.rename");
    assert_eq!(locked["x-command"], "element.drawingLock.set");
}

#[test]
fn describe_reports_declarations() {
    let d = registry().get("edit.undo").unwrap().describe();
    assert_eq!(d["kind"], "action");
    assert_eq!(d["status"], "implemented");
    assert_eq!(d["needsProject"], true);
    assert!(d["enablement"].as_str().unwrap().contains("undo"));
    assert_eq!(d["paramsSchema"]["properties"]["steps"]["maximum"], 1000);
    let stub = registry().get("render.frame.run").unwrap().describe();
    assert_eq!(stub["status"], "notImplemented");
    assert_eq!(stub["milestone"], "M3 C3.3");
}
