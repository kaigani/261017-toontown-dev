//! Colour recovery (03 §11.2, §1.6; 10 §4.11; 02 §3.11; M2 C2.2): the recovery copies drawings carry, the colour IDs
//! artwork uses that no palette defines, and their recreation.
//!
//! A missing swatch is recreated with the **same colour ID** from a recovery copy: into the visible palette with the
//! recorded name (styles it lacks are added by name, every swatch receiving a value for them), else into a new palette
//! of that name, or `Recovered` when no name is recorded. Without a recovery copy a placeholder `Deleted colour`
//! (opaque mid-grey, the fallback of 03 §1.7) is created in `Recovered`. One transaction ("Colour Recovery"; with
//! paste, the paste's own transaction), reported by `colour.recovery.performed`. A drawing's recovery list holds one
//! entry for every colour ID its strokes, sides, brush exterior records and text use (10 §4.11), refreshed on save.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::{Drawing, DrawingNode};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};

use super::draw::op;
use crate::cx::Cx;

/// Staged entry patches.
type Changes = Vec<(EntryKey, Vec<PatchOp>)>;

/// The fallback value of placeholder swatches (03 §1.7: opaque mid-grey).
pub(crate) const FALLBACK_RGBA: &str = "#808080FF";

fn node_colours(nodes: &[DrawingNode], out: &mut BTreeSet<String>) {
    for n in nodes {
        for s in n.strokes().iter().flatten() {
            if let Some(c) = s.colour_id() {
                out.insert(c.as_str().to_owned());
            }
            for side in [&s.sides().left, &s.sides().right].into_iter().flatten() {
                if let Some(c) = &side.colour_id {
                    out.insert(c.as_str().to_owned());
                }
            }
            if let Ok(v) = serde_json::to_value(s)
                && let Some(records) = v
                    .pointer(&format!("/ext/{}/exterior", tt_vector::planar::fill::EXT_NAMESPACE))
                    .and_then(Value::as_array)
            {
                for r in records {
                    if let Some(c) = r.pointer("/fill/colourId").and_then(Value::as_str) {
                        out.insert(c.to_owned());
                    }
                }
            }
        }
        for r in n.runs().iter().flatten() {
            if let Some(c) = &r.colour_id {
                out.insert(c.as_str().to_owned());
            }
        }
        if let Some(children) = n.children() {
            node_colours(children, out);
        }
    }
}

/// Every colour ID a drawing uses (strokes, zone fills on sides and brush exterior records, text).
pub(crate) fn used_colours(d: &Drawing) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    node_colours(d.nodes(), &mut out);
    out
}

fn defined(doc: &Document) -> BTreeMap<String, (Value, Value)> {
    let mut out = BTreeMap::new();
    for p in doc.palettes() {
        let Ok(pal) = serde_json::to_value(p) else { continue };
        for s in pal.get("swatches").and_then(Value::as_array).into_iter().flatten() {
            if let Some(c) = s.get("colourId").and_then(Value::as_str) {
                out.insert(c.to_owned(), (pal.clone(), s.clone()));
            }
        }
    }
    out
}

/// The recovery entry of a defined colour (10 §4.11).
fn entry_of(pal: &Value, swatch: &Value) -> Value {
    let styles: Vec<Value> = pal
        .get("styles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|s| json!({ "styleId": s.get("id"), "styleName": s.get("name") }))
        .collect();
    json!({
        "colourId": swatch.get("colourId"),
        "paletteId": pal.get("id"),
        "paletteName": pal.get("name"),
        "styles": styles,
        "swatch": swatch,
    })
}

/// The recovery list a drawing should hold: one entry per used colour, current definitions for defined colours and the
/// drawing's own copies for missing ones; `None` when the stored list already is that.
pub(crate) fn refreshed(doc: &Document, d: &Drawing) -> Option<Value> {
    let defs = defined(doc);
    let stored: BTreeMap<String, Value> = d
        .colour_recovery()
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok().map(|v| (e.colour_id.as_str().to_owned(), v)))
        .collect();
    let list: Vec<Value> = used_colours(d)
        .into_iter()
        .filter_map(|c| match defs.get(&c) {
            Some((pal, sw)) => Some(entry_of(pal, sw)),
            None => stored.get(&c).cloned(),
        })
        .collect();
    let current: Vec<Value> = stored.into_values().collect();
    let mut sorted_current = current;
    sorted_current
        .sort_by(|a, b| a.get("colourId").map(Value::to_string).cmp(&b.get("colourId").map(Value::to_string)));
    (sorted_current != list).then_some(Value::Array(list))
}

