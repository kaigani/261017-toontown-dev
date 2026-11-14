//! `inspect.*` (11 §15.2): visual and structural inspection. The frame description, hit test, coordinate conversion
//! and contact sheet read the compositor's render list (`render_list.rs`; M3 C3.3, ADR-0020), so they agree with the
//! rendered pixels; the GUI tree and screenshots come with the desktop shell (M4 C4.2).

#![allow(
    dead_code,
    reason = "declared commands: parameters are decoded and validated now and read by the handler of the owning checkpoint"
)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::num::tidy;
use tt_model::document::Document;
use tt_model::entities::element::ElementType;
use tt_registry::{CommandError, command};
use tt_vector::geom::{Affine, P};

use super::common::{Point, Rect};
use super::render;
use super::render_list::{self, DU_PER_FIELD, Overrides, RenderList, Request, Verdict};
use crate::cx::Cx;

/// Description detail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Detail {
    /// Summary.
    #[default]
    Summary,
    /// Everything.
    Full,
}

/// `inspect.frame.describe` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FrameParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame (default current).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Camera (default the scene's current camera).
    #[serde(default)]
    camera_id: Option<String>,
    /// Detail.
    #[serde(default)]
    detail: Detail,
    /// Elements to show although hidden or annotations (11 §7.5; the description of a render with the same overrides).
    #[serde(default)]
    include: Option<Vec<String>>,
    /// Elements to leave out, with their descendants (11 §7.5).
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

/// A rectangle `{x, y, width, height}`.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub(crate) struct Box2 {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Box2 {
    fn of(r: &tt_vector::geom::Rect) -> Self {
        Self { x: tidy(r.min_x), y: tidy(r.min_y), width: tidy(r.max_x - r.min_x), height: tidy(r.max_y - r.min_y) }
    }
}

/// Where an element is in the camera frame.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct CameraBounds {
    /// 0..1 from the frame's top-left (y down).
    normalized: Box2,
    /// Fields from the camera centre on the table plane (y up): `x`, `y` the bottom-left corner.
    fields: Box2,
    /// Output pixels of a render at the camera size.
    pixels: Box2,
}

/// An element's world transform.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct WorldTransform {
    /// 16 numbers, row-major.
    matrix: Vec<f64>,
    /// Origin (fields).
    position: [f64; 3],
    /// Rotation (degrees).
    rotation: f64,
    /// `[sx, sy]` (a mirror gives a negative `sy`, 05 §2.5).
    scale: [f64; 2],
    /// Skew (degrees).
    skew: f64,
}

/// A visible element (11 §7.5).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Described {
    element_id: String,
    name: String,
    /// The drawing shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    drawing_id: Option<String>,
    world_transform: WorldTransform,
    camera_bounds: CameraBounds,
    /// 1: elements have no opacity until effects (M7).
    opacity: f64,
    /// Active effects (none until M7).
    effects: Vec<Value>,
    /// `drawing`, `image` (full detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    /// `normal`, `foreground`, `background`, `envelope` (full detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    /// World Z of the origin (full detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<f64>,
    /// Drawing units → output pixels `[a, b, c, d, e, f]` (full detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    drawing_to_image: Option<[f64; 6]>,
    /// The drawing's bounds in drawing units (full detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    drawing_bounds: Option<Box2>,
}

/// An element of the frame that is not drawn.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Omitted {
    element_id: String,
    name: String,
    /// `hidden`, `annotation`, `noDrawing`, `empty`, `behindCamera`, `outOfFrame`.
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    drawing_id: Option<String>,
}

/// `inspect.frame.describe` result (11 §7.5, §15.2).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Description {
    scene_id: String,
    frame: u32,
    camera_id: String,
    /// The render size the pixel bounds refer to (the camera size).
    width: u32,
    height: u32,
    /// Visible elements in draw order, back to front.
    elements: Vec<Described>,
    /// Elements of the frame not drawn, and why (out-of-frame elements included).
    omitted: Vec<Omitted>,
    /// Audible sounds (M8).
    sounds: Vec<Value>,
    /// The mouth shape at the frame (M8).
    #[serde(skip_serializing_if = "Option::is_none")]
    mouth_shape: Option<String>,
    warnings: Vec<Value>,
    /// The document revision described.
    document_revision: u64,
}

/// The render list of a frame at the camera size, through a camera (default the scene's current one).
fn frame_list(
    cx: &Cx,
    scene: Option<&str>,
    frame: Option<u32>,
    camera: Option<&str>,
    overrides: &Overrides,
) -> Result<(Document, RenderList), CommandError> {
    let scene = render::scene_param(cx, scene)?;
    let frame = render::frame_param(cx, frame)?;
    let doc = cx.document()?.clone();
    let camera = camera.map(|r| super::anim_common::node_id(cx, r)).transpose()?;
    let (w, h) = render::output_size(&doc, None, None, 1.0)?;
    let req = Request { scene: &scene, frame, camera: camera.as_ref(), width: w, height: h, only: None, overrides };
    let mut ev = super::anim_common::evaluator(&doc, "describing a frame");
    let list = render_list::build(&doc, &req, &mut ev)?;
    drop(ev);
    Ok((doc, list))
}

fn role_str(r: tt_model::entities::element::Role) -> &'static str {
    use tt_model::entities::element::Role;
    match r {
        Role::Normal => "normal",
        Role::Foreground => "foreground",
        Role::Background => "background",
        Role::Envelope => "envelope",
    }
}

