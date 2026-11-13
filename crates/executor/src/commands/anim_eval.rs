//! Evaluation queries (05 §19.1.5, §19.4; M3 C3.2; ADR-0019): `anim.evaluate`, `anim.projection.get`, `anim.path.sample`,
//! `anim.function.sample`. Queries change nothing: no revision, undo step, zone ID, selection or saved default.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_anim::camera::CameraView;
use tt_anim::eval::{EvalError, Evaluator};
use tt_anim::exposure as x;
use tt_anim::matrix::Mat4;
use tt_anim::num::tidy;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::{Element, ElementType};
use tt_registry::{CommandError, command};
use tt_vector::geom::Rect;

use super::anim_common::{
    P2, P3, check_channel, eval_error, evaluable, evaluator, frame_invalid, node, node_id, not_found, scene_frame,
};
use super::timing::err;
use crate::app::AppContext;
use crate::cx::Cx;

/// Drawing units per field (02 §3).
const DU_PER_FIELD: f64 = 100.0;

/// The largest number of frame × node results or samples one query returns (a larger request is `limit.exceeded`).
pub(crate) const MAX_RESULTS: usize = 1_000_000;

fn too_many(n: usize) -> CommandError {
    err(
        tt_registry::codes::LIMIT_EXCEEDED,
        format!("{n} results exceed the {MAX_RESULTS}-result limit of one query"),
        json!({ "results": n, "limit": MAX_RESULTS }),
    )
    .hint("ask for fewer frames or nodes per call")
}

/// A part of `NodeEval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IncludePart {
    /// Static values.
    Static,
    /// Animated values.
    Animated,
    /// Effective local values and matrix.
    Local,
    /// World matrix and decomposition.
    World,
    /// World-space box of the visible content.
    Bounds,
    /// Output-pixel box.
    Screen,
}

/// `anim.evaluate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct EvaluateParams {
    /// Nodes to evaluate.
    #[schemars(length(min = 1, max = 10_000))]
    node_ids: Vec<String>,
    /// Scene frames, fractional allowed (SI-098); default: the current frame.
    #[serde(default)]
    #[schemars(length(min = 1, max = 100_000))]
    frames: Option<Vec<f64>>,
    /// What to report; default: everything.
    #[serde(default)]
    include: Option<Vec<IncludePart>>,
    /// Camera for `screen`; default: the node's scene's current camera.
    #[serde(default)]
    camera_id: Option<String>,
}

/// `static` of `NodeEval`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaticEval {
    position: P3,
    rotation: f64,
    scale: P2,
    skew: f64,
    rotation_pivot: P2,
    scale_pivot: P2,
}

/// `animated` of `NodeEval`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnimatedEval {
    position: P3,
    rotation: f64,
    scale: P2,
    skew: f64,
    pivot: P2,
    velocity: f64,
    orient_angle: f64,
}

/// `local` of `NodeEval`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalPart {
    position: P3,
    rotation: f64,
    scale: P2,
    skew: f64,
    rotation_pivot: P2,
    scale_pivot: P2,
    /// M_local, 16 numbers row-major.
    matrix: Vec<f64>,
}

/// `world` of `NodeEval`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldPart {
    /// World matrix, 16 numbers row-major.
    matrix: Vec<f64>,
    position: P3,
    rotation: f64,
    scale: P2,
    skew: f64,
}

/// `bounds` of `NodeEval`.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldBounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    z: f64,
}

/// An output-pixel box.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct Bbox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// `screen` of `NodeEval`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ScreenPart {
    /// In front of the near plane and overlapping the frame; for content nodes also not hidden, not an annotation,
    /// and with content at that frame (SI-100).
    visible: bool,
    bbox: Bbox,
    /// Depth of the node's origin in front of the camera.
    depth: f64,
}

/// One node at one frame (05 §19.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeEval {
    id: String,
    kind: String,
    /// Local frame (before the cycle mapping).
    local_frame: f64,
    #[serde(rename = "static", skip_serializing_if = "Option::is_none")]
    statics: Option<StaticEval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    animated: Option<AnimatedEval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local: Option<LocalPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world: Option<WorldPart>,
    /// Absent for nodes without visible content at that frame (pegs, cameras, a blank cell).
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<WorldBounds>,
    /// Absent when no camera applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    screen: Option<ScreenPart>,
}

