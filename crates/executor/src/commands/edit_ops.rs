//! `edit.cut`, `.copy`, `.paste`, `.delete`, `.cutSpecial`, `.pasteSpecial`, `.pasteAsNewElement` (01 §6.2–§6.3;
//! C12-R1-14): the object-level semantics over the object kinds the M1 model has, through the executor. Each command
//! acts on the current edit context (01 §6.2; `auto` follows the focused panel) and its selection (or `objectIds`):
//!
//! | Context | Cut / copy / delete act on | Clipboard kind | Paste puts |
//! |---|---|---|---|
//! | `drawing` | strokes and drawing nodes | `drawingObjects` | into the current drawing (strokes into its first layer) |
//! | `staging`, `timeline` | elements (`timeline`: cells when cells are selected) | `elements` | new elements in the current scene (clones share the drawing bank) |
//! | `xsheet`, `timeline` | cells (`element:@<id>/cell:#<n>`; a selected column means all its cells) | `cells` | exposures at `{elementId, frame}` (default: current element and frame) |
//! | `functionEditor` | keys / a motion point | `keyframes` / `motionPoint` | keys at `{functionId, frame}`; motion-point values onto selected points |
//! | `library` | library folders and items | `templates` | copies into `{catalogId}` (default: the library root) |
//!
//! Copies get new IDs (container-local for nodes, strokes and keys) and unique names; a paste never changes the
//! clipboard. The Timeline/Exposure-Sheet options of 01 §6.3 apply to cut special and paste special:
//! `createDeleteExposure` (cut special removes the frames instead of blanking them; paste special inserts instead of
//! overwriting), `overwriteDrawings` (a pasted drawing from another bank replaces a same-name drawing or is copied),
//! `extendExposures`, and the key-group options (`motionKeys` … `skewKeys`) that choose which channels' keys are
//! cut or pasted. `updateColours`, `keyframeSubstitution` and `boundaryKeys` need templates (M9) and curve
//! evaluation (M3 C3.2); when set they are reported as not applied (warning `edit.optionNotApplied`). Options are
//! remembered per operation (`remember`), and `pasteSpecial {useLast}` is paste special again. Pasting an image is
//! image import (M10, `command.notImplemented`); other mismatches are `edit.pasteNotAllowed`, a drawing into an
//! element without exposure is `edit.wrongElementType`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryKey, Target};
use tt_model::entities::drawing::{Drawing, DrawingBank, DrawingNode, LayerHint};
use tt_model::entities::element::{Element, ElementInit, ElementType};
use tt_model::value::Timestamp;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};

use super::common::EditContext;
use super::edit::{Clipboard, ClipboardKind, ClipboardSummary, resolve_context, selection_in, store_clipboard};
use crate::app::{cell_ref, parse_cell_ref};
use crate::cx::Cx;
use crate::types::Warning;

fn op(kind: &'static str, path: &str, value: Option<Value>) -> PatchOp {
    PatchOp { op: kind, from: None, path: path.to_owned(), range: None, value, old_value: None, elided: false }
}

fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::internal(e.to_string())
}

fn to_json<T: Serialize>(v: &T) -> Result<Value, CommandError> {
    serde_json::to_value(v).map_err(internal)
}

fn not_allowed(kind: ClipboardKind, context: EditContext, why: &str) -> CommandError {
    CommandError::with_details(
        "edit.pasteNotAllowed",
        format!("cannot paste {} in the {} context: {why}", kind.as_str(), context.as_str()),
        json!({ "kind": kind.as_str(), "context": context.as_str(), "reason": why }),
    )
}

fn nothing_selected(context: EditContext) -> CommandError {
    CommandError::with_details(
        codes::COMMAND_DISABLED,
        format!("nothing is selected in the {} context", context.as_str()),
        json!({ "context": context.as_str(), "reason": "noSelection" }),
    )
    .hint("select objects (edit.selection.set) or pass objectIds")
}

fn wrong(context: EditContext, what: &str, id: &str) -> CommandError {
    CommandError::with_details(
        tt_model::codes::REF_WRONG_TYPE,
        format!("a {what} cannot be edited in the {} context", context.as_str()),
        json!({ "id": id, "context": context.as_str() }),
    )
}

/// JSON Pointer tokens with array indices compared as numbers (for removing from the end first).
fn pointer_key(p: &str) -> Vec<(u8, u64, String)> {
    p.split('/')
        .skip(1)
        .map(|t| match t.parse::<u64>() {
            Ok(n) => (0, n, String::new()),
            Err(_) => (1, 0, t.to_owned()),
        })
        .collect()
}

// ----- options (01 §6.3) ------------------------------------------------------------------------------------------

/// Timeline / Exposure Sheet edit options (01 §12.4 `TimelineEditOptions`); omitted fields take the remembered values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
pub(crate) struct TimelineEditOptions {
    overwrite_drawings: bool,
    create_delete_exposure: bool,
    extend_exposures: bool,
    update_colours: bool,
    keyframe_substitution: bool,
    motion_keys: bool,
    rotation_keys: bool,
    scale_keys: bool,
    pivot_keys: bool,
    velocity_keys: bool,
    skew_keys: bool,
    boundary_keys: bool,
}

impl Default for TimelineEditOptions {
    fn default() -> Self {
        Self {
            overwrite_drawings: true,
            create_delete_exposure: true,
            extend_exposures: false,
            update_colours: true,
            keyframe_substitution: false,
            motion_keys: true,
            rotation_keys: true,
            scale_keys: true,
            pivot_keys: true,
            velocity_keys: true,
            skew_keys: true,
            boundary_keys: false,
        }
    }
}

impl TimelineEditOptions {
    /// Whether keys of a channel take part (the key-group options; other channels always do).
    fn channel_enabled(&self, channel: &str) -> bool {
        match channel {
            "position" | "position.x" | "position.y" | "position.z" | "offset.x" | "offset.y" | "offset.z" => {
                self.motion_keys
            }
            "rotation" | "angle" => self.rotation_keys,
            c if c.starts_with("scale") => self.scale_keys,
            c if c.starts_with("pivot") => self.pivot_keys,
            "velocity" => self.velocity_keys,
            "skew" => self.skew_keys,
            _ => true,
        }
    }

    /// The options that are set but need later milestones.
    fn not_applied(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.keyframe_substitution {
            v.push("keyframeSubstitution (M3 C3.2)");
        }
        if self.boundary_keys {
            v.push("boundaryKeys (M3 C3.2)");
        }
        v
    }
}

/// Motion-point paste options (01 §12.4 `MotionPointPasteOptions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
pub(crate) struct MotionPointPasteOptions {
    x: bool,
    y: bool,
    z: bool,
    tension: bool,
    continuity: bool,
    bias: bool,
    angle: bool,
    rotation_pivot: bool,
    scale_x: bool,
    scale_y: bool,
    scale_pivot: bool,
}

impl Default for MotionPointPasteOptions {
    fn default() -> Self {
        Self {
            x: true,
            y: true,
            z: true,
            tension: true,
            continuity: true,
            bias: true,
            angle: true,
            rotation_pivot: true,
            scale_x: true,
            scale_y: true,
            scale_pivot: true,
        }
    }
}

