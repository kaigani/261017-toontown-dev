//! Canonical paths and reference resolution (10 §2.4, AG-03; 11 §5.1).

use serde_json::json;

use super::Document;
use crate::entities::ModelObject;
use crate::entities::drawing::DrawingNode;
use crate::entities::library::LibraryNode;
use crate::error::{ModelError, codes};
use crate::id::{Id, IdRef};
use crate::path::{LibraryKeyword, Path, Ref, Segment, SegmentType, Selector, SpecialRef, quote};
use crate::types::ObjectType;

/// What a reference resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An object, by type and index key (plain ID, or `<containerId>/<localId>`).
    Object {
        /// Object type.
        ty: ObjectType,
        /// Index key.
        key: String,
    },
    /// A cell of a cell-based element (not an object; 10 §4.7).
    Cell {
        /// The element.
        element: Id,
        /// The frame (1-based).
        frame: u32,
    },
}

/// Supplies the targets of special references (`@current`, `@selection`, …; 11 §5.1), which depend on UI state the
/// document does not hold. The unit type supplies none.
pub trait ResolveContext {
    /// The target of a special reference, if the context has one.
    fn special(&self, r: SpecialRef) -> Option<Target>;
}

impl ResolveContext for () {
    fn special(&self, _: SpecialRef) -> Option<Target> {
        None
    }
}

struct Cand {
    ty: ObjectType,
    key: String,
    name: Option<String>,
}

fn not_found(msg: impl Into<String>, r: &str) -> ModelError {
    ModelError::with_details(codes::REF_NOT_FOUND, msg, json!({ "ref": r }))
}

fn sel_name(name: Option<&str>, unique: bool, id: &str) -> String {
    match name {
        Some(n) if unique => quote(n),
        _ => format!("@{id}"),
    }
}

fn find_node<'a>(nodes: &'a [DrawingNode], id: &str) -> Option<(&'a DrawingNode, &'a [DrawingNode])> {
    for n in nodes {
        if n.id.as_str() == id {
            return Some((n, nodes));
        }
        if let Some(found) = n.children.as_deref().and_then(|c| find_node(c, id)) {
            return Some(found);
        }
    }
    None
}

fn find_library<'a>(children: &'a [LibraryNode], id: &str) -> Option<(&'a LibraryNode, &'a [LibraryNode])> {
    for c in children {
        let (cid, sub) = match c {
            LibraryNode::Folder(f) => (f.id.as_str(), Some(f.children.as_slice())),
            LibraryNode::Item(i) => (i.id.as_str(), None),
        };
        if cid == id {
            return Some((c, children));
        }
        if let Some(found) = sub.and_then(|s| find_library(s, id)) {
            return Some(found);
        }
    }
    None
}

fn library_name(n: &LibraryNode) -> &str {
    match n {
        LibraryNode::Folder(f) => &f.name,
        LibraryNode::Item(i) => &i.name,
    }
}

fn exact_count<'a, I: IntoIterator<Item = Option<&'a str>>>(names: I, name: &str) -> usize {
    names.into_iter().filter(|n| *n == Some(name)).count()
}

/// Splits an index key into (container, local).
fn split_key(key: &str) -> (Option<&str>, &str) {
    match key.split_once('/') {
        Some((c, l)) => (Some(c), l),
        None => (None, key),
    }
}

/// Memo of one canonical-path computation: parent paths and per-container name counts, so computing the paths of
/// many objects costs one pass instead of a sibling scan per object (C12-R1-23: 10 000 drawings in one bank).
#[derive(Default)]
struct PathMemo {
    paths: std::collections::HashMap<String, String>,
    counts: std::collections::HashMap<String, std::collections::HashMap<String, usize>>,
}

