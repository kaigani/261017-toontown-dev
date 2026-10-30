//! Object JSON: one serialiser for query results and persisted entries (10 DM-1, §2.2 stubs and depth).

use serde_json::{Map, Value};

use super::{Document, EntryKey};
use crate::entities::Stub;
use crate::entities::drawing::Drawing;
use crate::error::{ModelError, codes};
use crate::id::Id;
use crate::types::ObjectType;

/// How a child collection of an object is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// A list of children stored in their own entries (stubs in the parent's entry).
    Split(ObjectType),
    /// One child stored in its own entry.
    SplitOne(ObjectType),
    /// A list of inline child objects.
    Inline,
    /// One inline child object.
    InlineOne,
    /// A map of inline child objects (an element's `channels`).
    InlineMap,
}

/// The child collections of each type (10 §3; the `type` of inline children is read from the child).
pub fn slots(ty: ObjectType) -> &'static [(&'static str, Slot)] {
    match ty {
        ObjectType::Project => &[
            ("startTransition", Slot::InlineOne),
            ("palettes", Slot::Split(ObjectType::Palette)),
            ("pens", Slot::Inline),
            ("assets", Slot::Inline),
            ("drawingBanks", Slot::Split(ObjectType::DrawingBank)),
            ("scenes", Slot::Split(ObjectType::Scene)),
            ("library", Slot::SplitOne(ObjectType::Library)),
        ],
        ObjectType::Palette => &[("styles", Slot::Inline), ("swatches", Slot::Inline)],
        ObjectType::DrawingBank => &[("drawings", Slot::Split(ObjectType::Drawing))],
        ObjectType::Drawing => &[("nodes", Slot::Inline)],
        ObjectType::DrawingNode => &[("strokes", Slot::Inline), ("children", Slot::Inline)],
        ObjectType::Scene => &[("outTransition", Slot::InlineOne), ("elements", Slot::Split(ObjectType::Element))],
        ObjectType::Element => &[
            ("channels", Slot::InlineMap),
            ("blocks", Slot::Inline),
            ("placements", Slot::Inline),
            ("sequences", Slot::Inline),
        ],
        ObjectType::Channel => &[("keys", Slot::Inline), ("points", Slot::Inline)],
        ObjectType::Library | ObjectType::LibraryFolder => &[("children", Slot::Inline)],
        _ => &[],
    }
}

/// Options of [`Document::object_json`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Object levels returned in full: `1` = the object with its children as stubs (the default of
    /// `doc.object.get`), `n` = `n` levels, `0` = the object itself as a stub, `None` = everything (`depth: -1`).
    pub depth: Option<u32>,
    /// Add the transient canonical `path` to every full object (query results, 10 §2.4).
    pub path: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { depth: Some(1), path: true }
    }
}

fn type_of(v: &Value) -> Option<ObjectType> {
    v.get("type").and_then(Value::as_str).and_then(ObjectType::parse)
}

fn id_of(v: &Value, ty: ObjectType) -> Option<&str> {
    v.get(ty.id_field()).and_then(Value::as_str)
}

fn stub_of(v: &Value, ty: ObjectType) -> Value {
    let id = id_of(v, ty).and_then(|s| Id::parse(s).ok());
    let name = v.get("name").and_then(Value::as_str).map(str::to_owned);
    match id {
        Some(id) => Stub::new(id, ty, name).to_json(),
        None => Value::Null,
    }
}

fn internal(e: impl std::fmt::Display) -> ModelError {
    ModelError::new(codes::INTERNAL, format!("serialisation failed: {e}"))
}

/// Inserts `path` right after the envelope (`name`, or `rev` for unnamed types).
fn with_path(v: Value, path: String) -> Value {
    let Value::Object(m) = v else { return v };
    let after = if m.contains_key("name") { "name" } else { "rev" };
    let mut out = Map::with_capacity(m.len() + 1);
    let mut inserted = false;
    for (k, val) in m {
        let is_after = k == after;
        out.insert(k, val);
        if is_after {
            out.insert("path".into(), Value::String(path.clone()));
            inserted = true;
        }
    }
    if !inserted {
        out.insert("path".into(), Value::String(path));
    }
    Value::Object(out)
}

