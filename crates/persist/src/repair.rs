//! Validation and repair on load (10 §7.2, REP-01/REP-02, LOAD-03).
//!
//! Repairs work on the entries' JSON, in a fixed order and in entry-path order, so the same files always produce the
//! same document, IDs and repairs (LOAD-03). New IDs come from a generator seeded with a hash of the input. Each
//! object entry first goes through the model's input path (`object::from_json`); object-local problems that have a
//! defined repair (out-of-range values, invalid names, unreadable transitions) are repaired and the entry is parsed
//! again; an entry that still fails is handled as unreadable (palette, element: skipped, raw text kept under the
//! parent's `ext["app.unreadable"]`; drawing: kept empty and flagged; scene: kept empty with its stub's name; bank,
//! library: rebuilt; project: fatal). Then the document-level passes run: duplicate IDs, entry membership, names,
//! references, the scene trees, revisions. The result is checked by the model (`Document::from_parts`); nothing the
//! model reports as an error survives, and the executor checks the candidate again before installing it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{Map, Value, json};
use tt_model::document::{Document, DocumentParts, EntryKey, Severity, slots_of};
use tt_model::entities::drawing::{Drawing, DrawingBank};
use tt_model::entities::element::{Element, ElementType};
use tt_model::entities::library::Library;
use tt_model::entities::palette::Palette;
use tt_model::entities::project::Project;
use tt_model::entities::scene::Scene;
use tt_model::name::NameRule;
use tt_model::overlay::{self, Overlay};
use tt_model::{Id, IdGenerator, ModelError, ObjectType, object, schema};

use crate::codes;
use crate::layout::{self, Kind};
use crate::report::Issues;

/// An entry carrying values of a newer minor version that this version cannot represent (VER-02): the object is
/// read-only and the raw entry is written back unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenEntry {
    /// The entry exactly as read.
    pub raw: Value,
    /// Positions whose unknown value was replaced by a known one in memory: (pointer, original, substitute).
    pub substitutes: Vec<(String, Value, Value)>,
}

/// One object entry being loaded.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub path: String,
    pub ty: ObjectType,
    /// The scene (elements) or bank (drawings) the path names.
    pub owner: Option<String>,
    pub value: Value,
    pub overlay: Option<Overlay>,
    pub frozen: Option<FrozenEntry>,
    pub touched: bool,
}

impl Item {
    fn id(&self) -> String {
        self.value.get(self.ty.id_field()).and_then(Value::as_str).unwrap_or_default().to_owned()
    }
}

/// Everything being loaded.
#[derive(Debug)]
pub(crate) struct Work {
    pub project: Item,
    pub palettes: Vec<Item>,
    pub banks: Vec<Item>,
    pub drawings: Vec<Item>,
    pub scenes: Vec<Item>,
    pub elements: Vec<Item>,
    pub library: Option<Item>,
    pub issues: Issues,
    pub ids: IdGenerator,
    /// Timestamp for objects the repairs create (deterministic: the file's save time, else the epoch).
    pub stamp: String,
    pub newer_minor: bool,
}

fn parse_typed<T>(v: &Value) -> Result<Value, Vec<ModelError>>
where
    T: tt_model::entities::ModelObject + tt_model::validate::Check,
{
    let o: T = object::from_json(v)?;
    object::to_json(&o).map_err(|e| vec![e])
}

/// The model's input path for an entry object; returns the canonical JSON.
pub(crate) fn input(ty: ObjectType, v: &Value) -> Result<Value, Vec<ModelError>> {
    match ty {
        ObjectType::Project => parse_typed::<Project>(v),
        ObjectType::Palette => parse_typed::<Palette>(v),
        ObjectType::DrawingBank => parse_typed::<DrawingBank>(v),
        ObjectType::Drawing => parse_typed::<Drawing>(v),
        ObjectType::Scene => parse_typed::<Scene>(v),
        ObjectType::Element => parse_typed::<Element>(v),
        ObjectType::Library => parse_typed::<Library>(v),
        _ => Err(vec![ModelError::new(codes::INTERNAL, "not an entry type")]),
    }
}

fn pointer_get<'a>(v: &'a Value, p: &str) -> Option<&'a Value> {
    if p.is_empty() { Some(v) } else { v.pointer(p) }
}

fn pointer_mut<'a>(v: &'a mut Value, p: &str) -> Option<&'a mut Value> {
    if p.is_empty() { Some(v) } else { v.pointer_mut(p) }
}

fn parent_pointer(p: &str) -> &str {
    p.rsplit_once('/').map_or("", |(a, _)| a)
}

/// The value of a schema keyword at a keyword location of `jsonschema` (with `$ref` segments).
fn schema_keyword(ty: ObjectType, schema_path: &str) -> Option<Value> {
    let root = schema::object_schema(ty);
    // `jsonschema` reports a JSON pointer into the root schema (through `$defs`) …
    if let Some(v) = root.pointer(schema_path) {
        return Some(v.clone());
    }
    // … or a keyword location through `$ref` segments.
    let mut cur = &root;
    for raw in schema_path.strip_prefix('/')?.split('/') {
        let seg = raw.replace("~1", "/").replace("~0", "~");
        if seg == "$ref" {
            let r = cur.get("$ref")?.as_str()?;
            let name = r.strip_prefix("#/$defs/")?;
            cur = root.get("$defs")?.get(name)?;
            continue;
        }
        cur = match cur {
            Value::Object(m) => m.get(&seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur.clone())
}

/// A repaired name (10 §7.2 `doc.nameRepaired`): forbidden and control characters become `_`, a trailing `.` where
/// forbidden becomes `_`, the name is trimmed and truncated to the rule's length; empty names get `fallback`.
#[must_use]
pub fn repair_name(ty: ObjectType, raw: &str, fallback: &str) -> String {
    let rule = NameRule::for_type(ty);
    let n = tt_model::name::normalize(raw);
    let mut s: String = n
        .chars()
        .map(|c| if c.is_control() || rule.forbidden.contains(&c) || (rule.no_dot && c == '.') { '_' } else { c })
        .collect();
    s = s.chars().take(rule.max_len).collect::<String>().trim().to_owned();
    if rule.no_trailing_dot {
        while s.ends_with('.') {
            s.pop();
            s.push('_');
        }
    }
    if s.is_empty() { fallback.to_owned() } else { s }
}

/// A name made unique among `taken` by suffixing `-2`, `-3`, … (10 §7.2), within the rule's length.
#[must_use]
pub fn unique_name(ty: ObjectType, base: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(base) {
        return base.to_owned();
    }
    let max = NameRule::for_type(ty).max_len;
    for n in 2u32.. {
        let suffix = format!("-{n}");
        let room = max.saturating_sub(suffix.chars().count());
        let stem: String = base.chars().take(room).collect();
        let candidate = format!("{}{suffix}", stem.trim_end());
        if !taken.contains(&candidate) {
            return candidate;
        }
        if n > 1_000_000 {
            break;
        }
    }
    base.to_owned()
}

fn default_name(ty: ObjectType) -> &'static str {
    match ty {
        ObjectType::Project => "Untitled",
        ObjectType::Scene => "Scene-1",
        ObjectType::Element => "Element",
        ObjectType::Drawing => "1",
        ObjectType::Palette => "Palette",
        ObjectType::PaletteStyle => "Default",
        ObjectType::Pen => "Pen",
        ObjectType::Swatch => "Colour",
        _ => "Untitled",
    }
}

fn type_at(v: &Value, pointer: &str) -> Option<ObjectType> {
    let mut p = pointer;
    loop {
        if let Some(t) =
            pointer_get(v, p).and_then(|x| x.get("type")).and_then(Value::as_str).and_then(ObjectType::parse)
        {
            return Some(t);
        }
        if p.is_empty() {
            return None;
        }
        p = parent_pointer(p);
    }
}

/// Inserts `value` under `ext[ns][key]` of an object.
pub(crate) fn put_ext(object: &mut Value, ns: &str, key: &str, value: Value) {
    let Some(m) = object.as_object_mut() else { return };
    let ext = m.entry("ext").or_insert_with(|| json!({}));
    if !ext.is_object() {
        *ext = json!({});
    }
    if let Some(e) = ext.as_object_mut() {
        let slot = e.entry(ns).or_insert_with(|| json!({}));
        if !slot.is_object() {
            *slot = json!({});
        }
        if let Some(s) = slot.as_object_mut() {
            s.insert(key.to_owned(), value);
        }
    }
}

/// Replaces every string equal to `from` by `to` inside a value.
fn rename_refs(v: &mut Value, from: &str, to: &str) {
    match v {
        Value::String(s) if s == from => *s = to.to_owned(),
        Value::Array(a) => a.iter_mut().for_each(|x| rename_refs(x, from, to)),
        Value::Object(m) => {
            // Opaque values are the user's (10 §2.2): never rewritten.
            for (k, x) in m.iter_mut() {
                if k != "meta" && k != "ext" {
                    rename_refs(x, from, to);
                }
            }
            // Map keys that are IDs (paletteStyles, swatch values).
            if m.contains_key(from) {
                let entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
                for (k, x) in entries {
                    m.insert(if k == from { to.to_owned() } else { k }, x);
                }
            }
        }
        _ => {}
    }
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn arr_mut<'a>(v: &'a mut Value, k: &str) -> Option<&'a mut Vec<Value>> {
    v.get_mut(k).and_then(Value::as_array_mut)
}

fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

fn element_type(v: &Value) -> Option<ElementType> {
    v.get("elementType").and_then(|t| serde_json::from_value(t.clone()).ok())
}

impl Work {
    fn new_id(&mut self, ty: ObjectType) -> String {
        self.ids.next(ty).map(|i| i.as_str().to_owned()).unwrap_or_else(|_| format!("{}_repaired", ty.as_str()))
    }

