//! The interactive canvas's drawing payloads (M4 C4.2b; design note 1 §2, §6 and §8; ADR-0007 decisions 1, 2, 4).
//!
//! The document thread only takes a snapshot (the document by reference count, the planar cache's handle, the blob
//! store) and the drawing's revision; walking, tessellating and encoding run on the caller's thread (design note 1
//! §6: the host's worker), so the document thread's hold stays within plan §6's 20 ms.

use serde_json::{Value, json};
use tt_model::Id;
use tt_model::document::Document;
use tt_render::canvas::{self, CanvasDrawing, CanvasItem, Identity, Slot, SlotKind};
use tt_render::{Item, Paint};

use crate::commands::draw_render::{Sources, Tag, canvas_items};
use tt_registry::CommandError;

/// What a payload is built from, taken on the document thread.
#[derive(Clone)]
pub(crate) struct CanvasSource {
    pub doc: Document,
    pub drawing: Id,
    /// The drawing's selected objects (local ids) when the source was taken: the selection parts' filter.
    pub selection: Vec<String>,
    pub cache: crate::planar::SharedCache,
    pub blobs: Option<tt_persist::container::Blobs>,
}

/// One encoded drawing for the canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasPayload {
    /// The drawing id (`dr_…`).
    pub drawing_id: String,
    /// The drawing's revision.
    pub rev: u64,
    /// The TTRD bytes.
    pub bytes: Vec<u8>,
    /// Render warnings of the walk (a missing colour, a missing bitmap).
    pub warnings: Vec<Value>,
}

fn not_found(id: &str) -> CommandError {
    CommandError::with_details("ref.notFound", format!("drawing `{id}` was not found"), json!({ "ref": id }))
}

/// The mean straight-alpha colour of RGBA8 pixels (a texture's or a bitmap's preview colour, SI-137).
fn mean(rgba: &[u8]) -> [u8; 4] {
    let n = (rgba.len() / 4).max(1) as u64;
    let mut sum = [0u64; 4];
    for px in rgba.as_chunks::<4>().0 {
        for (s, c) in sum.iter_mut().zip(px) {
            *s += u64::from(*c);
        }
    }
    sum.map(|s| u8::try_from((s + n / 2) / n).unwrap_or(u8::MAX))
}

/// The slot value of a paint.
fn slot_of(paint: &Paint) -> (SlotKind, [u8; 4]) {
    match paint {
        Paint::Solid(c) => (SlotKind::Solid, *c),
        Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => {
            (SlotKind::Gradient, stops.first().map_or([0, 0, 0, 0], |s| s.1))
        }
        Paint::Image { rgba, .. } => (SlotKind::Texture, mean(rgba)),
    }
}

fn paint_of(item: &Item) -> Option<&Paint> {
    match item {
        Item::Fill { paint, .. } | Item::Stroke { paint, .. } => Some(paint),
        Item::Layer { .. } => None,
    }
}

/// The variants of one drawing's payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PayloadOptions {
    /// Outlines only (02 §10.2; header flag 1).
    pub outline: bool,
    /// Approximate pencil strokes: no round joins (`display.fastPencilRendering`, 01 §7.2; header flag 2).
    pub fast_pencil: bool,
    /// Which objects (M4 C4.3b, preview note §3): every object, only the selected ones, or the rest.
    pub part: Part,
}

/// The objects a payload holds (preview note §3): the Drawing view's live preview draws the selected objects apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Part {
    /// Every object.
    #[default]
    All,
    /// The drawing's selected objects only.
    Selection,
    /// Every object but the selected ones.
    Rest,
}

/// Header flag of a payload with approximate pencil strokes.
pub const FLAG_FAST_PENCIL: u16 = 2;