#[derive(Clone, Copy)]
enum Depth {
    All,
    Levels(u32),
}

impl Depth {
    fn child_full(self) -> bool {
        match self {
            Self::All => true,
            Self::Levels(n) => n > 1,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::All => Self::All,
            Self::Levels(n) => Self::Levels(n.saturating_sub(1)),
        }
    }
}

impl Document {
    /// The JSON of an entry object as stored (split children as ID stubs without names).
    pub(super) fn entry_raw(&self, key: &EntryKey) -> Result<Value, ModelError> {
        let v = match key {
            EntryKey::Project => serde_json::to_value(&*self.project),
            EntryKey::Library => serde_json::to_value(&*self.library),
            EntryKey::Palette(id) => {
                self.palettes.get(id).map(|o| serde_json::to_value(&**o)).ok_or(()).map_err(|()| missing(id))?
            }
            EntryKey::Bank(id) => {
                self.banks.get(id).map(|o| serde_json::to_value(&**o)).ok_or(()).map_err(|()| missing(id))?
            }
            EntryKey::Drawing(id) => {
                self.drawings.get(id).map(|o| serde_json::to_value(&**o)).ok_or(()).map_err(|()| missing(id))?
            }
            EntryKey::Scene(id) => {
                self.scenes.get(id).map(|o| serde_json::to_value(&**o)).ok_or(()).map_err(|()| missing(id))?
            }
            EntryKey::Element(id) => {
                self.elements.get(id).map(|o| serde_json::to_value(&**o)).ok_or(()).map_err(|()| missing(id))?
            }
        };
        v.map_err(internal)
    }

    fn split_child_raw(&self, ty: ObjectType, id: &str) -> Option<Value> {
        let id = Id::parse(id).ok()?;
        let key = match ty {
            ObjectType::Palette => EntryKey::Palette(id),
            ObjectType::DrawingBank => EntryKey::Bank(id),
            ObjectType::Drawing => EntryKey::Drawing(id),
            ObjectType::Scene => EntryKey::Scene(id),
            ObjectType::Element => EntryKey::Element(id),
            ObjectType::Library => EntryKey::Library,
            _ => return None,
        };
        self.entry_raw(&key).ok()
    }

    /// The persisted JSON of an entry (10 §6.3): inline children in full, split children as `{id, type, name}`
    /// stubs, no derived or transient properties. This is exactly the query JSON at the split depth (DM-1).
    ///
    /// # Errors
    /// `ref.notFound` for a missing entry; `internal` if serialisation fails.
    pub fn entry_json(&self, key: &EntryKey) -> Result<Value, ModelError> {
        let raw = self.entry_raw(key)?;
        let ty = type_of(&raw).ok_or_else(|| internal("entry without type"))?;
        self.render(raw, ty, None, false, true)
    }

    /// The canonical text of an entry (10 §6.5).
    ///
    /// # Errors
    /// As [`Document::entry_json`].
    pub fn entry_canonical(&self, key: &EntryKey) -> Result<String, ModelError> {
        crate::canonical::to_canonical_string(&self.entry_json(key)?)
    }

