//! The document: an immutable, cheaply clonable snapshot of one project (10 §3).
//!
//! Objects that the package stores in their own entries (project, palettes, drawing banks, drawings, scenes,
//! elements, the local library; 10 LAY-01) are held behind `Arc`s keyed by ID, so cloning a document copies only
//! pointers and two snapshots share every unchanged entry (copy-on-write; 10 D15, 11 §16.5). Objects that live
//! inline in an entry (styles, swatches, pens, assets, transitions, nodes, strokes, channels, keys, motion points,
//! particle blocks, placements, sequences, library folders and items) are found through an index built when the
//! snapshot is assembled.
//!
//! A `Document` has **no mutating API**. It is built from validated parts ([`Document::from_parts`]); the executor
//! (C1.2) derives each next snapshot from the previous one inside a transaction and is the only component that holds
//! the open project's current snapshot.

mod find;
mod index;
mod paths;
mod render;
mod stamp;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde_json::json;

use crate::entities::drawing::{Drawing, DrawingBank};
use crate::entities::element::{Element, ElementType, ParamSlot};
use crate::entities::library::Library;
use crate::entities::palette::Palette;
use crate::entities::project::Project;
use crate::entities::scene::{Scene, TreeNode};
use crate::error::{ModelError, codes};
use crate::id::Id;
use crate::validate::{Check, Issues};

pub use index::{EntryKey, Location};
pub use paths::{ResolveContext, Target};
pub use render::{RenderOptions, Slot as SlotKind, slots as slots_of};

/// Maximum scene length in frames (10 §5).
pub const MAX_SCENE_LENGTH: u64 = 100_000;

/// Severity of a document issue (10 §10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Informational.
    Info,
    /// Warning: the document is usable.
    Warning,
    /// Error: the document violates an integrity rule.
    Error,
}

/// A problem found in a document (the core of 10 §10.4 `Issue`; `issueId` and `fixes` are added by `doc.validate`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Stable code (10 §9).
    pub code: &'static str,
    /// Severity.
    pub severity: Severity,
    /// The object the issue is about.
    pub object_id: Option<String>,
    /// JSON Pointer into that object's JSON.
    pub pointer: String,
    /// Human-readable message.
    pub message: String,
}

impl Issue {
    fn error(code: &'static str, object: &Id, pointer: &str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            object_id: Some(object.as_str().to_owned()),
            pointer: pointer.to_owned(),
            message: message.into(),
        }
    }

    fn warning(code: &'static str, object: &Id, pointer: &str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            object_id: Some(object.as_str().to_owned()),
            pointer: pointer.to_owned(),
            message: message.into(),
        }
    }

    /// The issue as a model error.
    #[must_use]
    pub fn to_error(&self) -> ModelError {
        ModelError::with_details(
            self.code,
            self.message.clone(),
            json!({ "objectId": self.object_id, "pointer": self.pointer, "severity": self.severity }),
        )
    }
}

/// The entry objects of a document, before assembly.
#[derive(Debug, Clone)]
pub struct DocumentParts {
    /// `documentRev` of the snapshot.
    pub document_rev: u64,
    /// The project entry.
    pub project: Project,
    /// Palette entries.
    pub palettes: Vec<Palette>,
    /// Drawing-bank entries.
    pub banks: Vec<DrawingBank>,
    /// Drawing entries.
    pub drawings: Vec<Drawing>,
    /// Scene entries.
    pub scenes: Vec<Scene>,
    /// Element entries.
    pub elements: Vec<Element>,
    /// The local library entry.
    pub library: Library,
}

/// The undo and redo patches of one entry changed by [`Document::with_patched_entries`].
#[derive(Debug, Clone, PartialEq)]
pub struct EntryCommit {
    /// The entry.
    pub key: EntryKey,
    /// Turns the committed entry back into the pre-state (canonical, verified).
    pub undo: Vec<crate::change::PatchOp>,
    /// Turns the pre-state into the committed entry (canonical, verified).
    pub redo: Vec<crate::change::PatchOp>,
}

/// A patch from `from` to `to`, verified to reproduce `to` exactly; a whole-entry `replace` otherwise.
fn verified_diff(
    from: &serde_json::Value,
    to: &serde_json::Value,
    ty: crate::types::ObjectType,
) -> Vec<crate::change::PatchOp> {
    let patch = crate::change::diff_json(from, to, "", ty);
    if crate::change::apply(from, &patch).ok().as_ref() == Some(to) {
        return patch;
    }
    vec![crate::change::PatchOp {
        op: "replace",
        from: None,
        path: String::new(),
        range: None,
        value: Some(to.clone()),
        old_value: Some(from.clone()),
        elided: false,
    }]
}

/// An entry's JSON after a patch, where the entry may be created (`add ""` while absent) or removed (`remove ""`) by
/// the patch; every other operation applies through `change::apply_with_inverse` (frame limits re-checked).
fn entry_after(
    before: Option<&serde_json::Value>,
    patch: &[crate::change::PatchOp],
    key: &EntryKey,
) -> Result<Option<serde_json::Value>, Vec<ModelError>> {
    let mut cur = before.cloned();
    let mut rest: Vec<crate::change::PatchOp> = Vec::new();
    let flush =
        |cur: &mut Option<serde_json::Value>, rest: &mut Vec<crate::change::PatchOp>| -> Result<(), Vec<ModelError>> {
            if rest.is_empty() {
                return Ok(());
            }
            let Some(v) = cur.as_ref() else {
                return Err(vec![ModelError::with_details(
                    codes::ARG_INVALID,
                    format!("the entry {key:?} does not exist"),
                    json!({ "pointer": rest[0].path }),
                )]);
            };
            let (after, _) = crate::change::apply_with_inverse(v, rest).map_err(|e| vec![e])?;
            *cur = Some(after);
            rest.clear();
            Ok(())
        };
    for op in patch {
        match (op.op, op.path.is_empty(), cur.is_some() || !rest.is_empty()) {
            ("add", true, false) => cur = op.value.clone(),
            ("remove", true, true) => {
                flush(&mut cur, &mut rest)?;
                cur = None;
            }
            _ => rest.push(op.clone()),
        }
    }
    flush(&mut cur, &mut rest)?;
    Ok(cur)
}

