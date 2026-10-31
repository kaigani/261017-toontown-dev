//! Painting commands (03 §8, §16; 02 §13.1 item 3; M2 C2.2): paint, paint unpainted, unpaint, the dropper read-back,
//! close gap, the automatic gap-closing setting, and the hit test of 02 §13.5 that shares their point rules.
//!
//! Targets are object IDs (zones, pencil lines, brush strokes, text) or points in drawing space; a press–drag is a
//! crossing `path`, a `lasso` or (SI-061) a `rect`. Point targets follow 03 §8.1: layers front to back, and within a
//! layer lines before brush strokes before zones (a point within a line's rendered thickness hits the line). Zone fills
//! are colour IDs plus mapping matrices (03 §1.6, §7.2), stored on the bounding strokes by the settle pass. Protected
//! colours (SI-061, swatch `ext["dev.toontown.paint"].protected`) are not painted over unless
//! `respectProtected: false`; refusals are reported per target. A command that paints nothing fails with the reason
//! of its first refusal; one that paints some targets reports the others in `skipped` (03 §16.1 item 8).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::drawing::{Drawing, DrawingNode, NodeKind};
use tt_model::entities::palette::SwatchKind;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};
use tt_vector::geom::{P, Rect};
use tt_vector::planar::fill::Fill;
use tt_vector::planar::gap;
use tt_vector::work::Work;

use super::draw::{Changed, Point, check_editable, committed_rev, err, op, target};
use crate::cx::Cx;
use crate::planar::{LayerZones, drawing_zones};

/// The extension namespace of painting data on swatches (SI-061).
pub(crate) const PAINT_EXT: &str = "dev.toontown.paint";

/// The preference holding the automatic gap-closing mode (03 §8.8: a persisted user setting).
pub(crate) const GAP_PREF: &str = "paint.gapClosing";

/// Coordinate space of point targets (03 §16.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Space {
    /// Drawing space of the target drawing.
    #[default]
    Drawing,
    /// Scene space at a frame (M3 C3.2).
    Scene,
    /// A view's pixels (M4 C4.2).
    View,
}

/// Gap-closing choice of a paint command (03 §8.8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GapChoice {
    /// The user setting.
    #[default]
    Current,
    /// No gap closing.
    Off,
    /// Small (3 view px).
    Small,
    /// Medium (8 view px).
    Medium,
    /// Large (16 view px).
    Large,
}

/// The automatic gap-closing mode (03 §8.8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GapMode {
    /// Off (default).
    #[default]
    Off,
    /// Small.
    Small,
    /// Medium.
    Medium,
    /// Large.
    Large,
}

impl GapMode {
    fn tolerance(self) -> Option<f64> {
        match self {
            GapMode::Off => None,
            GapMode::Small => Some(gap::TOLERANCE_PX[0]),
            GapMode::Medium => Some(gap::TOLERANCE_PX[1]),
            GapMode::Large => Some(gap::TOLERANCE_PX[2]),
        }
    }

    fn of(v: Option<&Value>) -> Self {
        match v.and_then(Value::as_str) {
            Some("small") => GapMode::Small,
            Some("medium") => GapMode::Medium,
            Some("large") => GapMode::Large,
            _ => GapMode::Off,
        }
    }
}

fn gap_tolerance(cx: &Cx, choice: GapChoice) -> Option<f64> {
    match choice {
        GapChoice::Current => GapMode::of(cx.session().env.pref(GAP_PREF).as_ref()).tolerance(),
        GapChoice::Off => None,
        GapChoice::Small => GapMode::Small.tolerance(),
        GapChoice::Medium => GapMode::Medium.tolerance(),
        GapChoice::Large => GapMode::Large.tolerance(),
    }
}

fn check_space(space: Space) -> Result<(), CommandError> {
    match space {
        Space::Drawing => Ok(()),
        Space::Scene => Err(err(
            codes::ARG_INVALID,
            "scene-space point targets need the scene transforms (M3 C3.2)",
            json!({ "pointer": "/space", "space": "scene" }),
        )
        .hint("give points in drawing space (space: \"drawing\")")),
        Space::View => Err(err(
            codes::ARG_INVALID,
            "view-space point targets need the desktop views (M4 C4.2)",
            json!({ "pointer": "/space", "space": "view" }),
        )
        .hint("give points in drawing space (space: \"drawing\")")),
    }
}

fn point(p: Point, pointer: &str) -> Result<P, CommandError> {
    for (v, k) in [(p.x, "x"), (p.y, "y")] {
        tt_vector::path::check_coord(v, &format!("{pointer}/{k}")).map_err(|e| super::draw::invalid_geometry(&e))?;
    }
    Ok(P::new(p.x, p.y))
}

/// A swatch's kind and whether it is protected (SI-061), by colour ID.
fn swatch_info(doc: &Document, colour: &str) -> Option<(SwatchKind, bool)> {
    doc.palettes().flat_map(|p| p.swatches().iter()).find(|s| s.colour_id().as_str() == colour).map(|s| {
        let protected = serde_json::to_value(s)
            .ok()
            .and_then(|v| v.pointer(&format!("/ext/{PAINT_EXT}/protected")).and_then(Value::as_bool))
            .unwrap_or(false);
        (*s.kind(), protected)
    })
}

/// The default fill mapping of a gradient or texture over a box (03 §7.2): unit space `[0, 1]²` onto the box (linear
/// axis left → right, texture stretched); radial centred on the box, unit radius ½ reaching its corners (SI-064).
pub(crate) fn default_matrix(kind: SwatchKind, b: &Rect) -> Option<[f64; 6]> {
    let (w, h) = (b.max_x - b.min_x, b.max_y - b.min_y);
    match kind {
        SwatchKind::Solid => None,
        SwatchKind::LinearGradient | SwatchKind::Texture => Some([w, 0.0, 0.0, h, b.min_x, b.min_y]),
        SwatchKind::RadialGradient => {
            let r = (w * w + h * h).sqrt() / 2.0;
            let (cx, cy) = ((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0);
            Some([2.0 * r, 0.0, 0.0, 2.0 * r, cx - r, cy - r])
        }
    }
}

/// The fill mapping of a press–drag (03 §7.2): linear from the press (0) to the release (1); radial centred at the
/// press with the drag as radius; texture origin at the press, width = drag length, rotated to the drag (SI-064).
pub(crate) fn drag_matrix(kind: SwatchKind, from: P, to: P) -> Option<[f64; 6]> {
    let d = to - from;
    match kind {
        SwatchKind::Solid => None,
        SwatchKind::LinearGradient => Some([d.x, d.y, -d.y, d.x, from.x + 0.5 * d.y, from.y - 0.5 * d.x]),
        SwatchKind::RadialGradient => {
            let r = d.len();
            Some([2.0 * r, 0.0, 0.0, 2.0 * r, from.x - r, from.y - r])
        }
        SwatchKind::Texture => Some([d.x, d.y, -d.y, d.x, from.x, from.y]),
    }
}

// ----- hit testing ------------------------------------------------------------------------------------------------

/// What a point hits.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HitKind {
    /// A pencil line.
    Pencil,
    /// A brush stroke.
    Brush,
    /// An invisible stroke.
    Invisible,
    /// A zone (layer index into the drawing's zones, zone index).
    Zone(usize, usize),
    /// A text object.
    Text,
    /// A bitmap.
    Bitmap,
}

impl HitKind {
    fn as_str(&self) -> &'static str {
        match self {
            HitKind::Pencil => "pencil",
            HitKind::Brush => "brush",
            HitKind::Invisible => "invisible",
            HitKind::Zone(..) => "zone",
            HitKind::Text => "text",
            HitKind::Bitmap => "bitmap",
        }
    }
}

