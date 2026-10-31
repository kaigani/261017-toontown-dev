//! The planar map in the executor (M2 C2.2, ADR-0017): the derived-map cache, the zones and fills of a drawing's
//! layers, and the settle pass every transaction goes through before it commits (02 §3.12 item 1: each affected layer
//! re-noded, zones recomputed, colours carried over).
//!
//! **Cache.** Derived maps are keyed by a SHA-256 of a layer's geometry (stroke IDs, kinds, closed flags and points —
//! fills do not change the map), shared by every invocation of the session, and bounded: at most [`CACHE_ENTRIES`]
//! maps and [`CACHE_COST`] vertices plus half-edges in total, least recently used first out (the order of use is a
//! function of the command sequence, so eviction is deterministic). It never changes results, only their cost.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tt_model::entities::drawing::{Drawing, DrawingNode, NodeKind};
use tt_registry::CommandError;
use tt_vector::path::GeomError;
use tt_vector::planar::LayerMap;
use tt_vector::planar::fill::{self, Fill};
use tt_vector::planar::layer;
use tt_vector::work::Work;

/// Most maps the cache keeps.
pub(crate) const CACHE_ENTRIES: usize = 512;

/// Most vertices plus half-edges the cached maps hold together.
pub(crate) const CACHE_COST: usize = 8_000_000;

/// The derived-map cache of a session.
#[derive(Debug, Default)]
pub(crate) struct Cache {
    entries: BTreeMap<[u8; 32], (Arc<LayerMap>, u64)>,
    cost: usize,
    tick: u64,
    /// Lookups answered from the cache.
    pub hits: u64,
    /// Maps built.
    pub misses: u64,
    /// Maps evicted.
    pub evictions: u64,
}

/// A cache shared by the invocations of a session.
pub(crate) type SharedCache = Arc<Mutex<Cache>>;

/// What of a layer's strokes the planar map depends on: identity, kind, closed flag, points, sides and the stored
/// brush exterior records.
pub(crate) fn planar_view(strokes: &[Value]) -> Vec<[Option<&Value>; 6]> {
    strokes
        .iter()
        .map(|s| {
            [
                s.get("id"),
                s.get("kind"),
                s.get("closed"),
                s.get("points"),
                s.get("sides"),
                s.pointer(&format!("/ext/{}", tt_vector::planar::fill::EXT_NAMESPACE)),
            ]
        })
        .collect()
}

/// The geometry key of a layer's strokes.
pub(crate) fn geometry_key(strokes: &[Value]) -> [u8; 32] {
    let mut h = Sha256::new();
    for s in strokes {
        for k in ["id", "kind", "closed", "points"] {
            h.update(s.get(k).map(Value::to_string).unwrap_or_default().as_bytes());
            h.update([0]);
        }
        h.update([1]);
    }
    h.finalize().into()
}

impl Cache {
    /// The map of a layer's strokes, built (and kept) when missing.
    pub(crate) fn map(&mut self, strokes: &[Value], work: &mut Work) -> Result<Arc<LayerMap>, GeomError> {
        let key = geometry_key(strokes);
        self.tick += 1;
        if let Some(e) = self.entries.get_mut(&key) {
            e.1 = self.tick;
            self.hits += 1;
            return Ok(e.0.clone());
        }
        let map = Arc::new(tt_vector::planar::build(&layer::inputs(strokes), work)?);
        self.misses += 1;
        self.insert(key, map.clone());
        Ok(map)
    }

    /// Keeps a map built elsewhere.
    pub(crate) fn insert(&mut self, key: [u8; 32], map: Arc<LayerMap>) {
        self.tick += 1;
        self.cost += map.cost();
        if let Some(old) = self.entries.insert(key, (map, self.tick)) {
            self.cost = self.cost.saturating_sub(old.0.cost());
        }
        while self.entries.len() > CACHE_ENTRIES || (self.cost > CACHE_COST && self.entries.len() > 1) {
            let Some((&k, _)) = self.entries.iter().min_by_key(|(_, (_, t))| *t) else { break };
            if let Some((m, _)) = self.entries.remove(&k) {
                self.cost = self.cost.saturating_sub(m.cost());
                self.evictions += 1;
            }
        }
    }
}

/// Locks a shared cache (a poisoned lock still holds a usable cache).
pub(crate) fn lock(c: &SharedCache) -> std::sync::MutexGuard<'_, Cache> {
    c.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A layer of a drawing: its ID, the JSON Pointer of the node and its strokes as stored.
#[derive(Debug, Clone)]
pub(crate) struct LayerRef {
    pub id: String,
    pub pointer: String,
    pub strokes: Vec<Value>,
}

