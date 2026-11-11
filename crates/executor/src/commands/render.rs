//! `render.*` (09 §11; M3 C3.3, ADR-0020): deterministic frame renders through the compositor (`render_list.rs`) and
//! the final-quality rasteriser (`tt-render`, ADR-0006), the inline/file rules and the image-sequence job.
//!
//! - **Size**: the project's camera size, or `width`/`height` (one alone keeps the camera aspect), times `scale`; each
//!   side 1–8192 (`limit.exceeded` otherwise, before any work).
//! - **Background**: `scene` (the scene's background colour), `transparent`, or a colour.
//! - **Determinism** (09 §11.1 item 2): equal document revision and parameters give equal pixels and, for PNG, equal
//!   files (no metadata); JPEG, BMP and TGA through the pinned encoders, no timestamps.
//! - **Inline vs file** (09 §11.1 item 3): inline when the long side is within the session's inline image size and the
//!   encoded image within 8 MB (and the session's inline payload limit), else `render.inlineTooLarge`; `output: "file"`
//!   writes an absolute `path` under the file-access policy with `onConflict` (`export.fileExists`).
//! - **Never block**: the render charges a work account (path elements and pixels / 1024) and observes the caller's
//!   cancellation inside the rasteriser; a range runs as a job on a snapshot, one frame at a time, cancellable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::Document;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_render::Scene;

use super::anim_common::evaluator;
use super::common::{Job, NoParams, invalid};
use super::render_list::{self, Overrides, RenderList, Request};
use super::timing::err;
use crate::cx::Cx;

/// The longest side of a render (09 §11.4; `tt_render::MAX_SIDE`).
pub(crate) const MAX_SIDE: u32 = tt_render::MAX_SIDE;
/// The default inline image limit (09 §11.1 item 3): 8 MB encoded.
pub(crate) const INLINE_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// Work units one frame may spend (path elements and pixels / 1024), as a drawing render.
const FRAME_WORK_BUDGET: u64 = 200_000_000;
/// Wall-clock limit of one frame's rasterisation (under the executor's watchdog budget).
const FRAME_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(45);
/// Frames one range may render.
pub(crate) const MAX_RANGE_FRAMES: u32 = 100_000;

/// Composition (09 §11.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Composition {
    /// Through the scene's camera.
    #[default]
    Camera,
    /// The exposure-sheet composition (owner M5 C5.1).
    Xsheet,
    /// The given elements through the camera, or with `drawingId` one drawing in drawing space.
    Element,
    /// What the active view shows (owner M4 C4.2).
    ActiveView,
}

/// Quality (09 §11.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Quality {
    /// The final-quality renderer.
    #[default]
    Final,
    /// The interactive renderer's output (until M4 C4.2: as final, SI-107).
    Draft,
}

/// Image format of `render.frame.run`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FrameFormat {
    /// PNG.
    #[default]
    Png,
    /// JPEG.
    Jpeg,
}

/// Image format of `render.range.run`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SequenceFormat {
    /// PNG.
    #[default]
    Png,
    /// JPEG.
    Jpeg,
    /// TGA.
    Tga,
    /// BMP.
    Bmp,
}

/// Inline or file output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Output {
    /// Base64 in the result.
    #[default]
    Inline,
    /// Written to `path`.
    File,
}

/// What to do with an existing output file (09 §11.1 item 5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnConflict {
    /// Refuse (`export.fileExists`).
    #[default]
    Fail,
    /// Replace (confirmable under Assist, 11 §6.2).
    Overwrite,
    /// Append ` (n)` before the extension.
    Rename,
}

fn scene_default() -> String {
    "scene".to_owned()
}

fn one() -> f64 {
    1.0
}

fn ninety() -> u8 {
    90
}

/// `render.frame.run` parameters (09 §11.2).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FrameParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// Scene frame (default current).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// Composition.
    #[serde(default)]
    composition: Composition,
    /// Elements of the `element` composition.
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// The drawing of the `element` composition in drawing space.
    #[serde(default)]
    drawing_id: Option<String>,
    /// Width in pixels (default the camera's).
    #[serde(default)]
    #[schemars(range(min = 1, max = 8192))]
    width: Option<u32>,
    /// Height in pixels (default by the camera aspect).
    #[serde(default)]
    #[schemars(range(min = 1, max = 8192))]
    height: Option<u32>,
    /// Scale of the size.
    #[serde(default = "one")]
    #[schemars(range(min = 0.05, max = 8.0))]
    scale: f64,
    /// Quality.
    #[serde(default)]
    quality: Quality,
    /// `scene`, `transparent` or a colour.
    #[serde(default = "scene_default")]
    background: String,
    /// Outline the camera frame (compositions larger than the frame; SI-107).
    #[serde(default)]
    camera_frame: bool,
    /// Format.
    #[serde(default)]
    format: FrameFormat,
    /// JPEG quality.
    #[serde(default = "ninety")]
    #[schemars(range(min = 0, max = 100))]
    jpeg_quality: u8,
    /// Inline or file.
    #[serde(default)]
    output: Output,
    /// Output file (absolute).
    #[serde(default)]
    path: Option<String>,
    /// Existing file.
    #[serde(default)]
    on_conflict: OnConflict,
    /// Camera (default the scene's current camera; the generic `render_frame` tool's `camera`, SI-107).
    #[serde(default)]
    camera_id: Option<String>,
    /// Elements to show although hidden or annotations (11 §7.5; render-time only, the document is not changed).
    #[serde(default)]
    include: Option<Vec<String>>,
    /// Elements to leave out, with their descendants (11 §7.5; render-time only).
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

/// `RenderResult` (09 §11.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenderResult {
    /// `image/png` or `image/jpeg`.
    pub mime_type: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Encoded size.
    pub bytes: u64,
    /// SHA-256 of the encoded image (hex).
    pub sha256: String,
    /// The image, base64 (inline output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// The written file (file output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The document revision rendered.
    pub document_revision: u64,
    /// The scene frame rendered.
    pub frame: u32,
    /// Wall-clock time of the call in milliseconds (11 §7.5): volatile, not part of the determinism contract (the image
    /// bytes and `sha256` are; SI-107).
    pub render_ms: u64,
    /// Output pixels → camera space (fields from the camera centre on the table plane, y up) `[a, b, c, d, e, f]`
    /// (11 §7.5 `imageToCamera`); for a drawing-space render, output pixels → drawing units.
    pub image_to_camera: [f64; 6],
    /// Objects not drawn and why, and notes (`render.draftAsFinal`).
    pub warnings: Vec<Value>,
}

/// The rendered pixels of a frame.
pub(crate) struct Rendered {
    pub pixels: Vec<u8>,
    pub list: RenderList,
    pub warnings: Vec<Value>,
}