/// An immutable snapshot of a project.
#[derive(Debug, Clone)]
pub struct Document {
    document_rev: u64,
    project: Arc<Project>,
    palettes: BTreeMap<Id, Arc<Palette>>,
    banks: BTreeMap<Id, Arc<DrawingBank>>,
    drawings: BTreeMap<Id, Arc<Drawing>>,
    scenes: BTreeMap<Id, Arc<Scene>>,
    elements: BTreeMap<Id, Arc<Element>>,
    library: Arc<Library>,
    index: Arc<index::Index>,
    /// Scene of each element.
    element_scene: Arc<HashMap<Id, Id>>,
    /// Elements using each bank, in canonical order.
    bank_users: Arc<HashMap<Id, Vec<Id>>>,
    /// Bank of each drawing.
    drawing_bank: Arc<HashMap<Id, Id>>,
    /// Problems found while assembling the parts (entries given twice; C11-R1-01).
    assembly: Arc<Vec<Issue>>,
    /// Entries known to satisfy every object-local rule: entry → address of the validated `Arc`. Entries are
    /// immutable, so a validated entry stays valid; `issues()` skips its object-local re-check, and the result is
    /// identical to checking it (C1.2 touched-entry validation).
    validated: Arc<HashMap<EntryKey, usize>>,
}

