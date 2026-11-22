//! One store of the Timeline's transfer options (04 §11.14, §17.3; 01 §6.3, §7.3; SI-201, ruling E-8; M5 C5.1a,
//! `reviews/M5-design-timeline-kernel.md` §4).
//!
//! Each operation (paste special, cut special, drag and drop) has one stored option set: its 01 §7.3 preference key, in
//! 01's member names, persisted per user (04 line 759). The defaults are one table, the keys' defaults in
//! `prefs.json`. `timeline.transferOptions.*` reads and writes the store in 04's names through a fixed mapping;
//! `edit.cutSpecial` and `edit.pasteSpecial` read it in 01's names and write it back when they remember; the `cell.*`
//! commands take their omitted members from it (04 line 1366).
//!
//! **One default table (ruling E-13).** 01 §6.3 turns *Overwrite existing drawings* and *Update colours* on, 04 §11.14
//! and §17.3 off; the table is 04's (interim until the user rules on SI-201), in `prefs.json` alone, so a ruling changes
//! it in one place.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, codes, command};

use crate::cx::Cx;

/// The operations with a stored option set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Operation {
    /// Paste special.
    Paste,
    /// Cut special.
    Cut,
    /// Drag and drop of cells.
    DragDrop,
}

impl Operation {
    /// The 01 §7.3 preference key of the operation's set.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Paste => "edit.pasteSpecial.timeline",
            Self::Cut => "edit.cutSpecial.timeline",
            Self::DragDrop => "edit.dragDrop.timeline",
        }
    }
}

/// 04 §17.3's flat members and their 01 §7.3 names.
const FLAT: [(&str, &str); 6] = [
    ("overwriteDrawings", "overwriteDrawings"),
    ("exposure", "createDeleteExposure"),
    ("autoExtend", "extendExposures"),
    ("updateColours", "updateColours"),
    ("keyframeSubstitution", "keyframeSubstitution"),
    ("boundaryKeys", "boundaryKeys"),
];

/// 04 §17.3's `pegKeys` members and their 01 §7.3 names.
const PEG: [(&str, &str); 6] = [
    ("motion", "motionKeys"),
    ("rotation", "rotationKeys"),
    ("scale", "scaleKeys"),
    ("pivot", "pivotKeys"),
    ("velocity", "velocityKeys"),
    ("skew", "skewKeys"),
];

/// A set in 01's names as 04's object.
pub(crate) fn to_04(v01: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for (a, b) in FLAT {
        if let Some(v) = v01.get(b) {
            out.insert(a.to_owned(), v.clone());
        }
    }
    let mut peg = Map::new();
    for (a, b) in PEG {
        if let Some(v) = v01.get(b) {
            peg.insert(a.to_owned(), v.clone());
        }
    }
    if !peg.is_empty() {
        out.insert("pegKeys".to_owned(), Value::Object(peg));
    }
    out
}

/// 04's object (possibly partial) in 01's names; an unknown member is refused (`arg.invalid`, 11 §10).
pub(crate) fn to_01(v04: &Map<String, Value>, pointer: &str) -> Result<Map<String, Value>, CommandError> {
    let mut out = Map::new();
    for (k, v) in v04 {
        if k == "pegKeys" {
            let m = v.as_object().ok_or_else(|| invalid(&format!("{pointer}/pegKeys"), "an object"))?;
            for (pk, pv) in m {
                let (_, b) = PEG
                    .iter()
                    .find(|(a, _)| a == pk)
                    .ok_or_else(|| unknown_member(&format!("{pointer}/pegKeys/{pk}")))?;
                out.insert((*b).to_owned(), boolean(pv, &format!("{pointer}/pegKeys/{pk}"))?);
            }
            continue;
        }
        let (_, b) = FLAT.iter().find(|(a, _)| a == k).ok_or_else(|| unknown_member(&format!("{pointer}/{k}")))?;
        out.insert((*b).to_owned(), boolean(v, &format!("{pointer}/{k}"))?);
    }
    Ok(out)
}

fn boolean(v: &Value, pointer: &str) -> Result<Value, CommandError> {
    if v.is_boolean() { Ok(v.clone()) } else { Err(invalid(pointer, "a boolean")) }
}

fn invalid(pointer: &str, what: &str) -> CommandError {
    CommandError::with_details(codes::ARG_INVALID, format!("`{pointer}` must be {what}"), json!({ "pointer": pointer }))
}

fn unknown_member(pointer: &str) -> CommandError {
    CommandError::with_details(
        codes::ARG_UNKNOWN,
        format!("`{pointer}` is not a transfer option"),
        json!({ "pointer": pointer }),
    )
}