    /// The JSON of any object by plain or compound ID, as `doc.object.get` returns it (10 §10.2): the object with
    /// children expanded to `depth` and stubs beyond, plus the transient `path` when requested.
    ///
    /// # Errors
    /// `ref.notFound` when no object has the ID; `internal` if serialisation fails.
    pub fn object_json(&self, id: &str, opts: RenderOptions) -> Result<Value, ModelError> {
        let loc = self.locate(id).ok_or_else(|| {
            ModelError::with_details(codes::REF_NOT_FOUND, format!("no object `{id}`"), serde_json::json!({"ref": id}))
        })?;
        let entry = self.entry_raw(&loc.entry)?;
        let ty = loc.ty;
        let mut raw = if ty.is_entry() {
            entry
        } else if ty.local_id_container() == Some(ObjectType::Channel) {
            // Keys and motion points are unique only within their channel: find the channel first.
            let channel =
                loc.parent.as_deref().and_then(|c| Id::parse(c).ok()).ok_or_else(|| internal("key without channel"))?;
            let ch = find_inline(entry, ObjectType::Channel, &channel)
                .ok_or_else(|| internal("indexed channel not found"))?;
            find_inline(ch, ty, &loc.id).ok_or_else(|| internal("indexed object not found in its channel"))?
        } else {
            find_inline(entry, ty, &loc.id).ok_or_else(|| internal("indexed object not found in its entry"))?
        };
        if opts.depth == Some(0) {
            return Ok(self.stub_with_name(&raw, ty));
        }
        let ctx = Ctx::from_key(ty, &loc.key);
        self.render_from(&mut raw, ty, opts.depth, opts.path, false, &ctx)?;
        Ok(raw)
    }

    fn stub_with_name(&self, raw: &Value, ty: ObjectType) -> Value {
        stub_of(raw, ty)
    }

    /// Renders an object value. `split = true` renders a persisted entry: split children stay stubs (with names),
    /// inline children are always full.
    fn render(
        &self,
        mut v: Value,
        ty: ObjectType,
        depth: Option<u32>,
        path: bool,
        split: bool,
    ) -> Result<Value, ModelError> {
        self.render_from(&mut v, ty, depth, path, split, &Ctx::default())?;
        Ok(v)
    }

