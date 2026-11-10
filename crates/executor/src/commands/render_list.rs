//! The compositor (06 §6.1–§6.2; ADR-0020; M3 C3.3): a frame's render list, built from the evaluated scene, and the
//! rasteriser items drawn from it. Renders, `describe_frame`, `hit_test` and the contact sheet all read this list, so
//! they agree on what is visible, in which order and where.
//!
//! - **Content**: drawing, image and annotation elements (cell-based elements with drawings). The exposed drawing at the
//!   scene frame (a cell-based element's local frame is its scene frame); a background element extends its last
//!   exposure to the scene length (04 §6.1; SI-106).
//! - **Visibility** as `anim.evaluate` reports it (SI-100): a hidden element or one under a hidden ancestor, and
//!   annotation elements (view-only, 04), are not drawn; nor are elements with no drawing at the frame, an empty
//!   drawing, or a plane at or behind the near plane. Each entry keeps its verdict.
//! - **Order** (SI-106): role tier (background, normal, foreground; 04 §6.1), then the world Z of the origin rounded to
//!   1e-6 field (05 §2.8; smaller Z first), then the Timeline order (index 0 of a level is the front, a parent's row
//!   above its children; back-to-front is the displayed rows bottom to top).
//! - **Projection**: an element's plane is parallel to the table plane (rotation is about Z only), so the camera's
//!   perspective takes it to output pixels by an affine map, fixed exactly by three projected points; drawing units are
//!   1/100 field (02).

use std::collections::{BTreeMap, BTreeSet};

use tt_anim::camera::CameraView;
use tt_anim::eval::Evaluator;
use tt_anim::matrix::Mat4;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::{Element, ElementType, Role};
use tt_model::entities::scene::TreeNode;
use tt_registry::CommandError;
use tt_render::Item;
use tt_vector::geom::{Affine, P, Rect};

use super::anim_common::eval_error;
use super::draw_render::Builder;
use super::timing::err;
use crate::app::AppContext;

/// Drawing units per field (02).
pub(crate) const DU_PER_FIELD: f64 = 100.0;

/// Why an element is or is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Drawn (at least partly inside the frame).
    Drawn,
    /// Hidden, or under a hidden ancestor (04 §5).
    Hidden,
    /// An annotation element: view-only (04).
    Annotation,
    /// No drawing exposed at the frame.
    NoDrawing,
    /// The exposed drawing has no visible geometry.
    Empty,
    /// The element's plane is at or behind the camera's near plane (05 §2.6).
    BehindCamera,
    /// Entirely outside the camera frame.
    OutOfFrame,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Verdict::Drawn => "drawn",
            Verdict::Hidden => "hidden",
            Verdict::Annotation => "annotation",
            Verdict::NoDrawing => "noDrawing",
            Verdict::Empty => "empty",
            Verdict::BehindCamera => "behindCamera",
            Verdict::OutOfFrame => "outOfFrame",
        }
    }
}

/// One content element of a frame.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub id: Id,
    pub name: String,
    pub kind: ElementType,
    pub role: Role,
    /// The drawing shown.
    pub drawing: Option<Id>,
    /// World matrix (fields).
    pub world: Mat4,
    /// World Z of the origin (fields).
    pub z: f64,
    /// Back-to-front Timeline rank.
    pub rank: usize,
    /// Drawing units → output pixels (when the plane is in front of the camera).
    pub to_pixel: Option<Affine>,
    /// Drawing bounds (du).
    pub bounds: Option<Rect>,
    /// Bounds in output pixels.
    pub pixel_box: Option<Rect>,
    pub verdict: Verdict,
}

/// A frame's render list, back to front.
#[derive(Debug, Clone)]
pub(crate) struct RenderList {
    pub scene: Id,
    pub frame: u32,
    pub camera: Id,
    /// The camera at the frame, at the output size.
    pub view: CameraView,
    pub width: u32,
    pub height: u32,
    /// Every content element of the composition, back to front; `verdict` says whether it is drawn.
    pub entries: Vec<Entry>,
}