/// The output size: the camera's, or `width`/`height` (one alone keeps the camera aspect), times `scale`.
///
/// # Errors
/// `arg.invalid` for a scale outside 0.05–8; `limit.exceeded` for a side outside 1–8192.
pub(crate) fn output_size(
    doc: &Document,
    width: Option<u32>,
    height: Option<u32>,
    scale: f64,
) -> Result<(u32, u32), CommandError> {
    if !(scale.is_finite() && (0.05..=8.0).contains(&scale)) {
        return Err(invalid("/scale", "the scale must be 0.05-8"));
    }
    let (cw, ch) = (f64::from(*doc.project().camera_width()), f64::from(*doc.project().camera_height()));
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) => (f64::from(w), f64::from(h)),
        (Some(w), None) => (f64::from(w), (f64::from(w) * ch / cw).round()),
        (None, Some(h)) => ((f64::from(h) * cw / ch).round(), f64::from(h)),
        (None, None) => (cw, ch),
    };
    let (w, h) = ((w * scale).round(), (h * scale).round());
    if !(1.0..=f64::from(MAX_SIDE)).contains(&w) || !(1.0..=f64::from(MAX_SIDE)).contains(&h) {
        return Err(err(
            "limit.exceeded",
            format!("a render of {w} x {h} pixels is outside 1-{MAX_SIDE} on a side"),
            json!({ "width": w, "height": h, "maxWidth": MAX_SIDE, "maxHeight": MAX_SIDE }),
        ));
    }
    Ok((w as u32, h as u32))
}

/// The background of a render: `scene`, `transparent` or a colour.
///
/// # Errors
/// `arg.invalid`.
pub(crate) fn background(doc: &Document, scene: &Id, given: &str) -> Result<[u8; 4], CommandError> {
    match given {
        "scene" | "sceneColour" => {
            Ok(doc.scene(scene).map_or([255, 255, 255, 255], |s| s.background_colour().channels()))
        }
        "transparent" => Ok([0, 0, 0, 0]),
        c => tt_model::value::Colour::parse(c)
            .map(|c| c.channels())
            .map_err(|_| invalid("/background", "the background is scene, transparent, #RRGGBB or #RRGGBBAA")),
    }
}

/// The scene of a `sceneId` parameter, or the current scene, or the first scene.
///
/// # Errors
/// `ref.notFound` and the reference errors; `render.noScene` when the project has no scene.
pub(crate) fn scene_param(cx: &Cx, given: Option<&str>) -> Result<Id, CommandError> {
    if let Some(r) = given {
        return Ok(Id::parse(&cx.resolve_as(r, ObjectType::Scene)?)?);
    }
    let doc = cx.document()?;
    cx.app()
        .scene
        .clone()
        .filter(|s| doc.scene(s).is_some())
        .or_else(|| doc.scenes().next().map(|s| s.id().clone()))
        .ok_or_else(|| err("render.noScene", "the project has no scene", json!({})))
}

/// A frame parameter or the current frame, inside 1–100 000.
pub(crate) fn frame_param(cx: &Cx, given: Option<u32>) -> Result<u32, CommandError> {
    let f = given.unwrap_or(cx.app().frame);
    if !(1..=100_000).contains(&f) {
        return Err(err("render.frameInvalid", format!("frame {f} is outside 1-100000"), json!({ "frame": f })));
    }
    Ok(f)
}

/// Rasterises a scene, observing the caller's cancellation and the frame's time limit.
///
/// # Errors
/// `command.timeout` when the caller's budget passed; `limit.exceeded` past the time limit; `render.failed`.
pub(crate) fn rasterise(scene: &Scene, cancelled: &dyn Fn() -> bool) -> Result<Vec<u8>, CommandError> {
    let started = std::time::Instant::now();
    let mut keep_going = |_: usize| {
        if tt_vector::work::cancel_requested() || cancelled() {
            Err("cancelled".to_owned())
        } else if started.elapsed() > FRAME_TIME_LIMIT {
            Err("time".to_owned())
        } else {
            Ok(())
        }
    };
    tt_render::render(scene, &mut keep_going).map_err(|e| match e {
        tt_render::RenderError::Stopped(m) if m == "cancelled" => CommandError::new(
            tt_registry::codes::COMMAND_TIMEOUT,
            "the render was cancelled: the call overran its watchdog budget or its job was cancelled",
        ),
        tt_render::RenderError::Stopped(_) => err(
            "limit.exceeded",
            format!("the render took longer than {} s", FRAME_TIME_LIMIT.as_secs()),
            json!({ "limitSeconds": FRAME_TIME_LIMIT.as_secs() }),
        ),
        other => err("render.failed", other.to_string(), json!({})),
    })
}

/// Renders a frame of `doc`: the render list, its items, the pixels.
///
/// # Errors
/// As [`render_list::build`], [`render_list::items`] and [`rasterise`].
pub(crate) fn render_frame(
    doc: &Document,
    blobs: Option<&tt_persist::container::Blobs>,
    cache: &crate::planar::SharedCache,
    req: &Request<'_>,
    background: [u8; 4],
    cancelled: &dyn Fn() -> bool,
) -> Result<Rendered, CommandError> {
    let mut ev = evaluator(doc, "rendering a frame");
    let list = render_list::build(doc, req, &mut ev)?;
    let pixels_work = u64::from(req.width) * u64::from(req.height) / 1024;
    let (items, warnings, used) = render_list::items(doc, blobs, cache, &list, pixels_work)?;
    if used > FRAME_WORK_BUDGET {
        return Err(err(
            "limit.exceeded",
            "the frame is too large to render within the work limit",
            json!({ "limit": FRAME_WORK_BUDGET }),
        ));
    }
    let scene = Scene { width: req.width, height: req.height, background, items };
    let pixels = rasterise(&scene, cancelled)?;
    Ok(Rendered { pixels, list, warnings })
}

