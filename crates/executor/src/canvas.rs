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

/// The canvas form of one drawing: the final renderer's walk, colour slots, tessellated records (design note 1 §8).
///
/// # Errors
/// `ref.notFound` for a drawing not in the snapshot; the walk's refusals (`limit.exceeded`, planar-map errors).
pub(crate) fn build(
    src: &CanvasSource,
    generation: u64,
    bucket: i16,
) -> Result<(CanvasDrawing, Vec<Value>), CommandError> {
    let d = src.doc.drawing(&src.drawing).ok_or_else(|| not_found(src.drawing.as_str()))?;
    let never = || false;
    let sources = Sources { cache: &src.cache, blobs: src.blobs.as_ref(), selection: None, stop: &never };
    let (items, tags, warnings) = canvas_items(&sources, &src.doc, d, None)?;
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
        out.push(CanvasItem { item, slot, bitmap });
    }
    let records = canvas::tessellate(&out, bucket);
    let bounds = canvas::bounds(&records);
    let ulid = src.drawing.as_str().strip_prefix("dr_").unwrap_or(src.drawing.as_str()).to_owned();
    let drawing = CanvasDrawing {
        identity: Identity { generation, rev: *d.rev(), drawing_id: ulid, bucket },
        flags: 0,
        slots,
        records,
        bounds,
    };
    Ok((drawing, warnings))
}

/// [`build`] then [`canvas::encode`].
///
/// # Errors
/// As [`build`]; `internal` when the drawing does not fit the format.
pub(crate) fn payload(src: &CanvasSource, generation: u64, bucket: i16) -> Result<CanvasPayload, CommandError> {
    let (drawing, warnings) = build(src, generation, bucket)?;
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

/// The Drawing view's composition (design note 1 §9): what `canvas_scene` answers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    /// The view (`drawingView` or a float of it).
    pub view_id: String,
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
    if let Some(id) = s.app.current_drawing(doc)
        && let Some(d) = doc.drawing(&id)
    {
        entries.push(SceneEntry {
            drawing_id: id.as_str().to_owned(),
            rev: *d.rev(),
            role: "current".into(),
            opacity: 1.0,
            tint: None,
            shade: false,
            outline: false,
        });
    }
    let mut colours = std::collections::BTreeMap::new();
    let mut stamp: u64 = 0;
    for pal in doc.palettes() {
        let first = pal.styles().first().map(|st| st.id().clone());
        for st in pal.styles() {
            stamp = stamp.wrapping_mul(31).wrapping_add(*st.rev());
        }
        for sw in pal.swatches() {
            stamp = stamp.wrapping_mul(31).wrapping_add(*sw.rev());
            if let Some(tt_model::entities::palette::SwatchValue::Solid(c)) =
                first.as_ref().and_then(|f| sw.values().get(f))
            {
                colours.insert(sw.colour_id().as_str().to_owned(), c.rgba.channels());
            }
        }
        stamp = stamp.wrapping_mul(31).wrapping_add(pal.swatches().len() as u64);
    }
    Ok(Scene {
        view_id: view_id.to_owned(),
        background,
        grid: SceneGrid { visible: aids.grid.visible, ty: aids.grid.ty.clone(), overlay: aids.grid.overlay },
        entries,
        colours,
        palette_stamp: stamp,
    })
}