    /// Runs the input path with object-local repairs. `Ok` holds the canonical JSON.
    fn input_with_repairs(&mut self, item: &mut Item) -> Result<(), Vec<ModelError>> {
        if self.newer_minor && item.overlay.is_none() {
            let (clean, ov) = overlay::split(item.ty, &item.value);
            if !ov.is_empty() {
                item.overlay = Some(ov);
            }
            item.value = clean;
        }
        let mut last = Vec::new();
        for _ in 0..6 {
            match input(item.ty, &item.value) {
                Ok(v) => {
                    item.value = v;
                    return Ok(());
                }
                Err(errors) => {
                    if !self.repair_object(item, &errors) {
                        last = errors;
                        break;
                    }
                    last = errors;
                }
            }
        }
        if self.newer_minor && self.substitute_unknown_values(item, &last) {
            return Ok(());
        }
        Err(last)
    }

    /// VER-02: an entry of a newer minor version whose only problems are unknown enum values is loaded read-only,
    /// with known values in memory, and written back as read.
    fn substitute_unknown_values(&mut self, item: &mut Item, errors: &[ModelError]) -> bool {
        // An unknown enum value: the violated keyword is `enum`, `const`, or a `oneOf` of constants.
        let ty = item.ty;
        let unknown = |e: &ModelError| {
            let Some(sp) = e.details().get("schemaPath").and_then(Value::as_str) else { return false };
            e.code() == codes::SCHEMA_VIOLATION
                && (sp.ends_with("/enum")
                    || sp.ends_with("/const")
                    || (sp.ends_with("/oneOf")
                        && schema_keyword(ty, sp).and_then(|v| v.as_array().cloned()).is_some_and(|alts| {
                            !alts.is_empty() && alts.iter().all(|a| a.get("const").is_some() || a.get("enum").is_some())
                        })))
        };
        if errors.is_empty() || !errors.iter().all(unknown) {
            return false;
        }
        // The entry as read: the unknown properties split off for the overlay go back in (C13-R1-05), so the frozen
        // entry is written back with everything the newer version put in it.
        let raw = match &item.overlay {
            Some(o) => overlay::merge(item.value.clone(), o),
            None => item.value.clone(),
        };
        let mut v = item.value.clone();
        let mut subs = Vec::new();
        for e in errors {
            let pointer = e.details().get("pointer").and_then(Value::as_str).unwrap_or_default().to_owned();
            let sp = e.details().get("schemaPath").and_then(Value::as_str).unwrap_or_default();
            let known = match schema_keyword(item.ty, sp) {
                Some(Value::Array(a)) => a.first().map(|x| {
                    x.get("const")
                        .cloned()
                        .or_else(|| x.get("enum").and_then(|e| e.get(0)).cloned())
                        .unwrap_or_else(|| x.clone())
                }),
                Some(c) => Some(c),
                None => None,
            };
            let (Some(known), Some(slot)) = (known, pointer_mut(&mut v, &pointer)) else { return false };
            subs.push((pointer, slot.clone(), known.clone()));
            *slot = known;
        }
        match input(item.ty, &v) {
            Ok(parsed) => {
                let id = item.id();
                self.issues.info(
                    codes::UNKNOWN_VALUE,
                    Some(&id),
                    Some(&item.path),
                    format!("`{}` uses values of a newer format version; it is read-only", item.path),
                    json!({ "substitutes": subs.iter().map(|(p, o, s)| json!({"pointer": p, "value": o, "shownAs": s})).collect::<Vec<_>>() }),
                );
                item.frozen = Some(FrozenEntry { raw, substitutes: subs });
                item.value = parsed;
                true
            }
            Err(_) => false,
        }
    }

    /// Repairs object-local problems that have a defined repair; returns whether anything changed.
    fn repair_object(&mut self, item: &mut Item, errors: &[ModelError]) -> bool {
        let mut changed = false;
        let id = item.id();
        // A scene's tree, column order and element list: repeated IDs are kept at their first occurrence (the
        // document-level pass repairs the rest, 10 §7.2 `tree.invalid`).
        if item.ty == ObjectType::Scene
            && errors.iter().any(|e| {
                let p = e.details().get("pointer").and_then(Value::as_str).unwrap_or_default();
                p.starts_with("/tree") || p.starts_with("/columnOrder") || p.starts_with("/elements")
            })
        {
            let removed = sanitize_scene(&mut item.value);
            if !removed.is_empty() {
                self.issues.repaired_error(
                    codes::TREE_INVALID,
                    Some(&id),
                    Some(&item.path),
                    "the scene repeats elements in its tree, column order or element list; the first occurrence is kept",
                    json!({ "removed": removed }),
                );
                item.touched = true;
                return true;
            }
        }
        for e in errors {
            let pointer = e.details().get("pointer").and_then(Value::as_str).unwrap_or_default().to_owned();
            let sp = e.details().get("schemaPath").and_then(Value::as_str).unwrap_or_default().to_owned();
            let rule = e.details().get("rule").and_then(Value::as_str).unwrap_or_default().to_owned();
            // Transitions: an unreadable transition loads as `none` (10 §4.4).
            for t in ["/startTransition", "/outTransition"] {
                if pointer.starts_with(t) && item.value.get(&t[1..]).is_some() {
                    let tid = item.value.pointer(&format!("{t}/id")).and_then(Value::as_str).map(str::to_owned);
                    let tid =
                        tid.filter(|s| Id::parse(s).is_ok()).unwrap_or_else(|| self.new_id(ObjectType::Transition));
                    let old = item.value.get(&t[1..]).cloned().unwrap_or(Value::Null);
                    if let Some(slot) = item.value.get_mut(&t[1..]) {
                        *slot = transition(&tid);
                        self.issues.repaired_warning(
                            codes::TRANSITION_INVALID,
                            Some(&id),
                            Some(&item.path),
                            "an unreadable transition was loaded as `none`",
                            json!({ "pointer": t, "old": old }),
                        );
                        changed = true;
                    }
                }
            }
            if changed {
                continue;
            }
            let keyword = sp.rsplit('/').next().unwrap_or_default();
            let is_name = pointer.ends_with("/name") || pointer == "/name";
            if is_name && (rule == "name" || matches!(keyword, "maxLength" | "minLength" | "pattern")) {
                changed |= self.fix_name_at(item, &pointer);
                continue;
            }
            if rule == "unique" {
                // `<list>/<i>`: a duplicate name in an object's own list (pens, styles).
                let name_ptr = format!("{pointer}/name");
                if pointer_get(&item.value, &name_ptr).is_some() {
                    changed |= self.dedupe_name_in_list(item, &pointer);
                    continue;
                }
            }
            if matches!(keyword, "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum") {
                let Some(bound) = schema_keyword(item.ty, &sp).and_then(|b| b.as_f64()) else { continue };
                let Some(slot) = pointer_mut(&mut item.value, &pointer) else { continue };
                let Some(old) = slot.as_f64() else { continue };
                let integer = slot.is_i64() || slot.is_u64();
                let new = match keyword {
                    "minimum" => bound,
                    "maximum" => bound,
                    "exclusiveMinimum" if integer => bound.floor() + 1.0,
                    "exclusiveMaximum" if integer => bound.ceil() - 1.0,
                    "exclusiveMinimum" => bound.next_up(),
                    _ => bound.next_down(),
                };
                let newv = if integer && new.fract() == 0.0 && new.abs() < 9.0e15 {
                    // Exact for |new| < 2^53.
                    #[allow(clippy::cast_possible_truncation, reason = "integral and below 2^53")]
                    let n = new as i64;
                    json!(n)
                } else {
                    json!(new)
                };
                if *slot != newv {
                    *slot = newv.clone();
                    self.issues.repaired_warning(
                        codes::VALUE_CLAMPED,
                        Some(&id),
                        Some(&item.path),
                        format!("value at `{pointer}` was out of range and was clamped"),
                        json!({ "pointer": pointer, "old": old, "new": newv }),
                    );
                    changed = true;
                }
            }
        }
        if changed {
            item.touched = true;
        }
        changed
    }

    fn fix_name_at(&mut self, item: &mut Item, pointer: &str) -> bool {
        let ty = type_at(&item.value, parent_pointer(pointer)).unwrap_or(item.ty);
        let id = item.id();
        let Some(slot) = pointer_mut(&mut item.value, pointer) else { return false };
        let old = slot.as_str().unwrap_or_default().to_owned();
        let new = repair_name(ty, &old, default_name(ty));
        if new == old {
            return false;
        }
        *slot = json!(new);
        self.issues.repaired_warning(
            codes::NAME_REPAIRED,
            Some(&id),
            Some(&item.path),
            format!("the name `{old}` was invalid and became `{new}`"),
            json!({ "pointer": pointer, "old": old, "new": new }),
        );
        true
    }

    fn dedupe_name_in_list(&mut self, item: &mut Item, pointer: &str) -> bool {
        let list_ptr = parent_pointer(pointer).to_owned();
        let Some(index) = pointer.rsplit('/').next().and_then(|s| s.parse::<usize>().ok()) else { return false };
        let Some(list) = pointer_get(&item.value, &list_ptr).and_then(Value::as_array) else { return false };
        let taken: BTreeSet<String> = list
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .filter_map(|(_, x)| str_of(x, "name").map(str::to_owned))
            .collect();
        let ty = list.get(index).and_then(|x| str_of(x, "type")).and_then(ObjectType::parse).unwrap_or(item.ty);
        let id = item.id();
        let name_ptr = format!("{pointer}/name");
        let Some(slot) = pointer_mut(&mut item.value, &name_ptr) else { return false };
        let old = slot.as_str().unwrap_or_default().to_owned();
        let new = unique_name(ty, &old, &taken);
        if new == old {
            return false;
        }
        *slot = json!(new);
        self.issues.repaired_warning(
            codes::NAME_REPAIRED,
            Some(&id),
            Some(&item.path),
            format!("the duplicate name `{old}` became `{new}`"),
            json!({ "pointer": name_ptr, "old": old, "new": new }),
        );
        true
    }