impl PathMemo {
    fn count(&mut self, container: &str, name: &str, names: impl FnOnce() -> Vec<String>) -> usize {
        let map = self.counts.entry(container.to_owned()).or_insert_with(|| {
            let mut m = std::collections::HashMap::new();
            for n in names() {
                *m.entry(n).or_insert(0) += 1;
            }
            m
        });
        map.get(name).copied().unwrap_or(0)
    }
}

impl Document {
    /// The canonical path of an object by index key (plain or compound ID).
    #[must_use]
    pub fn canonical_path(&self, key: &str) -> Option<String> {
        let loc = self.locate(key)?;
        Some(self.path_for(loc.ty, &loc.key, &mut PathMemo::default()))
    }

    /// The canonical paths of many objects, computed in one pass (shared parents and sibling-name counts).
    #[must_use]
    pub fn canonical_paths<'a>(&self, keys: impl IntoIterator<Item = &'a str>) -> Vec<Option<String>> {
        let mut memo = PathMemo::default();
        keys.into_iter().map(|k| self.locate(k).map(|loc| self.path_for(loc.ty, &loc.key, &mut memo))).collect()
    }

    pub(super) fn canonical_path_of(&self, ty: ObjectType, key: &str) -> String {
        self.path_for(ty, key, &mut PathMemo::default())
    }

    fn path_for(&self, ty: ObjectType, key: &str, memo: &mut PathMemo) -> String {
        if let Some(p) = memo.paths.get(key) {
            return p.clone();
        }
        let path = self.compute_path(ty, key, memo);
        memo.paths.insert(key.to_owned(), path.clone());
        path
    }

    #[allow(clippy::too_many_lines)]
    fn compute_path(&self, ty: ObjectType, key: &str, memo: &mut PathMemo) -> String {
        let fallback = || format!("{}:@{}", ty.as_str(), split_key(key).1);
        let loc = self.locate(key);
        let parent_key = loc.and_then(|l| l.parent.clone());
        let (_, local) = split_key(key);
        match ty {
            ObjectType::Project => "project".to_owned(),
            ObjectType::Palette => {
                let name = self.palettes.get(key).map(|p| p.name.as_str());
                let unique =
                    name.is_some_and(|n| exact_count(self.palettes.values().map(|p| Some(p.name.as_str())), n) == 1);
                format!("palette:{}", sel_name(name, unique, key))
            }
            ObjectType::PaletteStyle | ObjectType::Swatch => {
                let Some(pk) = parent_key.as_deref() else { return fallback() };
                let Some(pal) = self.palettes.get(pk) else { return fallback() };
                let names: Vec<Option<&str>> = if ty == ObjectType::Swatch {
                    pal.swatches.iter().map(|s| Some(s.name.as_str())).collect()
                } else {
                    pal.styles.iter().map(|s| Some(s.name.as_str())).collect()
                };
                let name = if ty == ObjectType::Swatch {
                    pal.swatches.iter().find(|s| s.colour_id.as_str() == key).map(|s| s.name.as_str())
                } else {
                    pal.styles.iter().find(|s| s.id.as_str() == key).map(|s| s.name.as_str())
                };
                let unique = name.is_some_and(|n| exact_count(names, n) == 1);
                format!(
                    "{}/{}:{}",
                    self.path_for(ObjectType::Palette, pk, memo),
                    ty.as_str(),
                    sel_name(name, unique, key)
                )
            }
            ObjectType::Pen | ObjectType::Asset => {
                let names: Vec<&str> = if ty == ObjectType::Pen {
                    self.project.pens.iter().map(|p| p.name.as_str()).collect()
                } else {
                    self.project.assets.iter().map(|a| a.name.as_str()).collect()
                };
                let name = if ty == ObjectType::Pen {
                    self.project.pens.iter().find(|p| p.id.as_str() == key).map(|p| p.name.as_str())
                } else {
                    self.project.assets.iter().find(|a| a.id.as_str() == key).map(|a| a.name.as_str())
                };
                let unique = name.is_some_and(|n| exact_count(names.iter().map(|s| Some(*s)), n) == 1);
                format!("{}:{}", ty.as_str(), sel_name(name, unique, key))
            }
            ObjectType::DrawingBank => format!("drawingBank:@{key}"),
            ObjectType::Drawing => {
                let Ok(id) = Id::parse(key) else { return fallback() };
                let Some(d) = self.drawings.get(&id) else { return fallback() };
                let bank = self.bank_of(&id).cloned();
                let base = match bank.as_ref().and_then(|b| self.bank_users(b).first()) {
                    Some(el) => self.path_for(ObjectType::Element, el.as_str(), memo),
                    None => match &bank {
                        Some(b) => format!("drawingBank:@{b}"),
                        None => return fallback(),
                    },
                };
                let unique = bank.as_ref().and_then(|b| self.banks.get(b)).is_some_and(|b| {
                    let names = || {
                        b.drawings.ids().iter().filter_map(|x| self.drawings.get(x)).map(|d| d.name.clone()).collect()
                    };
                    memo.count(&format!("bank:{}", b.id), &d.name, names) == 1
                });
                format!("{base}/drawing:{}", sel_name(Some(&d.name), unique, key))
            }
            ObjectType::DrawingNode | ObjectType::Stroke => {
                let (container, _) = split_key(key);
                let Some(drawing) = container.and_then(|c| self.drawings.get(c)) else { return fallback() };
                let Some(pk) = parent_key.as_deref() else { return fallback() };
                let parent = if pk.contains('/') {
                    self.path_for(ObjectType::DrawingNode, pk, memo)
                } else {
                    self.path_for(ObjectType::Drawing, pk, memo)
                };
                if ty == ObjectType::Stroke {
                    return format!("{parent}/stroke:@{local}");
                }
                let Some((node, siblings)) = find_node(&drawing.nodes, local) else { return fallback() };
                let unique = node
                    .name
                    .as_deref()
                    .is_some_and(|n| exact_count(siblings.iter().map(|s| s.name.as_deref()), n) == 1);
                format!("{parent}/drawingNode:{}", sel_name(node.name.as_deref(), unique, local))
            }
            ObjectType::Scene => {
                let name = self.scenes.get(key).map(|s| s.name.as_str());
                let unique =
                    name.is_some_and(|n| exact_count(self.scenes.values().map(|s| Some(s.name.as_str())), n) == 1);
                format!("scene:{}", sel_name(name, unique, key))
            }
            ObjectType::Transition => match parent_key.as_deref() {
                Some(p) if self.scenes.contains_key(p) => {
                    format!("{}/transition", self.path_for(ObjectType::Scene, p, memo))
                }
                _ => "transition".to_owned(),
            },
            ObjectType::Element => {
                let Ok(id) = Id::parse(key) else { return fallback() };
                let Some(e) = self.elements.get(&id) else { return fallback() };
                let Some(scene_id) = self.scene_of(&id).cloned() else { return fallback() };
                let unique = self.scenes.get(&scene_id).is_some_and(|s| {
                    let names = || {
                        s.elements.ids().iter().filter_map(|x| self.elements.get(x)).map(|e| e.name.clone()).collect()
                    };
                    memo.count(&format!("scene:{}", s.id), &e.name, names) == 1
                });
                format!(
                    "{}/element:{}",
                    self.path_for(ObjectType::Scene, scene_id.as_str(), memo),
                    sel_name(Some(&e.name), unique, key)
                )
            }
            ObjectType::Channel => {
                let Some(el) = parent_key.as_deref() else { return fallback() };
                let name = self
                    .elements
                    .get(el)
                    .and_then(|e| e.channels.as_ref())
                    .and_then(|m| m.values().find(|c| c.id.as_str() == key))
                    .map(|c| c.name.clone());
                match name {
                    Some(n) => format!("{}/channel:{}", self.path_for(ObjectType::Element, el, memo), quote(&n)),
                    None => fallback(),
                }
            }
            ObjectType::Key | ObjectType::MotionPoint => {
                let Some(ch) = parent_key.as_deref() else { return fallback() };
                let chan = self.channel(ch);
                let base = self.path_for(ObjectType::Channel, ch, memo);
                if ty == ObjectType::Key {
                    match chan.and_then(|c| c.keys.as_ref()).and_then(|ks| ks.iter().find(|k| k.id.as_str() == local)) {
                        Some(k) => format!("{base}/key:{}", quote(&k.frame.to_string())),
                        None => fallback(),
                    }
                } else {
                    format!("{base}/motionPoint:@{local}")
                }
            }
            ObjectType::ParticleBlock | ObjectType::SoundPlacement | ObjectType::MediaSequence => {
                match parent_key.as_deref().map(|p| self.path_for(ObjectType::Element, p, memo)) {
                    Some(p) => format!("{p}/{}:@{key}", ty.as_str()),
                    None => fallback(),
                }
            }
            ObjectType::Library => "library:local".to_owned(),
            ObjectType::LibraryFolder | ObjectType::LibraryItem => {
                let Some(pk) = parent_key.as_deref() else { return fallback() };
                let parent = if pk == self.library.id.as_str() {
                    "library:local".to_owned()
                } else {
                    self.path_for(ObjectType::LibraryFolder, pk, memo)
                };
                let Some((node, siblings)) = find_library(&self.library.children, key) else { return fallback() };
                let name = library_name(node);
                let lower = name.to_lowercase();
                let unique = siblings.iter().filter(|s| library_name(s).to_lowercase() == lower).count() == 1;
                format!("{parent}/{}:{}", ty.as_str(), sel_name(Some(name), unique, key))
            }
        }
    }

    fn channel(&self, key: &str) -> Option<&crate::entities::channel::Channel> {
        let loc = self.locate(key)?;
        let el = self.elements.get(loc.parent.as_deref()?)?;
        el.channels.as_ref()?.values().find(|c| c.id.as_str() == key)
    }

    /// Resolves a reference string (ID, compound ID or path) to a target (10 AG-03).
    ///
    /// # Errors
    /// `arg.invalid` for syntax errors; `ref.notFound`, `ref.ambiguous` (with `details.candidates`) or
    /// `ref.wrongType` as 10 §2.4 defines.
    pub fn resolve_str(&self, s: &str, ctx: &dyn ResolveContext) -> Result<Target, ModelError> {
        self.resolve(&Ref::parse(s)?, s, ctx)
    }

    /// Resolves a reference and checks its type (`ref.wrongType` otherwise).
    ///
    /// # Errors
    /// As [`Document::resolve_str`], plus `ref.wrongType`.
    pub fn resolve_as(&self, s: &str, expected: ObjectType, ctx: &dyn ResolveContext) -> Result<String, ModelError> {
        match self.resolve_str(s, ctx)? {
            Target::Object { ty, key } if ty == expected => Ok(key),
            Target::Object { ty, key } => Err(ModelError::with_details(
                codes::REF_WRONG_TYPE,
                format!("`{s}` is a {ty}, not a {expected}"),
                json!({ "ref": s, "id": key, "type": ty.as_str(), "expected": expected.as_str() }),
            )),
            Target::Cell { .. } => Err(ModelError::with_details(
                codes::REF_WRONG_TYPE,
                format!("`{s}` is a cell, not a {expected}"),
                json!({ "ref": s, "expected": expected.as_str() }),
            )),
        }
    }

    /// Resolves a parsed reference.
    ///
    /// # Errors
    /// As [`Document::resolve_str`].
    pub fn resolve(&self, r: &Ref, text: &str, ctx: &dyn ResolveContext) -> Result<Target, ModelError> {
        match r {
            Ref::Id(id) => self.resolve_id(id, text),
            Ref::Path(p) => self.resolve_path(p, text, ctx),
        }
    }

    fn resolve_id(&self, id: &IdRef, text: &str) -> Result<Target, ModelError> {
        let key = id.to_string();
        if id.container().is_none() {
            let cands = self.index.local_candidates(&key);
            if cands.len() > 1 && !self.index_has_plain(&key) {
                return Err(self.ambiguous(text, cands.iter().map(String::as_str)));
            }
        }
        match self.locate(&key) {
            Some(loc) => Ok(Target::Object { ty: loc.ty, key: loc.key.clone() }),
            None => Err(not_found(format!("no object `{key}`"), text)),
        }
    }

    fn index_has_plain(&self, key: &str) -> bool {
        self.locate(key).is_some_and(|l| l.key == key)
    }

    fn ambiguous<'a, I: Iterator<Item = &'a str>>(&self, text: &str, keys: I) -> ModelError {
        let candidates: Vec<serde_json::Value> =
            keys.map(|k| json!({ "id": k, "path": self.canonical_path(k) })).collect();
        ModelError::with_details(
            codes::REF_AMBIGUOUS,
            format!("`{text}` matches more than one object"),
            json!({ "ref": text, "candidates": candidates }),
        )
    }

    fn resolve_path(&self, p: &Path, text: &str, ctx: &dyn ResolveContext) -> Result<Target, ModelError> {
        let mut cur: Option<Target> = None; // None = project root
        if let Some(sp) = p.special() {
            cur = Some(ctx.special(sp).ok_or_else(|| {
                ModelError::with_details(
                    codes::REF_NOT_FOUND,
                    format!("`{}` needs an application context (current scene, selection, views)", sp.as_str()),
                    json!({ "ref": text, "reason": "noContext" }),
                )
            })?);
        }
        for seg in p.segments() {
            let next = self.step(cur.as_ref(), seg, text)?;
            cur = Some(next);
        }
        Ok(cur.unwrap_or(Target::Object { ty: ObjectType::Project, key: self.project.id.as_str().to_owned() }))
    }

    #[allow(clippy::too_many_lines)]
    fn step(&self, cur: Option<&Target>, seg: &Segment, text: &str) -> Result<Target, ModelError> {
        let (cur_ty, cur_key) = match cur {
            None => (ObjectType::Project, self.project.id.as_str().to_owned()),
            Some(Target::Object { ty, key }) => (*ty, key.clone()),
            Some(Target::Cell { .. }) => return Err(not_found("a cell has no children", text)),
        };
        // Singletons.
        match (seg.ty(), cur_ty) {
            (SegmentType::Object(ObjectType::Transition), ObjectType::Project)
                if seg.selector() == &Selector::Singleton =>
            {
                return Ok(Target::Object {
                    ty: ObjectType::Transition,
                    key: self.project.start_transition.id.as_str().to_owned(),
                });
            }
            (SegmentType::Object(ObjectType::Transition), ObjectType::Scene)
                if seg.selector() == &Selector::Singleton =>
            {
                let s = self.scenes.get(cur_key.as_str()).ok_or_else(|| not_found("scene not found", text))?;
                return Ok(Target::Object { ty: ObjectType::Transition, key: s.out_transition.id.as_str().to_owned() });
            }
            (SegmentType::Camera, ObjectType::Scene) => {
                let s = self.scenes.get(cur_key.as_str()).ok_or_else(|| not_found("scene not found", text))?;
                return match &s.current_camera_id {
                    Some(c) => Ok(Target::Object { ty: ObjectType::Element, key: c.as_str().to_owned() }),
                    None => Err(not_found(format!("scene `{}` has no camera", s.name), text)),
                };
            }
            (SegmentType::Cell, ObjectType::Element) => {
                let Selector::Index(frame) = seg.selector() else {
                    return Err(not_found("cells are selected by frame", text));
                };
                let e = self.elements.get(cur_key.as_str()).ok_or_else(|| not_found("element not found", text))?;
                if !e.element_type.is_cell_based() {
                    return Err(ModelError::with_details(
                        codes::REF_WRONG_TYPE,
                        format!("element `{}` has no cells", e.name),
                        json!({"ref": text}),
                    ));
                }
                return Ok(Target::Cell { element: e.id.clone(), frame: *frame });
            }
            (SegmentType::Object(ObjectType::Library), ObjectType::Project) => {
                return match seg.selector() {
                    Selector::Keyword(LibraryKeyword::Local) => {
                        Ok(Target::Object { ty: ObjectType::Library, key: self.library.id.as_str().to_owned() })
                    }
                    Selector::Keyword(LibraryKeyword::Global) => Err(ModelError::with_details(
                        codes::REF_NOT_FOUND,
                        "the global library is not part of the document; use the library commands (08)",
                        json!({"ref": text, "reason": "notInDocument"}),
                    )),
                    _ => self.pick(
                        ObjectType::Library,
                        vec![Cand {
                            ty: ObjectType::Library,
                            key: self.library.id.as_str().to_owned(),
                            name: Some(self.library.name.clone()),
                        }],
                        seg,
                        text,
                        false,
                    ),
                };
            }
            _ => {}
        }
        let SegmentType::Object(want) = seg.ty() else {
            return Err(not_found(format!("`{}` is not addressable below a {cur_ty}", seg.ty().as_str()), text));
        };
        let mut case_insensitive_unique = false;
        let cands: Vec<Cand> = match (cur_ty, want) {
            (ObjectType::Project, ObjectType::Palette) => self.palettes().map(cand).collect(),
            (ObjectType::Project, ObjectType::Pen) => self.project.pens.iter().map(cand).collect(),
            (ObjectType::Project, ObjectType::Asset) => self.project.assets.iter().map(cand).collect(),
            (ObjectType::Project, ObjectType::DrawingBank) => self.banks().map(cand).collect(),
            (ObjectType::Project, ObjectType::Scene) => self.scenes().map(cand).collect(),
            (ObjectType::Project, other) => {
                // `type:@id` from the root: any object of that type.
                if let Selector::Id(id) = seg.selector() {
                    return self.by_id_typed(id.as_str(), other, text);
                }
                return Err(not_found(
                    format!("`{other}` objects are not selected by name from the project root"),
                    text,
                ));
            }
            (ObjectType::Palette, ObjectType::PaletteStyle | ObjectType::Swatch) => {
                let pal = self.palettes.get(cur_key.as_str()).ok_or_else(|| not_found("palette not found", text))?;
                if want == ObjectType::Swatch {
                    pal.swatches.iter().map(cand).collect()
                } else {
                    pal.styles.iter().map(cand).collect()
                }
            }
            (ObjectType::DrawingBank, ObjectType::Drawing) => self.bank_drawings(&cur_key),
            (ObjectType::Element, ObjectType::Drawing) => {
                let e = self.elements.get(cur_key.as_str()).ok_or_else(|| not_found("element not found", text))?;
                match &e.drawing_bank_id {
                    Some(b) => self.bank_drawings(b.as_str()),
                    None => return Err(not_found(format!("element `{}` has no drawings", e.name), text)),
                }
            }
            (ObjectType::Drawing, ObjectType::DrawingNode) => {
                let d = self.drawings.get(cur_key.as_str()).ok_or_else(|| not_found("drawing not found", text))?;
                if let Selector::Id(id) = seg.selector() {
                    return self.by_id_typed(&format!("{}/{id}", d.id), ObjectType::DrawingNode, text);
                }
                d.nodes.iter().map(|n| local_cand(&d.id, n)).collect()
            }
            (ObjectType::DrawingNode, ObjectType::DrawingNode | ObjectType::Stroke) => {
                let (container, local) = split_key(&cur_key);
                let container = container.unwrap_or_default().to_owned();
                let d = self.drawings.get(container.as_str()).ok_or_else(|| not_found("drawing not found", text))?;
                let (node, _) = find_node(&d.nodes, local).ok_or_else(|| not_found("node not found", text))?;
                if want == ObjectType::Stroke {
                    node.strokes
                        .iter()
                        .flatten()
                        .map(|s| Cand { ty: ObjectType::Stroke, key: format!("{container}/{}", s.id), name: None })
                        .collect()
                } else {
                    node.children.iter().flatten().map(|n| local_cand(&d.id, n)).collect()
                }
            }
            (ObjectType::Scene, ObjectType::Element) => {
                let s = self.scenes.get(cur_key.as_str()).ok_or_else(|| not_found("scene not found", text))?;
                self.tree_order(s).iter().filter_map(|e| self.elements.get(e)).map(|e| cand(&**e)).collect()
            }
            (ObjectType::Element, ObjectType::Channel) => {
                let e = self.elements.get(cur_key.as_str()).ok_or_else(|| not_found("element not found", text))?;
                e.channels.iter().flat_map(|m| m.values()).map(cand).collect()
            }
            (
                ObjectType::Element,
                ObjectType::ParticleBlock | ObjectType::SoundPlacement | ObjectType::MediaSequence,
            ) => {
                let e = self.elements.get(cur_key.as_str()).ok_or_else(|| not_found("element not found", text))?;
                match want {
                    ObjectType::ParticleBlock => e.blocks.iter().flatten().map(cand).collect(),
                    ObjectType::SoundPlacement => e.placements.iter().flatten().map(cand).collect(),
                    _ => e.sequences.iter().flatten().map(cand).collect(),
                }
            }
            (ObjectType::Channel, ObjectType::Key | ObjectType::MotionPoint) => {
                let ch = self.channel(&cur_key).ok_or_else(|| not_found("channel not found", text))?;
                if want == ObjectType::Key && ch.points.is_some() {
                    // On the motion path, `key:"n"` selects the keyframe motion point at frame n.
                    ch.points
                        .iter()
                        .flatten()
                        .map(|p| Cand {
                            ty: ObjectType::MotionPoint,
                            key: format!("{}/{}", ch.id, p.id),
                            name: p.frame.map(|f| f.to_string()),
                        })
                        .collect()
                } else if want == ObjectType::Key {
                    ch.keys
                        .iter()
                        .flatten()
                        .map(|k| Cand {
                            ty: ObjectType::Key,
                            key: format!("{}/{}", ch.id, k.id),
                            name: Some(k.frame.to_string()),
                        })
                        .collect()
                } else {
                    ch.points
                        .iter()
                        .flatten()
                        .map(|p| Cand { ty: ObjectType::MotionPoint, key: format!("{}/{}", ch.id, p.id), name: None })
                        .collect()
                }
            }
            (ObjectType::Library | ObjectType::LibraryFolder, ObjectType::LibraryFolder | ObjectType::LibraryItem) => {
                case_insensitive_unique = true;
                let children = if cur_ty == ObjectType::Library {
                    self.library.children.as_slice()
                } else {
                    match find_library(&self.library.children, &cur_key) {
                        Some((LibraryNode::Folder(f), _)) => f.children.as_slice(),
                        _ => return Err(not_found("folder not found", text)),
                    }
                };
                children
                    .iter()
                    .filter_map(|c| match c {
                        LibraryNode::Folder(f) if want == ObjectType::LibraryFolder => Some(cand(f)),
                        LibraryNode::Item(i) if want == ObjectType::LibraryItem => Some(cand(i)),
                        _ => None,
                    })
                    .collect()
            }
            (parent, child) => {
                return Err(not_found(format!("a {parent} has no `{child}` children"), text));
            }
        };
        self.pick(want, cands, seg, text, case_insensitive_unique)
    }

    fn bank_drawings(&self, bank: &str) -> Vec<Cand> {
        let mut out: Vec<Cand> = self
            .banks
            .get(bank)
            .map(|b| b.drawings.ids().iter().filter_map(|d| self.drawings.get(d)).map(|d| cand(&**d)).collect())
            .unwrap_or_default();
        // Natural name order (10 §4.8) gives `drawing:#n` a stable meaning.
        out.sort_by(|a, b| {
            natural_cmp(a.name.as_deref().unwrap_or(""), b.name.as_deref().unwrap_or(""))
                .then_with(|| a.key.cmp(&b.key))
        });
        out
    }

    fn by_id_typed(&self, key: &str, want: ObjectType, text: &str) -> Result<Target, ModelError> {
        let t = self.resolve_id(&IdRef::parse(key)?, text)?;
        match &t {
            Target::Object { ty, .. } if *ty == want => Ok(t),
            Target::Object { ty, key } => Err(ModelError::with_details(
                codes::REF_WRONG_TYPE,
                format!("`{key}` is a {ty}, not a {want}"),
                json!({ "ref": text, "id": key, "type": ty.as_str(), "expected": want.as_str() }),
            )),
            Target::Cell { .. } => Err(not_found("not an object", text)),
        }
    }

    fn pick(
        &self,
        want: ObjectType,
        cands: Vec<Cand>,
        seg: &Segment,
        text: &str,
        ci_names: bool,
    ) -> Result<Target, ModelError> {
        let found: Vec<&Cand> = match seg.selector() {
            Selector::Index(n) => cands.get((*n as usize).saturating_sub(1)).into_iter().collect(),
            Selector::Id(id) => {
                let hits: Vec<&Cand> = cands.iter().filter(|c| split_key(&c.key).1 == id.as_str()).collect();
                if hits.is_empty() {
                    // The ID may exist elsewhere: wrong place or wrong type.
                    if let Ok(Target::Object { ty, key }) = self.resolve_id(&IdRef::plain(id.clone()), text)
                        && ty != want
                        && want != ObjectType::Key
                    {
                        return Err(ModelError::with_details(
                            codes::REF_WRONG_TYPE,
                            format!("`{key}` is a {ty}, not a {want}"),
                            json!({ "ref": text, "id": key, "type": ty.as_str(), "expected": want.as_str() }),
                        ));
                    }
                }
                hits
            }
            Selector::Name(name) => {
                let exact: Vec<&Cand> = cands.iter().filter(|c| c.name.as_deref() == Some(name.as_str())).collect();
                if !exact.is_empty() && !ci_names {
                    exact
                } else {
                    let lower = name.to_lowercase();
                    cands.iter().filter(|c| c.name.as_deref().is_some_and(|n| n.to_lowercase() == lower)).collect()
                }
            }
            Selector::Keyword(_) | Selector::Singleton => Vec::new(),
        };
        match found.as_slice() {
            [one] => Ok(Target::Object { ty: one.ty, key: one.key.clone() }),
            [] => Err(not_found(format!("`{text}`: no {want} matches `{seg}`"), text)),
            many => Err(self.ambiguous(text, many.iter().map(|c| c.key.as_str()))),
        }
    }
}

fn cand<T: ModelObject>(o: &T) -> Cand {
    Cand { ty: T::TYPE, key: o.object_id().as_str().to_owned(), name: o.object_name().map(str::to_owned) }
}

fn local_cand(drawing: &Id, n: &DrawingNode) -> Cand {
    Cand { ty: ObjectType::DrawingNode, key: format!("{drawing}/{}", n.id), name: n.name.clone() }
}

/// Natural order of names: digit runs compare numerically ("2" < "10").
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na: String = std::iter::from_fn(|| ai.next_if(char::is_ascii_digit)).collect();
                let nb: String = std::iter::from_fn(|| bi.next_if(char::is_ascii_digit)).collect();
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)).then_with(|| na.len().cmp(&nb.len()));
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase()).then_with(|| x.cmp(&y));
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}