/// One frame of `anim.evaluate`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameEval {
    frame: f64,
    nodes: Vec<NodeEval>,
}

/// `anim.evaluate` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EvaluateResult {
    frames: Vec<FrameEval>,
}

fn matrix(m: &Mat4) -> Vec<f64> {
    m.0.iter().map(|v| tidy(*v)).collect()
}

/// The frame parameters: each 1…100 000.
fn frames_param(cx: &Cx, given: Option<&[f64]>) -> Result<Vec<f64>, CommandError> {
    match given {
        Some(list) => {
            for f in list {
                if !(1.0..=x::MAX_FRAMES as f64).contains(f) {
                    return Err(frame_invalid(*f, "outside 1…100000"));
                }
            }
            Ok(list.to_vec())
        }
        None => Ok(vec![scene_frame(cx, None)?]),
    }
}

/// The camera of an optional `cameraId`.
fn camera_param(cx: &Cx, doc: &Document, given: Option<&str>) -> Result<Option<Id>, CommandError> {
    let Some(r) = given else { return Ok(None) };
    let id = node_id(cx, r).map_err(|_| not_found("camera", r))?;
    if node(doc, &id)?.camera().is_none() {
        return Err(not_found("camera", r));
    }
    Ok(Some(id))
}

/// The current camera of a node's scene.
fn scene_camera(doc: &Document, id: &Id) -> Option<Id> {
    doc.scene_of(id).and_then(|s| doc.scene(s)).and_then(|s| s.current_camera_id().clone())
}

/// Whether a node or an ancestor is hidden in the scene (04 §5).
fn hidden(doc: &Document, e: &Element) -> bool {
    !*e.visible_in_scene()
        || doc
            .scene_of(e.id())
            .and_then(|s| doc.scene(s))
            .is_some_and(|s| super::derived::ancestors_hidden(doc, s.tree(), e.id()))
}

/// Visible content of a node at a frame: the drawing exposed there (02 §3: drawing space in du).
fn content_box(doc: &Document, e: &Element, frame: f64, memo: &mut BTreeMap<Id, Option<Rect>>) -> Option<Rect> {
    if !e.element_type().is_cell_based() {
        return None;
    }
    let cell = AppContext::exposed_drawing(doc, e.id(), frame.floor() as u32)?;
    *memo
        .entry(cell.clone())
        .or_insert_with(|| doc.drawing(&cell).and_then(|d| tt_vector::drawing::drawing_bounds(d, false)))
}

/// The world box of a drawing box under `world` (05 §1.3: 1 field = 100 du; the box of the four transformed corners).
fn world_corners(world: &Mat4, r: &Rect) -> [[f64; 3]; 4] {
    let at = |x: f64, y: f64| world.apply([x / DU_PER_FIELD, y / DU_PER_FIELD, 0.0]);
    [at(r.min_x, r.min_y), at(r.max_x, r.min_y), at(r.max_x, r.max_y), at(r.min_x, r.max_y)]
}

