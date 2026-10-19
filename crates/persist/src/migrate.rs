//! In-memory migration of older files (10 §6.6 VER-03/VER-04).
//!
//! A migration step is a pure function from the JSON entries of one format version to those of the next. Steps never
//! delete information: anything without a new home goes under `ext["app.migrated"]` of the object it came from. The
//! product chain is [`CHAIN`]; format 1.0 is the first version, so it is empty. Hosts and tests may supply their own
//! chain (e.g. a synthetic `0.9 → 1.0` step); every step of the product chain must have fixture tests
//! (`tests/fixtures/migrations/<from>-<to>/`; `tests/migrations.rs` checks that each product step has one).

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tt_model::ModelError;

use crate::codes;
use crate::manifest::Version;

/// JSON object entries by entry path.
pub type Entries = BTreeMap<String, Value>;

/// One step `from → to`.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Version the step reads.
    pub from: Version,
    /// Version the step writes.
    pub to: Version,
    /// Name (for reports and fixture folders).
    pub name: &'static str,
    /// The pure step function.
    pub apply: fn(Entries) -> Result<Entries, String>,
}

/// The product migration chain (empty: 1.0 is the first format version).
pub const CHAIN: &[Migration] = &[];

/// Applies the steps that lead from `from` to `to`, in order.
///
/// # Errors
/// `project.unsupportedVersion` when no chain leads from `from` to `to`; `project.invalidFormat` when a step fails.
pub fn migrate(mut entries: Entries, from: Version, to: Version, chain: &[Migration]) -> Result<Entries, ModelError> {
    let mut at = from;
    let mut guard = 0;
    while at < to {
        guard += 1;
        let Some(step) = chain.iter().find(|m| m.from == at && m.to > at && m.to <= to) else {
            return Err(ModelError::with_details(
                codes::UNSUPPORTED_VERSION,
                format!("no migration from format {at} to {to}"),
                json!({ "formatVersion": from.to_string(), "readerVersion": to.to_string() }),
            ));
        };
        if guard > 1000 {
            return Err(ModelError::new(codes::INTERNAL, "migration chain does not terminate"));
        }
        entries = (step.apply)(entries).map_err(|e| {
            ModelError::with_details(
                codes::INVALID_FORMAT,
                format!("migration `{}` ({} → {}) failed: {e}", step.name, step.from, step.to),
                json!({ "reason": "migration", "step": step.name }),
            )
        })?;
        at = step.to;
    }
    Ok(entries)
}

/// Keeps a value without a new home under `ext["app.migrated"][key]` of an object (VER-03).
pub fn keep_migrated(object: &mut Value, key: &str, value: Value) {
    let Some(m) = object.as_object_mut() else { return };
    let ext = m.entry("ext").or_insert_with(|| json!({}));
    if !ext.is_object() {
        *ext = json!({});
    }
    if let Some(e) = ext.as_object_mut() {
        let slot = e.entry("app.migrated").or_insert_with(|| json!({}));
        if let Some(s) = slot.as_object_mut() {
            s.insert(key.to_owned(), value);
        }
    }
}