#[command(
    id = "inspect.frame.describe",
    kind = query,
    label = "Describe frame",
    scopes = [read],
    needs_project,
    example(title = "What the current frame shows", params = "{}"),
    example(title = "Frame 12 in full detail", params = r#"{"frame": 12, "detail": "full"}"#),
    agent_only = "agents verify frames structurally (11 §7.5 describe_frame)",
)]
pub(crate) fn inspect_frame_describe(cx: &mut Cx, p: FrameParams) -> Result<Description, CommandError> {
    let overrides = render::overrides(cx, p.include.as_deref(), p.exclude.as_deref())?;
    let (doc, list) = frame_list(cx, p.scene_id.as_deref(), p.frame, p.camera_id.as_deref(), &overrides)?;
    let full = p.detail == Detail::Full;
    let to_cam = render::image_to_camera(&list);
    let cam = Affine(to_cam);
    let (w, h) = (f64::from(list.width), f64::from(list.height));
    let mut elements = Vec::new();
    let mut omitted = Vec::new();
    let mut warnings = Vec::new();
    for en in &list.entries {
        let drawing_id = en.drawing.as_ref().map(|d| d.as_str().to_owned());
        if en.verdict != Verdict::Drawn {
            if en.verdict == Verdict::Empty {
                warnings.push(
                    json!({ "code": "describe.emptyExposure", "elementId": en.id.as_str(), "drawingId": drawing_id }),
                );
            }
            omitted.push(Omitted {
                element_id: en.id.as_str().to_owned(),
                name: en.name.clone(),
                reason: en.verdict.as_str().to_owned(),
                drawing_id,
            });
            continue;
        }
        let Some(px) = en.pixel_box else { continue };
        let d = en.world.decompose();
        let fields = render_list::map_rect(&cam, &px);
        elements.push(Described {
            element_id: en.id.as_str().to_owned(),
            name: en.name.clone(),
            drawing_id,
            world_transform: WorldTransform {
                matrix: en.world.0.iter().map(|v| tidy(*v)).collect(),
                position: d.position.map(tidy),
                rotation: tidy(d.rotation),
                scale: [tidy(d.sx), tidy(d.sy)],
                skew: tidy(d.skew),
            },
            camera_bounds: CameraBounds {
                normalized: Box2 {
                    x: tidy(px.min_x / w),
                    y: tidy(px.min_y / h),
                    width: tidy((px.max_x - px.min_x) / w),
                    height: tidy((px.max_y - px.min_y) / h),
                },
                fields: Box2::of(&fields),
                pixels: Box2::of(&px),
            },
            opacity: 1.0,
            effects: Vec::new(),
            kind: full.then(|| en.kind.as_str().to_owned()),
            role: full.then(|| role_str(en.role).to_owned()),
            depth: full.then(|| tidy(en.z)),
            drawing_to_image: if full { en.to_pixel.map(|m| m.0.map(tidy)) } else { None },
            drawing_bounds: if full { en.bounds.as_ref().map(Box2::of) } else { None },
        });
    }
    let has_sound = doc.scene(&list.scene).is_some_and(|s| {
        s.elements().ids().iter().any(|e| doc.element(e).is_some_and(|e| *e.element_type() == ElementType::Sound))
    });
    if has_sound {
        warnings.push(
            json!({ "code": "describe.soundUnavailable", "message": "audible sounds and mouth shapes arrive with M8" }),
        );
    }
    Ok(Description {
        scene_id: list.scene.as_str().to_owned(),
        frame: list.frame,
        camera_id: list.camera.as_str().to_owned(),
        width: list.width,
        height: list.height,
        elements,
        omitted,
        sounds: Vec::new(),
        mouth_shape: None,
        warnings,
        document_revision: doc.document_rev(),
    })
}

/// Coordinate space of a hit test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Space {
    /// View pixels (owner M4 C4.2).
    View,
    /// Drawing space (needs an element; convert to `scene` first).
    Drawing,
    /// Scene space, on the table plane (fields).
    Scene,
    /// Camera space: fields from the camera centre on the table plane, y up.
    Camera,
    /// Output pixels of a render at the camera size.
    Image,
}

/// `inspect.hitTest` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct HitParams {
    /// Space of `point`.
    space: Space,
    /// The point.
    point: Point,
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// View (owner M4 C4.2).
    #[serde(default)]
    view: Option<String>,
}

/// One object under the point.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HitOut {
    element_id: String,
    drawing_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    layer_id: Option<String>,
    object_id: String,
    /// `zone`, `brush`, `pencil`, `text`, `bitmap`.
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    colour_id: Option<String>,
}

/// `{hits}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Hits {
    /// Hits, top-most first.
    hits: Vec<HitOut>,
}

fn space_unavailable(space: &str, why: &str) -> CommandError {
    CommandError::with_details(
        "inspect.spaceUnavailable",
        format!("the `{space}` space {why}"),
        json!({ "space": space }),
    )
}

/// The node with `id`, at any depth.
fn find_node<'a>(
    nodes: &'a [tt_model::entities::drawing::DrawingNode],
    id: &str,
) -> Option<&'a tt_model::entities::drawing::DrawingNode> {
    nodes.iter().find_map(|n| {
        if n.id().as_str() == id { Some(n) } else { n.children().as_deref().and_then(|c| find_node(c, id)) }
    })
}

/// Whether a bitmap node is opaque at drawing-space point `p` (06 §6.8: fully transparent pixels are not hit).
fn bitmap_opaque(cx: &Cx, d: &tt_model::entities::drawing::Drawing, node_id: &str, p: P) -> bool {
    let Some(n) = find_node(d.nodes(), node_id) else { return false };
    let (Some(w), Some(h), Some(image)) = (*n.width(), *n.height(), n.image().as_ref()) else { return false };
    let m = n.transform().unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let q = Affine(render::invert(m)).apply(p);
    if !(q.x >= 0.0 && q.y >= 0.0 && q.x < w && q.y < h) {
        return false;
    }
    let Ok(project) = cx.project() else { return false };
    let Some((bw, bh, px)) =
        project.store.blobs.read(&image.entry, 256 << 20).ok().and_then(|b| tt_render::decode_png(&b, 16_384))
    else {
        return false;
    };
    let ix = ((q.x / w) * f64::from(bw)).floor() as usize;
    let iy = ((q.y / h) * f64::from(bh)).floor() as usize;
    let i = (iy.min(usize::from(bh).saturating_sub(1)) * usize::from(bw) + ix.min(usize::from(bw).saturating_sub(1)))
        * 4
        + 3;
    px.get(i).is_some_and(|a| *a > 0)
}