/// The canvas form of one drawing: the final renderer's walk, colour slots, tessellated records (design note 1 §8).
///
/// # Errors
/// `ref.notFound` for a drawing not in the snapshot; the walk's refusals (`limit.exceeded`, planar-map errors).
pub(crate) fn build(
    src: &CanvasSource,
    generation: u64,
    bucket: i16,
    opts: PayloadOptions,
) -> Result<(CanvasDrawing, Vec<Value>), CommandError> {
    let outline = opts.outline;
    let d = src.doc.drawing(&src.drawing).ok_or_else(|| not_found(src.drawing.as_str()))?;
    let never = || false;
    let sources = Sources { cache: &src.cache, blobs: src.blobs.as_ref(), selection: None, stop: &never };
    let only = match opts.part {
        Part::All => None,
        Part::Selection => Some((src.selection.iter().cloned().collect(), true)),
        Part::Rest => Some((src.selection.iter().cloned().collect(), false)),
    };
    let (items, tags, warnings) = canvas_items(&sources, &src.doc, d, None, only)?;
    let mut slots: Vec<Slot> = Vec::new();
    let mut out = Vec::with_capacity(items.len());
    for (item, tag) in items.into_iter().zip(tags) {
        let (id, bitmap) = match tag {
            Tag::Colour(c) => (c, None),
            Tag::Bitmap(e) => (e.clone(), Some(e)),
        };
        let slot = match slots.iter().position(|s| s.id == id) {
            Some(i) => i,
            None => {
                let (kind, rgba) = paint_of(&item).map_or((SlotKind::Solid, [0, 0, 0, 0]), slot_of);
                slots.push(Slot { id, kind, rgba });
                slots.len() - 1
            }
        };
        let Ok(slot) = u16::try_from(slot) else { continue };
        let item = if outline { outline_of(item, bucket) } else { Some(item) };
        if let Some(item) = item {
            out.push(CanvasItem { item, slot, bitmap });
        }
    }
    let records = canvas::tessellate_with(&out, bucket, !opts.fast_pencil);
    let bounds = canvas::bounds(&records);
    let ulid = src.drawing.as_str().strip_prefix("dr_").unwrap_or(src.drawing.as_str()).to_owned();
    let drawing = CanvasDrawing {
        identity: Identity { generation, rev: *d.rev(), drawing_id: ulid, bucket },
        flags: (if outline { FLAG_OUTLINE } else { 0 }) | (if opts.fast_pencil { FLAG_FAST_PENCIL } else { 0 }),
        slots,
        records,
        bounds,
    };
    Ok((drawing, warnings))
}

/// Header flag of an outline-only payload (02 §10.2; SI-139).
pub const FLAG_OUTLINE: u16 = 1;

/// The outline-only form of a walk item (02 §10.2: brush outlines and pencil centre lines, no fills; SI-139): one view
/// pixel wide at the bucket's top zoom; planar fills are left out.
fn outline_of(item: Item, bucket: i16) -> Option<Item> {
    let width = canvas::tolerance(bucket) * 4.0;
    match item {
        Item::Fill { rule: tt_render::Rule::EvenOdd, .. } => None,
        Item::Fill { path, paint, .. } | Item::Stroke { path, paint, .. } => Some(Item::Stroke { path, width, paint }),
        Item::Layer { transform, items } => {
            Some(Item::Layer { transform, items: items.into_iter().filter_map(|i| outline_of(i, bucket)).collect() })
        }
    }
}

/// [`build`] then [`canvas::encode`].
///
/// # Errors
/// As [`build`]; `internal` when the drawing does not fit the format.
pub(crate) fn payload(
    src: &CanvasSource,
    generation: u64,
    bucket: i16,
    opts: PayloadOptions,
) -> Result<CanvasPayload, CommandError> {
    let (drawing, warnings) = build(src, generation, bucket, opts)?;
    let bytes = canvas::encode(&drawing).map_err(|e| CommandError::internal(e.to_string()))?;
    Ok(CanvasPayload { drawing_id: src.drawing.as_str().to_owned(), rev: drawing.identity.rev, bytes, warnings })
}

/// One drawing shown in the Drawing view (02 §4.2 layers 3 to 7), back to front.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneEntry {
    /// The drawing id.
    pub drawing_id: String,
    /// Its revision: the canvas loads it when it holds another one.
    pub rev: u64,
    /// `current`, `onion`, `autoLight`, `staticUnder` or `staticOver`.
    pub role: String,
    /// Coverage multiplier, 0-1 (SI-139).
    pub opacity: f32,
    /// A tint replacing the colour, keeping coverage (SI-139); `null` for none.
    pub tint: Option<[u8; 4]>,
    /// Washed out (SI-139: 40 % opacity).
    pub shade: bool,
    /// Outlines only (02 §10.2).
    pub outline: bool,
    /// Camera view: the element this occurrence draws (scene answer form 1.1, `docs/canvas-format.md` §11.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    /// Camera view: drawing units → camera output pixels, the final renderer's plane matrix (camera-view note §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f64; 6]>,
}

