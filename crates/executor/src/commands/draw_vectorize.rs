//! Import and vectorize (02 §11.2–§11.3; M2 C2.4): `draw.vectorize.preview` and `draw.vectorize.run`.
//!
//! Files (or the bitmaps of an image element) are decoded within the import limits (`tt_io::image`) and traced by
//! `tt_vector::vectorize` into brush-type shapes in one layer, coloured with the black of a generated "Vectorization"
//! palette, or — with texture — with a texture colour holding the source bitmap. The preview renders the result; the
//! run is a cancellable job that commits each image as its own transaction and undo step ("Import and Vectorize")
//! through the executor's staging path (`Core::run_staged`), so cancelling keeps the images already completed.
//! Each image becomes one drawing in consecutive cells of the target drawing element from the start frame; occupied
//! cells are replaced only after confirmation (`onConflict`). Without a selected element, and always when converting
//! an image element (§11.3), a new drawing element is created, the latter with matching exposure.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::ElementType;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, command};
use tt_vector::path::Path;
use tt_vector::vectorize::{self, Filter, VECTORIZE_WORK_BUDGET};
use tt_vector::work::Work;

use super::common::{Job, invalid};
use super::draw::{err, op};
use crate::app::AppContext;
use crate::cx::Cx;
use crate::persistence;
use crate::planar::geom_error;

/// How long a job waits for the document thread to commit one image.
const POST_TIMEOUT: Duration = Duration::from_secs(600);

/// The generated palette of vectorization colours and its black (02 §11.2).
const PALETTE: &str = "Vectorization";
const BLACK: &str = "Vectorization Black";

/// Vectorization type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum VectorizeType {
    /// Ink becomes black brush shapes.
    #[default]
    BlackAndWhite,
    /// Ink shapes are filled with the source bitmap as a texture colour.
    WithTexture,
}

/// Pre-filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FilterIn {
    /// None.
    #[default]
    None,
    /// Light mean blur.
    Smooth,
    /// Stronger 3×3 mean blur.
    SmoothMore,
    /// High-pass edge enhancement.
    EdgeEnhance,
    /// Sharpening.
    Sharpen,
}

impl FilterIn {
    fn filter(self) -> Filter {
        match self {
            Self::None => Filter::None,
            Self::Smooth => Filter::Smooth,
            Self::SmoothMore => Filter::SmoothMore,
            Self::EdgeEnhance => Filter::EdgeEnhance,
            Self::Sharpen => Filter::Sharpen,
        }
    }
}

/// What to do with cells that already hold drawings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnConflict {
    /// Ask (`needs_confirmation` with `drawing.cellOccupied`), then replace.
    #[default]
    Ask,
    /// Replace them.
    Replace,
    /// Leave them and skip their images.
    Skip,
}

fn seventy() -> i64 {
    70
}

fn preview_width() -> i64 {
    512
}

/// `draw.vectorize.preview` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PreviewParams {
    /// An image file.
    #[serde(default)]
    file: Option<String>,
    /// An image element (its bitmap at `frame`).
    #[serde(default)]
    image_element_id: Option<String>,
    /// The frame of the image element (default: current frame).
    #[serde(default)]
    frame: Option<i64>,
    /// Vectorization type.
    #[serde(default, rename = "type")]
    kind: VectorizeType,
    /// Pre-filter.
    #[serde(default)]
    filter: FilterIn,
    /// Threshold, % of white (0–100; `vectorize.thresholdOutOfRange` outside).
    #[serde(default = "seventy")]
    threshold: i64,
    /// Preview width in pixels (1–4096).
    #[serde(default = "preview_width")]
    width: i64,
}

/// `{png, strokeCount}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviewResult {
    /// The rendered result, base64 PNG.
    png: String,
    /// Shapes the vectorization creates.
    stroke_count: u64,
}

/// `draw.vectorize.run` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RunParams {
    /// Image files, in order (one drawing each).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    files: Option<Vec<String>>,
    /// An image element to convert into a new drawing element with matching exposure (§11.3).
    #[serde(default)]
    image_element_id: Option<String>,
    /// The target drawing element (default: the selected element; a new element if none).
    #[serde(default)]
    target_element_id: Option<String>,
    /// The first cell (default: the current frame).
    #[serde(default)]
    start_frame: Option<i64>,
    /// Vectorization type.
    #[serde(default, rename = "type")]
    kind: VectorizeType,
    /// Pre-filter.
    #[serde(default)]
    filter: FilterIn,
    /// Threshold, % of white (0–100; `vectorize.thresholdOutOfRange` outside).
    #[serde(default = "seventy")]
    threshold: i64,
    /// Occupied cells.
    #[serde(default)]
    on_conflict: OnConflict,
}

