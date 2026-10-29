//! `doc.export.json` and `doc.import.json`: the interchange bundle (10 §6.8; C1.3).
//!
//! A bundle holds full-depth object-model JSON (DM-1): scenes with their elements inline, drawing banks with their
//! drawings, the palettes reduced to the colour IDs used (all styles), pens, assets and, per `binaries`, the binary
//! entries as base64. Importing splits the objects back into package entries and adds them in **one transaction**
//! (one undo step "Import"), validated as a whole by the model before anything changes.
//!
//! Import rules (ADR-0014 §7): with `idPolicy: "new"` every object gets a fresh ID and every reference to it inside
//! the bundle follows (colour IDs are kept: they are the identity colour recovery works with, 03 §11); entry names that
//! embed IDs (embedded media, bitmap pixels) follow too. With `"preserve"` IDs are kept and an ID that already exists
//! is a conflict. Name conflicts (scenes in the project, elements in the target scene, palettes) are `rename`d
//! (`-2`, `-3`, …), `replace`d (the existing object goes) or `fail` (`doc.importConflict`). Palettes merge by palette
//! ID; a swatch whose colour ID already exists keeps the existing values (`keepExisting`) or takes the imported ones in
//! the same palette (`updateExisting`). Imported elements placed in a target scene start at `atFrame`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey, RenderOptions, SlotKind, slots_of};
use tt_model::{Id, IdGenerator, ObjectType};
use tt_persist::container::BlobData;
use tt_persist::{b64, fsx};
use tt_registry::{CommandError, codes, command};

use super::common::{OnExists, invalid};
use crate::cx::Cx;
use crate::persistence;

const FULL: RenderOptions = RenderOptions { depth: None, path: false };

// ----- export ------------------------------------------------------------------------------------------------------

/// Export scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExportScope {
    /// The whole project.
    Project,
    /// Scenes (`refs`).
    Scenes,
    /// Elements with their descendants (`refs`).
    Elements,
}

/// What happens to binary entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Binaries {
    /// Inline as base64.
    Embed,
    /// Referenced by entry name (resolved against the bundle's folder or the source project).
    #[default]
    Reference,
    /// Left out.
    Omit,
}

fn yes() -> bool {
    true
}

/// `doc.export.json` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ExportJsonParams {
    /// Scope.
    scope: ExportScope,
    /// Scenes or elements of the scope.
    #[serde(default)]
    refs: Option<Vec<String>>,
    /// Write to this absolute file instead of returning the bundle.
    #[serde(default)]
    path: Option<String>,
    /// Binary entries.
    #[serde(default)]
    binaries: Binaries,
    /// Canonical, indented JSON (else compact) when writing a file.
    #[serde(default = "yes")]
    pretty: bool,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// `doc.export.json` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportJsonResult {
    /// The bundle (without `path`).
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle: Option<Map<String, Value>>,
    /// The written file.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// Size of the bundle text.
    bytes: u64,
    /// Objects exported (scenes, elements, banks, drawings, palettes, pens, assets).
    object_count: u64,
}