fn screen_part(view: &CameraView, world: &Mat4, corners: Option<&[[f64; 3]; 4]>, shown: bool) -> ScreenPart {
    let origin = world.translation();
    let projected = view.project(origin);
    let mut pixels: Vec<[f64; 2]> = Vec::new();
    match corners {
        Some(c) => pixels.extend(c.iter().filter_map(|p| view.project(*p).pixel)),
        None => pixels.extend(projected.pixel),
    }
    let empty = Bbox { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    let bbox = pixels.iter().fold(None, |acc: Option<(f64, f64, f64, f64)>, p| {
        Some(match acc {
            None => (p[0], p[1], p[0], p[1]),
            Some((a, b, c, d)) => (a.min(p[0]), b.min(p[1]), c.max(p[0]), d.max(p[1])),
        })
    });
    let (bbox, overlaps) = match bbox {
        None => (empty, false),
        Some((a, b, c, d)) => (
            Bbox { x: tidy(a), y: tidy(b), width: tidy(c - a), height: tidy(d - b) },
            c >= 0.0 && a <= view.width && d >= 0.0 && b <= view.height,
        ),
    };
    ScreenPart { visible: shown && overlaps, bbox, depth: tidy(projected.depth) }
}

/// `true` when `parts` asks for `part` (everything by default).
fn wants(parts: Option<&[IncludePart]>, part: IncludePart) -> bool {
    parts.is_none_or(|p| p.contains(&part))
}

fn node_eval(
    ev: &mut Evaluator<'_, CommandError>,
    doc: &Document,
    id: &Id,
    frame: f64,
    parts: Option<&[IncludePart]>,
    camera: Option<&Id>,
    memo: &mut BTreeMap<Id, Option<Rect>>,
) -> Result<NodeEval, CommandError> {
    let e = node(doc, id)?;
    let l = ev.local(id, frame).map_err(eval_error)?;
    let world = ev.world(id, frame).map_err(eval_error)?;
    let statics = wants(parts, IncludePart::Static).then(|| {
        let (cr, cs) = l.statics.pivots([0.0, 0.0]);
        StaticEval {
            position: l.statics.position.into(),
            rotation: tidy(l.statics.rotation),
            scale: l.statics.scale.into(),
            skew: tidy(l.statics.skew),
            rotation_pivot: cr.into(),
            scale_pivot: cs.into(),
        }
    });
    let animated = wants(parts, IncludePart::Animated).then(|| AnimatedEval {
        position: l.animated.position.into(),
        rotation: tidy(l.animated.rotation),
        scale: l.animated.scale.into(),
        skew: tidy(l.animated.skew),
        pivot: l.animated.pivot.into(),
        velocity: tidy(l.animated.velocity),
        orient_angle: tidy(l.animated.orient_angle),
    });
    let local = wants(parts, IncludePart::Local).then(|| LocalPart {
        position: l.position.into(),
        rotation: tidy(l.rotation),
        scale: l.scale.into(),
        skew: tidy(l.skew),
        rotation_pivot: l.rotation_pivot.into(),
        scale_pivot: l.scale_pivot.into(),
        matrix: matrix(&l.matrix),
    });
    let world_part = wants(parts, IncludePart::World).then(|| world.decompose());
    // ADR-0019 rule 5: a value that left the finite range is a typed refusal, never an encoding failure (C32-R1-06).
    let finite = |xs: &[f64]| xs.iter().all(|x| x.is_finite());
    if world_part.as_ref().is_some_and(|d| !finite(&[d.sx, d.sy, d.rotation, d.skew])) {
        return Err(eval_error(EvalError::NonFinite(id.clone())));
    }
    let world_part = world_part.map(|d| WorldPart {
        matrix: matrix(&world),
        position: d.position.into(),
        rotation: tidy(d.rotation),
        scale: [d.sx, d.sy].into(),
        skew: tidy(d.skew),
    });
    let want_bounds = wants(parts, IncludePart::Bounds);
    let want_screen = wants(parts, IncludePart::Screen);
    let corners = if want_bounds || want_screen {
        content_box(doc, e, frame, memo).map(|r| world_corners(&world, &r))
    } else {
        None
    };
    if corners.as_ref().is_some_and(|c| !c.iter().all(|p| finite(p))) {
        return Err(eval_error(EvalError::NonFinite(id.clone())));
    }
    let bounds = corners.filter(|_| want_bounds).map(|c| {
        let xs = c.iter().map(|p| p[0]);
        let ys = c.iter().map(|p| p[1]);
        WorldBounds {
            min_x: tidy(xs.clone().fold(f64::INFINITY, f64::min)),
            min_y: tidy(ys.clone().fold(f64::INFINITY, f64::min)),
            max_x: tidy(xs.fold(f64::NEG_INFINITY, f64::max)),
            max_y: tidy(ys.fold(f64::NEG_INFINITY, f64::max)),
            z: tidy(world.translation()[2]),
        }
    });
    let camera = camera.cloned().or_else(|| scene_camera(doc, id));
    let screen = match camera.filter(|_| want_screen) {
        Some(c) => {
            let view = ev.camera(&c, frame).map_err(eval_error)?;
            let content_kind = e.element_type().is_cell_based();
            let shown = if content_kind {
                corners.is_some() && !hidden(doc, e) && *e.element_type() != ElementType::Annotation
            } else {
                true
            };
            Some(screen_part(&view, &world, corners.as_ref(), shown))
        }
        None => None,
    };
    Ok(NodeEval {
        id: id.as_str().to_owned(),
        kind: e.element_type().as_str().to_owned(),
        local_frame: tidy(l.local_frame),
        statics,
        animated,
        local,
        world: world_part,
        bounds,
        screen,
    })
}

#[command(
    id = "anim.evaluate",
    kind = query,
    label = "Evaluate transforms",
    scopes = [read],
    needs_project,
    example(title = "World transform of Bob at frame 12", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob\""], "frames": [12], "include": ["world"]}"#),
    example(title = "A peg at an in-between frame, everything", params = r#"{"nodeIds": ["scene:\"Intro\"/element:\"Bob-P\""], "frames": [7.5]}"#),
    agent_only = "agents verify placement without rendering (05 §19.1.5); the Properties panel and views show these values (M6)",
)]
pub(crate) fn anim_evaluate(cx: &mut Cx, p: EvaluateParams) -> Result<EvaluateResult, CommandError> {
    let doc = cx.document()?.clone();
    let mut ids = Vec::with_capacity(p.node_ids.len());
    for r in &p.node_ids {
        let id = node_id(cx, r)?;
        evaluable(&doc, &id)?;
        ids.push(id);
    }
    let camera = camera_param(cx, &doc, p.camera_id.as_deref())?;
    let frames = frames_param(cx, p.frames.as_deref())?;
    let n = frames.len().saturating_mul(ids.len());
    if n > MAX_RESULTS {
        return Err(too_many(n));
    }
    let mut ev = evaluator(&doc, "evaluating transforms");
    let mut memo = BTreeMap::new();
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut nodes = Vec::with_capacity(ids.len());
        for id in &ids {
            ev.charge(64)?;
            nodes.push(node_eval(&mut ev, &doc, id, frame, p.include.as_deref(), camera.as_ref(), &mut memo)?);
        }
        out.push(FrameEval { frame: tidy(frame), nodes });
    }
    Ok(EvaluateResult { frames: out })
}