/// The final result of `draw.vectorize.run`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunResult {
    /// The drawing element that received the drawings.
    element_id: String,
    /// The drawings created, in order.
    drawing_ids: Vec<String>,
    /// Images completed.
    completed: u64,
    /// The job was cancelled after `completed` images.
    cancelled: bool,
    /// Images not committed, in order: their cell was occupied (`onConflict: "skip"`) or the job was cancelled first
    /// (SI-083: reported beside the spec's fields).
    skipped: Vec<Skipped>,
}

/// An image a run did not commit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Skipped {
    /// Its position in `files` (or among the image element's drawings), from 0.
    index: u64,
    /// The file, or the image element's drawing.
    source: String,
    /// `occupied` or `cancelled`.
    reason: String,
}

fn threshold(t: i64) -> Result<u8, CommandError> {
    u8::try_from(t).ok().filter(|t| *t <= 100).ok_or_else(|| {
        err(
            "vectorize.thresholdOutOfRange",
            format!("the threshold is a percentage of white, 0-100 (got {t})"),
            json!({ "pointer": "/threshold", "value": t }),
        )
    })
}

/// An image element's drawing and its bitmap's stored PNG.
type Bitmap = (Id, Arc<Vec<u8>>);

/// Exposure runs: a drawing (or blank) per run, and its frame count.
type Runs = Vec<(Option<Id>, u32)>;

/// The image element's exposure, the drawings converted so far (source → new) and this image's source drawing.
type Mirror = (Runs, Vec<(Option<Id>, String)>, Option<Id>);

/// An image to vectorize.
#[derive(Debug, Clone)]
enum Source {
    /// A file (read by the job).
    File(PathBuf),
    /// A bitmap node's stored PNG, read from the project.
    Bitmap(Arc<Vec<u8>>),
}

impl Source {
    fn decode(&self) -> Result<tt_io::image::Rgba, CommandError> {
        let limits = tt_io::image::LIMITS;
        let (bytes, ext) = match self {
            Source::File(p) => {
                let meta = std::fs::metadata(p).map_err(|e| file_error(p, &e.to_string()))?;
                if meta.len() > limits.max_bytes {
                    return Err(err(
                        "limit.exceeded",
                        format!("`{}` is larger than {} bytes", p.display(), limits.max_bytes),
                        json!({ "path": p.to_string_lossy(), "limit": "bytes", "maximum": limits.max_bytes }),
                    ));
                }
                let b = std::fs::read(p).map_err(|e| file_error(p, &e.to_string()))?;
                (Arc::new(b), p.extension().and_then(|e| e.to_str()).map(str::to_owned))
            }
            Source::Bitmap(b) => (b.clone(), Some("png".to_owned())),
        };
        tt_io::image::decode(&bytes, ext.as_deref(), &limits).map_err(|e| {
            let path = match self {
                Source::File(p) => p.to_string_lossy().into_owned(),
                Source::Bitmap(_) => String::new(),
            };
            err(e.code(), e.message(), json!({ "path": path }))
        })
    }
}

fn file_error(p: &std::path::Path, why: &str) -> CommandError {
    err(
        "vectorize.unsupportedFile",
        format!("`{}` cannot be read: {why}", p.display()),
        json!({ "path": p.to_string_lossy() }),
    )
}

/// Traces a decoded image into closed outlines (drawing space).
fn trace(img: &tt_io::image::Rgba, filter: FilterIn, t: u8, work: &mut Work) -> Result<Vec<Path>, CommandError> {
    vectorize::vectorize(img.width, img.height, &img.pixels, filter.filter(), t, work).map_err(|e| geom_error(&e))
}