/// Output pixels → camera space (fields from the camera centre on the table plane, y up).
pub(crate) fn image_to_camera(list: &RenderList) -> [f64; 6] {
    let depth = list.view.position[2];
    let half = tt_anim::num::tan_deg(list.view.fov / 2.0);
    let s = if depth > 0.0 && list.width > 0 { 2.0 * depth * half / f64::from(list.width) } else { 0.0 };
    [s, 0.0, 0.0, -s, -f64::from(list.width) / 2.0 * s, f64::from(list.height) / 2.0 * s]
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The session's inline limits: the long side and the encoded bytes.
pub(crate) fn inline_limits(cx: &Cx) -> (u32, u64) {
    let l = &cx.session().agents.limits;
    (l.max_inline_image_px, INLINE_IMAGE_BYTES.min(l.max_inline_bytes))
}

/// `path` with ` (n)` before the extension, the first that does not exist.
pub(crate) fn renamed(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    (2u32..)
        .map(|n| parent.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

pub(crate) fn file_exists(paths: &[PathBuf]) -> CommandError {
    err(
        "export.fileExists",
        format!("{} output file(s) exist", paths.len()),
        json!({ "paths": paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>() }),
    )
    .hint("pass onConflict: \"overwrite\" or \"rename\"")
}

fn files_write(cx: &Cx, command: &str) -> Result<(), CommandError> {
    if cx.origin().has_scope(tt_registry::Scope::FilesWrite) {
        return Ok(());
    }
    Err(CommandError::with_details(
        tt_registry::codes::PERMISSION_DENIED,
        "writing the render to a file needs the scope files.write",
        json!({ "commandId": command, "missingScopes": ["files.write"] }),
    ))
}

fn unavailable(c: Composition) -> CommandError {
    let (name, owner) = match c {
        Composition::Xsheet => ("xsheet", "M5 C5.1"),
        _ => ("activeView", "M4 C4.2"),
    };
    err(
        "render.compositionUnavailable",
        format!("the `{name}` composition arrives with {owner}"),
        json!({ "composition": name, "owner": owner }),
    )
}

/// The elements of an `element` composition.
fn element_set(cx: &Cx, given: Option<&[String]>) -> Result<BTreeSet<Id>, CommandError> {
    let list = given.filter(|l| !l.is_empty()).ok_or_else(|| {
        invalid("/elementIds", "the element composition needs elementIds (or drawingId for one drawing)")
    })?;
    list.iter()
        .enumerate()
        .map(|(i, r)| {
            cx.resolve_as(r, ObjectType::Element)
                .map_err(|_| invalid(&format!("/elementIds/{i}"), format!("no element `{r}`")))
                .and_then(|k| Ok(Id::parse(&k)?))
        })
        .collect()
}

/// The render-time visibility overrides of an `include`/`exclude` pair (11 §7.5; C33-R1-05).
///
/// # Errors
/// `arg.invalid` for an element that does not resolve.
pub(crate) fn overrides(
    cx: &Cx,
    include: Option<&[String]>,
    exclude: Option<&[String]>,
) -> Result<Overrides, CommandError> {
    let set = |list: Option<&[String]>, pointer: &str| -> Result<BTreeSet<Id>, CommandError> {
        list.unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                cx.resolve_as(r, ObjectType::Element)
                    .map_err(|_| invalid(&format!("{pointer}/{i}"), format!("no element `{r}`")))
                    .and_then(|k| Ok(Id::parse(&k)?))
            })
            .collect()
    };
    Ok(Overrides { include: set(include, "/include")?, exclude: set(exclude, "/exclude")? })
}

/// The inline long-side limit, checked from the requested size before any work (09 §11.1 item 3; C33-R1-04).
///
/// # Errors
/// `render.inlineTooLarge`.
pub(crate) fn inline_side(cx: &Cx, width: u32, height: u32) -> Result<(), CommandError> {
    let (side, max_bytes) = inline_limits(cx);
    if width.max(height) <= side {
        return Ok(());
    }
    Err(err(
        "render.inlineTooLarge",
        format!("a {width} x {height} image is over the inline limit of {side} pixels on the long side"),
        json!({ "maxInlinePixels": side, "maxInlineBytes": max_bytes, "width": width, "height": height }),
    )
    .hint("use output: \"file\" with a path, or a smaller size"))
}

#[command(
    id = "render.frame.run",
    kind = query,
    label = "Render frame",
    scopes = [read],
    needs_project,
    example(title = "The current frame through the camera, inline, a quarter size", params = r#"{"scale": 0.25}"#),
    example(title = "Frame 12 at 480 pixels wide on a transparent background", params = r#"{"frame": 12, "width": 480, "background": "transparent"}"#),
    gui = "File > Export render actions (09); Export Snapshot (M10)",
)]
pub(crate) fn render_frame_run(cx: &mut Cx, p: FrameParams) -> Result<RenderResult, CommandError> {
    let started = std::time::Instant::now();
    if matches!(p.composition, Composition::Xsheet | Composition::ActiveView) {
        return Err(unavailable(p.composition));
    }
    let scene = scene_param(cx, p.scene_id.as_deref())?;
    let frame = frame_param(cx, p.frame)?;
    let doc = cx.document()?.clone();
    if p.output == Output::File && p.path.is_none() {
        return Err(invalid("/path", "file output needs an absolute path"));
    }
    if p.composition == Composition::Element && p.drawing_id.is_some() {
        return drawing_space(cx, &p, &doc, frame);
    }
    let only =
        (p.composition == Composition::Element).then(|| element_set(cx, p.element_ids.as_deref())).transpose()?;
    let camera = match &p.camera_id {
        Some(r) => Some(super::anim_common::node_id(cx, r)?),
        None => None,
    };
    let (width, height) = output_size(&doc, p.width, p.height, p.scale)?;
    // Known size limits before any evaluation, rasterisation or encoding (C33-R1-04).
    if p.output == Output::Inline {
        inline_side(cx, width, height)?;
    }
    let bg = background(&doc, &scene, &p.background)?;
    let overrides = overrides(cx, p.include.as_deref(), p.exclude.as_deref())?;
    let req = Request {
        scene: &scene,
        frame,
        camera: camera.as_ref(),
        width,
        height,
        only: only.as_ref(),
        overrides: &overrides,
    };
    let blobs = cx.project().ok().map(|pr| &pr.store.blobs);
    let rendered = render_frame(&doc, blobs, &cx.planar_cache(), &req, bg, &|| false)?;
    let mut warnings = rendered.warnings;
    if p.camera_frame {
        warnings.push(json!({ "code": "render.cameraFrameNotApplicable", "message": "the composition is the camera frame itself" }));
    }
    if p.quality == Quality::Draft {
        warnings.push(json!({ "code": "render.draftAsFinal", "message": "draft renders as final until the interactive renderer (M4 C4.2)" }));
    }
    let (bytes, mime) = encode(p.format, &rendered.pixels, width, height, p.jpeg_quality)?;
    let image = Image {
        bytes,
        mime,
        width,
        height,
        rev: doc.document_rev(),
        frame,
        image_to_camera: image_to_camera(&rendered.list),
        warnings,
        started,
    };
    deliver(cx, &p, image)
}

/// An encoded render and its result fields.
struct Image {
    bytes: Vec<u8>,
    mime: &'static str,
    width: u32,
    height: u32,
    rev: u64,
    frame: u32,
    image_to_camera: [f64; 6],
    warnings: Vec<Value>,
    started: std::time::Instant,
}

/// The caller's cancellation (its watchdog budget passed, or its request was cancelled) as a typed error.
///
/// # Errors
/// `command.timeout` when cancellation was requested.
pub(crate) fn observe_cancel() -> Result<(), CommandError> {
    if tt_vector::work::cancel_requested() {
        return Err(CommandError::new(
            tt_registry::codes::COMMAND_TIMEOUT,
            "the render was cancelled: the call overran its watchdog budget or was cancelled",
        ));
    }
    Ok(())
}

/// Encodes a rendered frame. The work is bounded by the size limits checked before rendering; the caller's
/// cancellation is observed before and after it (C33-R1-04).
fn encode(f: FrameFormat, pixels: &[u8], w: u32, h: u32, q: u8) -> Result<(Vec<u8>, &'static str), CommandError> {
    observe_cancel()?;
    let encoded = match f {
        FrameFormat::Png => tt_render::encode_png(pixels, w, h)
            .map(|b| (b, "image/png"))
            .map_err(|e| err("render.failed", e.to_string(), json!({}))),
        FrameFormat::Jpeg => tt_io::image::encode_still(tt_io::image::StillFormat::Jpeg, pixels, w, h, q)
            .map(|b| (b, "image/jpeg"))
            .map_err(|e| err("render.failed", e, json!({}))),
    }?;
    observe_cancel()?;
    Ok(encoded)
}

fn deliver(cx: &mut Cx, p: &FrameParams, image: Image) -> Result<RenderResult, CommandError> {
    let Image { bytes, mime, width, height, rev, frame, image_to_camera, warnings, started } = image;
    let sha256 = sha256_hex(&bytes);
    let size = bytes.len() as u64;
    let (data, path) = match (p.output, &p.path) {
        (Output::Inline, _) => {
            let (side, max_bytes) = inline_limits(cx);
            if width.max(height) > side || size > max_bytes {
                return Err(err(
                    "render.inlineTooLarge",
                    format!("a {width} x {height} image of {size} bytes is over the inline limits"),
                    json!({ "maxInlinePixels": side, "maxInlineBytes": max_bytes, "width": width, "height": height, "bytes": size }),
                )
                .hint("use output: \"file\" with a path, or a smaller size"));
            }
            (Some(tt_persist::b64::encode(&bytes)), None)
        }
        (Output::File, Some(given)) => {
            let target = crate::persistence::path_param(cx, given, "/path")?;
            files_write(cx, "render.frame.run")?;
            let target = match (target.exists(), p.on_conflict) {
                (true, OnConflict::Fail) => return Err(file_exists(&[target])),
                (true, OnConflict::Overwrite) => {
                    crate::persistence::confirm_overwrite(cx, &target)?;
                    target
                }
                (true, OnConflict::Rename) => renamed(&target),
                (false, _) => target,
            };
            if target.parent().is_some_and(|d| !d.is_dir()) {
                return Err(invalid("/path", "the parent directory does not exist"));
            }
            cx.defer(crate::cx::Effect::WriteFile(Box::new(crate::cx::FileWrite::new(target.clone(), bytes))));
            (None, Some(target.to_string_lossy().into_owned()))
        }
        (Output::File, None) => return Err(invalid("/path", "file output needs an absolute path")),
    };
    Ok(RenderResult {
        mime_type: mime.to_owned(),
        width,
        height,
        bytes: size,
        sha256,
        data,
        path,
        document_revision: rev,
        frame,
        render_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        image_to_camera,
        warnings,
    })
}

/// `composition: "element"` with `drawingId`: the drawing in drawing space (09 §11.1 item 11: origin at the top-left of
/// the drawing's bounds), at the camera's table-plane scale (1200 du across the camera width) times `scale`, or fitted to
/// `width`; colours through the scene's active palette styles, as in the camera composition.
fn drawing_space(cx: &mut Cx, p: &FrameParams, doc: &Document, frame: u32) -> Result<RenderResult, CommandError> {
    let r = p.drawing_id.as_deref().unwrap_or_default();
    let id = Id::parse(
        &cx.resolve_as(r, ObjectType::Drawing).map_err(|_| invalid("/drawingId", format!("no drawing `{r}`")))?,
    )?;
    let d = doc.drawing(&id).ok_or_else(|| invalid("/drawingId", format!("no drawing `{r}`")))?;
    let b = tt_vector::drawing::drawing_bounds(d, false).ok_or_else(|| {
        err("render.emptyDrawing", "the drawing has no visible geometry", json!({ "drawingId": id.as_str() }))
    })?;
    if !(p.scale.is_finite() && (0.05..=8.0).contains(&p.scale)) {
        return Err(invalid("/scale", "the scale must be 0.05-8"));
    }
    let (bw, bh) = ((b.max_x - b.min_x).max(1e-9), (b.max_y - b.min_y).max(1e-9));
    let s = match p.width {
        Some(w) => f64::from(w) / bw,
        None => f64::from(*doc.project().camera_width()) / 1200.0 * p.scale,
    };
    let w = p.width.map_or_else(|| (bw * s).round(), f64::from);
    let h = p.height.map_or_else(|| (bh * s).round(), f64::from);
    if !(1.0..=f64::from(MAX_SIDE)).contains(&w) || !(1.0..=f64::from(MAX_SIDE)).contains(&h) {
        return Err(err(
            "limit.exceeded",
            format!("a render of {w} x {h} pixels is outside 1-{MAX_SIDE} on a side"),
            json!({ "width": w, "height": h, "maxWidth": MAX_SIDE, "maxHeight": MAX_SIDE }),
        ));
    }
    let (w, h) = (w as u32, h as u32);
    let d2p = [s, 0.0, 0.0, -s, -b.min_x * s, b.max_y * s];
    let scene = scene_param(cx, p.scene_id.as_deref())?;
    let bg = background(doc, &scene, &p.background)?;
    let styles: Option<std::collections::BTreeMap<Id, Id>> =
        doc.scene(&scene).map(|sc| sc.palette_styles().iter().map(|(a, b)| (a.clone(), b.clone())).collect());
    let blobs = cx.project().ok().map(|pr| &pr.store.blobs);
    let cache = cx.planar_cache();
    let zones: std::collections::BTreeMap<String, crate::planar::LayerZones> =
        crate::planar::drawing_zones(&cache, d)?.into_iter().map(|lz| (lz.layer.id.clone(), lz)).collect();
    let mut builder =
        super::draw_render::Builder::in_drawing_units(doc, blobs, styles, u64::from(w) * u64::from(h) / 1024);
    builder.nodes(d.nodes(), &zones)?;
    let items = vec![tt_render::Item::Layer { transform: d2p, items: builder.take_items() }];
    let warnings = std::mem::take(&mut builder.warnings);
    let pixels = rasterise(&Scene { width: w, height: h, background: bg, items }, &|| false)?;
    let (bytes, mime) = encode(p.format, &pixels, w, h, p.jpeg_quality)?;
    let image = Image {
        bytes,
        mime,
        width: w,
        height: h,
        rev: doc.document_rev(),
        frame,
        image_to_camera: invert(d2p),
        warnings,
        started: std::time::Instant::now(),
    };
    deliver(cx, p, image)
}

/// The inverse of an affine map (identity when singular).
pub(crate) fn invert(m: [f64; 6]) -> [f64; 6] {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if det == 0.0 || !det.is_finite() {
        return [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    [ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)]
}

/// `render.limits.get` result (09 §11.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Limits {
    /// Longest side of an inline image.
    max_inline_pixels: u32,
    /// Largest encoded inline image.
    max_inline_bytes: u64,
    /// Largest width.
    max_width: u32,
    /// Largest height.
    max_height: u32,
    /// Supported formats.
    formats: Vec<String>,
}

#[command(
    id = "render.limits.get",
    kind = query,
    label = "Render limits",
    scopes = [read],
    example(title = "The inline and output limits", params = "{}"),
    agent_only = "agents size their renders (09 §11.1 item 3)",
)]
pub(crate) fn render_limits_get(cx: &mut Cx, _p: NoParams) -> Result<Limits, CommandError> {
    let (side, bytes) = inline_limits(cx);
    Ok(Limits {
        max_inline_pixels: side,
        max_inline_bytes: bytes,
        max_width: MAX_SIDE,
        max_height: MAX_SIDE,
        formats: ["png", "jpeg", "tga", "bmp"].iter().map(|s| (*s).to_owned()).collect(),
    })
}