/// The options of one special operation: the remembered (or default) ones overlaid with the given fields.
fn merged<T: Default + Serialize + for<'de> Deserialize<'de>>(
    cx: &Cx,
    operation: &str,
    given: Option<&Map<String, Value>>,
) -> Result<T, CommandError> {
    let mut base = match cx.session().edit_options.get(operation) {
        Some(m) => Value::Object(m.clone()),
        None => to_json(&T::default())?,
    };
    if let (Some(given), Some(b)) = (given, base.as_object_mut()) {
        for (k, v) in given {
            b.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(base).map_err(|e| {
        CommandError::with_details(
            codes::ARG_INVALID,
            format!("invalid options: {e}"),
            json!({ "pointer": "/options" }),
        )
    })
}

// ----- what an edit acts on ---------------------------------------------------------------------------------------

/// The objects an edit acts on.
enum Picked {
    Drawing(Vec<String>),
    Elements(Vec<String>),
    Cells(Vec<(String, u32)>),
    Keys(Vec<String>),
    Points(Vec<String>),
    Library(Vec<String>),
}

/// The frames of an element's exposure, one drawing ID (or blank) per frame.
fn frames_of(doc: &Document, element: &str) -> Vec<Option<String>> {
    let Some(e) = Id::parse(element).ok().and_then(|id| doc.element(&id).cloned()) else { return Vec::new() };
    let mut out = Vec::new();
    for (d, n) in e.exposure().iter().flatten() {
        for _ in 0..*n {
            out.push(d.as_ref().map(|x| x.as_str().to_owned()));
        }
    }
    out
}

/// Exposure runs (10 §4.7) for frames; trailing blanks are dropped.
fn runs_of(frames: &[Option<String>]) -> Value {
    let mut runs: Vec<(Option<String>, u32)> = Vec::new();
    for f in frames {
        match runs.last_mut() {
            Some((d, n)) if d == f => *n += 1,
            _ => runs.push((f.clone(), 1)),
        }
    }
    while runs.last().is_some_and(|(d, _)| d.is_none()) {
        runs.pop();
    }
    Value::Array(runs.into_iter().map(|(d, n)| json!([d, n])).collect())
}

fn has_exposure(doc: &Document, element: &str) -> bool {
    Id::parse(element).ok().and_then(|id| doc.element(&id).map(|e| e.element_type().has_exposure())).unwrap_or(false)
}

fn pick(cx: &Cx, context: EditContext, object_ids: Option<Vec<String>>) -> Result<Picked, CommandError> {
    let ids = object_ids.unwrap_or_else(|| selection_in(cx, context));
    if ids.is_empty() {
        return Err(nothing_selected(context));
    }
    let doc = cx.document()?;
    let mut objects: Vec<(ObjectType, String)> = Vec::new();
    let mut cells: Vec<(String, u32)> = Vec::new();
    for r in &ids {
        if let Some(c) = parse_cell_ref(r) {
            cells.push(c);
            continue;
        }
        match cx.resolve(r)? {
            Target::Object { ty, key } => objects.push((ty, key)),
            Target::Cell { element, frame } => cells.push((element.as_str().to_owned(), frame)),
        }
    }
    let only = |types: &[ObjectType]| -> Result<Vec<String>, CommandError> {
        if let Some((e, _)) = cells.first() {
            return Err(wrong(context, "cell", e));
        }
        objects
            .iter()
            .map(|(t, k)| if types.contains(t) { Ok(k.clone()) } else { Err(wrong(context, t.as_str(), k)) })
            .collect()
    };
    Ok(match context {
        EditContext::Drawing => Picked::Drawing(only(&[ObjectType::Stroke, ObjectType::DrawingNode])?),
        EditContext::Staging => Picked::Elements(only(&[ObjectType::Element])?),
        EditContext::Timeline if cells.is_empty() => Picked::Elements(only(&[ObjectType::Element])?),
        EditContext::Xsheet | EditContext::Timeline => {
            // A selected column stands for all its cells.
            for (t, k) in &objects {
                if *t != ObjectType::Element {
                    return Err(wrong(context, t.as_str(), k));
                }
                let n = u32::try_from(frames_of(doc, k).len()).unwrap_or(u32::MAX);
                cells.extend((1..=n).map(|f| (k.clone(), f)));
            }
            for (e, _) in &cells {
                if !has_exposure(doc, e) {
                    return Err(CommandError::with_details(
                        "edit.wrongElementType",
                        format!("element `{e}` has no exposure cells"),
                        json!({ "elementId": e }),
                    ));
                }
            }
            cells.sort();
            cells.dedup();
            Picked::Cells(cells)
        }
        EditContext::FunctionEditor => {
            if objects.iter().all(|(t, _)| *t == ObjectType::Key) {
                Picked::Keys(only(&[ObjectType::Key])?)
            } else {
                Picked::Points(only(&[ObjectType::MotionPoint])?)
            }
        }
        EditContext::Library => Picked::Library(only(&[ObjectType::LibraryItem, ObjectType::LibraryFolder])?),
        EditContext::Auto => return Err(CommandError::internal("unresolved edit context")),
    })
}

/// Reports what a cut, copy or delete acts on (11 P4, §7.2 `targets`; C12-R2-08): the context and the objects or
/// cells, whether given or taken from the selection.
fn report_picked(cx: &mut Cx, context: EditContext, picked: &Picked) {
    let ids: Vec<String> = match picked {
        Picked::Drawing(k) | Picked::Elements(k) | Picked::Keys(k) | Picked::Points(k) | Picked::Library(k) => {
            k.clone()
        }
        Picked::Cells(c) => c.iter().map(|(e, f)| cell_ref(e, *f)).collect(),
    };
    cx.set_targets(json!({ "context": context.as_str(), "ids": ids }));
}

/// Drawing-object keys without those inside another selected node (cutting a layer cuts its strokes).
fn top_level(doc: &Document, keys: &[String]) -> Vec<String> {
    let set: BTreeSet<&String> = keys.iter().collect();
    keys.iter()
        .filter(|k| {
            let mut cur = doc.locate(k).and_then(|l| l.parent.clone());
            while let Some(p) = cur {
                if set.contains(&p) {
                    return false;
                }
                cur = doc.locate(&p).filter(|l| l.ty == ObjectType::DrawingNode).and_then(|l| l.parent.clone());
            }
            true
        })
        .cloned()
        .collect()
}

/// `(channel name, element id)` of a key or motion point.
fn channel_of(doc: &Document, key: &str) -> Result<(String, String, String), CommandError> {
    let loc = doc.locate(key).ok_or_else(|| internal("indexed key"))?;
    let channel = loc.parent.clone().ok_or_else(|| internal("key without channel"))?;
    let element =
        doc.locate(&channel).and_then(|l| l.parent.clone()).ok_or_else(|| internal("channel without element"))?;
    let el = Id::parse(&element).ok().and_then(|id| doc.element(&id)).ok_or_else(|| internal("element"))?;
    let name = el
        .channels()
        .iter()
        .flatten()
        .find(|(_, c)| c.id().as_str() == channel)
        .map(|(n, _)| n.clone())
        .ok_or_else(|| internal("channel name"))?;
    Ok((name, element, channel))
}

// ----- copy ---------------------------------------------------------------------------------------------------------

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// The clipboard content of a selection; `opts` filters keys by group (cut special).
fn copy_content(
    cx: &Cx,
    context: EditContext,
    picked: &Picked,
    opts: Option<&TimelineEditOptions>,
) -> Result<Clipboard, CommandError> {
    let doc = cx.document()?;
    let raw = |k: &str| -> Result<Map<String, Value>, CommandError> { Ok(obj(doc.object_raw_json(k)?.1)) };
    let (kind, items): (ClipboardKind, Vec<Map<String, Value>>) = match picked {
        Picked::Drawing(keys) => {
            (ClipboardKind::DrawingObjects, top_level(doc, keys).iter().map(|k| raw(k)).collect::<Result<_, _>>()?)
        }
        Picked::Elements(keys) => {
            // 05 §15: the selected nodes with all their descendants; each item says where it hangs (`parentId`).
            let mut items = Vec::new();
            for (_, picked) in with_descendants(doc, keys)? {
                for (id, parent) in picked {
                    let mut m = raw(&id)?;
                    m.insert("parentId".into(), json!(parent));
                    items.push(m);
                }
            }
            (ClipboardKind::Elements, items)
        }
        Picked::Cells(cells) => {
            let first = cells.iter().map(|(_, f)| *f).min().unwrap_or(1);
            let mut items = Vec::with_capacity(cells.len());
            for (e, f) in cells {
                let frames = frames_of(doc, e);
                let d = frames.get((*f as usize).saturating_sub(1)).cloned().flatten();
                items.push(obj(json!({ "elementId": e, "frameOffset": f - first, "drawingRef": d })));
            }
            (ClipboardKind::Cells, items)
        }
        Picked::Keys(keys) => {
            let mut items = Vec::new();
            for k in keys {
                let (channel, element, _) = channel_of(doc, k)?;
                if opts.is_some_and(|o| !o.channel_enabled(&channel)) {
                    continue;
                }
                items.push(obj(json!({ "channel": channel, "elementId": element, "key": doc.object_raw_json(k)?.1 })));
            }
            (ClipboardKind::Keyframes, items)
        }
        Picked::Points(keys) => {
            let Some(k) = keys.first() else { return Err(nothing_selected(context)) };
            let (channel, element, _) = channel_of(doc, k)?;
            (
                ClipboardKind::MotionPoint,
                vec![obj(json!({ "channel": channel, "elementId": element, "point": doc.object_raw_json(k)?.1 }))],
            )
        }
        Picked::Library(keys) => {
            (ClipboardKind::Templates, top_level_library(doc, keys).iter().map(|k| raw(k)).collect::<Result<_, _>>()?)
        }
    };
    Ok(Clipboard {
        kind,
        source_context: Some(context.as_str().to_owned()),
        source_project_id: Some(doc.project().id().as_str().to_owned()),
        item_count: u32::try_from(items.len()).unwrap_or(u32::MAX),
        items: Some(items),
        options: match opts {
            Some(o) => Some(obj(to_json(o)?)),
            None => None,
        },
    })
}

fn top_level_library(doc: &Document, keys: &[String]) -> Vec<String> {
    let set: BTreeSet<&String> = keys.iter().collect();
    keys.iter()
        .filter(|k| {
            let mut cur = doc.locate(k).and_then(|l| l.parent.clone());
            while let Some(p) = cur {
                if set.contains(&p) {
                    return false;
                }
                cur = doc.locate(&p).and_then(|l| l.parent.clone());
            }
            true
        })
        .cloned()
        .collect()
}

fn store(cx: &mut Cx, clip: &Clipboard) -> Result<ClipboardSummary, CommandError> {
    store_clipboard(cx, clip)
}

// ----- delete -------------------------------------------------------------------------------------------------------

fn check_drawing_unlocked(doc: &Document, drawing: &str) -> Result<(), CommandError> {
    if Id::parse(drawing).ok().and_then(|d| doc.drawing(&d).map(|x| *x.locked())).unwrap_or(false) {
        return Err(CommandError::with_details(
            "drawing.locked",
            format!("drawing `{drawing}` is locked"),
            json!({ "drawingId": drawing }),
        ));
    }
    Ok(())
}

/// Removal operations for objects inside entries (from the last array position first).
fn removals(doc: &Document, keys: &[String]) -> Result<Vec<(EntryKey, Vec<PatchOp>)>, CommandError> {
    let mut by_entry: Vec<(EntryKey, Vec<String>)> = Vec::new();
    for k in keys {
        let (entry, _, at) = doc.object_pointer(k)?;
        match by_entry.iter_mut().find(|(e, _)| *e == entry) {
            Some((_, v)) => v.push(at),
            None => by_entry.push((entry, vec![at])),
        }
    }
    Ok(by_entry
        .into_iter()
        .map(|(e, mut ptrs)| {
            ptrs.sort_by_key(|p| std::cmp::Reverse(pointer_key(p)));
            (e, ptrs.iter().map(|p| op("remove", p, None)).collect())
        })
        .collect())
}

/// The Timeline tree without the given nodes; the children of a removed node take its place (04 `element.delete`:
/// children are re-attached to the deleted element's parent).
fn tree_without(nodes: &Value, gone: &BTreeSet<String>) -> Value {
    let mut out = Vec::new();
    for n in nodes.as_array().into_iter().flatten() {
        let children = n.get("children").map(|c| tree_without(c, gone));
        if n.get("id").and_then(Value::as_str).is_some_and(|id| gone.contains(id)) {
            out.extend(children.and_then(|c| c.as_array().cloned()).unwrap_or_default());
        } else {
            let mut n = n.clone();
            if let Some(c) = children {
                n["children"] = c;
            }
            out.push(n);
        }
    }
    Value::Array(out)
}

/// The Timeline tree of a scene as `child → parent` (roots map to `None`) and its pre-order.
fn tree_shape(nodes: &Value) -> (BTreeMap<String, Option<String>>, Vec<String>) {
    fn walk(
        nodes: &Value,
        parent: Option<&str>,
        parents: &mut BTreeMap<String, Option<String>>,
        order: &mut Vec<String>,
    ) {
        for n in nodes.as_array().into_iter().flatten() {
            let Some(id) = n.get("id").and_then(Value::as_str) else { continue };
            parents.insert(id.to_owned(), parent.map(str::to_owned));
            order.push(id.to_owned());
            if let Some(c) = n.get("children") {
                walk(c, Some(id), parents, order);
            }
        }
    }
    let (mut parents, mut order) = (BTreeMap::new(), Vec::new());
    walk(nodes, None, &mut parents, &mut order);
    (parents, order)
}

/// Elements of one scene in Timeline pre-order, each with its parent in the scene tree.
type Forest = Vec<(String, Option<String>)>;

/// The selected elements and all their descendants (05 §15), per scene, in Timeline pre-order, each with its
/// parent in the scene tree.
fn with_descendants(doc: &Document, keys: &[String]) -> Result<Vec<(String, Forest)>, CommandError> {
    let mut by_scene: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for k in keys {
        let id = Id::parse(k)?;
        let scene = doc.scene_of(&id).ok_or_else(|| internal("element without scene"))?;
        by_scene.entry(scene.as_str().to_owned()).or_default().insert(k.clone());
    }
    let mut out = Vec::new();
    for (scene, roots) in by_scene {
        let sj = doc.entry_raw_json(&EntryKey::Scene(Id::parse(&scene)?))?;
        let (parents, order) = tree_shape(&sj["tree"]);
        let inside = |id: &str| {
            let mut cur = Some(id.to_owned());
            while let Some(c) = cur {
                if roots.contains(&c) {
                    return true;
                }
                cur = parents.get(&c).cloned().flatten();
            }
            false
        };
        let picked: Vec<(String, Option<String>)> =
            order.iter().filter(|id| inside(id)).map(|id| (id.clone(), parents.get(id).cloned().flatten())).collect();
        out.push((scene, picked));
    }
    Ok(out)
}

/// Derived and transient element properties (AG-04): carried on the clipboard as information, never stored.
const ELEMENT_DERIVED: [&str; 11] = [
    "path",
    "parentId",
    "childIds",
    "depth",
    "timelineIndex",
    "xsheetIndex",
    "cloneGroupId",
    "cloneIds",
    "drawingCount",
    "hiddenByParent",
    "effectiveMarkerColour",
];

/// Rewrites element references (schema `x-ref: "element"` locations, never inside opaque data) through `map`.
fn remap_element_refs(v: &mut Value, map: &BTreeMap<String, String>, at: &str) {
    match v {
        Value::String(s) => {
            if let Some(new) = map.get(s.as_str())
                && tt_model::schema::property_schema(ObjectType::Element, at)
                    .is_some_and(|x| x.get("x-ref").and_then(Value::as_str) == Some("element"))
            {
                *s = new.clone();
            }
        }
        Value::Object(m) => {
            for (k, x) in m.iter_mut().filter(|(k, _)| !OPAQUE.contains(&k.as_str())) {
                remap_element_refs(x, map, &format!("{at}/{}", tt_model::pointer::escape(k)));
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                remap_element_refs(x, map, &format!("{at}/{i}"));
            }
        }
        _ => {}
    }
}

/// Opaque members (10 §2.2): never interpreted, never traversed.
const OPAQUE: [&str; 2] = ["meta", "ext"];

/// Fixes for references to deleted elements in an element's JSON (10 §7.2 dangling-reference repair, applied when
/// elements are deleted; SI-022). Only locations the element schema declares as element references (`x-ref:
/// "element"`) count, and opaque `meta`/`ext` data is never entered (C12-R2-05): a reference inside a list item
/// removes the item, a listed ID removes the entry, any other reference becomes `null`.
fn reference_fixes(v: &Value, ids: &BTreeSet<String>, at: &str, in_item: Option<&str>, out: &mut Vec<PatchOp>) {
    let is_ref = |p: &str| {
        tt_model::schema::property_schema(ObjectType::Element, p)
            .is_some_and(|s| s.get("x-ref").and_then(Value::as_str) == Some("element"))
    };
    match v {
        Value::String(s) if ids.contains(s) && is_ref(at) => match in_item {
            Some(item) => {
                if !out.iter().any(|o| o.path == item) {
                    out.push(op("remove", item, None));
                }
            }
            None => out.push(op("replace", at, Some(Value::Null))),
        },
        Value::Object(m) => {
            for (k, x) in m.iter().filter(|(k, _)| !OPAQUE.contains(&k.as_str())) {
                reference_fixes(x, ids, &format!("{at}/{}", tt_model::pointer::escape(k)), in_item, out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                let here = format!("{at}/{i}");
                match x {
                    Value::String(s) if ids.contains(s) && is_ref(&here) => out.push(op("remove", &here, None)),
                    Value::Object(_) => reference_fixes(x, ids, &here, Some(&here), out),
                    _ => reference_fixes(x, ids, &here, in_item, out),
                }
            }
        }
        _ => {}
    }
}

/// Deletes the picked objects (staged); returns the deleted keys (cells as cell references).
/// `descendants`: a cut takes the selected nodes with their descendants (05 §15); a delete re-attaches the children
/// to the deleted element's parent (04 §8.3).
/// Deletes elements as `edit.delete` does (tree, column order, references; SI-022); used by `doc.import.json` with
/// `onConflict: "replace"`.
pub(crate) fn delete_elements(cx: &mut Cx, ids: Vec<String>) -> Result<(), CommandError> {
    if ids.is_empty() {
        return Ok(());
    }
    delete(cx, &Picked::Elements(ids), false, false).map(|_| ())
}

fn delete(cx: &mut Cx, picked: &Picked, remove_frames: bool, descendants: bool) -> Result<Vec<String>, CommandError> {
    let doc = cx.document()?.clone();
    match picked {
        Picked::Drawing(keys) => {
            let keys = top_level(&doc, keys);
            for k in &keys {
                if let Some((d, _)) = k.split_once('/') {
                    check_drawing_unlocked(&doc, d)?;
                }
            }
            cx.patch_entries(removals(&doc, &keys)?)?;
            Ok(keys)
        }
        Picked::Keys(keys) | Picked::Points(keys) => {
            cx.patch_entries(removals(&doc, keys)?)?;
            Ok(keys.clone())
        }
        Picked::Library(keys) => {
            let keys = top_level_library(&doc, keys);
            cx.patch_entries(removals(&doc, &keys)?)?;
            Ok(keys)
        }
        Picked::Elements(keys) => {
            let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
            let mut deleted: BTreeSet<String> = BTreeSet::new();
            let mut scenes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            let all: Vec<String> = if descendants {
                with_descendants(&doc, keys)?.into_iter().flat_map(|(_, p)| p.into_iter().map(|(id, _)| id)).collect()
            } else {
                keys.clone()
            };
            for k in &all {
                let id = Id::parse(k)?;
                if doc.element(&id).is_some_and(|e| *e.locked()) {
                    return Err(CommandError::with_details(
                        "element.locked",
                        format!("element `{k}` is locked"),
                        json!({ "elementId": k }),
                    ));
                }
                let scene = doc.scene_of(&id).ok_or_else(|| internal("element without scene"))?;
                scenes.entry(scene.as_str().to_owned()).or_default().insert(k.clone());
            }
            for (scene, roots) in scenes {
                let sid = Id::parse(&scene)?;
                let sj = doc.entry_raw_json(&EntryKey::Scene(sid.clone()))?;
                let gone = roots;
                let stubs: Vec<Value> = sj["elements"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|s| s.get("id").and_then(Value::as_str).is_none_or(|id| !gone.contains(id)))
                    .cloned()
                    .collect();
                let columns: Vec<Value> = sj["columnOrder"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c.as_str().is_none_or(|id| !gone.contains(id)))
                    .cloned()
                    .collect();
                let mut ops = vec![
                    op("replace", "/elements", Some(Value::Array(stubs))),
                    op("replace", "/tree", Some(tree_without(&sj["tree"], &gone))),
                    op("replace", "/columnOrder", Some(Value::Array(columns))),
                ];
                if sj["currentCameraId"].as_str().is_some_and(|c| gone.contains(c)) {
                    ops.push(op("replace", "/currentCameraId", Some(Value::Null)));
                }
                changes.push((EntryKey::Scene(sid.clone()), ops));
                for id in &gone {
                    changes.push((EntryKey::Element(Id::parse(id)?), vec![op("remove", "", None)]));
                }
                // References from the remaining elements of the scene to the deleted ones.
                for other in doc.scene(&sid).map(|sc| sc.elements().ids().to_vec()).unwrap_or_default() {
                    if gone.contains(other.as_str()) {
                        continue;
                    }
                    let key = EntryKey::Element(other.clone());
                    let mut fixes = Vec::new();
                    reference_fixes(&doc.entry_raw_json(&key)?, &gone, "", None, &mut fixes);
                    if !fixes.is_empty() {
                        fixes.sort_by_key(|o| std::cmp::Reverse(pointer_key(&o.path)));
                        changes.push((key, fixes));
                    }
                }
                deleted.extend(gone);
            }
            cx.patch_entries(changes)?;
            Ok(deleted.into_iter().collect())
        }
        Picked::Cells(cells) => {
            let mut by_element: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
            for (e, f) in cells {
                by_element.entry(e.clone()).or_default().insert(*f);
            }
            let mut changes = Vec::new();
            for (e, frames) in &by_element {
                let mut all = frames_of(&doc, e);
                for f in frames.iter().rev() {
                    let i = (*f as usize).saturating_sub(1);
                    if i < all.len() {
                        if remove_frames {
                            all.remove(i);
                        } else {
                            all[i] = None;
                        }
                    }
                }
                changes.push((EntryKey::Element(Id::parse(e)?), vec![op("replace", "/exposure", Some(runs_of(&all)))]));
            }
            cx.patch_entries(changes)?;
            Ok(cells.iter().map(|(e, f)| cell_ref(e, *f)).collect())
        }
    }
}

// ----- paste --------------------------------------------------------------------------------------------------------

/// The child slots of the model's containment tree (10 §3): the only places a copied object holds other objects.
const CHILD_SLOTS: [&str; 9] =
    ["children", "nodes", "strokes", "channels", "keys", "points", "blocks", "placements", "sequences"];

/// Gives a copied model object, and the objects in its child slots, new IDs where their type is one of `types`
/// (container-local types get local IDs). Only the containment slots are followed; opaque `meta`/`ext` and every
/// other property are copied verbatim, whatever they look like (C12-R2-05).
fn regenerate(
    cx: &mut Cx,
    v: &mut Value,
    types: &[ObjectType],
    created: &mut Vec<(ObjectType, String)>,
) -> Result<(), CommandError> {
    let Value::Object(m) = v else { return Ok(()) };
    if let Some(ty) = m.get("type").and_then(Value::as_str).and_then(ObjectType::parse)
        && types.contains(&ty)
        && m.contains_key("id")
    {
        let id = cx.new_id(ty)?;
        m.insert("id".into(), json!(id.as_str()));
        m.insert("rev".into(), json!(0));
        created.push((ty, id.as_str().to_owned()));
    }
    for slot in CHILD_SLOTS {
        if let Some(x) = m.get_mut(slot) {
            regenerate_slot(cx, x, types, created)?;
        }
    }
    Ok(())
}

/// The objects of one child slot: a list, or a map of objects (`channels`).
fn regenerate_slot(
    cx: &mut Cx,
    v: &mut Value,
    types: &[ObjectType],
    created: &mut Vec<(ObjectType, String)>,
) -> Result<(), CommandError> {
    match v {
        Value::Array(a) => a.iter_mut().try_for_each(|x| regenerate(cx, x, types, created)),
        Value::Object(m) => m.values_mut().try_for_each(|x| regenerate(cx, x, types, created)),
        _ => Ok(()),
    }
}

/// `name`, or `name-2`, `name-3`, … — the first not in `taken`.
fn unique(name: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_owned();
    }
    (2..).map(|n| format!("{name}-{n}")).find(|n| !taken.contains(n)).unwrap_or_else(|| name.to_owned())
}

/// The pointer of the first layer node of a drawing.
fn first_layer(nodes: &Value, at: &str) -> Option<String> {
    for (i, n) in nodes.as_array().into_iter().flatten().enumerate() {
        let here = format!("{at}/{i}");
        if n.get("kind").and_then(Value::as_str) == Some("layer") {
            return Some(here);
        }
        if let Some(found) = n.get("children").and_then(|c| first_layer(c, &format!("{here}/children"))) {
            return Some(found);
        }
    }
    None
}

/// Paste destination (01 §12.1): `{drawingId}`, `{elementId, frame}`, `{catalogId}`, `{functionId, frame}`,
/// `{objectIds}` or `{sceneId}`. Typed and closed: a wrong-typed or unknown field is refused by the parameter schema
/// before any default applies (C12-R2-09).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Destination {
    /// Drawing to paste drawing objects into (default: the current drawing).
    #[serde(default, rename = "drawingId")]
    drawing: Option<String>,
    /// Element (column) to paste cells into (default: the current element).
    #[serde(default, rename = "elementId")]
    element: Option<String>,
    /// First frame (default: the current frame).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Library or folder to paste templates into (default: the project library).
    #[serde(default, rename = "catalogId")]
    catalog: Option<String>,
    /// Function (channel) to paste keys into (default: the same-named channel of the current element).
    #[serde(default, rename = "functionId")]
    function: Option<String>,
    /// Objects to paste onto (motion points, element properties; default: the selection).
    #[serde(default, rename = "objectIds")]
    #[schemars(length(max = 100_000))]
    objects: Option<Vec<String>>,
    /// Scene to paste elements into (default: the current scene).
    #[serde(default, rename = "sceneId")]
    scene: Option<String>,
}

/// What a paste created and modified.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasteResult {
    /// Created objects.
    created_ids: Vec<String>,
    /// Modified objects.
    modified_ids: Vec<String>,
    /// Warnings (options that do not apply).
    warnings: Vec<Warning>,
}