fn element_ref(cx: &mut Cx, r: &str, pointer: &str) -> Result<(Id, ElementType), CommandError> {
    let key = cx.resolve_as(r, ObjectType::Element).map_err(|_| invalid(pointer, format!("no element `{r}`")))?;
    let id = Id::parse(&key)?;
    let ty = cx.document()?.element(&id).map(|e| *e.element_type()).ok_or_else(|| invalid(pointer, "no element"))?;
    Ok((id, ty))
}

/// The bitmap nodes of the drawings of an image element: per bank drawing, its first bitmap's stored PNG.
fn bitmaps(cx: &Cx, doc: &Document, element: &Id) -> Result<Vec<Bitmap>, CommandError> {
    let e = doc.element(element).ok_or_else(|| invalid("/imageElementId", "no element"))?;
    let Some(bank) = e.drawing_bank_id().clone() else { return Ok(Vec::new()) };
    let bank = doc.bank(&bank).ok_or_else(|| invalid("/imageElementId", "the element has no drawing bank"))?;
    let store = &cx.project()?.store.blobs;
    let mut out = Vec::new();
    for d in bank.drawings().ids() {
        let Some(drawing) = doc.drawing(d) else { continue };
        let image = tt_vector::drawing::objects(drawing).into_iter().find_map(|o| {
            let n = tt_vector::drawing::node_at(drawing, &o.pointer)?;
            n.image().clone()
        });
        if let Some(name) = image {
            let bytes = store.read(&name.entry, tt_io::image::LIMITS.max_bytes).map_err(|e| {
                err(
                    "vectorize.unsupportedFile",
                    format!("bitmap `{}`: {e}", name.entry),
                    json!({ "entry": name.entry }),
                )
            })?;
            out.push((d.clone(), Arc::new(bytes)));
        }
    }
    Ok(out)
}

/// The exposure runs of an element (drawing per run, frames).
fn runs(doc: &Document, element: &Id) -> Runs {
    doc.element(element)
        .map_or_else(Vec::new, |e| e.exposure().iter().flatten().map(|(d, n)| (d.clone(), *n)).collect())
}

#[command(
    id = "draw.vectorize.preview",
    precheck = crate::prechecks::draw_vectorize_preview,
    kind = query,
    label = "Preview vectorization",
    scopes = [read, files_read],
    needs_project,
    example(title = "Preview the E2 line art", params = r#"{"file": "/tmp/lineart.png", "width": 256}"#),
    gui = "Import and Vectorize dialog: Preview (M4 C4.1)",
)]
pub(crate) fn draw_vectorize_preview(cx: &mut Cx, p: PreviewParams) -> Result<PreviewResult, CommandError> {
    let t = threshold(p.threshold)?;
    if !(1..=4096).contains(&p.width) {
        return Err(invalid("/width", "the preview width must be 1-4096"));
    }
    let source = match (&p.file, &p.image_element_id) {
        (Some(f), None) => Source::File(persistence::path_param(cx, f, "/file")?),
        (None, Some(r)) => {
            let (id, ty) = element_ref(cx, r, "/imageElementId")?;
            if ty != ElementType::Image {
                return Err(invalid("/imageElementId", "not an image element"));
            }
            let doc = cx.document()?.clone();
            let frame = u32::try_from(p.frame.unwrap_or(i64::from(cx.app().frame)).max(1)).unwrap_or(1);
            let shown = AppContext::exposed_drawing(&doc, &id, frame)
                .ok_or_else(|| invalid("/frame", "the image element shows no drawing at that frame"))?;
            let bytes = bitmaps(cx, &doc, &id)?
                .into_iter()
                .find(|(d, _)| *d == shown)
                .map(|(_, b)| b)
                .ok_or_else(|| invalid("/frame", "the drawing at that frame has no bitmap"))?;
            Source::Bitmap(bytes)
        }
        _ => return Err(invalid("", "give exactly one of `file` and `imageElementId`")),
    };
    let img = source.decode()?;
    let mut work = Work::new(VECTORIZE_WORK_BUDGET, "vectorizing this image");
    let paths = trace(&img, p.filter, t, &mut work)?;
    let width = u32::try_from(p.width).unwrap_or(512);
    let png = preview_png(&img, &paths, width, p.kind)?;
    Ok(PreviewResult { png: tt_persist::b64::encode(&png), stroke_count: paths.len() as u64 })
}