/// One object under a point.
#[derive(Debug, Clone)]
pub(crate) struct Hit {
    pub id: String,
    pub kind: HitKind,
    pub layer: Option<String>,
    pub group_path: Vec<String>,
    pub colour: Option<String>,
}

fn walk_nodes<'a>(nodes: &'a [DrawingNode], path: &[String], out: &mut Vec<(&'a DrawingNode, Vec<String>)>) {
    for n in nodes {
        out.push((n, path.to_vec()));
        if let Some(children) = n.children() {
            let mut p = path.to_vec();
            p.push(n.id().as_str().to_owned());
            walk_nodes(children, &p, out);
        }
    }
}

/// Every object under `p`, front to back (03 §8.1): `tolerance` widens line hits (du).
pub(crate) fn hits(
    d: &Drawing,
    zones: &[LayerZones],
    p: P,
    tolerance: f64,
    include_invisible: bool,
) -> Result<Vec<Hit>, CommandError> {
    let mut work = Work::new(tt_vector::work::PLANAR_WORK_BUDGET, "hit testing this drawing");
    let mut ordered = Vec::new();
    walk_nodes(d.nodes(), &[], &mut ordered);
    let mut out = Vec::new();
    for (n, path) in ordered.into_iter().rev() {
        match n.kind() {
            NodeKind::Layer => {
                let lid = n.id().as_str().to_owned();
                let strokes = n.strokes().clone().unwrap_or_default();
                let mut lines = Vec::new();
                let mut brushes = Vec::new();
                for s in strokes.iter().rev() {
                    let path_s = tt_vector::path::Path::from_points(s.points(), *s.closed());
                    let colour = s.colour_id().as_ref().map(|c| c.as_str().to_owned());
                    let hit = |kind| Hit {
                        id: s.id().as_str().to_owned(),
                        kind,
                        layer: Some(lid.clone()),
                        group_path: path.clone(),
                        colour: colour.clone(),
                    };
                    match s.kind() {
                        tt_model::entities::drawing::StrokeKind::Brush => {
                            if gap::inside(&path_s, p, &mut work).map_err(|e| crate::planar::geom_error(&e))? {
                                brushes.push(hit(HitKind::Brush));
                            }
                        }
                        k => {
                            let invisible = *k == tt_model::entities::drawing::StrokeKind::Invisible;
                            if invisible && !include_invisible {
                                continue;
                            }
                            let half = s.line_style().as_ref().map_or(0.0, |l| l.thickness / 2.0);
                            let dist =
                                gap::distance_to(&path_s, p, &mut work).map_err(|e| crate::planar::geom_error(&e))?;
                            if dist <= half + tolerance {
                                lines.push(hit(if invisible { HitKind::Invisible } else { HitKind::Pencil }));
                            }
                        }
                    }
                }
                out.extend(lines);
                out.extend(brushes);
                if let Some(li) = zones.iter().position(|z| z.layer.id == lid)
                    && let Some(z) = zones[li].map.zone_at(p)
                {
                    out.push(Hit {
                        id: zones[li].map.zones[z].id.clone(),
                        kind: HitKind::Zone(li, z),
                        layer: Some(lid),
                        group_path: path.clone(),
                        colour: zones[li].fills.get(z).cloned().flatten().and_then(|f| f.colour_id),
                    });
                }
            }
            NodeKind::Text | NodeKind::Bitmap => {
                if let Some(b) = tt_vector::drawing::node_bounds(n, false)
                    && p.x >= b.min_x
                    && p.x <= b.max_x
                    && p.y >= b.min_y
                    && p.y <= b.max_y
                {
                    let text = *n.kind() == NodeKind::Text;
                    let colour = if text {
                        n.runs()
                            .as_ref()
                            .and_then(|r| r.first())
                            .and_then(|r| r.colour_id.as_ref())
                            .map(|c| c.as_str().to_owned())
                    } else {
                        None
                    };
                    out.push(Hit {
                        id: n.id().as_str().to_owned(),
                        kind: if text { HitKind::Text } else { HitKind::Bitmap },
                        layer: None,
                        group_path: path.clone(),
                        colour,
                    });
                }
            }
            NodeKind::Group => {}
        }
    }
    Ok(out)
}

// ----- painting -----------------------------------------------------------------------------------------------------

/// A paint target the command resolved.
#[derive(Debug, Clone, PartialEq)]
enum Resolved {
    Zone { layer: usize, zone: usize },
    Stroke { id: String, pencil: bool, invisible: bool },
    Text { id: String },
}

/// One painted object.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Painted {
    /// The drawing.
    drawing_id: String,
    /// The object (zone IDs are those of the drawing after the paint).
    object_id: String,
    /// `zone`, `pencil`, `brush` or `text`.
    object_kind: String,
    /// Its colour before (`null` = unpainted).
    previous_colour_id: Option<String>,
}

/// One target left alone.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Skipped {
    /// The target: an object ID, or `point:<i>` / `path` / `lasso` / `rect`.
    target: String,
    /// A `colour.*` code: `colour.noTarget`, `colour.zoneNotClosed`, `colour.protected`, `colour.alreadyPainted`,
    /// `colour.notPainted`, `colour.cannotUnpaint`, `colour.invisibleTarget`, `colour.notPaintable`.
    reason: String,
}

/// What a painting pass does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Paint (with `only`: unpainted or painted targets only).
    Paint { only_unpainted: bool, only_painted: bool },
    /// Unpaint.
    Remove,
}

struct Pass<'a> {
    drawing: Id,
    colour: Option<String>,
    kind: SwatchKind,
    mapping: Option<(P, P)>,
    respect_protected: bool,
    mode: Mode,
    doc: &'a Document,
}

/// The current colour of a resolved target.
fn colour_of(r: &Resolved, d: &Drawing, zones: &[LayerZones]) -> Option<String> {
    match r {
        Resolved::Zone { layer, zone } => zones[*layer].fills.get(*zone).cloned().flatten().and_then(|f| f.colour_id),
        Resolved::Stroke { id, .. } => tt_vector::drawing::locate(d, id)
            .and_then(|l| tt_vector::drawing::stroke_at(d, &l.pointer))
            .and_then(|s| s.colour_id().as_ref().map(|c| c.as_str().to_owned())),
        Resolved::Text { id } => tt_vector::drawing::locate(d, id)
            .and_then(|l| tt_vector::drawing::node_at(d, &l.pointer))
            .and_then(|n| n.runs().as_ref().and_then(|r| r.first()).and_then(|r| r.colour_id.clone()))
            .map(|c| c.as_str().to_owned()),
    }
}

fn target_id(r: &Resolved, zones: &[LayerZones]) -> String {
    match r {
        Resolved::Zone { layer, zone } => zones[*layer].map.zones[*zone].id.clone(),
        Resolved::Stroke { id, .. } | Resolved::Text { id } => id.clone(),
    }
}

fn kind_str(r: &Resolved) -> &'static str {
    match r {
        Resolved::Zone { .. } => "zone",
        Resolved::Stroke { pencil: true, .. } => "pencil",
        Resolved::Stroke { invisible: true, .. } => "invisible",
        Resolved::Stroke { .. } => "brush",
        Resolved::Text { .. } => "text",
    }
}