/// Special options of one paste.
#[derive(Clone, Copy)]
enum Special {
    None,
    Timeline(TimelineEditOptions),
    MotionPoint(MotionPointPasteOptions),
}

fn current_scene(cx: &Cx, given: Option<&str>) -> Result<Id, CommandError> {
    match given {
        Some(s) => Ok(Id::parse(&cx.resolve_as(s, ObjectType::Scene)?)?),
        None => {
            cx.app().scene.clone().ok_or_else(|| {
                CommandError::with_details(codes::COMMAND_DISABLED, "there is no current scene", json!({}))
            })
        }
    }
}

/// Stages the copy of a drawing into a bank (new ID, unique name) and returns its ID.
fn copy_drawing(
    cx: &mut Cx,
    doc: &Document,
    source: &str,
    bank: &Id,
    taken: &mut BTreeSet<String>,
    changes: &mut Vec<(EntryKey, Vec<PatchOp>)>,
    content: Option<&Value>,
) -> Result<String, CommandError> {
    let mut v = match content {
        Some(c) => c.clone(),
        None => doc.entry_raw_json(&EntryKey::Drawing(Id::parse(source)?))?,
    };
    let id = cx.new_id(ObjectType::Drawing)?;
    let name = unique(v.get("name").and_then(Value::as_str).unwrap_or("1"), taken);
    taken.insert(name.clone());
    v["id"] = json!(id.as_str());
    v["rev"] = json!(0);
    v["name"] = json!(name);
    changes.push((EntryKey::Drawing(id.clone()), vec![op("add", "", Some(v))]));
    changes.push((
        EntryKey::Bank(bank.clone()),
        vec![op("add", "/drawings/-", Some(json!({ "id": id.as_str(), "type": "drawing" })))],
    ));
    Ok(id.as_str().to_owned())
}