/// The stored set of an operation, in 01's names (the table's default when nothing was stored).
pub(crate) fn stored(cx: &Cx, op: Operation) -> Map<String, Value> {
    cx.pref(op.key()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// The table's default set of an operation, in 01's names.
#[cfg(test)]
pub(crate) fn table_default(op: Operation) -> Map<String, Value> {
    crate::store::pref_default(op.key()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// The base a `cell.*` command overlays its given members on: the stored set in 04's names.
pub(crate) fn cell_base(cx: &Cx, op: Operation) -> Map<String, Value> {
    to_04(&stored(cx, op))
}

/// A `cell.*` command's options: its given members over [`cell_base`].
pub(crate) fn cell_options(
    cx: &Cx,
    op: Operation,
    given: Option<&Value>,
) -> Result<super::cell::TransferOptions, CommandError> {
    let mut base = cell_base(cx, op);
    if let Some(g) = given {
        let m = g.as_object().ok_or_else(|| invalid("/options", "an object"))?;
        for (k, v) in m {
            if k == "pegKeys" && v.is_object() {
                let mut peg = base.get("pegKeys").and_then(Value::as_object).cloned().unwrap_or_default();
                if let Some(pm) = v.as_object() {
                    for (pk, pv) in pm {
                        peg.insert(pk.clone(), pv.clone());
                    }
                }
                base.insert("pegKeys".to_owned(), Value::Object(peg));
            } else {
                base.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::from_value(Value::Object(base)).map_err(|e| {
        CommandError::with_details(
            codes::ARG_INVALID,
            format!("invalid options: {e}"),
            json!({ "pointer": "/options" }),
        )
    })
}

// ----- timeline.transferOptions.* -----------------------------------------------------------------------------------------

/// `timeline.transferOptions.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    /// Paste special, cut special or drag and drop.
    operation: Operation,
    /// The options to store (04 §17.3; omitted members keep their stored value).
    #[schemars(with = "super::cell::TransferOptions")]
    options: Value,
}

/// `{operation, options}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SetResult {
    operation: Operation,
    /// The stored set (04 §17.3).
    #[schemars(with = "super::cell::TransferOptions")]
    options: Value,
}

#[command(
    id = "timeline.transferOptions.set",
    kind = ui,
    label = "Set transfer options",
    scopes = [ui],
    needs_project,
    example(title = "Paste special keeps the destination's timing", params = r#"{"operation": "paste", "options": {"exposure": false}}"#),
    gui = "Edit > Paste Special and Cut Special with the Timeline focused: the options dialog; Timeline: drag-and-drop options (M5 C5.1)",
)]
pub(crate) fn timeline_transfer_options_set(cx: &mut Cx, p: SetParams) -> Result<SetResult, CommandError> {
    let given = p.options.as_object().ok_or_else(|| invalid("/options", "an object"))?;
    let mut set = stored(cx, p.operation);
    for (k, v) in to_01(given, "/options")? {
        set.insert(k, v);
    }
    let written = super::timeline::write_pref(cx, p.operation.key(), Value::Object(set), "/options")?;
    let options = Value::Object(to_04(written.as_object().unwrap_or(&Map::new())));
    Ok(SetResult { operation: p.operation, options })
}

/// `timeline.transferOptions.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    /// Paste special, cut special or drag and drop.
    operation: Operation,
}

/// `{options}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct GetResult {
    /// The stored set (04 §17.3).
    #[schemars(with = "super::cell::TransferOptions")]
    options: Value,
}

#[command(
    id = "timeline.transferOptions.get",
    kind = query,
    label = "Get transfer options",
    scopes = [read],
    needs_project,
    example(title = "The drag-and-drop options", params = r#"{"operation": "dragDrop"}"#),
    gui = "the paste special, cut special and drag-and-drop options dialog: the values it opens with (M5 C5.1)",
)]
pub(crate) fn timeline_transfer_options_get(cx: &mut Cx, p: GetParams) -> Result<GetResult, CommandError> {
    Ok(GetResult { options: Value::Object(to_04(&stored(cx, p.operation))) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mapping_is_a_bijection_over_every_member() {
        let table = table_default(Operation::Paste);
        assert_eq!(table.len(), FLAT.len() + PEG.len(), "the table names every member");
        let there = to_04(&table);
        let back = to_01(&there, "/options").unwrap_or_default();
        assert_eq!(back, table);
        assert!(to_01(&serde_json::from_value(json!({"nope": true})).unwrap_or_default(), "/options").is_err());
    }

    #[test]
    fn the_rust_defaults_are_the_one_table() {
        // Ruling E-13: one table (04 §11.14's values), `prefs.json`; the structs' fallbacks equal it.
        for op in [Operation::Paste, Operation::Cut, Operation::DragDrop] {
            let table = table_default(op);
            assert_eq!(table["overwriteDrawings"], false, "{op:?}");
            assert_eq!(table["updateColours"], false, "{op:?}");
            let edit = serde_json::to_value(super::super::edit_ops::TimelineEditOptions::default()).unwrap_or_default();
            assert_eq!(edit.as_object(), Some(&table), "{op:?}");
            let cell = serde_json::to_value(super::super::cell::TransferOptions::default()).unwrap_or_default();
            assert_eq!(cell.as_object(), Some(&to_04(&table)), "{op:?}");
        }
    }
}