/// The Camera view's camera (scene answer form 1.1, §11.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneCamera {
    /// The camera element.
    pub element_id: String,
    /// The output size in pixels, `[width, height]`.
    pub size: [u32; 2],
    /// Camera output pixels → view pixels (the view transform).
    pub camera_to_view: [f64; 6],
}

/// The selection display options (02 lines 959-961), as `drawAid.selectionDisplay.set` sets them.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelectionDisplay {
    /// Outline on selection.
    pub outline_on_selection: bool,
    /// Box highlighting.
    pub box_highlighting: bool,
    /// Keep the selection visible.
    pub keep_selection_visible: bool,
}

/// The Camera view's overlays in camera output pixels (scene answer form 1.1, §11.4): never in a render (05 lines
/// 283-284).
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneOverlays {
    /// The camera frame, `[0, 0, width, height]`.
    pub camera_frame: [f64; 4],
    /// Each selected element's pivot.
    pub pivots: Vec<[f64; 2]>,
    /// Each selected element's box.
    pub selection_boxes: Vec<[f64; 4]>,
    /// The union of the selection's boxes; `null` without a selection.
    pub selection_union: Option<[f64; 4]>,
    /// Each selected non-current camera's frame on the table plane, four corners (05 line 901).
    pub other_cameras: Vec<[[f64; 2]; 4]>,
    /// 02 lines 959-961.
    pub selection_display: SceneSelectionDisplay,
    /// 02 line 946.
    pub current_on_top: bool,
}

/// The grid settings the canvas draws (02 §10.1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneGrid {
    /// Shown.
    pub visible: bool,
    /// `normal`, `field12` or `field16`.
    #[serde(rename = "type")]
    pub ty: String,
    /// Over the drawings (layer 8) rather than under them (layer 2).
    pub overlay: bool,
}

/// A view's composition (design note 1 §9; camera-view note §3): what `canvas_scene` answers, the scene answer of
/// `docs/canvas-format.md` §11.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    /// The scene answer's form (§11.2): `"1.1"`.
    pub scene_form: String,
    /// The document revision the scene was built at (C43B-R3S-02, ruling D-48): the canvas lets a gesture's preview
    /// go only when a scene at the revision its commit answered with is resident, never on an answer requested before.
    pub document_rev: u64,
    /// The view (`drawingView`, `cameraView`, or a float of either).
    pub view_id: String,
    /// `drawing` or `camera`.
    pub view_type: String,
    /// The background (02 §10.7), straight-alpha RGBA8.
    pub background: [u8; 4],
    /// The grid (02 §10.1).
    pub grid: SceneGrid,
    /// The drawings shown, back to front.
    pub entries: Vec<SceneEntry>,
    /// Every solid colour id's value (the palette's first style, as the final renderer's walk resolves it).
    pub colours: std::collections::BTreeMap<String, [u8; 4]>,
    /// Changes when any palette, style or swatch changes: drawings with gradient or texture slots reload on it.
    pub palette_stamp: u64,
    /// The current drawing's pivot in drawing units (02 §7.1.1: shown with the Select tool, draggable); `null` without
    /// a current drawing.
    pub drawing_pivot: Option<[f64; 2]>,
    /// The current drawing's selection (frame-loop note §7.1); `null` when nothing is selected.
    pub selection: Option<SceneSelection>,
    /// Camera view: the scene frame shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    /// Camera view: the camera, `null` without a current camera (SI-147); absent in the Drawing view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<Option<SceneCamera>>,
    /// Camera view: the overlays; absent in the Drawing view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlays: Option<SceneOverlays>,
}

/// The scene answer's form (`docs/canvas-format.md` §11.2).
pub const SCENE_FORM: &str = "1.1";

/// The selection the Drawing view's overlay draws (layer 9; frame-loop note §7.1), in drawing units.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelection {
    /// Each selected object's bounds, `[minX, minY, maxX, maxY]` (an object's outline in C4.2, SI-141).
    pub objects: Vec<[f64; 4]>,
    /// The selection's bounding box, as `draw.selection.get` answers it; `null` for a point selection alone.
    pub bounds: Option<[f64; 4]>,
    /// The transform pivot: the bounding box's centre (`draw.transform.apply`'s default pivot); `null` with `bounds`.
    pub pivot: Option<[f64; 2]>,
    /// The point selection's anchors (Contour Editor).
    pub points: Vec<[f64; 2]>,
}