    // ----- unreadable entries ---------------------------------------------------------------------------------

    /// Keeps an unreadable entry's raw content under `ext["app.unreadable"][path]` of `parent`.
    fn preserve(parent: &mut Value, path: &str, raw: &Value) {
        put_ext(parent, "app.unreadable", path, raw.clone());
    }

    fn scene_of_element(&self, element: &str, path_scene: Option<&str>) -> Option<usize> {
        self.scenes
            .iter()
            .position(|s| arr(&s.value, "elements").iter().any(|x| str_of(x, "id") == Some(element)))
            .or_else(|| path_scene.and_then(|p| self.scenes.iter().position(|s| s.id() == p)))
    }

    /// Handles an entry that fails its schema after the repairs (10 §7.2): palettes and elements are skipped and kept
    /// in the parent's `ext`; drawings stay as empty, flagged drawings; scenes stay as empty scenes; banks and the
    /// library are rebuilt.
    fn unreadable(&mut self, item: Item, raw: Value, errors: &[ModelError]) {
        let messages: Vec<Value> = errors
            .iter()
            .take(20)
            .map(|e| json!({ "code": e.code(), "message": e.message(), "details": e.details() }))
            .collect();
        let raw_id = raw.get(item.ty.id_field()).and_then(Value::as_str).map(str::to_owned);
        match item.ty {
            ObjectType::Palette => {
                Self::preserve(&mut self.project.value, &item.path, &raw);
                self.project.touched = true;
                self.issues.repaired_error(
                    codes::SCHEMA_VIOLATION,
                    raw_id.as_deref(),
                    Some(&item.path),
                    format!(
                        "palette entry `{}` is unreadable; it was skipped and kept in the project's ext",
                        item.path
                    ),
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
            ObjectType::Element => {
                let at = match raw_id.as_deref() {
                    Some(id) => self.scene_of_element(id, item.owner.as_deref()),
                    None => item.owner.as_deref().and_then(|o| self.scenes.iter().position(|s| s.id() == o)),
                };
                match at {
                    Some(i) => {
                        Self::preserve(&mut self.scenes[i].value, &item.path, &raw);
                        self.scenes[i].touched = true;
                    }
                    None => {
                        Self::preserve(&mut self.project.value, &item.path, &raw);
                        self.project.touched = true;
                    }
                }
                self.issues.repaired_error(
                    codes::SCHEMA_VIOLATION,
                    raw_id.as_deref(),
                    Some(&item.path),
                    format!("element entry `{}` is unreadable; it was skipped and kept in its scene's ext", item.path),
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
            ObjectType::Drawing => {
                let id = raw_id.unwrap_or_else(|| self.new_id(ObjectType::Drawing));
                let name = raw.get("name").and_then(Value::as_str).unwrap_or("1").to_owned();
                let mut d = self.empty_drawing(&id, &name);
                put_ext(&mut d, "app.unreadable", &item.path, raw);
                self.drawings.push(Item { value: d, touched: true, overlay: None, frozen: None, ..item.clone() });
                self.issues.repaired_error(
                    codes::CONTENT_MISSING,
                    Some(&id),
                    Some(&item.path),
                    format!("drawing entry `{}` is unreadable; the drawing is kept empty", item.path),
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
            ObjectType::Scene => {
                let id = raw_id.unwrap_or_else(|| self.new_id(ObjectType::Scene));
                let name = arr(&self.project.value, "scenes")
                    .iter()
                    .find(|s| str_of(s, "id") == Some(&id))
                    .and_then(|s| str_of(s, "name"))
                    .or_else(|| raw.get("name").and_then(Value::as_str))
                    .unwrap_or("Scene-1")
                    .to_owned();
                let mut s = self.empty_scene(&id, &name);
                put_ext(&mut s, "app.unreadable", &item.path, raw);
                self.scenes.push(Item { value: s, touched: true, overlay: None, frozen: None, ..item.clone() });
                self.issues.repaired_error(
                    codes::SCHEMA_VIOLATION,
                    Some(&id),
                    Some(&item.path),
                    format!("scene entry `{}` is unreadable; the scene is kept empty", item.path),
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
            ObjectType::DrawingBank => {
                let id = raw_id.or_else(|| item.path.split('/').nth(1).map(str::to_owned)).unwrap_or_default();
                let mut b = json!({ "id": id, "type": "drawingBank", "rev": 1, "drawings": [] });
                put_ext(&mut b, "app.unreadable", &item.path, raw);
                self.banks.push(Item { value: b, touched: true, overlay: None, frozen: None, ..item.clone() });
                self.issues.repaired_error(
                    codes::SCHEMA_VIOLATION,
                    Some(&id),
                    Some(&item.path),
                    format!("bank entry `{}` is unreadable; it is rebuilt from its drawings", item.path),
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
            _ => {
                // The library: rebuilt empty, the raw entry kept in the project's ext.
                Self::preserve(&mut self.project.value, &item.path, &raw);
                self.project.touched = true;
                self.library = None;
                self.issues.repaired_error(
                    codes::SCHEMA_VIOLATION,
                    raw_id.as_deref(),
                    Some(&item.path),
                    "the local library index is unreadable; an empty library is used",
                    json!({ "errors": messages, "keptAt": "/ext/app.unreadable" }),
                );
            }
        }
    }

    fn empty_drawing(&self, id: &str, name: &str) -> Value {
        json!({
            "id": id, "type": "drawing", "rev": 1, "name": name, "locked": false, "pivot": { "x": 0, "y": 0 },
            "mouthShape": null, "nodes": [], "colourRecovery": [], "createdAt": self.stamp
        })
    }

    fn empty_scene(&mut self, id: &str, name: &str) -> Value {
        let tid = self.new_id(ObjectType::Transition);
        json!({
            "id": id, "type": "scene", "rev": 1, "name": name, "visible": true, "backgroundColour": "#FFFFFFFF",
            "currentCameraId": null, "paletteStyles": {}, "outTransition": transition(&tid),
            "tree": [], "columnOrder": [], "elements": []
        })
    }

    // ----- the passes ------------------------------------------------------------------------------------------

    /// Validates every entry (object-local repairs; unreadable entries handled).
    pub(crate) fn validate_entries(&mut self) -> Result<(), ModelError> {
        let mut project = std::mem::replace(&mut self.project, placeholder(ObjectType::Project));
        let result = self.input_with_repairs(&mut project);
        self.project = project;
        if let Err(errors) = result {
            return Err(ModelError::with_details(
                codes::INVALID_FORMAT,
                format!("project.json is invalid: {}", errors.first().map(ModelError::message).unwrap_or_default()),
                json!({
                    "reason": "project",
                    "entry": layout::PROJECT,
                    "errors": errors.iter().take(20).map(|e| json!({"code": e.code(), "message": e.message(), "details": e.details()})).collect::<Vec<_>>(),
                }),
            ));
        }
        // Scenes first (elements preserve unreadable content into their scene).
        for kind in
            [ObjectType::Scene, ObjectType::Palette, ObjectType::DrawingBank, ObjectType::Drawing, ObjectType::Element]
        {
            let items = std::mem::take(self.list_mut(kind));
            for mut item in items {
                // An entry that is not JSON arrives as `{type: "unreadable", raw}`: its raw text is what is kept.
                let raw = if item.value.get("type").and_then(Value::as_str) == Some("unreadable") {
                    item.value.get("raw").cloned().unwrap_or(Value::Null)
                } else {
                    item.value.clone()
                };
                let expected = item.value.get("type").and_then(Value::as_str) == Some(kind.as_str());
                let result = if expected {
                    self.input_with_repairs(&mut item)
                } else {
                    Err(vec![ModelError::schema("/type", format!("entry is not a {}", kind.as_str()))])
                };
                match result {
                    Ok(()) => self.list_mut(kind).push(item),
                    Err(errors) => self.unreadable(item, raw, &errors),
                }
            }
        }
        if let Some(mut lib) = self.library.take() {
            let raw = if lib.value.get("type").and_then(Value::as_str) == Some("unreadable") {
                lib.value.get("raw").cloned().unwrap_or(Value::Null)
            } else {
                lib.value.clone()
            };
            match self.input_with_repairs(&mut lib) {
                Ok(()) => self.library = Some(lib),
                Err(errors) => self.unreadable(lib, raw, &errors),
            }
        }
        Ok(())
    }

    fn list_mut(&mut self, ty: ObjectType) -> &mut Vec<Item> {
        match ty {
            ObjectType::Palette => &mut self.palettes,
            ObjectType::DrawingBank => &mut self.banks,
            ObjectType::Drawing => &mut self.drawings,
            ObjectType::Scene => &mut self.scenes,
            _ => &mut self.elements,
        }
    }

    /// Entry objects given twice (same ID in two entries): the later one in entry-path order gets a new ID, and the
    /// references inside its own entry are rewritten (10 §7.2 `id.duplicate`).
    pub(crate) fn entry_duplicates(&mut self) {
        for ty in
            [ObjectType::Palette, ObjectType::DrawingBank, ObjectType::Drawing, ObjectType::Scene, ObjectType::Element]
        {
            let mut items = std::mem::take(self.list_mut(ty));
            items.sort_by(|a, b| a.path.cmp(&b.path));
            let mut seen = BTreeSet::new();
            for item in &mut items {
                let id = item.id();
                if seen.insert(id.clone()) {
                    continue;
                }
                let new = self.new_id(ty);
                rename_refs(&mut item.value, &id, &new);
                item.touched = true;
                seen.insert(new.clone());
                self.issues.repaired_error(
                    codes::ID_DUPLICATE,
                    Some(&new),
                    Some(&item.path),
                    format!("{} `{id}` is given by more than one entry; the later one became `{new}`", ty.as_str()),
                    json!({ "oldId": id, "newId": new }),
                );
            }
            *self.list_mut(ty) = items;
        }
    }

    /// Entry membership: listed children exist, every entry is listed exactly once (10 §7.2 `ref.dangling`).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn membership(&mut self) {
        // Banks rebuilt from drawings found under their folder (a bank entry missing or unreadable).
        let bank_ids: BTreeSet<String> = self.banks.iter().map(Item::id).collect();
        let mut orphan_banks: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for d in &self.drawings {
            if let Some(b) = &d.owner
                && !bank_ids.contains(b)
            {
                orphan_banks
                    .entry(b.clone())
                    .or_default()
                    .push((d.id(), str_of(&d.value, "name").unwrap_or("1").to_owned()));
            }
        }
        for (b, drawings) in orphan_banks {
            if Id::parse(&b).is_err() {
                continue;
            }
            let stubs: Vec<Value> = drawings.iter().map(|(id, _)| json!({ "id": id, "type": "drawing" })).collect();
            self.banks.push(Item {
                path: format!("banks/{b}/bank.json"),
                ty: ObjectType::DrawingBank,
                owner: None,
                value: json!({ "id": b, "type": "drawingBank", "rev": 1, "drawings": stubs }),
                overlay: None,
                frozen: None,
                touched: true,
            });
            self.issues.repaired_error(
                codes::REF_DANGLING,
                Some(&b),
                Some(&format!("banks/{b}/bank.json")),
                format!("drawing bank `{b}` had no entry; it was rebuilt from its drawings"),
                json!({ "drawings": drawings.iter().map(|(i, _)| i).collect::<Vec<_>>() }),
            );
        }
        // Bank ↔ drawings. Each drawing has one owner bank: the bank its entry lives in when that bank exists, else the
        // first bank listing it; other listings are removed. Listed drawings without an entry are kept empty and
        // flagged (10 §7.2 `drawing.contentMissing`); drawing entries no bank owns are removed and kept in the
        // project's ext (REP-02).
        let bank_ids: Vec<String> = self.banks.iter().map(Item::id).collect();
        let drawing_ids: BTreeSet<String> = self.drawings.iter().map(Item::id).collect();
        let mut owner_of: BTreeMap<String, String> = BTreeMap::new();
        for d in &self.drawings {
            if let Some(b) = d.owner.as_ref().filter(|b| bank_ids.contains(b)) {
                owner_of.insert(d.id(), b.clone());
            }
        }
        let mut missing: Vec<(String, String, String)> = Vec::new();
        for b in &self.banks {
            for stub in arr(&b.value, "drawings") {
                let Some(d) = str_of(stub, "id") else { continue };
                if owner_of.contains_key(d) {
                    continue;
                }
                owner_of.insert(d.to_owned(), b.id());
                if !drawing_ids.contains(d) {
                    missing.push((d.to_owned(), b.id(), str_of(stub, "name").unwrap_or("1").to_owned()));
                }
            }
        }
        for (d, bank, name) in missing {
            let path = format!("banks/{bank}/{d}.json");
            let mut v = self.empty_drawing(&d, &name);
            put_ext(&mut v, "app.contentMissing", "entry", json!(path));
            self.drawings.push(Item {
                path: path.clone(),
                ty: ObjectType::Drawing,
                owner: Some(bank.clone()),
                value: v,
                overlay: None,
                frozen: None,
                touched: true,
            });
            self.issues.repaired_error(
                codes::CONTENT_MISSING,
                Some(&d),
                Some(&path),
                format!("drawing `{name}` has no entry; it is kept empty"),
                json!({ "bankId": bank }),
            );
        }
        let orphans: Vec<usize> =
            (0..self.drawings.len()).filter(|i| !owner_of.contains_key(&self.drawings[*i].id())).collect();
        for i in orphans.into_iter().rev() {
            let item = self.drawings.remove(i);
            let id = item.id();
            self.project_keep(&item.path, item.value);
            self.issues.repaired_error(
                codes::REF_DANGLING,
                Some(&id),
                Some(&item.path),
                format!("drawing `{id}` belongs to no bank; it was removed and kept in the project's ext"),
                json!({ "keptAt": "/ext/app.unreadable" }),
            );
        }
        for bi in 0..self.banks.len() {
            let bank = self.banks[bi].id();
            let path = self.banks[bi].path.clone();
            let listed: Vec<String> = arr(&self.banks[bi].value, "drawings")
                .iter()
                .filter_map(|x| str_of(x, "id").map(str::to_owned))
                .collect();
            let mut keep: Vec<String> = Vec::new();
            for d in &listed {
                if owner_of.get(d) == Some(&bank) && !keep.contains(d) {
                    keep.push(d.clone());
                } else {
                    self.issues.repaired_error(
                        codes::ID_DUPLICATE,
                        Some(d),
                        Some(&path),
                        format!("drawing `{d}` is listed by more than one bank; it stays in the bank that stores it"),
                        json!({ "bankId": bank }),
                    );
                }
            }
            let owned: Vec<String> = self.drawings.iter().map(Item::id).collect();
            for d in owned {
                if owner_of.get(&d) == Some(&bank) && !keep.contains(&d) {
                    self.issues.repaired_error(
                        codes::REF_DANGLING,
                        Some(&d),
                        Some(&path),
                        format!("drawing `{d}` was listed by no bank; it was added to the bank that stores it"),
                        json!({}),
                    );
                    keep.push(d);
                }
            }
            if keep != listed {
                if let Some(list) = arr_mut(&mut self.banks[bi].value, "drawings") {
                    *list = keep.iter().map(|d| json!({ "id": d, "type": "drawing" })).collect();
                }
                self.banks[bi].touched = true;
            }
        }
        // Scene ↔ elements.
        let element_ids: BTreeSet<String> = self.elements.iter().map(Item::id).collect();
        let mut owner: HashMap<String, String> = HashMap::new();
        for si in 0..self.scenes.len() {
            let scene = self.scenes[si].id();
            let path = self.scenes[si].path.clone();
            let mut keep = Vec::new();
            for stub in arr(&self.scenes[si].value, "elements").to_vec() {
                let Some(e) = str_of(&stub, "id").map(str::to_owned) else { continue };
                if !element_ids.contains(&e) {
                    self.scenes[si].touched = true;
                    self.issues.repaired_error(
                        codes::REF_DANGLING,
                        Some(&scene),
                        Some(&path),
                        format!("scene lists element `{e}`, which does not exist; the reference was removed"),
                        json!({ "removed": stub }),
                    );
                    continue;
                }
                if let Some(first) = owner.get(&e) {
                    self.scenes[si].touched = true;
                    self.issues.repaired_error(
                        codes::ID_DUPLICATE,
                        Some(&e),
                        Some(&path),
                        format!("element `{e}` is listed by scenes `{first}` and `{scene}`; the first is kept"),
                        json!({}),
                    );
                    continue;
                }
                owner.insert(e.clone(), scene.clone());
                keep.push(json!({ "id": e, "type": "element" }));
            }
            if let Some(list) = arr_mut(&mut self.scenes[si].value, "elements") {
                *list = keep;
            }
        }
        let unlisted: Vec<(String, Option<String>, String)> = self
            .elements
            .iter()
            .map(|e| (e.id(), e.owner.clone(), e.path.clone()))
            .filter(|(id, _, _)| !owner.contains_key(id))
            .collect();
        for (e, path_scene, path) in unlisted {
            match path_scene.and_then(|s| self.scenes.iter().position(|x| x.id() == s)) {
                Some(si) => {
                    if let Some(list) = arr_mut(&mut self.scenes[si].value, "elements") {
                        list.push(json!({ "id": e, "type": "element" }));
                    }
                    self.scenes[si].touched = true;
                    self.issues.repaired_error(
                        codes::REF_DANGLING,
                        Some(&e),
                        Some(&path),
                        format!("element `{e}` was listed by no scene; it was added to the scene that stores it"),
                        json!({}),
                    );
                }
                None => {
                    let removed = self.elements.iter().find(|x| x.id() == e).map(|x| x.value.clone());
                    self.elements.retain(|x| x.id() != e);
                    self.project_keep(&path, removed.unwrap_or(Value::Null));
                    self.issues.repaired_error(
                        codes::REF_DANGLING,
                        Some(&e),
                        Some(&path),
                        format!("element `{e}` belongs to no scene; it was removed and kept in the project's ext"),
                        json!({ "keptAt": "/ext/app.unreadable" }),
                    );
                }
            }
        }
        // Project ↔ palettes, banks, scenes, library.
        self.project_list("palettes", ObjectType::Palette);
        self.project_list("drawingBanks", ObjectType::DrawingBank);
        self.project_list("scenes", ObjectType::Scene);
        let lib_id = self.library.as_ref().map(Item::id);
        let stub_id = self.project.value.pointer("/library/id").and_then(Value::as_str).map(str::to_owned);
        match (lib_id, stub_id) {
            (Some(l), Some(s)) if l == s => {}
            (Some(l), _) => {
                if let Some(m) = self.project.value.as_object_mut() {
                    m.insert("library".into(), json!({ "id": l, "type": "library" }));
                }
                self.project.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&l),
                    Some(layout::PROJECT),
                    "the project's library reference did not match the library; it was corrected",
                    json!({}),
                );
            }
            (None, s) => {
                let id = s.filter(|x| Id::parse(x).is_ok()).unwrap_or_else(|| self.new_id(ObjectType::Library));
                self.library = Some(Item {
                    path: layout::LIBRARY.to_owned(),
                    ty: ObjectType::Library,
                    owner: None,
                    value: json!({ "id": id, "type": "library", "rev": 1, "name": "Local", "kind": "local",
                                   "formatVersion": "1.0", "children": [] }),
                    overlay: None,
                    frozen: None,
                    touched: true,
                });
                if let Some(m) = self.project.value.as_object_mut() {
                    m.insert("library".into(), json!({ "id": id, "type": "library" }));
                }
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&id),
                    Some(layout::LIBRARY),
                    "the local library index is missing; an empty library is used",
                    json!({}),
                );
            }
        }
        if let Some(lib) = self.library.as_mut()
            && str_of(&lib.value, "kind") != Some("local")
        {
            let old = lib.value.get("kind").cloned();
            if let Some(m) = lib.value.as_object_mut() {
                m.insert("kind".into(), json!("local"));
            }
            lib.touched = true;
            let id = lib.id();
            self.issues.repaired_warning(
                codes::VALUE_CLAMPED,
                Some(&id),
                Some(layout::LIBRARY),
                "the project's library has kind `local`",
                json!({ "pointer": "/kind", "old": old, "new": "local" }),
            );
        }
    }

    fn project_keep(&mut self, path: &str, raw: Value) {
        Self::preserve(&mut self.project.value, path, &raw);
        self.project.touched = true;
    }

    fn project_list(&mut self, field: &str, ty: ObjectType) {
        let present: Vec<String> = match ty {
            ObjectType::Palette => self.palettes.iter().map(Item::id).collect(),
            ObjectType::DrawingBank => self.banks.iter().map(Item::id).collect(),
            _ => self.scenes.iter().map(Item::id).collect(),
        };
        let present_set: BTreeSet<&String> = present.iter().collect();
        let mut keep = Vec::new();
        let mut seen = BTreeSet::new();
        for stub in arr(&self.project.value, field).to_vec() {
            let Some(id) = str_of(&stub, "id").map(str::to_owned) else { continue };
            if !present_set.contains(&id) || !seen.insert(id.clone()) {
                self.project.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&id),
                    Some(layout::PROJECT),
                    format!(
                        "the project lists {} `{id}`, which does not exist (or twice); the reference was removed",
                        ty.as_str()
                    ),
                    json!({ "removed": stub, "list": field }),
                );
                continue;
            }
            keep.push(json!({ "id": id, "type": ty.as_str() }));
        }
        for id in present {
            if !seen.contains(&id) {
                seen.insert(id.clone());
                keep.push(json!({ "id": id, "type": ty.as_str() }));
                self.project.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&id),
                    Some(layout::PROJECT),
                    format!("{} `{id}` was not listed by the project; it was added", ty.as_str()),
                    json!({ "list": field }),
                );
            }
        }
        if let Some(m) = self.project.value.as_object_mut() {
            m.insert(field.into(), Value::Array(keep));
        }
    }

    /// Inline objects sharing an ID (10 §7.2 `id.duplicate`): within one entry, the later object gets a new ID; across
    /// entries, the object in the later entry gets a new ID and the references inside its own entry are rewritten.
    /// Container-local IDs (strokes, nodes, keys, motion points; 10 ID-03) are compared within their container.
    pub(crate) fn inline_duplicates(&mut self) {
        let mut seen: HashMap<(String, String), String> = HashMap::new();
        // Entry IDs are already unique per kind; register them first.
        let mut order: Vec<(u8, usize)> = vec![(0, 0)];
        for (k, list) in
            [(1u8, &self.palettes), (2, &self.banks), (3, &self.drawings), (4, &self.scenes), (5, &self.elements)]
        {
            let mut idx: Vec<usize> = (0..list.len()).collect();
            idx.sort_by(|a, b| list[*a].path.cmp(&list[*b].path));
            order.extend(idx.into_iter().map(|i| (k, i)));
        }
        if self.library.is_some() {
            order.push((6, 0));
        }
        for (k, i) in order {
            let item = match k {
                0 => &mut self.project,
                1 => &mut self.palettes[i],
                2 => &mut self.banks[i],
                3 => &mut self.drawings[i],
                4 => &mut self.scenes[i],
                5 => &mut self.elements[i],
                _ => match self.library.as_mut() {
                    Some(l) => l,
                    None => continue,
                },
            };
            let path = item.path.clone();
            let mut dups: Vec<(String, String, ObjectType, bool)> = Vec::new();
            collect_ids(&item.value, item.ty, None, &path, &mut seen, &mut dups, true);
            for (old, pointer, ty, same_entry) in dups {
                let new = self.ids.next(ty).map(|x| x.as_str().to_owned()).unwrap_or_else(|_| format!("{old}2"));
                let item = match k {
                    0 => &mut self.project,
                    1 => &mut self.palettes[i],
                    2 => &mut self.banks[i],
                    3 => &mut self.drawings[i],
                    4 => &mut self.scenes[i],
                    5 => &mut self.elements[i],
                    _ => match self.library.as_mut() {
                        Some(l) => l,
                        None => continue,
                    },
                };
                if same_entry {
                    if let Some(slot) = pointer_mut(&mut item.value, &pointer) {
                        *slot = json!(new);
                    }
                } else {
                    rename_refs(&mut item.value, &old, &new);
                }
                item.touched = true;
                let id_owner = item.id();
                self.issues.repaired_error(
                    codes::ID_DUPLICATE,
                    Some(&new),
                    Some(&path),
                    format!("{} `{old}` duplicates an earlier object; it became `{new}`", ty.as_str()),
                    json!({ "oldId": old, "newId": new, "pointer": pointer, "entryObject": id_owner }),
                );
            }
        }
    }

    /// Names unique in their scope (10 §1.3, §7.2 `doc.nameRepaired`): palettes and scenes in the project, elements in
    /// their scene, drawings in their bank.
    pub(crate) fn names(&mut self) {
        let fix = |items: &mut Vec<Item>, idx: &[usize], ty: ObjectType, issues: &mut Issues| {
            let mut taken = BTreeSet::new();
            let mut idx = idx.to_vec();
            idx.sort_by(|a, b| items[*a].path.cmp(&items[*b].path));
            for i in idx {
                let it = &mut items[i];
                let old = str_of(&it.value, "name").unwrap_or_default().to_owned();
                let new = unique_name(ty, &old, &taken);
                taken.insert(new.clone());
                if new != old {
                    if let Some(m) = it.value.as_object_mut() {
                        m.insert("name".into(), json!(new));
                    }
                    it.touched = true;
                    let id = it.id();
                    issues.repaired_warning(
                        codes::NAME_REPAIRED,
                        Some(&id),
                        Some(&it.path),
                        format!("the duplicate name `{old}` became `{new}`"),
                        json!({ "pointer": "/name", "old": old, "new": new }),
                    );
                }
            }
        };
        // Project order for palettes and scenes.
        let order = |items: &[Item], listed: &[Value]| -> Vec<usize> {
            let ids: Vec<String> = listed.iter().filter_map(|s| str_of(s, "id").map(str::to_owned)).collect();
            let mut v: Vec<usize> = (0..items.len()).collect();
            v.sort_by_key(|i| ids.iter().position(|x| *x == items[*i].id()).unwrap_or(usize::MAX));
            v
        };
        let pal = order(&self.palettes, arr(&self.project.value, "palettes"));
        fix(&mut self.palettes, &pal, ObjectType::Palette, &mut self.issues);
        let sc = order(&self.scenes, arr(&self.project.value, "scenes"));
        fix(&mut self.scenes, &sc, ObjectType::Scene, &mut self.issues);
        for s in self.scenes.clone() {
            let members: Vec<String> =
                arr(&s.value, "elements").iter().filter_map(|x| str_of(x, "id").map(str::to_owned)).collect();
            let idx: Vec<usize> =
                members.iter().filter_map(|m| self.elements.iter().position(|e| e.id() == *m)).collect();
            fix(&mut self.elements, &idx, ObjectType::Element, &mut self.issues);
        }
        for b in self.banks.clone() {
            let members: Vec<String> =
                arr(&b.value, "drawings").iter().filter_map(|x| str_of(x, "id").map(str::to_owned)).collect();
            let idx: Vec<usize> =
                members.iter().filter_map(|m| self.drawings.iter().position(|d| d.id() == *m)).collect();
            fix(&mut self.drawings, &idx, ObjectType::Drawing, &mut self.issues);
        }
    }

    /// References (10 §7.2): banks of cell-based elements, exposures, placements, sequences, lip-sync mappings,
    /// current cameras, palette styles, scene trees and column orders, at least one visible palette.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn references(&mut self) {
        // At least one visible palette (03 §1.2); a project without palettes gets the default palette.
        if self.palettes.is_empty() {
            let mut ids = self.ids.clone();
            if let Ok((v, _)) = tt_model::defaults::default_palette_json(&mut ids, "Default") {
                let id = str_of(&v, "id").unwrap_or_default().to_owned();
                self.palettes.push(Item {
                    path: format!("palettes/{id}.json"),
                    ty: ObjectType::Palette,
                    owner: None,
                    value: v,
                    overlay: None,
                    frozen: None,
                    touched: true,
                });
                if let Some(list) = arr_mut(&mut self.project.value, "palettes") {
                    list.push(json!({ "id": id, "type": "palette" }));
                }
                self.project.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&id),
                    Some(layout::PROJECT),
                    "the project has no readable palette; the default palette was added",
                    json!({}),
                );
            }
        }
        if !self.palettes.iter().any(|p| p.value.get("hidden") != Some(&Value::Bool(true))) {
            let first = arr(&self.project.value, "palettes").first().and_then(|s| str_of(s, "id")).map(str::to_owned);
            if let Some(p) = first.and_then(|f| self.palettes.iter_mut().find(|p| p.id() == f)) {
                if let Some(m) = p.value.as_object_mut() {
                    m.insert("hidden".into(), json!(false));
                }
                p.touched = true;
                let id = p.id();
                self.issues.repaired_warning(
                    codes::VALUE_CLAMPED,
                    Some(&id),
                    Some(&p.path.clone()),
                    "every palette was hidden; the first one is shown (03 §1.2)",
                    json!({ "pointer": "/hidden", "old": true, "new": false }),
                );
            }
        }
        // Palette styles available.
        let styles: BTreeMap<String, Vec<String>> = self
            .palettes
            .iter()
            .map(|p| {
                (p.id(), arr(&p.value, "styles").iter().filter_map(|s| str_of(s, "id").map(str::to_owned)).collect())
            })
            .collect();
        let palette_order: Vec<String> =
            arr(&self.project.value, "palettes").iter().filter_map(|s| str_of(s, "id").map(str::to_owned)).collect();
        let assets: BTreeMap<String, (String, String)> = arr(&self.project.value, "assets")
            .iter()
            .filter_map(|a| {
                Some((
                    str_of(a, "id")?.to_owned(),
                    (
                        str_of(a, "kind").unwrap_or_default().to_owned(),
                        str_of(a, "storage").unwrap_or_default().to_owned(),
                    ),
                ))
            })
            .collect();
        let bank_drawings: BTreeMap<String, BTreeSet<String>> = self
            .banks
            .iter()
            .map(|b| {
                (b.id(), arr(&b.value, "drawings").iter().filter_map(|d| str_of(d, "id").map(str::to_owned)).collect())
            })
            .collect();
        // Cell-based elements whose bank is missing get an empty bank with that ID.
        let mut new_banks = Vec::new();
        for e in &mut self.elements {
            let has_bank = element_type(&e.value).is_some_and(ElementType::has_exposure);
            let Some(b) = str_of(&e.value, "drawingBankId").map(str::to_owned) else { continue };
            if !has_bank || bank_drawings.contains_key(&b) || new_banks.iter().any(|(x, _): &(String, String)| *x == b)
            {
                continue;
            }
            new_banks.push((b, e.id()));
        }
        for (b, e) in new_banks {
            self.banks.push(Item {
                path: format!("banks/{b}/bank.json"),
                ty: ObjectType::DrawingBank,
                owner: None,
                value: json!({ "id": b, "type": "drawingBank", "rev": 1, "drawings": [] }),
                overlay: None,
                frozen: None,
                touched: true,
            });
            if let Some(list) = arr_mut(&mut self.project.value, "drawingBanks") {
                list.push(json!({ "id": b, "type": "drawingBank" }));
            }
            self.project.touched = true;
            self.issues.repaired_error(
                codes::REF_DANGLING,
                Some(&e),
                None,
                format!("drawing bank `{b}` of element `{e}` does not exist; an empty bank was created"),
                json!({ "pointer": "/drawingBankId" }),
            );
        }
        let bank_drawings: BTreeMap<String, BTreeSet<String>> = self
            .banks
            .iter()
            .map(|b| {
                (b.id(), arr(&b.value, "drawings").iter().filter_map(|d| str_of(d, "id").map(str::to_owned)).collect())
            })
            .collect();
        // Per scene.
        for si in 0..self.scenes.len() {
            let scene_id = self.scenes[si].id();
            let scene_path = self.scenes[si].path.clone();
            let members: Vec<String> = arr(&self.scenes[si].value, "elements")
                .iter()
                .filter_map(|x| str_of(x, "id").map(str::to_owned))
                .collect();
            let member_set: BTreeSet<&String> = members.iter().collect();
            // Element references.
            for eid in &members {
                let Some(ei) = self.elements.iter().position(|e| e.id() == *eid) else { continue };
                let e = &mut self.elements[ei];
                let epath = e.path.clone();
                let bank = str_of(&e.value, "drawingBankId").map(str::to_owned);
                let in_bank = |d: &str| bank.as_ref().and_then(|b| bank_drawings.get(b)).is_some_and(|s| s.contains(d));
                if let Some(runs) = arr_mut(&mut e.value, "exposure") {
                    let mut foreign = Vec::new();
                    for run in runs.iter_mut() {
                        if let Some(d) = run.get(0).and_then(Value::as_str).map(str::to_owned)
                            && !in_bank(&d)
                        {
                            foreign.push(d);
                            if let Some(slot) = run.get_mut(0) {
                                *slot = Value::Null;
                            }
                        }
                    }
                    if !foreign.is_empty() {
                        e.touched = true;
                        self.issues.repaired_warning(
                            codes::EXPOSURE_INVALID,
                            Some(eid),
                            Some(&epath),
                            "the exposure names drawings outside the element's bank; those cells became blank",
                            json!({ "drawings": foreign }),
                        );
                    }
                }
                let e = &mut self.elements[ei];
                for (field, needs) in [("placements", "placement"), ("sequences", "sequence")] {
                    let Some(list) = arr_mut(&mut e.value, field) else { continue };
                    let mut removed = Vec::new();
                    list.retain(|p| {
                        let ok = str_of(p, "assetId").and_then(|a| assets.get(a)).is_some_and(|(kind, storage)| {
                            if needs == "placement" { kind == "sound" || kind == "movie" } else { storage == "linked" }
                        });
                        if !ok {
                            removed.push(p.clone());
                        }
                        ok
                    });
                    if !removed.is_empty() {
                        e.touched = true;
                        self.issues.repaired_error(
                            codes::REF_DANGLING,
                            Some(eid),
                            Some(&epath),
                            format!("{field} with missing or unsuitable assets were removed"),
                            json!({ "removed": removed }),
                        );
                    }
                }
            }
            // Lip-sync mappings: the mapped element must be a member, and mapped drawings in its bank.
            let banks_of: BTreeMap<String, Option<String>> =
                self.elements.iter().map(|e| (e.id(), str_of(&e.value, "drawingBankId").map(str::to_owned))).collect();
            for eid in &members {
                let Some(ei) = self.elements.iter().position(|e| e.id() == *eid) else { continue };
                let e = &mut self.elements[ei];
                let epath = e.path.clone();
                let Some(maps) = arr_mut(&mut e.value, "lipSyncMappings") else { continue };
                let mut removed = Vec::new();
                maps.retain(|m| {
                    let ok = str_of(m, "drawingElementId").is_some_and(|t| member_set.contains(&t.to_owned()));
                    if !ok {
                        removed.push(m.clone());
                    }
                    ok
                });
                for m in maps.iter_mut() {
                    let target = str_of(m, "drawingElementId").unwrap_or_default().to_owned();
                    let bank = banks_of.get(&target).cloned().flatten();
                    let allowed = bank.and_then(|b| bank_drawings.get(&b)).cloned().unwrap_or_default();
                    if let Some(map) = m.get_mut("mapping").and_then(Value::as_object_mut) {
                        let bad: Vec<String> = map
                            .iter()
                            .filter(|(_, d)| d.as_str().is_some_and(|d| !allowed.contains(d)))
                            .map(|(k, _)| k.clone())
                            .collect();
                        for k in bad {
                            if let Some(old) = map.remove(&k) {
                                removed.push(json!({ "shape": k, "drawingId": old }));
                            }
                        }
                    }
                }
                if !removed.is_empty() {
                    e.touched = true;
                    self.issues.repaired_error(
                        codes::REF_DANGLING,
                        Some(eid),
                        Some(&epath),
                        "lip-sync mappings to elements or drawings that do not exist were removed",
                        json!({ "removed": removed }),
                    );
                }
            }
            // Tree and column order.
            self.repair_tree(si, &members);
            // Current camera.
            let cameras: Vec<String> = self
                .tree_order(si)
                .into_iter()
                .filter(|e| {
                    self.elements.iter().any(|x| x.id() == *e && element_type(&x.value) == Some(ElementType::Camera))
                })
                .collect();
            let scene = &mut self.scenes[si];
            let current = str_of(&scene.value, "currentCameraId").map(str::to_owned);
            let valid = current.as_ref().is_some_and(|c| cameras.contains(c));
            if !valid && !(current.is_none() && cameras.is_empty()) {
                let new = cameras.first().cloned();
                if let Some(m) = scene.value.as_object_mut() {
                    m.insert("currentCameraId".into(), json!(new));
                }
                scene.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&scene_id),
                    Some(&scene_path),
                    "the scene's current camera is not one of its cameras; it was reset",
                    json!({ "pointer": "/currentCameraId", "old": current, "new": new }),
                );
            }
            // Palette styles: exactly one entry per palette, each a style of it.
            let scene = &mut self.scenes[si];
            let old = scene.value.get("paletteStyles").cloned().unwrap_or(json!({}));
            let mut fixed = Map::new();
            for p in &palette_order {
                let have = old.get(p).and_then(Value::as_str).map(str::to_owned);
                let list = styles.get(p).cloned().unwrap_or_default();
                let pick = have.filter(|s| list.contains(s)).or_else(|| list.first().cloned());
                if let Some(s) = pick {
                    fixed.insert(p.clone(), json!(s));
                }
            }
            if Value::Object(fixed.clone()) != old {
                if let Some(m) = scene.value.as_object_mut() {
                    m.insert("paletteStyles".into(), Value::Object(fixed.clone()));
                }
                scene.touched = true;
                self.issues.repaired_error(
                    codes::REF_DANGLING,
                    Some(&scene_id),
                    Some(&scene_path),
                    "the scene's palette styles did not match the palettes; they were corrected",
                    json!({ "pointer": "/paletteStyles", "old": old, "new": Value::Object(fixed) }),
                );
            }
        }
    }

    fn tree_order(&self, si: usize) -> Vec<String> {
        fn walk(nodes: &[Value], out: &mut Vec<String>, depth: usize) {
            if depth > 1000 {
                return;
            }
            for n in nodes {
                if let Some(id) = str_of(n, "id") {
                    out.push(id.to_owned());
                }
                walk(arr(n, "children"), out, depth + 1);
            }
        }
        let mut out = Vec::new();
        walk(arr(&self.scenes[si].value, "tree"), &mut out, 0);
        out
    }

    /// The scene tree (10 §7.2 `tree.invalid`): nodes of unknown elements dropped (their children kept in their
    /// place), repeated elements kept at their first occurrence, children of elements that cannot have children moved
    /// up after their parent, parameter elements outside their effect removed (kept in the scene's ext), missing
    /// elements appended at the top level. The column order is rebuilt from the cell-based members.
    #[allow(clippy::too_many_lines)]
    fn repair_tree(&mut self, si: usize, members: &[String]) {
        let types: HashMap<String, (Option<ElementType>, Option<String>)> = self
            .elements
            .iter()
            .filter(|e| members.contains(&e.id()))
            .map(|e| (e.id(), (element_type(&e.value), str_of(&e.value, "slot").map(str::to_owned))))
            .collect();
        let scene_id = self.scenes[si].id();
        let scene_path = self.scenes[si].path.clone();
        let mut problems: Vec<Value> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut misplaced_params: Vec<String> = Vec::new();
        #[allow(clippy::too_many_arguments)]
        fn rebuild(
            nodes: &[Value],
            parent: Option<(ElementType, &str)>,
            types: &HashMap<String, (Option<ElementType>, Option<String>)>,
            seen: &mut BTreeSet<String>,
            problems: &mut Vec<Value>,
            params: &mut Vec<String>,
            depth: usize,
        ) -> (Vec<Value>, Vec<Value>) {
            // Returns (nodes for this level, nodes to hoist to the parent's level).
            let mut out = Vec::new();
            let mut hoist = Vec::new();
            if depth > 1000 {
                return (out, hoist);
            }
            for n in nodes {
                let Some(id) = str_of(n, "id").map(str::to_owned) else {
                    problems.push(json!({ "dropped": n, "why": "no id" }));
                    continue;
                };
                let children = arr(n, "children");
                let Some((ty, slot)) = types.get(&id) else {
                    problems.push(json!({ "dropped": id, "why": "not an element of the scene" }));
                    let (mut kids, mut up) = rebuild(children, parent, types, seen, problems, params, depth + 1);
                    out.append(&mut kids);
                    out.append(&mut up);
                    continue;
                };
                if !seen.insert(id.clone()) {
                    problems.push(json!({ "dropped": id, "why": "listed twice" }));
                    let (mut kids, mut up) = rebuild(children, parent, types, seen, problems, params, depth + 1);
                    out.append(&mut kids);
                    out.append(&mut up);
                    continue;
                }
                if *ty == Some(ElementType::EffectParameter) {
                    let ok = matches!(
                        (slot.as_deref(), parent.map(|p| p.0)),
                        (Some("mask"), Some(ElementType::Clipping))
                            | (Some("sprites"), Some(ElementType::ParticleNature | ElementType::ParticleImpact))
                            | (Some("bones" | "skin"), Some(ElementType::Skeleton))
                    );
                    if !ok {
                        params.push(id.clone());
                        problems.push(json!({ "removed": id, "why": "parameter element outside its effect" }));
                        continue;
                    }
                }
                let me = ty.map(|t| (t, id.as_str()));
                let (kids, up) = rebuild(children, me.or(parent), types, seen, problems, params, depth + 1);
                let (kids, moved) = if ty.is_some_and(ElementType::can_have_children) {
                    (kids, Vec::new())
                } else {
                    if !kids.is_empty() {
                        problems.push(json!({ "moved": kids.iter().filter_map(|k| str_of(k, "id")).collect::<Vec<_>>(), "why": "parent cannot have children" }));
                    }
                    (Vec::new(), kids)
                };
                out.push(json!({ "id": id, "children": kids }));
                out.extend(moved);
                hoist.extend(up);
            }
            (out, hoist)
        }
        let tree = arr(&self.scenes[si].value, "tree").to_vec();
        let (mut nodes, up) = rebuild(&tree, None, &types, &mut seen, &mut problems, &mut misplaced_params, 0);
        nodes.extend(up);
        for m in members {
            if !seen.contains(m) && !misplaced_params.contains(m) {
                if types.get(m).is_some_and(|(t, _)| *t == Some(ElementType::EffectParameter)) {
                    misplaced_params.push(m.clone());
                    problems.push(json!({ "removed": m, "why": "parameter element outside its effect" }));
                    continue;
                }
                nodes.push(json!({ "id": m, "children": [] }));
                problems.push(json!({ "appended": m, "why": "missing from the tree" }));
            }
        }
        // Parameter elements outside their effect: removed from the scene, kept in its ext (REP-02).
        for p in &misplaced_params {
            if let Some(ei) = self.elements.iter().position(|e| e.id() == *p) {
                let item = self.elements.remove(ei);
                put_ext(&mut self.scenes[si].value, "app.unreadable", &item.path, item.value);
            }
            if let Some(list) = arr_mut(&mut self.scenes[si].value, "elements") {
                list.retain(|x| str_of(x, "id") != Some(p.as_str()));
            }
        }
        let scene = &mut self.scenes[si];
        if !problems.is_empty() {
            if let Some(m) = scene.value.as_object_mut() {
                m.insert("tree".into(), Value::Array(nodes));
            }
            scene.touched = true;
            self.issues.repaired_error(
                codes::TREE_INVALID,
                Some(&scene_id),
                Some(&scene_path),
                "the scene tree was invalid and was repaired",
                json!({ "repairs": problems }),
            );
        }
        // Column order: the cell-based members, in their listed order, then the rest in tree order.
        let cells: Vec<String> = self
            .tree_order(si)
            .into_iter()
            .filter(|e| {
                types.get(e).and_then(|(t, _)| *t).is_some_and(ElementType::is_cell_based)
                    && !misplaced_params.contains(e)
            })
            .collect();
        let scene = &mut self.scenes[si];
        let old: Vec<String> =
            arr(&scene.value, "columnOrder").iter().filter_map(|x| x.as_str().map(str::to_owned)).collect();
        let mut order: Vec<String> = Vec::new();
        for e in &old {
            if cells.contains(e) && !order.contains(e) {
                order.push(e.clone());
            }
        }
        for e in &cells {
            if !order.contains(e) {
                order.push(e.clone());
            }
        }
        if order != old {
            if let Some(m) = scene.value.as_object_mut() {
                m.insert("columnOrder".into(), json!(order));
            }
            scene.touched = true;
            self.issues.repaired_error(
                codes::TREE_INVALID,
                Some(&scene_id),
                Some(&scene_path),
                "the column order did not list each cell-based element once; it was rebuilt",
                json!({ "pointer": "/columnOrder", "old": old, "new": order }),
            );
        }
    }

    /// The highest `rev` of any object (REV-01: `documentRev` is never below an object's `rev`).
    pub(crate) fn max_rev(&self) -> u64 {
        let mut max = 0;
        for it in self.all() {
            walk_indexed(&it.value, it.ty, &mut |obj, _, _, _| {
                if let Some(r) = obj.get("rev").and_then(Value::as_u64) {
                    max = max.max(r);
                }
            });
        }
        max
    }

    fn all(&self) -> impl Iterator<Item = &Item> {
        std::iter::once(&self.project)
            .chain(self.palettes.iter())
            .chain(self.banks.iter())
            .chain(self.drawings.iter())
            .chain(self.scenes.iter())
            .chain(self.elements.iter())
            .chain(self.library.iter())
    }

    /// Stamps the entry objects touched by repairs with `rev` (REV-02).
    pub(crate) fn stamp(&mut self, rev: u64) {
        let lists: [&mut Vec<Item>; 5] =
            [&mut self.palettes, &mut self.banks, &mut self.drawings, &mut self.scenes, &mut self.elements];
        for list in lists {
            for it in list.iter_mut().filter(|i| i.touched) {
                if let Some(m) = it.value.as_object_mut() {
                    m.insert("rev".into(), json!(rev));
                }
            }
        }
        for it in std::iter::once(&mut self.project).chain(self.library.iter_mut()).filter(|i| i.touched) {
            if let Some(m) = it.value.as_object_mut() {
                m.insert("rev".into(), json!(rev));
            }
        }
    }

    /// Whether any repair changed the content.
    pub(crate) fn any_touched(&self) -> bool {
        self.all().any(|i| i.touched)
    }

    /// Parses every entry into typed parts (the model's input path again: the repaired JSON is validated, not trusted).
    pub(crate) fn parts(&self, document_rev: u64) -> Result<DocumentParts, Vec<ModelError>> {
        fn typed<T: tt_model::entities::ModelObject + tt_model::validate::Check>(
            it: &Item,
            errors: &mut Vec<ModelError>,
        ) -> Option<T> {
            object::from_json::<T>(&it.value)
                .map_err(|es| {
                    for e in es {
                        let mut d = e.details().clone();
                        if let Some(m) = d.as_object_mut() {
                            m.insert("entry".into(), json!(it.path));
                        }
                        errors.push(ModelError::with_details(e.code(), e.message().to_owned(), d));
                    }
                })
                .ok()
        }
        let mut errors = Vec::new();
        let project: Option<Project> = typed(&self.project, &mut errors);
        let palettes = self.palettes.iter().filter_map(|i| typed::<Palette>(i, &mut errors)).collect();
        let banks = self.banks.iter().filter_map(|i| typed::<DrawingBank>(i, &mut errors)).collect();
        let drawings = self.drawings.iter().filter_map(|i| typed::<Drawing>(i, &mut errors)).collect();
        let scenes = self.scenes.iter().filter_map(|i| typed::<Scene>(i, &mut errors)).collect();
        let elements = self.elements.iter().filter_map(|i| typed::<Element>(i, &mut errors)).collect();
        let library: Option<Library> = self.library.as_ref().and_then(|i| typed(i, &mut errors));
        match (project, library) {
            (Some(project), Some(library)) if errors.is_empty() => {
                Ok(DocumentParts { document_rev, project, palettes, banks, drawings, scenes, elements, library })
            }
            _ => Err(errors),
        }
    }

    /// Entry keys of the overlays and frozen entries, by the entries' final IDs.
    pub(crate) fn preserved(&self) -> (BTreeMap<EntryKey, Overlay>, BTreeMap<EntryKey, FrozenEntry>) {
        let mut overlays = BTreeMap::new();
        let mut frozen = BTreeMap::new();
        for it in self.all() {
            let key = match it.ty {
                ObjectType::Project => Some(EntryKey::Project),
                ObjectType::Library => Some(EntryKey::Library),
                ty => Id::parse(&it.id()).ok().map(|id| match ty {
                    ObjectType::Palette => EntryKey::Palette(id),
                    ObjectType::DrawingBank => EntryKey::Bank(id),
                    ObjectType::Drawing => EntryKey::Drawing(id),
                    ObjectType::Scene => EntryKey::Scene(id),
                    _ => EntryKey::Element(id),
                }),
            };
            let Some(key) = key else { continue };
            if let Some(o) = &it.overlay {
                overlays.insert(key.clone(), o.clone());
            }
            if let Some(f) = &it.frozen {
                frozen.insert(key, f.clone());
            }
        }
        (overlays, frozen)
    }

    /// The last-resort pass: an assembled document still has errors (a rule without a dedicated repair). Entries that
    /// can be dropped (palettes, elements) are treated as unreadable; returns whether anything changed.
    pub(crate) fn drop_failing(&mut self, doc: &Document) -> bool {
        let mut changed = false;
        let errors: Vec<_> = doc.issues().into_iter().filter(|i| i.severity == Severity::Error).collect();
        for e in errors {
            let Some(id) = e.object_id.clone() else { continue };
            for ty in [ObjectType::Element, ObjectType::Palette] {
                if let Some(pos) = self.list_mut(ty).iter().position(|x| x.id() == id) {
                    let item = self.list_mut(ty).remove(pos);
                    let raw = item.value.clone();
                    let err = ModelError::with_details(e.code, e.message.clone(), json!({ "pointer": e.pointer }));
                    self.unreadable(item, raw, &[err]);
                    changed = true;
                    break;
                }
            }
        }
        changed
    }
}