/// `anim.projection.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProjectionParams {
    /// Scene frame; default: the current frame.
    #[serde(default)]
    frame: Option<i64>,
    /// Camera; default: the current scene's current camera.
    #[serde(default)]
    camera_id: Option<String>,
    /// World points.
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    points: Option<Vec<super::anim_common::Xyz>>,
    /// Nodes whose bounds to project.
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    node_ids: Option<Vec<String>>,
}

/// A projected point.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ProjectedPoint {
    /// Output pixel x, `null` at or behind the near plane (SI-100).
    x: Option<f64>,
    /// Output pixel y, `null` at or behind the near plane.
    y: Option<f64>,
    /// Depth in front of the camera.
    depth: f64,
    /// In front of the near plane and inside the frame.
    visible: bool,
}

/// A projected node.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ProjectedNode {
    id: String,
    bbox: Bbox,
    visible: bool,
}

/// `anim.projection.get` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ProjectionResult {
    points: Vec<ProjectedPoint>,
    nodes: Vec<ProjectedNode>,
}

#[command(
    id = "anim.projection.get",
    kind = query,
    label = "Project into output pixels",
    scopes = [read],
    needs_project,
    example(title = "Where the scene origin lands at frame 1", params = r#"{"frame": 1, "points": [{"x": 0, "y": 0, "z": 0}]}"#),
    example(title = "Bob's box in output pixels", params = r#"{"frame": 1, "nodeIds": ["scene:\"Intro\"/element:\"Bob\""]}"#),
    agent_only = "agents read output-pixel positions (05 §19.1.5); C3.3's convert_coordinates builds on it",
)]
pub(crate) fn anim_projection_get(cx: &mut Cx, p: ProjectionParams) -> Result<ProjectionResult, CommandError> {
    let doc = cx.document()?.clone();
    let frame = scene_frame(cx, p.frame.map(|f| f as f64))?;
    let camera = match camera_param(cx, &doc, p.camera_id.as_deref())? {
        Some(c) => c,
        None => {
            let scene = super::timing::scene_id(cx, None)?;
            doc.scene(&scene)
                .and_then(|s| s.current_camera_id().clone())
                .ok_or_else(|| not_found("current camera of the scene", scene.as_str()))?
        }
    };
    let mut ev = evaluator(&doc, "projecting");
    let view = ev.camera(&camera, frame).map_err(eval_error)?;
    let mut points = Vec::new();
    for q in p.points.iter().flatten() {
        ev.charge(1)?;
        let [a, b, c] = q.parts();
        let r = view.project([a.unwrap_or(0.0), b.unwrap_or(0.0), c.unwrap_or(0.0)]);
        let visible = r.pixel.is_some_and(|m| m[0] >= 0.0 && m[0] <= view.width && m[1] >= 0.0 && m[1] <= view.height);
        points.push(ProjectedPoint {
            x: r.pixel.map(|m| tidy(m[0])),
            y: r.pixel.map(|m| tidy(m[1])),
            depth: tidy(r.depth),
            visible,
        });
    }
    let mut memo = BTreeMap::new();
    let mut nodes = Vec::new();
    for r in p.node_ids.iter().flatten() {
        let id = node_id(cx, r)?;
        evaluable(&doc, &id)?;
        let e = node(&doc, &id)?;
        let world = ev.world(&id, frame).map_err(eval_error)?;
        let corners = content_box(&doc, e, frame, &mut memo).map(|c| world_corners(&world, &c));
        let shown = if e.element_type().is_cell_based() {
            corners.is_some() && !hidden(&doc, e) && *e.element_type() != ElementType::Annotation
        } else {
            true
        };
        let s = screen_part(&view, &world, corners.as_ref(), shown);
        nodes.push(ProjectedNode { id: id.as_str().to_owned(), bbox: s.bbox, visible: s.visible });
    }
    Ok(ProjectionResult { points, nodes })
}