/// What a recovery did.
#[derive(Debug, Clone, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryResult {
    /// Swatches recreated from recovery copies, with the palette that received each.
    recovered: Vec<RecoveredColour>,
    /// Placeholder swatches created for colours without a recovery copy.
    placeholders: Vec<String>,
    /// Palettes created.
    created_palette_ids: Vec<String>,
}

/// One recreated swatch.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveredColour {
    /// The colour ID (unchanged).
    colour_id: String,
    /// The palette it went into.
    palette_id: String,
}

impl RecoveryResult {
    /// Whether anything was recovered.
    pub(crate) fn is_empty(&self) -> bool {
        self.recovered.is_empty() && self.placeholders.is_empty()
    }
}

struct Plan {
    /// Palettes as they will be (modified existing ones and new ones), by ID.
    palettes: BTreeMap<String, Value>,
    /// Which of them are new.
    created: Vec<String>,
}

fn find_palette(doc: &Document, plan: &Plan, name: &str) -> Option<String> {
    plan.palettes
        .iter()
        .find(|(_, v)| v.get("name").and_then(Value::as_str) == Some(name) && v.get("hidden") != Some(&json!(true)))
        .map(|(k, _)| k.clone())
        .or_else(|| doc.palettes().find(|p| !*p.hidden() && p.name() == name).map(|p| p.id().as_str().to_owned()))
}

fn palette_json(doc: &Document, plan: &Plan, id: &str) -> Option<Value> {
    plan.palettes.get(id).cloned().or_else(|| {
        let pid = Id::parse(id).ok()?;
        doc.palette(&pid).and_then(|p| serde_json::to_value(p).ok())
    })
}

fn new_palette(
    cx: &mut Cx,
    doc: &Document,
    plan: &mut Plan,
    name: &str,
    styles: &[String],
) -> Result<String, CommandError> {
    let taken: BTreeSet<String> = doc
        .palettes()
        .map(|p| p.name().clone())
        .chain(plan.palettes.values().filter_map(|v| v.get("name").and_then(Value::as_str).map(str::to_owned)))
        .collect();
    let name = if taken.contains(name) {
        tt_persist::repair::unique_name(ObjectType::Palette, name, &taken)
    } else {
        name.to_owned()
    };
    let id = cx.new_id(ObjectType::Palette)?.as_str().to_owned();
    let mut style_list = Vec::new();
    let names: Vec<String> = if styles.is_empty() { vec!["Default".into()] } else { styles.to_vec() };
    for s in names {
        let sid = cx.new_id(ObjectType::PaletteStyle)?;
        style_list.push(json!({ "id": sid.as_str(), "type": "paletteStyle", "rev": 0, "name": s }));
    }
    plan.palettes.insert(
        id.clone(),
        json!({ "id": id, "type": "palette", "rev": 0, "name": name, "styles": style_list, "swatches": [], "hidden": false }),
    );
    plan.created.push(id.clone());
    Ok(id)
}

/// Adds styles a palette lacks (by name); every existing swatch gets its first style's value for them.
fn ensure_styles(cx: &mut Cx, pal: &mut Value, names: &[String]) -> Result<(), CommandError> {
    let have: BTreeSet<String> = pal
        .get("styles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for n in names {
        if have.contains(n) {
            continue;
        }
        let sid = cx.new_id(ObjectType::PaletteStyle)?.as_str().to_owned();
        if let Some(list) = pal.get_mut("styles").and_then(Value::as_array_mut) {
            list.push(json!({ "id": sid, "type": "paletteStyle", "rev": 0, "name": n }));
        }
        if let Some(sw) = pal.get_mut("swatches").and_then(Value::as_array_mut) {
            for s in sw {
                let first = s.get("values").and_then(Value::as_object).and_then(|m| m.values().next().cloned());
                if let (Some(v), Some(m)) = (first, s.get_mut("values").and_then(Value::as_object_mut)) {
                    m.insert(sid.clone(), v);
                }
            }
        }
    }
    Ok(())
}