/// Colour IDs a value uses (strokes, fills, pens), outside opaque values and colour-recovery copies.
fn colours(v: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::String(s) if s.starts_with("col_") => {
            out.insert(s.clone());
        }
        Value::Array(a) => a.iter().for_each(|x| colours(x, out, depth + 1)),
        Value::Object(m) => {
            for (k, x) in m {
                if k != "meta" && k != "ext" && k != "colourRecovery" {
                    colours(x, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

/// Entry names a value references (`entry` of blobs and assets, `image` of textures).
fn entries(v: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "meta" || k == "ext" {
                    continue;
                }
                match (k.as_str(), x) {
                    ("entry", Value::String(s)) => {
                        out.insert(s.clone());
                    }
                    ("image", Value::String(s)) if s.starts_with("textures/") => {
                        out.insert(s.clone());
                    }
                    _ => entries(x, out, depth + 1),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| entries(x, out, depth + 1)),
        _ => {}
    }
}

/// The elements with their descendants in tree order.
fn with_descendants(doc: &Document, roots: &[String]) -> Vec<String> {
    fn walk(nodes: &[Value], take: bool, roots: &[String], out: &mut Vec<String>) {
        for n in nodes {
            let id = n.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            let here = take || roots.contains(&id);
            if here && !out.contains(&id) {
                out.push(id.clone());
            }
            walk(n.get("children").and_then(Value::as_array).map_or(&[], Vec::as_slice), here, roots, out);
        }
    }
    let mut out = Vec::new();
    for s in doc.scenes() {
        if let Ok(v) = doc.entry_raw_json(&EntryKey::Scene(s.id().clone())) {
            walk(v.get("tree").and_then(Value::as_array).map_or(&[], Vec::as_slice), false, roots, &mut out);
        }
    }
    out
}

/// The tree fragment of selected elements (subtrees rooted at the selected elements that have no selected ancestor).
fn tree_fragment(doc: &Document, selected: &[String]) -> Vec<Value> {
    fn walk(nodes: &[Value], selected: &[String], out: &mut Vec<Value>) {
        for n in nodes {
            let id = n.get("id").and_then(Value::as_str).unwrap_or_default();
            if selected.iter().any(|s| s == id) {
                out.push(n.clone());
            } else {
                walk(n.get("children").and_then(Value::as_array).map_or(&[], Vec::as_slice), selected, out);
            }
        }
    }
    let mut out = Vec::new();
    for s in doc.scenes() {
        if let Ok(v) = doc.entry_raw_json(&EntryKey::Scene(s.id().clone())) {
            walk(v.get("tree").and_then(Value::as_array).map_or(&[], Vec::as_slice), selected, &mut out);
        }
    }
    out
}

/// Builds the bundle of 10 §6.8.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_bundle(
    cx: &Cx,
    scope: ExportScope,
    refs: Option<&Vec<String>>,
    binaries: Binaries,
) -> Result<(Value, u64), CommandError> {
    let doc = cx.document()?;
    let resolve_all = |ty: ObjectType| -> Result<Vec<String>, CommandError> {
        refs.ok_or_else(|| invalid("/refs", "give the objects of the scope in `refs`"))?
            .iter()
            .map(|r| cx.resolve_as(r, ty))
            .collect()
    };
    let (scene_ids, element_ids): (Vec<String>, Vec<String>) = match scope {
        ExportScope::Project => {
            (doc.project().scenes().ids().iter().map(|i| i.as_str().to_owned()).collect(), Vec::new())
        }
        ExportScope::Scenes => (resolve_all(ObjectType::Scene)?, Vec::new()),
        ExportScope::Elements => (Vec::new(), with_descendants(doc, &resolve_all(ObjectType::Element)?)),
    };
    let mut scenes = Vec::new();
    let mut elements = Vec::new();
    let mut used_elements: Vec<String> = element_ids.clone();
    for s in &scene_ids {
        let v = doc.object_json(s, FULL)?;
        for e in v.get("elements").and_then(Value::as_array).into_iter().flatten() {
            if let Some(id) = e.get("id").and_then(Value::as_str) {
                used_elements.push(id.to_owned());
            }
        }
        scenes.push(v);
    }
    for e in &element_ids {
        elements.push(doc.object_json(e, FULL)?);
    }
    // Banks of the exported elements, with their drawings.
    let mut bank_ids: Vec<String> = Vec::new();
    for e in &used_elements {
        let Some(el) = doc.element(&Id::parse(e).map_err(CommandError::from)?) else { continue };
        if let Some(b) = el.drawing_bank_id()
            && !bank_ids.contains(&b.as_str().to_owned())
        {
            bank_ids.push(b.as_str().to_owned());
        }
    }
    if scope == ExportScope::Project {
        for b in doc.banks() {
            if !bank_ids.contains(&b.id().as_str().to_owned()) {
                bank_ids.push(b.id().as_str().to_owned());
            }
        }
    }
    let banks: Vec<Value> = bank_ids.iter().map(|b| doc.object_json(b, FULL)).collect::<Result<_, _>>()?;
    // Assets used by the exported elements (all of them for the project).
    let refs_by_asset = super::doc_io::asset_refs(doc);
    let assets: Vec<Value> = doc
        .project()
        .assets()
        .iter()
        .filter(|a| {
            scope == ExportScope::Project
                || refs_by_asset
                    .get(a.id().as_str())
                    .is_some_and(|users| users.iter().any(|u| used_elements.contains(u)))
        })
        .map(|a| doc.object_json(a.id().as_str(), FULL))
        .collect::<Result<_, _>>()?;
    let pens: Vec<Value> = if scope == ExportScope::Project {
        doc.project().pens().iter().map(|p| doc.object_json(p.id().as_str(), FULL)).collect::<Result<_, _>>()?
    } else {
        Vec::new()
    };
    // Palettes reduced to the colour IDs used (all styles); the whole palettes for the project.
    let mut used = BTreeSet::new();
    for v in scenes.iter().chain(elements.iter()).chain(banks.iter()).chain(pens.iter()) {
        colours(v, &mut used, 0);
    }
    let mut palettes = Vec::new();
    for p in doc.palettes() {
        let mut v = doc.object_json(p.id().as_str(), FULL)?;
        if scope != ExportScope::Project {
            if let Some(list) = v.get_mut("swatches").and_then(Value::as_array_mut) {
                list.retain(|s| s.get("colourId").and_then(Value::as_str).is_some_and(|c| used.contains(c)));
            }
            if v.get("swatches").and_then(Value::as_array).is_none_or(Vec::is_empty) {
                continue;
            }
        }
        palettes.push(v);
    }
    // Binary entries.
    let mut blobs = Map::new();
    if binaries == Binaries::Embed {
        let mut names = BTreeSet::new();
        for v in banks.iter().chain(assets.iter()).chain(palettes.iter()) {
            entries(v, &mut names, 0);
        }
        let store = &cx.project()?.store.blobs;
        for n in names {
            if let Ok(bytes) = store.read(&n, 1 << 32) {
                blobs.insert(
                    n.clone(),
                    json!({ "mediaType": super::doc_io::media_type(&n), "base64": b64::encode(&bytes) }),
                );
            }
        }
    }
    let project = doc.project();
    let mut bundle = json!({
        "format": "animjson",
        "formatVersion": tt_model::FORMAT_VERSION,
        "generator": tt_persist::manifest::generator(),
        "exportedAt": crate::time::now_iso(),
        "sourceProjectId": project.id().as_str(),
        "scope": {
            "kind": match scope { ExportScope::Project => "project", ExportScope::Scenes => "scenes", ExportScope::Elements => "elements" },
            "ids": match scope { ExportScope::Project => vec![project.id().as_str().to_owned()], ExportScope::Scenes => scene_ids.clone(), ExportScope::Elements => refs.map(|r| r.iter().map(|x| cx.resolve_as(x, ObjectType::Element)).collect::<Result<Vec<_>, _>>()).transpose()?.unwrap_or_default() },
        },
        "settings": { "frameRate": project.frame_rate(), "cameraWidth": project.camera_width(), "cameraHeight": project.camera_height() },
        "scenes": scenes,
        "elements": elements,
        "drawingBanks": banks,
        "palettes": palettes,
        "pens": pens,
        "assets": assets,
        "blobs": blobs,
    });
    if scope == ExportScope::Elements
        && let Some(m) = bundle.as_object_mut()
    {
        m.insert("tree".into(), Value::Array(tree_fragment(doc, &element_ids)));
        let order: Vec<String> = doc
            .scenes()
            .flat_map(|s| s.column_order().iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>())
            .filter(|e| element_ids.contains(e))
            .collect();
        m.insert("columnOrder".into(), json!(order));
    }
    let count = (bundle["scenes"].as_array().map_or(0, Vec::len)
        + bundle["elements"].as_array().map_or(0, Vec::len)
        + bundle["drawingBanks"].as_array().map_or(0, Vec::len)
        + bundle["palettes"].as_array().map_or(0, Vec::len)
        + bundle["pens"].as_array().map_or(0, Vec::len)
        + bundle["assets"].as_array().map_or(0, Vec::len)) as u64;
    Ok((bundle, count))
}

#[command(
    id = "doc.export.json",
    kind = query,
    label = "Export JSON",
    scopes = [read],
    needs_project,
    example(title = "The first scene as an inline bundle", params = r#"{"scope": "scenes", "refs": ["scene:#1"], "binaries": "omit"}"#),
    agent_only = "agents exchange object-model bundles",
)]
pub(crate) fn doc_export_json(cx: &mut Cx, p: ExportJsonParams) -> Result<ExportJsonResult, CommandError> {
    let (bundle, object_count) = build_bundle(cx, p.scope, p.refs.as_ref(), p.binaries)?;
    match &p.path {
        Some(path) => {
            let target = persistence::path_param(cx, path, "/path")?;
            if !cx.origin().has_scope(tt_registry::Scope::FilesWrite) {
                return Err(CommandError::with_details(
                    codes::PERMISSION_DENIED,
                    "writing the bundle to a file needs the scope files.write",
                    json!({ "commandId": "doc.export.json", "missingScopes": ["files.write"] }),
                ));
            }
            if target.exists() && p.on_exists == OnExists::Fail {
                return Err(CommandError::with_details(
                    tt_persist::codes::FS_EXISTS,
                    format!("`{}` exists", target.display()),
                    json!({ "path": target.to_string_lossy() }),
                )
                .hint("pass onExists: \"overwrite\""));
            }
            let text = if p.pretty {
                tt_model::canonical::to_canonical_string(&bundle)?
            } else {
                tt_model::canonical::to_compact_string(&bundle)?
            };
            if !cx.dry_run() {
                fsx::write_atomic(&target, text.as_bytes(), fsx::fs_error)?;
            }
            Ok(ExportJsonResult {
                bundle: None,
                path: Some(target.to_string_lossy().into_owned()),
                bytes: text.len() as u64,
                object_count,
            })
        }
        None => {
            let text = tt_model::canonical::to_compact_string(&bundle)?;
            if text.len() > super::doc::MAX_RESULT_BYTES {
                return Err(CommandError::with_details(
                    "doc.exportTooLarge",
                    format!("the bundle has {} bytes, more than the inline limit", text.len()),
                    json!({ "bytes": text.len(), "limit": super::doc::MAX_RESULT_BYTES }),
                )
                .hint("write it to a file with `path`"));
            }
            let Value::Object(m) = bundle else { return Err(CommandError::internal("bundle is not an object")) };
            Ok(ExportJsonResult { bundle: Some(m), path: None, bytes: text.len() as u64, object_count })
        }
    }
}

// ----- import ------------------------------------------------------------------------------------------------------

/// A bundle given inline or as a file.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum ImportSource {
    /// Absolute path of a bundle file.
    Path(#[schemars(length(min = 1, max = 4096))] String),
    /// The bundle.
    Bundle(Map<String, Value>),
}

/// `doc.import.json` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ImportJsonParams {
    /// The bundle, or the path of a bundle file.
    source: ImportSource,
    /// A scene to import the elements into (omit: the bundle's scenes become new scenes).
    #[serde(default)]
    target: Option<String>,
    /// Position of new scenes in the storyline (default: the end).
    #[serde(default)]
    index: Option<u32>,
    /// First frame of imported elements in the target scene.
    #[serde(default = "one")]
    #[schemars(range(min = 1, max = 100_000))]
    at_frame: u32,
    /// IDs: fresh (`new`) or kept (`preserve`).
    #[serde(default)]
    id_policy: IdPolicy,
    /// ID and name conflicts.
    #[serde(default)]
    on_conflict: OnConflict,
    /// Colours whose ID already exists.
    #[serde(default)]
    colours: ColourPolicy,
    /// Report what would be imported; change nothing.
    #[serde(default)]
    #[allow(dead_code, reason = "read by the executor: a set `dryRun` makes the call a dry run (10 AG-02)")]
    dry_run: bool,
}

fn one() -> u32 {
    1
}

/// ID policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IdPolicy {
    /// Fresh IDs.
    #[default]
    New,
    /// Keep the bundle's IDs.
    Preserve,
}

