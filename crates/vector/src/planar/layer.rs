//! Settling one layer after an edit (02 §3.12 item 1): persistent noding of the strokes the edit changed, the derived
//! map, fill carry-over (or the staged fills when no geometry changed), fills a command painted, and the stroke sides
//! and brush exterior records that store the result. Works on the persisted stroke JSON (10 §4.9) so the executor
//! can stage the outcome as one patch.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::fill::{self, EXT_NAMESPACE, Exterior, Fill, SideFills, Stored};
use super::graph::{self, EdgeKind, LayerMap, StrokeIn};
use super::node::{self, NodeStroke, Outcome};
use crate::path::{GeomError, Path};
use crate::work::Work;

/// Why settling failed.
#[derive(Debug)]
pub enum SettleError<E> {
    /// Bounded work exceeded, or invalid geometry.
    Geom(GeomError),
    /// The ID source failed.
    Id(E),
}

impl<E> From<GeomError> for SettleError<E> {
    fn from(e: GeomError) -> Self {
        SettleError::Geom(e)
    }
}

/// The settled layer.
#[derive(Debug, Clone)]
pub struct Settled {
    /// The layer's strokes as they are to be stored.
    pub strokes: Vec<Value>,
    /// Whether they differ from the staged strokes.
    pub changed: bool,
    /// IDs of strokes created by noding (pieces).
    pub created: Vec<String>,
    /// IDs of strokes replaced by their pieces.
    pub removed: Vec<String>,
    /// The derived map of the settled layer.
    pub map: LayerMap,
    /// Zone fills of the settled layer, aligned with `map.zones`.
    pub fills: Vec<Option<Fill>>,
    /// Zones of the layer before the edit with their fills.
    pub zones_before: Vec<(String, Option<Fill>)>,
    /// Zones whose bounding strokes disagreed (repaired).
    pub inconsistent: usize,
}

fn kind_of(s: &Value) -> EdgeKind {
    match s.get("kind").and_then(Value::as_str) {
        Some("brush") => EdgeKind::Brush,
        Some("invisible") => EdgeKind::Invisible,
        _ => EdgeKind::Pencil,
    }
}

fn id_of(s: &Value) -> String {
    s.get("id").and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// The path of a stored stroke (a brush stroke's outline, other strokes' centre path).
#[must_use]
pub fn path_of(s: &Value) -> Path {
    let points: Vec<[f64; 6]> = s
        .get("points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let v: Vec<f64> = p.as_array()?.iter().filter_map(Value::as_f64).collect();
                    <[f64; 6]>::try_from(v).ok()
                })
                .collect()
        })
        .unwrap_or_default();
    Path::from_points(&points, s.get("closed").and_then(Value::as_bool).unwrap_or(false))
}

fn geometry_eq(a: &Value, b: &Value) -> bool {
    a.get("kind") == b.get("kind") && a.get("closed") == b.get("closed") && a.get("points") == b.get("points")
}

/// The planar inputs of a layer's stored strokes.
#[must_use]
pub fn inputs(strokes: &[Value]) -> Vec<StrokeIn> {
    strokes.iter().map(|s| StrokeIn { id: id_of(s), kind: kind_of(s), path: path_of(s) }).collect()
}

fn fill_of(v: &Value) -> Option<Fill> {
    if v.is_null() {
        return None;
    }
    let colour_id = v.get("colourId").and_then(Value::as_str).map(str::to_owned);
    let matrix = v.get("matrix").and_then(Value::as_array).and_then(|a| {
        let m: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
        <[f64; 6]>::try_from(m).ok()
    });
    Some(Fill { colour_id, matrix })
}

fn fill_json(f: &Option<Fill>) -> Value {
    match f {
        Some(f) if f.painted() => json!({ "colourId": f.colour_id, "matrix": f.matrix }),
        _ => Value::Null,
    }
}

