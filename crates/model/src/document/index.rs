//! The ID index of a snapshot: where every addressable object lives.

use std::collections::{BTreeMap, HashMap};

use super::Document;
use crate::entities::drawing::DrawingNode;
use crate::entities::library::LibraryNode;
use crate::id::Id;
use crate::types::ObjectType;

/// The package entry an object lives in (10 §6.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryKey {
    /// `project.json`.
    Project,
    /// `palettes/<id>.json`.
    Palette(Id),
    /// `banks/<id>/bank.json`.
    Bank(Id),
    /// `banks/<bankId>/<id>.json`.
    Drawing(Id),
    /// `scenes/<id>/scene.json`.
    Scene(Id),
    /// `scenes/<sceneId>/elements/<id>.json`.
    Element(Id),
    /// `library/library.json`.
    Library,
}

/// Where an object lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Its type.
    pub ty: ObjectType,
    /// The entry holding it.
    pub entry: EntryKey,
    /// The index key of the containing object: the bank of a drawing, the scene of an element, the node of a stroke
    /// (`<drawingId>/<nodeId>`), the channel of a key, …; `None` for the project.
    pub parent: Option<String>,
    /// The key of the object in the index (`<containerId>/<localId>` for container-local IDs, 10 ID-03).
    pub key: String,
    /// The plain (local) ID.
    pub id: Id,
    /// The object's `rev` (10 REV-02), for `expectRev` checks (REV-05).
    pub rev: u64,
}

/// The index.
#[derive(Debug, Clone, Default)]
pub(super) struct Index {
    by_key: HashMap<String, Location>,
    /// Plain local ID → keys of container-local objects carrying it.
    local: HashMap<String, Vec<String>>,
    duplicates: BTreeMap<Id, usize>,
    /// Objects whose `rev` is newer than the document's `documentRev` (impossible by REV-01/REV-02).
    ahead: Vec<(String, u64)>,
}

impl Index {
    pub(super) fn build(doc: &Document) -> Index {
        let mut b = Builder { index: Index::default(), document_rev: doc.document_rev };
        let p = &doc.project;
        b.add(ObjectType::Project, &p.id, EntryKey::Project, None, p.rev);
        b.add(ObjectType::Transition, &p.start_transition.id, EntryKey::Project, Some(&p.id), p.start_transition.rev);
        for pen in &p.pens {
            b.add(ObjectType::Pen, &pen.id, EntryKey::Project, Some(&p.id), pen.rev);
        }
        for a in &p.assets {
            b.add(ObjectType::Asset, &a.id, EntryKey::Project, Some(&p.id), a.rev);
        }
        for pal in doc.palettes.values() {
            let e = EntryKey::Palette(pal.id.clone());
            b.add(ObjectType::Palette, &pal.id, e.clone(), Some(&p.id), pal.rev);
            for s in &pal.styles {
                b.add(ObjectType::PaletteStyle, &s.id, e.clone(), Some(&pal.id), s.rev);
            }
            for s in &pal.swatches {
                b.add(ObjectType::Swatch, &s.colour_id, e.clone(), Some(&pal.id), s.rev);
            }
        }
        let mut drawing_bank: HashMap<&Id, &Id> = HashMap::new();
        for bank in doc.banks.values() {
            b.add(ObjectType::DrawingBank, &bank.id, EntryKey::Bank(bank.id.clone()), Some(&p.id), bank.rev);
            for d in bank.drawings.ids() {
                drawing_bank.entry(d).or_insert(&bank.id);
            }
        }
        for d in doc.drawings.values() {
            let e = EntryKey::Drawing(d.id.clone());
            b.add(ObjectType::Drawing, &d.id, e.clone(), drawing_bank.get(&d.id).copied(), d.rev);
            b.nodes(&d.id, d.id.as_str(), &d.nodes, &e);
        }
        for s in doc.scenes.values() {
            let e = EntryKey::Scene(s.id.clone());
            b.add(ObjectType::Scene, &s.id, e.clone(), Some(&p.id), s.rev);
            b.add(ObjectType::Transition, &s.out_transition.id, e, Some(&s.id), s.out_transition.rev);
        }
        for el in doc.elements.values() {
            let e = EntryKey::Element(el.id.clone());
            let scene = doc.element_scene.get(&el.id);
            b.add(ObjectType::Element, &el.id, e.clone(), scene, el.rev);
            for ch in el.channels.iter().flat_map(|m| m.values()) {
                b.add(ObjectType::Channel, &ch.id, e.clone(), Some(&el.id), ch.rev);
                for k in ch.keys.iter().flatten() {
                    b.add_local(ObjectType::Key, &ch.id, &k.id, e.clone(), ch.id.as_str().to_owned(), k.rev);
                }
                for mp in ch.points.iter().flatten() {
                    b.add_local(ObjectType::MotionPoint, &ch.id, &mp.id, e.clone(), ch.id.as_str().to_owned(), mp.rev);
                }
            }
            for x in el.blocks.iter().flatten() {
                b.add(ObjectType::ParticleBlock, &x.id, e.clone(), Some(&el.id), x.rev);
            }
            for x in el.placements.iter().flatten() {
                b.add(ObjectType::SoundPlacement, &x.id, e.clone(), Some(&el.id), x.rev);
            }
            for x in el.sequences.iter().flatten() {
                b.add(ObjectType::MediaSequence, &x.id, e.clone(), Some(&el.id), x.rev);
            }
        }
        let lib = &doc.library;
        b.add(ObjectType::Library, &lib.id, EntryKey::Library, Some(&p.id), lib.rev);
        b.library(&lib.id, &lib.children);
        b.index
    }

