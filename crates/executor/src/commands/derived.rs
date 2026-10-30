//! Derived properties for `doc.object.get {include: ["derived"]}` (10 §2.7, AG-04; C1.1 handoff gap 3).
//!
//! Each property is computed from the persisted data with the name and shape the type schema declares (`x-derived`).
//! Properties whose inputs belong to later milestones are not invented: they are left out and named, with their
//! owner, in a `doc.derivedUnavailable` warning (ADR-0013 §9).

use serde_json::{Map, Value, json};
use tt_model::document::Document;
use tt_model::entities::drawing::DrawingNode;
use tt_model::entities::scene::TreeNode;
use tt_model::{Id, ObjectType};

/// Derived properties not computed yet, with their owners.
pub(crate) fn unavailable(ty: ObjectType) -> &'static [(&'static str, &'static str)] {
    match ty {
        ObjectType::Project => &[
            ("sceneStartFrames", "M9 (storyline timing)"),
            ("globalLength", "M9 (storyline timing)"),
            ("thumbnail", "M3 C3.3"),
        ],
        ObjectType::Scene => &[("paddedLength", "M9 (storyline timing)")],
        ObjectType::Element => &[("effectiveMarkerColour", "M4 C4.1 (preferences)")],
        ObjectType::Drawing => &[("thumbnail", "M3 C3.3"), ("zones", "M2 C2.2")],
        ObjectType::Asset => &[("info", "M8"), ("refs", "M1 C1.3"), ("status", "M1 C1.3")],
        ObjectType::SoundPlacement => &[("naturalEndFrame", "M8")],
        ObjectType::MediaSequence => &[("sourceLength", "M10"), ("missing", "M10")],
        ObjectType::LibraryItem => &[("broken", "M9")],
        _ => &[],
    }
}

fn find_in_tree<'a>(
    nodes: &'a [TreeNode],
    id: &Id,
    parent: Option<&'a Id>,
    depth: usize,
) -> Option<(Option<&'a Id>, &'a TreeNode, usize)> {
    for n in nodes {
        if &n.id == id {
            return Some((parent, n, depth));
        }
        if let Some(found) = find_in_tree(&n.children, id, Some(&n.id), depth + 1) {
            return Some(found);
        }
    }
    None
}

fn ancestors_hidden(doc: &Document, nodes: &[TreeNode], id: &Id) -> bool {
    fn path<'a>(nodes: &'a [TreeNode], id: &Id, acc: &mut Vec<&'a Id>) -> bool {
        for n in nodes {
            if &n.id == id {
                return true;
            }
            acc.push(&n.id);
            if path(&n.children, id, acc) {
                return true;
            }
            acc.pop();
        }
        false
    }
    let mut acc = Vec::new();
    path(nodes, id, &mut acc);
    acc.iter().any(|a| doc.element(a).is_some_and(|e| !*e.visible_in_scene()))
}

fn node_has_content(nodes: &[DrawingNode]) -> bool {
    nodes.iter().any(|n| {
        n.strokes().as_ref().is_some_and(|s| !s.is_empty())
            || matches!(
                n.kind(),
                tt_model::entities::drawing::NodeKind::Text | tt_model::entities::drawing::NodeKind::Bitmap
            )
            || n.children().as_deref().is_some_and(node_has_content)
    })
}

/// Frame ranges where an element exposes a drawing.
fn ranges(doc: &Document, element: &Id, drawing: &Id) -> Vec<Value> {
    let Some(runs) = doc.element(element).and_then(|e| e.exposure().clone()) else { return Vec::new() };
    let mut out = Vec::new();
    let mut at: u64 = 1;
    for (d, count) in runs {
        let end = at + u64::from(count) - 1;
        if d.as_ref() == Some(drawing) {
            match out.last_mut() {
                Some(Value::Object(m)) if m.get("to").and_then(Value::as_u64) == Some(at - 1) => {
                    m.insert("to".into(), json!(end));
                }
                _ => out.push(json!({ "from": at, "to": end })),
            }
        }
        at = end + 1;
    }
    out
}