/// What `anim.path.sample` follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SampleWhat {
    /// The node's origin.
    #[default]
    Origin,
    /// The node's rotation pivot.
    Pivot,
}

/// The space of a sample.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SampleSpace {
    /// Parent space.
    Parent,
    /// World space.
    #[default]
    World,
}

fn one() -> f64 {
    1.0
}

/// `anim.path.sample` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PathSampleParams {
    /// The node.
    node_id: String,
    /// First scene frame; default: the bar's first.
    #[serde(default)]
    from: Option<i64>,
    /// Last scene frame; default: the bar's last.
    #[serde(default)]
    to: Option<i64>,
    /// Frame step, above 0.
    #[serde(default = "one")]
    step: f64,
    /// Origin or pivot.
    #[serde(default)]
    what: SampleWhat,
    /// Parent or world space.
    #[serde(default)]
    space: SampleSpace,
}

/// A position sample.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PositionSample {
    frame: f64,
    x: f64,
    y: f64,
    z: f64,
}

/// `anim.path.sample` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PathSampleResult {
    samples: Vec<PositionSample>,
}

/// The sample frames `from, from + step, …` up to `to`, computed by multiplication (no accumulated rounding).
fn sample_frames(from: f64, to: f64, step: f64) -> Result<Vec<f64>, CommandError> {
    if step <= 0.0 {
        return Err(err(
            "anim.valueOutOfRange",
            format!("step {step} must be above 0"),
            json!({ "what": "step", "value": step, "minimum": 0 }),
        ));
    }
    if to < from {
        return Err(frame_invalid(to, "`to` is before `from`"));
    }
    let count = ((to - from) / step).floor() + 1.0;
    if count > MAX_RESULTS as f64 {
        return Err(too_many(MAX_RESULTS.saturating_add(1)));
    }
    Ok((0..count as usize).map(|k| from + k as f64 * step).collect())
}