fn one_u32() -> u32 {
    1
}

fn default_pattern() -> String {
    "{name}_{frame:4}".to_owned()
}

/// `render.range.run` parameters (09 §11.2).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeParams {
    /// Scene (default current).
    #[serde(default)]
    scene_id: Option<String>,
    /// First frame (default 1).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    from: Option<u32>,
    /// Last frame (default the scene length).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    to: Option<u32>,
    /// Every `step` frames.
    #[serde(default = "one_u32")]
    #[schemars(range(min = 1, max = 100_000))]
    step: u32,
    /// Composition (`camera` or `element`; `xsheet`: owner M5 C5.1).
    #[serde(default)]
    composition: Composition,
    /// Elements of the `element` composition.
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Width in pixels (default the camera's).
    #[serde(default)]
    #[schemars(range(min = 1, max = 8192))]
    width: Option<u32>,
    /// Height in pixels (default by the camera aspect).
    #[serde(default)]
    #[schemars(range(min = 1, max = 8192))]
    height: Option<u32>,
    /// Scale of the size.
    #[serde(default = "one")]
    #[schemars(range(min = 0.05, max = 8.0))]
    scale: f64,
    /// Quality.
    #[serde(default)]
    quality: Quality,
    /// `scene`, `transparent` or a colour.
    #[serde(default = "scene_default")]
    background: String,
    /// Format.
    #[serde(default)]
    format: SequenceFormat,
    /// Output directory (absolute).
    directory: String,
    /// File name pattern: `{name}` (the scene name), `{frame}` or `{frame:N}` (zero-padded to N digits).
    #[serde(default = "default_pattern")]
    #[schemars(length(min = 1, max = 200))]
    name_pattern: String,
    /// Existing files.
    #[serde(default)]
    on_conflict: OnConflict,
    /// Create the directory when missing.
    #[serde(default)]
    create_dirs: bool,
    /// Wait for the job and answer its manifest (09 §11.1 item 4): the caller's thread waits, bounded by the call's
    /// watchdog budget; at the timeout the answer is `job.waitTimeout` with the `jobId` (SI-107).
    #[serde(default)]
    wait: bool,
    /// Elements to show although hidden or annotations (11 §7.5; render-time only).
    #[serde(default)]
    include: Option<Vec<String>>,
    /// Elements to leave out, with their descendants (11 §7.5; render-time only).
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

/// One file of a manifest (09 §11.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestFile {
    /// `frame`.
    role: String,
    /// Absolute path.
    path: String,
    /// Size.
    bytes: u64,
    /// SHA-256 (hex).
    sha256: String,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// Scene frame.
    frame: u32,
}