impl Pass<'_> {
    /// Why a target is left alone, if it is.
    fn refusal(&self, r: &Resolved, current: Option<&String>) -> Option<&'static str> {
        if self.respect_protected
            && let Some(c) = current
            && swatch_info(self.doc, c).is_some_and(|(_, p)| p)
            && self.colour.as_ref() != Some(c)
        {
            return Some("colour.protected");
        }
        match (self.mode, r) {
            (_, Resolved::Stroke { invisible: true, .. }) => Some("colour.invisibleTarget"),
            (Mode::Remove, Resolved::Stroke { pencil: false, .. } | Resolved::Text { .. }) => {
                Some("colour.cannotUnpaint")
            }
            (Mode::Remove, Resolved::Zone { .. }) if current.is_none() => Some("colour.notPainted"),
            (Mode::Paint { only_unpainted: true, .. }, Resolved::Zone { .. }) if current.is_some() => {
                Some("colour.alreadyPainted")
            }
            (Mode::Paint { only_unpainted: true, .. }, Resolved::Stroke { .. } | Resolved::Text { .. }) => {
                Some("colour.notPaintable")
            }
            (Mode::Paint { only_painted: true, .. }, Resolved::Zone { .. }) if current.is_none() => {
                Some("colour.notPainted")
            }
            _ => None,
        }
    }
}

/// Stages a paint pass over resolved targets; returns what was painted and what was skipped.
fn apply(
    cx: &mut Cx,
    pass: &Pass<'_>,
    d: &Drawing,
    zones: &[LayerZones],
    targets: Vec<(String, Resolved)>,
) -> Result<(Vec<Painted>, Vec<Skipped>), CommandError> {
    let mut painted = Vec::new();
    let mut skipped = Vec::new();
    let mut zone_fills: BTreeMap<String, BTreeMap<String, Option<Fill>>> = BTreeMap::new();
    let mut ops = Vec::new();
    let mut seen = Vec::new();
    for (label, r) in targets {
        if seen.contains(&r) {
            continue;
        }
        seen.push(r.clone());
        let current = colour_of(&r, d, zones);
        if let Some(reason) = pass.refusal(&r, current.as_ref()) {
            skipped.push(Skipped { target: label, reason: reason.to_owned() });
            continue;
        }
        match &r {
            Resolved::Zone { layer, zone } => {
                let z = &zones[*layer].map.zones[*zone];
                let fill = match pass.mode {
                    Mode::Remove => None,
                    Mode::Paint { .. } => {
                        let matrix = match pass.mapping {
                            Some((a, b)) => drag_matrix(pass.kind, a, b),
                            None => default_matrix(pass.kind, &z.bounds),
                        };
                        Some(Fill { colour_id: pass.colour.clone(), matrix })
                    }
                };
                zone_fills.entry(zones[*layer].layer.id.clone()).or_default().insert(z.id.clone(), fill);
            }
            Resolved::Stroke { id, pencil, .. } => {
                let Some(l) = tt_vector::drawing::locate(d, id) else { continue };
                match pass.mode {
                    Mode::Remove => {
                        // 02 §7.3.2 / 03 §8.3: an unpainted line becomes an invisible stroke (geometry kept).
                        ops.push(op("replace", &format!("{}/kind", l.pointer), Some(json!("invisible"))));
                        ops.push(op("replace", &format!("{}/colourId", l.pointer), Some(Value::Null)));
                        ops.push(op("replace", &format!("{}/lineStyle", l.pointer), Some(Value::Null)));
                    }
                    Mode::Paint { .. } => {
                        ops.push(op("replace", &format!("{}/colourId", l.pointer), Some(json!(pass.colour))));
                        if !pencil {
                            let s = tt_vector::drawing::stroke_at(d, &l.pointer);
                            let bounds = s.and_then(tt_vector::drawing::stroke_bounds);
                            let matrix = match (pass.mapping, bounds) {
                                (Some((a, b)), _) => drag_matrix(pass.kind, a, b),
                                (None, Some(b)) => default_matrix(pass.kind, &b),
                                (None, None) => None,
                            };
                            ops.push(op("replace", &format!("{}/fillMatrix", l.pointer), Some(json!(matrix))));
                        }
                    }
                }
            }
            Resolved::Text { id } => {
                let Some(l) = tt_vector::drawing::locate(d, id) else { continue };
                let runs =
                    tt_vector::drawing::node_at(d, &l.pointer).and_then(|n| n.runs().clone()).unwrap_or_default();
                for i in 0..runs.len() {
                    ops.push(op("replace", &format!("{}/runs/{i}/colourId", l.pointer), Some(json!(pass.colour))));
                }
            }
        }
        painted.push(Painted {
            drawing_id: pass.drawing.as_str().to_owned(),
            object_id: target_id(&r, zones),
            object_kind: kind_str(&r).to_owned(),
            previous_colour_id: current,
        });
    }
    if !ops.is_empty() {
        cx.patch_entries(vec![(EntryKey::Drawing(pass.drawing.clone()), ops)])?;
    }
    for (layer, fills) in zone_fills {
        cx.paint_zones(&pass.drawing, &layer, fills);
    }
    Ok((painted, skipped))
}

/// Where the targets of a call are, before gap closing.
struct Located {
    targets: Vec<(String, Resolved)>,
    skipped: Vec<Skipped>,
    /// Points that hit nothing closed: `(label, point)`.
    open: Vec<(String, P)>,
}

fn resolve_ids(d: &Drawing, zones: &[LayerZones], ids: &[String], out: &mut Located) -> Result<(), CommandError> {
    for id in ids {
        let local = tt_vector::drawing::local_id(d.id().as_str(), id).unwrap_or(id);
        if let Some((li, z)) = zones.iter().enumerate().find_map(|(li, lz)| lz.map.zone_by_id(local).map(|z| (li, z))) {
            out.targets.push((id.clone(), Resolved::Zone { layer: li, zone: z }));
            continue;
        }
        if local.starts_with("zn_") {
            return Err(err(
                "colour.staleZoneId",
                format!("zone `{local}` is not a zone of the drawing's current revision"),
                json!({ "zoneId": local, "rev": d.rev() }),
            )
            .hint("read the zones again with draw.drawing.get, or paint by point"));
        }
        let Some(l) = tt_vector::drawing::locate(d, local) else {
            return Err(super::draw::not_found(id));
        };
        match l.kind {
            tt_vector::drawing::ObjectKind::Stroke => {
                let s = tt_vector::drawing::stroke_at(d, &l.pointer);
                let kind = s.map(|s| *s.kind());
                out.targets.push((
                    id.clone(),
                    Resolved::Stroke {
                        id: local.to_owned(),
                        pencil: kind == Some(tt_model::entities::drawing::StrokeKind::Pencil),
                        invisible: kind == Some(tt_model::entities::drawing::StrokeKind::Invisible),
                    },
                ));
            }
            tt_vector::drawing::ObjectKind::Text => {
                out.targets.push((id.clone(), Resolved::Text { id: local.to_owned() }))
            }
            _ => out.skipped.push(Skipped { target: id.clone(), reason: "colour.notPaintable".into() }),
        }
    }
    Ok(())
}

fn resolved_of(h: &Hit) -> Option<Resolved> {
    match h.kind {
        HitKind::Zone(layer, zone) => Some(Resolved::Zone { layer, zone }),
        HitKind::Pencil => Some(Resolved::Stroke { id: h.id.clone(), pencil: true, invisible: false }),
        HitKind::Brush => Some(Resolved::Stroke { id: h.id.clone(), pencil: false, invisible: false }),
        HitKind::Text => Some(Resolved::Text { id: h.id.clone() }),
        HitKind::Invisible | HitKind::Bitmap => None,
    }
}

/// Whether a point lies within the artwork of a layer (its strokes' bounds): an open region rather than empty canvas.
fn in_artwork(d: &Drawing, p: P) -> bool {
    tt_vector::drawing::drawing_bounds(d, true)
        .is_some_and(|b| p.x >= b.min_x && p.x <= b.max_x && p.y >= b.min_y && p.y <= b.max_y)
}