#[command(
    id = "inspect.hitTest",
    kind = query,
    label = "Hit test",
    scopes = [read],
    needs_project,
    example(title = "What is at the camera centre", params = r#"{"space": "camera", "point": {"x": 0, "y": 0}}"#),
    example(title = "What is under an image pixel of frame 12", params = r#"{"space": "image", "point": {"x": 960, "y": 540}, "frame": 12}"#),
    agent_only = "agents locate objects by position (11 §7.5 hit_test)",
)]
pub(crate) fn inspect_hit_test(cx: &mut Cx, p: HitParams) -> Result<Hits, CommandError> {
    if p.space == Space::View {
        return view_hits(cx, &p);
    }
    if p.view.is_some() {
        return Err(super::common::invalid("/view", "`view` applies to view-space points"));
    }
    if p.space == Space::Drawing {
        return Err(space_unavailable("drawing", "names no element; convert the point to `scene` first"));
    }
    if !(p.point.x.is_finite() && p.point.y.is_finite()) {
        return Err(super::common::invalid("/point", "the point must be finite"));
    }
    let (doc, list) = frame_list(cx, p.scene_id.as_deref(), p.frame, None, &Overrides::NONE)?;
    let pixel = match p.space {
        Space::Image => P::new(p.point.x, p.point.y),
        Space::Camera => Affine(render::invert(render::image_to_camera(&list))).apply(P::new(p.point.x, p.point.y)),
        _ => match list.view.project([p.point.x, p.point.y, 0.0]).pixel {
            Some(q) => P::new(q[0], q[1]),
            None => return Err(behind_camera(&[0])),
        },
    };
    let cache = cx.planar_cache();
    let mut hits = Vec::new();
    for en in list.entries.iter().rev().filter(|en| en.verdict == Verdict::Drawn) {
        let (Some(m), Some(did)) = (en.to_pixel, en.drawing.as_ref()) else { continue };
        let Some(d) = doc.drawing(did) else { continue };
        let inv = render::invert(m.0);
        if inv == [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] && m.det() == 0.0 {
            continue;
        }
        let q = Affine(inv).apply(pixel);
        let zones = crate::planar::drawing_zones(&cache, d)?;
        for hit in super::colour::hits(d, &zones, q, 0.0, false)? {
            let kind = hit.kind.as_str();
            if kind == "bitmap" && !bitmap_opaque(cx, d, &hit.id, q) {
                continue;
            }
            hits.push(HitOut {
                element_id: en.id.as_str().to_owned(),
                drawing_id: did.as_str().to_owned(),
                layer_id: hit.layer.clone(),
                object_id: hit.id.clone(),
                kind: kind.to_owned(),
                colour_id: hit.colour.clone(),
            });
        }
    }
    Ok(Hits { hits })
}

/// Hits under a view point of the Drawing view (SI-109): the current drawing's objects, top-most first.
fn view_hits(cx: &Cx, p: &HitParams) -> Result<Hits, CommandError> {
    if !(p.point.x.is_finite() && p.point.y.is_finite()) {
        return Err(super::common::invalid("/point", "the point must be finite"));
    }
    let hits = view_hit_list(cx, p.view.as_deref(), crate::shell::Point { x: p.point.x, y: p.point.y })?;
    Ok(Hits { hits })
}

/// The objects of the Drawing view's current drawing under a view point, top-most first (`inspect.hitTest` in view
/// space; the `pointer` tool's final hover).
pub(crate) fn view_hit_list(cx: &Cx, view: Option<&str>, at: crate::shell::Point) -> Result<Vec<HitOut>, CommandError> {
    let w = super::view::drawing_view(cx, view)?;
    let q = super::view::to_drawing(w, at);
    let doc = cx.document()?;
    let Some(did) = cx.app().current_drawing(doc) else { return Ok(Vec::new()) };
    let Some(d) = doc.drawing(&did) else { return Ok(Vec::new()) };
    let element = cx.app().element.as_ref().map(|e| e.as_str().to_owned()).unwrap_or_default();
    let zones = crate::planar::drawing_zones(&cx.planar_cache(), d)?;
    let mut hits = Vec::new();
    for hit in super::colour::hits(d, &zones, P::new(q.x, q.y), 0.0, false)? {
        let kind = hit.kind.as_str();
        if kind == "bitmap" && !bitmap_opaque(cx, d, &hit.id, P::new(q.x, q.y)) {
            continue;
        }
        hits.push(HitOut {
            element_id: element.clone(),
            drawing_id: did.as_str().to_owned(),
            layer_id: hit.layer.clone(),
            object_id: hit.id.clone(),
            kind: kind.to_owned(),
            colour_id: hit.colour.clone(),
        });
    }
    Ok(hits)
}

/// A point with an optional depth.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Point3 {
    /// X.
    x: f64,
    /// Y.
    y: f64,
    /// Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    z: Option<f64>,
}

/// `inspect.coordinates.convert` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ConvertParams {
    /// Source space: `drawing`, `local`, `parent` (these three with `elementId`), `scene` (or `world`, `field`),
    /// `camera`, `cameraNormalized`, `image` (SI-109; `view`: owner M4 C4.2).
    from: String,
    /// Target space (as `from`).
    to: String,
    /// Points.
    #[schemars(length(min = 1, max = 100_000))]
    points: Vec<Point3>,
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frame.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Element of `drawing`, `local` and `parent`.
    #[serde(default)]
    element_id: Option<String>,
    /// View (owner M4 C4.2).
    #[serde(default)]
    view: Option<String>,
}

/// `{points}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Points {
    /// Converted points.
    points: Vec<Point3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CSpace {
    Drawing,
    Local,
    Parent,
    Scene,
    Camera,
    CameraNormalized,
    Image,
}

fn cspace(s: &str, pointer: &str) -> Result<CSpace, CommandError> {
    Ok(match s {
        "drawing" => CSpace::Drawing,
        "local" => CSpace::Local,
        "parent" => CSpace::Parent,
        "scene" | "world" | "field" => CSpace::Scene,
        "camera" => CSpace::Camera,
        "cameraNormalized" => CSpace::CameraNormalized,
        "image" => CSpace::Image,
        other => {
            return Err(super::common::invalid(
                pointer,
                format!(
                    "unknown space `{other}` (view, drawing, local, parent, scene, camera, cameraNormalized, image)"
                ),
            ));
        }
    })
}

fn behind_camera(indexes: &[usize]) -> CommandError {
    CommandError::with_details(
        "inspect.behindCamera",
        "a point is at or behind the camera's near plane",
        json!({ "points": indexes }),
    )
}

fn singular(what: &str) -> CommandError {
    CommandError::with_details(
        "inspect.singularTransform",
        format!("the {what} transform is singular, so points cannot be mapped into it"),
        json!({ "transform": what }),
    )
}

