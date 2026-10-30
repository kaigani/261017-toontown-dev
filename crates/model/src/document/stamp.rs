//! Revision stamping (10 REV-01…REV-03): a committed transaction sets `documentRev` to its own revision and stamps
//! the objects it changed with it. Stamping walks the typed entries; it does not re-serialise or re-validate them
//! (only `rev` values change, and they stay ≤ `documentRev`).

use std::collections::BTreeMap;
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
        let stamps: Vec<(String, u64)> = keys.iter().map(|k| (k.clone(), document_rev)).collect();
        self.with_object_revisions(document_rev, &stamps)
    }

    /// The snapshot with `documentRev = document_rev` and each listed object's `rev` set to the given value (at most
    /// `document_rev`). Used when restoring a snapshot (checkpoints, history jumps): objects whose content changes get
    /// the new revision, objects whose content does not change keep their current revision (never a lower one,
    /// REV-03). A drawing-local object stamped with `document_rev` also stamps its drawing (REV-02).
    ///
    /// # Errors
    /// As [`Document::with_revisions`]; `arg.invalid` for a revision above `document_rev`.
    pub fn with_object_revisions(&self, document_rev: u64, stamps: &[(String, u64)]) -> Result<Document, ModelError> {
        self.stamp_objects(document_rev, stamps, false)
    }

    /// The snapshot as it was at an earlier `document_rev`: the listed objects get their earlier revisions (each at
    /// most `document_rev`; keys that do not exist are skipped, they were created later). Only reconstructions of
    /// history use it (`doc.diff` rebuilding an earlier revision from back-patches, which restore content but not the
    /// revisions stamped afterwards); no drawing rule applies, the drawing's earlier revision is listed explicitly.
    ///
    /// # Errors
    /// `arg.invalid` for a revision above `document_rev`, and `internal` if a stamp is not carried by the returned
    /// document (C12-R2-01: every stamp is checked against the object returned). A reconstruction whose other objects
    /// are newer than `document_rev` (stamps that do not cover everything changed since) is returned as built; its
    /// `issues()` report REV-01, so no installation path accepts it (the executor's load transaction runs the full
    /// integrity gate).
    pub fn at_earlier_revision(&self, document_rev: u64, stamps: &[(String, u64)]) -> Result<Document, ModelError> {
        let present: Vec<(String, u64)> = stamps.iter().filter(|(k, _)| self.locate(k).is_some()).cloned().collect();
        let earlier = self.stamp_objects(document_rev, &present, true)?;
        for (key, rev) in &present {
            if earlier.locate(key).map(|l| l.rev) != Some(*rev) {
                return Err(ModelError::with_details(
                    codes::INTERNAL,
                    format!("`{key}` does not carry its earlier revision {rev} after the rewind"),
                    serde_json::json!({ "ref": key, "rev": rev }),
                ));
            }
        }
        Ok(earlier)
    }

    fn stamp_objects(
        &self,
        document_rev: u64,
        stamps: &[(String, u64)],
        restore: bool,
    ) -> Result<Document, ModelError> {
        if !restore && document_rev <= self.document_rev {
            return Err(ModelError::with_details(
                codes::ARG_INVALID,
                format!("documentRev {document_rev} is not newer than {}", self.document_rev),
                serde_json::json!({ "documentRev": self.document_rev }),
            ));
        }
        let mut by_entry: BTreeMap<EntryKey, BTreeMap<String, u64>> = BTreeMap::new();
        for (key, rev) in stamps {
            if *rev > document_rev {
                return Err(ModelError::with_details(
                    codes::ARG_INVALID,
                    format!("rev {rev} of `{key}` is above documentRev {document_rev}"),
                    serde_json::json!({ "ref": key }),
                ));
            }
            let loc = self.locate(key).ok_or_else(|| {
                ModelError::with_details(
                    codes::REF_NOT_FOUND,
                    format!("`{key}` is not in the document"),
                    serde_json::json!({ "ref": key }),
                )
            })?;
            let map = by_entry.entry(loc.entry.clone()).or_default();
            map.insert(loc.key.clone(), *rev);
            if let EntryKey::Drawing(d) = &loc.entry
                && !restore
                && *rev == document_rev
            {
                map.insert(d.as_str().to_owned(), document_rev);
            }
        }
        let mut next = self.clone();
        next.document_rev = document_rev;
        for (entry, set) in by_entry {
            let get = |key: &str| set.get(key).copied();
            match &entry {
                EntryKey::Project => {
                    let mut p = (*self.project).clone();
                    if let Some(r) = get(p.id.as_str()) {
                        p.rev = r;
                    }
                    if let Some(r) = get(p.start_transition.id.as_str()) {
                        p.start_transition.rev = r;
                    }
                    p.pens.iter_mut().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
                    p.assets.iter_mut().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
                    next.project = Arc::new(p);
                }
                EntryKey::Library => {
                    let mut l = (*self.library).clone();
                    if let Some(r) = get(l.id.as_str()) {
                        l.rev = r;
                    }
                    stamp_library(&mut l.children, &set);
                    next.library = Arc::new(l);
                }
                EntryKey::Palette(id) => {
                    if let Some(p) = self.palettes.get(id) {
                        let mut p = (**p).clone();
                        if let Some(r) = get(p.id.as_str()) {
                            p.rev = r;
                        }
                        p.styles.iter_mut().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
                        p.swatches.iter_mut().for_each(|x| x.rev = get(x.colour_id.as_str()).unwrap_or(x.rev));
                        next.palettes.insert(id.clone(), Arc::new(p));
                    }
                }
                EntryKey::Bank(id) => {
                    if let Some(b) = self.banks.get(id) {
                        let mut b = (**b).clone();
                        if let Some(r) = get(b.id.as_str()) {
                            b.rev = r;
                        }
                        next.banks.insert(id.clone(), Arc::new(b));
                    }
                }
                EntryKey::Drawing(id) => {
                    if let Some(d) = self.drawings.get(id) {
                        let mut d = (**d).clone();
                        if let Some(r) = get(d.id.as_str()) {
                            d.rev = r;
                        }
                        let prefix = format!("{}/", d.id);
                        stamp_nodes(&mut d.nodes, &prefix, &set);
                        next.drawings.insert(id.clone(), Arc::new(d));
                    }
                }
                EntryKey::Scene(id) => {
                    if let Some(s) = self.scenes.get(id) {
                        let mut s = (**s).clone();
                        if let Some(r) = get(s.id.as_str()) {
                            s.rev = r;
                        }
                        if let Some(r) = get(s.out_transition.id.as_str()) {
                            s.out_transition.rev = r;
                        }
                        next.scenes.insert(id.clone(), Arc::new(s));
                    }
                }
                EntryKey::Element(id) => {
                    if let Some(e) = self.elements.get(id) {
                        let mut e = (**e).clone();
                        if let Some(r) = get(e.id.as_str()) {
                            e.rev = r;
                        }
                        for ch in e.channels.iter_mut().flat_map(|m| m.values_mut()) {
                            if let Some(r) = get(ch.id.as_str()) {
                                ch.rev = r;
                            }
                            let prefix = format!("{}/", ch.id);
                            let local = |x: &Id| set.get(&format!("{prefix}{x}")).copied();
                            ch.keys.iter_mut().flatten().for_each(|k| k.rev = local(&k.id).unwrap_or(k.rev));
                            ch.points.iter_mut().flatten().for_each(|p| p.rev = local(&p.id).unwrap_or(p.rev));
                        }
                        e.blocks.iter_mut().flatten().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
                        e.placements.iter_mut().flatten().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
                        e.sequences.iter_mut().flatten().for_each(|x| x.rev = get(x.id.as_str()).unwrap_or(x.rev));
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

fn stamp_nodes(nodes: &mut [DrawingNode], prefix: &str, set: &BTreeMap<String, u64>) {
    for n in nodes {
        if let Some(r) = set.get(&format!("{prefix}{}", n.id)) {
            n.rev = *r;
        }
        for s in n.strokes.iter_mut().flatten() {
            if let Some(r) = set.get(&format!("{prefix}{}", s.id)) {
                s.rev = *r;
            }
        }
        if let Some(children) = n.children.as_mut() {
            stamp_nodes(children, prefix, set);
        }
    }
}

fn stamp_library(children: &mut [LibraryNode], set: &BTreeMap<String, u64>) {
    for c in children {
        match c {
            LibraryNode::Folder(f) => {
                if let Some(r) = set.get(f.id.as_str()) {
                    f.rev = *r;
                }
                stamp_library(&mut f.children, set);
            }
            LibraryNode::Item(i) => {
                if let Some(r) = set.get(i.id.as_str()) {
                    i.rev = *r;
                }
            }
        }
    }
}
