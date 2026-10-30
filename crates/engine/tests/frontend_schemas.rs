//! The front end's schemas and wire types are generated from the Rust types (C1.1 readiness bullet 1; C1.2 command
//! schemas). This test regenerates `desktop/src/ipc/generated/{wire,model,commands}.ts` and fails when the committed
//! files differ.
//! `TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas` rewrites them.
#![allow(clippy::unwrap_used, reason = "test")]

use std::path::PathBuf;

use serde_json::json;

const HEADER: &str = "GENERATED FILE: do not edit.\nSource: Rust types via schemars (crates/engine/src/wire.rs, crates/model/src/schema.rs).\nRegenerate: TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas";

fn check(name: &str, text: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../desktop/src/ipc/generated").join(name);
    if std::env::var("TT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "{} is stale: run `TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas`",
        path.display()
    );
}

#[test]
fn wire_module_is_current() {
    let consts = [
        ("FORMAT_VERSION", json!(tt_model::FORMAT_VERSION)),
        ("REGISTRY_VERSION", json!(tt_registry::REGISTRY_VERSION)),
        ("COMMAND_ID_PATTERN", json!(tt_engine::wire::COMMAND_ID_PATTERN)),
    ];
    check("wire.ts", &tt_model::schema_ts::module(HEADER, &consts, "WIRE_DEFS", &tt_engine::wire::wire_definitions()));
}

#[test]
fn model_module_is_current() {
    let types: serde_json::Map<String, serde_json::Value> = tt_model::ObjectType::ALL
        .iter()
        .map(|t| (t.as_str().to_owned(), json!(tt_model::schema::def_name(*t))))
        .collect();
    let consts = [
        ("MODEL_FORMAT_VERSION", json!(tt_model::FORMAT_VERSION)),
        ("OBJECT_TYPE_DEFS", serde_json::Value::Object(types)),
    ];
    check("model.ts", &tt_model::schema_ts::module(HEADER, &consts, "MODEL_DEFS", tt_model::schema::definitions()));
}

#[test]
fn commands_module_is_current() {
    let (defs, index) = tt_engine::commands::command_definitions();
    let consts = [
        ("COMMANDS_REGISTRY_VERSION", json!(tt_registry::REGISTRY_VERSION)),
        ("COMMAND_INDEX", serde_json::Value::Object(index)),
    ];
    let header = "GENERATED FILE: do not edit.
Source: the command declarations (crates/executor/src/commands, crates/engine/src/commands.rs).
Regenerate: TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas";
    check("commands.ts", &tt_model::schema_ts::module(header, &consts, "COMMAND_DEFS", &defs));
}