/// Conflict policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnConflict {
    /// Rename the imported object.
    #[default]
    Rename,
    /// Replace the existing object.
    Replace,
    /// Fail with `doc.importConflict`.
    Fail,
}

/// Colour policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ColourPolicy {
    /// Existing swatches keep their values.
    #[default]
    KeepExisting,
    /// Existing swatches of the same palette take the imported values.
    UpdateExisting,
}

/// Objects created.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub(crate) struct Created {
    /// Scenes.
    scenes: Vec<String>,
    /// Elements.
    elements: Vec<String>,
    /// Drawings.
    drawings: Vec<String>,
    /// Palettes.
    palettes: Vec<String>,
    /// Assets.
    assets: Vec<String>,
}

/// A renamed object.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Renamed {
    /// Object id.
    id: String,
    /// Name in the bundle.
    from: String,
    /// Name in the project.
    to: String,
}

/// `doc.import.json` result.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportResult {
    /// Objects created.
    created: Created,
    /// Objects renamed on import.
    renamed: Vec<Renamed>,
    /// Colour IDs used by the bundle that neither the bundle nor the project defines (colour recovery, 03 §11.2, is
    /// M2 C2.2's; they stay unresolved).
    recovered_colour_ids: Vec<String>,
    /// Issues met (unresolved binary entries, conflicts resolved).
    issues: Vec<Value>,
}

fn conflict(conflicts: Vec<Value>) -> CommandError {
    CommandError::with_details(
        "doc.importConflict",
        format!("{} conflict(s) between the bundle and the project", conflicts.len()),
        json!({ "conflicts": conflicts }),
    )
    .hint("pass onConflict: \"rename\" or \"replace\"")
}