impl Document {
    /// Assembles and checks a document: the validated constructor. Parts are typed values that may come from the
    /// public `Deserialize` impls without having passed [`crate::object::from_json`], so every entry first runs
    /// through that input path again ([`crate::object::revalidate`]: depth limit, normalisation, schema, canonical
    /// form, object-local rules; C11-R1-02). Then the document-level integrity rules of 10 §5 and §7.2 apply, and an
    /// entry given twice is an error, never a silent overwrite (C11-R1-01).
    ///
    /// # Errors
    /// Every error found (object input errors with their codes and `details.objectId`, integrity rules with the
    /// codes of 10 §9). Warnings (e.g. unresolved colour IDs) do not prevent assembly; read them with
    /// [`Document::issues`].
    pub fn from_parts(parts: DocumentParts) -> Result<Self, Vec<ModelError>> {
        let parts = Self::revalidate_parts(parts)?;
        let mut doc = Self::assemble(parts);
        // Every entry just passed the full object input path.
        for key in doc.entry_keys() {
            doc.mark_validated(&key);
        }
        let errors: Vec<ModelError> =
            doc.issues().into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_error()).collect();
        if errors.is_empty() { Ok(doc) } else { Err(errors) }
    }

    fn revalidate_parts(parts: DocumentParts) -> Result<DocumentParts, Vec<ModelError>> {
        fn one<T: crate::entities::ModelObject + Check>(o: &T, errors: &mut Vec<ModelError>) -> Option<T> {
            match crate::object::revalidate(o) {
                Ok(v) => Some(v),
                Err(es) => {
                    for e in es {
                        let mut details = e.details().clone();
                        if let Some(m) = details.as_object_mut() {
                            m.insert("objectId".into(), json!(o.object_id().as_str()));
                        }
                        errors.push(ModelError::with_details(e.code(), e.message().to_owned(), details));
                    }
                    None
                }
            }
        }
        fn many<T: crate::entities::ModelObject + Check>(v: &[T], errors: &mut Vec<ModelError>) -> Vec<T> {
            v.iter().filter_map(|o| one(o, errors)).collect()
        }
        let mut errors = Vec::new();
        let out = DocumentParts {
            document_rev: parts.document_rev,
            project: one(&parts.project, &mut errors).unwrap_or(parts.project.clone()),
            palettes: many(&parts.palettes, &mut errors),
            banks: many(&parts.banks, &mut errors),
            drawings: many(&parts.drawings, &mut errors),
            scenes: many(&parts.scenes, &mut errors),
            elements: many(&parts.elements, &mut errors),
            library: one(&parts.library, &mut errors).unwrap_or(parts.library.clone()),
        };
        if errors.is_empty() { Ok(out) } else { Err(errors) }
    }

    /// Assembles a document without rejecting integrity errors (the load path reports and repairs them, 10 §7.2).
    /// An entry ID given twice keeps the first entry and reports `id.duplicate` with the dropped entry's name.
    #[must_use]
    pub fn assemble(parts: DocumentParts) -> Self {
        fn collect<T>(
            items: Vec<T>,
            id: impl Fn(&T) -> &Id,
            name: impl Fn(&T) -> &str,
            what: &str,
            issues: &mut Vec<Issue>,
        ) -> BTreeMap<Id, Arc<T>> {
            let mut out = BTreeMap::new();
            for item in items {
                let key = id(&item).clone();
                if out.contains_key(&key) {
                    issues.push(Issue::error(
                        codes::ID_DUPLICATE,
                        &key,
                        "",
                        format!("{what} `{key}` is given more than once (dropped duplicate named `{}`)", name(&item)),
                    ));
                    continue;
                }
                out.insert(key, Arc::new(item));
            }
            out
        }
        let mut assembly = Vec::new();
        let palettes = collect(parts.palettes, |p| &p.id, |p| &p.name, "palette", &mut assembly);
        let banks = collect(parts.banks, |b| &b.id, |_| "", "drawing bank", &mut assembly);
        let drawings = collect(parts.drawings, |d| &d.id, |d| &d.name, "drawing", &mut assembly);
        let scenes = collect(parts.scenes, |s| &s.id, |s| &s.name, "scene", &mut assembly);
        let elements = collect(parts.elements, |e| &e.id, |e| &e.name, "element", &mut assembly);
        let mut doc = Self {
            document_rev: parts.document_rev,
            project: Arc::new(parts.project),
            palettes,
            banks,
            drawings,
            scenes,
            elements,
            library: Arc::new(parts.library),
            index: Arc::new(index::Index::default()),
            element_scene: Arc::new(HashMap::new()),
            bank_users: Arc::new(HashMap::new()),
            drawing_bank: Arc::new(HashMap::new()),
            assembly: Arc::new(assembly),
            validated: Arc::new(HashMap::new()),
        };
        doc.rebuild_derived();
        doc
    }

    /// The address of an entry's `Arc` (identity of the immutable entry value).
    fn entry_ptr(&self, key: &EntryKey) -> Option<usize> {
        fn ptr<T>(a: &Arc<T>) -> usize {
            Arc::as_ptr(a).cast::<()>() as usize
        }
        match key {
            EntryKey::Project => Some(ptr(&self.project)),
            EntryKey::Library => Some(ptr(&self.library)),
            EntryKey::Palette(id) => self.palettes.get(id).map(ptr),
            EntryKey::Bank(id) => self.banks.get(id).map(ptr),
            EntryKey::Drawing(id) => self.drawings.get(id).map(ptr),
            EntryKey::Scene(id) => self.scenes.get(id).map(ptr),
            EntryKey::Element(id) => self.elements.get(id).map(ptr),
        }
    }

    /// Records that the entry's current value passed the full object input path.
    fn mark_validated(&mut self, key: &EntryKey) {
        if let Some(p) = self.entry_ptr(key) {
            Arc::make_mut(&mut self.validated).insert(key.clone(), p);
        }
    }

    /// Carries the validation of `from`'s entry over to this snapshot's entry of the same key (stamping changes only
    /// `rev` values, which no object-local rule reads).
    pub(super) fn carry_validation(&mut self, from: &Document, key: &EntryKey) {
        if from.is_validated(key) {
            self.mark_validated(key);
        }
    }

    fn is_validated(&self, key: &EntryKey) -> bool {
        self.entry_ptr(key).is_some_and(|p| self.validated.get(key) == Some(&p))
    }

    fn rebuild_derived(&mut self) {
        let mut element_scene = HashMap::new();
        let mut bank_users: HashMap<Id, Vec<Id>> = HashMap::new();
        for scene_id in self.project.scenes.ids() {
            if let Some(scene) = self.scenes.get(scene_id) {
                for el in self.tree_order(scene) {
                    element_scene.insert(el.clone(), scene_id.clone());
                    if let Some(bank) = self.elements.get(&el).and_then(|e| e.drawing_bank_id.clone()) {
                        bank_users.entry(bank).or_default().push(el.clone());
                    }
                }
                for el in scene.elements.ids() {
                    element_scene.entry(el.clone()).or_insert_with(|| scene_id.clone());
                }
            }
        }
        let mut drawing_bank = HashMap::new();
        for b in self.banks.values() {
            for d in b.drawings.ids() {
                drawing_bank.entry(d.clone()).or_insert_with(|| b.id.clone());
            }
        }
        self.element_scene = Arc::new(element_scene);
        self.bank_users = Arc::new(bank_users);
        self.drawing_bank = Arc::new(drawing_bank);
        self.index = Arc::new(index::Index::build(self));
    }

    /// `documentRev` of this snapshot (10 REV-01).
    #[must_use]
    pub fn document_rev(&self) -> u64 {
        self.document_rev
    }

    /// The project.
    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// A palette by ID.
    #[must_use]
    pub fn palette(&self, id: &Id) -> Option<&Palette> {
        self.palettes.get(id).map(AsRef::as_ref)
    }

    /// Palettes in project order.
    pub fn palettes(&self) -> impl Iterator<Item = &Palette> {
        self.project.palettes.ids().iter().filter_map(|id| self.palette(id))
    }

    /// A drawing bank by ID.
    #[must_use]
    pub fn bank(&self, id: &Id) -> Option<&DrawingBank> {
        self.banks.get(id).map(AsRef::as_ref)
    }

    /// Drawing banks in ID order.
    pub fn banks(&self) -> impl Iterator<Item = &DrawingBank> {
        self.banks.values().map(AsRef::as_ref)
    }

    /// A drawing by ID.
    #[must_use]
    pub fn drawing(&self, id: &Id) -> Option<&Drawing> {
        self.drawings.get(id).map(AsRef::as_ref)
    }

    /// Drawings in ID order.
    pub fn drawings(&self) -> impl Iterator<Item = &Drawing> {
        self.drawings.values().map(AsRef::as_ref)
    }

    /// A scene by ID.
    #[must_use]
    pub fn scene(&self, id: &Id) -> Option<&Scene> {
        self.scenes.get(id).map(AsRef::as_ref)
    }

    /// Scenes in storyline order.
    pub fn scenes(&self) -> impl Iterator<Item = &Scene> {
        self.project.scenes.ids().iter().filter_map(|id| self.scene(id))
    }

    /// An element by ID.
    #[must_use]
    pub fn element(&self, id: &Id) -> Option<&Element> {
        self.elements.get(id).map(AsRef::as_ref)
    }

    /// Elements in ID order.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.elements.values().map(AsRef::as_ref)
    }

    /// The scene an element belongs to.
    #[must_use]
    pub fn scene_of(&self, element: &Id) -> Option<&Id> {
        self.element_scene.get(element)
    }

    /// The elements using a bank (its clone group), in canonical order.
    #[must_use]
    pub fn bank_users(&self, bank: &Id) -> &[Id] {
        self.bank_users.get(bank).map_or(&[], Vec::as_slice)
    }

    /// The bank holding a drawing.
    #[must_use]
    pub fn bank_of(&self, drawing: &Id) -> Option<&Id> {
        self.drawing_bank.get(drawing)
    }

    /// Number of indexed objects.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.index.len()
    }

    /// The local library.
    #[must_use]
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Whether two snapshots share the same entry object (pointer equality; used to find changed entries).
    #[must_use]
    pub fn same_entry(&self, other: &Document, key: &EntryKey) -> bool {
        match key {
            EntryKey::Project => Arc::ptr_eq(&self.project, &other.project),
            EntryKey::Library => Arc::ptr_eq(&self.library, &other.library),
            EntryKey::Palette(id) => ptr_eq_opt(self.palettes.get(id), other.palettes.get(id)),
            EntryKey::Bank(id) => ptr_eq_opt(self.banks.get(id), other.banks.get(id)),
            EntryKey::Drawing(id) => ptr_eq_opt(self.drawings.get(id), other.drawings.get(id)),
            EntryKey::Scene(id) => ptr_eq_opt(self.scenes.get(id), other.scenes.get(id)),
            EntryKey::Element(id) => ptr_eq_opt(self.elements.get(id), other.elements.get(id)),
        }
    }

    /// Every entry key of the document, in a deterministic order (project, palettes, banks, drawings, scenes,
    /// elements, library).
    #[must_use]
    pub fn entry_keys(&self) -> Vec<EntryKey> {
        let mut keys = vec![EntryKey::Project];
        keys.extend(self.palettes.keys().cloned().map(EntryKey::Palette));
        keys.extend(self.banks.keys().cloned().map(EntryKey::Bank));
        keys.extend(self.drawings.keys().cloned().map(EntryKey::Drawing));
        keys.extend(self.scenes.keys().cloned().map(EntryKey::Scene));
        keys.extend(self.elements.keys().cloned().map(EntryKey::Element));
        keys.push(EntryKey::Library);
        keys
    }

    /// The JSON of an entry object exactly as stored in memory (split children as `{id, type}` stubs).
    ///
    /// # Errors
    /// `ref.notFound` for a missing entry; `internal` if serialisation fails.
    pub fn entry_raw_json(&self, key: &EntryKey) -> Result<serde_json::Value, ModelError> {
        self.entry_raw(key)
    }

    /// Applies a patch to one entry as a **staged commit** (10 §2.6, §5; C11-R5-01) and returns the next snapshot
    /// with the patch's inverse: [`Document::with_patched_entries`] for a single entry.
    ///
    /// # Errors
    /// As [`Document::with_patched_entries`].
    pub fn with_patched_entry(
        &self,
        key: &EntryKey,
        patch: &[crate::change::PatchOp],
    ) -> Result<(Document, Vec<crate::change::PatchOp>), Vec<ModelError>> {
        let (next, commits) = self.with_patched_entries(&[(key.clone(), patch.to_vec())])?;
        let undo = commits.into_iter().next().map(|c| c.undo).unwrap_or_default();
        Ok((next, undo))
    }

    /// Applies patches to several entries as **one staged commit** (10 §2.6, §5; C11-R5-01; atomic across entries).
    ///
    /// Each entry's patches are applied in order to a copy of the entry's JSON (`change::apply_with_inverse`, which
    /// checks every frame-bearing field an operation touched). Each staged entry then goes through the full input path
    /// again (`object::from_json`: depth limit, normalisation, schema, canonical form, object-local rules, including
    /// the exposure and scene limits) and its ID must be unchanged (ID-01). The staged entries replace the old ones
    /// in a copy-on-write clone, and the clone's complete `issues()` — every integrity rule and every 10 §5 limit —
    /// must have no error; it is checked once, on the final state, so a change spanning entries is validated as a
    /// whole. Only then is the staged snapshot returned; `self` is immutable and is never changed.
    ///
    /// For each entry the result carries the canonical **undo** and **redo** patches (C11-R6-01): the diff from the
    /// committed canonical entry back to the canonical pre-state, and from the pre-state to the committed entry, each
    /// verified to reproduce its target exactly (a whole-entry `replace` is the never-expected safety net).
    ///
    /// # Errors
    /// The first failing stage's errors: a patch's typed error (e.g. `doc.limitExceeded`, `arg.invalid`,
    /// `doc.patchTestFailed`, with `details.pointer`), an entry's input errors (`doc.schemaViolation`,
    /// `arg.invalid`, `doc.limitExceeded`; `details.objectId`), an attempt to change an entry's ID (`arg.invalid`,
    /// ID-01), or the integrity errors of the staged document (codes of 10 §9 with `details.objectId` and
    /// `details.pointer`).
    pub fn with_patched_entries(
        &self,
        changes: &[(EntryKey, Vec<crate::change::PatchOp>)],
    ) -> Result<(Document, Vec<EntryCommit>), Vec<ModelError>> {
        self.staged_commit(changes, true)
    }

    /// [`Document::with_patched_entries`] without deriving the redo patches: each [`EntryCommit::redo`] is empty.
    /// For callers that obtain the redo when they undo (the executor: replaying a step's verified undo returns the
    /// verified patch back, which becomes the step's redo; C1.1 ticket "use the redo returned by undo"). Saves one
    /// verified whole-entry diff per commit (≈ 0.45 s for a 100 000-stroke drawing).
    ///
    /// # Errors
    /// As [`Document::with_patched_entries`].
    pub fn with_patched_entries_undo_only(
        &self,
        changes: &[(EntryKey, Vec<crate::change::PatchOp>)],
    ) -> Result<(Document, Vec<EntryCommit>), Vec<ModelError>> {
        self.staged_commit(changes, false)
    }

    fn staged_commit(
        &self,
        changes: &[(EntryKey, Vec<crate::change::PatchOp>)],
        with_redo: bool,
    ) -> Result<(Document, Vec<EntryCommit>), Vec<ModelError>> {
        fn staged<T: crate::entities::ModelObject + Check>(
            v: &serde_json::Value,
            id: &Id,
        ) -> Result<T, Vec<ModelError>> {
            let parsed: T = crate::object::from_json(v).map_err(|errors| {
                errors
                    .into_iter()
                    .map(|e| {
                        let mut details = e.details().clone();
                        if let Some(m) = details.as_object_mut() {
                            m.insert("objectId".into(), json!(id.as_str()));
                        }
                        ModelError::with_details(e.code(), e.message().to_owned(), details)
                    })
                    .collect::<Vec<_>>()
            })?;
            if parsed.object_id() != id {
                return Err(vec![ModelError::with_details(
                    codes::ARG_INVALID,
                    format!("the id of `{id}` is immutable (10 ID-01)"),
                    json!({ "objectId": id.as_str(), "pointer": format!("/{}", T::TYPE.id_field()) }),
                )]);
            }
            Ok(parsed)
        }
        // Group the patches by entry, keeping their order.
        let mut order: Vec<EntryKey> = Vec::new();
        let mut grouped: BTreeMap<EntryKey, Vec<crate::change::PatchOp>> = BTreeMap::new();
        for (key, patch) in changes {
            if !grouped.contains_key(key) {
                order.push(key.clone());
            }
            grouped.entry(key.clone()).or_default().extend(patch.iter().cloned());
        }
        let mut next = self.clone();
        let mut befores: Vec<(EntryKey, Option<serde_json::Value>)> = Vec::with_capacity(order.len());
        for key in &order {
            let patch = grouped.get(key).map(Vec::as_slice).unwrap_or_default();
            let before = if self.has_entry(key) { Some(self.entry_raw_json(key).map_err(|e| vec![e])?) } else { None };
            let after = entry_after(before.as_ref(), patch, key)?;
            match (&after, key) {
                (None, EntryKey::Project | EntryKey::Library) => {
                    return Err(vec![ModelError::with_details(
                        codes::ARG_INVALID,
                        "the project and library entries cannot be removed",
                        json!({ "pointer": "" }),
                    )]);
                }
                (None, EntryKey::Palette(id)) => {
                    next.palettes.remove(id);
                }
                (None, EntryKey::Bank(id)) => {
                    next.banks.remove(id);
                }
                (None, EntryKey::Drawing(id)) => {
                    next.drawings.remove(id);
                }
                (None, EntryKey::Scene(id)) => {
                    next.scenes.remove(id);
                }
                (None, EntryKey::Element(id)) => {
                    next.elements.remove(id);
                }
                (Some(after), EntryKey::Project) => next.project = Arc::new(staged(after, &self.project.id)?),
                (Some(after), EntryKey::Library) => next.library = Arc::new(staged(after, &self.library.id)?),
                (Some(after), EntryKey::Palette(id)) => {
                    next.palettes.insert(id.clone(), Arc::new(staged(after, id)?));
                }
                (Some(after), EntryKey::Bank(id)) => {
                    next.banks.insert(id.clone(), Arc::new(staged(after, id)?));
                }
                (Some(after), EntryKey::Drawing(id)) => {
                    next.drawings.insert(id.clone(), Arc::new(staged(after, id)?));
                }
                (Some(after), EntryKey::Scene(id)) => {
                    next.scenes.insert(id.clone(), Arc::new(staged(after, id)?));
                }
                (Some(after), EntryKey::Element(id)) => {
                    next.elements.insert(id.clone(), Arc::new(staged(after, id)?));
                }
            }
            if after.is_some() {
                // The staged entry passed the full object input path.
                next.mark_validated(key);
            }
            befores.push((key.clone(), before));
        }
        next.rebuild_derived();
        let errors: Vec<ModelError> =
            next.issues().into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_error()).collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        // Undo and redo describe the **committed** canonical entry (C11-R6-01). Revalidation canonicalises — exposure
        // runs merge, keys reorder, strings normalise — so the raw-JSON inverse of the staged value may name positions
        // that no longer exist or restore the wrong frames. Both are derived from canonical snapshots and verified.
        // An entry created by the commit is undone by removing it (`remove ""`) and one removed by the commit is
        // undone by adding it back (`add ""`), each carrying the canonical entry.
        let mut commits = Vec::with_capacity(befores.len());
        for (key, before) in befores {
            let committed =
                if next.has_entry(&key) { Some(next.entry_raw_json(&key).map_err(|e| vec![e])?) } else { None };
            let root = |op: &'static str, value: Option<serde_json::Value>, old: Option<serde_json::Value>| {
                vec![crate::change::PatchOp {
                    op,
                    from: None,
                    path: String::new(),
                    range: None,
                    value,
                    old_value: old,
                    elided: false,
                }]
            };
            let (undo, redo) = match (&before, &committed) {
                (Some(b), Some(c)) => {
                    let ty = b
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .and_then(crate::types::ObjectType::parse)
                        .unwrap_or(crate::types::ObjectType::Project);
                    let undo = verified_diff(c, b, ty);
                    let redo = if with_redo { verified_diff(b, c, ty) } else { Vec::new() };
                    (undo, redo)
                }
                (None, Some(c)) => (
                    root("remove", None, Some(c.clone())),
                    if with_redo { root("add", Some(c.clone()), None) } else { Vec::new() },
                ),
                (Some(b), None) => (
                    root("add", Some(b.clone()), None),
                    if with_redo { root("remove", None, Some(b.clone())) } else { Vec::new() },
                ),
                (None, None) => (Vec::new(), Vec::new()),
            };
            commits.push(EntryCommit { key, undo, redo });
        }
        Ok((next, commits))
    }

    /// Whether the document holds an entry.
    #[must_use]
    pub fn has_entry(&self, key: &EntryKey) -> bool {
        match key {
            EntryKey::Project | EntryKey::Library => true,
            EntryKey::Palette(id) => self.palettes.contains_key(id),
            EntryKey::Bank(id) => self.banks.contains_key(id),
            EntryKey::Drawing(id) => self.drawings.contains_key(id),
            EntryKey::Scene(id) => self.scenes.contains_key(id),
            EntryKey::Element(id) => self.elements.contains_key(id),
        }
    }

    /// Where an object lives (by plain or compound ID string).
    #[must_use]
    pub fn locate(&self, id: &str) -> Option<&Location> {
        self.index.location_str(id)
    }

    /// Every addressable object's location, in no particular order (C1.2 queries, revision bookkeeping).
    pub fn locations(&self) -> impl Iterator<Item = &Location> {
        self.index.iter()
    }

    /// Element IDs of a scene in Timeline order (depth-first, parents before children).
    #[must_use]
    pub fn tree_order(&self, scene: &Scene) -> Vec<Id> {
        fn walk(nodes: &[TreeNode], out: &mut Vec<Id>, depth: usize) {
            if depth > 1000 {
                return;
            }
            for n in nodes {
                out.push(n.id.clone());
                walk(&n.children, out, depth + 1);
            }
        }
        let mut out = Vec::new();
        walk(&scene.tree, &mut out, 0);
        out
    }

    /// The scene length (10 §4.3, 04 §11.7): the last frame used by any element's exposures, bars or sound
    /// placements; minimum 1.
    #[must_use]
    pub fn scene_length(&self, scene: &Scene) -> u64 {
        let mut len = 1u64;
        for id in scene.elements.ids() {
            let Some(e) = self.element(id) else { continue };
            if let Some(runs) = &e.exposure {
                let mut end = 0u64;
                let mut last_exposed = 0u64;
                for (d, c) in runs {
                    end += u64::from(*c);
                    if d.is_some() {
                        last_exposed = end;
                    }
                }
                len = len.max(last_exposed);
            }
            if let (Some(s), Some(d)) = (e.start, e.duration) {
                len = len.max((u64::from(s) + u64::from(d)).saturating_sub(1));
            }
            for p in e.placements.iter().flatten() {
                len = len.max(u64::from(p.stop_frame));
            }
            for s in e.sequences.iter().flatten() {
                len = len.max(u64::from(s.stop_frame));
            }
        }
        len
    }

    /// Every issue of the document: object-local rules of each entry and the document-level integrity rules.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        let mut out = self.assembly.as_ref().clone();
        self.object_issues(&mut out);
        self.integrity_issues(&mut out);
        out
    }

    fn object_issues(&self, out: &mut Vec<Issue>) {
        let mut push = |id: &Id, issues: Issues| {
            for e in issues.into_errors() {
                let pointer = e.details().get("pointer").and_then(|p| p.as_str()).unwrap_or("").to_owned();
                out.push(Issue::error(codes::ARG_INVALID, id, &pointer, e.message()));
            }
        };
        let check = |c: &dyn Fn(&mut Issues)| {
            let mut i = Issues::new(200);
            c(&mut i);
            i
        };
        // Entries that passed the full object input path have no object-local issue; skipping them gives the same
        // result as checking them (see `validated`).
        let skip = |key: EntryKey| self.is_validated(&key);
        if !skip(EntryKey::Project) {
            push(&self.project.id, check(&|i| self.project.check("", i)));
        }
        for p in self.palettes.values().filter(|p| !skip(EntryKey::Palette(p.id.clone()))) {
            push(&p.id, check(&|i| p.check("", i)));
        }
        for b in self.banks.values().filter(|b| !skip(EntryKey::Bank(b.id.clone()))) {
            push(&b.id, check(&|i| b.check("", i)));
        }
        for d in self.drawings.values().filter(|d| !skip(EntryKey::Drawing(d.id.clone()))) {
            push(&d.id, check(&|i| d.check("", i)));
        }
        for s in self.scenes.values().filter(|s| !skip(EntryKey::Scene(s.id.clone()))) {
            push(&s.id, check(&|i| s.check("", i)));
        }
        for e in self.elements.values().filter(|e| !skip(EntryKey::Element(e.id.clone()))) {
            push(&e.id, check(&|i| e.check("", i)));
        }
        if !skip(EntryKey::Library) {
            push(&self.library.id, check(&|i| self.library.check("", i)));
        }
    }

    #[allow(clippy::too_many_lines)]
    fn integrity_issues(&self, out: &mut Vec<Issue>) {
        let project_id = &self.project.id;
        // Entry membership: every listed child exists, every entry is listed exactly once.
        let membership =
            |listed: &[Id], present: &BTreeSet<&Id>, what: &str, pointer: &str, owner: &Id, out: &mut Vec<Issue>| {
                for (i, id) in listed.iter().enumerate() {
                    if !present.contains(id) {
                        out.push(Issue::error(
                            codes::REF_DANGLING,
                            owner,
                            &format!("{pointer}/{i}"),
                            format!("{what} `{id}` does not exist"),
                        ));
                    }
                }
            };
        let palette_ids: BTreeSet<&Id> = self.palettes.keys().collect();
        let bank_ids: BTreeSet<&Id> = self.banks.keys().collect();
        let scene_ids: BTreeSet<&Id> = self.scenes.keys().collect();
        membership(self.project.palettes.ids(), &palette_ids, "palette", "/palettes", project_id, out);
        membership(self.project.drawing_banks.ids(), &bank_ids, "drawing bank", "/drawingBanks", project_id, out);
        membership(self.project.scenes.ids(), &scene_ids, "scene", "/scenes", project_id, out);
        for (set, listed, what) in [
            (&palette_ids, self.project.palettes.ids(), "palette"),
            (&bank_ids, self.project.drawing_banks.ids(), "drawing bank"),
            (&scene_ids, self.project.scenes.ids(), "scene"),
        ] {
            for id in set.iter() {
                if !listed.contains(id) {
                    out.push(Issue::error(
                        codes::REF_DANGLING,
                        id,
                        "",
                        format!("{what} `{id}` is not listed by the project"),
                    ));
                }
            }
        }
        if self.project.library.id() != &self.library.id {
            out.push(Issue::error(
                codes::REF_DANGLING,
                project_id,
                "/library",
                "the project's library stub does not match the library",
            ));
        }
        // 10 §3, §4.17: the document's library is the project-local library.
        if self.library.kind != crate::entities::library::LibraryKind::Local {
            out.push(Issue::error(
                codes::ARG_INVALID,
                &self.library.id,
                "/kind",
                "the project's library has kind `local`",
            ));
        }
        // 03 §1.2: at least one visible palette.
        if !self.palettes().any(|p| !p.hidden) {
            out.push(Issue::error(
                codes::ARG_INVALID,
                project_id,
                "/palettes",
                "a project has at least one visible (not hidden) palette",
            ));
        }
        // REV-01/REV-02: an object's rev is the documentRev of a committed transaction, never newer than the document.
        for (key, rev) in self.index.revs_ahead() {
            out.push(Issue {
                code: codes::ARG_INVALID,
                severity: Severity::Error,
                object_id: Some(key.clone()),
                pointer: "/rev".to_owned(),
                message: format!("rev {rev} is newer than documentRev {}", self.document_rev),
            });
        }
        // ID-01: unique across the project.
        for (id, count) in self.index.duplicates() {
            out.push(Issue::error(codes::ID_DUPLICATE, &id, "", format!("id `{id}` is used by {count} objects")));
        }
        // Names unique in their scope (10 §1.3).
        let mut names = HashMap::new();
        for p in self.palettes() {
            if let Some(prev) = names.insert(p.name.clone(), &p.id) {
                out.push(Issue::error(
                    codes::ARG_INVALID,
                    &p.id,
                    "/name",
                    format!("palette name `{}` is also used by `{prev}`", p.name),
                ));
            }
        }
        let mut scene_names = HashMap::new();
        for s in self.scenes() {
            if let Some(prev) = scene_names.insert(s.name.clone(), &s.id) {
                out.push(Issue::error(
                    codes::ARG_INVALID,
                    &s.id,
                    "/name",
                    format!("scene name `{}` is also used by `{prev}`", s.name),
                ));
            }
        }
        // Banks and drawings.
        let mut drawing_bank: HashMap<&Id, &Id> = HashMap::new();
        for b in self.banks.values() {
            let mut bank_names = HashMap::new();
            for (i, d) in b.drawings.ids().iter().enumerate() {
                match self.drawings.get(d) {
                    None => out.push(Issue::error(
                        codes::REF_DANGLING,
                        &b.id,
                        &format!("/drawings/{i}"),
                        format!("drawing `{d}` does not exist"),
                    )),
                    Some(dr) => {
                        if let Some(prev) = bank_names.insert(dr.name.clone(), &dr.id) {
                            out.push(Issue::error(
                                codes::ARG_INVALID,
                                d,
                                "/name",
                                format!("drawing name `{}` is also used by `{prev}` in the same bank", dr.name),
                            ));
                        }
                    }
                }
                if let Some(prev) = drawing_bank.insert(d, &b.id) {
                    out.push(Issue::error(
                        codes::ID_DUPLICATE,
                        d,
                        "",
                        format!("drawing `{d}` is listed by banks `{prev}` and `{}`", b.id),
                    ));
                }
            }
            if self.bank_users(&b.id).is_empty() {
                out.push(Issue::warning(codes::REF_DANGLING, &b.id, "", "drawing bank is used by no element"));
            }
        }
        for d in self.drawings.keys() {
            if !drawing_bank.contains_key(d) {
                out.push(Issue::error(codes::REF_DANGLING, d, "", "drawing belongs to no bank"));
            }
        }
        // Scenes and elements.
        let mut element_owner: HashMap<&Id, &Id> = HashMap::new();
        for s in self.scenes() {
            self.scene_integrity(s, &mut element_owner, out);
        }
        for e in self.elements.keys() {
            if !element_owner.contains_key(e) {
                out.push(Issue::error(codes::REF_DANGLING, e, "", "element belongs to no scene"));
            }
        }
        // Colour references resolve (warning: colour recovery handles them, 03 §11.2).
        let colours: BTreeSet<&Id> = self.palettes().flat_map(|p| p.swatches.iter().map(|s| &s.colour_id)).collect();
        for (i, pen) in self.project.pens.iter().enumerate() {
            if let Some(c) = &pen.colour_id
                && !colours.contains(c)
            {
                out.push(Issue::warning(
                    "colour.unresolved",
                    project_id,
                    &format!("/pens/{i}/colourId"),
                    format!("colour `{c}` is defined by no palette"),
                ));
            }
        }
        for d in self.drawings.values() {
            for (pointer, c) in render::drawing_colour_refs(d) {
                if !colours.contains(&c) {
                    out.push(Issue::warning(
                        "colour.unresolved",
                        &d.id,
                        &pointer,
                        format!("colour `{c}` is defined by no palette"),
                    ));
                }
            }
        }
    }

    fn scene_integrity<'a>(&'a self, s: &'a Scene, element_owner: &mut HashMap<&'a Id, &'a Id>, out: &mut Vec<Issue>) {
        let members: BTreeSet<&Id> = s.elements.ids().iter().collect();
        let mut element_names = HashMap::new();
        for (i, e) in s.elements.ids().iter().enumerate() {
            match self.elements.get(e) {
                None => out.push(Issue::error(
                    codes::REF_DANGLING,
                    &s.id,
                    &format!("/elements/{i}"),
                    format!("element `{e}` does not exist"),
                )),
                Some(el) => {
                    if let Some(prev) = element_names.insert(el.name.clone(), &el.id) {
                        out.push(Issue::error(
                            codes::ARG_INVALID,
                            e,
                            "/name",
                            format!("element name `{}` is also used by `{prev}` in the scene", el.name),
                        ));
                    }
                }
            }
            if let Some(prev) = element_owner.insert(e, &s.id) {
                out.push(Issue::error(
                    codes::ID_DUPLICATE,
                    e,
                    "",
                    format!("element `{e}` is listed by scenes `{prev}` and `{}`", s.id),
                ));
            }
        }
        // Tree: every element exactly once, no unknown IDs, children only below elements that can have them,
        // parameter elements directly below their effect (10 §4.3, §5; 06 §2.3).
        let mut seen = BTreeSet::new();
        self.tree_integrity(s, &s.tree, None, "/tree", &members, &mut seen, out);
        for e in &members {
            if !seen.contains(*e) {
                out.push(Issue::error(
                    codes::TREE_INVALID,
                    &s.id,
                    "/tree",
                    format!("element `{e}` is missing from the scene tree"),
                ));
            }
        }
        // Column order: exactly the cell-based elements.
        let cells: BTreeSet<&Id> = members
            .iter()
            .copied()
            .filter(|e| self.elements.get(*e).is_some_and(|el| el.element_type.is_cell_based()))
            .collect();
        let listed: BTreeSet<&Id> = s.column_order.iter().collect();
        if cells != listed || listed.len() != s.column_order.len() {
            out.push(Issue::error(
                codes::TREE_INVALID,
                &s.id,
                "/columnOrder",
                "columnOrder lists each cell-based element of the scene exactly once",
            ));
        }
        // Current camera.
        let cameras: Vec<&Id> = members
            .iter()
            .copied()
            .filter(|e| self.elements.get(*e).is_some_and(|el| el.element_type == ElementType::Camera))
            .collect();
        match &s.current_camera_id {
            Some(c) if !cameras.contains(&c) => out.push(Issue::error(
                codes::REF_DANGLING,
                &s.id,
                "/currentCameraId",
                "currentCameraId is not a camera of this scene",
            )),
            None if !cameras.is_empty() => out.push(Issue::error(
                codes::REF_DANGLING,
                &s.id,
                "/currentCameraId",
                "a scene with cameras has a current camera",
            )),
            _ => {}
        }
        // Palette styles: one entry per palette, each a style of that palette.
        let palettes: Vec<&Id> = self.project.palettes.ids().iter().collect();
        let keys: Vec<&Id> = s.palette_styles.keys().collect();
        if palettes.iter().collect::<BTreeSet<_>>() != keys.iter().collect::<BTreeSet<_>>() {
            out.push(Issue::error(
                codes::REF_DANGLING,
                &s.id,
                "/paletteStyles",
                "paletteStyles has exactly one entry per palette",
            ));
        }
        for (p, st) in &s.palette_styles {
            if let Some(pal) = self.palettes.get(p)
                && !pal.styles.iter().any(|x| &x.id == st)
            {
                out.push(Issue::error(
                    codes::REF_DANGLING,
                    &s.id,
                    &format!("/paletteStyles/{p}"),
                    format!("style `{st}` is not a style of palette `{p}`"),
                ));
            }
        }
        // Element references.
        let assets: HashMap<&Id, &crate::entities::asset::Asset> =
            self.project.assets.iter().map(|a| (&a.id, a)).collect();
        for e in members.iter().filter_map(|e| self.elements.get(*e)) {
            self.element_refs(s, e, &assets, &members, out);
        }
        if self.scene_length(s) > MAX_SCENE_LENGTH {
            out.push(Issue::error(
                codes::LIMIT_EXCEEDED,
                &s.id,
                "",
                format!("scene is longer than {MAX_SCENE_LENGTH} frames"),
            ));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tree_integrity(
        &self,
        s: &Scene,
        nodes: &[TreeNode],
        parent: Option<&Element>,
        pointer: &str,
        members: &BTreeSet<&Id>,
        seen: &mut BTreeSet<Id>,
        out: &mut Vec<Issue>,
    ) {
        for (i, n) in nodes.iter().enumerate() {
            let p = format!("{pointer}/{i}");
            if !members.contains(&n.id) {
                out.push(Issue::error(
                    codes::TREE_INVALID,
                    &s.id,
                    &format!("{p}/id"),
                    format!("tree entry `{}` is not an element of the scene", n.id),
                ));
                continue;
            }
            if !seen.insert(n.id.clone()) {
                out.push(Issue::error(
                    codes::TREE_INVALID,
                    &s.id,
                    &format!("{p}/id"),
                    format!("element `{}` appears more than once in the tree", n.id),
                ));
                continue;
            }
            let Some(el) = self.elements.get(&n.id) else { continue };
            if let Some(par) = parent
                && !par.element_type.can_have_children()
            {
                out.push(Issue::error(
                    codes::TREE_INVALID,
                    &s.id,
                    &p,
                    format!("a {} element cannot have children", par.element_type.as_str()),
                ));
            }
            if el.element_type == ElementType::EffectParameter {
                let ok = matches!(
                    (el.slot, parent.map(|p| p.element_type)),
                    (Some(ParamSlot::Mask), Some(ElementType::Clipping))
                        | (Some(ParamSlot::Sprites), Some(ElementType::ParticleNature | ElementType::ParticleImpact))
                        | (Some(ParamSlot::Bones | ParamSlot::Skin), Some(ElementType::Skeleton))
                );
                if !ok {
                    out.push(Issue::error(
                        codes::TREE_INVALID,
                        &s.id,
                        &p,
                        "a parameter element stays directly under its effect",
                    ));
                }
            }
            self.tree_integrity(s, &n.children, Some(el), &format!("{p}/children"), members, seen, out);
        }
    }

    fn element_refs(
        &self,
        s: &Scene,
        e: &Element,
        assets: &HashMap<&Id, &crate::entities::asset::Asset>,
        members: &BTreeSet<&Id>,
        out: &mut Vec<Issue>,
    ) {
        use crate::entities::asset::{AssetKind, AssetStorage};
        if let Some(bank_id) = &e.drawing_bank_id {
            match self.banks.get(bank_id) {
                None => out.push(Issue::error(
                    codes::REF_DANGLING,
                    &e.id,
                    "/drawingBankId",
                    format!("drawing bank `{bank_id}` does not exist"),
                )),
                Some(bank) => {
                    if e.element_type == ElementType::Image {
                        // 10 §4.10: a drawing of an image element contains exactly one bitmap node.
                        for d in bank.drawings.ids().iter().filter_map(|d| self.drawings.get(d)) {
                            let ok = matches!(d.nodes.as_slice(),
                                [n] if n.kind == crate::entities::drawing::NodeKind::Bitmap);
                            if !ok {
                                out.push(Issue::error(
                                    codes::ARG_INVALID,
                                    &d.id,
                                    "/nodes",
                                    "a drawing of an image element contains exactly one bitmap node",
                                ));
                            }
                        }
                    }
                    let drawings: BTreeSet<&Id> = bank.drawings.ids().iter().collect();
                    for (i, (d, _)) in e.exposure.iter().flatten().enumerate() {
                        if let Some(d) = d
                            && !drawings.contains(d)
                        {
                            out.push(Issue::error(
                                codes::EXPOSURE_INVALID,
                                &e.id,
                                &format!("/exposure/{i}/0"),
                                format!("drawing `{d}` is not in the element's bank"),
                            ));
                        }
                    }
                }
            }
        }
        for (i, p) in e.placements.iter().flatten().enumerate() {
            match assets.get(&p.asset_id) {
                Some(a) if matches!(a.kind, AssetKind::Sound | AssetKind::Movie) => {}
                Some(_) => out.push(Issue::error(
                    codes::REF_WRONG_TYPE,
                    &e.id,
                    &format!("/placements/{i}/assetId"),
                    "placements use sound or movie assets",
                )),
                None => out.push(Issue::error(
                    codes::REF_DANGLING,
                    &e.id,
                    &format!("/placements/{i}/assetId"),
                    format!("asset `{}` does not exist", p.asset_id),
                )),
            }
        }
        for (i, q) in e.sequences.iter().flatten().enumerate() {
            match assets.get(&q.asset_id) {
                Some(a) if a.storage == AssetStorage::Linked => {}
                Some(_) => out.push(Issue::error(
                    codes::REF_WRONG_TYPE,
                    &e.id,
                    &format!("/sequences/{i}/assetId"),
                    "media sequences use linked assets",
                )),
                None => out.push(Issue::error(
                    codes::REF_DANGLING,
                    &e.id,
                    &format!("/sequences/{i}/assetId"),
                    format!("asset `{}` does not exist", q.asset_id),
                )),
            }
        }
        for (i, m) in e.lip_sync_mappings.iter().flatten().enumerate() {
            let target = self.elements.get(&m.drawing_element_id).filter(|t| members.contains(&t.id));
            match target {
                None => out.push(Issue::error(
                    codes::REF_DANGLING,
                    &e.id,
                    &format!("/lipSyncMappings/{i}/drawingElementId"),
                    "the mapped element is not an element of this scene",
                )),
                Some(t) => {
                    let bank: BTreeSet<&Id> = t
                        .drawing_bank_id
                        .as_ref()
                        .and_then(|b| self.banks.get(b))
                        .map(|b| b.drawings.ids().iter().collect())
                        .unwrap_or_default();
                    for shape in [
                        crate::entities::drawing::MouthShape::A,
                        crate::entities::drawing::MouthShape::B,
                        crate::entities::drawing::MouthShape::C,
                        crate::entities::drawing::MouthShape::D,
                        crate::entities::drawing::MouthShape::E,
                        crate::entities::drawing::MouthShape::F,
                        crate::entities::drawing::MouthShape::G,
                        crate::entities::drawing::MouthShape::X,
                    ] {
                        if let Some(d) = m.mapping.get(shape)
                            && !bank.contains(d)
                        {
                            out.push(Issue::error(
                                codes::REF_DANGLING,
                                &e.id,
                                &format!("/lipSyncMappings/{i}/mapping/{shape:?}"),
                                format!("drawing `{d}` is not a drawing of the mapped element"),
                            ));
                        }
                    }
                }
            }
        }
        let _ = s;
    }
}

fn ptr_eq_opt<T>(a: Option<&Arc<T>>, b: Option<&Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