/// The objects a list of point targets hits (`zones_only`: the zone under each point, lines ignored).
fn resolve_points(
    d: &Drawing,
    zones: &[LayerZones],
    pts: &[P],
    zones_only: bool,
    out: &mut Located,
) -> Result<(), CommandError> {
    for (i, p) in pts.iter().enumerate() {
        let label = format!("point:{i}");
        let hs = hits(d, zones, *p, 0.0, false)?;
        let first = if zones_only { hs.iter().find(|h| matches!(h.kind, HitKind::Zone(..))) } else { hs.first() };
        match first.and_then(resolved_of) {
            Some(r) => out.targets.push((label, r)),
            None if in_artwork(d, *p) => out.open.push((label, *p)),
            None => out.skipped.push(Skipped { target: label, reason: "colour.noTarget".into() }),
        }
    }
    Ok(())
}

/// Zones whose interior points lie inside a polygon (lasso semantics, 03 §8.4).
fn resolve_region(zones: &[LayerZones], poly: &[P], label: &str, out: &mut Located) {
    let inside = |q: P| {
        let n = poly.len();
        let mut w = 0i32;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let o = (b - a).cross(q - a);
            if a.y <= q.y {
                if b.y > q.y && o > 0.0 {
                    w += 1;
                }
            } else if b.y <= q.y && o < 0.0 {
                w -= 1;
            }
        }
        w != 0
    };
    for (li, lz) in zones.iter().enumerate() {
        for (z, zone) in lz.map.zones.iter().enumerate() {
            if inside(zone.interior) {
                out.targets.push((format!("{label}:{}", zone.id), Resolved::Zone { layer: li, zone: z }));
            }
        }
    }
}

/// The objects a crossing path paints: every zone and line it passes over (03 §8.2 design decision).
fn resolve_path(d: &Drawing, zones: &[LayerZones], path: &[P], out: &mut Located) -> Result<(), CommandError> {
    for w in path.windows(2) {
        let n = ((w[1].dist(w[0]) / 0.5).ceil() as usize).clamp(1, 10_000);
        for k in 0..=n {
            let p = w[0].lerp(w[1], k as f64 / n as f64);
            for h in hits(d, zones, p, 0.0, false)? {
                if let Some(r) = resolved_of(&h) {
                    out.targets.push(("path".into(), r));
                }
            }
        }
    }
    Ok(())
}

/// Closes gaps around open points (03 §8.8): the closing segments are added as invisible strokes to the front-most
/// layer whose artwork they close, then the zone under the point is a target. Returns the added strokes.
fn close_open(
    cx: &mut Cx,
    drawing: &Id,
    tol: f64,
    open: &[(String, P)],
    out: &mut Located,
) -> Result<Vec<String>, CommandError> {
    let mut added = Vec::new();
    let mut work = Work::new(tt_vector::work::PLANAR_WORK_BUDGET, "closing gaps");
    for (label, p) in open {
        let doc = cx.stamped_document()?;
        let Some(d) = doc.drawing(drawing) else { break };
        let mut closed = false;
        for l in crate::planar::layers(d).into_iter().rev() {
            let inputs = tt_vector::planar::layer::inputs(&l.strokes);
            let Some(segs) =
                gap::close_around(&inputs, *p, tol, &mut work).map_err(|e| crate::planar::geom_error(&e))?
            else {
                continue;
            };
            let mut ops = Vec::new();
            let mut ids = Vec::new();
            for (a, b) in segs {
                let id = cx.new_id(ObjectType::Stroke)?;
                ids.push(id.as_str().to_owned());
                ops.push(op(
                    "add",
                    &format!("{}/strokes/-", l.pointer),
                    Some(json!({
                        "id": id.as_str(), "type": "stroke", "rev": 0, "kind": "invisible", "closed": false,
                        "points": [[a.x, a.y, 0, 0, 0, 0], [b.x, b.y, 0, 0, 0, 0]], "colourId": null,
                        "fillMatrix": null, "lineStyle": null, "spine": null, "sides": {"left": null, "right": null}
                    })),
                ));
            }
            if !ops.is_empty() {
                cx.patch_entries(vec![(EntryKey::Drawing(drawing.clone()), ops)])?;
            }
            cx.settle()?;
            for id in &ids {
                added.extend(cx.pieces_of(drawing, id));
            }
            let doc = cx.stamped_document()?;
            if let Some(d2) = doc.drawing(drawing) {
                let zones = drawing_zones(&cx.planar_cache(), d2)?;
                if let Some((li, z)) = zones
                    .iter()
                    .enumerate()
                    .find_map(|(li, lz)| (lz.layer.id == l.id).then(|| lz.map.zone_at(*p).map(|z| (li, z))).flatten())
                {
                    out.targets.push((label.clone(), Resolved::Zone { layer: li, zone: z }));
                    closed = true;
                }
            }
            break;
        }
        if !closed {
            out.skipped.push(Skipped { target: label.clone(), reason: "colour.zoneNotClosed".into() });
        }
    }
    Ok(added)
}

/// The error of a call that painted nothing: its first refusal.
fn nothing_painted(skipped: &[Skipped]) -> CommandError {
    let Some(first) = skipped.first() else {
        return err("colour.noTarget", "nothing to paint: give objectIds, points, path, lasso or rect", json!({}));
    };
    let message = match first.reason.as_str() {
        "colour.zoneNotClosed" => "the region under the point is not closed (after gap closing, if enabled)",
        "colour.noTarget" => "the point hits no artwork",
        "colour.protected" => "the target's colour is protected",
        "colour.cannotUnpaint" => "brush strokes and text cannot be unpainted (delete them instead)",
        _ => "no target could be painted",
    };
    err(&first.reason, message, json!({ "skipped": skipped.iter().map(|s| json!({"target": s.target, "reason": s.reason})).collect::<Vec<_>>() }))
        .hint("read the drawing with draw.drawing.get and target zone IDs, or enable gap closing (gapClosing)")
}

/// The drawing revision check of paint targets (03 §16.1 item 5): zone IDs → `colour.staleZoneId`, other IDs →
/// `drawing.staleRevision`; point targets are re-resolved.
fn check_rev(d: &Drawing, expect: Option<u64>, ids: Option<&Vec<String>>) -> Result<(), CommandError> {
    let Some(rev) = expect else { return Ok(()) };
    if *d.rev() == rev || ids.is_none_or(Vec::is_empty) {
        return Ok(());
    }
    let zones = ids.is_some_and(|l| {
        l.iter().any(|i| tt_vector::drawing::local_id(d.id().as_str(), i).unwrap_or(i).starts_with("zn_"))
    });
    let code = if zones { "colour.staleZoneId" } else { "drawing.staleRevision" };
    Err(err(
        code,
        format!("the drawing changed since revision {rev} (now {})", d.rev()),
        json!({ "expectRev": rev, "rev": d.rev() }),
    )
    .hint("read the drawing again (draw.drawing.get) or paint by point"))
}

// ----- colour.paint.apply -------------------------------------------------------------------------------------------

/// `{from, to}` of a press–drag (gradient or texture mapping).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MappingDrag {
    /// Press point.
    from: Point,
    /// Release point.
    to: Point,
}

/// A rectangle `{x, y, width, height}` (drawing space).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RectIn {
    /// Minimum x.
    x: f64,
    /// Minimum y.
    y: f64,
    /// Width.
    #[schemars(range(min = 0.0))]
    width: f64,
    /// Height.
    #[schemars(range(min = 0.0))]
    height: f64,
}