/// `Manifest` of a range (09 §11.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Manifest {
    /// `succeeded`.
    status: String,
    /// `png`, `jpeg`, `tga` or `bmp`.
    format: String,
    /// The output directory.
    output_path: String,
    /// Frames written.
    frame_count: u32,
    /// Project frame rate.
    fps: f64,
    /// Frames divided by fps.
    duration_sec: f64,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// `{from, to, step}`.
    range: Value,
    /// The document revision rendered.
    document_revision: u64,
    /// One entry per frame file.
    files: Vec<ManifestFile>,
    /// The companion manifest file written with the frames (this manifest without itself).
    manifest_path: String,
    /// Objects not drawn and why.
    warnings: Vec<Value>,
}

/// The file name of a frame.
fn frame_name(pattern: &str, name: &str, frame: u32) -> Result<String, CommandError> {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let j = rest[i..].find('}').ok_or_else(|| invalid("/namePattern", "an unclosed brace in the pattern"))? + i;
        let token = &rest[i + 1..j];
        match token {
            "name" | "scene" => out.push_str(name),
            "frame" => out.push_str(&frame.to_string()),
            t if t.starts_with("frame:") => {
                let n: usize = t[6..]
                    .parse()
                    .ok()
                    .filter(|n| (1..=9).contains(n))
                    .ok_or_else(|| invalid("/namePattern", "`{frame:N}` needs N from 1 to 9"))?;
                out.push_str(&format!("{frame:0n$}"));
            }
            other => return Err(invalid("/namePattern", format!("unknown token `{other}`"))),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    if out.is_empty() || out.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(invalid("/namePattern", "the pattern must give a plain file name"));
    }
    Ok(out)
}

/// A scene name usable in file names.
fn file_safe(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') { c } else { '_' }).collect()
}

/// A staged file: written beside its target, published when every frame is written.
struct Staged {
    tmp: PathBuf,
    target: PathBuf,
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> CommandError {
    err("fs.io", format!("cannot {what} `{}`: {e}", path.display()), json!({ "path": path.to_string_lossy() }))
}

/// The file name of a path (empty when it has none).
fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Everything a range job needs, owned (the job runs off the document thread on a snapshot).
struct RangeJob {
    doc: Document,
    blobs: tt_persist::container::Blobs,
    cache: crate::planar::SharedCache,
    scene: Id,
    only: Option<BTreeSet<Id>>,
    overrides: Overrides,
    frames: Vec<u32>,
    targets: Vec<PathBuf>,
    manifest_target: PathBuf,
    dir: PathBuf,
    width: u32,
    height: u32,
    background: [u8; 4],
    format: SequenceFormat,
    format_name: &'static str,
    quality: Quality,
    range: Value,
    /// The staging token of this job: its staged files are `.<target>.tt-partial-<token>` (ADR-0020 decision 11).
    token: String,
}

impl RangeJob {
    /// The job's staging sidecar, listing every target it may write (the frames and the manifest).
    fn sidecar(&self, job_id: &str) -> Sidecar {
        let targets = self.targets.iter().chain(std::iter::once(&self.manifest_target)).map(|t| file_name(t)).collect();
        Sidecar::new(&self.token, job_id, targets)
    }