#[command(
    id = "inspect.coordinates.convert",
    kind = query,
    label = "Convert coordinates",
    scopes = [read],
    needs_project,
    example(title = "Field units to camera space", params = r#"{"from": "field", "to": "camera", "points": [{"x": 1, "y": 1}]}"#),
    example(title = "An image pixel onto the table plane", params = r#"{"from": "image", "to": "scene", "points": [{"x": 960, "y": 540}]}"#),
    agent_only = "agents convert between coordinate spaces (11 §7.5 convert_coordinates)",
)]
pub(crate) fn inspect_coordinates_convert(cx: &mut Cx, p: ConvertParams) -> Result<Points, CommandError> {
    // View space is the Drawing view's pixels (SI-109; M4 C4.2): view points pass through drawing space with the one
    // mapping (design note 6), so `view` converts to and from every other space.
    let (vf, vt) = (p.from == "view", p.to == "view");
    if vf || vt {
        let w = super::view::drawing_view(cx, p.view.as_deref())?.clone();
        let pts: Vec<Point3> = if vf {
            p.points
                .iter()
                .map(|q| {
                    let d = super::view::to_drawing(&w, crate::shell::Point { x: q.x, y: q.y });
                    Point3 { x: d.x, y: d.y, z: None }
                })
                .collect()
        } else {
            p.points.clone()
        };
        let (from, to) = (
            if vf { "drawing".to_owned() } else { p.from.clone() },
            if vt { "drawing".to_owned() } else { p.to.clone() },
        );
        let inner = if from == to {
            if pts.iter().any(|q| !(q.x.is_finite() && q.y.is_finite())) {
                return Err(super::common::invalid("/points", "points must be finite"));
            }
            pts
        } else {
            let q = ConvertParams { from, to, points: pts, view: None, ..p };
            inspect_coordinates_convert(cx, q)?.points
        };
        let out = if vt {
            inner
                .iter()
                .map(|q| {
                    let v = super::view::to_view(&w, crate::shell::Point { x: q.x, y: q.y });
                    Point3 { x: v.x, y: v.y, z: None }
                })
                .collect()
        } else {
            inner
        };
        return Ok(Points { points: out });
    }
    if p.view.is_some() {
        return Err(super::common::invalid("/view", "`view` applies when `from` or `to` is the view space"));
    }
    let from = cspace(&p.from, "/from")?;
    let to = cspace(&p.to, "/to")?;
    if p.points.iter().any(|q| !(q.x.is_finite() && q.y.is_finite() && q.z.is_none_or(f64::is_finite))) {
        return Err(super::common::invalid("/points", "points must be finite"));
    }
    let (doc, list) = frame_list(cx, p.scene_id.as_deref(), p.frame, None, &Overrides::NONE)?;
    let needs_element = |s: CSpace| matches!(s, CSpace::Drawing | CSpace::Local | CSpace::Parent);
    let (world, parent) = if needs_element(from) || needs_element(to) {
        let r = p.element_id.as_deref().ok_or_else(|| {
            super::common::invalid("/elementId", "the drawing, local and parent spaces need elementId")
        })?;
        let id = super::anim_common::node_id(cx, r)?;
        let mut ev = super::anim_common::evaluator(&doc, "converting coordinates");
        let f = f64::from(list.frame);
        let w = ev.world(&id, f).map_err(super::anim_common::eval_error)?;
        let pw = ev.parent_world(&id, f).map_err(super::anim_common::eval_error)?;
        (Some(w), Some(pw))
    } else {
        (None, None)
    };
    let to_cam = render::image_to_camera(&list);
    let (w, h) = (f64::from(list.width), f64::from(list.height));
    let cam_z = list.view.position[2];
    let mut behind = Vec::new();
    let mut out = Vec::with_capacity(p.points.len());
    for (i, q) in p.points.iter().enumerate() {
        let z = q.z.unwrap_or(0.0);
        // Into scene space.
        let s: [f64; 3] = match from {
            CSpace::Scene => [q.x, q.y, z],
            CSpace::Drawing => world.map_or([0.0; 3], |m| m.apply([q.x / DU_PER_FIELD, q.y / DU_PER_FIELD, 0.0])),
            CSpace::Local => world.map_or([0.0; 3], |m| m.apply([q.x, q.y, z])),
            CSpace::Parent => parent.map_or([0.0; 3], |m| m.apply([q.x, q.y, z])),
            CSpace::Image | CSpace::Camera | CSpace::CameraNormalized => {
                let px = match from {
                    CSpace::Image => [q.x, q.y],
                    CSpace::CameraNormalized => [q.x * w, q.y * h],
                    _ => {
                        let r = Affine(render::invert(to_cam)).apply(P::new(q.x, q.y));
                        [r.x, r.y]
                    }
                };
                // Onto the plane Z = z (default the table plane).
                let depth = cam_z - z;
                match (depth > tt_anim::camera::NEAR_PLANE).then(|| list.view.unproject(px, depth)).flatten() {
                    Some(s) => s,
                    None => {
                        behind.push(i);
                        continue;
                    }
                }
            }
        };
        // Out of scene space.
        let r = match to {
            CSpace::Scene => Point3 { x: tidy(s[0]), y: tidy(s[1]), z: Some(tidy(s[2])) },
            CSpace::Drawing | CSpace::Local | CSpace::Parent => {
                let m = if to == CSpace::Parent { parent } else { world };
                let inv = m
                    .and_then(|m| m.inverse())
                    .ok_or_else(|| singular(if to == CSpace::Parent { "parent" } else { "element" }))?;
                let l = inv.apply(s);
                if to == CSpace::Drawing {
                    Point3 { x: tidy(l[0] * DU_PER_FIELD), y: tidy(l[1] * DU_PER_FIELD), z: None }
                } else {
                    Point3 { x: tidy(l[0]), y: tidy(l[1]), z: Some(tidy(l[2])) }
                }
            }
            CSpace::Image | CSpace::Camera | CSpace::CameraNormalized => {
                let Some(px) = list.view.project(s).pixel else {
                    behind.push(i);
                    continue;
                };
                match to {
                    CSpace::Image => Point3 { x: tidy(px[0]), y: tidy(px[1]), z: None },
                    CSpace::CameraNormalized => Point3 { x: tidy(px[0] / w), y: tidy(px[1] / h), z: None },
                    _ => {
                        let c = Affine(to_cam).apply(P::new(px[0], px[1]));
                        Point3 { x: tidy(c.x), y: tidy(c.y), z: None }
                    }
                }
            }
        };
        out.push(r);
    }
    if !behind.is_empty() {
        return Err(behind_camera(&behind));
    }
    Ok(Points { points: out })
}