/// `colour.paint.apply` parameters (03 §16.3; `onlyPainted`, `lasso`, `rect`, `respectProtected`: SI-061).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PaintParams {
    /// The drawing (default: the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Zones, lines, brush strokes or text to paint.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    object_ids: Option<Vec<String>>,
    /// Points: the front-most object under each is painted.
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    points: Option<Vec<Point>>,
    /// A crossing path: every zone and line it passes over is painted.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    path: Option<Vec<Point>>,
    /// A lasso: every zone whose interior point lies inside is painted (SI-061).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    lasso: Option<Vec<Point>>,
    /// A marquee: every zone whose interior point lies inside is painted (SI-061).
    #[serde(default)]
    rect: Option<RectIn>,
    /// Coordinate space of points (only `drawing` until M3/M4).
    #[serde(default)]
    space: Space,
    /// Scene of `scene` points (M3 C3.2).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame of `scene` points.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// The element of the drawing (default: current).
    #[serde(default)]
    element_id: Option<String>,
    /// View of `view` points (M4 C4.2).
    #[serde(default)]
    view: Option<String>,
    /// The colour (default: the current fill colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// Paint unpainted zones only.
    #[serde(default)]
    only_unpainted: bool,
    /// Repaint: painted zones only (SI-061).
    #[serde(default)]
    only_painted: bool,
    /// Respect protected colours (SI-061).
    #[serde(default = "yes")]
    respect_protected: bool,
    /// Gradient/texture mapping of a press–drag.
    #[serde(default)]
    mapping: Option<MappingDrag>,
    /// Automatic gap closing for point targets.
    #[serde(default)]
    gap_closing: GapChoice,
    /// The drawing revision the caller read.
    #[serde(default)]
    expect_rev: Option<u64>,
}

fn yes() -> bool {
    true
}

/// `colour.paint.apply` result (03 §16.3, plus the common edit members of 02 §13.1 item 11, SI-058).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaintResult {
    /// The drawing.
    drawing_id: String,
    /// Painted objects.
    painted: Vec<Painted>,
    /// Targets left alone, with the reason.
    skipped: Vec<Skipped>,
    /// Invisible strokes added by gap closing (kept so the zones stay closed, 03 §8.8).
    added_stroke_ids: Vec<String>,
    /// Changed objects (repainted zones and strokes are modified; gap closing creates strokes and zones).
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

fn resolve_colour(cx: &Cx, given: Option<&str>) -> Result<(Option<String>, SwatchKind), CommandError> {
    let doc = cx.document()?;
    let colour = match given {
        Some(c) => Some(c.to_owned()),
        None => super::draw_stroke::current_colour(doc),
    };
    let Some(c) = colour else {
        return Err(err(
            "colour.noCurrentColour",
            "the project has no colour to paint with",
            json!({ "pointer": "/colourId" }),
        ));
    };
    let Some((kind, _)) = swatch_info(doc, &c) else {
        return Err(err(
            "colour.notFound",
            format!("colour `{c}` is not in any palette"),
            json!({ "pointer": "/colourId", "colourId": c }),
        ));
    };
    Ok((Some(c), kind))
}

/// One painting call.
struct Call<'a> {
    drawing_id: Option<&'a str>,
    element_id: Option<&'a str>,
    space: Space,
    targets: Targets<'a>,
    mode: Mode,
    colour: (Option<String>, SwatchKind),
    gap_choice: GapChoice,
}

fn paint_common(cx: &mut Cx, call: Call<'_>) -> Result<PaintResult, CommandError> {
    let Call { drawing_id, element_id, space, targets, mode, colour, gap_choice } = call;
    check_space(space)?;
    let mut none = Vec::new();
    let t = target(cx, drawing_id, element_id, None, false, &mut none)?;
    let before = cx.document()?.clone();
    let d = t.drawing(&before)?.into_owned();
    check_editable(&d, None)?;
    check_rev(&d, targets.expect_rev, targets.object_ids)?;
    let zones = drawing_zones(&cx.planar_cache(), &d)?;
    let mut loc = Located { targets: Vec::new(), skipped: Vec::new(), open: Vec::new() };
    if let Some(ids) = targets.object_ids {
        resolve_ids(&d, &zones, ids, &mut loc)?;
    }
    let pts: Vec<P> = targets
        .points
        .map(|v| v.iter().enumerate().map(|(i, p)| point(*p, &format!("/points/{i}"))).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    resolve_points(&d, &zones, &pts, targets.zones_only, &mut loc)?;
    if let Some(path) = targets.path {
        let path: Vec<P> =
            path.iter().enumerate().map(|(i, p)| point(*p, &format!("/path/{i}"))).collect::<Result<_, _>>()?;
        resolve_path(&d, &zones, &path, &mut loc)?;
    }
    if let Some(lasso) = targets.lasso {
        let poly: Vec<P> =
            lasso.iter().enumerate().map(|(i, p)| point(*p, &format!("/lasso/{i}"))).collect::<Result<_, _>>()?;
        resolve_region(&zones, &poly, "lasso", &mut loc);
    }
    if let Some(r) = targets.rect {
        let poly = [
            P::new(r.x, r.y),
            P::new(r.x + r.width, r.y),
            P::new(r.x + r.width, r.y + r.height),
            P::new(r.x, r.y + r.height),
        ];
        resolve_region(&zones, &poly, "rect", &mut loc);
    }
    if targets.all {
        for (li, lz) in zones.iter().enumerate() {
            for z in 0..lz.map.zones.len() {
                loc.targets.push(("all".into(), Resolved::Zone { layer: li, zone: z }));
            }
        }
    }
    // Gap closing for points in open regions (03 §8.8).
    let mut added = Vec::new();
    let open = std::mem::take(&mut loc.open);
    if !open.is_empty() {
        match gap_tolerance(cx, gap_choice) {
            Some(tol) => {
                let mut closing = Located { targets: Vec::new(), skipped: Vec::new(), open: Vec::new() };
                added = close_open(cx, &t.id, tol, &open, &mut closing)?;
                loc.skipped.extend(closing.skipped);
                if !closing.targets.is_empty() {
                    // Zones were recomputed: resolve the other targets again against the settled drawing.
                    let doc = cx.stamped_document()?;
                    let d2 = doc.drawing(&t.id).cloned().ok_or_else(|| super::draw::not_found(t.id.as_str()))?;
                    let zones2 = drawing_zones(&cx.planar_cache(), &d2)?;
                    let pass = Pass {
                        drawing: t.id.clone(),
                        colour: colour.0.clone(),
                        kind: colour.1,
                        mapping: targets.mapping,
                        respect_protected: targets.respect_protected,
                        mode,
                        doc: &doc,
                    };
                    let (p1, s1) = apply(cx, &pass, &d2, &zones2, closing.targets)?;
                    return finish(cx, &before, &t.id, (p1, s1), loc.skipped, &added);
                }
            }
            None => loc.skipped.extend(
                open.into_iter().map(|(label, _)| Skipped { target: label, reason: "colour.zoneNotClosed".into() }),
            ),
        }
    }
    let pass = Pass {
        drawing: t.id.clone(),
        colour: colour.0.clone(),
        kind: colour.1,
        mapping: targets.mapping,
        respect_protected: targets.respect_protected,
        mode,
        doc: &before,
    };
    let (painted, skipped) = apply(cx, &pass, &d, &zones, std::mem::take(&mut loc.targets))?;
    let mut all_skipped = loc.skipped;
    all_skipped.extend(skipped);
    if painted.is_empty() {
        return Err(nothing_painted(&all_skipped));
    }
    let settled = crate::planar::settled(cx, &before, &t.id, &[])?;
    let mut changed = Changed::default();
    settled.report(&mut changed);
    for p in &painted {
        if p.object_kind != "zone" && !changed.modified_ids.contains(&p.object_id) {
            changed.modified_ids.push(p.object_id.clone());
        }
    }
    // Painting never changes a zone's ID (fills are not part of its boundary), so zone IDs are reported as resolved.
    Ok(PaintResult {
        drawing_id: t.id.as_str().to_owned(),
        painted,
        skipped: all_skipped,
        added_stroke_ids: added,
        changed,
        rev: committed_rev(cx)?,
    })
}

/// Completes a call whose point targets were closed by gap closing (the other targets were resolved against the
/// same settled drawing).
fn finish(
    cx: &mut Cx,
    before: &Document,
    drawing: &Id,
    first: (Vec<Painted>, Vec<Skipped>),
    earlier: Vec<Skipped>,
    added: &[String],
) -> Result<PaintResult, CommandError> {
    let (painted, skipped) = first;
    let mut all_skipped = earlier;
    all_skipped.extend(skipped);
    if painted.is_empty() {
        return Err(nothing_painted(&all_skipped));
    }
    let settled = crate::planar::settled(cx, before, drawing, added)?;
    let mut changed = Changed::default();
    settled.report(&mut changed);
    Ok(PaintResult {
        drawing_id: drawing.as_str().to_owned(),
        painted,
        skipped: all_skipped,
        added_stroke_ids: added.to_vec(),
        changed,
        rev: committed_rev(cx)?,
    })
}

/// The targets of a call.
#[derive(Clone, Copy)]
struct Targets<'a> {
    object_ids: Option<&'a Vec<String>>,
    points: Option<&'a Vec<Point>>,
    path: Option<&'a Vec<Point>>,
    lasso: Option<&'a Vec<Point>>,
    rect: Option<RectIn>,
    all: bool,
    zones_only: bool,
    mapping: Option<(P, P)>,
    respect_protected: bool,
    expect_rev: Option<u64>,
}