/// Renders traced outlines over white (black, or the source bitmap as their texture), the image's width at `width`
/// pixels.
fn preview_png(
    img: &tt_io::image::Rgba,
    paths: &[Path],
    width: u32,
    kind: VectorizeType,
) -> Result<Vec<u8>, CommandError> {
    use tt_render::{El, Item, Paint, Rule, Scene};
    let s = f64::from(width) / vectorize::FRAME_WIDTH;
    let height = ((f64::from(img.height) * f64::from(width) / f64::from(img.width)).round() as u32)
        .clamp(1, tt_render::MAX_SIDE);
    let half_h = f64::from(height) / s / 2.0;
    let px = |p: tt_vector::geom::P| ((p.x + vectorize::FRAME_WIDTH / 2.0) * s, (half_h - p.y) * s);
    let paint = match kind {
        VectorizeType::BlackAndWhite => Paint::Solid([0, 0, 0, 255]),
        VectorizeType::WithTexture => Paint::Image {
            rgba: Arc::new(img.pixels.clone()),
            width: u16::try_from(img.width).unwrap_or(u16::MAX),
            height: u16::try_from(img.height).unwrap_or(u16::MAX),
            unit_to_pixel: [f64::from(width), 0.0, 0.0, f64::from(height), 0.0, 0.0],
            repeat: false,
        },
    };
    let items = paths
        .iter()
        .map(|path| {
            let mut els = Vec::new();
            if let Some(first) = path.anchors.first() {
                let (x, y) = px(first.p);
                els.push(El::MoveTo(x, y));
                for c in path.segments() {
                    let ((ax, ay), (bx, by), (ex, ey)) = (px(c.p1), px(c.p2), px(c.p3));
                    els.push(El::CubicTo(ax, ay, bx, by, ex, ey));
                }
                els.push(El::Close);
            }
            Item::Fill { path: els, rule: Rule::NonZero, paint: paint.clone() }
        })
        .collect();
    let scene = Scene { width, height, background: [255, 255, 255, 255], items };
    let pixels = tt_render::render(&scene, &mut |_| {
        if tt_vector::work::cancel_requested() {
            Err("the call overran its watchdog budget".to_owned())
        } else {
            Ok(())
        }
    })
    .map_err(|e| err("render.failed", e.to_string(), json!({})))?;
    tt_render::encode_png(&pixels, width, height).map_err(|e| err("render.failed", e.to_string(), json!({})))
}

/// One image of a run.
#[derive(Debug, Clone)]
struct Item {
    source: Source,
    /// The cell (file sources) or the source drawing's first frame (image element).
    frame: u32,
    /// The image element drawing it converts.
    source_drawing: Option<Id>,
    replace: bool,
    skip: bool,
    /// A name for a texture colour (the file name).
    name: String,
    /// The file path or source drawing, as reported.
    label: String,
}

/// What a run creates.
#[derive(Debug, Clone)]
struct Plan {
    element: Option<Id>,
    /// The scene of a new element.
    scene: Option<Id>,
    new_name: String,
    /// The image element's exposure, mirrored onto the new element (§11.3).
    mirror: Option<Runs>,
    items: Vec<Item>,
    kind: VectorizeType,
    filter: FilterIn,
    threshold: u8,
}