/// What to build.
pub(crate) struct Request<'a> {
    pub scene: &'a Id,
    pub frame: u32,
    pub camera: Option<&'a Id>,
    pub width: u32,
    pub height: u32,
    /// Only these elements (composition `element`); `None`: every element.
    pub only: Option<&'a BTreeSet<Id>>,
}

/// The scene's elements in displayed Timeline order (front first, a parent before its children).
fn displayed(nodes: &[TreeNode], out: &mut Vec<Id>) {
    for n in nodes {
        out.push(n.id.clone());
        displayed(&n.children, out);
    }
}

/// Whether an element is hidden in its scene, itself or through an ancestor (as `anim.evaluate`, SI-100).
pub(crate) fn hidden(doc: &Document, tree: &[TreeNode], e: &Element) -> bool {
    !*e.visible_in_scene() || super::derived::ancestors_hidden(doc, tree, e.id())
}

/// The drawing an element shows at a scene frame, with a background element's last exposure extended to the scene
/// length (04 §6.1).
pub(crate) fn shown_drawing(doc: &Document, e: &Element, frame: u32, scene_length: u64) -> Option<Id> {
    if let Some(d) = AppContext::exposed_drawing(doc, e.id(), frame) {
        return Some(d);
    }
    if *e.role() != Role::Background || u64::from(frame) > scene_length {
        return None;
    }
    let runs = e.exposure().as_ref()?;
    let total: u64 = runs.iter().map(|(_, n)| u64::from(*n)).sum();
    if u64::from(frame) <= total {
        return None;
    }
    runs.iter().rev().find_map(|(d, _)| d.clone())
}

fn tier(role: Role) -> u8 {
    match role {
        Role::Background => 0,
        Role::Normal | Role::Envelope => 1,
        Role::Foreground => 2,
    }
}

/// The affine map drawing units → output pixels of a plane with world matrix `world`, or `None` when it is at or behind
/// the near plane.
pub(crate) fn plane_to_pixel(view: &CameraView, world: &Mat4) -> Option<Affine> {
    let at = |x: f64, y: f64| view.project(world.apply([x, y, 0.0])).pixel;
    let (o, a, b) = (at(0.0, 0.0)?, at(1.0, 0.0)?, at(0.0, 1.0)?);
    Some(Affine([
        (a[0] - o[0]) / DU_PER_FIELD,
        (a[1] - o[1]) / DU_PER_FIELD,
        (b[0] - o[0]) / DU_PER_FIELD,
        (b[1] - o[1]) / DU_PER_FIELD,
        o[0],
        o[1],
    ]))
}

/// The box of `r` through `m`.
pub(crate) fn map_rect(m: &Affine, r: &Rect) -> Rect {
    let c = [P::new(r.min_x, r.min_y), P::new(r.max_x, r.min_y), P::new(r.max_x, r.max_y), P::new(r.min_x, r.max_y)];
    c.iter().skip(1).fold(Rect::of(m.apply(c[0])), |acc, p| acc.with(m.apply(*p)))
}

