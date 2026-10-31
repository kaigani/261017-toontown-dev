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

use serde_json::Value;
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

    /// Entries and total cost held.
    pub(crate) fn size(&self) -> (usize, usize) {
        (self.entries.len(), self.cost)
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