#[command(
    id = "draw.vectorize.run",
    precheck = crate::prechecks::draw_vectorize_run,
    kind = job,
    label = "Import and Vectorize",
    scopes = [edit, files_read],
    confirm = ["destructive"],
    undo = "Import and Vectorize",
    needs_project,
    example(title = "Vectorize the E2 line art into Bob's frame 1", params = r#"{"files": ["/tmp/lineart.png"], "targetElementId": "scene:#1/element:\"Bob\"", "startFrame": 1, "onConflict": "replace"}"#),
    example(title = "Two images into frames 5 and 6: each is its own undo step, and cancelling keeps those already committed", params = r#"{"files": ["/tmp/lineart.png", "/tmp/lineart.png"], "targetElementId": "scene:#1/element:\"Bob\"", "startFrame": 5, "onConflict": "replace"}"#),
    gui = "File › Import › Import and Vectorize; cell context menu (M4 C4.1)",
)]
pub(crate) fn draw_vectorize_run(cx: &mut Cx, p: RunParams) -> Result<Job<RunResult>, CommandError> {
    let t = threshold(p.threshold)?;
    let doc = cx.document()?.clone();
    let mut plan = Plan {
        element: None,
        scene: cx.app().scene.clone().or_else(|| doc.scenes().next().map(|s| s.id().clone())),
        new_name: "Vectorized".to_owned(),
        mirror: None,
        items: Vec::new(),
        kind: p.kind,
        filter: p.filter,
        threshold: t,
    };
    match (&p.files, &p.image_element_id) {
        (Some(files), None) => {
            if files.is_empty() {
                return Err(invalid("/files", "give at least one file"));
            }
            plan.element = match &p.target_element_id {
                Some(r) => Some(vector_element(cx, r, "/targetElementId")?),
                None => match cx.app().element.clone() {
                    Some(e) => {
                        let ty = doc.element(&e).map(|x| *x.element_type());
                        if !matches!(ty, Some(ElementType::Drawing | ElementType::Annotation)) {
                            return Err(not_vector(&e));
                        }
                        Some(e)
                    }
                    None => None,
                },
            };
            let start = u32::try_from(p.start_frame.unwrap_or(i64::from(cx.app().frame)).max(1)).unwrap_or(1);
            for (i, f) in files.iter().enumerate() {
                let path = persistence::path_param(cx, f, &format!("/files/{i}"))?;
                if !path.is_file() {
                    return Err(file_error(&path, "no such file"));
                }
                let frame = start.saturating_add(u32::try_from(i).unwrap_or(u32::MAX));
                let name =
                    path.file_stem().map_or_else(|| format!("image {}", i + 1), |s| s.to_string_lossy().into_owned());
                if i == 0 {
                    name.clone_into(&mut plan.new_name);
                }
                let label = path.to_string_lossy().into_owned();
                plan.items.push(Item {
                    source: Source::File(path),
                    frame,
                    source_drawing: None,
                    replace: false,
                    skip: false,
                    name,
                    label,
                });
            }
        }
        (None, Some(r)) => {
            if p.target_element_id.is_some() {
                return Err(invalid(
                    "/targetElementId",
                    "converting an image element always creates a new drawing element",
                ));
            }
            let (id, ty) = element_ref(cx, r, "/imageElementId")?;
            if ty != ElementType::Image {
                return Err(invalid("/imageElementId", "not an image element"));
            }
            let e = doc.element(&id).ok_or_else(|| invalid("/imageElementId", "no element"))?;
            plan.new_name = format!("{} Vector", e.name());
            let exposure = runs(&doc, &id);
            for (d, bytes) in bitmaps(cx, &doc, &id)? {
                let mut frame = 1u32;
                let mut first = None;
                for (rd, n) in &exposure {
                    if rd.as_ref() == Some(&d) {
                        first = Some(frame);
                        break;
                    }
                    frame = frame.saturating_add(*n);
                }
                // Drawings the element does not expose are converted too, after its last frame.
                let frame = first.unwrap_or_else(|| exposure.iter().map(|(_, n)| *n).sum::<u32>().saturating_add(1));
                let name = format!("{} {}", e.name(), d.as_str());
                let label = d.as_str().to_owned();
                plan.items.push(Item {
                    source: Source::Bitmap(bytes),
                    frame,
                    source_drawing: Some(d),
                    replace: false,
                    skip: false,
                    name,
                    label,
                });
            }
            if plan.items.is_empty() {
                return Err(err(
                    "drawing.nothingToDo",
                    "the image element has no bitmaps",
                    json!({ "imageElementId": id.as_str() }),
                ));
            }
            plan.mirror = Some(exposure);
        }
        _ => return Err(invalid("", "give exactly one of `files` and `imageElementId`")),
    }
    // Occupied cells of an existing target element.
    if let Some(element) = plan.element.clone() {
        let occupied: Vec<(u32, Id)> = plan
            .items
            .iter()
            .filter_map(|it| AppContext::exposed_drawing(&doc, &element, it.frame).map(|d| (it.frame, d)))
            .collect();
        if !occupied.is_empty() {
            let replace = match p.on_conflict {
                OnConflict::Skip => false,
                OnConflict::Replace => true,
                OnConflict::Ask => {
                    cx.confirm(
                        crate::agents::Category::Destructive,
                        format!("Replace the drawings of {} occupied cell(s) (drawing.cellOccupied)", occupied.len()),
                        occupied
                            .iter()
                            .map(|(f, d)| json!({ "kind": "replaceCell", "code": "drawing.cellOccupied", "frame": f, "drawingId": d.as_str() }))
                            .collect(),
                        None,
                    )?;
                    true
                }
            };
            for it in &mut plan.items {
                if occupied.iter().any(|(f, _)| *f == it.frame) {
                    it.replace = replace;
                    it.skip = !replace;
                }
            }
        }
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    let poster = cx
        .session()
        .env
        .poster
        .clone()
        .ok_or_else(|| CommandError::internal("the document thread cannot be reached"))?;
    let origin = cx.origin().clone();
    let job = cx.session().jobs.submit("draw.vectorize.run", &origin.session_key(), move |progress| {
        run_job(&plan, &poster, &origin, progress)
    })?;
    Ok(Job::new(job))
}

fn not_vector(e: &Id) -> CommandError {
    err(
        "drawing.notVectorElement",
        "the target element is not a vector drawing element",
        json!({ "elementId": e.as_str() }),
    )
}

fn vector_element(cx: &mut Cx, r: &str, pointer: &str) -> Result<Id, CommandError> {
    let (id, ty) = element_ref(cx, r, pointer)?;
    if matches!(ty, ElementType::Drawing | ElementType::Annotation) { Ok(id) } else { Err(not_vector(&id)) }
}

fn run_job(
    plan: &Plan,
    poster: &crate::Poster,
    origin: &crate::types::Origin,
    progress: &crate::jobs::JobProgress,
) -> Result<Value, CommandError> {
    let n = plan.items.len();
    let mut element = plan.element.clone();
    let mut created: Vec<(Option<Id>, String)> = Vec::new();
    let mut cancelled = false;
    let mut skipped: Vec<Skipped> = Vec::new();
    let skip = |i: usize, it: &Item, reason: &str| Skipped {
        index: i as u64,
        source: it.label.clone(),
        reason: reason.to_owned(),
    };
    for (i, it) in plan.items.iter().enumerate() {
        if cancelled || progress.cancelled() {
            cancelled = true;
            skipped.push(skip(i, it, "cancelled"));
            continue;
        }
        progress.report(i as f64 / n.max(1) as f64, &format!("vectorizing image {} of {n}", i + 1));
        if it.skip {
            skipped.push(skip(i, it, "occupied"));
            continue;
        }
        let img = it.source.decode()?;
        let mut work = Work::new(VECTORIZE_WORK_BUDGET, "vectorizing this image");
        let paths = trace(&img, plan.filter, plan.threshold, &mut work)?;
        if progress.cancelled() {
            cancelled = true;
            skipped.push(skip(i, it, "cancelled"));
            continue;
        }
        let commit = Commit {
            element: element.clone(),
            scene: plan.scene.clone(),
            new_name: plan.new_name.clone(),
            frame: it.frame,
            replace: it.replace,
            paths,
            texture: (plan.kind == VectorizeType::WithTexture).then(|| (img, it.name.clone())),
            mirror: plan.mirror.clone().map(|m| (m, created.clone(), it.source_drawing.clone())),
        };
        let origin = origin.clone();
        let env = poster.run(POST_TIMEOUT, move |core| {
            core.run_staged(
                &origin,
                "draw.vectorize.run",
                "draw.vectorize.run",
                "Import and Vectorize",
                false,
                move |cx| commit_image(cx, commit),
            )
        })?;
        if let Some(e) = env.error {
            return Err(e);
        }
        let r = env.result.unwrap_or(Value::Null);
        let drawing = r["drawingId"].as_str().unwrap_or_default().to_owned();
        element = r["elementId"].as_str().and_then(|e| Id::parse(e).ok()).or(element);
        created.push((it.source_drawing.clone(), drawing));
    }
    let result = RunResult {
        element_id: element.map(|e| e.as_str().to_owned()).unwrap_or_default(),
        drawing_ids: created.into_iter().map(|(_, d)| d).collect(),
        completed: 0,
        cancelled,
        skipped,
    };
    let completed = result.drawing_ids.len() as u64;
    serde_json::to_value(RunResult { completed, ..result }).map_err(|e| CommandError::internal(e.to_string()))
}

/// One image's transaction.
struct Commit {
    element: Option<Id>,
    scene: Option<Id>,
    new_name: String,
    frame: u32,
    replace: bool,
    paths: Vec<Path>,
    texture: Option<(tt_io::image::Rgba, String)>,
    /// The image element's exposure mirrored onto the new element.
    mirror: Option<Mirror>,
}

/// Stages one vectorized image: the new element on the first image without a target, the colour, the cleared cell
/// when replacing, the drawing with one layer of brush shapes (`draw.content.add`), and the mirrored exposure.
fn commit_image(cx: &mut Cx, c: Commit) -> Result<Value, CommandError> {
    let element = match &c.element {
        Some(e) => e.clone(),
        None => stage_element(cx, c.scene.as_ref(), &c.new_name)?,
    };
    let (colour, matrix) = match &c.texture {
        None => (black(cx)?, None),
        Some((img, name)) => texture(cx, img, name)?,
    };
    if c.replace {
        clear_cell(cx, &element, c.frame)?;
    }
    let strokes: Vec<Value> = c
        .paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pts: Vec<Value> = p.to_bezier_list().iter().map(|q| json!({ "x": q.x, "y": q.y })).collect();
            let mut s = json!({ "id": format!("s_{i}"), "kind": "brush", "closed": true, "colourId": colour, "outline": [pts] });
            if let Some(m) = matrix {
                s["textureMatrix"] = json!(m);
            }
            s
        })
        .collect();
    let params = json!({
        "elementId": element.as_str(), "frame": c.frame, "createDrawing": true, "select": false,
        "content": { "nodes": [{ "type": "layer", "id": "l_1", "name": "Vectorized", "layerKind": "brush", "strokes": strokes }] },
    });
    let p: super::draw_stroke::ContentAddParams =
        serde_json::from_value(params).map_err(|e| CommandError::internal(format!("vectorized content: {e}")))?;
    let r = super::draw_stroke::draw_content_add(cx, p)?;
    let r = serde_json::to_value(r).map_err(|e| CommandError::internal(e.to_string()))?;
    let drawing = r["drawingId"].as_str().unwrap_or_default().to_owned();
    if let Some((exposure, done, source)) = &c.mirror {
        let mut map: Vec<(Option<Id>, String)> = done.clone();
        map.push((source.clone(), drawing.clone()));
        let runs: Vec<Value> = exposure
            .iter()
            .map(|(d, n)| {
                let mapped =
                    d.as_ref().and_then(|d| map.iter().find(|(s, _)| s.as_ref() == Some(d)).map(|(_, x)| x.clone()));
                json!([mapped, n])
            })
            .collect();
        cx.patch_entries(vec![(
            EntryKey::Element(element.clone()),
            vec![op("replace", "/exposure", Some(Value::Array(runs)))],
        )])?;
    }
    Ok(json!({ "elementId": element.as_str(), "drawingId": drawing }))
}