    fn run(
        &self,
        progress: &crate::jobs::JobProgress,
        sidecar: &mut Sidecar,
        staged: &mut Vec<Staged>,
    ) -> Result<Manifest, CommandError> {
        if !self.dir.is_dir() {
            std::fs::create_dir_all(&self.dir).map_err(|e| io_error("create", &self.dir, &e))?;
        }
        let token = &self.token;
        let mut files = Vec::with_capacity(self.frames.len());
        let mut warnings: Vec<Value> = Vec::new();
        // Staging of range jobs that did not finish (a killed process), proven by their sidecars and only theirs, is
        // recovered first and listed (C33-R1-08, C33-R2-01).
        if let Some(w) = sweep(&self.dir).warning("render.staleStagingRemoved") {
            progress.warn(w.clone());
            warnings.push(w);
        }
        if self.quality == Quality::Draft {
            warnings.push(json!({ "code": "render.draftAsFinal" }));
        }
        // The sidecar exists before the first staged file and is removed last.
        sidecar.write(&self.dir)?;
        let cancelled = || progress.cancelled();
        let total = self.frames.len() as f64;
        for (i, (f, target)) in self.frames.iter().zip(&self.targets).enumerate() {
            if progress.cancelled() {
                return Err(range_cancelled());
            }
            progress.report(i as f64 / total, &format!("Rendering frame {f}"));
            let req = Request {
                scene: &self.scene,
                frame: *f,
                camera: None,
                width: self.width,
                height: self.height,
                only: self.only.as_ref(),
                overrides: &self.overrides,
            };
            // A frame stopped by this job's cancellation is the job's cancellation, not a watchdog timeout (C33-R3-01).
            let r = render_frame(&self.doc, Some(&self.blobs), &self.cache, &req, self.background, &cancelled)
                .map_err(|e| if progress.cancelled() { range_cancelled() } else { e })?;
            for w in r.warnings {
                if !warnings.contains(&w) {
                    warnings.push(w);
                }
            }
            let bytes = encode_sequence(self.format, &r.pixels, self.width, self.height)?;
            let tmp = target.with_file_name(staged_name(&file_name(target), token));
            std::fs::write(&tmp, &bytes).map_err(|e| io_error("write", &tmp, &e))?;
            staged.push(Staged { tmp, target: target.clone() });
            files.push(ManifestFile {
                role: "frame".to_owned(),
                path: target.to_string_lossy().into_owned(),
                bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
                width: self.width,
                height: self.height,
                frame: *f,
            });
        }
        let fps = *self.doc.project().frame_rate();
        let manifest = Manifest {
            status: "succeeded".to_owned(),
            format: self.format_name.to_owned(),
            output_path: self.dir.to_string_lossy().into_owned(),
            frame_count: u32::try_from(files.len()).unwrap_or(u32::MAX),
            fps,
            duration_sec: if fps > 0.0 { files.len() as f64 / fps } else { 0.0 },
            width: self.width,
            height: self.height,
            range: self.range.clone(),
            document_revision: self.doc.document_rev(),
            files,
            manifest_path: self.manifest_target.to_string_lossy().into_owned(),
            warnings,
        };
        let text =
            serde_json::to_string_pretty(&manifest).map_err(|e| err("render.failed", e.to_string(), json!({})))?;
        let tmp = self.manifest_target.with_file_name(staged_name(&file_name(&self.manifest_target), token));
        std::fs::write(&tmp, text.as_bytes()).map_err(|e| io_error("write", &tmp, &e))?;
        staged.push(Staged { tmp, target: self.manifest_target.clone() });
        // The commit point: past it the job publishes every file and can no longer be cancelled.
        if !progress.commit() {
            return Err(range_cancelled());
        }
        for w in publish(&self.dir, staged, sidecar)? {
            progress.warn(w);
        }
        progress.report(1.0, "Done");
        Ok(manifest)
    }
}

/// The typed error of a cancelled range job.
fn range_cancelled() -> CommandError {
    CommandError::new("job.cancelled", "the range was cancelled; nothing was published")
}

/// The staging marker: a staged file is `.<target name>.tt-partial-<token>`.
const PARTIAL: &str = ".tt-partial-";
/// The backup marker: an original moved aside during publication is `.<target name>.tt-backup-<token>`.
const BACKUP: &str = ".tt-backup-";
/// A job's staging sidecar is `.tt-partial-<token>.job`.
const SIDECAR_EXT: &str = ".job";
/// The `kind` of a staging sidecar.
const SIDECAR_KIND: &str = "toontown.render.range.staging";
/// Sidecars larger than this are not read (a job's sidecar lists at most 100 001 short names).
const SIDECAR_MAX_BYTES: u64 = 64 << 20;

fn staged_name(target: &str, token: &str) -> String {
    format!(".{target}{PARTIAL}{token}")
}

fn backup_name(target: &str, token: &str) -> String {
    format!(".{target}{BACKUP}{token}")
}

fn sidecar_name(token: &str) -> String {
    format!("{PARTIAL}{token}{SIDECAR_EXT}")
}

/// Whether `t` has the exact format of the staging token generator: 12 lowercase hexadecimal characters.
fn valid_token(t: &str) -> bool {
    t.len() == 12 && t.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A new staging token in the exact format (six random bytes as lowercase hex).
fn new_token() -> String {
    let t = tt_persist::fsx::random_token(6);
    if valid_token(&t) {
        return t;
    }
    // The generator's time-and-counter fallback (no system randomness), brought to the same format.
    sha256_hex(format!("{t}-{}", std::process::id()).as_bytes()).chars().take(12).collect()
}

/// Whether a name listed in a sidecar is a plain file name in the job's directory, never a path.
fn plain_name(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(['/', '\\', ':']) && !n.chars().any(char::is_control)
}

/// A range job's staging sidecar: the ownership proof recovery requires (ADR-0020 decision 11, C33-R2-01). A
/// recovery acts only on the files a valid sidecar lists, derived from its targets and its own token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sidecar {
    kind: String,
    version: u32,
    token: String,
    job_id: String,
    /// The product revision that wrote it (the CI commit; `unknown` in local builds).
    head: String,
    product_version: String,
    host: String,
    pid: u32,
    /// Set once every staged file took its target's place: a recovery then keeps the targets and drops the backups.
    published: bool,
    /// Target file names (frames, then the manifest).
    targets: Vec<String>,
    /// `.<target>.tt-partial-<token>` for each target.
    staged: Vec<String>,
    /// `.<target>.tt-backup-<token>` for each target.
    backups: Vec<String>,
}

impl Sidecar {
    fn new(token: &str, job_id: &str, targets: Vec<String>) -> Self {
        Self {
            kind: SIDECAR_KIND.to_owned(),
            version: 1,
            token: token.to_owned(),
            job_id: job_id.to_owned(),
            head: option_env!("GITHUB_SHA").unwrap_or("unknown").to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            host: tt_persist::host::host(),
            pid: std::process::id(),
            published: false,
            staged: targets.iter().map(|t| staged_name(t, token)).collect(),
            backups: targets.iter().map(|t| backup_name(t, token)).collect(),
            targets,
        }
    }

    fn write(&self, dir: &Path) -> Result<(), CommandError> {
        let path = dir.join(sidecar_name(&self.token));
        let text = serde_json::to_vec_pretty(self).map_err(|e| err("render.failed", e.to_string(), json!({})))?;
        std::fs::write(&path, text).map_err(|e| io_error("write", &path, &e))
    }

    /// The sidecar `name` in `dir`, if it is exactly what a range job writes: the name `.tt-partial-<token>.job`
    /// with a well-formed token, a regular file, the kind and version, the same token inside, plain target names and
    /// staged/backup names derived from them. Anything else is not ours and is never acted on.
    fn read(dir: &Path, name: &str) -> Option<Self> {
        let token = name.strip_prefix(PARTIAL)?.strip_suffix(SIDECAR_EXT)?;
        if !valid_token(token) {
            return None;
        }
        let path = dir.join(name);
        let meta = std::fs::symlink_metadata(&path).ok()?;
        if !meta.is_file() || meta.len() > SIDECAR_MAX_BYTES {
            return None;
        }
        let s: Self = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
        let owned = s.kind == SIDECAR_KIND
            && s.version == 1
            && s.token == token
            && s.targets.iter().all(|t| plain_name(t))
            && s.staged.iter().eq(s.targets.iter().map(|t| staged_name(t, token)).collect::<Vec<_>>().iter())
            && s.backups.iter().eq(s.targets.iter().map(|t| backup_name(t, token)).collect::<Vec<_>>().iter());
        owned.then_some(s)
    }