/// Object IDs defined inside a value (`id`, never colour IDs), with their types.
fn defined_ids(v: &Value, out: &mut BTreeMap<String, ObjectType>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            if let (Some(t), Some(id)) =
                (m.get("type").and_then(Value::as_str).and_then(ObjectType::parse), m.get("id").and_then(Value::as_str))
                && t != ObjectType::Swatch
            {
                out.insert(id.to_owned(), t);
            }
            for (k, x) in m {
                if k != "meta" && k != "ext" && k != "colourRecovery" {
                    defined_ids(x, out, depth + 1);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| defined_ids(x, out, depth + 1)),
        _ => {}
    }
}

/// Rewrites references: strings equal to a remapped ID; entry names (`entry`, texture `image`) containing one.
fn remap(v: &mut Value, map: &BTreeMap<String, String>, depth: usize) {
    if depth > 200 || map.is_empty() {
        return;
    }
    match v {
        Value::String(s) => {
            if let Some(n) = map.get(s.as_str()) {
                *s = n.clone();
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| remap(x, map, depth + 1)),
        Value::Object(m) => {
            let keys: Vec<String> = m.keys().cloned().collect();
            for k in keys {
                if k == "meta" || k == "ext" {
                    continue;
                }
                if let Some(x) = m.get_mut(&k) {
                    if (k == "entry" || k == "image") && x.is_string() {
                        if let Value::String(s) = x {
                            *s = remap_entry(s, map);
                        }
                    } else {
                        remap(x, map, depth + 1);
                    }
                }
                if let Some(n) = map.get(&k)
                    && let Some(x) = m.shift_remove(&k)
                {
                    m.insert(n.clone(), x);
                }
            }
        }
        _ => {}
    }
}

/// An entry name with every remapped ID segment (or `<id>.<ext>` file name) replaced.
fn remap_entry(entry: &str, map: &BTreeMap<String, String>) -> String {
    entry
        .split('/')
        .map(|seg| {
            if let Some(n) = map.get(seg) {
                return n.clone();
            }
            match seg.split_once('.') {
                Some((stem, ext)) if map.contains_key(stem) => format!("{}.{ext}", map[stem]),
                _ => seg.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn add_entry(changes: &mut Vec<(EntryKey, Vec<PatchOp>)>, key: EntryKey, value: Value) {
    changes.push((
        key,
        vec![PatchOp {
            op: "add",
            from: None,
            path: String::new(),
            range: None,
            value: Some(value),
            old_value: None,
            elided: false,
        }],
    ));
}

fn replace_op(path: &str, value: Value) -> PatchOp {
    PatchOp {
        op: "replace",
        from: None,
        path: path.into(),
        range: None,
        value: Some(value),
        old_value: None,
        elided: false,
    }
}

/// Splits a full-depth object into its own entry (split children as `{id, type}` stubs) and its split children.
fn split_entry(v: &Value) -> (Value, Vec<Value>) {
    let Some(ty) = v.get("type").and_then(Value::as_str).and_then(ObjectType::parse) else {
        return (v.clone(), Vec::new());
    };
    let mut own = v.clone();
    let mut children = Vec::new();
    for (field, slot) in slots_of(ty) {
        if let SlotKind::Split(cty) = slot
            && let Some(list) = own.get_mut(*field).and_then(Value::as_array_mut)
        {
            for c in list.iter_mut() {
                let id = c.get("id").cloned().unwrap_or(Value::Null);
                if c.as_object().is_some_and(|m| m.len() > 3) {
                    children.push(c.clone());
                }
                *c = json!({ "id": id, "type": cty.as_str() });
            }
        }
    }
    if let Some(m) = own.as_object_mut() {
        m.shift_remove("path");
    }
    (own, children)
}

/// Shifts an element's timing by `offset` frames (imported at `atFrame`).
fn shift_element(e: &mut Value, offset: u32) {
    if offset == 0 {
        return;
    }
    fn frames(v: &mut Value, offset: u32, depth: usize) {
        if depth > 50 {
            return;
        }
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    if matches!(k.as_str(), "frame" | "startFrame" | "stopFrame")
                        && let Some(n) = x.as_u64()
                    {
                        *x = json!(n + u64::from(offset));
                    } else if k != "meta" && k != "ext" {
                        frames(x, offset, depth + 1);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| frames(x, offset, depth + 1)),
            _ => {}
        }
    }
    if let Some(m) = e.as_object_mut() {
        if let Some(s) = m.get("start").and_then(Value::as_u64) {
            m.insert("start".into(), json!(s + u64::from(offset)));
        }
        if let Some(Value::Array(runs)) = m.get_mut("exposure")
            && !runs.is_empty()
        {
            runs.insert(0, json!([null, offset]));
        }
        if let Some(Value::Object(notes)) = m.get("cellNotes").cloned() {
            let shifted: Map<String, Value> = notes
                .into_iter()
                .map(|(k, v)| (k.parse::<u64>().map_or(k, |n| (n + u64::from(offset)).to_string()), v))
                .collect();
            m.insert("cellNotes".into(), Value::Object(shifted));
        }
        for field in ["channels", "placements", "sequences"] {
            if let Some(x) = m.get_mut(field) {
                frames(x, offset, 0);
            }
        }
    }
}

/// What `onConflict: "replace"` removes: entry removals and style-entry removals to apply with the import (one patch,
/// so a project never passes through a state without scenes), the removed scene and palette IDs, and the position of
/// the first replaced scene.
#[derive(Debug, Default)]
struct Replaced {
    removals: Vec<(EntryKey, Vec<PatchOp>)>,
    gone: BTreeSet<String>,
    scene_index: Option<usize>,
}

/// The objects that the bundle's objects replace by name (10 §10.2 `onConflict: "replace"`; C13-R1-11):
/// - without a target scene, the scenes named like a bundle scene (with their elements);
/// - with a target scene, the elements of that scene named like a bundle element (as `edit.delete` removes them);
/// - palettes named like a bundle palette, unless the bundle palette has the same ID (palettes merge by ID); their
///   style entries in the scenes go with them.
fn replace_conflicting(cx: &mut Cx, bundle: &Value, target: Option<&str>) -> Result<Replaced, CommandError> {
    let doc = cx.document()?.clone();
    let list = |k: &str| bundle.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let names_of = |v: &[Value]| -> BTreeSet<String> {
        v.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_owned)).collect()
    };
    let op = |op: &'static str, path: String, value: Option<Value>| PatchOp {
        op,
        from: None,
        path,
        range: None,
        value,
        old_value: None,
        elided: false,
    };
    let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
    let mut gone: BTreeSet<String> = BTreeSet::new();
    // Palettes.
    let bundle_palettes = list("palettes");
    let pal_names = names_of(&bundle_palettes);
    let pal_ids: BTreeSet<&str> = bundle_palettes.iter().filter_map(|v| v.get("id").and_then(Value::as_str)).collect();
    let palettes: Vec<String> = doc
        .palettes()
        .filter(|pl| pal_names.contains(pl.name().as_str()) && !pal_ids.contains(pl.id().as_str()))
        .map(|pl| pl.id().as_str().to_owned())
        .collect();
    // Scenes.
    let scenes: Vec<Id> = if target.is_none() {
        let wanted = names_of(&list("scenes"));
        doc.scenes().filter(|s| wanted.contains(s.name().as_str())).map(|s| s.id().clone()).collect()
    } else {
        Vec::new()
    };
    for sid in &scenes {
        gone.insert(sid.as_str().to_owned());
        for e in doc.scene(sid).map(|s| s.elements().ids().to_vec()).unwrap_or_default() {
            changes.push((EntryKey::Element(e), vec![op("remove", String::new(), None)]));
        }
        changes.push((EntryKey::Scene(sid.clone()), vec![op("remove", String::new(), None)]));
    }
    for pid in &palettes {
        gone.insert(pid.clone());
        changes.push((
            EntryKey::Palette(Id::parse(pid).map_err(CommandError::from)?),
            vec![op("remove", String::new(), None)],
        ));
        for s in doc.scenes().filter(|s| !scenes.contains(s.id())) {
            if s.palette_styles().contains_key(pid.as_str()) {
                changes.push((
                    EntryKey::Scene(s.id().clone()),
                    vec![op("remove", format!("/paletteStyles/{}", tt_model::pointer::escape(pid)), None)],
                ));
            }
        }
    }
    let scene_index = scenes.first().and_then(|first| doc.project().scenes().ids().iter().position(|s| s == first));
    // Elements of the target scene.
    if let Some(t) = target {
        let sid = Id::parse(&cx.resolve_as(t, ObjectType::Scene)?).map_err(CommandError::from)?;
        let doc = cx.document()?.clone();
        let mut bundle_elements = list("elements");
        for s in list("scenes") {
            bundle_elements.extend(s.get("elements").and_then(Value::as_array).cloned().unwrap_or_default());
        }
        let wanted = names_of(&bundle_elements);
        let ids: Vec<String> = doc
            .scene(&sid)
            .map(|s| s.elements().ids().to_vec())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| doc.element(e).is_some_and(|x| wanted.contains(x.name().as_str())))
            .map(|e| e.as_str().to_owned())
            .collect();
        // Elements are removed as `edit.delete` removes them (tree, columns, references), before the import reads
        // the scene.
        super::edit_ops::delete_elements(cx, ids)?;
    }
    Ok(Replaced { removals: changes, gone, scene_index })
}