#[command(
    id = "anim.path.sample",
    kind = query,
    label = "Sample positions",
    scopes = [read],
    needs_project,
    example(title = "Bob's world origin per frame over frames 1-6", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 6}"#),
    example(title = "The pivot in parent space on half frames", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob-P\"", "from": 1, "to": 3, "step": 0.5, "what": "pivot", "space": "parent"}"#),
    gui = "Scene views: frame markers, ghosts and pivot paths (M6 C6.1)",
)]
pub(crate) fn anim_path_sample(cx: &mut Cx, p: PathSampleParams) -> Result<PathSampleResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    evaluable(&doc, &id)?;
    let mut ev = evaluator(&doc, "sampling a path");
    let bar = ev.node(&id).map_err(eval_error)?.bar;
    let from = p.from.map_or(bar.start, |f| f as f64);
    let to = p.to.map_or(bar.scene(bar.duration), |f| f as f64);
    let mut samples = Vec::new();
    for frame in sample_frames(from, to, p.step)? {
        if !(1.0..=x::MAX_FRAMES as f64).contains(&frame) {
            return Err(frame_invalid(frame, "outside 1…100000"));
        }
        ev.charge(16)?;
        let l = ev.local(&id, frame).map_err(eval_error)?;
        let parent_point = match p.what {
            SampleWhat::Origin => l.matrix.translation(),
            SampleWhat::Pivot => {
                [l.position[0] + l.rotation_pivot[0], l.position[1] + l.rotation_pivot[1], l.position[2]]
            }
        };
        let q = match p.space {
            SampleSpace::Parent => parent_point,
            SampleSpace::World => ev.parent_world(&id, frame).map_err(eval_error)?.apply(parent_point),
        };
        samples.push(PositionSample { frame: tidy(frame), x: tidy(q[0]), y: tidy(q[1]), z: tidy(q[2]) });
    }
    Ok(PathSampleResult { samples })
}

/// `anim.function.sample` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FunctionSampleParams {
    /// The node.
    node_id: String,
    /// The channel (05 §1.6), including the `position.x/y/z` projections.
    #[schemars(length(min = 1, max = 256))]
    channel: String,
    /// First scene frame.
    from: f64,
    /// Last scene frame.
    to: f64,
    /// Frame step, above 0.
    #[serde(default = "one")]
    step: f64,
}

/// A channel sample.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ValueSample {
    frame: f64,
    value: f64,
}

/// `anim.function.sample` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FunctionSampleResult {
    samples: Vec<ValueSample>,
}

#[command(
    id = "anim.function.sample",
    kind = query,
    label = "Sample a channel",
    scopes = [read],
    needs_project,
    example(title = "Bob's rotation over frames 1-6", params = r#"{"nodeId": "scene:\"Intro\"/element:\"Bob\"", "channel": "rotation", "from": 1, "to": 6}"#),
    agent_only = "agents read curves numerically (05 §19.1.5); the function editor plots them (M6 C6.2)",
)]
pub(crate) fn anim_function_sample(cx: &mut Cx, p: FunctionSampleParams) -> Result<FunctionSampleResult, CommandError> {
    let doc = cx.document()?.clone();
    let id = node_id(cx, &p.node_id)?;
    let e = node(&doc, &id)?;
    check_channel(e, &p.channel)?;
    if p.channel == "position" {
        return Err(super::anim_common::unsupported(&p.channel, e, "the motion path is 3-D: use position.x, .y or .z"));
    }
    let mut ev = evaluator(&doc, "sampling a channel");
    ev.node(&id).map_err(eval_error)?;
    let mut samples = Vec::new();
    for frame in sample_frames(p.from, p.to, p.step)? {
        if !(1.0..=x::MAX_FRAMES as f64).contains(&frame) {
            return Err(frame_invalid(frame, "outside 1…100000"));
        }
        ev.charge(16)?;
        let data = ev.node(&id).map_err(eval_error)?;
        let local = data.bar.local(frame);
        let (fc, _) = data.bar.cycle_frame(local);
        let value = match p.channel.as_str() {
            "position.x" => data.position_at(local).position[0],
            "position.y" => data.position_at(local).position[1],
            "position.z" => data.position_at(local).position[2],
            "velocity" => data.velocity_at(fc),
            c => data.curve_at(c, fc).unwrap_or(match c {
                "scale.x" | "scale.y" => 1.0,
                _ => 0.0,
            }),
        };
        samples.push(ValueSample { frame: tidy(frame), value: tidy(value) });
    }
    Ok(FunctionSampleResult { samples })
}