#[command(
    id = "colour.paint.apply",
    kind = action,
    label = "Paint",
    scopes = [edit],
    undo = "Paint",
    needs_project,
    example(title = "Paint a closed zone by point", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "points": [{"x": 50, "y": 50}]}"#),
    example(title = "Repaint a pencil line by ID", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectIds": ["st_01K742SG0R61SYK9GG0HGYVNBT"]}"#),
    gui = "Drawing and Camera views: Paint tool (M4 C4.2)",
)]
pub(crate) fn colour_paint_apply(cx: &mut Cx, p: PaintParams) -> Result<PaintResult, CommandError> {
    let colour = resolve_colour(cx, p.colour_id.as_deref())?;
    let mapping = match p.mapping {
        Some(m) => Some((point(m.from, "/mapping/from")?, point(m.to, "/mapping/to")?)),
        None => None,
    };
    let targets = Targets {
        object_ids: p.object_ids.as_ref(),
        points: p.points.as_ref(),
        path: p.path.as_ref(),
        lasso: p.lasso.as_ref(),
        rect: p.rect,
        all: false,
        zones_only: false,
        mapping,
        respect_protected: p.respect_protected,
        expect_rev: p.expect_rev,
    };
    let _ = (&p.scene_id, p.frame, &p.view);
    cx.set_label("Paint");
    paint_common(
        cx,
        Call {
            drawing_id: p.drawing_id.as_deref(),
            element_id: p.element_id.as_deref(),
            space: p.space,
            targets,
            mode: Mode::Paint { only_unpainted: p.only_unpainted, only_painted: p.only_painted },
            colour,
            gap_choice: p.gap_closing,
        },
    )
}

// ----- colour.paintUnpainted.apply ----------------------------------------------------------------------------------

/// `colour.paintUnpainted.apply` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PaintUnpaintedParams {
    /// The drawing (default: the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Zones to fill when unpainted.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    object_ids: Option<Vec<String>>,
    /// Points: the zone under each, when unpainted.
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    points: Option<Vec<Point>>,
    /// A lasso: every unpainted zone whose interior point lies inside.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    lasso: Option<Vec<Point>>,
    /// Every unpainted zone of the drawing.
    #[serde(default)]
    all: bool,
    /// Coordinate space of points (only `drawing` until M3/M4).
    #[serde(default)]
    space: Space,
    /// Scene of `scene` points (M3 C3.2).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame of `scene` points.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// View of `view` points (M4 C4.2).
    #[serde(default)]
    view: Option<String>,
    /// The colour (default: the current fill colour).
    #[serde(default)]
    colour_id: Option<String>,
    /// Automatic gap closing for point targets.
    #[serde(default)]
    gap_closing: GapChoice,
    /// The drawing revision the caller read.
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `colour.paintUnpainted.apply` result (03 §16.3 plus the common edit members, SI-058).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaintUnpaintedResult {
    /// The drawing.
    drawing_id: String,
    /// Filled zones.
    painted: Vec<Painted>,
    /// Targets already painted (left unchanged).
    skipped_painted: usize,
    /// Targets left alone for other reasons.
    skipped: Vec<Skipped>,
    /// Invisible strokes added by gap closing.
    added_stroke_ids: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "colour.paintUnpainted.apply",
    kind = action,
    label = "Paint Unpainted",
    scopes = [edit],
    undo = "Paint Unpainted",
    needs_project,
    example(title = "Fill every unpainted zone", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "all": true}"#),
    gui = "Drawing and Camera views: Paint Unpainted tool (M4 C4.2)",
)]
pub(crate) fn colour_paint_unpainted_apply(
    cx: &mut Cx,
    p: PaintUnpaintedParams,
) -> Result<PaintUnpaintedResult, CommandError> {
    let colour = resolve_colour(cx, p.colour_id.as_deref())?;
    let _ = (&p.scene_id, p.frame, &p.view);
    let targets = Targets {
        object_ids: p.object_ids.as_ref(),
        points: p.points.as_ref(),
        path: None,
        lasso: p.lasso.as_ref(),
        rect: None,
        all: p.all,
        zones_only: true,
        mapping: None,
        respect_protected: true,
        expect_rev: p.expect_rev,
    };
    cx.set_label("Paint Unpainted");
    let r = paint_common(
        cx,
        Call {
            drawing_id: p.drawing_id.as_deref(),
            element_id: None,
            space: p.space,
            targets,
            mode: Mode::Paint { only_unpainted: true, only_painted: false },
            colour,
            gap_choice: p.gap_closing,
        },
    );
    let r = match r {
        Ok(r) => r,
        // Every target already painted, or no zone at all with `all`: nothing to do, reported as such.
        Err(e) if e.code == "colour.alreadyPainted" || (p.all && e.code == "colour.noTarget") => {
            let count = e.details.get("skipped").and_then(Value::as_array).map_or(0, Vec::len);
            let mut none = Vec::new();
            let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
            return Ok(PaintUnpaintedResult {
                drawing_id: t.id.as_str().to_owned(),
                painted: Vec::new(),
                skipped_painted: count,
                skipped: Vec::new(),
                added_stroke_ids: Vec::new(),
                changed: Changed::default(),
                rev: committed_rev(cx)?,
            });
        }
        Err(e) => return Err(e),
    };
    let (already, other): (Vec<Skipped>, Vec<Skipped>) =
        r.skipped.into_iter().partition(|s| s.reason == "colour.alreadyPainted");
    Ok(PaintUnpaintedResult {
        drawing_id: r.drawing_id,
        painted: r.painted,
        skipped_painted: already.len(),
        skipped: other,
        added_stroke_ids: r.added_stroke_ids,
        changed: r.changed,
        rev: r.rev,
    })
}

// ----- colour.paint.remove ------------------------------------------------------------------------------------------

/// `colour.paint.remove` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct UnpaintParams {
    /// The drawing (default: the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Zones or lines to unpaint.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    object_ids: Option<Vec<String>>,
    /// Points: the front-most object under each.
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    points: Option<Vec<Point>>,
    /// Coordinate space of points (only `drawing` until M3/M4).
    #[serde(default)]
    space: Space,
    /// Scene of `scene` points (M3 C3.2).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame of `scene` points.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// View of `view` points (M4 C4.2).
    #[serde(default)]
    view: Option<String>,
    /// Automatic gap closing for point targets.
    #[serde(default)]
    gap_closing: GapChoice,
    /// The drawing revision the caller read.
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// One unpainted object.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Unpainted {
    /// The drawing.
    drawing_id: String,
    /// The object.
    object_id: String,
    /// Its colour before.
    previous_colour_id: Option<String>,
}