fn bank_names(doc: &Document, bank: &Id) -> BTreeSet<String> {
    doc.bank(bank)
        .map(|b| b.drawings().ids().iter().filter_map(|d| doc.drawing(d)).map(|d| d.name().clone()).collect())
        .unwrap_or_default()
}

#[allow(clippy::too_many_lines)]
fn paste(
    cx: &mut Cx,
    context: EditContext,
    clip: &Clipboard,
    dest: &Destination,
    special: Special,
) -> Result<PasteResult, CommandError> {
    let doc = cx.document()?.clone();
    let items: Vec<Map<String, Value>> = clip.items.clone().unwrap_or_default();
    let mut out = PasteResult::default();
    let topts = match special {
        Special::Timeline(o) => Some(o),
        _ => None,
    };
    if let Some(o) = topts {
        for name in o.not_applied() {
            out.warnings.push(Warning {
                code: "edit.optionNotApplied".into(),
                message: format!("option {name} is not applied yet"),
                object_ids: Vec::new(),
            });
        }
    }
    let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
    match clip.kind {
        ClipboardKind::DrawingObjects => {
            if context != EditContext::Drawing && dest.drawing.is_none() {
                return Err(not_allowed(clip.kind, context, "drawing objects paste into a drawing"));
            }
            let drawing = match &dest.drawing {
                Some(d) => cx.resolve_as(d, ObjectType::Drawing)?,
                None => cx
                    .resolve_as("@current", ObjectType::Drawing)
                    .map_err(|_| not_allowed(clip.kind, context, "there is no current drawing"))?,
            };
            check_drawing_unlocked(&doc, &drawing)?;
            cx.set_targets(json!({ "context": context.as_str(), "destination": { "drawingId": drawing } }));
            let dj = doc.entry_raw_json(&EntryKey::Drawing(Id::parse(&drawing)?))?;
            let mut ops = Vec::new();
            for item in items {
                let mut v = Value::Object(item);
                let mut created = Vec::new();
                regenerate(cx, &mut v, &[ObjectType::DrawingNode, ObjectType::Stroke], &mut created)?;
                let ty = v.get("type").and_then(Value::as_str).unwrap_or_default();
                if let Some((_, id)) = created.first() {
                    out.created_ids.push(format!("{drawing}/{id}"));
                }
                if ty == "stroke" {
                    let layer = first_layer(&dj["nodes"], "/nodes")
                        .ok_or_else(|| not_allowed(clip.kind, context, "the drawing has no layer"))?;
                    ops.push(op("add", &format!("{layer}/strokes/-"), Some(v)));
                } else {
                    ops.push(op("add", "/nodes/-", Some(v)));
                }
            }
            changes.push((EntryKey::Drawing(Id::parse(&drawing)?), ops));
        }
        ClipboardKind::Elements => {
            if !matches!(context, EditContext::Staging | EditContext::Timeline) && dest.scene.is_none() {
                return Err(not_allowed(clip.kind, context, "elements paste into the Timeline or the camera view"));
            }
            let scene = current_scene(cx, dest.scene.as_deref())?;
            cx.set_targets(json!({ "context": context.as_str(), "destination": { "sceneId": scene.as_str() } }));
            let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
            let mut taken: BTreeSet<String> =
                sc.elements().ids().iter().filter_map(|e| doc.element(e)).map(|e| e.name().clone()).collect();
            // Every copied element gets a new ID first, so references between copied elements (the scene tree,
            // `x-ref: "element"` properties) are remapped onto the copies (05 §15: the copied forest is pasted whole).
            let mut prepared: Vec<(String, Option<String>, Value)> = Vec::new();
            for item in items {
                // A reference item (`{elementId}`) pastes a copy of that element of the project (SI-021).
                let (old, parent, v) = match item.get("elementId").and_then(Value::as_str) {
                    Some(r) => {
                        let key = cx
                            .resolve_as(r, ObjectType::Element)
                            .map_err(|_| not_allowed(clip.kind, context, "a copied element is not in this project"))?;
                        (key.clone(), None, doc.object_raw_json(&key)?.1)
                    }
                    None => {
                        let old = item.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let parent = item.get("parentId").and_then(Value::as_str).map(str::to_owned);
                        (old, parent, Value::Object(item))
                    }
                };
                prepared.push((old, parent, v));
            }
            let mut map: BTreeMap<String, String> = BTreeMap::new();
            for (old, _, _) in &prepared {
                let id = cx.new_id(ObjectType::Element)?;
                map.insert(old.clone(), id.as_str().to_owned());
            }
            let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let mut roots: Vec<String> = Vec::new();
            let mut scene_ops = Vec::new();
            for (old, parent, mut v) in prepared {
                let id = map.get(&old).cloned().ok_or_else(|| internal("mapped id"))?;
                match parent.as_ref().and_then(|p| map.get(p)) {
                    Some(np) => children.entry(np.clone()).or_default().push(id.clone()),
                    None => roots.push(id.clone()),
                }
                if let Some(bank) = v.get("drawingBankId").and_then(Value::as_str)
                    && !Id::parse(bank).ok().is_some_and(|b| doc.bank(&b).is_some())
                {
                    return Err(not_allowed(
                        clip.kind,
                        context,
                        "the copied element's drawings are not in this project",
                    ));
                }
                if let Some(m) = v.as_object_mut() {
                    for k in ELEMENT_DERIVED {
                        m.shift_remove(k);
                    }
                }
                let mut nested = Vec::new();
                for key in ["channels", "blocks", "placements", "sequences"] {
                    if let Some(x) = v.get_mut(key) {
                        regenerate_slot(
                            cx,
                            x,
                            &[
                                ObjectType::Channel,
                                ObjectType::ParticleBlock,
                                ObjectType::SoundPlacement,
                                ObjectType::MediaSequence,
                            ],
                            &mut nested,
                        )?;
                    }
                }
                remap_element_refs(&mut v, &map, "");
                let name = unique(v.get("name").and_then(Value::as_str).unwrap_or("Element"), &taken);
                taken.insert(name.clone());
                v["id"] = json!(id);
                v["rev"] = json!(0);
                v["name"] = json!(name);
                let cell_based = v
                    .get("elementType")
                    .and_then(|t| serde_json::from_value::<ElementType>(t.clone()).ok())
                    .is_some_and(ElementType::is_cell_based);
                scene_ops.push(op("add", "/elements/-", Some(json!({ "id": id, "type": "element" }))));
                if cell_based {
                    scene_ops.push(op("add", "/columnOrder/-", Some(json!(id))));
                }
                changes.push((EntryKey::Element(Id::parse(&id)?), vec![op("add", "", Some(v))]));
                out.created_ids.push(id);
            }
            fn node(id: &str, children: &BTreeMap<String, Vec<String>>) -> Value {
                let kids: Vec<Value> = children.get(id).into_iter().flatten().map(|c| node(c, children)).collect();
                json!({ "id": id, "children": kids })
            }
            for r in &roots {
                scene_ops.push(op("add", "/tree/-", Some(node(r, &children))));
            }
            changes.push((EntryKey::Scene(scene), scene_ops));
        }
        ClipboardKind::Cells => {
            if !matches!(context, EditContext::Xsheet | EditContext::Timeline) && dest.element.is_none() {
                return Err(not_allowed(clip.kind, context, "cells paste into the Exposure Sheet or the Timeline"));
            }
            let target = match &dest.element {
                Some(e) => cx.resolve_as(e, ObjectType::Element)?,
                None => cx
                    .resolve_as("@current", ObjectType::Element)
                    .map_err(|_| not_allowed(clip.kind, context, "there is no current element"))?,
            };
            let start = dest.frame.unwrap_or(cx.app().frame).max(1);
            cx.set_targets(
                json!({ "context": context.as_str(), "destination": { "elementId": target, "frame": start } }),
            );
            // Source columns in order of first appearance map onto the target column and the following ones.
            let mut sources: Vec<String> = Vec::new();
            for it in &items {
                if let Some(e) = it.get("elementId").and_then(Value::as_str)
                    && !sources.iter().any(|x| x == e)
                {
                    sources.push(e.to_owned());
                }
            }
            let tid = Id::parse(&target)?;
            let scene = doc.scene_of(&tid).and_then(|s| doc.scene(s)).ok_or_else(|| internal("scene"))?;
            let order = scene.column_order();
            let at = order.iter().position(|c| *c == tid).unwrap_or(0);
            let opts = topts.unwrap_or_default();
            let insert = topts.is_some() && opts.create_delete_exposure;
            for (n, src) in sources.iter().enumerate() {
                let t = if n == 0 {
                    tid.clone()
                } else {
                    order.get(at + n).cloned().ok_or_else(|| {
                        not_allowed(clip.kind, context, "not enough columns to the right of the target")
                    })?
                };
                let tel = doc.element(&t).ok_or_else(|| internal("element"))?;
                if !tel.element_type().has_exposure() {
                    return Err(CommandError::with_details(
                        "edit.wrongElementType",
                        format!("element `{}` has no exposure cells", tel.name()),
                        json!({ "elementId": t.as_str() }),
                    ));
                }
                let bank = tel.drawing_bank_id().clone().ok_or_else(|| internal("cell element without bank"))?;
                let mut names = bank_names(&doc, &bank);
                let mut mapped: BTreeMap<String, String> = BTreeMap::new();
                let mut frames = frames_of(&doc, t.as_str());
                let block: Vec<(u32, Option<String>)> = items
                    .iter()
                    .filter(|it| it.get("elementId").and_then(Value::as_str) == Some(src.as_str()))
                    .map(|it| {
                        (
                            u32::try_from(it.get("frameOffset").and_then(Value::as_u64).unwrap_or(0)).unwrap_or(0),
                            it.get("drawingRef").and_then(Value::as_str).map(str::to_owned),
                        )
                    })
                    .collect();
                let width = block.iter().map(|(o, _)| *o + 1).max().unwrap_or(0) as usize;
                let mut placed: Vec<Option<String>> = vec![None; width];
                for (offset, d) in &block {
                    let resolved = match d {
                        None => None,
                        Some(d) => {
                            if let Some(m) = mapped.get(d) {
                                Some(m.clone())
                            } else {
                                let did = Id::parse(d)?;
                                let in_bank = doc.bank_of(&did) == Some(&bank);
                                let id = if in_bank {
                                    d.clone()
                                } else if let Some(src_drawing) = doc.drawing(&did) {
                                    let same = doc.bank(&bank).and_then(|b| {
                                        b.drawings()
                                            .ids()
                                            .iter()
                                            .find(|x| doc.drawing(x).is_some_and(|y| y.name() == src_drawing.name()))
                                            .cloned()
                                    });
                                    match same {
                                        Some(existing) if opts.overwrite_drawings || topts.is_none() => {
                                            // Overwriting content of a locked drawing is refused (04 §13.16).
                                            check_drawing_unlocked(&doc, existing.as_str())?;
                                            let nodes = to_json(src_drawing.nodes())?;
                                            changes.push((
                                                EntryKey::Drawing(existing.clone()),
                                                vec![op("replace", "/nodes", Some(nodes))],
                                            ));
                                            out.modified_ids.push(existing.as_str().to_owned());
                                            existing.as_str().to_owned()
                                        }
                                        _ => {
                                            let id = copy_drawing(cx, &doc, d, &bank, &mut names, &mut changes, None)?;
                                            out.created_ids.push(id.clone());
                                            id
                                        }
                                    }
                                } else {
                                    return Err(not_allowed(
                                        clip.kind,
                                        context,
                                        "a copied drawing is not in this project",
                                    ));
                                };
                                mapped.insert(d.clone(), id.clone());
                                Some(id)
                            }
                        }
                    };
                    if let Some(slot) = placed.get_mut(*offset as usize) {
                        *slot = resolved;
                    }
                }
                let from = (start as usize).saturating_sub(1);
                if from > frames.len() {
                    // Before the pasted block: blanks, or the last drawing extended (extendExposures).
                    let fill =
                        if opts.extend_exposures && topts.is_some() { frames.last().cloned().flatten() } else { None };
                    frames.resize(from, fill);
                }
                if insert {
                    for (i, f) in placed.into_iter().enumerate() {
                        frames.insert(from + i, f);
                    }
                } else {
                    if frames.len() < from + width {
                        frames.resize(from + width, None);
                    }
                    for (i, f) in placed.into_iter().enumerate() {
                        frames[from + i] = f;
                    }
                }
                changes.push((EntryKey::Element(t.clone()), vec![op("replace", "/exposure", Some(runs_of(&frames)))]));
                out.modified_ids.push(t.as_str().to_owned());
            }
        }
        ClipboardKind::Keyframes | ClipboardKind::FunctionPoints => {
            if context != EditContext::FunctionEditor && dest.function.is_none() {
                return Err(not_allowed(clip.kind, context, "keys paste into a function"));
            }
            let base = dest.frame.unwrap_or(cx.app().frame).max(1);
            let first = items
                .iter()
                .filter_map(|it| it.get("key").and_then(|k| k.get("frame")).and_then(Value::as_u64))
                .min()
                .unwrap_or(1);
            // Target channel per item: the given function, else the channel of the same name on the current element.
            let fixed = match &dest.function {
                Some(f) => Some(cx.resolve_as(f, ObjectType::Channel)?),
                None => None,
            };
            cx.set_targets(
                json!({ "context": context.as_str(), "destination": { "functionId": fixed, "frame": base } }),
            );
            let mut per_channel: BTreeMap<String, Vec<Value>> = BTreeMap::new();
            for it in &items {
                let name = it.get("channel").and_then(Value::as_str).unwrap_or_default();
                if topts.is_some_and(|o| !o.channel_enabled(name)) {
                    continue;
                }
                let channel = match &fixed {
                    Some(c) => c.clone(),
                    None => {
                        let el = cx
                            .app()
                            .element
                            .clone()
                            .and_then(|e| doc.element(&e).cloned())
                            .ok_or_else(|| not_allowed(clip.kind, context, "there is no current element"))?;
                        el.channels()
                            .iter()
                            .flatten()
                            .find(|(n, _)| n.as_str() == name)
                            .map(|(_, c)| c.id().as_str().to_owned())
                            .ok_or_else(|| {
                                not_allowed(clip.kind, context, "the current element has no such function")
                            })?
                    }
                };
                let mut key = it.get("key").cloned().unwrap_or(Value::Null);
                let frame = key.get("frame").and_then(Value::as_u64).unwrap_or(first);
                key["frame"] = json!(u64::from(base) + frame - first);
                per_channel.entry(channel).or_default().push(key);
            }
            for (channel, mut keys) in per_channel {
                let (ename, element, _) = (
                    channel_name(&doc, &channel)?,
                    doc.locate(&channel).and_then(|l| l.parent.clone()).ok_or_else(|| internal("channel"))?,
                    (),
                );
                let el = doc.element(&Id::parse(&element)?).ok_or_else(|| internal("element"))?;
                let existing: Vec<Value> = el
                    .channels()
                    .iter()
                    .flatten()
                    .find(|(n, _)| **n == ename)
                    .and_then(|(_, c)| c.keys().clone())
                    .map(|ks| ks.iter().map(to_json).collect::<Result<Vec<_>, _>>())
                    .transpose()?
                    .ok_or_else(|| {
                        not_allowed(clip.kind, context, "the function has no keys (a motion path takes motion points)")
                    })?;
                let frames: BTreeSet<u64> =
                    keys.iter().filter_map(|k| k.get("frame").and_then(Value::as_u64)).collect();
                let mut all: Vec<Value> = existing
                    .into_iter()
                    .filter(|k| k.get("frame").and_then(Value::as_u64).is_none_or(|f| !frames.contains(&f)))
                    .collect();
                for k in &mut keys {
                    let id = cx.new_id(ObjectType::Key)?;
                    k["id"] = json!(id.as_str());
                    k["rev"] = json!(0);
                    out.created_ids.push(format!("{channel}/{}", id.as_str()));
                }
                all.extend(keys);
                all.sort_by_key(|k| k.get("frame").and_then(Value::as_u64).unwrap_or(0));
                let path = format!("/channels/{}/keys", tt_model::pointer::escape(&ename));
                changes.push((
                    EntryKey::Element(Id::parse(&element)?),
                    vec![op("replace", &path, Some(Value::Array(all)))],
                ));
            }
        }
        ClipboardKind::MotionPoint => {
            let opts = match special {
                Special::MotionPoint(o) => o,
                _ => MotionPointPasteOptions::default(),
            };
            let targets: Vec<String> = match &dest.objects {
                Some(ids) => ids.clone(),
                None => selection_in(cx, EditContext::FunctionEditor),
            };
            let points: Vec<String> = cx
                .resolve_objects(&targets)?
                .into_iter()
                .filter(|(t, _)| *t == ObjectType::MotionPoint)
                .map(|(_, k)| k)
                .collect();
            if points.is_empty() {
                return Err(not_allowed(clip.kind, context, "select motion points to paste onto"));
            }
            cx.set_targets(json!({ "context": context.as_str(), "destination": { "objectIds": points } }));
            let src = items.first().and_then(|it| it.get("point")).cloned().unwrap_or(Value::Null);
            let mut ops = Vec::new();
            for (flag, path) in [
                (opts.x, "/position/x"),
                (opts.y, "/position/y"),
                (opts.z, "/position/z"),
                (opts.tension, "/tension"),
                (opts.continuity, "/continuity"),
                (opts.bias, "/bias"),
            ] {
                if flag && let Some(v) = src.pointer(path) {
                    ops.push(op("replace", path, Some(v.clone())));
                }
            }
            let inapplicable: Vec<&str> = [
                (opts.angle, "angle"),
                (opts.rotation_pivot, "rotationPivot"),
                (opts.scale_x, "scaleX"),
                (opts.scale_y, "scaleY"),
                (opts.scale_pivot, "scalePivot"),
            ]
            .into_iter()
            .filter(|(f, _)| *f && matches!(special, Special::MotionPoint(_)))
            .map(|(_, n)| n)
            .collect();
            if !inapplicable.is_empty() {
                out.warnings.push(Warning {
                    code: "edit.optionNotApplied".into(),
                    message: format!(
                        "options {} do not apply to a motion point without time-locked rotation/scale keys",
                        inapplicable.join(", ")
                    ),
                    object_ids: points.clone(),
                });
            }
            cx.patch_objects(points.iter().map(|p| (p.clone(), ops.clone())).collect())?;
            out.modified_ids.extend(points);
            return Ok(out);
        }
        ClipboardKind::Templates => {
            if context != EditContext::Library && dest.catalog.is_none() {
                return Err(not_allowed(clip.kind, context, "templates paste into a library catalog"));
            }
            let container = match &dest.catalog {
                Some(c) => match cx.resolve(c)? {
                    Target::Object { ty: ObjectType::LibraryFolder | ObjectType::Library, key } => key,
                    _ => return Err(not_allowed(clip.kind, context, "catalogId names a library or folder")),
                },
                None => doc.library().id().as_str().to_owned(),
            };
            cx.set_targets(json!({ "context": context.as_str(), "destination": { "catalogId": container } }));
            let (entry, _, at) = doc.object_pointer(&container)?;
            let (_, cj) = doc.object_raw_json(&container)?;
            let mut taken: BTreeSet<String> = cj["children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect();
            let mut ops = Vec::new();
            for item in items {
                let mut v = Value::Object(item);
                let mut created = Vec::new();
                regenerate(cx, &mut v, &[ObjectType::LibraryFolder, ObjectType::LibraryItem], &mut created)?;
                let name = unique(v.get("name").and_then(Value::as_str).unwrap_or("Item"), &taken);
                taken.insert(name.clone());
                v["name"] = json!(name);
                if let Some((_, id)) = created.first() {
                    out.created_ids.push(id.clone());
                }
                ops.push(op("add", &format!("{at}/children/-"), Some(v)));
            }
            changes.push((entry, ops));
        }
        ClipboardKind::ElementProperties => {
            let targets = match &dest.objects {
                Some(ids) => ids.clone(),
                None => selection_in(cx, EditContext::Staging),
            };
            if targets.is_empty() {
                return Err(not_allowed(clip.kind, context, "select elements to paste properties onto"));
            }
            let mut props = Map::new();
            for item in items {
                props.extend(item);
            }
            let modified = super::doc::stage_props(cx, &targets, &props)?;
            cx.set_targets(json!({ "context": context.as_str(), "destination": { "objectIds": modified } }));
            out.modified_ids.extend(modified);
            return Ok(out);
        }
        ClipboardKind::Image => {
            return Err(CommandError::not_implemented("edit.paste (image import, 09)", "M10"));
        }
    }
    cx.patch_entries(changes)?;
    Ok(out)
}

fn channel_name(doc: &Document, channel: &str) -> Result<String, CommandError> {
    let element = doc.locate(channel).and_then(|l| l.parent.clone()).ok_or_else(|| internal("channel"))?;
    let el = doc.element(&Id::parse(&element)?).ok_or_else(|| internal("element"))?;
    el.channels()
        .iter()
        .flatten()
        .find(|(_, c)| c.id().as_str() == channel)
        .map(|(n, _)| n.clone())
        .ok_or_else(|| internal("channel name"))
}

fn clipboard(cx: &Cx) -> Result<Clipboard, CommandError> {
    match &cx.session().clipboard {
        Some(v) => serde_json::from_value(v.clone()).map_err(internal),
        None => Err(CommandError::with_details("edit.clipboardEmpty", "the clipboard is empty", json!({}))),
    }
}

// ----- declarations -------------------------------------------------------------------------------------------------

/// `context` + `objectIds` parameters of cut, copy and delete.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ObjectsParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Objects or cells (default: the selection of the context).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    object_ids: Option<Vec<String>>,
}