/// Plans the recreation of missing colours (`(colourId, recovery entry)`) and returns the staged changes.
pub(crate) fn recover(
    cx: &mut Cx,
    doc: &Document,
    missing: &[(String, Option<Value>)],
) -> Result<(RecoveryResult, Changes), CommandError> {
    let mut plan = Plan { palettes: BTreeMap::new(), created: Vec::new() };
    let mut result = RecoveryResult::default();
    for (colour, entry) in missing {
        match entry {
            Some(e) => {
                let recorded = e.get("paletteName").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
                let style_names: Vec<String> = e
                    .get("styles")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|s| s.get("styleName").and_then(Value::as_str).map(str::to_owned))
                    .collect();
                let name = if recorded.is_empty() { "Recovered".to_owned() } else { recorded };
                let pid = match find_palette(doc, &plan, &name) {
                    Some(p) => p,
                    None => new_palette(cx, doc, &mut plan, &name, &style_names)?,
                };
                let mut pal = palette_json(doc, &plan, &pid).unwrap_or(Value::Null);
                ensure_styles(cx, &mut pal, &style_names)?;
                // Values by style name, the recovery copy's first value for styles it has no value for.
                let by_name: BTreeMap<String, Value> = e
                    .get("styles")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|s| {
                        let sid = s.get("styleId")?.as_str()?;
                        let v = e.pointer(&format!("/swatch/values/{sid}"))?.clone();
                        Some((s.get("styleName")?.as_str()?.to_owned(), v))
                    })
                    .collect();
                let first =
                    e.pointer("/swatch/values").and_then(Value::as_object).and_then(|m| m.values().next().cloned());
                let mut values = serde_json::Map::new();
                for s in pal.get("styles").and_then(Value::as_array).into_iter().flatten() {
                    let (Some(sid), Some(sname)) =
                        (s.get("id").and_then(Value::as_str), s.get("name").and_then(Value::as_str))
                    else {
                        continue;
                    };
                    if let Some(v) = by_name.get(sname).cloned().or_else(|| first.clone()) {
                        values.insert(sid.to_owned(), v);
                    }
                }
                let sw = e.get("swatch").cloned().unwrap_or(Value::Null);
                let swatch = json!({
                    "colourId": colour, "type": "swatch", "rev": 0,
                    "name": sw.get("name").cloned().unwrap_or(json!("Recovered")),
                    "kind": sw.get("kind").cloned().unwrap_or(json!("solid")),
                    "deformable": sw.get("deformable").cloned().unwrap_or(json!(false)),
                    "values": values,
                });
                if let Some(list) = pal.get_mut("swatches").and_then(Value::as_array_mut) {
                    list.push(swatch);
                }
                plan.palettes.insert(pid.clone(), pal);
                result.recovered.push(RecoveredColour { colour_id: colour.clone(), palette_id: pid });
            }
            None => {
                let pid = match find_palette(doc, &plan, "Recovered") {
                    Some(p) => p,
                    None => new_palette(cx, doc, &mut plan, "Recovered", &[])?,
                };
                let mut pal = palette_json(doc, &plan, &pid).unwrap_or(Value::Null);
                let mut values = serde_json::Map::new();
                for s in pal.get("styles").and_then(Value::as_array).into_iter().flatten() {
                    if let Some(sid) = s.get("id").and_then(Value::as_str) {
                        values.insert(sid.to_owned(), json!({ "rgba": FALLBACK_RGBA }));
                    }
                }
                if let Some(list) = pal.get_mut("swatches").and_then(Value::as_array_mut) {
                    list.push(json!({
                        "colourId": colour, "type": "swatch", "rev": 0, "name": "Deleted colour", "kind": "solid",
                        "deformable": false, "values": values,
                    }));
                }
                plan.palettes.insert(pid.clone(), pal);
                result.placeholders.push(colour.clone());
            }
        }
    }
    let mut changes = Vec::new();
    let mut project_ops = Vec::new();
    for (pid, pal) in plan.palettes {
        let key = EntryKey::Palette(Id::parse(&pid)?);
        if plan.created.contains(&pid) {
            changes.push((key, vec![op("add", "", Some(pal))]));
            project_ops.push(op("add", "/palettes/-", Some(json!({ "id": pid, "type": "palette" }))));
        } else {
            changes.push((key, vec![op("replace", "", Some(pal))]));
        }
    }
    if !project_ops.is_empty() {
        changes.insert(0, (EntryKey::Project, project_ops));
    }
    // 03 §1.5: a palette added later is shown in every existing scene with its first style.
    let firsts: Vec<(String, String)> = plan
        .created
        .iter()
        .filter_map(|pid| {
            let key = EntryKey::Palette(Id::parse(pid).ok()?);
            let pal = changes.iter().find(|(k, _)| *k == key)?;
            let first = pal.1.first()?.value.as_ref()?.pointer("/styles/0/id")?.as_str()?.to_owned();
            Some((pid.clone(), first))
        })
        .collect();
    if !firsts.is_empty() {
        for scene in doc.scenes() {
            let ops: Vec<PatchOp> = firsts
                .iter()
                .map(|(pal, st)| {
                    op("add", &format!("/paletteStyles/{}", tt_model::pointer::escape(pal)), Some(json!(st)))
                })
                .collect();
            changes.push((EntryKey::Scene(scene.id().clone()), ops));
        }
    }
    result.created_palette_ids = plan.created;
    Ok((result, changes))
}