/// `colour.paint.remove` result (03 §16.3 plus the common edit members, SI-058).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnpaintResult {
    /// The drawing.
    drawing_id: String,
    /// Unpainted objects (lines became invisible strokes).
    unpainted: Vec<Unpainted>,
    /// Targets left alone, with the reason.
    skipped: Vec<Skipped>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "colour.paint.remove",
    kind = action,
    label = "Unpaint",
    scopes = [edit],
    undo = "Unpaint",
    needs_project,
    example(title = "Turn a pencil line into an invisible stroke", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "objectIds": ["st_01K742SG0R61SYK9GG0HGYVNBT"]}"#),
    gui = "Drawing and Camera views: Unpaint tool (M4 C4.2)",
)]
pub(crate) fn colour_paint_remove(cx: &mut Cx, p: UnpaintParams) -> Result<UnpaintResult, CommandError> {
    let _ = (&p.scene_id, p.frame, &p.view);
    let targets = Targets {
        object_ids: p.object_ids.as_ref(),
        points: p.points.as_ref(),
        path: None,
        lasso: None,
        rect: None,
        all: false,
        zones_only: false,
        mapping: None,
        respect_protected: false,
        expect_rev: p.expect_rev,
    };
    cx.set_label("Unpaint");
    let r = paint_common(
        cx,
        Call {
            drawing_id: p.drawing_id.as_deref(),
            element_id: None,
            space: p.space,
            targets,
            mode: Mode::Remove,
            colour: (None, SwatchKind::Solid),
            gap_choice: p.gap_closing,
        },
    )?;
    if r.painted.iter().any(|x| x.object_kind == "pencil") {
        cx.warn(
            "drawing.strokesHidden",
            "unpainted lines are invisible strokes, shown only with Show Strokes",
            Vec::new(),
        );
    }
    Ok(UnpaintResult {
        drawing_id: r.drawing_id,
        unpainted: r
            .painted
            .into_iter()
            .map(|x| Unpainted {
                drawing_id: x.drawing_id,
                object_id: x.object_id,
                previous_colour_id: x.previous_colour_id,
            })
            .collect(),
        skipped: r.skipped,
        changed: r.changed,
        rev: r.rev,
    })
}

// ----- colour.pick --------------------------------------------------------------------------------------------------

/// `colour.pick` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PickParams {
    /// The point.
    point: Point,
    /// Its space (`drawing` until M3/M4; the spec's default `view` needs the desktop views).
    #[serde(default = "view_space")]
    space: Space,
    /// The drawing of a `drawing` point (default: current).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Scene of a `scene` point.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame of a `scene` point.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Element restriction.
    #[serde(default)]
    element_id: Option<String>,
    /// View of a `view` point.
    #[serde(default)]
    view: Option<String>,
}

fn view_space() -> Space {
    Space::View
}

/// The object a pick found.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickedObject {
    /// The drawing.
    drawing_id: String,
    /// An element exposing it.
    element_id: Option<String>,
    /// The object.
    object_id: String,
    /// `zone`, `pencil`, `brush` or `text`.
    object_kind: String,
}

/// `colour.pick` result (03 §16.3).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickResult {
    /// The colour (`null`: unpainted or nothing).
    colour_id: Option<String>,
    /// Its palette.
    palette_id: Option<String>,
    /// Its swatch name.
    swatch_name: Option<String>,
    /// The swatch's value for the scene's style as `#RRGGBBAA` (first stop of a gradient; `#00000000` for textures,
    /// unpainted and empty canvas) until the renderer composites (SI-065).
    rgba: String,
    /// The fill mapping of a gradient or texture fill.
    matrix: Option<[f64; 6]>,
    /// The object picked.
    object: Option<PickedObject>,
    /// The colour ID is in no palette.
    missing: bool,
}

#[command(
    id = "colour.pick",
    kind = query,
    label = "Pick colour",
    scopes = [read],
    needs_project,
    example(title = "The colour under a point of a drawing", params = r#"{"point": {"x": 0, "y": 0}, "space": "drawing", "drawingId": "drawingBank:#1/drawing:#1"}"#),
    gui = "Drawing and Camera views: Dropper tool (M4 C4.2)",
)]
pub(crate) fn colour_pick(cx: &mut Cx, p: PickParams) -> Result<PickResult, CommandError> {
    check_space(p.space)?;
    let _ = (&p.scene_id, p.frame, &p.view);
    let at = point(p.point, "/point")?;
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), None, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let zones = drawing_zones(&cx.planar_cache(), &d)?;
    let hs = hits(&d, &zones, at, 0.0, false)?;
    let element = t.element.clone().or_else(|| doc.bank_of(d.id()).and_then(|b| doc.bank_users(b).first().cloned()));
    let Some(h) = hs.into_iter().find(|h| !matches!(h.kind, HitKind::Bitmap | HitKind::Invisible)) else {
        return Ok(PickResult {
            colour_id: None,
            palette_id: None,
            swatch_name: None,
            rgba: "#00000000".into(),
            matrix: None,
            object: None,
            missing: false,
        });
    };
    let matrix = match h.kind {
        HitKind::Zone(li, z) => zones[li].fills.get(z).cloned().flatten().and_then(|f| f.matrix),
        _ => None,
    };
    let swatch = h.colour.as_ref().and_then(|c| {
        doc.palettes().find_map(|pal| pal.swatches().iter().find(|s| s.colour_id().as_str() == c).map(|s| (pal, s)))
    });
    let rgba = swatch.and_then(|(pal, s)| {
        let style = pal.styles().first()?.id().clone();
        match s.values().get(&style)? {
            tt_model::entities::palette::SwatchValue::Solid(v) => Some(v.rgba.to_string()),
            tt_model::entities::palette::SwatchValue::Gradient(g) => g.stops.first().map(|s| s.rgba.to_string()),
            tt_model::entities::palette::SwatchValue::Texture(_) => None,
        }
    });
    Ok(PickResult {
        colour_id: h.colour.clone(),
        palette_id: swatch.map(|(pal, _)| pal.id().as_str().to_owned()),
        swatch_name: swatch.map(|(_, s)| s.name().clone()),
        rgba: rgba.unwrap_or_else(|| "#00000000".into()),
        matrix,
        missing: h.colour.is_some() && swatch.is_none(),
        object: Some(PickedObject {
            drawing_id: d.id().as_str().to_owned(),
            element_id: element.map(|e| e.as_str().to_owned()),
            object_id: h.id,
            object_kind: h.kind.as_str().to_owned(),
        }),
    })
}

// ----- draw.hitTest.get ---------------------------------------------------------------------------------------------

/// `draw.hitTest.get` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct HitTestParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell exposes the drawing.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// The point (drawing space).
    point: Point,
    /// Extra reach of line hits in view pixels (1 px = 1 du at 100 % zoom, SI-063).
    #[serde(default = "three")]
    #[schemars(range(min = 0.0, max = 1000.0))]
    tolerance: f64,
    /// Report invisible strokes.
    #[serde(default)]
    include_invisible: bool,
}

fn three() -> f64 {
    3.0
}

/// One hit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HitOut {
    /// The object.
    object_id: String,
    /// `pencil`, `brush`, `invisible`, `zone`, `text` or `bitmap`.
    object_kind: String,
    /// Its layer.
    layer_id: Option<String>,
    /// Enclosing groups, outermost first.
    group_path: Vec<String>,
    /// Its colour.
    colour_id: Option<String>,
}

