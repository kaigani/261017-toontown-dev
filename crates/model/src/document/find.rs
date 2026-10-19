//! Typed lookup of one object: its JSON without serialising the rest of its entry (C1.2 touched-object diffs).

use serde::Serialize;
use serde_json::Value;

use super::{Document, EntryKey};
use crate::entities::drawing::DrawingNode;
use crate::entities::library::LibraryNode;
use crate::error::{ModelError, codes};
use crate::types::ObjectType;

fn to_value<T: Serialize>(v: &T) -> Result<Value, ModelError> {
    serde_json::to_value(v).map_err(|e| ModelError::new(codes::INTERNAL, format!("serialisation failed: {e}")))
}

fn find_node(nodes: &[DrawingNode], prefix: &str, key: &str) -> Option<Result<Value, ModelError>> {
    for n in nodes {
        if format!("{prefix}{}", n.id) == key {
            return Some(to_value(n));
        }
        for s in n.strokes.iter().flatten() {
            if format!("{prefix}{}", s.id) == key {
                return Some(to_value(s));
            }
        }
        if let Some(found) = n.children.as_deref().and_then(|c| find_node(c, prefix, key)) {
            return Some(found);
        }
    }
    None
}

fn find_library(children: &[LibraryNode], key: &str) -> Option<Result<Value, ModelError>> {
    for c in children {
        match c {
            LibraryNode::Folder(f) => {
                if f.id.as_str() == key {
                    return Some(to_value(f));
                }
                if let Some(found) = find_library(&f.children, key) {
                    return Some(found);
                }
            }
            LibraryNode::Item(i) if i.id.as_str() == key => return Some(to_value(i)),
            LibraryNode::Item(_) => {}
        }
    }
    None
}

fn node_pointer(nodes: &[DrawingNode], at: &str, prefix: &str, key: &str) -> Option<String> {
    for (i, n) in nodes.iter().enumerate() {
        let here = format!("{at}/{i}");
        if format!("{prefix}{}", n.id) == key {
            return Some(here);
        }
        if let Some(k) = n.strokes.iter().flatten().position(|s| format!("{prefix}{}", s.id) == key) {
            return Some(format!("{here}/strokes/{k}"));
        }
        if let Some(found) =
            n.children.as_deref().and_then(|c| node_pointer(c, &format!("{here}/children"), prefix, key))
        {
            return Some(found);
        }
    }
    None
}

fn library_pointer(children: &[LibraryNode], at: &str, key: &str) -> Option<String> {
    for (i, c) in children.iter().enumerate() {
        let here = format!("{at}/{i}");
        match c {
            LibraryNode::Folder(f) => {
                if f.id.as_str() == key {
                    return Some(here);
                }
                if let Some(found) = library_pointer(&f.children, &format!("{here}/children"), key) {
                    return Some(found);
                }
            }
            LibraryNode::Item(it) if it.id.as_str() == key => return Some(here),
            LibraryNode::Item(_) => {}
        }
    }
    None
}

impl Document {
    /// The entry holding an object and the JSON Pointer of the object inside the entry's JSON, found through the
    /// index and the typed entry without serialising anything (C1.2: addressing a stroke of a 100 000-stroke
    /// drawing costs a walk, not a serialisation).
    ///
    /// # Errors
    /// `ref.notFound` for an unknown key.
    pub fn object_pointer(&self, key: &str) -> Result<(EntryKey, ObjectType, String), ModelError> {
        let not_found = || {
            ModelError::with_details(
                codes::REF_NOT_FOUND,
                format!("`{key}` is not in the document"),
                serde_json::json!({ "ref": key }),
            )
        };
        let loc = self.locate(key).ok_or_else(not_found)?;
        let ty = loc.ty;
        let key = loc.key.as_str();
        let pos = |found: Option<usize>, field: &str| found.map(|i| format!("/{field}/{i}"));
        let pointer: Option<String> = match &loc.entry {
            EntryKey::Project => {
                let p = &*self.project;
                if p.id.as_str() == key {
                    Some(String::new())
                } else if p.start_transition.id.as_str() == key {
                    Some("/startTransition".to_owned())
                } else {
                    pos(p.pens.iter().position(|x| x.id.as_str() == key), "pens")
                        .or_else(|| pos(p.assets.iter().position(|x| x.id.as_str() == key), "assets"))
                }
            }
            EntryKey::Library => {
                let l = &*self.library;
                if l.id.as_str() == key { Some(String::new()) } else { library_pointer(&l.children, "/children", key) }
            }
            EntryKey::Palette(id) => self.palettes.get(id).and_then(|p| {
                if p.id.as_str() == key {
                    Some(String::new())
                } else {
                    pos(p.styles.iter().position(|x| x.id.as_str() == key), "styles")
                        .or_else(|| pos(p.swatches.iter().position(|x| x.colour_id.as_str() == key), "swatches"))
                }
            }),
            EntryKey::Bank(id) => self.banks.get(id).filter(|b| b.id.as_str() == key).map(|_| String::new()),
            EntryKey::Drawing(id) => self.drawings.get(id).and_then(|d| {
                if d.id.as_str() == key {
                    Some(String::new())
                } else {
                    node_pointer(&d.nodes, "/nodes", &format!("{}/", d.id), key)
                }
            }),
            EntryKey::Scene(id) => self.scenes.get(id).and_then(|s| {
                if s.id.as_str() == key {
                    Some(String::new())
                } else if s.out_transition.id.as_str() == key {
                    Some("/outTransition".to_owned())
                } else {
                    None
                }
            }),
            EntryKey::Element(id) => self.elements.get(id).and_then(|e| {
                if e.id.as_str() == key {
                    return Some(String::new());
                }
                for (name, ch) in e.channels.iter().flat_map(|m| m.iter()) {
                    let at = format!("/channels/{}", crate::pointer::escape(name));
                    if ch.id.as_str() == key {
                        return Some(at);
                    }
                    let prefix = format!("{}/", ch.id);
                    if let Some(i) = ch.keys.iter().flatten().position(|k| format!("{prefix}{}", k.id) == key) {
                        return Some(format!("{at}/keys/{i}"));
                    }
                    if let Some(i) = ch.points.iter().flatten().position(|p| format!("{prefix}{}", p.id) == key) {
                        return Some(format!("{at}/points/{i}"));
                    }
                }
                pos(e.blocks.iter().flatten().position(|x| x.id.as_str() == key), "blocks")
                    .or_else(|| pos(e.placements.iter().flatten().position(|x| x.id.as_str() == key), "placements"))
                    .or_else(|| pos(e.sequences.iter().flatten().position(|x| x.id.as_str() == key), "sequences"))
            }),
        };
        pointer.map(|p| (loc.entry.clone(), ty, p)).ok_or_else(not_found)
    }