fn rgba(text: &str, fallback: [u8; 4]) -> [u8; 4] {
    tt_model::value::Colour::parse(text).map_or(fallback, |c| c.channels())
}

/// The composition of the Drawing view `view_id` from the session (document thread; reads only).
///
/// # Errors
/// `view.notFound` for a view that is not a Drawing view window; `no.project`.
pub(crate) fn scene(s: &crate::session::Session, view_id: &str) -> Result<Scene, CommandError> {
    if view_id != "drawingView" {
        return Err(CommandError::with_details(
            "view.notFound",
            format!("`{view_id}` is not a Drawing view"),
            json!({ "viewId": view_id }),
        ));
    }
    let p = s.project.as_ref().ok_or_else(crate::cx::no_project)?;
    let doc = &p.doc;
    let aids = &s.ui.drawing().aids;
    let background = if aids.background.use_scene_colour {
        s.app.scene.as_ref().and_then(|sc| doc.scene(sc)).map_or_else(
            || rgba(&aids.background.colour, [240, 240, 240, 255]),
            |sc| rgba(&sc.background_colour().to_string(), [240, 240, 240, 255]),
        )
    } else {
        rgba(&aids.background.colour, [240, 240, 240, 255])
    };
    let mut entries = Vec::new();
    let entry = |id: &Id, role: &str, opacity: f32, tint: Option<[u8; 4]>, shade: bool, outline: bool| {
        doc.drawing(id).map(|d| SceneEntry {
            drawing_id: id.as_str().to_owned(),
            rev: *d.rev(),
            role: role.to_owned(),
            opacity,
            tint,
            shade,
            outline,
            element_id: None,
            matrix: None,
        })
    };
    let current = s.app.current_drawing(doc);
    // 02 §10.4: the scene's static light-table entries shown, under or over the current drawing; shade when on.
    let statics: Vec<_> = s
        .app
        .scene
        .as_ref()
        .and_then(|sc| s.timeline.light_tables.get(sc.as_str()))
        .map(|items| items.iter().filter(|i| i.visible).collect())
        .unwrap_or_default();
    for item in statics.iter().filter(|i| i.placement != "over") {
        if let Ok(id) = Id::parse(&item.drawing_id) {
            entries.extend(entry(&id, "staticUnder", 1.0, None, aids.light_table_shade.stat, false));
        }
    }
    // 02 §10.3: the other drawing elements' drawings exposed at the current frame, back to front (the final renderer's
    // order for equal depths: the Timeline's bottom row first), not those hidden in the Exposure Sheet; shade when on.
    if aids.auto_light_table
        && let Some(sc) = s.app.scene.as_ref().and_then(|sc| doc.scene(sc))
    {
        let mut order = Vec::new();
        fn walk(nodes: &[tt_model::entities::scene::TreeNode], out: &mut Vec<Id>) {
            for n in nodes {
                out.push(n.id.clone());
                walk(&n.children, out);
            }
        }
        walk(sc.tree(), &mut order);
        for el in order.iter().rev() {
            if Some(el) == s.app.element.as_ref() {
                continue;
            }
            let hidden = doc.element(el).is_some_and(|e| *e.visible_in_xsheet() == Some(false));
            if hidden {
                continue;
            }
            if let Some(d) = crate::app::AppContext::exposed_drawing(doc, el, s.app.frame) {
                entries.extend(entry(&d, "autoLight", 1.0, None, aids.light_table_shade.auto, false));
            }
        }
    }
    // 02 §10.2: previous and next drawings of the current element, counted in distinct drawings of its exposure order,
    // farther ones first (they are fainter: 50, 30, 15 % with transparency), tinted with colour shading (SI-139).
    let os = &aids.onion_skin;
    if os.enabled
        && let (Some(el), Some(cur)) = (s.app.element.as_ref(), current.as_ref())
    {
        let style = &aids.onion_skin_style;
        // The distinct drawings in exposure order (blanks skipped, repeats of one drawing merged), each with the first
        // frame of its occurrence (frames count from 1, as `AppContext::exposed_drawing`).
        let mut distinct: Vec<Id> = Vec::new();
        let mut starts: Vec<u64> = Vec::new();
        let mut frame = 1u64;
        for (d, count) in doc.element(el).and_then(|e| e.exposure().clone()).unwrap_or_default() {
            if let Some(d) = d
                && distinct.last() != Some(&d)
            {
                distinct.push(d);
                starts.push(frame);
            }
            frame += u64::from(count);
        }
        // The current drawing's place is its occurrence at the current frame: the last occurrence starting at or before
        // it (a drawing exposed again later in the element is a later occurrence, C42B-R1-02); an explicit current
        // drawing not exposed by then takes its first occurrence.
        let now = u64::from(s.app.frame);
        let at = (0..distinct.len())
            .rev()
            .find(|&i| distinct[i] == *cur && starts[i] <= now)
            .or_else(|| distinct.iter().position(|d| d == cur));
        let fade =
            |k: usize| if style.transparency { [0.5f32, 0.3, 0.15].get(k - 1).copied().unwrap_or(0.15) } else { 1.0 };
        let tint = |c: &str| style.colour_shading.then(|| rgba(c, [255, 0, 0, 255]));
        if let Some(i) = at {
            let take = |n: i64| usize::try_from(n.clamp(0, 3)).unwrap_or(0);
            let prev: Vec<(usize, &Id)> =
                (1..=take(os.previous)).filter_map(|k| i.checked_sub(k).map(|j| (k, &distinct[j]))).collect();
            let next: Vec<(usize, &Id)> =
                (1..=take(os.next)).filter_map(|k| distinct.get(i + k).map(|d| (k, d))).collect();
            let mut onion: Vec<(usize, &Id, &str)> =
                prev.iter().map(|(k, d)| (*k, *d, style.previous_colour.as_str())).collect();
            onion.extend(next.iter().map(|(k, d)| (*k, *d, style.next_colour.as_str())));
            onion.sort_by_key(|o| std::cmp::Reverse(o.0));
            for (k, d, colour) in onion {
                if d != cur {
                    entries.extend(entry(d, "onion", fade(k), tint(colour), false, style.outline));
                }
            }
        }
    }
    if let Some(id) = current.as_ref() {
        entries.extend(entry(id, "current", 1.0, None, false, false));
    }
    for item in statics.iter().filter(|i| i.placement == "over") {
        if let Ok(id) = Id::parse(&item.drawing_id) {
            entries.extend(entry(&id, "staticOver", 1.0, None, aids.light_table_shade.stat, false));
        }
    }
    let (colours, palette_stamp) = colours_of(doc, None);
    Ok(Scene {
        scene_form: SCENE_FORM.to_owned(),
        document_rev: doc.document_rev(),
        view_id: view_id.to_owned(),
        view_type: "drawing".to_owned(),
        background,
        grid: SceneGrid { visible: aids.grid.visible, ty: aids.grid.ty.clone(), overlay: aids.grid.overlay },
        entries,
        colours,
        palette_stamp,
        drawing_pivot: current.as_ref().and_then(|id| doc.drawing(id)).map(|d| [d.pivot().x, d.pivot().y]),
        // Filled by [`selection`] with a read-only context (the selection walk needs one).
        selection: None,
        frame: None,
        camera: None,
        overlays: None,
    })
}

