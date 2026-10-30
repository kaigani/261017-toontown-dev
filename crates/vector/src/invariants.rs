//! The topology invariants of 02 §3.12 that hold without the planar map, as checkable properties (used by the
//! property tests of `tt-vector` and of the executor's drawing commands):
//!
//! - **Identity.** Node and stroke IDs are unique within the drawing (ID-03), and an edit keeps the ID and the
//!   content of every object it does not touch ([`preserved`]; item 4).
//! - **Geometry.** Every path has the anchors its kind needs, finite coordinates within range, brush outlines are
//!   closed, spines and pressure maps are well formed (§3.2), pencil lines have a line style (10 §4.9).
//! - **Text.** Runs cover the text consecutively and there is one paragraph per line (§3.7).
//! - **Revision.** A content change raises the drawing revision (item 4; checked by the callers, who see revisions).
//!
//! Re-noding and zones (items 1–2) are the planar map's (M2 C2.2).

use std::collections::BTreeMap;

use tt_model::entities::drawing::{Drawing, NodeKind, StrokeKind};

use crate::drawing::{self as vd, ObjectKind};
use crate::fit::check_pressure;
use crate::path::Path;

/// A violated invariant: the object, the rule and a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Local ID of the object (empty for the drawing).
    pub object: String,
    /// The rule.
    pub rule: &'static str,
    /// What is wrong.
    pub message: String,
}

fn v(object: &str, rule: &'static str, message: impl Into<String>) -> Violation {
    Violation { object: object.to_owned(), rule, message: message.into() }
}

/// Checks one drawing against the invariants above.
#[must_use]
pub fn check(d: &Drawing) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for o in vd::objects(d) {
        *seen.entry(o.id.clone()).or_default() += 1;
        match o.kind {
            ObjectKind::Stroke => {
                let Some(s) = vd::stroke_at(d, &o.pointer) else { continue };
                let p = Path::from_points(s.points(), *s.closed());
                if let Err(e) = p.check("/points") {
                    out.push(v(&o.id, "geometry", e.message));
                }
                match s.kind() {
                    StrokeKind::Brush => {
                        if !*s.closed() {
                            out.push(v(&o.id, "brushClosed", "brush outlines are closed"));
                        }
                        if let Some(sp) = s.spine() {
                            if let Err(e) = Path::from_points(&sp.points, sp.points.len() == 1).check("/spine/points") {
                                out.push(v(&o.id, "spine", e.message));
                            }
                            if let Err(e) = check_pressure(&sp.pressure, "/spine/pressure") {
                                out.push(v(&o.id, "pressure", e.message));
                            }
                        }
                    }
                    StrokeKind::Pencil => {
                        if s.line_style().is_none() {
                            out.push(v(&o.id, "lineStyle", "pencil lines have a line style"));
                        }
                    }
                    StrokeKind::Invisible => {
                        if s.colour_id().is_some() || s.line_style().is_some() {
                            out.push(v(&o.id, "invisible", "invisible strokes have no colour or line style"));
                        }
                    }
                }
            }
            ObjectKind::Text => {
                let Some(n) = vd::node_at(d, &o.pointer) else { continue };
                let (Some(text), Some(runs), Some(paras)) = (n.text(), n.runs(), n.paragraphs()) else {
                    out.push(v(&o.id, "text", "a text object has text, runs and paragraphs"));
                    continue;
                };
                let len = text.chars().count() as u64;
                let mut pos = 0u64;
                for r in runs {
                    if u64::from(r.start) != pos {
                        out.push(v(&o.id, "runsCover", "runs cover the text consecutively"));
                    }
                    pos += u64::from(r.length);
                }
                if pos != len {
                    out.push(v(&o.id, "runsCover", "runs cover the whole text"));
                }
                let lines = text.chars().filter(|c| *c == '\n').count() + 1;
                if paras.len() != lines {
                    out.push(v(&o.id, "paragraphs", format!("{} paragraphs for {lines} lines", paras.len())));
                }
            }
            ObjectKind::Layer | ObjectKind::Group | ObjectKind::Bitmap => {
                let Some(n) = vd::node_at(d, &o.pointer) else { continue };
                if *n.kind() == NodeKind::Bitmap && (n.image().is_none() || n.transform().is_none()) {
                    out.push(v(&o.id, "bitmap", "a bitmap has pixels and a placement"));
                }
            }
        }
    }
    for (id, n) in seen {
        if n > 1 {
            out.push(v(&id, "unique", format!("the ID is used {n} times in the drawing")));
        }
    }
    out
}

/// The persisted JSON of every object of a drawing by local ID (children reduced to their IDs, so a child's change
/// does not count as its parent's).
#[must_use]
pub fn own_views(d: &Drawing) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    for o in vd::objects(d) {
        let mut j = if o.kind == ObjectKind::Stroke {
            vd::stroke_at(d, &o.pointer).and_then(|s| serde_json::to_value(s).ok())
        } else {
            vd::node_at(d, &o.pointer).and_then(|n| serde_json::to_value(n).ok())
        };
        if let Some(serde_json::Value::Object(m)) = j.as_mut() {
            m.remove("rev");
            for k in ["strokes", "children"] {
                if let Some(serde_json::Value::Array(a)) = m.get_mut(k) {
                    for c in a.iter_mut() {
                        *c = c.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    }
                }
            }
        }
        if let Some(j) = j {
            out.insert(o.id, j);
        }
    }
    out
}

/// 02 §3.12 item 4: every object of `before` that is not in `touched` is in `after` with the same ID and the same
/// own content. Returns the IDs that changed or disappeared although untouched.
#[must_use]
pub fn preserved(before: &Drawing, after: &Drawing, touched: &[String]) -> Vec<String> {
    let a = own_views(before);
    let b = own_views(after);
    a.iter()
        .filter(|(id, _)| !touched.iter().any(|t| t == *id))
        .filter(|(id, view)| b.get(*id) != Some(view))
        .map(|(id, _)| id.clone())
        .collect()
}