    /// Whether the job that wrote it may still run: a job of this process while its token is live, a job of another
    /// process on this machine unless that process is known to be gone, and any job of another machine.
    fn alive(&self) -> bool {
        if !tt_persist::host::is_this_host(&self.host) {
            return true;
        }
        if self.pid == std::process::id() {
            return live_tokens().contains(&self.token);
        }
        tt_persist::host::process_alive(self.pid) != Some(false)
    }
}

/// Staging tokens of the range jobs running in this process: a sweep never recovers a live job's files.
static LIVE_TOKENS: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());

fn live_tokens() -> std::sync::MutexGuard<'static, BTreeSet<String>> {
    LIVE_TOKENS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Marks a job's staging token live until dropped.
struct LiveToken(String);

impl LiveToken {
    fn new() -> Self {
        let mut live = live_tokens();
        let mut token = new_token();
        while live.contains(&token) {
            token = new_token();
        }
        live.insert(token.clone());
        Self(token)
    }
}

impl Drop for LiveToken {
    fn drop(&mut self) {
        live_tokens().remove(&self.0);
    }
}

/// What a recovery did: files removed, targets whose original came back, and files left because the file system
/// refused (the sidecar is then kept, so a later job retries).
#[derive(Debug, Default)]
struct Recovery {
    removed: Vec<String>,
    restored: Vec<String>,
    failed: Vec<String>,
}

impl Recovery {
    fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.restored.is_empty() && self.failed.is_empty()
    }

    fn merge(&mut self, other: Recovery) {
        self.removed.extend(other.removed);
        self.restored.extend(other.restored);
        self.failed.extend(other.failed);
    }

    /// The receipt as a warning: `files` (names removed or moved back onto their targets), `restored` (targets whose
    /// original came back) and `unrestored` (names left in place).
    fn warning(mut self, code: &str) -> Option<Value> {
        if self.is_empty() {
            return None;
        }
        self.removed.sort();
        self.restored.sort();
        self.failed.sort();
        let mut w = json!({ "code": code, "files": self.removed });
        if !self.restored.is_empty() {
            w["restored"] = json!(self.restored);
        }
        if !self.failed.is_empty() {
            w["unrestored"] = json!(self.failed);
        }
        Some(w)
    }
}

fn is_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
}

/// Puts an original back from its backup, replacing the file an interrupted publication left at the target.
fn restore(backup: &Path, target: &Path) -> bool {
    tt_persist::lock::boundary(tt_persist::lock::Boundary::RangeRestore, target);
    match std::fs::symlink_metadata(target) {
        Ok(m) if m.is_file() => {
            if std::fs::remove_file(target).is_err() {
                return false;
            }
        }
        Ok(_) => return false,
        Err(_) => {}
    }
    std::fs::rename(backup, target).is_ok()
}

/// Recovers the staging a sidecar lists, and nothing else (ADR-0020 decision 11): staged files are removed; a backup
/// is deleted when the publication had completed (`published`), otherwise its original goes back to its name in
/// place of what the interrupted publication put there. The sidecar is removed last, only when nothing failed.
fn recover(dir: &Path, s: &Sidecar) -> Recovery {
    let mut r = Recovery::default();
    for ((target, staged), backup) in s.targets.iter().zip(&s.staged).zip(&s.backups) {
        let staged_path = dir.join(staged);
        if is_file(&staged_path) {
            match std::fs::remove_file(&staged_path) {
                Ok(()) => r.removed.push(staged.clone()),
                Err(_) => r.failed.push(staged.clone()),
            }
        }
        let backup_path = dir.join(backup);
        if !is_file(&backup_path) {
            continue;
        }
        if s.published {
            match std::fs::remove_file(&backup_path) {
                Ok(()) => r.removed.push(backup.clone()),
                Err(_) => r.failed.push(backup.clone()),
            }
        } else if restore(&backup_path, &dir.join(target)) {
            r.removed.push(backup.clone());
            r.restored.push(target.clone());
        } else {
            r.failed.push(backup.clone());
        }
    }
    if r.failed.is_empty() {
        let name = sidecar_name(&s.token);
        match std::fs::remove_file(dir.join(&name)) {
            Ok(()) => r.removed.push(name),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => r.failed.push(name),
        }
    }
    r
}

/// Recovers the staging of the range jobs in `dir` whose sidecars are valid and whose jobs are not alive (a process
/// killed mid-job; C33-R1-08). Files no valid sidecar lists, malformed tokens and marker-like names are never
/// touched and never reported (C33-R2-01).
fn sweep(dir: &Path) -> Recovery {
    let Ok(rd) = std::fs::read_dir(dir) else { return Recovery::default() };
    let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut r = Recovery::default();
    for name in names {
        if let Some(s) = Sidecar::read(dir, &name)
            && !s.alive()
        {
            r.merge(recover(dir, &s));
        }
    }
    r
}

fn target_invalid(path: &Path, why: &str) -> CommandError {
    err(
        "export.targetInvalid",
        format!("`{}` {why}; nothing was published", path.display()),
        json!({ "path": path.to_string_lossy() }),
    )
}