    fn render_from(
        &self,
        v: &mut Value,
        ty: ObjectType,
        depth: Option<u32>,
        path: bool,
        split: bool,
        ctx: &Ctx,
    ) -> Result<(), ModelError> {
        let depth = match depth {
            None => Depth::All,
            Some(n) => Depth::Levels(n),
        };
        self.render_at(v, ty, depth, path, split, ctx, 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_at(
        &self,
        v: &mut Value,
        ty: ObjectType,
        depth: Depth,
        path: bool,
        split: bool,
        ctx: &Ctx,
        guard: usize,
    ) -> Result<(), ModelError> {
        if guard > crate::MAX_JSON_DEPTH {
            return Err(internal("object nesting too deep"));
        }
        let id = id_of(v, ty).map(str::to_owned).unwrap_or_default();
        let key = ctx.key(ty, &id);
        let child_ctx = ctx.enter(ty, &id);
        for (field, slot) in slots(ty) {
            let Some(child) = v.get_mut(*field) else { continue };
            match slot {
                Slot::Split(cty) | Slot::SplitOne(cty) => {
                    let items: Vec<&mut Value> = match child {
                        Value::Array(a) => a.iter_mut().collect(),
                        other => vec![other],
                    };
                    for item in items {
                        let Some(cid) = id_of(item, *cty).map(str::to_owned) else { continue };
                        let Some(full) = self.split_child_raw(*cty, &cid) else { continue };
                        if !split && depth.child_full() {
                            let mut full = full;
                            self.render_at(&mut full, *cty, depth.next(), path, split, &Ctx::default(), guard + 1)?;
                            *item = full;
                        } else {
                            *item = stub_of(&full, *cty);
                        }
                    }
                }
                Slot::Inline | Slot::InlineOne | Slot::InlineMap => {
                    let items: Vec<&mut Value> = match child {
                        Value::Array(a) => a.iter_mut().collect(),
                        Value::Object(m) if *slot == Slot::InlineMap => m.values_mut().collect(),
                        other => vec![other],
                    };
                    for item in items {
                        let Some(cty) = type_of(item) else { continue };
                        if split || depth.child_full() {
                            let d = if split { Depth::All } else { depth.next() };
                            self.render_at(item, cty, d, path, split, &child_ctx, guard + 1)?;
                        } else {
                            *item = stub_of(item, cty);
                        }
                    }
                }
            }
        }
        if path && !id.is_empty() {
            let p = self.canonical_path_of(ty, &key);
            *v = with_path(std::mem::take(v), p);
        }
        Ok(())
    }
}

/// The containers of container-local IDs while rendering (10 ID-03).
#[derive(Debug, Clone, Default)]
struct Ctx {
    drawing: Option<String>,
    channel: Option<String>,
}

impl Ctx {
    fn from_key(ty: ObjectType, key: &str) -> Self {
        let container = key.split_once('/').map(|(c, _)| c.to_owned());
        match ty.local_id_container() {
            Some(ObjectType::Drawing) => Self { drawing: container, channel: None },
            Some(ObjectType::Channel) => Self { drawing: None, channel: container },
            _ => Self::default(),
        }
    }

    fn key(&self, ty: ObjectType, id: &str) -> String {
        let container = match ty.local_id_container() {
            Some(ObjectType::Drawing) => self.drawing.as_deref(),
            Some(ObjectType::Channel) => self.channel.as_deref(),
            _ => None,
        };
        match container {
            Some(c) => format!("{c}/{id}"),
            None => id.to_owned(),
        }
    }

    fn enter(&self, ty: ObjectType, id: &str) -> Self {
        match ty {
            ObjectType::Drawing => Self { drawing: Some(id.to_owned()), channel: None },
            ObjectType::Channel => Self { drawing: self.drawing.clone(), channel: Some(id.to_owned()) },
            _ => self.clone(),
        }
    }
}

fn missing(id: &Id) -> ModelError {
    ModelError::with_details(codes::REF_NOT_FOUND, format!("no object `{id}`"), serde_json::json!({"ref": id.as_str()}))
}

/// Finds an inline object (by type and plain ID) inside an entry's JSON.
pub(super) fn find_inline(entry: Value, ty: ObjectType, id: &Id) -> Option<Value> {
    fn go(v: Value, ty: ObjectType, id: &str, owner: ObjectType, depth: usize) -> Option<Value> {
        if depth > crate::MAX_JSON_DEPTH {
            return None;
        }
        let Value::Object(mut m) = v else { return None };
        for (field, slot) in slots(owner) {
            if matches!(slot, Slot::Split(_) | Slot::SplitOne(_)) {
                continue;
            }
            let Some(child) = m.remove(*field) else { continue };
            let items: Vec<Value> = match child {
                Value::Array(a) => a,
                Value::Object(cm) if *slot == Slot::InlineMap => cm.into_iter().map(|(_, v)| v).collect(),
                other => vec![other],
            };
            for item in items {
                let Some(cty) = type_of(&item) else { continue };
                if cty == ty && id_of(&item, cty) == Some(id) {
                    return Some(item);
                }
                if let Some(found) = go(item, ty, id, cty, depth + 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    let owner = type_of(&entry)?;
    go(entry, ty, id.as_str(), owner, 0)
}

/// Every colour reference of a drawing's content with its JSON pointer: stroke colours and side fills, text run
/// colours (10 §4.11).
pub(super) fn drawing_colour_refs(d: &Drawing) -> Vec<(String, Id)> {
    fn nodes(ns: &[crate::entities::drawing::DrawingNode], at: &str, out: &mut Vec<(String, Id)>) {
        for (i, n) in ns.iter().enumerate() {
            let nat = format!("{at}/{i}");
            for (j, s) in n.strokes.iter().flatten().enumerate() {
                let sat = format!("{nat}/strokes/{j}");
                if let Some(c) = &s.colour_id {
                    out.push((format!("{sat}/colourId"), c.clone()));
                }
                for (side, fill) in [("left", &s.sides.left), ("right", &s.sides.right)] {
                    if let Some(c) = fill.as_ref().and_then(|f| f.colour_id.as_ref()) {
                        out.push((format!("{sat}/sides/{side}/colourId"), c.clone()));
                    }
                }
            }
            for (j, r) in n.runs.iter().flatten().enumerate() {
                if let Some(c) = &r.colour_id {
                    out.push((format!("{nat}/runs/{j}/colourId"), c.clone()));
                }
            }
            if let Some(children) = &n.children {
                nodes(children, &format!("{nat}/children"), out);
            }
        }
    }
    let mut out = Vec::new();
    nodes(&d.nodes, "/nodes", &mut out);
    out
}