/// The frames of a contact sheet (spec A `inspect.contactSheet.render`): a list, or a stepped range.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum FrameSelection {
    /// The frames listed.
    List(#[schemars(length(min = 1, max = 1000), inner(range(min = 1, max = 100_000)))] Vec<u32>),
    /// `from` to `to` every `step` frames.
    Range {
        /// First frame.
        #[schemars(range(min = 1, max = 100_000))]
        from: u32,
        /// Last frame.
        #[schemars(range(min = 1, max = 100_000))]
        to: u32,
        /// Step (default 1).
        #[serde(default)]
        #[schemars(range(min = 1, max = 100_000))]
        step: Option<u32>,
    },
}

/// `inspect.contactSheet.render` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContactSheetParams {
    /// Scene.
    #[serde(default)]
    scene_id: Option<String>,
    /// Frames: a list, or `{from, to, step}`.
    frames: FrameSelection,
    /// Columns (default ⌈√n⌉).
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    columns: Option<u32>,
    /// Cell width (default 240).
    #[serde(default)]
    #[schemars(range(min = 16, max = 2048))]
    cell_width: Option<u32>,
    /// Frame labels (default true).
    #[serde(default)]
    labels: Option<bool>,
    /// Elements to show although hidden or annotations (11 §7.5; render-time only).
    #[serde(default)]
    include: Option<Vec<String>>,
    /// Elements to leave out, with their descendants (11 §7.5; render-time only).
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

/// A cell of a contact sheet.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct Cell {
    frame: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// `inspect.contactSheet.render` result (11 §15.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContactSheet {
    /// The PNG, base64.
    png: String,
    width: u32,
    height: u32,
    /// Frame cells, in order.
    cells: Vec<Cell>,
    /// Encoded size.
    bytes: u64,
    /// SHA-256 of the PNG (hex).
    sha256: String,
    document_revision: u64,
    /// Objects not drawn and why.
    warnings: Vec<Value>,
}

/// Gap between cells and around the sheet (pixels).
const SHEET_GAP: u32 = 4;
/// Height of a label band under a cell (pixels).
const LABEL_BAND: u32 = 14;
/// Digits 0–9 as 3 x 5 bitmaps (rows top to bottom, bit 2 = left), drawn at 2 x 2 pixels per dot (SI-108).
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

/// Draws `text` (digits) with its top-left at (x, y) into a premultiplied RGBA buffer of width `w`.
fn draw_digits(buf: &mut [u8], w: u32, x: u32, y: u32, text: &str) {
    let mut cx = x;
    for ch in text.chars() {
        let Some(d) = ch.to_digit(10) else { continue };
        for (row, bits) in DIGITS[d as usize].iter().enumerate() {
            for col in 0..3u32 {
                if bits & (4 >> col) == 0 {
                    continue;
                }
                for dy in 0..2u32 {
                    for dx in 0..2u32 {
                        let (px, py) = (cx + col * 2 + dx, y + row as u32 * 2 + dy);
                        let i = ((py * w + px) * 4) as usize;
                        if let Some(p) = buf.get_mut(i..i + 4) {
                            p.copy_from_slice(&[0, 0, 0, 255]);
                        }
                    }
                }
            }
        }
        cx += 8;
    }
}

#[command(
    id = "inspect.contactSheet.render",
    kind = query,
    label = "Contact sheet",
    scopes = [read],
    needs_project,
    example(title = "Every fourth frame of 1–24", params = r#"{"frames": {"from": 1, "to": 24, "step": 4}}"#),
    example(title = "Three frames in one row, 120 pixels wide", params = r#"{"frames": [1, 12, 24], "columns": 3, "cellWidth": 120}"#),
    agent_only = "agents review motion in one image (11 §7.5 render_contact_sheet)",
)]
pub(crate) fn inspect_contact_sheet_render(cx: &mut Cx, p: ContactSheetParams) -> Result<ContactSheet, CommandError> {
    let frames: Vec<u32> = match &p.frames {
        FrameSelection::List(l) => l.clone(),
        FrameSelection::Range { from, to, step } => {
            if to < from {
                return Err(super::common::invalid("/frames/to", "`to` is before `from`"));
            }
            (*from..=*to).step_by(step.unwrap_or(1).max(1) as usize).collect()
        }
    };
    if frames.is_empty() || frames.len() > 1000 || frames.iter().any(|f| !(1..=100_000).contains(f)) {
        return Err(super::common::invalid("/frames", "1-1000 frames, each 1-100000"));
    }
    let scene = render::scene_param(cx, p.scene_id.as_deref())?;
    let doc = cx.document()?.clone();
    let n = u32::try_from(frames.len()).unwrap_or(1000);
    let cols = p.columns.unwrap_or_else(|| (f64::from(n)).sqrt().ceil() as u32).clamp(1, 64).min(n);
    let rows = n.div_ceil(cols);
    let cw = p.cell_width.unwrap_or(240).clamp(16, 2048);
    let (camw, camh) = (f64::from(*doc.project().camera_width()), f64::from(*doc.project().camera_height()));
    let ch = ((f64::from(cw) * camh / camw).round() as u32).max(1);
    let band = if p.labels.unwrap_or(true) { LABEL_BAND } else { 0 };
    let sheet_w = cols * cw + (cols + 1) * SHEET_GAP;
    let sheet_h = rows * (ch + band) + (rows + 1) * SHEET_GAP;
    if sheet_w > render::MAX_SIDE || sheet_h > render::MAX_SIDE {
        return Err(CommandError::with_details(
            "limit.exceeded",
            format!("a contact sheet of {sheet_w} x {sheet_h} pixels is over {} on a side", render::MAX_SIDE),
            json!({ "width": sheet_w, "height": sheet_h, "maxWidth": render::MAX_SIDE, "maxHeight": render::MAX_SIDE }),
        ));
    }
    // The sheet is inline: its size is known before any frame is evaluated or rendered (C33-R1-04).
    render::inline_side(cx, sheet_w, sheet_h)?;
    let overrides = render::overrides(cx, p.include.as_deref(), p.exclude.as_deref())?;
    let bg = render::background(&doc, &scene, "scene")?;
    let blobs = cx.project().ok().map(|pr| &pr.store.blobs);
    let cache = cx.planar_cache();
    let mut sheet = vec![255u8; (sheet_w * sheet_h * 4) as usize];
    let mut cells = Vec::with_capacity(frames.len());
    let mut warnings: Vec<Value> = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let i = u32::try_from(i).unwrap_or(0);
        let (col, row) = (i % cols, i / cols);
        let x = SHEET_GAP + col * (cw + SHEET_GAP);
        let y = SHEET_GAP + row * (ch + band + SHEET_GAP);
        let req = Request {
            scene: &scene,
            frame: *f,
            camera: None,
            width: cw,
            height: ch,
            only: None,
            overrides: &overrides,
        };
        let r = render::render_frame(&doc, blobs, &cache, &req, bg, &|| false)?;
        for w in r.warnings {
            if !warnings.contains(&w) {
                warnings.push(w);
            }
        }
        for yy in 0..ch {
            let src = (yy * cw * 4) as usize;
            let dst = (((y + yy) * sheet_w + x) * 4) as usize;
            sheet[dst..dst + (cw * 4) as usize].copy_from_slice(&r.pixels[src..src + (cw * 4) as usize]);
        }
        if band > 0 {
            draw_digits(&mut sheet, sheet_w, x + 2, y + ch + 2, &f.to_string());
        }
        cells.push(Cell { frame: *f, x, y, w: cw, h: ch });
    }
    // Encoding is bounded by the sheet size checked above; the caller's cancellation is observed around it.
    render::observe_cancel()?;
    let png = tt_render::encode_png(&sheet, sheet_w, sheet_h)
        .map_err(|e| CommandError::with_details("render.failed", e.to_string(), json!({})))?;
    render::observe_cancel()?;
    let (side, max_bytes) = render::inline_limits(cx);
    if sheet_w.max(sheet_h) > side || png.len() as u64 > max_bytes {
        return Err(CommandError::with_details(
            "render.inlineTooLarge",
            format!("a {sheet_w} x {sheet_h} contact sheet of {} bytes is over the inline limits", png.len()),
            json!({ "maxInlinePixels": side, "maxInlineBytes": max_bytes, "width": sheet_w, "height": sheet_h, "bytes": png.len() }),
        )
        .hint("use fewer columns or a smaller cellWidth"));
    }
    Ok(ContactSheet {
        sha256: render::sha256_hex(&png),
        bytes: png.len() as u64,
        png: tt_persist::b64::encode(&png),
        width: sheet_w,
        height: sheet_h,
        cells,
        document_revision: doc.document_rev(),
        warnings,
    })
}