/// Adds the computable derived properties of an object to its JSON; returns the names left out with their owners.
pub(crate) fn add(
    doc: &Document,
    ty: ObjectType,
    key: &str,
    obj: &mut Map<String, Value>,
) -> Vec<(&'static str, &'static str)> {
    match ty {
        ObjectType::Scene => {
            if let Some(s) = Id::parse(key).ok().and_then(|id| doc.scene(&id)) {
                obj.insert("length".into(), json!(doc.scene_length(s).max(1)));
            }
        }
        ObjectType::Element => {
            let Ok(id) = Id::parse(key) else { return unavailable(ty).to_vec() };
            let Some(el) = doc.element(&id) else { return unavailable(ty).to_vec() };
            if let Some(scene) = doc.scene_of(&id).and_then(|s| doc.scene(s)) {
                if let Some((parent, node, depth)) = find_in_tree(scene.tree(), &id, None, 0) {
                    obj.insert("parentId".into(), json!(parent.map(Id::as_str)));
                    obj.insert(
                        "childIds".into(),
                        json!(node.children.iter().map(|c| c.id.as_str()).collect::<Vec<_>>()),
                    );
                    obj.insert("depth".into(), json!(depth));
                }
                let order = doc.tree_order(scene);
                if let Some(i) = order.iter().position(|x| x == &id) {
                    obj.insert("timelineIndex".into(), json!(i));
                }
                obj.insert("xsheetIndex".into(), json!(scene.column_order().iter().position(|x| x == &id)));
                obj.insert("hiddenByParent".into(), json!(ancestors_hidden(doc, scene.tree(), &id)));
            }
            let bank = el.drawing_bank_id().clone();
            let users: Vec<&Id> = bank.as_ref().map(|b| doc.bank_users(b).iter().collect()).unwrap_or_default();
            let shared = users.len() > 1;
            obj.insert("cloneGroupId".into(), json!(if shared { bank.as_ref().map(Id::as_str) } else { None }));
            obj.insert(
                "cloneIds".into(),
                json!(users.iter().filter(|u| **u != &id).map(|u| u.as_str()).collect::<Vec<_>>()),
            );
            obj.insert(
                "drawingCount".into(),
                json!(bank.as_ref().and_then(|b| doc.bank(b)).map_or(0, |b| b.drawings().ids().len())),
            );
        }
        ObjectType::DrawingBank => {
            if let Ok(id) = Id::parse(key) {
                obj.insert("elementIds".into(), json!(doc.bank_users(&id).iter().map(Id::as_str).collect::<Vec<_>>()));
            }
        }
        ObjectType::Drawing => {
            if let Some(d) = Id::parse(key).ok().and_then(|id| doc.drawing(&id)) {
                let usage: Vec<Value> = doc
                    .bank_of(d.id())
                    .map(|b| doc.bank_users(b).to_vec())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|e| {
                        let r = ranges(doc, e, d.id());
                        (!r.is_empty()).then(|| json!({ "elementId": e.as_str(), "ranges": r }))
                    })
                    .collect();
                obj.insert("usage".into(), Value::Array(usage));
                obj.insert("isEmpty".into(), json!(!node_has_content(d.nodes())));
            }
        }
        ObjectType::ParticleBlock => {
            if let Some(loc) = doc.locate(key)
                && let Some(el) = loc.parent.as_deref().and_then(|p| Id::parse(p).ok()).and_then(|p| doc.element(&p))
                && let Some(i) = el.blocks().iter().flatten().position(|b| b.id().as_str() == key)
            {
                obj.insert("index".into(), json!(i));
            }
        }
        ObjectType::Library => {
            obj.insert("readOnly".into(), json!(false));
            obj.insert("writeAccessHeldBy".into(), Value::Null);
            obj.insert("location".into(), Value::Null);
        }
        _ => {}
    }
    unavailable(ty).to_vec()
}