fn walk(nodes: &[DrawingNode], at: &str, out: &mut Vec<LayerRef>) {
    for (i, n) in nodes.iter().enumerate() {
        let p = format!("{at}/{i}");
        if *n.kind() == NodeKind::Layer {
            let strokes: Vec<Value> =
                n.strokes().iter().flatten().filter_map(|s| serde_json::to_value(s).ok()).collect();
            out.push(LayerRef { id: n.id().as_str().to_owned(), pointer: p.clone(), strokes });
        }
        if let Some(children) = n.children() {
            walk(children, &format!("{p}/children"), out);
        }
    }
}

/// The layers of a drawing, back to front.
pub(crate) fn layers(d: &Drawing) -> Vec<LayerRef> {
    let mut out = Vec::new();
    walk(d.nodes(), "/nodes", &mut out);
    out
}

/// A layer's derived map with its zone fills.
#[derive(Debug, Clone)]
pub(crate) struct LayerZones {
    pub layer: LayerRef,
    pub map: Arc<LayerMap>,
    pub fills: Vec<Option<Fill>>,
}

/// The zones of every layer of a drawing.
pub(crate) fn drawing_zones(cache: &SharedCache, d: &Drawing) -> Result<Vec<LayerZones>, CommandError> {
    let mut work = Work::new(tt_vector::work::PLANAR_WORK_BUDGET, "computing the zones of this drawing");
    let mut out = Vec::new();
    for l in layers(d) {
        let map = lock(cache).map(&l.strokes, &mut work).map_err(|e| geom_error(&e))?;
        let fills = fill::read(&map, &layer::stored(&l.strokes)).fills;
        out.push(LayerZones { layer: l, map, fills });
    }
    Ok(out)
}

/// A geometry error as a command error (`limit.exceeded` keeps its code; a hint names what to reduce).
pub(crate) fn geom_error(e: &GeomError) -> CommandError {
    let mut err = crate::commands::draw::invalid_geometry(e);
    if e.code == "limit.exceeded" {
        err = err.hint("split the edit into smaller ones, or simplify the strokes of the layer");
    }
    err
}

/// Zone changes of a drawing between two documents: created, removed and repainted zone IDs.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ZoneDelta {
    pub created: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

/// The zone changes between two versions of a drawing (`None` = absent).
pub(crate) fn zone_delta(cache: &SharedCache, before: Option<&Drawing>, after: Option<&Drawing>) -> ZoneDelta {
    let fills = |d: Option<&Drawing>| -> BTreeMap<String, Option<Fill>> {
        let Some(d) = d else { return BTreeMap::new() };
        drawing_zones(cache, d)
            .map(|layers| {
                layers
                    .into_iter()
                    .flat_map(|l| l.map.zones.iter().map(|z| z.id.clone()).zip(l.fills.clone()).collect::<Vec<_>>())
                    .collect()
            })
            .unwrap_or_default()
    };
    let (b, a) = (fills(before), fills(after));
    let mut delta = ZoneDelta::default();
    for (id, f) in &a {
        match b.get(id) {
            None => delta.created.push(id.clone()),
            Some(old) if old != f => delta.modified.push(id.clone()),
            Some(_) => {}
        }
    }
    delta.removed = b.keys().filter(|k| !a.contains_key(*k)).cloned().collect();
    delta
}

/// A zone in the 02 §13.4 shape.
pub(crate) fn zone_json(lz: &LayerZones, z: usize) -> Value {
    let zone = &lz.map.zones[z];
    let fill = lz.fills.get(z).cloned().flatten();
    let ids = |list: &[Option<usize>]| -> Vec<Value> {
        list.iter().map(|o| o.map_or(Value::Null, |k| json!(lz.map.zones[k].id))).collect()
    };
    let holes: Vec<Value> =
        zone.holes.iter().map(|h| json!({ "boundaryStrokeIds": h.boundary, "zoneId": ids(&[h.zone])[0] })).collect();
    json!({
        "type": "zone",
        "id": zone.id,
        "colourId": fill.as_ref().and_then(|f| f.colour_id.clone()),
        "textureMatrix": fill.as_ref().and_then(|f| f.matrix),
        "boundaryStrokeIds": zone.boundary,
        "holes": holes,
        "parentZoneId": ids(&[zone.parent])[0],
        "area": zone.area,
        "bounds": zone.bounds.to_json(),
        "interiorPoint": { "x": zone.interior.x, "y": zone.interior.y },
    })
}

/// The zones on the left and right of a stroke, by ID.
pub(crate) fn side_zone_ids(lz: &LayerZones, stroke: &str) -> (Value, Value) {
    let Some(k) = lz.map.strokes.iter().position(|(id, _)| id == stroke) else { return (Value::Null, Value::Null) };
    let (l, r) = lz.map.stroke_sides(k);
    let id = |z: Option<usize>| z.map_or(Value::Null, |z| json!(lz.map.zones[z].id));
    (id(l), id(r))
}