/// Stages a new drawing element over a new empty bank at the end of `scene`'s columns.
fn stage_element(cx: &mut Cx, scene: Option<&Id>, name: &str) -> Result<Id, CommandError> {
    let scene = scene
        .cloned()
        .ok_or_else(|| err("drawing.nothingToDo", "the project has no scene for a new element", json!({})))?;
    let id = cx.new_id(ObjectType::Element)?;
    let bank = cx.new_id(ObjectType::DrawingBank)?;
    let mut element = tt_model::defaults::drawing_element_json(&id, name, &bank, &crate::time::now_iso());
    element["rev"] = json!(0);
    cx.patch_entries(vec![
        (EntryKey::Element(id.clone()), vec![op("add", "", Some(element))]),
        (
            EntryKey::Bank(bank.clone()),
            vec![op("add", "", Some(json!({ "id": bank.as_str(), "type": "drawingBank", "rev": 0, "drawings": [] })))],
        ),
        (
            EntryKey::Project,
            vec![op("add", "/drawingBanks/-", Some(json!({ "id": bank.as_str(), "type": "drawingBank" })))],
        ),
        (
            EntryKey::Scene(scene),
            vec![
                op("add", "/elements/-", Some(json!({ "id": id.as_str(), "type": "element" }))),
                op("add", "/tree/-", Some(json!({ "id": id.as_str(), "children": [] }))),
                op("add", "/columnOrder/-", Some(json!(id.as_str()))),
            ],
        ),
    ])?;
    Ok(id)
}

