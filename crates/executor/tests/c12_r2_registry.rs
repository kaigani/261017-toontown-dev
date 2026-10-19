//! Independent declarations audit: schema-valid does not mean specification-valid.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regressions")]
use serde_json::{Value, json};
use tt_registry::{Scope, schema_violations, validate_params};

#[test]
fn generated_result_rejects_wrong_enabled_type() {
    let e = tt_executor::registry().unwrap().get("anim.pegOnlyMode.set").unwrap();
    assert!(
        !schema_violations(&e.result_schema(), &json!({"enabled":{"wrong":"type"}}), 10).unwrap().is_empty(),
        "05 / A enabled is boolean"
    );
}

#[test]
fn generated_nested_parameters_reject_wrong_handle_type() {
    let e = tt_executor::registry().unwrap().get("anim.function.key.set").unwrap();
    assert!(
        validate_params(
            e.id(),
            &e.params_schema(),
            &json!({"nodeId":"scene:#1/element:#1","channel":"rotation","frame":12,"left":{"dt":false,"dv":[]}})
        )
        .is_err(),
        "05 §9 handles are numeric"
    );
}

#[test]
fn generated_named_result_is_not_an_unconstrained_object() {
    let e = tt_executor::registry().unwrap().get("library.tree.get").unwrap();
    assert!(
        !schema_violations(&e.result_schema(), &json!({"nodes":[{"bogus":42}]}), 10).unwrap().is_empty(),
        "08 LibraryNode shape must be declared"
    );
}

#[test]
fn library_global_write_scope_is_declared() {
    let e = tt_executor::registry().unwrap().get("library.item.delete").unwrap();
    assert!(e.scopes().contains(&Scope::LibraryGlobal), "11 §6.1 changing global library requires library.global");
}

#[test]
fn library_close_has_no_project_rule_from_01_1_3() {
    let e = tt_executor::registry().unwrap().get("library.close").unwrap();
    assert!(e.command.unwrap().decl.needs_project, "01 §1.3 only saving global library is allowed without project");
}

#[test]
fn result_schema_catalogue_does_not_accept_null_for_device_record() {
    let e = tt_executor::registry().unwrap().get("capture.device.get").unwrap();
    assert!(
        !schema_violations(&e.result_schema(), &Value::Null, 10).unwrap().is_empty(),
        "09 Device is an object, not any JSON"
    );
}

#[test]
fn independently_validate_every_declared_parameter_example() {
    let mut rows = Vec::new();
    for e in tt_executor::registry().unwrap().entries() {
        let command = e.command.unwrap();
        assert!(!command.decl.examples.is_empty(), "{} has no example", e.id());
        for example in command.decl.examples {
            let params: Value = serde_json::from_str(example.params).unwrap();
            let result = validate_params(e.id(), &e.params_schema(), &params);
            rows.push(json!({"id":e.id(),"title":example.title,"valid":result.is_ok()}));
            assert!(result.is_ok(), "{}: {result:?}", e.id());
        }
    }
    if let Ok(dir) = std::env::var("TT_C12_REVIEW_ARTIFACT_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("example-validation.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
}