/// `{clipboard: {kind, itemCount}}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ClipResult {
    /// What the clipboard holds now.
    clipboard: ClipboardSummary,
}

#[command(
    id = "edit.cut",
    kind = action,
    label = "Cut",
    scopes = [edit],
    undo = "Cut",
    needs_project,
    example(title = "Cut the first scene's first element", params = r#"{"context": "staging", "objectIds": ["scene:#1/element:#1"]}"#),
)]
pub(crate) fn edit_cut(cx: &mut Cx, p: ObjectsParams) -> Result<ClipResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let picked = pick(cx, context, p.object_ids)?;
    report_picked(cx, context, &picked);
    let clip = copy_content(cx, context, &picked, None)?;
    delete(cx, &picked, false, true)?;
    let clipboard = store(cx, &clip)?;
    Ok(ClipResult { clipboard })
}

#[command(
    id = "edit.copy",
    kind = ui,
    label = "Copy",
    scopes = [ui],
    needs_project,
    example(title = "Copy two cells of a column", params = r#"{"context": "xsheet", "objectIds": ["scene:#1/element:\"Bob\"/cell:#1", "scene:#1/element:\"Bob\"/cell:#2"]}"#),
)]
pub(crate) fn edit_copy(cx: &mut Cx, p: ObjectsParams) -> Result<ClipResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let picked = pick(cx, context, p.object_ids)?;
    report_picked(cx, context, &picked);
    let clip = copy_content(cx, context, &picked, None)?;
    let clipboard = store(cx, &clip)?;
    Ok(ClipResult { clipboard })
}