/// Removes the drawing a cell exposes (its frames become blank), so the cell receives a new drawing.
fn clear_cell(cx: &mut Cx, element: &Id, frame: u32) -> Result<(), CommandError> {
    let doc = cx.document()?.clone();
    let mut frames: Vec<Option<String>> = Vec::new();
    for (d, n) in runs(&doc, element) {
        for _ in 0..n {
            frames.push(d.as_ref().map(|x| x.as_str().to_owned()));
        }
    }
    if let Some(f) = frames.get_mut(frame.saturating_sub(1) as usize) {
        *f = None;
    }
    let mut out: Vec<(Option<String>, u32)> = Vec::new();
    for f in frames {
        match out.last_mut() {
            Some((d, n)) if *d == f => *n += 1,
            _ => out.push((f, 1)),
        }
    }
    while out.last().is_some_and(|(d, _)| d.is_none()) {
        out.pop();
    }
    let runs = Value::Array(out.into_iter().map(|(d, n)| json!([d, n])).collect());
    cx.patch_entries(vec![(EntryKey::Element(element.clone()), vec![op("replace", "/exposure", Some(runs))])])
}

/// The vectorization black: the swatch of that name in the "Vectorization" palette, created with the palette when
/// missing.
fn black(cx: &mut Cx) -> Result<String, CommandError> {
    let doc = cx.document()?.clone();
    if let Some(c) = doc
        .palettes()
        .filter(|p| p.name() == PALETTE)
        .flat_map(|p| p.swatches().iter())
        .find(|s| s.name() == BLACK)
        .map(|s| s.colour_id().as_str().to_owned())
    {
        return Ok(c);
    }
    let id = cx.new_id(ObjectType::Swatch)?.as_str().to_owned();
    stage_colour(cx, &doc, &id, BLACK, "solid", json!({ "rgba": "#000000FF" }))?;
    Ok(id)
}