/// `inspect.ui.tree` parameters (11 §7.5 `inspect_ui`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UiTreeParams {
    /// `window` (default), a panel or view id (01 §5.2), a tool palette id, or `dialogs`; an unknown target, however
    /// long, is `window.panelNotFound` (no schema bound pre-empts it: C41-R2-21 audit).
    #[serde(default)]
    target: Option<String>,
    /// Levels below the target to include (default: all).
    #[serde(default)]
    #[schemars(range(min = 1, max = 32))]
    depth: Option<u32>,
}

/// A node of the virtual UI tree (headless, or before the GUI has reported one).
fn node(role: &str, id: &str, label: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("role".into(), json!(role));
    m.insert("id".into(), json!(id));
    m.insert("label".into(), json!(label));
    m
}

/// The tree the kernel can describe itself (11 §3 virtual views): window, main toolbar, tool palettes, panels with
/// their place, view windows with type, size and transform, status bar, start screen. Every widget that invokes a
/// command names it.
fn virtual_tree(cx: &Cx) -> Value {
    let ui = cx.ui();
    let layout = ui.layout();
    let mut root = node("window", "main", crate::help::APP_TITLE);
    root.insert("fullScreen".into(), json!(ui.full_screen()));
    let mut children: Vec<Value> = Vec::new();
    let mut toolbar = node("toolbar", "mainToolbar", "Main toolbar");
    toolbar.insert("visible".into(), json!(layout.main_toolbar.visible));
    toolbar.insert(
        "children".into(),
        Value::Array(
            layout
                .main_toolbar
                .items
                .iter()
                .map(|i| {
                    let mut b = node("button", i, i);
                    if i.contains('.') {
                        b.insert("command".into(), json!({ "commandId": i }));
                    }
                    Value::Object(b)
                })
                .collect(),
        ),
    );
    children.push(Value::Object(toolbar));
    for p in &layout.tool_palettes {
        let mut t = node("toolPalette", &p.id, &p.id);
        t.insert("visible".into(), json!(p.visible));
        t.insert("floating".into(), json!(p.floating));
        children.push(Value::Object(t));
    }
    for p in ui.panels() {
        let mut n = node("panel", &p.panel_id, &p.title);
        n.insert("visible".into(), json!(p.visible));
        n.insert("floating".into(), json!(p.floating));
        n.insert("focused".into(), json!(p.focused));
        if let Some(a) = &p.area {
            n.insert("area".into(), json!(a));
        }
        if let Ok(v) = ui.shell().view(&p.panel_id) {
            let t = v.transform();
            n.insert(
                "view".into(),
                json!({ "viewId": v.view_id, "type": v.ty.as_str(), "size": v.size,
                        "transform": { "zoom": t.zoom, "rotationDegrees": t.rotation_degrees, "centre": t.centre } }),
            );
        }
        children.push(Value::Object(n));
    }
    let mut bar = node("statusBar", "statusBar", "Status bar");
    bar.insert(
        "visible".into(),
        json!(cx.pref("interface.statusBar.visible").and_then(|v| v.as_bool()).unwrap_or(true)),
    );
    bar.insert(
        "children".into(),
        json!([
            { "role": "text", "id": "status.description", "value": ui.shell().sensors.description },
            { "role": "text", "id": "status.pointer", "value": ui.shell().sensors.pointer },
            { "role": "text", "id": "status.agent", "value": super::ui_shell::agent_indicator(cx) }
        ]),
    );
    children.push(Value::Object(bar));
    let project = cx.session().project.as_ref().map(|p| p.doc.project().id().as_str().to_owned());
    if ui.shell().start_screen_visible(project.as_deref()) {
        let mut st = node("startScreen", "startScreen", "Start screen");
        st.insert("children".into(), json!([
            { "role": "button", "id": "start.create", "label": "Create", "command": { "commandId": "project.create" } },
            { "role": "button", "id": "start.open", "label": "Open other", "command": { "commandId": "project.open" } }
        ]));
        children.push(Value::Object(st));
    }
    children.push(json!({ "role": "dialogs", "id": "dialogs", "label": "Dialogs", "children": [] }));
    root.insert("children".into(), Value::Array(children));
    Value::Object(root)
}