/// Undoes a failed publication: published targets without an original are removed, originals are restored from
/// their backups. Returns `cause` when everything was undone, else `export.rollbackIncomplete` naming the targets
/// that could not be restored (the sidecar stays, so the next range job into the directory retries).
fn rolled_back(cause: CommandError, published: &[PathBuf], backups: &[(PathBuf, PathBuf)]) -> CommandError {
    let mut unrestored: Vec<String> = Vec::new();
    for t in published.iter().filter(|t| !backups.iter().any(|(target, _)| target == *t)) {
        match std::fs::remove_file(t) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => unrestored.push(t.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    for (target, backup) in backups {
        if !restore(backup, target) {
            unrestored.push(target.to_string_lossy().into_owned());
        }
    }
    if unrestored.is_empty() {
        return cause;
    }
    err(
        "export.rollbackIncomplete",
        format!(
            "publishing failed ({}) and {} target(s) could not be restored; the job's staging sidecar is kept so the \
             next range job into this directory retries",
            cause.message,
            unrestored.len()
        ),
        json!({ "cause": { "code": cause.code, "message": cause.message }, "unrestored": unrestored }),
    )
}

/// Publishes staged files as one transaction (C33-R1-03): every target is checked first (an existing target must be a
/// writable regular file); then each existing original is moved aside to its backup and the staged file renamed onto
/// the target. On any failure the publication is undone ([`rolled_back`]). On success the sidecar is marked
/// `published`, the backups are deleted and the sidecar removed; leftovers are returned as a warning, never dropped.
fn publish(dir: &Path, staged: &[Staged], sidecar: &mut Sidecar) -> Result<Vec<Value>, CommandError> {
    for s in staged {
        if let Ok(meta) = std::fs::symlink_metadata(&s.target) {
            if !meta.is_file() {
                return Err(target_invalid(&s.target, "exists and is not a file"));
            }
            if meta.permissions().readonly() {
                return Err(target_invalid(&s.target, "is read-only"));
            }
        }
    }
    let token = sidecar.token.clone();
    let mut backups: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut published: Vec<PathBuf> = Vec::new();
    for s in staged {
        if s.target.exists() {
            let backup = s.target.with_file_name(backup_name(&file_name(&s.target), &token));
            if let Err(e) = std::fs::rename(&s.target, &backup) {
                return Err(rolled_back(io_error("set aside", &s.target, &e), &published, &backups));
            }
            backups.push((s.target.clone(), backup));
        }
        tt_persist::lock::boundary(tt_persist::lock::Boundary::RangePublish, &s.target);
        if let Err(e) = std::fs::rename(&s.tmp, &s.target) {
            return Err(rolled_back(io_error("publish", &s.target, &e), &published, &backups));
        }
        published.push(s.target.clone());
    }
    let mut left: Vec<String> = Vec::new();
    sidecar.published = true;
    if sidecar.write(dir).is_err() {
        left.push(sidecar_name(&token));
    }
    for (_, backup) in &backups {
        if std::fs::remove_file(backup).is_err() {
            left.push(file_name(backup));
        }
    }
    if std::fs::remove_file(dir.join(sidecar_name(&token))).is_err() {
        left.push(sidecar_name(&token));
    }
    left.dedup();
    Ok(if left.is_empty() {
        Vec::new()
    } else {
        vec![
            json!({ "code": "render.stagingLeft", "message": "published; these staging files could not be removed", "files": left }),
        ]
    })
}

fn encode_sequence(format: SequenceFormat, pixels: &[u8], w: u32, h: u32) -> Result<Vec<u8>, CommandError> {
    let still = match format {
        SequenceFormat::Png => {
            return tt_render::encode_png(pixels, w, h).map_err(|e| err("render.failed", e.to_string(), json!({})));
        }
        SequenceFormat::Jpeg => tt_io::image::StillFormat::Jpeg,
        SequenceFormat::Tga => tt_io::image::StillFormat::Tga,
        SequenceFormat::Bmp => tt_io::image::StillFormat::Bmp,
    };
    tt_io::image::encode_still(still, pixels, w, h, 90).map_err(|e| err("render.failed", e, json!({})))
}

#[command(
    id = "render.range.run",
    kind = job,
    label = "Render range",
    scopes = [read, files_write],
    confirm = ["overwrite"],
    needs_project,
    example(title = "Frames 1-24 of the current scene as PNGs, a quarter size", params = r#"{"from": 1, "to": 24, "scale": 0.25, "directory": "/tmp/tt-range", "createDirs": true, "onConflict": "overwrite"}"#),
    gui = "File > Export > Image Sequence (M10)",
)]
pub(crate) fn render_range_run(cx: &mut Cx, p: RangeParams) -> Result<Job<Manifest>, CommandError> {
    let _ = p.wait;
    if p.composition == Composition::ActiveView || p.composition == Composition::Xsheet {
        return Err(unavailable(p.composition));
    }
    let scene = scene_param(cx, p.scene_id.as_deref())?;
    let doc = cx.document()?.clone();
    let length = doc.scene(&scene).map_or(1, |s| doc.scene_length(s)).clamp(1, 100_000);
    let from = p.from.unwrap_or(1);
    let to = p.to.unwrap_or(u32::try_from(length).unwrap_or(100_000));
    if to < from {
        return Err(invalid("/to", "`to` is before `from`"));
    }
    let step = p.step.max(1);
    let frames: Vec<u32> = (from..=to).step_by(step as usize).collect();
    if frames.len() as u64 > u64::from(MAX_RANGE_FRAMES) {
        return Err(err("limit.exceeded", "too many frames in one range", json!({ "limit": MAX_RANGE_FRAMES })));
    }
    let only =
        (p.composition == Composition::Element).then(|| element_set(cx, p.element_ids.as_deref())).transpose()?;
    let (width, height) = output_size(&doc, p.width, p.height, p.scale)?;
    let bg = background(&doc, &scene, &p.background)?;
    let overrides = overrides(cx, p.include.as_deref(), p.exclude.as_deref())?;
    let dir = crate::persistence::path_param(cx, &p.directory, "/directory")?;
    files_write(cx, "render.range.run")?;
    if !dir.is_dir() && !p.create_dirs {
        return Err(invalid("/directory", "the directory does not exist (createDirs: true creates it)"));
    }
    let (ext, format_name) = match p.format {
        SequenceFormat::Png => ("png", "png"),
        SequenceFormat::Jpeg => ("jpg", "jpeg"),
        SequenceFormat::Tga => ("tga", "tga"),
        SequenceFormat::Bmp => ("bmp", "bmp"),
    };
    let name = file_safe(doc.scene(&scene).map_or("scene", |s| s.name().as_str()));
    let mut targets = Vec::with_capacity(frames.len());
    let mut seen = BTreeSet::new();
    for f in &frames {
        let file = format!("{}.{ext}", frame_name(&p.name_pattern, &name, *f)?);
        if !seen.insert(file.clone()) {
            return Err(invalid("/namePattern", "the pattern gives two frames the same file name; use {frame}"));
        }
        targets.push(dir.join(file));
    }
    let mut manifest_target = dir.join(format!("{name}_{from:04}-{to:04}.manifest.json"));
    let existing: Vec<PathBuf> =
        targets.iter().chain(std::iter::once(&manifest_target)).filter(|t| t.exists()).cloned().collect();
    if !existing.is_empty() {
        match p.on_conflict {
            OnConflict::Fail => return Err(file_exists(&existing)),
            OnConflict::Overwrite => {
                for t in &existing {
                    crate::persistence::confirm_overwrite(cx, t)?;
                }
            }
            OnConflict::Rename => {
                for t in &mut targets {
                    if t.exists() {
                        *t = renamed(t);
                    }
                }
                if manifest_target.exists() {
                    manifest_target = renamed(&manifest_target);
                }
            }
        }
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    let live = LiveToken::new();
    let job = RangeJob {
        blobs: cx.project()?.store.blobs.clone(),
        cache: cx.planar_cache(),
        doc,
        scene,
        only,
        overrides,
        token: live.0.clone(),
        frames,
        targets,
        manifest_target,
        dir,
        width,
        height,
        background: bg,
        format: p.format,
        format_name,
        quality: p.quality,
        range: json!({ "from": from, "to": to, "step": step }),
    };
    let origin = cx.origin().clone();
    let id = cx.session().jobs.submit("render.range.run", &origin.session_key(), move |progress| {
        // The token stays live, so no sweep recovers this job's files, until the job ends.
        let _live = live;
        let mut sidecar = job.sidecar(progress.job_id());
        let mut staged: Vec<Staged> = Vec::new();
        match job.run(progress, &mut sidecar, &mut staged) {
            Ok(m) => Ok(serde_json::to_value(m).unwrap_or(Value::Null)),
            Err(e) => {
                // A failed or cancelled job recovers its own staging and only its own (C33-R2-01); the receipt stays in
                // the job record with the state and, when failed, the error (C33-R2-02).
                if let Some(w) = recover(&job.dir, &sidecar).warning("render.stagingDiscarded") {
                    progress.warn(w);
                }
                Err(e)
            }
        }
    })?;
    Ok(Job::new(id))
}