    /// Looks up a plain or compound ID. A plain ID that only exists as a container-local ID resolves when it is
    /// unique among local IDs.
    pub(super) fn location_str(&self, key: &str) -> Option<&Location> {
        if let Some(l) = self.by_key.get(key) {
            return Some(l);
        }
        match self.local.get(key).map(Vec::as_slice) {
            Some([only]) => self.by_key.get(only),
            _ => None,
        }
    }

    /// Keys of container-local objects whose local ID is `plain` (more than one = ambiguous).
    pub(super) fn local_candidates(&self, plain: &str) -> &[String] {
        self.local.get(plain).map_or(&[], Vec::as_slice)
    }

    pub(super) fn duplicates(&self) -> Vec<(Id, usize)> {
        self.duplicates.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    pub(super) fn revs_ahead(&self) -> &[(String, u64)] {
        &self.ahead
    }

    pub(super) fn len(&self) -> usize {
        self.by_key.len()
    }
}

struct Builder {
    index: Index,
    document_rev: u64,
}

impl Builder {
    fn note_rev(&mut self, key: &str, rev: u64) {
        if rev > self.document_rev {
            self.index.ahead.push((key.to_owned(), rev));
        }
    }

    fn add(&mut self, ty: ObjectType, id: &Id, entry: EntryKey, parent: Option<&Id>, rev: u64) {
        let key = id.as_str().to_owned();
        self.note_rev(&key, rev);
        let loc = Location {
            ty,
            entry,
            parent: parent.map(|p| p.as_str().to_owned()),
            key: key.clone(),
            id: id.clone(),
            rev,
        };
        if self.index.by_key.insert(key, loc).is_some() {
            *self.index.duplicates.entry(id.clone()).or_insert(1) += 1;
        }
    }

    fn add_local(&mut self, ty: ObjectType, container: &Id, id: &Id, entry: EntryKey, parent: String, rev: u64) {
        let key = format!("{container}/{id}");
        self.note_rev(&key, rev);
        let loc = Location { ty, entry, parent: Some(parent), key: key.clone(), id: id.clone(), rev };
        if self.index.by_key.insert(key.clone(), loc).is_some() {
            *self.index.duplicates.entry(id.clone()).or_insert(1) += 1;
        } else {
            self.index.local.entry(id.as_str().to_owned()).or_default().push(key);
        }
    }

    fn nodes(&mut self, drawing: &Id, parent: &str, nodes: &[DrawingNode], entry: &EntryKey) {
        for n in nodes {
            let key = format!("{drawing}/{}", n.id);
            self.add_local(ObjectType::DrawingNode, drawing, &n.id, entry.clone(), parent.to_owned(), n.rev);
            for s in n.strokes.iter().flatten() {
                self.add_local(ObjectType::Stroke, drawing, &s.id, entry.clone(), key.clone(), s.rev);
            }
            if let Some(children) = &n.children {
                self.nodes(drawing, &key, children, entry);
            }
        }
    }

    fn library(&mut self, parent: &Id, children: &[LibraryNode]) {
        for c in children {
            match c {
                LibraryNode::Folder(f) => {
                    self.add(ObjectType::LibraryFolder, &f.id, EntryKey::Library, Some(parent), f.rev);
                    self.library(&f.id, &f.children);
                }
                LibraryNode::Item(i) => {
                    self.add(ObjectType::LibraryItem, &i.id, EntryKey::Library, Some(parent), i.rev)
                }
            }
        }
    }
}