fn unique(base: &str, taken: &BTreeSet<String>, ty: ObjectType) -> String {
    tt_persist::repair::unique_name(ty, base, taken)
}

/// Reads the bundle of `source`.
fn read_source(cx: &Cx, source: ImportSource) -> Result<(Value, Option<PathBuf>), CommandError> {
    match source {
        ImportSource::Bundle(m) => Ok((Value::Object(m), None)),
        ImportSource::Path(p) => {
            let path = persistence::path_param(cx, &p, "/source")?;
            if !cx.origin().has_scope(tt_registry::Scope::FilesRead) {
                return Err(CommandError::with_details(
                    codes::PERMISSION_DENIED,
                    "reading a bundle file needs the scope files.read",
                    json!({ "commandId": "doc.import.json", "missingScopes": ["files.read"] }),
                ));
            }
            let bytes = fsx::read_limited(&path, tt_persist::load::JSON_LIMIT)
                .map_err(|e| CommandError::from(fsx::fs_error(&e, &path, "cannot read")))?;
            let text = String::from_utf8(bytes).map_err(|_| invalid("/source", "the bundle is not UTF-8"))?;
            let v: Value = tt_model::json_text::parse(&text)?;
            Ok((v, path.parent().map(std::path::Path::to_path_buf)))
        }
    }
}