fn placeholder(ty: ObjectType) -> Item {
    Item { path: String::new(), ty, owner: None, value: Value::Null, overlay: None, frozen: None, touched: false }
}

/// Keeps the first occurrence of each element in a scene's tree (the children of a dropped repeat stay in its place),
/// column order and element list. Returns what was removed.
fn sanitize_scene(scene: &mut Value) -> Vec<Value> {
    fn tree(nodes: &[Value], seen: &mut BTreeSet<String>, removed: &mut Vec<Value>, depth: usize) -> Vec<Value> {
        let mut out = Vec::new();
        if depth > 1000 {
            return out;
        }
        for n in nodes {
            let id = n.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            let kids =
                tree(n.get("children").and_then(Value::as_array).map_or(&[], Vec::as_slice), seen, removed, depth + 1);
            if seen.insert(id.clone()) {
                out.push(json!({ "id": id, "children": kids }));
            } else {
                removed.push(json!({ "tree": id }));
                out.extend(kids);
            }
        }
        out
    }
    let mut removed = Vec::new();
    let mut seen = BTreeSet::new();
    let nodes = scene.get("tree").and_then(Value::as_array).cloned().unwrap_or_default();
    let fixed = tree(&nodes, &mut seen, &mut removed, 0);
    let mut order_seen = BTreeSet::new();
    let order: Vec<Value> = scene
        .get("columnOrder")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|x| {
            let keep = order_seen.insert(x.to_string());
            if !keep {
                removed.push(json!({ "columnOrder": x }));
            }
            keep
        })
        .cloned()
        .collect();
    let mut el_seen = BTreeSet::new();
    let elements: Vec<Value> = scene
        .get("elements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|x| {
            let keep = el_seen.insert(x.get("id").map(Value::to_string).unwrap_or_default());
            if !keep {
                removed.push(json!({ "elements": x }));
            }
            keep
        })
        .cloned()
        .collect();
    if let Some(m) = scene.as_object_mut() {
        m.insert("tree".into(), Value::Array(fixed));
        if m.contains_key("columnOrder") {
            m.insert("columnOrder".into(), Value::Array(order));
        }
        if m.contains_key("elements") {
            m.insert("elements".into(), Value::Array(elements));
        }
    }
    removed
}