/// The recovery copy of a colour among the given drawings (first found).
fn copy_of(doc: &Document, drawings: &[Id], colour: &str) -> Option<Value> {
    drawings
        .iter()
        .filter_map(|d| doc.drawing(d))
        .chain(doc.drawings())
        .flat_map(|d| d.colour_recovery().iter())
        .find(|e| e.colour_id.as_str() == colour)
        .and_then(|e| serde_json::to_value(e).ok())
}

/// The missing colours of some drawings with the drawings using each.
fn missing_in(doc: &Document, drawings: &[Id]) -> BTreeMap<String, Vec<String>> {
    let defs = defined(doc);
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in drawings {
        let Some(d) = doc.drawing(id) else { continue };
        for c in used_colours(d) {
            if !defs.contains_key(&c) {
                out.entry(c).or_default().push(id.as_str().to_owned());
            }
        }
    }
    out
}

/// Scope of colour queries (03 §16.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Scope {
    /// Every drawing of the project.
    #[default]
    Project,
    /// Drawings of the elements of a scene.
    Scene,
    /// Drawings of given elements.
    Element,
    /// Given drawings.
    Drawing,
}

fn scoped(
    cx: &mut Cx,
    scope: Scope,
    scene: Option<&str>,
    elements: Option<&[String]>,
    drawings: Option<&[String]>,
) -> Result<Vec<Id>, CommandError> {
    let doc = cx.document()?.clone();
    let of_elements = |ids: &[Id]| -> Vec<Id> {
        ids.iter()
            .filter_map(|e| doc.element(e).and_then(|el| el.drawing_bank_id().clone()))
            .filter_map(|b| doc.bank(&b).map(|bank| bank.drawings().ids().to_vec()))
            .flatten()
            .collect()
    };
    Ok(match scope {
        Scope::Project => doc.drawings().map(|d| d.id().clone()).collect(),
        Scope::Drawing => {
            let mut out = Vec::new();
            for d in drawings.unwrap_or_default() {
                out.push(Id::parse(&cx.resolve_as(d, ObjectType::Drawing)?)?);
            }
            out
        }
        Scope::Element => {
            let mut ids = Vec::new();
            for e in elements.unwrap_or_default() {
                ids.push(Id::parse(&cx.resolve_as(e, ObjectType::Element)?)?);
            }
            of_elements(&ids)
        }
        Scope::Scene => {
            let sid = match scene {
                Some(s) => Id::parse(&cx.resolve_as(s, ObjectType::Scene)?)?,
                None => cx.app().scene.clone().ok_or_else(|| {
                    super::draw::err("colour.notFound", "no current scene", json!({ "pointer": "/sceneId" }))
                })?,
            };
            let elements: Vec<Id> =
                doc.elements().filter(|e| doc.scene_of(e.id()) == Some(&sid)).map(|e| e.id().clone()).collect();
            of_elements(&elements)
        }
    })
}

// ----- colour.missing.list ------------------------------------------------------------------------------------------

/// `colour.missing.list` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct MissingParams {
    /// Scope.
    #[serde(default)]
    scope: Scope,
    /// Scene of `scene` scope (default: current).
    #[serde(default)]
    scene_id: Option<String>,
    /// Elements of `element` scope.
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Drawings of `drawing` scope.
    #[serde(default)]
    drawing_ids: Option<Vec<String>>,
}