/// `draw.hitTest.get` result (02 §13.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HitTestResult {
    /// The point.
    point: Point,
    /// Hits, front to back.
    hits: Vec<HitOut>,
    /// The front-most zone under the point.
    zone_id: Option<String>,
    /// The drawing revision.
    rev: u64,
}

#[command(
    id = "draw.hitTest.get",
    kind = query,
    label = "Hit test",
    scopes = [read],
    needs_project,
    example(title = "What lies under a point", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "point": {"x": 0, "y": 0}, "includeInvisible": true}"#),
    agent_only = "verification and targeting for agents (02 §13.1 item 10); the GUI hit-tests in the views",
)]
pub(crate) fn draw_hit_test_get(cx: &mut Cx, p: HitTestParams) -> Result<HitTestResult, CommandError> {
    let at = point(p.point, "/point")?;
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, false, &mut none)?;
    let doc = cx.document()?.clone();
    let d = t.drawing(&doc)?.into_owned();
    let zones = drawing_zones(&cx.planar_cache(), &d)?;
    let hs = hits(&d, &zones, at, p.tolerance, p.include_invisible)?;
    let zone_id = hs.iter().find(|h| matches!(h.kind, HitKind::Zone(..))).map(|h| h.id.clone());
    Ok(HitTestResult {
        point: p.point,
        hits: hs
            .into_iter()
            .map(|h| HitOut {
                object_kind: h.kind.as_str().to_owned(),
                object_id: h.id,
                layer_id: h.layer,
                group_path: h.group_path,
                colour_id: h.colour,
            })
            .collect(),
        zone_id,
        rev: *d.rev(),
    })
}

// ----- colour.gap.close ---------------------------------------------------------------------------------------------

/// `colour.gap.close` parameters (03 §16.3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GapCloseParams {
    /// The drawing (default: the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// Start of the drag.
    from: Point,
    /// End of the drag.
    to: Point,
    /// Coordinate space (only `drawing` until M3/M4).
    #[serde(default)]
    space: Space,
    /// Scene of `scene` points.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame of `scene` points.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// View of `view` points.
    #[serde(default)]
    view: Option<String>,
}

/// `colour.gap.close` result (03 §16.3 plus the common edit members, SI-058).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GapCloseResult {
    /// The drawing.
    drawing_id: String,
    /// The invisible stroke added (its first piece when noding split it).
    stroke_id: Option<String>,
    /// All strokes it became.
    stroke_ids: Vec<String>,
    /// Hint codes (`drawing.strokesHidden`).
    hints: Vec<String>,
    /// Changed objects.
    #[serde(flatten)]
    changed: Changed,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "colour.gap.close",
    kind = action,
    label = "Close Gap",
    scopes = [edit],
    undo = "Close Gap",
    needs_project,
    example(title = "Join the ends of two strokes", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "from": {"x": -48, "y": -50}, "to": {"x": 2, "y": 2}}"#),
    gui = "Drawing and Camera views: Close Gap tool (M4 C4.2)",
)]
pub(crate) fn colour_gap_close(cx: &mut Cx, p: GapCloseParams) -> Result<GapCloseResult, CommandError> {
    check_space(p.space)?;
    let _ = (&p.scene_id, p.frame, &p.view);
    let (from, to) = (point(p.from, "/from")?, point(p.to, "/to")?);
    let mut none = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), None, None, false, &mut none)?;
    let before = cx.document()?.clone();
    let d = t.drawing(&before)?.into_owned();
    check_editable(&d, None)?;
    // A tolerance proportional to the drag, at least the large gap tolerance (03 §8.6).
    let tol = (from.dist(to) * 0.5).max(gap::TOLERANCE_PX[2]);
    let mut work = Work::new(tt_vector::work::PLANAR_WORK_BUDGET, "closing a gap");
    let mut found = None;
    for l in crate::planar::layers(&d).into_iter().rev() {
        let inputs = tt_vector::planar::layer::inputs(&l.strokes);
        if let Some(seg) =
            gap::gap_segment(&inputs, from, to, tol, &mut work).map_err(|e| crate::planar::geom_error(&e))?
        {
            found = Some((l, seg));
            break;
        }
    }
    let Some((l, (a, b))) = found else {
        return Err(err("colour.noGapFound", "no stroke ends near both ends of the drag", json!({ "tolerance": tol }))
            .hint("drag closer to the stroke ends, or draw an invisible stroke with draw.stroke.add"));
    };
    let id = cx.new_id(ObjectType::Stroke)?;
    cx.patch_entries(vec![(
        EntryKey::Drawing(t.id.clone()),
        vec![op(
            "add",
            &format!("{}/strokes/-", l.pointer),
            Some(json!({
                "id": id.as_str(), "type": "stroke", "rev": 0, "kind": "invisible", "closed": false,
                "points": [[a.x, a.y, 0, 0, 0, 0], [b.x, b.y, 0, 0, 0, 0]], "colourId": null,
                "fillMatrix": null, "lineStyle": null, "spine": null, "sides": {"left": null, "right": null}
            })),
        )],
    )])?;
    cx.set_label("Close Gap");
    let settled = crate::planar::settled(cx, &before, &t.id, &[id.as_str().to_owned()])?;
    let mut changed = Changed { modified_ids: vec![l.id.clone()], ..Changed::default() };
    settled.report(&mut changed);
    Ok(GapCloseResult {
        drawing_id: t.id.as_str().to_owned(),
        stroke_id: settled.added.first().cloned(),
        stroke_ids: settled.added.clone(),
        hints: vec!["drawing.strokesHidden".into()],
        changed,
        rev: committed_rev(cx)?,
    })
}

// ----- colour.gapClosing.set / get ----------------------------------------------------------------------------------

/// `colour.gapClosing.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GapClosingSetParams {
    /// The mode.
    mode: GapMode,
}

/// `{mode}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GapClosingSetResult {
    /// The mode now.
    mode: GapMode,
}

#[command(
    id = "colour.gapClosing.set",
    kind = ui,
    label = "Automatic gap closing",
    scopes = [app],
    example(title = "Close small gaps while painting", params = r#"{"mode": "small"}"#),
    gui = "Main menu and paint tool options: Auto Close Gap (M4 C4.2)",
)]
pub(crate) fn colour_gap_closing_set(cx: &mut Cx, p: GapClosingSetParams) -> Result<GapClosingSetResult, CommandError> {
    let mut prefs = serde_json::Map::new();
    prefs.insert(GAP_PREF.into(), json!(p.mode));
    cx.defer(crate::cx::Effect::Prefs(prefs));
    Ok(GapClosingSetResult { mode: p.mode })
}

/// Tolerances in view pixels.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Tolerances {
    /// Small.
    small: f64,
    /// Medium.
    medium: f64,
    /// Large.
    large: f64,
}

/// `colour.gapClosing.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GapClosingGetResult {
    /// The mode.
    mode: GapMode,
    /// Tolerances (view px; 1 px = 1 du at 100 % zoom, SI-063).
    tolerances_px: Tolerances,
}

#[command(
    id = "colour.gapClosing.get",
    kind = query,
    label = "Get automatic gap closing",
    scopes = [read],
    example(title = "Current gap closing mode", params = "{}"),
    agent_only = "the GUI shows the mode in the paint tool options",
)]
pub(crate) fn colour_gap_closing_get(
    cx: &mut Cx,
    _p: super::common::NoParams,
) -> Result<GapClosingGetResult, CommandError> {
    let mode = GapMode::of(cx.session().env.pref(GAP_PREF).as_ref());
    Ok(GapClosingGetResult {
        mode,
        tolerances_px: Tolerances {
            small: gap::TOLERANCE_PX[0],
            medium: gap::TOLERANCE_PX[1],
            large: gap::TOLERANCE_PX[2],
        },
    })
}