/// `{deletedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// Deleted objects (cells as cell references).
    deleted_ids: Vec<String>,
}

#[command(
    id = "edit.delete",
    kind = action,
    label = "Delete",
    scopes = [edit],
    undo = "Delete",
    needs_project,
    example(title = "Clear the first cell of a column", params = r#"{"context": "xsheet", "objectIds": ["scene:#1/element:\"Bob\"/cell:#1"]}"#),
)]
pub(crate) fn edit_delete(cx: &mut Cx, p: ObjectsParams) -> Result<DeleteResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let picked = pick(cx, context, p.object_ids)?;
    report_picked(cx, context, &picked);
    let deleted_ids = delete(cx, &picked, false, false)?;
    Ok(DeleteResult { deleted_ids })
}

/// `edit.paste` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasteParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Where to paste: `{drawingId}`, `{elementId, frame}`, `{catalogId}`, `{functionId, frame}`, `{objectIds}` or
    /// `{sceneId}`.
    #[serde(default)]
    destination: Option<Destination>,
}

#[command(
    id = "edit.paste",
    kind = action,
    label = "Paste",
    scopes = [edit],
    undo = "Paste",
    needs_project,
    example(title = "Paste copied cells at frame 30 of a column", params = r#"{"context": "xsheet", "destination": {"elementId": "scene:#1/element:\"Bob\"", "frame": 30}}"#),
)]
pub(crate) fn edit_paste(cx: &mut Cx, p: PasteParams) -> Result<PasteResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let clip = clipboard(cx)?;
    let dest = p.destination.unwrap_or_default();
    paste(cx, context, &clip, &dest, Special::None)
}