/// One missing colour.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Missing {
    /// The colour ID.
    colour_id: String,
    /// Drawings using it.
    drawing_ids: Vec<String>,
    /// A recovery copy exists.
    recoverable: bool,
    /// The palette name the copy records.
    recorded_palette_name: Option<String>,
}

/// `colour.missing.list` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct MissingResult {
    /// Missing colours.
    missing: Vec<Missing>,
}

#[command(
    id = "colour.missing.list",
    kind = query,
    label = "List missing colours",
    scopes = [read],
    needs_project,
    example(title = "Colours artwork uses that no palette defines", params = "{}"),
    gui = "Colour palette view: missing colours (M7 C7.1)",
)]
pub(crate) fn colour_missing_list(cx: &mut Cx, p: MissingParams) -> Result<MissingResult, CommandError> {
    let drawings = scoped(cx, p.scope, p.scene_id.as_deref(), p.element_ids.as_deref(), p.drawing_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let missing = missing_in(&doc, &drawings)
        .into_iter()
        .map(|(c, ds)| {
            let copy = copy_of(&doc, &drawings, &c);
            Missing {
                recorded_palette_name: copy
                    .as_ref()
                    .and_then(|e| e.get("paletteName").and_then(Value::as_str).map(str::to_owned)),
                recoverable: copy.is_some(),
                colour_id: c,
                drawing_ids: ds,
            }
        })
        .collect();
    Ok(MissingResult { missing })
}

// ----- colour.recovery.run ------------------------------------------------------------------------------------------

/// `colour.recovery.run` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RecoveryParams {
    /// Drawings whose missing colours are recovered.
    #[serde(default)]
    drawing_ids: Option<Vec<String>>,
    /// Every drawing of the project.
    #[serde(default)]
    all: bool,
}

#[command(
    id = "colour.recovery.run",
    kind = action,
    label = "Colour Recovery",
    scopes = [edit],
    undo = "Colour Recovery",
    needs_project,
    example(title = "Recover every missing colour of the project", params = r#"{"all": true}"#),
    gui = "Colour palette view: Recover Colours (M7 C7.1)",
)]
pub(crate) fn colour_recovery_run(cx: &mut Cx, p: RecoveryParams) -> Result<RecoveryResult, CommandError> {
    let scope = if p.all || p.drawing_ids.is_none() { Scope::Project } else { Scope::Drawing };
    let drawings = scoped(cx, scope, None, None, p.drawing_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let missing: Vec<(String, Option<Value>)> = missing_in(&doc, &drawings)
        .into_keys()
        .map(|c| {
            let e = copy_of(&doc, &drawings, &c);
            (c, e)
        })
        .collect();
    let (result, changes) = recover(cx, &doc, &missing)?;
    if !changes.is_empty() {
        cx.patch_entries(changes)?;
        cx.set_label("Colour Recovery");
        performed(cx, &result);
    }
    Ok(result)
}

/// Emits `colour.recovery.performed` for a recovery that added swatches.
pub(crate) fn performed(cx: &mut Cx, r: &RecoveryResult) {
    if r.is_empty() {
        return;
    }
    let colours: Vec<String> =
        r.recovered.iter().map(|x| x.colour_id.clone()).chain(r.placeholders.iter().cloned()).collect();
    let mut palettes: Vec<String> =
        r.recovered.iter().map(|x| x.palette_id.clone()).chain(r.created_palette_ids.iter().cloned()).collect();
    palettes.sort();
    palettes.dedup();
    cx.emit("colour.recovery.performed", json!({ "colourIds": colours, "paletteIds": palettes }));
}

/// `colour.recovery.performed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryEvent {
    /// Colours recreated (placeholders included).
    colour_ids: Vec<String>,
    /// Palettes that received them.
    palette_ids: Vec<String>,
}

#[command(
    id = "colour.recovery.performed",
    kind = event,
    label = "Colour recovery performed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers (emitted when recovery adds swatches)",
)]
pub(crate) fn colour_recovery_performed(
    _cx: &mut Cx,
    _p: super::common::NoParams,
) -> Result<RecoveryEvent, CommandError> {
    Err(super::common::event_not_invocable("colour.recovery.performed"))
}