fn find<'a>(n: &'a Value, id: &str) -> Option<&'a Value> {
    if n.get("id").and_then(Value::as_str) == Some(id) {
        return Some(n);
    }
    n.get("children").and_then(Value::as_array).and_then(|c| c.iter().find_map(|x| find(x, id)))
}

fn prune(n: &mut Value, depth: u32) {
    if let Some(m) = n.as_object_mut() {
        if depth == 0 {
            if let Some(c) = m.shift_remove("children") {
                m.insert("childCount".into(), json!(c.as_array().map_or(0, Vec::len)));
            }
            return;
        }
        if let Some(Value::Array(c)) = m.get_mut("children") {
            c.iter_mut().for_each(|x| prune(x, depth - 1));
        }
    }
}

/// A required member that is a 64-bit unsigned integer or `null` (`renderedSeq`: always present, `null` when unvouched).
fn nullable_u64(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": ["integer", "null"], "minimum": 0, "maximum": u64::MAX })
}

/// `inspect.ui.tree` result (11 §7.5; additive fields `renderedSeq` and `stale`: ADR-0021 decision 5, SI-121).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiTreeResult {
    /// `gui` (the tree the front end rendered and reported) or `virtual` (the kernel's tree, headless).
    source: String,
    /// The target the tree starts at.
    target: String,
    /// The tree: nodes with `role`, `id`, `label`, `value`, `enabled`, `checked`, `visible`, `command`, `children`.
    root: Value,
    /// The engine event number the tree vouches for: every window of the freshness quorum rendered the events up to
    /// it; `null` when the tree is not vouched for or is the headless virtual tree. Always present.
    #[schemars(required, schema_with = "nullable_u64")]
    rendered_seq: Option<u64>,
    /// Set by the MCP `inspect_ui` tool only: `true` when the front end had not rendered the events before the call
    /// within 2 s (the tree is the last one it has). Absent otherwise; never `null`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    stale: Option<bool>,
}

#[command(
    id = "inspect.ui.tree",
    kind = query,
    label = "UI tree",
    scopes = [read],
    example(title = "The window's UI tree, 3 levels", params = r#"{"depth": 3}"#),
    example(title = "The History panel", params = r#"{"target": "history"}"#),
    agent_only = "agents inspect the GUI (11 §7.5 inspect_ui)",
)]
pub(crate) fn inspect_ui_tree(cx: &mut Cx, p: UiTreeParams) -> Result<UiTreeResult, CommandError> {
    // Attached mode: the tree the front end rendered and reported (ADR-0021); headless: the virtual tree.
    // `renderedSeq`: the engine event the accepted GUI tree reflects (C41-R2-07, SI-121); `null` when the tree is not
    // vouched for or is the headless virtual tree.
    let sensors = &cx.ui().shell().sensors;
    let (source, tree, seq) = match sensors.ui_tree.clone() {
        Some(t) if t.is_object() => ("gui", t, sensors.ui_tree_seq),
        _ => ("virtual", virtual_tree(cx), None),
    };
    let target = p.target.unwrap_or_else(|| "window".to_owned());
    let mut sub = if target == "window" {
        tree
    } else {
        find(&tree, &target).cloned().ok_or_else(|| {
            CommandError::with_details(
                "window.panelNotFound",
                format!("the UI tree has no `{target}`"),
                json!({ "target": target }),
            )
            .hint("targets are window, dialogs, a panel or view id (01 §5.2) or a tool palette id")
        })?
    };
    if let Some(d) = p.depth {
        prune(&mut sub, d);
    }
    Ok(UiTreeResult { source: source.to_owned(), target, root: sub, rendered_seq: seq, stale: None })
}

/// `inspect.screenshot` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScreenshotParams {
    /// Window, view, panel or dialog.
    #[serde(default = "window")]
    target: String,
    /// Region.
    #[serde(default)]
    region: Option<Rect>,
}

fn window() -> String {
    "window".into()
}

/// The inline side limit of a screenshot (11 §6.5: 2048 px).
pub(crate) const SCREENSHOT_MAX_SIDE: u32 = 2048;

fn inline_too_large(w: u32, h: u32, bytes: Option<usize>, max_bytes: u64) -> CommandError {
    CommandError::with_details(
        "render.inlineTooLarge",
        format!("a {w} x {h} screenshot is over the inline limits ({SCREENSHOT_MAX_SIDE} px, {max_bytes} bytes)"),
        json!({ "maxInlinePixels": SCREENSHOT_MAX_SIDE, "maxInlineBytes": max_bytes, "width": w, "height": h,
                "bytes": bytes }),
    )
    .hint("pass a region of at most 2048 x 2048 view pixels")
}