/// Builds the render list of a frame.
///
/// # Errors
/// `render.noCamera` when the scene has no camera to render through; `anim.notFound` for an unknown camera; the
/// evaluator's typed refusals (`anim.valueOutOfRange`, `limit.exceeded`, `command.timeout`).
pub(crate) fn build(
    doc: &Document,
    req: &Request<'_>,
    ev: &mut Evaluator<'_, CommandError>,
) -> Result<RenderList, CommandError> {
    let scene = doc
        .scene(req.scene)
        .ok_or_else(|| err("ref.notFound", format!("no scene `{}`", req.scene), serde_json::json!({})))?;
    let camera = match req.camera {
        Some(c) => c.clone(),
        None => scene.current_camera_id().clone().ok_or_else(|| {
            err(
                "render.noCamera",
                format!("scene `{}` has no camera to render through", scene.name()),
                serde_json::json!({ "sceneId": scene.id().as_str() }),
            )
        })?,
    };
    let f = f64::from(req.frame);
    let mut view = ev.camera(&camera, f).map_err(eval_error)?;
    view.width = f64::from(req.width);
    view.height = f64::from(req.height);
    let length = doc.scene_length(scene);
    let mut order = Vec::new();
    displayed(scene.tree(), &mut order);
    let n = order.len();
    let frame_rect = Rect { min_x: 0.0, min_y: 0.0, max_x: f64::from(req.width), max_y: f64::from(req.height) };
    let mut entries = Vec::new();
    for (i, id) in order.iter().enumerate() {
        if req.only.is_some_and(|only| !only.contains(id)) {
            continue;
        }
        let Some(e) = doc.element(id) else { continue };
        let kind = *e.element_type();
        if !matches!(kind, ElementType::Drawing | ElementType::Image | ElementType::Annotation) {
            continue;
        }
        let world = ev.world(id, f).map_err(eval_error)?;
        let drawing = shown_drawing(doc, e, req.frame, length);
        let bounds =
            drawing.as_ref().and_then(|d| doc.drawing(d)).and_then(|d| tt_vector::drawing::drawing_bounds(d, false));
        let to_pixel = plane_to_pixel(&view, &world);
        let pixel_box = match (&to_pixel, &bounds) {
            (Some(m), Some(b)) => Some(map_rect(m, b)),
            _ => None,
        };
        let verdict = if hidden(doc, scene.tree(), e) {
            Verdict::Hidden
        } else if kind == ElementType::Annotation {
            Verdict::Annotation
        } else if drawing.is_none() {
            Verdict::NoDrawing
        } else if bounds.is_none() {
            Verdict::Empty
        } else if to_pixel.is_none() {
            Verdict::BehindCamera
        } else if pixel_box.is_some_and(|b| {
            b.max_x <= frame_rect.min_x
                || b.min_x >= frame_rect.max_x
                || b.max_y <= frame_rect.min_y
                || b.min_y >= frame_rect.max_y
        }) {
            Verdict::OutOfFrame
        } else {
            Verdict::Drawn
        };
        entries.push(Entry {
            id: id.clone(),
            name: e.name().clone(),
            kind,
            role: *e.role(),
            drawing,
            z: world.translation()[2],
            world,
            rank: n - 1 - i,
            to_pixel,
            bounds,
            pixel_box,
            verdict,
        });
    }
    // Z rounded to 1e-6 field: equal depths fall back to the Timeline order (a total order, so sorting is stable).
    entries.sort_by_key(|en| (tier(en.role), (en.z * 1e6).round() as i64, en.rank));
    Ok(RenderList {
        scene: req.scene.clone(),
        frame: req.frame,
        camera,
        view,
        width: req.width,
        height: req.height,
        entries,
    })
}

/// The rasteriser items of a render list: each drawn element's drawing, built in drawing units and wrapped in a layer
/// that takes them to output pixels. Returns the items, the warnings (objects not drawn and why) and the work used.
///
/// # Errors
/// `limit.exceeded` past the render work budget; the planar map's errors.
pub(crate) fn items(
    doc: &Document,
    blobs: Option<&tt_persist::container::Blobs>,
    cache: &crate::planar::SharedCache,
    list: &RenderList,
    work: u64,
) -> Result<(Vec<Item>, Vec<serde_json::Value>, u64), CommandError> {
    let styles: Option<BTreeMap<Id, Id>> =
        doc.scene(&list.scene).map(|s| s.palette_styles().iter().map(|(p, st)| (p.clone(), st.clone())).collect());
    let mut b = Builder::in_drawing_units(doc, blobs, styles, work);
    let mut out = Vec::new();
    for en in list.entries.iter().filter(|en| en.verdict == Verdict::Drawn) {
        let (Some(d), Some(m)) = (en.drawing.as_ref().and_then(|d| doc.drawing(d)), en.to_pixel) else { continue };
        let zones: BTreeMap<String, crate::planar::LayerZones> =
            crate::planar::drawing_zones(cache, d)?.into_iter().map(|lz| (lz.layer.id.clone(), lz)).collect();
        b.nodes(d.nodes(), &zones)?;
        let items = b.take_items();
        if !items.is_empty() {
            out.push(Item::Layer { transform: m.0, items });
        }
    }
    let used = b.work();
    Ok((out, std::mem::take(&mut b.warnings), used))
}