/// A texture colour holding the source bitmap (stored as a PNG package entry) and the fill matrix that spreads it
/// over the image's extent (the frame width, centred on the origin).
fn texture(cx: &mut Cx, img: &tt_io::image::Rgba, name: &str) -> Result<(String, Option<[f64; 6]>), CommandError> {
    let mut premul = img.pixels.clone();
    for px in premul.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = u8::try_from((u16::from(*c) * a + 127) / 255).unwrap_or(255);
        }
    }
    let png = tt_render::encode_png(&premul, img.width, img.height)
        .map_err(|e| err("render.failed", e.to_string(), json!({})))?;
    let hash = tt_persist::fsx::sha256(&png);
    let entry = format!("textures/vectorize-{}.png", &hash[..16]);
    if !cx.dry_run()
        && let Ok(p) = cx.project_mut()
    {
        // Content-addressed, so inserting it again is harmless; like `doc.import.json`, the entry is stored with the
        // call and an unreferenced entry is dropped at the next save.
        p.store.blobs.insert(entry.clone(), tt_persist::container::BlobData::Bytes(Arc::new(png)));
    }
    let doc = cx.document()?.clone();
    let id = cx.new_id(ObjectType::Swatch)?.as_str().to_owned();
    stage_colour(
        cx,
        &doc,
        &id,
        name,
        "texture",
        json!({ "image": entry, "tile": false, "width": img.width, "height": img.height }),
    )?;
    let w = vectorize::FRAME_WIDTH;
    let h = w * f64::from(img.height) / f64::from(img.width);
    // Unit square → drawing space: the bitmap's top-left corner at (−w/2, h/2), its rows running down.
    Ok((id, Some([w, 0.0, 0.0, -h, -w / 2.0, h / 2.0])))
}

fn stage_colour(
    cx: &mut Cx,
    doc: &Document,
    id: &str,
    name: &str,
    kind: &str,
    value: Value,
) -> Result<(), CommandError> {
    let entry = json!({
        "paletteName": PALETTE,
        "styles": [],
        "swatch": { "name": name, "kind": kind, "deformable": false, "values": { "value": value } },
    });
    let (_, changes) = super::colour_recovery::recover(cx, doc, &[(id.to_owned(), Some(entry))])?;
    cx.patch_entries(changes)
}