/// The headless screenshot (design note 5 §1): the target view window rendered at its virtual size through the final
/// software rasteriser, the current drawing through the view's mapping on the Drawing-view background. The display
/// aids are drawn by the interactive renderer only (06 §6.10).
fn virtual_screenshot(cx: &Cx, p: &ScreenshotParams) -> Result<ScreenshotResult, CommandError> {
    let shell = cx.ui().shell();
    let target = p.target.clone();
    let id = match target.as_str() {
        "window" => {
            let active = cx.ui().active_view();
            if shell.view(active).is_ok_and(|w| w.ty == crate::shell::ViewType::Drawing) {
                active.to_owned()
            } else {
                "drawingView".to_owned()
            }
        }
        "drawing" => "drawingView".to_owned(),
        other => other.to_owned(),
    };
    let w = shell.view(&id).map_err(|_| {
        CommandError::with_details(
            "view.notFound",
            format!("headless, `{target}` has nothing to capture (only view windows are rendered)"),
            json!({ "target": target, "pointer": "/target" }),
        )
        .hint("target window, drawing or a view id (inspect.ui.tree lists them)")
    })?;
    if w.ty != crate::shell::ViewType::Drawing {
        return Err(CommandError::with_details(
            tt_registry::codes::COMMAND_NOT_IMPLEMENTED,
            format!("headless screenshots of the {} view arrive with that view (M4 C4.3)", w.ty.as_str()),
            json!({ "target": target, "viewType": w.ty, "milestone": "M4 C4.3" }),
        ));
    }
    let (vw, vh) = (w.size.width.round(), w.size.height.round());
    let region = match &p.region {
        Some(r) => {
            if ![r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) || r.w < 1.0 || r.h < 1.0 {
                return Err(super::common::invalid(
                    "/region",
                    "the region needs a finite position and a size of at least 1 px",
                ));
            }
            (r.x.round(), r.y.round(), r.w.round(), r.h.round())
        }
        None => (0.0, 0.0, vw, vh),
    };
    let (_, max_bytes) = render::inline_limits(cx);
    let side = |v: f64| if v.is_finite() && v >= 1.0 && v <= f64::from(u32::MAX) { v as u32 } else { u32::MAX };
    let (width, height) = (side(region.2), side(region.3));
    // The caps are checked before anything is rendered (S2).
    if width.max(height) > SCREENSHOT_MAX_SIDE {
        return Err(inline_too_large(width, height, None, max_bytes));
    }
    let bg = &cx.ui().drawing().aids.background;
    let colour = if bg.use_scene_colour {
        cx.document()
            .ok()
            .and_then(|doc| {
                cx.app().scene.as_ref().and_then(|s| doc.scene(s)).map(|s| s.background_colour().to_string())
            })
            .unwrap_or_else(|| bg.colour.clone())
    } else {
        bg.colour.clone()
    };
    let background = tt_model::value::Colour::parse(&colour).map(|c| c.channels()).unwrap_or([240, 240, 240, 255]);
    let m = super::view::drawing_to_view(w);
    let d2p = Affine([m[0], m[1], m[2], m[3], m[4] - region.0, m[5] - region.1]);
    let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
    // The snapshot (design note 5 §1a; C42A-R1-02): the document by reference count, the drawing's id, the planar
    // cache's handle and the blob store; the rasterising and the encoding run on a job thread, never on this one.
    let doc = cx.document().ok().cloned();
    let drawing_id = doc.as_ref().and_then(|d| cx.app().current_drawing(d));
    let capture = Capture {
        doc,
        drawing_id: drawing_id.clone(),
        cache: cx.planar_cache(),
        blobs: cx.project().ok().map(|pr| pr.store.blobs.clone()),
        raster: super::draw_render::Raster {
            d2p,
            scale,
            width,
            height,
            background,
            show_strokes: false,
            show_selection: false,
        },
        max_bytes,
    };
    let mut result = ScreenshotResult {
        png: None,
        width,
        height,
        sha256: None,
        target,
        view_id: id,
        source: "virtual".into(),
        drawing_id: drawing_id.map(|d| d.as_str().to_owned()),
        note:
            "headless: the drawing through the view mapping on the Drawing-view background; display aids are drawn by \
               the interactive renderer only (06 §6.10)"
                .into(),
        job_id: None,
    };
    if cx.dry_run() {
        result.job_id = Some("dryRun".into());
        return Ok(result);
    }
    let done = result.clone();
    let origin = cx.origin().clone();
    let job = cx.session().jobs.submit("inspect.screenshot", &origin.session_key(), move |progress| {
        let (png, sha) = capture.run(&|| progress.cancelled())?;
        let finished = ScreenshotResult { png: Some(png), sha256: Some(sha), ..done };
        serde_json::to_value(finished).map_err(|e| CommandError::internal(e.to_string()))
    })?;
    result.job_id = Some(job);
    Ok(result)
}

/// What a headless capture needs, taken on the document thread (C42A-R1-02).
struct Capture {
    doc: Option<tt_model::document::Document>,
    drawing_id: Option<tt_model::Id>,
    cache: crate::planar::SharedCache,
    blobs: Option<tt_persist::container::Blobs>,
    raster: super::draw_render::Raster,
    max_bytes: u64,
}

impl Capture {
    /// Rasterises and encodes off the document thread: the PNG (base64) and its sha256.
    fn run(&self, stop: &dyn Fn() -> bool) -> Result<(String, String), CommandError> {
        let r = self.raster;
        let drawing =
            self.doc.as_ref().zip(self.drawing_id.as_ref()).and_then(|(doc, id)| doc.drawing(id).map(|d| (doc, d)));
        let pixels = match drawing {
            Some((doc, d)) => {
                let src = super::draw_render::Sources {
                    cache: &self.cache,
                    blobs: self.blobs.as_ref(),
                    selection: None,
                    stop,
                };
                super::draw_render::rasterise_with(&src, doc, d, None, r, std::time::Instant::now())?.0
            }
            None => r.background.repeat(r.width as usize * r.height as usize),
        };
        if stop() {
            return Err(CommandError::new("job.cancelled", "the capture was cancelled"));
        }
        let png = tt_render::encode_png(&pixels, r.width, r.height)
            .map_err(|e| CommandError::with_details("render.failed", e.to_string(), json!({})))?;
        // The byte cap is known only after encoding (design note 5 §3; ADR-0020 decision 8).
        if png.len() as u64 > self.max_bytes {
            return Err(inline_too_large(r.width, r.height, Some(png.len()), self.max_bytes));
        }
        Ok((tt_persist::b64::encode(&png), render::sha256_hex(&png)))
    }
}

/// `inspect.screenshot` result (spec A row 1062: `{png, width, height, sha256}`; design note 5 §3).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScreenshotResult {
    /// The PNG, base64 (`null` only inside a batch, where the capture job's id is answered instead of waiting).
    png: Option<String>,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// The PNG's sha256 (hex; `null` with `png`).
    sha256: Option<String>,
    /// The target captured.
    target: String,
    /// The view window captured.
    view_id: String,
    /// `virtual` (headless) or `gui` (attached, C4.2b).
    source: String,
    /// The drawing shown.
    drawing_id: Option<String>,
    /// What a headless capture contains.
    note: String,
    /// The capture job (the handle waits for it; design note 5 §1a).
    #[serde(skip_serializing_if = "Option::is_none")]
    job_id: Option<String>,
}

#[command(
    id = "inspect.screenshot",
    kind = query,
    label = "Screenshot",
    scopes = [read],
    example(title = "The active view", params = "{}"),
    example(title = "A 400 x 300 region of the Drawing view", params = r#"{"target": "drawingView", "region": {"x": 760, "y": 390, "w": 400, "h": 300}}"#),
    agent_only = "agents look at the GUI (11 §7.8 screenshot)",
)]
pub(crate) fn inspect_screenshot(cx: &mut Cx, p: ScreenshotParams) -> Result<ScreenshotResult, CommandError> {
    virtual_screenshot(cx, &p)
}