fn yes() -> bool {
    true
}

/// `edit.cutSpecial` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CutSpecialParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Objects or cells (default: the selection).
    #[serde(default)]
    object_ids: Option<Vec<String>>,
    /// `TimelineEditOptions` (01 §12.4); omitted fields take the remembered values.
    #[serde(default)]
    options: Option<Map<String, Value>>,
    /// Remember the options.
    #[serde(default = "yes")]
    remember: bool,
}

#[command(
    id = "edit.cutSpecial",
    kind = action,
    label = "Cut Special",
    scopes = [edit],
    undo = "Cut Special",
    needs_project,
    example(title = "Cut cells and remove their frames", params = r#"{"context": "xsheet", "objectIds": ["scene:#1/element:\"Bob\"/cell:#1"], "options": {"createDeleteExposure": true}}"#),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_cut_special(cx: &mut Cx, p: CutSpecialParams) -> Result<ClipResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let opts: TimelineEditOptions = merged(cx, "cutSpecial", p.options.as_ref())?;
    let picked = pick(cx, context, p.object_ids)?;
    report_picked(cx, context, &picked);
    let clip = copy_content(cx, context, &picked, Some(&opts))?;
    let picked = match picked {
        // Only the keys the key-group options select are cut.
        Picked::Keys(keys) => {
            let doc = cx.document()?.clone();
            Picked::Keys(
                keys.into_iter()
                    .filter(|k| channel_of(&doc, k).is_ok_and(|(n, _, _)| opts.channel_enabled(&n)))
                    .collect(),
            )
        }
        other => other,
    };
    delete(cx, &picked, opts.create_delete_exposure, true)?;
    for name in opts.not_applied() {
        cx.warn("edit.optionNotApplied", format!("option {name} is not applied yet"), Vec::new());
    }
    if p.remember {
        cx.session_mut().edit_options.insert("cutSpecial".into(), obj(to_json(&opts)?));
    }
    let clipboard = store(cx, &clip)?;
    Ok(ClipResult { clipboard })
}

