//! Command schemas for the front end (C1.2 IPC ticket): the parameter and result schemas of every typed command
//! declaration and the command result envelope, published as `desktop/src/ipc/generated/commands.ts` (kept in sync
//! by `crates/engine/tests/frontend_schemas.rs`) and validated there by the same validator and wire controls as the
//! model and wire schemas (lossless wide integers, finite doubles, strict text and depth, own-property and snapshot
//! checks; ADR-0012).
//!
//! Every declaration's schema is a self-contained root with its own `$defs`; definition names repeat across commands
//! (`NameParams` in several namespaces), so each command's definitions are renamed `<commandId>.<Name>` and its root
//! schemas become `<commandId>.params` and `<commandId>.result` in one shared table.

use serde_json::{Map, Value, json};
use tt_registry::{Kind, Status};

/// Rewrites `#/$defs/<Name>` references to `#/$defs/<prefix>.<Name>`, and the root self-reference `#` (a recursive
/// type such as the envelope's `steps`) to `#/$defs/<root>`.
fn rename_refs(v: &mut Value, prefix: &str, root: &str) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(r)) = m.get_mut("$ref") {
                if let Some(name) = r.strip_prefix("#/$defs/") {
                    *r = format!("#/$defs/{prefix}.{name}");
                } else if r == "#" {
                    *r = format!("#/$defs/{root}");
                }
            }
            m.values_mut().for_each(|x| rename_refs(x, prefix, root));
        }
        Value::Array(a) => a.iter_mut().for_each(|x| rename_refs(x, prefix, root)),
        _ => {}
    }
}

/// Moves a root schema's `$defs` into `defs` under `<prefix>.<Name>` and stores the root as `<prefix>.<suffix>`.
fn add_root(defs: &mut Map<String, Value>, prefix: &str, suffix: &str, mut root: Value) {
    rename_refs(&mut root, prefix, &format!("{prefix}.{suffix}"));
    if let Value::Object(m) = &mut root {
        m.shift_remove("$schema");
        if let Some(Value::Object(inner)) = m.shift_remove("$defs") {
            for (name, d) in inner {
                defs.insert(format!("{prefix}.{name}"), d);
            }
        }
    }
    defs.insert(format!("{prefix}.{suffix}"), root);
}

/// The definitions table (`<commandId>.params`, `<commandId>.result`, their nested definitions, and `envelope.*`) and
/// the command index (`{id: {kind, status, milestone?, params, result, examples}}`).
#[must_use]
pub fn command_definitions() -> (Map<String, Value>, Map<String, Value>) {
    let mut defs = Map::new();
    let mut index = Map::new();
    add_root(&mut defs, "envelope", "root", tt_registry::schema_of::<tt_executor::Envelope>());
    let Ok(registry) = tt_executor::registry() else { return (defs, index) };
    for e in registry.entries() {
        let Some(c) = e.command else { continue };
        let id = e.id();
        add_root(&mut defs, id, "params", e.params_schema());
        add_root(&mut defs, id, "result", e.result_schema());
        let examples: Vec<Value> =
            c.decl.examples.iter().map(|x| serde_json::from_str(x.params).unwrap_or(Value::Null)).collect();
        let mut entry = json!({
            "kind": e.spec.kind.as_str(),
            "params": format!("{id}.params"),
            "result": format!("{id}.result"),
            "examples": examples,
        });
        if let (Status::NotImplemented { owner }, Some(m)) = (e.status(), entry.as_object_mut()) {
            m.insert("milestone".into(), json!(owner));
        }
        if e.spec.kind == Kind::Event
            && let Some(m) = entry.as_object_mut()
        {
            m.insert("event".into(), json!(true));
        }
        index.insert(id.to_owned(), entry);
    }
    (defs, index)
}
