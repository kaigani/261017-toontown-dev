//! One store of the Timeline's transfer options (04 §11.14, §17.3; 01 §6.3, §7.3; SI-201, ruling E-8; M5 C5.1a,
//! `reviews/M5-design-timeline-kernel.md` §4).
//!
//! Each operation (paste special, cut special, drag and drop) has one stored option set: its 01 §7.3 preference key, in
//! 01's member names, persisted per user (04 line 759). `timeline.transferOptions.*` reads and writes the store in
//! 04's names through a fixed mapping; `edit.cutSpecial` and `edit.pasteSpecial` read it in 01's names and store the
//! members they are given when they remember; the `cell.*` commands take their omitted members from it (04 line 1366).
//!
//! **Per field, two families of defaults (ruling E-19).** The store keeps the members that were set, field by field (a
//! partial object under the key; the preference's effective value fills the others from its default by the per-member
//! rule O-17). A member never stored is read from its family's documented default: the `edit.*` family from 01 §6.3
//! ([`EDIT_DEFAULTS`], the `prefs.json` defaults), the 04 family (`cell.clipboard.*`, `cell.range.move`,
//! `timeline.transferOptions.get`, the drag-and-drop set) from 04 §11.14 ([`CELL_DEFAULTS`]). They differ in two members
//! (overwrite existing drawings, update colours); a member stored by either family governs both. The user's answer to
//! SI-201 changes one of the two constants.

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

/// The `edit.*` family's defaults, 01 §6.3 (in 01's names; equal to `prefs.json`'s `edit.*.timeline` defaults).
pub(crate) const EDIT_DEFAULTS: [(&str, bool); 12] = [
    ("overwriteDrawings", true),
    ("createDeleteExposure", true),
    ("extendExposures", false),
    ("updateColours", true),
    ("keyframeSubstitution", false),
    ("motionKeys", true),
    ("rotationKeys", true),
    ("scaleKeys", true),
    ("pivotKeys", true),
    ("velocityKeys", true),
    ("skewKeys", true),
    ("boundaryKeys", false),
];

/// The 04 family's defaults, 04 §11.14 and §17.3 (in 01's names).
pub(crate) const CELL_DEFAULTS: [(&str, bool); 12] = [
    ("overwriteDrawings", false),
    ("createDeleteExposure", true),
    ("extendExposures", false),
    ("updateColours", false),
    ("keyframeSubstitution", false),
    ("motionKeys", true),
    ("rotationKeys", true),
    ("scaleKeys", true),
    ("pivotKeys", true),
    ("velocityKeys", true),
    ("skewKeys", true),
    ("boundaryKeys", false),
];

fn table(t: &[(&str, bool); 12]) -> Map<String, Value> {
    t.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect()
}

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

/// The members stored for an operation (01's names), each a boolean: what was set, field by field.
pub(crate) fn stored_members(cx: &Cx, op: Operation) -> Map<String, Value> {
    let known: Vec<&str> = EDIT_DEFAULTS.iter().map(|(k, _)| *k).collect();
    cx.pref_raw(op.key())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(|(k, v)| v.is_boolean() && known.contains(&k.as_str()))
        .collect()
}

/// The `edit.*` family's set of an operation (01's names): the stored members over 01 §6.3's defaults.
pub(crate) fn edit_set(cx: &Cx, op: Operation) -> Map<String, Value> {
    let mut m = table(&EDIT_DEFAULTS);
    m.extend(stored_members(cx, op));
    m
}

/// The 04 family's set of an operation (01's names): the stored members over 04 §11.14's defaults.
pub(crate) fn cell_set(cx: &Cx, op: Operation) -> Map<String, Value> {
    let mut m = table(&CELL_DEFAULTS);
    m.extend(stored_members(cx, op));
    m
}

/// The base a `cell.*` command overlays its given members on: the 04 family's set in 04's names.
pub(crate) fn cell_base(cx: &Cx, op: Operation) -> Map<String, Value> {
    to_04(&cell_set(cx, op))
}

/// Stores the given members of an operation's set (01's names), keeping the members stored before: the one write path
/// of `prefs.set` on the three keys, `timeline.transferOptions.set` and the special commands' memory. The effective
/// value is validated as `prefs.set` validates it; `prefs.changed` names the key when the effective value changes.
pub(crate) fn store_members(
    cx: &mut Cx,
    op: Operation,
    given: Map<String, Value>,
    pointer: &str,
) -> Result<(), CommandError> {
    if stage_members(cx, op, given, pointer)? {
        let src = if matches!(cx.origin(), crate::Origin::Agent { .. }) { "agent" } else { "user" };
        cx.emit("prefs.changed", json!({ "keys": [op.key()], "source": src }));
    }
    Ok(())
}