/// `edit.pasteSpecial` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PasteSpecialParams {
    /// Context; `auto` follows the focused panel.
    #[serde(default)]
    context: EditContext,
    /// Where to paste (as `edit.paste`).
    #[serde(default)]
    destination: Option<Destination>,
    /// `TimelineEditOptions` or `MotionPointPasteOptions` (01 §12.4), by clipboard content.
    #[serde(default)]
    options: Option<Map<String, Value>>,
    /// Paste special again: the last options of this session.
    #[serde(default)]
    use_last: bool,
    /// Remember the options.
    #[serde(default = "yes")]
    remember: bool,
}

#[command(
    id = "edit.pasteSpecial",
    kind = action,
    label = "Paste Special",
    scopes = [edit],
    undo = "Paste Special",
    needs_project,
    example(title = "Insert copied cells at frame 1 of a column", params = r#"{"context": "xsheet", "destination": {"elementId": "scene:#1/element:\"Bob\"", "frame": 1}, "options": {"createDeleteExposure": true}}"#),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_paste_special(cx: &mut Cx, p: PasteSpecialParams) -> Result<PasteResult, CommandError> {
    let context = resolve_context(cx, p.context);
    let clip = clipboard(cx)?;
    let dest = p.destination.unwrap_or_default();
    let motion = clip.kind == ClipboardKind::MotionPoint;
    let operation = if motion { "pasteSpecialMotionPoint" } else { "pasteSpecial" };
    if p.use_last && !cx.session().edit_options.contains_key(operation) {
        return Err(CommandError::with_details(
            "edit.noPreviousPasteSpecial",
            "paste special again needs a paste special earlier in the session",
            json!({}),
        ));
    }
    let given = if p.use_last { None } else { p.options.as_ref() };
    let (special, remembered) = if motion {
        let o: MotionPointPasteOptions = merged(cx, operation, given)?;
        (Special::MotionPoint(o), obj(to_json(&o)?))
    } else {
        let o: TimelineEditOptions = merged(cx, operation, given)?;
        (Special::Timeline(o), obj(to_json(&o)?))
    };
    let out = paste(cx, context, &clip, &dest, special)?;
    if p.remember || p.use_last {
        cx.session_mut().edit_options.insert(operation.into(), remembered);
    }
    Ok(out)
}

/// `edit.pasteAsNewElement` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PasteNewParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// First frame (default current).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
}

/// `{elementId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewElementResult {
    /// The new element.
    element_id: String,
}

#[command(
    id = "edit.pasteAsNewElement",
    kind = action,
    label = "Paste as New Element",
    scopes = [edit],
    undo = "Paste as New Element",
    needs_project,
    example(title = "A new element from the copied cells, starting at frame 1", params = r#"{"frame": 1}"#),
    gui = "Edit menu (M4)",
)]
pub(crate) fn edit_paste_as_new_element(cx: &mut Cx, p: PasteNewParams) -> Result<NewElementResult, CommandError> {
    let clip = clipboard(cx)?;
    let doc = cx.document()?.clone();
    let scene = current_scene(cx, p.scene_id.as_deref())?;
    let frame = p.frame.unwrap_or(cx.app().frame).max(1);
    cx.set_targets(json!({ "destination": { "sceneId": scene.as_str(), "frame": frame } }));
    let items = clip.items.clone().unwrap_or_default();
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let taken: BTreeSet<String> =
        sc.elements().ids().iter().filter_map(|e| doc.element(e)).map(|e| e.name().clone()).collect();
    let bank = cx.new_id(ObjectType::DrawingBank)?;
    let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
    let mut names = BTreeSet::new();
    let mut drawings: Vec<Value> = Vec::new();
    let (element_type, base_name, frames): (ElementType, String, Vec<Option<String>>) = match clip.kind {
        ClipboardKind::Cells => {
            let source = items
                .iter()
                .find_map(|it| it.get("elementId").and_then(Value::as_str))
                .and_then(|e| Id::parse(e).ok())
                .and_then(|e| doc.element(&e));
            let ty = source.map_or(ElementType::Drawing, |e| *e.element_type());
            let name = source.map_or_else(|| "Element".to_owned(), |e| e.name().clone());
            let width = items
                .iter()
                .filter_map(|it| it.get("frameOffset").and_then(Value::as_u64))
                .max()
                .map_or(0, |m| m as usize + 1);
            let mut placed: Vec<Option<String>> = vec![None; width];
            let mut mapped: BTreeMap<String, String> = BTreeMap::new();
            for it in &items {
                let offset = it.get("frameOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
                let Some(d) = it.get("drawingRef").and_then(Value::as_str) else { continue };
                let id = match mapped.get(d) {
                    Some(id) => id.clone(),
                    None => {
                        let mut v = doc.entry_raw_json(&EntryKey::Drawing(Id::parse(d)?)).map_err(|_| {
                            not_allowed(clip.kind, EditContext::Xsheet, "a copied drawing is not in this project")
                        })?;
                        let id = cx.new_id(ObjectType::Drawing)?;
                        let name = unique(v.get("name").and_then(Value::as_str).unwrap_or("1"), &names);
                        names.insert(name.clone());
                        v["id"] = json!(id.as_str());
                        v["rev"] = json!(0);
                        v["name"] = json!(name);
                        drawings.push(v);
                        mapped.insert(d.to_owned(), id.as_str().to_owned());
                        id.as_str().to_owned()
                    }
                };
                if let Some(slot) = placed.get_mut(offset) {
                    *slot = Some(id);
                }
            }
            (ty, name, placed)
        }
        ClipboardKind::DrawingObjects => {
            let id = cx.new_id(ObjectType::Drawing)?;
            let mut d = to_json(&Drawing::empty(id.clone(), "1".to_owned(), Timestamp::now()))?;
            let mut nodes: Vec<Value> = Vec::new();
            let mut strokes: Vec<Value> = Vec::new();
            for item in items {
                let mut v = Value::Object(item);
                regenerate(cx, &mut v, &[ObjectType::DrawingNode, ObjectType::Stroke], &mut Vec::new())?;
                if v.get("type").and_then(Value::as_str) == Some("stroke") {
                    strokes.push(v);
                } else {
                    nodes.push(v);
                }
            }
            if !strokes.is_empty() {
                let hint = if strokes.iter().all(|s| s.get("kind").and_then(Value::as_str) == Some("pencil")) {
                    LayerHint::Pencil
                } else {
                    LayerHint::Brush
                };
                let mut layer = to_json(&DrawingNode::layer(cx.new_id(ObjectType::DrawingNode)?, hint))?;
                layer["strokes"] = Value::Array(strokes);
                nodes.insert(0, layer);
            }
            d["nodes"] = Value::Array(nodes);
            drawings.push(d);
            (ElementType::Drawing, "Drawing".to_owned(), vec![Some(id.as_str().to_owned())])
        }
        other => {
            return Err(not_allowed(other, EditContext::Xsheet, "a new element is made from cells or drawing objects"));
        }
    };
    let element = cx.new_id(ObjectType::Element)?;
    let mut el = to_json(&Element::new(ElementInit {
        id: element.clone(),
        element_type,
        name: unique(&base_name, &taken),
        created_at: Timestamp::now(),
        drawing_bank_id: Some(bank.clone()),
        bar: (frame, u32::try_from(frames.len()).unwrap_or(1).max(1)),
        slot: None,
    }))?;
    let mut exposure: Vec<Option<String>> = vec![None; (frame as usize).saturating_sub(1)];
    exposure.extend(frames);
    el["exposure"] = runs_of(&exposure);
    let stubs: Vec<Value> = drawings.iter().map(|d| json!({ "id": d["id"], "type": "drawing" })).collect();
    let bank_json = to_json(&DrawingBank::new(bank.clone(), Vec::new()))?;
    let mut bank_json = bank_json;
    bank_json["drawings"] = Value::Array(stubs);
    changes.push((
        EntryKey::Project,
        vec![op("add", "/drawingBanks/-", Some(json!({ "id": bank.as_str(), "type": "drawingBank" })))],
    ));
    changes.push((EntryKey::Bank(bank.clone()), vec![op("add", "", Some(bank_json))]));
    for d in drawings {
        let id = Id::parse(d["id"].as_str().unwrap_or_default())?;
        changes.push((EntryKey::Drawing(id), vec![op("add", "", Some(d))]));
    }
    changes.push((EntryKey::Element(element.clone()), vec![op("add", "", Some(el))]));
    changes.push((
        EntryKey::Scene(scene),
        vec![
            op("add", "/elements/-", Some(json!({ "id": element.as_str(), "type": "element" }))),
            op("add", "/tree/-", Some(json!({ "id": element.as_str(), "children": [] }))),
            op("add", "/columnOrder/-", Some(json!(element.as_str()))),
        ],
    ));
    cx.patch_entries(changes)?;
    Ok(NewElementResult { element_id: element.as_str().to_owned() })
}