/// Every solid colour id's value and the palette stamp. `styles` maps a palette to the style the scene uses (the final
/// renderer's camera walk, `render_list::items`); a palette it does not name, and every palette without `styles` (the
/// Drawing view), takes its first style.
fn colours_of(
    doc: &Document,
    styles: Option<&std::collections::BTreeMap<Id, Id>>,
) -> (std::collections::BTreeMap<String, [u8; 4]>, u64) {
    let mut colours = std::collections::BTreeMap::new();
    let mut stamp: u64 = 0;
    for pal in doc.palettes() {
        let first = pal.styles().first().map(|st| st.id().clone());
        let style = styles.and_then(|m| m.get(pal.id()).cloned()).or(first);
        for st in pal.styles() {
            stamp = stamp.wrapping_mul(31).wrapping_add(*st.rev());
        }
        for sw in pal.swatches() {
            stamp = stamp.wrapping_mul(31).wrapping_add(*sw.rev());
            if let Some(tt_model::entities::palette::SwatchValue::Solid(c)) =
                style.as_ref().and_then(|f| sw.values().get(f))
            {
                colours.insert(sw.colour_id().as_str().to_owned(), c.rgba.channels());
            }
        }
        stamp = stamp.wrapping_mul(31).wrapping_add(pal.swatches().len() as u64);
    }
    // Within 2^53, so it crosses JSON as an exact number.
    (colours, stamp & ((1 << 53) - 1))
}