#[allow(clippy::too_many_lines)]
fn import(cx: &mut Cx, p: ImportJsonParams) -> Result<ImportResult, CommandError> {
    let (mut bundle, folder) = read_source(cx, p.source)?;
    if bundle.get("format").and_then(Value::as_str) != Some("animjson") {
        return Err(invalid("/source", "not an interchange bundle (`format` is not `animjson`)"));
    }
    let version = bundle.get("formatVersion").and_then(Value::as_str).and_then(tt_persist::manifest::Version::parse);
    if version.is_none_or(|v| v.major != tt_persist::manifest::Version::current().major) {
        return Err(CommandError::with_details(
            tt_persist::codes::UNSUPPORTED_VERSION,
            "the bundle's format version is not readable",
            json!({ "formatVersion": bundle.get("formatVersion") }),
        ));
    }
    // `replace` (C13-R1-11): the existing objects the imported ones conflict with by name go first, in this same
    // transaction; the import then meets no name conflict.
    let replaced = if p.on_conflict == OnConflict::Replace {
        replace_conflicting(cx, &bundle, p.target.as_deref())?
    } else {
        Replaced::default()
    };
    let gone = |id: &str| replaced.gone.contains(id);
    let doc = cx.document()?.clone();
    let mut result = ImportResult::default();
    let source_project = bundle.get("sourceProjectId").and_then(Value::as_str).map(str::to_owned);
    let arr = |b: &Value, k: &str| b.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    // IDs: what the bundle defines, and the mapping that applies.
    let mut defined = BTreeMap::new();
    for k in ["scenes", "elements", "drawingBanks", "palettes", "pens", "assets"] {
        for v in arr(&bundle, k) {
            defined_ids(&v, &mut defined, 0);
        }
    }
    let mut ids = IdGenerator::system();
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let mut conflicts = Vec::new();
    for (id, ty) in &defined {
        let exists = doc.locate(id).is_some();
        let fresh = match p.id_policy {
            IdPolicy::New => ty.local_id_container().is_none(),
            IdPolicy::Preserve => {
                if exists {
                    conflicts.push(json!({ "kind": "id", "id": id, "type": ty.as_str() }));
                }
                exists && p.on_conflict != OnConflict::Fail
            }
        };
        if fresh {
            map.insert(id.clone(), ids.next(*ty)?.as_str().to_owned());
        }
    }
    if p.on_conflict == OnConflict::Fail && !conflicts.is_empty() && p.id_policy == IdPolicy::Preserve {
        return Err(conflict(conflicts));
    }
    // Entry names of the bundle's binaries, before the remap.
    let blobs_in = bundle.get("blobs").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut blob_names = BTreeSet::new();
    for k in ["drawingBanks", "assets", "palettes"] {
        for v in arr(&bundle, k) {
            entries(&v, &mut blob_names, 0);
        }
    }
    remap(&mut bundle, &map, 0);
    let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = replaced.removals.clone();
    let mut project = doc.entry_raw_json(&EntryKey::Project)?;
    let mut project_changed = !replaced.gone.is_empty();
    for k in ["scenes", "palettes"] {
        if let Some(stubs) = project.get_mut(k).and_then(Value::as_array_mut) {
            stubs.retain(|s| s.get("id").and_then(Value::as_str).is_none_or(|id| !gone(id)));
        }
    }
    // Palettes.
    let mut palette_first_style: BTreeMap<String, String> = BTreeMap::new();
    let mut palette_names: BTreeSet<String> =
        doc.palettes().filter(|x| !gone(x.id().as_str())).map(|x| x.name().clone()).collect();
    let existing_colours: BTreeMap<String, String> = doc
        .palettes()
        .filter(|x| !gone(x.id().as_str()))
        .flat_map(|pal| {
            pal.swatches().iter().map(move |s| (s.colour_id().as_str().to_owned(), pal.id().as_str().to_owned()))
        })
        .collect();
    let mut defined_colours: BTreeSet<String> = existing_colours.keys().cloned().collect();
    for mut pal in arr(&bundle, "palettes") {
        let pid = pal.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        if let Some(m) = pal.as_object_mut() {
            m.shift_remove("path");
        }
        let incoming: Vec<Value> = arr(&pal, "swatches");
        if let Some(existing) = doc.palette(&Id::parse(&pid).map_err(CommandError::from)?) {
            // Merge by palette ID.
            let mut current = doc.entry_raw_json(&EntryKey::Palette(existing.id().clone()))?;
            let mut list = arr(&current, "swatches");
            for s in incoming {
                let cid = s.get("colourId").and_then(Value::as_str).unwrap_or_default().to_owned();
                defined_colours.insert(cid.clone());
                match existing_colours.get(&cid) {
                    Some(owner) if owner == &pid => {
                        if p.colours == ColourPolicy::UpdateExisting
                            && let Some(slot) = list
                                .iter_mut()
                                .find(|x| x.get("colourId").and_then(Value::as_str) == Some(cid.as_str()))
                            && let (Some(m), Some(v)) = (slot.as_object_mut(), s.get("values"))
                        {
                            m.insert("values".into(), v.clone());
                        }
                    }
                    Some(_) => {}
                    None => list.push(s),
                }
            }
            if let Some(m) = current.as_object_mut() {
                m.insert("swatches".into(), Value::Array(list));
            }
            changes.push((EntryKey::Palette(existing.id().clone()), vec![replace_op("", current)]));
            continue;
        }
        // A new palette: swatches whose colour ID exists elsewhere stay with their existing definition.
        let kept: Vec<Value> = incoming
            .into_iter()
            .filter(|s| s.get("colourId").and_then(Value::as_str).is_none_or(|c| !existing_colours.contains_key(c)))
            .collect();
        for s in &kept {
            if let Some(c) = s.get("colourId").and_then(Value::as_str) {
                defined_colours.insert(c.to_owned());
            }
        }
        if kept.is_empty() {
            continue;
        }
        let name = pal.get("name").and_then(Value::as_str).unwrap_or("Palette").to_owned();
        if palette_names.contains(&name) {
            match p.on_conflict {
                OnConflict::Fail => conflicts.push(json!({ "kind": "name", "type": "palette", "name": name })),
                _ => {
                    let to = unique(&name, &palette_names, ObjectType::Palette);
                    result.renamed.push(Renamed { id: pid.clone(), from: name.clone(), to: to.clone() });
                    if let Some(m) = pal.as_object_mut() {
                        m.insert("name".into(), json!(to));
                    }
                }
            }
        }
        palette_names.insert(pal.get("name").and_then(Value::as_str).unwrap_or_default().to_owned());
        if let Some(m) = pal.as_object_mut() {
            m.insert("swatches".into(), Value::Array(kept));
        }
        if let Some(st) = pal.pointer("/styles/0/id").and_then(Value::as_str) {
            palette_first_style.insert(pid.clone(), st.to_owned());
        }
        if let Some(list) = project.get_mut("palettes").and_then(Value::as_array_mut) {
            list.push(json!({ "id": pid, "type": "palette" }));
        }
        project_changed = true;
        add_entry(&mut changes, EntryKey::Palette(Id::parse(&pid).map_err(CommandError::from)?), pal);
        result.created.palettes.push(pid);
    }
    // Pens (project scope).
    let mut pen_names: BTreeSet<String> = doc.project().pens().iter().map(|x| x.name().clone()).collect();
    for mut pen in arr(&bundle, "pens") {
        let name = pen.get("name").and_then(Value::as_str).unwrap_or("Pen").to_owned();
        if pen_names.contains(&name) {
            let to = unique(&name, &pen_names, ObjectType::Pen);
            if let Some(m) = pen.as_object_mut() {
                m.insert("name".into(), json!(to));
            }
        }
        pen_names.insert(pen.get("name").and_then(Value::as_str).unwrap_or_default().to_owned());
        if let Some(m) = pen.as_object_mut() {
            m.shift_remove("path");
        }
        if let Some(list) = project.get_mut("pens").and_then(Value::as_array_mut) {
            list.push(pen);
            project_changed = true;
        }
    }
    // Assets and their binary entries.
    for mut a in arr(&bundle, "assets") {
        if let Some(m) = a.as_object_mut() {
            m.shift_remove("path");
        }
        let id = a.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        if let Some(list) = project.get_mut("assets").and_then(Value::as_array_mut) {
            list.push(a);
            project_changed = true;
        }
        result.created.assets.push(id);
    }
    let mut new_blobs: Vec<(String, BlobData)> = Vec::new();
    for old in &blob_names {
        let new = remap_entry(old, &map);
        let bytes = blobs_in
            .get(old)
            .and_then(|b| b.get("base64"))
            .and_then(Value::as_str)
            .and_then(b64::decode)
            .or_else(|| {
                folder.as_ref().and_then(|f| std::fs::read(old.split('/').fold(f.clone(), |p, s| p.join(s))).ok())
            })
            .or_else(|| {
                (source_project.as_deref() == Some(doc.project().id().as_str()))
                    .then(|| cx.project().ok().and_then(|p| p.store.blobs.read(old, 1 << 32).ok()))
                    .flatten()
            });
        match bytes {
            Some(b) => new_blobs.push((new, BlobData::Bytes(std::sync::Arc::new(b)))),
            None => result.issues.push(json!({ "code": tt_persist::codes::MEDIA_MISSING, "entry": new, "message": "the binary entry is not in the bundle, its folder or the project" })),
        }
    }
    // Drawing banks and drawings.
    for bank in arr(&bundle, "drawingBanks") {
        let (own, drawings) = split_entry(&bank);
        let bid = own.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
        for d in drawings {
            let (dv, _) = split_entry(&d);
            let did = dv.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            add_entry(&mut changes, EntryKey::Drawing(Id::parse(&did).map_err(CommandError::from)?), dv);
            result.created.drawings.push(did);
        }
        if let Some(list) = project.get_mut("drawingBanks").and_then(Value::as_array_mut) {
            list.push(json!({ "id": bid, "type": "drawingBank" }));
            project_changed = true;
        }
        add_entry(&mut changes, EntryKey::Bank(Id::parse(&bid).map_err(CommandError::from)?), own);
    }
    // Palette styles of every scene cover every palette.
    let all_palettes: Vec<(String, String)> = doc
        .palettes()
        .filter(|x| !gone(x.id().as_str()))
        .filter_map(|pal| pal.styles().first().map(|s| (pal.id().as_str().to_owned(), s.id().as_str().to_owned())))
        .chain(palette_first_style.iter().map(|(a, b)| (a.clone(), b.clone())))
        .collect();
    let offset = p.at_frame.saturating_sub(1);
    match &p.target {
        Some(t) => {
            let sid = cx.resolve_as(t, ObjectType::Scene)?;
            let mut scene = doc.entry_raw_json(&EntryKey::Scene(Id::parse(&sid).map_err(CommandError::from)?))?;
            if let Some(ps) = scene.get_mut("paletteStyles").and_then(Value::as_object_mut) {
                ps.retain(|pal, _| !gone(pal));
            }
            let mut names: BTreeSet<String> = arr(&scene, "elements")
                .iter()
                .filter_map(|e| e.get("id").and_then(Value::as_str))
                .filter_map(|e| doc.element(&Id::parse(e).ok()?).map(|x| x.name().clone()))
                .collect();
            let (elements, trees, order): (Vec<Value>, Vec<Value>, Vec<Value>) = if arr(&bundle, "scenes").is_empty() {
                (arr(&bundle, "elements"), arr(&bundle, "tree"), arr(&bundle, "columnOrder"))
            } else {
                let mut e = Vec::new();
                let mut tr = Vec::new();
                let mut o = Vec::new();
                for s in arr(&bundle, "scenes") {
                    e.extend(arr(&s, "elements"));
                    tr.extend(arr(&s, "tree"));
                    o.extend(arr(&s, "columnOrder"));
                }
                (e, tr, o)
            };
            let mut cameras: Vec<String> = Vec::new();
            for mut e in elements {
                let (mut own, _) = split_entry(&e);
                shift_element(&mut own, offset);
                let name = own.get("name").and_then(Value::as_str).unwrap_or("Element").to_owned();
                let eid = own.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                if names.contains(&name) {
                    match p.on_conflict {
                        OnConflict::Fail => conflicts.push(json!({ "kind": "name", "type": "element", "name": name })),
                        _ => {
                            let to = unique(&name, &names, ObjectType::Element);
                            result.renamed.push(Renamed { id: eid.clone(), from: name.clone(), to: to.clone() });
                            if let Some(m) = own.as_object_mut() {
                                m.insert("name".into(), json!(to));
                            }
                        }
                    }
                }
                names.insert(own.get("name").and_then(Value::as_str).unwrap_or_default().to_owned());
                if let Some(list) = scene.get_mut("elements").and_then(Value::as_array_mut) {
                    list.push(json!({ "id": eid, "type": "element" }));
                }
                if own.get("elementType").and_then(Value::as_str) == Some("camera") {
                    cameras.push(eid.clone());
                }
                e = own;
                add_entry(&mut changes, EntryKey::Element(Id::parse(&eid).map_err(CommandError::from)?), e);
                result.created.elements.push(eid);
            }
            // A scene without a current camera (its camera was replaced) takes the imported scene's, or the first
            // imported camera (10 §5: a scene with cameras has a current camera).
            if scene.get("currentCameraId").is_none_or(Value::is_null) {
                let preferred = arr(&bundle, "scenes")
                    .iter()
                    .find_map(|s| s.get("currentCameraId").and_then(Value::as_str).map(str::to_owned))
                    .filter(|c| cameras.contains(c));
                if let (Some(c), Some(m)) = (preferred.or_else(|| cameras.first().cloned()), scene.as_object_mut()) {
                    m.insert("currentCameraId".into(), json!(c));
                }
            }
            if let Some(list) = scene.get_mut("tree").and_then(Value::as_array_mut) {
                list.extend(trees);
            }
            if let Some(list) = scene.get_mut("columnOrder").and_then(Value::as_array_mut) {
                list.extend(order);
            }
            if let Some(ps) = scene.get_mut("paletteStyles").and_then(Value::as_object_mut) {
                for (pal, st) in &palette_first_style {
                    ps.entry(pal.clone()).or_insert(json!(st));
                }
            }
            changes.push((EntryKey::Scene(Id::parse(&sid).map_err(CommandError::from)?), vec![replace_op("", scene)]));
        }
        None => {
            let mut scene_names: BTreeSet<String> =
                doc.scenes().filter(|s| !gone(s.id().as_str())).map(|s| s.name().clone()).collect();
            let mut stubs = Vec::new();
            for s in arr(&bundle, "scenes") {
                let (mut own, elements) = split_entry(&s);
                let sid = own.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                let name = own.get("name").and_then(Value::as_str).unwrap_or("Scene-1").to_owned();
                if scene_names.contains(&name) {
                    match p.on_conflict {
                        OnConflict::Fail => conflicts.push(json!({ "kind": "name", "type": "scene", "name": name })),
                        _ => {
                            let to = unique(&name, &scene_names, ObjectType::Scene);
                            result.renamed.push(Renamed { id: sid.clone(), from: name.clone(), to: to.clone() });
                            if let Some(m) = own.as_object_mut() {
                                m.insert("name".into(), json!(to));
                            }
                        }
                    }
                }
                scene_names.insert(own.get("name").and_then(Value::as_str).unwrap_or_default().to_owned());
                // Every palette of the project has a style entry (10 §4.3).
                let old_ps = own.get("paletteStyles").and_then(Value::as_object).cloned().unwrap_or_default();
                let mut ps = Map::new();
                for (pal, st) in &all_palettes {
                    ps.insert(pal.clone(), old_ps.get(pal).cloned().unwrap_or_else(|| json!(st)));
                }
                if let Some(m) = own.as_object_mut() {
                    m.insert("paletteStyles".into(), Value::Object(ps));
                }
                for e in elements {
                    let (ev, _) = split_entry(&e);
                    let eid = ev.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                    add_entry(&mut changes, EntryKey::Element(Id::parse(&eid).map_err(CommandError::from)?), ev);
                    result.created.elements.push(eid);
                }
                add_entry(&mut changes, EntryKey::Scene(Id::parse(&sid).map_err(CommandError::from)?), own);
                stubs.push(json!({ "id": sid, "type": "scene" }));
                result.created.scenes.push(sid);
            }
            if !stubs.is_empty()
                && let Some(list) = project.get_mut("scenes").and_then(Value::as_array_mut)
            {
                // A replaced scene's place goes to its replacement unless `index` says otherwise.
                let at = p
                    .index
                    .map(|i| usize::try_from(i).unwrap_or(usize::MAX))
                    .or(replaced.scene_index)
                    .map_or(list.len(), |i| i.min(list.len()));
                for (k, s) in stubs.into_iter().enumerate() {
                    list.insert(at + k, s);
                }
                project_changed = true;
            }
        }
    }
    if !conflicts.is_empty() {
        return Err(conflict(conflicts));
    }
    // New palettes get a style entry in every existing scene.
    if !palette_first_style.is_empty() {
        for s in doc.scenes().filter(|s| !gone(s.id().as_str())) {
            let earlier = changes.iter().position(|(k, _)| *k == EntryKey::Scene(s.id().clone()));
            // A scene this import rewrites whole already has its styles; one with style removals gets these too.
            if earlier.is_some_and(|i| changes[i].1.iter().any(|o| o.path.is_empty())) {
                continue;
            }
            let mut ops = Vec::new();
            for (pal, st) in &palette_first_style {
                ops.push(PatchOp {
                    op: "add",
                    from: None,
                    path: format!("/paletteStyles/{}", tt_model::pointer::escape(pal)),
                    range: None,
                    value: Some(json!(st)),
                    old_value: None,
                    elided: false,
                });
            }
            match earlier {
                Some(i) => changes[i].1.extend(ops),
                None => changes.push((EntryKey::Scene(s.id().clone()), ops)),
            }
        }
    }
    if project_changed {
        changes.insert(0, (EntryKey::Project, vec![replace_op("", project)]));
    }
    // Colour IDs the imported content uses that nothing defines.
    let mut used = BTreeSet::new();
    for k in ["scenes", "elements", "drawingBanks", "pens"] {
        for v in arr(&bundle, k) {
            colours(&v, &mut used, 0);
        }
    }
    result.recovered_colour_ids = used.into_iter().filter(|c| !defined_colours.contains(c)).collect();
    if changes.is_empty() {
        return Ok(result);
    }
    cx.patch_entries(changes)?;
    if !cx.dry_run()
        && let Ok(proj) = cx.project_mut()
    {
        for (n, d) in new_blobs {
            proj.store.blobs.insert(n, d);
        }
    }
    Ok(result)
}

#[command(
    id = "doc.import.json",
    kind = action,
    label = "Import JSON",
    scopes = [edit],
    undo = "Import JSON",
    needs_project,
    example(title = "Dry-run an import of an inline bundle", params = r#"{"source": {"format": "animjson", "formatVersion": "1.0", "scenes": [], "elements": [], "drawingBanks": [], "palettes": [], "pens": [], "assets": [], "blobs": {}}, "dryRun": true}"#),
    agent_only = "agents exchange object-model bundles",
)]
pub(crate) fn doc_import_json(cx: &mut Cx, p: ImportJsonParams) -> Result<ImportResult, CommandError> {
    import(cx, p)
}