/// [`store_members`] without the event: stages the members and answers whether the key's effective value changed
/// (`prefs.set` lists the key in its own `prefs.changed`).
pub(crate) fn stage_members(
    cx: &mut Cx,
    op: Operation,
    given: Map<String, Value>,
    pointer: &str,
) -> Result<bool, CommandError> {
    let known: Vec<&str> = EDIT_DEFAULTS.iter().map(|(k, _)| *k).collect();
    for (k, v) in &given {
        if !known.contains(&k.as_str()) {
            return Err(unknown_member(&format!("{pointer}/{k}")));
        }
        if !v.is_boolean() {
            return Err(invalid(&format!("{pointer}/{k}"), "a boolean"));
        }
    }
    let before_raw = stored_members(cx, op);
    let mut raw = before_raw.clone();
    raw.extend(given);
    if raw.is_empty() || (raw == before_raw && cx.pref_raw(op.key()).is_some()) {
        return Ok(false);
    }
    let before = cx.pref(op.key());
    let themes = cx.session().env.settings.theme_ids();
    let full = crate::prefs::overlay(op.key(), &Value::Object(raw.clone()), before.clone());
    crate::prefs::validate(op.key(), &full, &themes)?;
    let mut values = Map::new();
    values.insert(op.key().to_owned(), Value::Object(raw));
    cx.defer(crate::cx::Effect::Prefs(values));
    Ok(cx.pref(op.key()) != before)
}

/// The operation whose set a preference key stores.
pub(crate) fn operation_of_key(key: &str) -> Option<Operation> {
    [Operation::Paste, Operation::Cut, Operation::DragDrop].into_iter().find(|o| o.key() == key)
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
    /// The options to store (04 §17.3; omitted members keep their stored value, or 04 §11.14's default when never
    /// stored).
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
    store_members(cx, p.operation, to_01(given, "/options")?, "/options")?;
    Ok(SetResult { operation: p.operation, options: Value::Object(cell_base(cx, p.operation)) })
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
    /// The set the 04 family applies (04 §17.3): stored members, else 04 §11.14's defaults (ruling E-19).
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
    Ok(GetResult { options: Value::Object(cell_base(cx, p.operation)) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mapping_is_a_bijection_over_every_member() {
        let t = table(&EDIT_DEFAULTS);
        assert_eq!(t.len(), FLAT.len() + PEG.len(), "the table names every member");
        let back = to_01(&to_04(&t), "/options").unwrap_or_default();
        assert_eq!(back, t);
        assert!(to_01(&serde_json::from_value(json!({"nope": true})).unwrap_or_default(), "/options").is_err());
    }

    /// Ruling E-19: two named tables side by side; this test prints the members in which they differ (the user's answer
    /// to SI-201 changes one constant), and keeps each family's fallbacks and `prefs.json` equal to its table.
    #[test]
    fn the_two_default_tables_differ_in_two_members_and_the_fallbacks_follow_them() {
        let (edit, cell) = (table(&EDIT_DEFAULTS), table(&CELL_DEFAULTS));
        let differ: Vec<&String> = edit.keys().filter(|k| edit[k.as_str()] != cell[k.as_str()]).collect();
        println!("01 section 6.3 and 04 section 11.14 differ in: {differ:?}");
        assert_eq!(differ, vec!["overwriteDrawings", "updateColours"]);
        for op in [Operation::Paste, Operation::Cut, Operation::DragDrop] {
            let prefs = crate::store::pref_default(op.key()).and_then(|v| v.as_object().cloned()).unwrap_or_default();
            assert_eq!(prefs, edit, "{op:?}: prefs.json holds the edit family's defaults");
        }
        let e = serde_json::to_value(super::super::edit_ops::TimelineEditOptions::default()).unwrap_or_default();
        assert_eq!(e.as_object(), Some(&edit));
        let c = serde_json::to_value(super::super::cell::TransferOptions::default()).unwrap_or_default();
        assert_eq!(c.as_object(), Some(&to_04(&cell)));
    }
}