/// The opacity of an unselected occurrence when box highlighting is off (SI-153).
const FADED: f32 = 0.35;
/// The highlight colour of the selection display (SI-153: SI-138's accent colour).
const HIGHLIGHT: [u8; 4] = [32, 128, 255, 255];

/// The Camera view's composition (camera-view note §1-§3): the final renderer's camera walk at the view's frame and
/// the camera's output size, each element with a plane matrix in front of the camera as an occurrence (drawn, or out
/// of the frame: the view shows the table around it), in the walk's order; the selection display's view-only entries
/// (camera-view note §6, CV5); and the overlays.
///
/// # Errors
/// `view.notFound`, `no.project`, `render.noScene`; the evaluator's refusals.
pub(crate) fn camera_scene(cx: &crate::cx::Cx, view_id: &str) -> Result<Scene, CommandError> {
    use crate::commands::render_list::{self, Overrides, Request, Verdict};
    let w = cx.ui().shell().view(view_id)?.clone();
    let doc = cx.document()?;
    let scene_id = crate::camera_view::current_scene(cx)?;
    let sc = doc.scene(&scene_id);
    let background =
        sc.map_or([240, 240, 240, 255], |sc| rgba(&sc.background_colour().to_string(), [240, 240, 240, 255]));
    let styles: Option<std::collections::BTreeMap<Id, Id>> =
        sc.map(|s| s.palette_styles().iter().map(|(p, st)| (p.clone(), st.clone())).collect());
    let (colours, palette_stamp) = colours_of(doc, styles.as_ref());
    let aids = &cx.ui().drawing().aids;
    let display = SceneSelectionDisplay {
        outline_on_selection: aids.selection_display.outline_on_selection,
        box_highlighting: aids.selection_display.box_highlighting,
        keep_selection_visible: aids.selection_display.keep_selection_visible,
    };
    let mut out = Scene {
        scene_form: SCENE_FORM.to_owned(),
        document_rev: doc.document_rev(),
        view_id: view_id.to_owned(),
        view_type: "camera".to_owned(),
        background,
        grid: SceneGrid { visible: aids.grid.visible, ty: aids.grid.ty.clone(), overlay: aids.grid.overlay },
        entries: Vec::new(),
        colours,
        palette_stamp,
        drawing_pivot: None,
        selection: None,
        frame: Some(cx.app().frame.max(1)),
        camera: Some(None),
        overlays: None,
    };
    let chain = match crate::camera_view::chain(cx, &w) {
        Ok(c) => c,
        Err(e) if e.code == "render.noCamera" => {
            let (cw, ch) = crate::commands::render::output_size(doc, None, None, 1.0)?;
            out.overlays = Some(SceneOverlays {
                camera_frame: [0.0, 0.0, f64::from(cw), f64::from(ch)],
                pivots: Vec::new(),
                selection_boxes: Vec::new(),
                selection_union: None,
                other_cameras: Vec::new(),
                selection_display: display,
                current_on_top: aids.current_on_top,
            });
            return Ok(out);
        }
        Err(e) => return Err(e),
    };
    let overrides = Overrides::default();
    let req = Request {
        scene: &chain.scene,
        frame: chain.frame,
        camera: None,
        width: chain.width,
        height: chain.height,
        only: None,
        overrides: &overrides,
    };
    let mut ev = crate::commands::anim_common::evaluator(doc, "composing the Camera view");
    let list = render_list::build(doc, &req, &mut ev)?;
    let current = cx.app().element.clone();
    let selected: Vec<Id> = cx.app().selection_list("staging").iter().filter_map(|k| Id::parse(k).ok()).collect();
    // The selection display (05 §3.2 lines 319-321, 02 §10.7; camera-view note §6, CV5; SI-153): with box highlighting
    // off the unselected occurrences are faded; with outline on selection (which needs box highlighting) each selected
    // occurrence is drawn again as outlines in the highlight colour, on top, even where hidden; with selection always
    // visible a selected element hidden in the scene is shown. These entries are the view's alone: a render never has
    // them (CV4).
    let fade = !display.box_highlighting;
    let outlines = display.box_highlighting && display.outline_on_selection;
    let mut outlined = Vec::new();
    for en in &list.entries {
        let (Some(m), Some(d)) = (en.to_pixel, en.drawing.as_ref().and_then(|d| doc.drawing(d))) else { continue };
        let is_selected = selected.contains(&en.id);
        let shown = matches!(en.verdict, Verdict::Drawn | Verdict::OutOfFrame)
            || (en.verdict == Verdict::Hidden && is_selected && display.keep_selection_visible);
        if is_selected && outlines && (shown || en.verdict == Verdict::Hidden) {
            outlined.push((d, en, m));
        }
        if !shown {
            continue;
        }
        out.entries.push(SceneEntry {
            drawing_id: d.id().as_str().to_owned(),
            rev: *d.rev(),
            role: if current.as_ref() == Some(&en.id) { "current" } else { "staticUnder" }.to_owned(),
            opacity: if fade && !is_selected { FADED } else { 1.0 },
            tint: None,
            shade: false,
            outline: false,
            element_id: Some(en.id.as_str().to_owned()),
            matrix: Some(m.0),
        });
    }
    for (d, en, m) in outlined {
        out.entries.push(SceneEntry {
            drawing_id: d.id().as_str().to_owned(),
            rev: *d.rev(),
            role: if current.as_ref() == Some(&en.id) { "current" } else { "staticUnder" }.to_owned(),
            opacity: 1.0,
            tint: Some(HIGHLIGHT),
            shade: false,
            outline: true,
            element_id: Some(en.id.as_str().to_owned()),
            matrix: Some(m.0),
        });
    }
    // The overlays: the selected elements' boxes and pivots, and the selected non-current cameras' frames.
    let mut boxes = Vec::new();
    let mut pivots = Vec::new();
    let mut cameras = Vec::new();
    for id in &selected {
        if let Some(en) = list.entries.iter().find(|en| &en.id == id) {
            if let Some(b) = en.pixel_box {
                boxes.push([b.min_x, b.min_y, b.max_x, b.max_y]);
            }
            let pivot = doc
                .element(id)
                .and_then(|e| e.transform().as_ref().and_then(|t| t.rotation_pivot.as_ref()).map(|p| (p.x, p.y)))
                .unwrap_or((0.0, 0.0));
            if let Some(q) = crate::camera_view::scene_to_output(&chain, en.world.apply([pivot.0, pivot.1, 0.0])) {
                pivots.push([q.x, q.y]);
            }
        } else if id != &chain.camera
            && doc.element(id).is_some_and(|e| *e.element_type() == tt_model::entities::element::ElementType::Camera)
        {
            let cam = ev.camera(id, f64::from(chain.frame)).map_err(crate::commands::anim_common::eval_error)?;
            if let Some(corners) = cam.frame_corners_at_z0() {
                let projected: Vec<_> =
                    corners.iter().filter_map(|c| crate::camera_view::scene_to_output(&chain, *c)).collect();
                if let [a, b, c, d] = projected.as_slice() {
                    cameras.push([[a.x, a.y], [b.x, b.y], [c.x, c.y], [d.x, d.y]]);
                }
            }
        }
    }
    let union = boxes.iter().copied().reduce(|u, b| [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])]);
    out.overlays = Some(SceneOverlays {
        camera_frame: [0.0, 0.0, f64::from(chain.width), f64::from(chain.height)],
        pivots,
        selection_boxes: boxes,
        selection_union: union,
        other_cameras: cameras,
        selection_display: display,
        current_on_top: aids.current_on_top,
    });
    out.frame = Some(chain.frame);
    out.camera = Some(Some(SceneCamera {
        element_id: chain.camera.as_str().to_owned(),
        size: [chain.width, chain.height],
        camera_to_view: chain.output_to_view,
    }));
    Ok(out)
}

/// The current drawing's selection for the scene (frame-loop note §7.1): `draw.selection.get`'s geometry.
///
/// # Errors
/// The selection walk's work-budget refusal.
pub(crate) fn selection(cx: &crate::cx::Cx) -> Result<Option<SceneSelection>, CommandError> {
    let Ok(doc) = cx.document() else { return Ok(None) };
    let Some(id) = cx.app().current_drawing(doc) else { return Ok(None) };
    let Some(d) = doc.drawing(&id) else { return Ok(None) };
    let g = crate::commands::draw_select::selection_geometry(cx, d)?;
    Ok(g.map(|g| SceneSelection {
        pivot: g.bounds.map(|b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0]),
        objects: g.objects,
        bounds: g.bounds,
        points: g.points,
    }))
}