    /// The stored JSON of one object (as in its entry: inline children in full, split children as `{id, type}`
    /// stubs), found through the index and the typed entry, without serialising the rest of the entry.
    ///
    /// # Errors
    /// `ref.notFound` for an unknown key; `internal` if serialisation fails.
    pub fn object_raw_json(&self, key: &str) -> Result<(ObjectType, Value), ModelError> {
        let not_found = || {
            ModelError::with_details(
                codes::REF_NOT_FOUND,
                format!("`{key}` is not in the document"),
                serde_json::json!({ "ref": key }),
            )
        };
        let loc = self.locate(key).ok_or_else(not_found)?;
        let ty = loc.ty;
        let key = loc.key.as_str();
        let found: Option<Result<Value, ModelError>> = match &loc.entry {
            EntryKey::Project => {
                let p = &*self.project;
                if p.id.as_str() == key {
                    Some(to_value(p))
                } else if p.start_transition.id.as_str() == key {
                    Some(to_value(&p.start_transition))
                } else {
                    p.pens
                        .iter()
                        .find(|x| x.id.as_str() == key)
                        .map(to_value)
                        .or_else(|| p.assets.iter().find(|x| x.id.as_str() == key).map(to_value))
                }
            }
            EntryKey::Library => {
                let l = &*self.library;
                if l.id.as_str() == key { Some(to_value(l)) } else { find_library(&l.children, key) }
            }
            EntryKey::Palette(id) => self.palettes.get(id).and_then(|p| {
                if p.id.as_str() == key {
                    Some(to_value(&**p))
                } else {
                    p.styles
                        .iter()
                        .find(|x| x.id.as_str() == key)
                        .map(to_value)
                        .or_else(|| p.swatches.iter().find(|x| x.colour_id.as_str() == key).map(to_value))
                }
            }),
            EntryKey::Bank(id) => self.banks.get(id).filter(|b| b.id.as_str() == key).map(|b| to_value(&**b)),
            EntryKey::Drawing(id) => self.drawings.get(id).and_then(|d| {
                if d.id.as_str() == key {
                    Some(to_value(&**d))
                } else {
                    find_node(&d.nodes, &format!("{}/", d.id), key)
                }
            }),
            EntryKey::Scene(id) => self.scenes.get(id).and_then(|s| {
                if s.id.as_str() == key {
                    Some(to_value(&**s))
                } else if s.out_transition.id.as_str() == key {
                    Some(to_value(&s.out_transition))
                } else {
                    None
                }
            }),
            EntryKey::Element(id) => self.elements.get(id).and_then(|e| {
                if e.id.as_str() == key {
                    return Some(to_value(&**e));
                }
                for ch in e.channels.iter().flat_map(|m| m.values()) {
                    if ch.id.as_str() == key {
                        return Some(to_value(ch));
                    }
                    let prefix = format!("{}/", ch.id);
                    if let Some(k) = ch.keys.iter().flatten().find(|k| format!("{prefix}{}", k.id) == key) {
                        return Some(to_value(k));
                    }
                    if let Some(p) = ch.points.iter().flatten().find(|p| format!("{prefix}{}", p.id) == key) {
                        return Some(to_value(p));
                    }
                }
                e.blocks
                    .iter()
                    .flatten()
                    .find(|x| x.id.as_str() == key)
                    .map(to_value)
                    .or_else(|| e.placements.iter().flatten().find(|x| x.id.as_str() == key).map(to_value))
                    .or_else(|| e.sequences.iter().flatten().find(|x| x.id.as_str() == key).map(to_value))
            }),
        };
        found.ok_or_else(not_found)?.map(|v| (ty, v))
    }
}
