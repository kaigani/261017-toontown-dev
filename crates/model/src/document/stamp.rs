//! Revision stamping (10 REV-01…REV-03): a committed transaction sets `documentRev` to its own revision and stamps
//! the objects it changed with it. Stamping walks the typed entries; it does not re-serialise or re-validate them
//! (only `rev` values change, and they stay ≤ `documentRev`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{Document, EntryKey};
use crate::entities::drawing::DrawingNode;
use crate::entities::library::LibraryNode;
use crate::error::{ModelError, codes};
use crate::id::Id;

impl Document {
    /// The snapshot with `documentRev = document_rev` and every object in `keys` (index keys: plain or compound
    /// IDs) stamped with `rev = document_rev`. A drawing-local object (node, stroke) also stamps its drawing (REV-02:
    /// content edits of a drawing change the drawing's `rev`). Unchanged entries stay shared.
    ///
    /// # Errors
    /// `arg.invalid` if `document_rev` is not greater than the current `documentRev` (REV-01: revisions never
    /// decrease, every transaction increments by one); `ref.notFound` for a key that is not in the document.
    pub fn with_revisions(&self, document_rev: u64, keys: &[String]) -> Result<Document, ModelError> {
        if document_rev <= self.document_rev {
            return Err(ModelError::with_details(
                codes::ARG_INVALID,
                format!("documentRev {document_rev} is not newer than {}", self.document_rev),
                serde_json::json!({ "documentRev": self.document_rev }),
            ));
        }
        let mut by_entry: BTreeMap<EntryKey, BTreeSet<String>> = BTreeMap::new();
        for key in keys {
            let loc = self.locate(key).ok_or_else(|| {
                ModelError::with_details(
                    codes::REF_NOT_FOUND,
                    format!("`{key}` is not in the document"),
                    serde_json::json!({ "ref": key }),
                )
            })?;
            let set = by_entry.entry(loc.entry.clone()).or_default();
            set.insert(loc.key.clone());
            if let EntryKey::Drawing(d) = &loc.entry {
                set.insert(d.as_str().to_owned());
            }
        }
        let mut next = self.clone();
        next.document_rev = document_rev;
        let rev = document_rev;
        for (entry, set) in by_entry {
            let hit = |id: &Id| set.contains(id.as_str());
            match &entry {
                EntryKey::Project => {
                    let mut p = (*self.project).clone();
                    if hit(&p.id) {
                        p.rev = rev;
                    }
                    if hit(&p.start_transition.id) {
                        p.start_transition.rev = rev;
                    }
                    p.pens.iter_mut().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                    p.assets.iter_mut().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                    next.project = Arc::new(p);
                }
                EntryKey::Library => {
                    let mut l = (*self.library).clone();
                    if hit(&l.id) {
                        l.rev = rev;
                    }
                    stamp_library(&mut l.children, &set, rev);
                    next.library = Arc::new(l);
                }
                EntryKey::Palette(id) => {
                    if let Some(p) = self.palettes.get(id) {
                        let mut p = (**p).clone();
                        if hit(&p.id) {
                            p.rev = rev;
                        }
                        p.styles.iter_mut().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                        p.swatches.iter_mut().filter(|x| hit(&x.colour_id)).for_each(|x| x.rev = rev);
                        next.palettes.insert(id.clone(), Arc::new(p));
                    }
                }
                EntryKey::Bank(id) => {
                    if let Some(b) = self.banks.get(id) {
                        let mut b = (**b).clone();
                        if hit(&b.id) {
                            b.rev = rev;
                        }
                        next.banks.insert(id.clone(), Arc::new(b));
                    }
                }
                EntryKey::Drawing(id) => {
                    if let Some(d) = self.drawings.get(id) {
                        let mut d = (**d).clone();
                        if hit(&d.id) {
                            d.rev = rev;
                        }
                        let prefix = format!("{}/", d.id);
                        stamp_nodes(&mut d.nodes, &prefix, &set, rev);
                        next.drawings.insert(id.clone(), Arc::new(d));
                    }
                }
                EntryKey::Scene(id) => {
                    if let Some(s) = self.scenes.get(id) {
                        let mut s = (**s).clone();
                        if hit(&s.id) {
                            s.rev = rev;
                        }
                        if hit(&s.out_transition.id) {
                            s.out_transition.rev = rev;
                        }
                        next.scenes.insert(id.clone(), Arc::new(s));
                    }
                }
                EntryKey::Element(id) => {
                    if let Some(e) = self.elements.get(id) {
                        let mut e = (**e).clone();
                        if hit(&e.id) {
                            e.rev = rev;
                        }
                        for ch in e.channels.iter_mut().flat_map(|m| m.values_mut()) {
                            if hit(&ch.id) {
                                ch.rev = rev;
                            }
                            let prefix = format!("{}/", ch.id);
                            let local = |x: &Id| set.contains(&format!("{prefix}{x}"));
                            ch.keys.iter_mut().flatten().filter(|k| local(&k.id)).for_each(|k| k.rev = rev);
                            ch.points.iter_mut().flatten().filter(|p| local(&p.id)).for_each(|p| p.rev = rev);
                        }
                        e.blocks.iter_mut().flatten().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                        e.placements.iter_mut().flatten().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                        e.sequences.iter_mut().flatten().filter(|x| hit(&x.id)).for_each(|x| x.rev = rev);
                        next.elements.insert(id.clone(), Arc::new(e));
                    }
                }
            }
            // Only `rev` values changed: an entry that was validated stays validated.
            next.carry_validation(self, &entry);
        }
        next.rebuild_derived();
        Ok(next)
    }
}

fn stamp_nodes(nodes: &mut [DrawingNode], prefix: &str, set: &BTreeSet<String>, rev: u64) {
    for n in nodes {
        if set.contains(&format!("{prefix}{}", n.id)) {
            n.rev = rev;
        }
        for s in n.strokes.iter_mut().flatten() {
            if set.contains(&format!("{prefix}{}", s.id)) {
                s.rev = rev;
            }
        }
        if let Some(children) = n.children.as_mut() {
            stamp_nodes(children, prefix, set, rev);
        }
    }
}

fn stamp_library(children: &mut [LibraryNode], set: &BTreeSet<String>, rev: u64) {
    for c in children {
        match c {
            LibraryNode::Folder(f) => {
                if set.contains(f.id.as_str()) {
                    f.rev = rev;
                }
                stamp_library(&mut f.children, set, rev);
            }
            LibraryNode::Item(i) => {
                if set.contains(i.id.as_str()) {
                    i.rev = rev;
                }
            }
        }
    }
}