/// Adds zones to the layers and side zone IDs to the pencil and invisible strokes of read-back nodes (nested or
/// flat).
pub(crate) fn annotate(nodes: &mut [Value], zones: &BTreeMap<String, LayerZones>) {
    for n in nodes.iter_mut() {
        let Some(obj) = n.as_object_mut() else { continue };
        match obj.get("type").and_then(Value::as_str) {
            Some("layer") => {
                let id = obj.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
                if let Some(lz) = zones.get(&id) {
                    let list: Vec<Value> = (0..lz.map.zones.len()).map(|z| zone_json(lz, z)).collect();
                    obj.insert("zones".into(), Value::Array(list));
                    if let Some(strokes) = obj.get_mut("strokes").and_then(Value::as_array_mut) {
                        for s in strokes {
                            side_ids(s, lz);
                        }
                    }
                }
            }
            Some("stroke") => {
                let parent = obj.get("parentId").and_then(Value::as_str).unwrap_or_default().to_owned();
                if let Some(lz) = zones.get(&parent) {
                    side_ids(n, lz);
                }
            }
            _ => {}
        }
        if let Some(children) = n.get_mut("children").and_then(Value::as_array_mut) {
            annotate(children, zones);
        }
    }
}

fn side_ids(s: &mut Value, lz: &LayerZones) {
    let kind = s.get("kind").and_then(Value::as_str).unwrap_or_default();
    if kind == "brush" {
        return;
    }
    let id = s.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
    let (l, r) = side_zone_ids(lz, &id);
    if let Some(o) = s.as_object_mut() {
        o.insert("leftZoneId".into(), l);
        o.insert("rightZoneId".into(), r);
    }
}

/// What settling an edit of one drawing did, for its result (02 §13.1 item 11, §3.12 item 4): the strokes the added
/// strokes became, strokes split by noding (all, and those that existed before the call), and zones created, removed
/// and repainted.
#[derive(Debug, Clone, Default)]
pub(crate) struct Settled {
    pub added: Vec<String>,
    pub split_all: Vec<String>,
    pub split_existing: Vec<String>,
    pub zones: ZoneDelta,
}

/// Settles the transaction and reports what it did to `drawing` relative to `before` (the document before the call);
/// `added` are the IDs of strokes the call added.
pub(crate) fn settled(
    cx: &mut crate::cx::Cx,
    before: &tt_model::document::Document,
    drawing: &tt_model::Id,
    added: &[String],
) -> Result<Settled, CommandError> {
    cx.settle()?;
    let after = cx.stamped_document()?;
    let added_final: Vec<String> = added.iter().flat_map(|a| cx.pieces_of(drawing, a)).collect();
    let split_all = cx.split_strokes(drawing);
    let existed = |s: &String| before.drawing(drawing).is_some_and(|d| tt_vector::drawing::locate(d, s).is_some());
    let split_existing: Vec<String> = split_all.iter().filter(|s| existed(s)).cloned().collect();
    let zones = zone_delta(&cx.planar_cache(), before.drawing(drawing), after.drawing(drawing));
    Ok(Settled { added: added_final, split_all, split_existing, zones })
}

impl Settled {
    /// Adds the outcome to a result's changed IDs.
    pub(crate) fn report(&self, changed: &mut crate::commands::draw::Changed) {
        changed.created_ids.retain(|c| !self.split_all.contains(c));
        for a in &self.added {
            if !changed.created_ids.contains(a) {
                changed.created_ids.push(a.clone());
            }
        }
        changed.created_ids.extend(self.zones.created.iter().cloned());
        changed.modified_ids.retain(|m| !self.split_all.contains(m));
        changed.modified_ids.extend(self.zones.modified.iter().cloned());
        for s in &self.split_existing {
            if !changed.removed_ids.contains(s) {
                changed.removed_ids.push(s.clone());
            }
        }
        changed.removed_ids.extend(self.zones.removed.iter().cloned());
    }
}

/// Paints the zones containing given points of given layers of a drawing as it stands staged (settling first), and
/// settles again so the fills are stored; returns the painted zone IDs.
pub(crate) fn paint_at(
    cx: &mut crate::cx::Cx,
    drawing: &tt_model::Id,
    targets: &[(String, tt_vector::geom::P, Fill)],
) -> Result<Vec<String>, CommandError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    cx.settle()?;
    let doc = cx.stamped_document()?;
    let Some(d) = doc.drawing(drawing) else { return Ok(Vec::new()) };
    let layers = drawing_zones(&cx.planar_cache(), d)?;
    let mut painted = Vec::new();
    let mut by_layer: BTreeMap<String, BTreeMap<String, Option<Fill>>> = BTreeMap::new();
    for (layer, p, fill) in targets {
        let Some(lz) = layers.iter().find(|l| &l.layer.id == layer) else { continue };
        if let Some(z) = lz.map.zone_at(*p) {
            let id = lz.map.zones[z].id.clone();
            by_layer.entry(layer.clone()).or_default().insert(id.clone(), Some(fill.clone()));
            painted.push(id);
        }
    }
    for (layer, fills) in by_layer {
        cx.paint_zones(drawing, &layer, fills);
    }
    cx.settle()?;
    Ok(painted)
}