/// The fills stored on a layer's strokes.
#[must_use]
pub fn stored(strokes: &[Value]) -> Stored {
    let mut out = Stored::default();
    for s in strokes {
        let sides = s.get("sides");
        out.sides.push(SideFills {
            left: sides.and_then(|x| x.get("left")).and_then(fill_of),
            right: sides.and_then(|x| x.get("right")).and_then(fill_of),
        });
        let ext: Vec<Exterior> = s
            .pointer(&format!("/ext/{EXT_NAMESPACE}/exterior"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        Some(Exterior {
                            left: r.get("side")?.as_str()? == "left",
                            s0: r.get("from")?.as_f64()?,
                            s1: r.get("to")?.as_f64()?,
                            fill: fill_of(r.get("fill")?)?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.exterior.push(ext);
    }
    out
}

/// Writes stored fills into stroke JSON (sides of every stroke; exterior records of brush strokes, the extension key
/// removed when there are none).
fn apply_stored(strokes: &mut [Value], st: &Stored) {
    for (k, s) in strokes.iter_mut().enumerate() {
        let Some(obj) = s.as_object_mut() else { continue };
        let sides = st.sides.get(k).cloned().unwrap_or_default();
        obj.insert("sides".into(), json!({ "left": fill_json(&sides.left), "right": fill_json(&sides.right) }));
        let records: Vec<Value> = st
            .exterior
            .get(k)
            .map(|list| {
                list.iter()
                    .map(|r| {
                        json!({
                            "side": if r.left { "left" } else { "right" },
                            "from": graph::key_param(r.s0) as f64 / 1e6,
                            "to": graph::key_param(r.s1) as f64 / 1e6,
                            "fill": fill_json(&Some(r.fill.clone())),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let ext = obj.entry("ext").or_insert_with(|| Value::Object(Map::new()));
        if let Some(e) = ext.as_object_mut() {
            if records.is_empty() {
                e.remove(EXT_NAMESPACE);
            } else {
                e.insert(EXT_NAMESPACE.into(), json!({ "exterior": records }));
            }
        }
        if obj.get("ext").and_then(Value::as_object).is_some_and(Map::is_empty) {
            obj.remove("ext");
        }
    }
}

/// Settles a layer: `before` is the layer's strokes before the edit (`None` for a new layer), `after` the staged
/// strokes, `before_map` the derived map of `before` when the caller has it cached, `painted` fills a command set by
/// zone ID of the settled map, and `fresh` gives IDs for stroke pieces.
///
/// # Errors
/// `limit.exceeded` past the work budget (nothing is to be staged then); the ID source's error.
pub fn settle<E>(
    before: Option<&[Value]>,
    after: &[Value],
    before_map: Option<&LayerMap>,
    painted: &BTreeMap<String, Option<Fill>>,
    fresh: &mut dyn FnMut() -> Result<String, E>,
    work: &mut Work,
) -> Result<Settled, SettleError<E>> {
    let before_by_id: BTreeMap<String, &Value> = before.unwrap_or_default().iter().map(|s| (id_of(s), s)).collect();
    let after_ids: BTreeSet<String> = after.iter().map(id_of).collect();
    let geometry_changed = before.is_none_or(|b| {
        b.len() != after.len()
            || after.iter().any(|s| before_by_id.get(&id_of(s)).is_none_or(|old| !geometry_eq(old, s)))
            || before_by_id.keys().any(|k| !after_ids.contains(k))
    });
    // Persistent noding of what the edit changed.
    let mut strokes: Vec<Value> = after.to_vec();
    let mut created = Vec::new();
    let mut removed = Vec::new();
    if geometry_changed {
        let input: Vec<NodeStroke> = after
            .iter()
            .map(|s| NodeStroke {
                id: id_of(s),
                kind: kind_of(s),
                path: path_of(s),
                changed: before_by_id.get(&id_of(s)).is_none_or(|old| !geometry_eq(old, s)),
            })
            .collect();
        let noding = node::node(&input, work)?;
        let mut out: Vec<Value> = Vec::with_capacity(after.len());
        for (i, s) in after.iter().enumerate() {
            match noding.outcomes.get(&i) {
                None => out.push(s.clone()),
                Some(Outcome::Moved(p)) => {
                    let mut v = s.clone();
                    if let Some(o) = v.as_object_mut() {
                        o.insert("points".into(), p.points_json());
                    }
                    out.push(v);
                }
                Some(Outcome::Split(pieces)) => {
                    removed.push(id_of(s));
                    for p in pieces {
                        let id = fresh().map_err(SettleError::Id)?;
                        let mut v = s.clone();
                        if let Some(o) = v.as_object_mut() {
                            o.insert("id".into(), json!(id));
                            o.insert("closed".into(), json!(false));
                            o.insert("points".into(), p.points_json());
                        }
                        created.push(id);
                        out.push(v);
                    }
                }
            }
        }
        strokes = out;
    }
    let map = graph::build(&inputs(&strokes), work)?;
    let owned: Option<LayerMap> = match (before, before_map) {
        (Some(b), None) => Some(graph::build(&inputs(b), work)?),
        _ => None,
    };
    let old_map: Option<&LayerMap> = before_map.or(owned.as_ref());
    let before_fills: Vec<Option<Fill>> = match (before, old_map) {
        (Some(b), Some(m)) => fill::read(m, &stored(b)).fills,
        _ => Vec::new(),
    };
    let staged = fill::read(&map, &stored(&strokes));
    let mut fills = match old_map {
        Some(m) if geometry_changed => fill::carry(m, &before_fills, &map, work)?,
        _ => staged.fills.clone(),
    };
    for (z, zone) in map.zones.iter().enumerate() {
        if let Some(f) = painted.get(&zone.id) {
            fills[z] = f.clone().filter(Fill::painted);
        }
    }
    let st = fill::write(&map, &fills);
    apply_stored(&mut strokes, &st);
    let changed = strokes.as_slice() != after;
    let zones_before =
        old_map.map(|m| m.zones.iter().map(|z| z.id.clone()).zip(before_fills).collect()).unwrap_or_default();
    Ok(Settled {
        strokes,
        changed,
        created,
        removed,
        map,
        fills,
        zones_before,
        inconsistent: if geometry_changed { 0 } else { staged.inconsistent.len() },
    })
}