fn transition(id: &str) -> Value {
    json!({
        "id": id, "type": "transition", "rev": 1, "transitionType": "none", "durationLeft": 12, "blankDuration": 0,
        "durationRight": 12, "colour": "#000000FF", "angle": 0, "direction": null, "size": 25
    })
}

/// A visitor of indexed objects: `(object, type, container, pointer)`.
pub(crate) type Visit<'a> = dyn FnMut(&Value, ObjectType, Option<&str>, &str) + 'a;

/// Visits every object of an entry the model indexes (10 §3): the entry object and the objects reachable through its
/// inline child slots (`slots_of`), never stubs of split children and never opaque or copied values (`meta`, `ext`,
/// colour-recovery copies). `f(object, type, container, pointer)`; `container` is the drawing of nodes and strokes and
/// the channel of keys and motion points (10 ID-03).
pub(crate) fn walk_indexed(v: &Value, ty: ObjectType, f: &mut Visit<'_>) {
    fn go(v: &Value, ty: ObjectType, container: Option<&str>, pointer: &str, depth: usize, f: &mut Visit<'_>) {
        if depth > 200 {
            return;
        }
        f(v, ty, container, pointer);
        let own = v.get("id").and_then(Value::as_str);
        let inner = match ty {
            ObjectType::Drawing | ObjectType::Channel => own,
            _ => container,
        };
        for (field, slot) in slots_of(ty) {
            let Some(child) = v.get(*field) else { continue };
            let at = format!("{pointer}/{}", tt_model::pointer::escape(field));
            let child_ty = |c: &Value| c.get("type").and_then(Value::as_str).and_then(ObjectType::parse);
            match slot {
                tt_model::document::SlotKind::Inline => {
                    for (i, c) in child.as_array().into_iter().flatten().enumerate() {
                        if let Some(t) = child_ty(c) {
                            go(c, t, inner, &format!("{at}/{i}"), depth + 1, f);
                        }
                    }
                }
                tt_model::document::SlotKind::InlineOne => {
                    if let Some(t) = child_ty(child) {
                        go(child, t, inner, &at, depth + 1, f);
                    }
                }
                tt_model::document::SlotKind::InlineMap => {
                    for (k, c) in child.as_object().into_iter().flatten() {
                        if let Some(t) = child_ty(c) {
                            go(c, t, inner, &format!("{at}/{}", tt_model::pointer::escape(k)), depth + 1, f);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    go(v, ty, None, "", 0, f);
}

/// Collects inline objects whose ID was seen before. `seen`: (scope, id) → entry path of the first occurrence. Entry
/// objects are registered by their own pass (`is_entry`).
#[allow(clippy::too_many_arguments)]
fn collect_ids(
    v: &Value,
    ty: ObjectType,
    _container: Option<&str>,
    path: &str,
    seen: &mut HashMap<(String, String), String>,
    dups: &mut Vec<(String, String, ObjectType, bool)>,
    is_entry: bool,
) {
    let entry_id = v.get(ty.id_field()).and_then(Value::as_str).unwrap_or_default().to_owned();
    walk_indexed(v, ty, &mut |obj, t, container, pointer| {
        if is_entry && pointer.is_empty() {
            if !entry_id.is_empty() {
                seen.entry((String::new(), entry_id.clone())).or_insert_with(|| path.to_owned());
            }
            return;
        }
        let field = t.id_field();
        let Some(id) = obj.get(field).and_then(Value::as_str) else { return };
        let scope =
            if t.local_id_container().is_some() { container.unwrap_or_default().to_owned() } else { String::new() };
        let key = (scope, id.to_owned());
        match seen.get(&key) {
            Some(first) => dups.push((id.to_owned(), format!("{pointer}/{field}"), t, first == path)),
            None => {
                seen.insert(key, path.to_owned());
            }
        }
    });
}

/// Where an entry of the given kind is filed while loading.
pub(crate) fn item_for(kind: &Kind, path: &str, value: Value) -> Option<Item> {
    let (ty, owner) = match kind {
        Kind::Palette(_) => (ObjectType::Palette, None),
        Kind::Bank(_) => (ObjectType::DrawingBank, None),
        Kind::Drawing { bank, .. } => (ObjectType::Drawing, Some(bank.clone())),
        Kind::Scene(_) => (ObjectType::Scene, None),
        Kind::Element { scene, .. } => (ObjectType::Element, Some(scene.clone())),
        Kind::LibraryIndex => (ObjectType::Library, None),
        Kind::Project => (ObjectType::Project, None),
        _ => return None,
    };
    Some(Item { path: path.to_owned(), ty, owner, value, overlay: None, frozen: None, touched: false })
}
