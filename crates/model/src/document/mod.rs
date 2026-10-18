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

mod index;
mod paths;
mod render;

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
}

impl Document {
    /// Assembles and checks a document. Every object must already satisfy its object-local rules (they are checked
    /// again here) and the document-level integrity rules of 10 §5 and §7.2.
    ///
    /// # Errors
    /// Every error-severity issue found (object-local rules as `arg.invalid`, integrity rules with the codes of
    /// 10 §9). Warnings (e.g. unresolved colour IDs) do not prevent assembly; read them with [`Document::issues`].
    pub fn from_parts(parts: DocumentParts) -> Result<Self, Vec<ModelError>> {
        let doc = Self::assemble(parts);
        let errors: Vec<ModelError> =
            doc.issues().into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_error()).collect();
        if errors.is_empty() { Ok(doc) } else { Err(errors) }
    }

    /// Assembles a document without rejecting integrity errors (the load path reports and repairs them, 10 §7.2).
    #[must_use]
    pub fn assemble(parts: DocumentParts) -> Self {
        let palettes = parts.palettes.into_iter().map(|p| (p.id.clone(), Arc::new(p))).collect();
        let banks = parts.banks.into_iter().map(|b| (b.id.clone(), Arc::new(b))).collect();
        let drawings = parts.drawings.into_iter().map(|d| (d.id.clone(), Arc::new(d))).collect();
        let scenes = parts.scenes.into_iter().map(|s| (s.id.clone(), Arc::new(s))).collect();
        let elements = parts.elements.into_iter().map(|e| (e.id.clone(), Arc::new(e))).collect();
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
        };
        doc.rebuild_derived();
        doc
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

    /// Where an object lives (by plain or compound ID string).
    #[must_use]
    pub fn locate(&self, id: &str) -> Option<&Location> {
        self.index.location_str(id)
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
        let mut out = Vec::new();
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
        push(&self.project.id, check(&|i| self.project.check("", i)));
        for p in self.palettes.values() {
            push(&p.id, check(&|i| p.check("", i)));
        }
        for b in self.banks.values() {
            push(&b.id, check(&|i| b.check("", i)));
        }
        for d in self.drawings.values() {
            push(&d.id, check(&|i| d.check("", i)));
        }
        for s in self.scenes.values() {
            push(&s.id, check(&|i| s.check("", i)));
        }
        for e in self.elements.values() {
            push(&e.id, check(&|i| e.check("", i)));
        }
        push(&self.library.id, check(&|i| self.library.check("", i)));
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
