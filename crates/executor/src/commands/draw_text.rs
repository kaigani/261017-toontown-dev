//! `draw.text.add`, `draw.text.set`, `draw.textFormat.set` (02 §3.7, §7.6, §13.5): text objects with
//! per-character runs and per-paragraph formatting, laid out by `tt-vector` for their bounds. A missing font keeps its
//! name and is reported (`drawing.fontMissing`, 02 §3.7); `doc.validate` lists missing fonts (`missingFonts`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::ObjectType;
use tt_model::document::EntryKey;
use tt_model::entities::drawing::{DrawingNode, WidthMode};
use tt_registry::{CommandError, codes, command};
use tt_vector::drawing::{self as vd, ObjectKind};
use tt_vector::geom::Affine;
use tt_vector::text::{self, Content, Format};

use super::draw::{Bounds, Point, check_editable, committed_rev, err, op, target};
use super::draw_stroke::{colour_of, stroke_target};
use crate::cx::Cx;

/// Baseline (`script` in the project format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Baseline {
    /// Normal.
    Normal,
    /// Subscript (60 % size).
    Subscript,
    /// Superscript (60 % size).
    Superscript,
}

impl Baseline {
    fn as_str(self) -> &'static str {
        match self {
            Baseline::Normal => "normal",
            Baseline::Subscript => "subscript",
            Baseline::Superscript => "superscript",
        }
    }
}

/// Paragraph alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Align {
    /// Left.
    Left,
    /// Centre.
    Centre,
    /// Right.
    Right,
    /// Justify.
    Justify,
}

impl Align {
    fn as_str(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Centre => "centre",
            Align::Right => "right",
            Align::Justify => "justify",
        }
    }
}

/// The Text panel fields (02 §7.6.1; SI-026 `format`).
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextFormat {
    /// Font family (kept when missing).
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    font: Option<String>,
    /// Size (du).
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 512.0))]
    size: Option<f64>,
    /// Bold.
    #[serde(default)]
    bold: Option<bool>,
    /// Italic.
    #[serde(default)]
    italic: Option<bool>,
    /// Baseline.
    #[serde(default)]
    baseline: Option<Baseline>,
    /// Manual kerning (1/100 em).
    #[serde(default)]
    #[schemars(range(min = -60, max = 60))]
    kerning: Option<i32>,
    /// Font kerning pairs.
    #[serde(default)]
    auto_kern: Option<bool>,
    /// Alignment.
    #[serde(default)]
    align: Option<Align>,
    /// Left margin (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1000.0))]
    left_margin: Option<f64>,
    /// Right margin (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1000.0))]
    right_margin: Option<f64>,
    /// First-line indent (du).
    #[serde(default)]
    #[schemars(range(min = -99.0, max = 1000.0))]
    indent: Option<f64>,
    /// Additional line spacing (du).
    #[serde(default)]
    #[schemars(range(min = -99.0, max = 1000.0))]
    line_spacing: Option<f64>,
    /// Outline colour: only `null` (10 §4.9 runs store no outline colour; SI-055).
    #[serde(default)]
    outline_colour_id: Option<String>,
}

fn format_of(f: &TextFormat, colour: Option<Option<String>>, pointer: &str) -> Result<Format, CommandError> {
    if f.outline_colour_id.is_some() {
        return Err(err(
            codes::ARG_INVALID,
            "outline colours are not stored by the project format (10 §4.9; SI-055)",
            json!({ "pointer": format!("{pointer}/outlineColourId") }),
        ));
    }
    Ok(Format {
        font: f.font.clone(),
        size: f.size,
        bold: f.bold,
        italic: f.italic,
        baseline: f.baseline.map(|b| b.as_str().to_owned()),
        kerning: f.kerning.map(f64::from),
        auto_kern: f.auto_kern,
        colour_id: colour,
        align: f.align.map(|a| a.as_str().to_owned()),
        left_margin: f.left_margin,
        right_margin: f.right_margin,
        indent: f.indent,
        line_spacing: f.line_spacing,
    })
}

fn check_len(t: &str, pointer: &str) -> Result<(), CommandError> {
    if t.chars().count() > text::MAX_TEXT {
        return Err(err(
            codes::ARG_INVALID,
            format!("a text object holds at most {} characters", text::MAX_TEXT),
            json!({ "pointer": pointer }),
        ));
    }
    Ok(())
}

fn check_position(p: Point, pointer: &str) -> Result<(), CommandError> {
    for (v, k) in [(p.x, "x"), (p.y, "y")] {
        tt_vector::path::check_coord(v, &format!("{pointer}/{k}")).map_err(|e| super::draw::invalid_geometry(&e))?;
    }
    Ok(())
}

fn check_width(w: f64, pointer: &str) -> Result<(), CommandError> {
    if !(w.is_finite() && w > 0.0 && w <= tt_vector::path::MAX_COORD) {
        return Err(err(codes::ARG_INVALID, "the width is a positive number of du", json!({ "pointer": pointer })));
    }
    Ok(())
}

/// The persisted text node JSON.
fn node_json(id: &str, transform: [f64; 6], width: Option<f64>, c: &Content) -> Value {
    let mut v = json!({
        "id": id, "type": "drawingNode", "rev": 0, "kind": "text", "transform": transform,
        "widthMode": if width.is_some() { "fixed" } else { "auto" },
        "text": c.text(), "runs": c.runs, "paragraphs": c.paras,
    });
    if let Some(w) = width {
        v["width"] = json!(w);
    }
    v
}

fn bounds_and_fonts(cx: &mut Cx, c: &Content, width: Option<f64>, transform: [f64; 6], object: &str) -> Bounds {
    let l = text::layout(c, width);
    for f in &l.missing_fonts {
        cx.warn(
            "drawing.fontMissing",
            format!("font `{f}` is not installed; the text keeps the name and is displayed with a fallback font"),
            vec![object.to_owned()],
        );
    }
    text::bounds(&l, &Affine(transform)).into()
}

/// `draw.text.add` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextAddParams {
    /// The drawing (default: `elementId` + `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell receives the text.
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 2_147_483_647))]
    frame: Option<u32>,
    /// Create a drawing in an empty addressed cell.
    #[serde(default = "yes")]
    create_drawing: bool,
    /// Top-left corner of the text box (du).
    position: Point,
    /// The characters (`\n` starts a paragraph).
    #[schemars(length(max = 100_000))]
    text: String,
    /// Fixed width with word wrap (du); omitted = auto width.
    #[serde(default)]
    width: Option<f64>,
    /// Text panel fields (default: the Text panel defaults, 02 §7.6.1).
    #[serde(default)]
    format: Option<TextFormat>,
    /// Text colour (default: the current colour).
    #[serde(default)]
    colour_id: Option<String>,
}

fn yes() -> bool {
    true
}

/// `{textId, bounds, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextResult {
    /// The drawing.
    drawing_id: String,
    /// The text object.
    text_id: String,
    /// Its bounds in drawing space (laid out with the installed fonts).
    bounds: Bounds,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.text.add",
    kind = action,
    label = "Add text",
    scopes = [edit],
    undo = "New Text Object",
    needs_project,
    example(title = "A caption above Bob", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "position": {"x": -100, "y": 300}, "text": "Hello\nworld", "format": {"size": 48, "bold": true, "align": "centre"}}"#),
    example(title = "Fixed-width wrapped text", params = r#"{"elementId": "scene:#1/element:\"Bob\"", "frame": 1, "position": {"x": -200, "y": 0}, "text": "A longer note that wraps inside its box", "width": 300}"#),
    gui = "Drawing view: Text tool (M4 C4.2)",
)]
pub(crate) fn draw_text_add(cx: &mut Cx, p: TextAddParams) -> Result<TextResult, CommandError> {
    check_position(p.position, "/position")?;
    check_len(&p.text, "/text")?;
    if let Some(w) = p.width {
        check_width(w, "/width")?;
    }
    let mut changes = Vec::new();
    let t = target(cx, p.drawing_id.as_deref(), p.element_id.as_deref(), p.frame, p.create_drawing, &mut changes)?;
    let d = t.drawing(cx.document()?)?;
    check_editable(&d, None)?;
    let colour = colour_of(cx, p.colour_id.as_deref(), None, "/colourId")?;
    let mut c = Content::new(&p.text, text::default_run(colour), text::default_para());
    if let Some(f) = &p.format {
        let fmt = format_of(f, None, "/format")?;
        c.format(0, c.chars.len(), &fmt);
        if c.chars.is_empty() {
            c.format(0, 0, &fmt);
        }
    }
    let id = cx.new_id(ObjectType::DrawingNode)?;
    let transform = [1.0, 0.0, 0.0, 1.0, p.position.x, p.position.y];
    let node = node_json(id.as_str(), transform, p.width, &c);
    if t.created {
        if let Some((_, first)) = changes.iter_mut().find(|(k, _)| *k == EntryKey::Drawing(t.id.clone()))
            && let Some(o) = first.first_mut()
            && let Some(nodes) = o.value.as_mut().and_then(|v| v.get_mut("nodes")).and_then(Value::as_array_mut)
        {
            nodes.push(node);
        }
    } else {
        changes.push((EntryKey::Drawing(t.id.clone()), vec![op("add", "/nodes/-", Some(node))]));
    }
    cx.patch_entries(changes)?;
    let object = format!("{}/{}", t.id.as_str(), id.as_str());
    let bounds = bounds_and_fonts(cx, &c, p.width, transform, &object);
    Ok(TextResult {
        drawing_id: t.id.as_str().to_owned(),
        text_id: id.as_str().to_owned(),
        bounds,
        rev: committed_rev(cx)?,
    })
}

/// `{at, text}` of an insertion.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Insert {
    /// Code-point position.
    #[schemars(range(min = 0, max = 100_000))]
    at: u32,
    /// The characters.
    #[schemars(length(max = 100_000))]
    text: String,
}

/// `{start, length}` of a deletion.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Delete {
    /// First code point.
    #[schemars(range(min = 0, max = 100_000))]
    start: u32,
    /// Number of code points.
    #[schemars(range(min = 0, max = 100_000))]
    length: u32,
}

/// Width mode parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WidthModeParam {
    /// No wrapping.
    Auto,
    /// Word wrap at `width`.
    Fixed,
}

/// `draw.text.set` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextSetParams {
    /// The text object (`<drawingId>/<localId>`, or local with `drawingId` / the current drawing).
    text_id: String,
    /// The drawing holding a local ID.
    #[serde(default)]
    drawing_id: Option<String>,
    /// Replace the whole text (the first run's and paragraph's format is kept).
    #[serde(default)]
    #[schemars(length(max = 100_000))]
    text: Option<String>,
    /// Insert characters.
    #[serde(default)]
    insert: Option<Insert>,
    /// Delete characters.
    #[serde(default)]
    delete: Option<Delete>,
    /// Auto or fixed width.
    #[serde(default)]
    width_mode: Option<WidthModeParam>,
    /// Fixed width (du); switches to fixed width (02 §7.6).
    #[serde(default)]
    width: Option<f64>,
    /// New top-left corner (du).
    #[serde(default)]
    position: Option<Point>,
    /// The drawing revision the caller read (02 §3.12).
    #[serde(default)]
    expect_rev: Option<u64>,
}

fn text_node(
    cx: &Cx,
    text_id: &str,
    drawing_id: Option<&str>,
) -> Result<(tt_model::entities::drawing::Drawing, vd::Located, DrawingNode), CommandError> {
    let (d, l) = stroke_target(cx, text_id, drawing_id).map_err(|mut e| {
        if let Some(m) = e.details.as_object_mut()
            && m.contains_key("pointer")
        {
            m.insert("pointer".into(), json!("/textId"));
        }
        e
    })?;
    if l.kind != ObjectKind::Text {
        return Err(err(
            codes::ARG_INVALID,
            format!("`{text_id}` is a {}, not a text object", l.kind.as_str()),
            json!({ "pointer": "/textId" }),
        ));
    }
    let n = vd::node_at(&d, &l.pointer).cloned().ok_or_else(|| CommandError::internal("text node vanished"))?;
    Ok((d, l, n))
}

#[command(
    id = "draw.text.set",
    kind = action,
    label = "Edit text",
    scopes = [edit],
    undo = "Typing",
    needs_project,
    example(title = "Type at the end of the first drawing's caption", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "textId": "nd_01K742SG0WFAQJFKAZKV4CD56C", "insert": {"at": 5, "text": "!"}}"#),
    gui = "Drawing view: Text tool typing, box width and move (M4 C4.2)",
)]
pub(crate) fn draw_text_set(cx: &mut Cx, p: TextSetParams) -> Result<TextResult, CommandError> {
    let content_edit = p.text.is_some() || p.insert.is_some() || p.delete.is_some();
    let width_edit = p.width_mode.is_some() || p.width.is_some();
    if !content_edit && !width_edit && p.position.is_none() {
        return Err(err(
            codes::ARG_MISSING,
            "give text, insert, delete, widthMode, width or position",
            json!({ "pointer": "/text" }),
        ));
    }
    if p.text.is_some() && (p.insert.is_some() || p.delete.is_some()) {
        return Err(err(
            codes::ARG_INVALID,
            "text replaces everything: do not combine it with insert or delete",
            json!({ "pointer": "/text" }),
        ));
    }
    let (d, l, n) = text_node(cx, &p.text_id, p.drawing_id.as_deref())?;
    check_editable(&d, p.expect_rev)?;
    let mut c = vd::text_content(&n).ok_or_else(|| CommandError::internal("text node without content"))?;
    if let Some(t) = &p.text {
        check_len(t, "/text")?;
        let run = c.runs.first().cloned().unwrap_or_else(|| text::default_run(None));
        let para = c.paras.first().cloned().unwrap_or_else(text::default_para);
        c = Content::new(t, run, para);
    }
    if let Some(del) = &p.delete {
        let len = c.chars.len();
        if del.start as usize > len || (del.start + del.length) as usize > len {
            return Err(err(
                codes::ARG_INVALID,
                format!("the text has {len} characters"),
                json!({ "pointer": "/delete", "allowed": { "maximum": len } }),
            ));
        }
        c.delete(del.start as usize, del.length as usize);
    }
    if let Some(ins) = &p.insert {
        if ins.at as usize > c.chars.len() {
            return Err(err(
                codes::ARG_INVALID,
                format!("the text has {} characters", c.chars.len()),
                json!({ "pointer": "/insert/at" }),
            ));
        }
        c.insert(ins.at as usize, &ins.text);
    }
    check_len(&c.text(), "/insert/text")?;
    let mut width = if *n.width_mode() == Some(WidthMode::Fixed) { *n.width() } else { None };
    match (p.width_mode, p.width) {
        (Some(WidthModeParam::Auto), Some(_)) => {
            return Err(err(codes::ARG_INVALID, "auto width has no width", json!({ "pointer": "/width" })));
        }
        (Some(WidthModeParam::Auto), None) => width = None,
        (_, Some(w)) => {
            check_width(w, "/width")?;
            width = Some(w);
        }
        (Some(WidthModeParam::Fixed), None) => {
            if width.is_none() {
                let l = text::layout(&c, None);
                width = Some(l.width.max(1.0));
            }
        }
        (None, None) => {}
    }
    let mut transform = n.transform().unwrap_or(tt_model::value::IDENTITY);
    if let Some(pos) = p.position {
        check_position(pos, "/position")?;
        transform[4] = pos.x;
        transform[5] = pos.y;
    }
    let node = node_json(n.id().as_str(), transform, width, &c);
    let raw = cx.document()?.entry_raw_json(&EntryKey::Drawing(d.id().clone()))?;
    let mut merged = raw.pointer(&l.pointer).cloned().unwrap_or(Value::Null);
    // Keep the envelope (name, meta, ext) and replace the text fields.
    if let (Some(m), Some(src)) = (merged.as_object_mut(), node.as_object()) {
        for k in ["transform", "widthMode", "text", "runs", "paragraphs"] {
            if let Some(v) = src.get(k) {
                m.insert(k.into(), v.clone());
            }
        }
        match src.get("width") {
            Some(w) => {
                m.insert("width".into(), w.clone());
            }
            None => {
                m.remove("width");
            }
        }
    }
    cx.patch_entries(vec![(EntryKey::Drawing(d.id().clone()), vec![op("replace", &l.pointer, Some(merged))])])?;
    cx.set_label(if content_edit {
        "Typing"
    } else if width_edit {
        "Set Text Width"
    } else {
        "Move Text Object"
    });
    let object = format!("{}/{}", d.id().as_str(), l.id);
    let bounds = bounds_and_fonts(cx, &c, width, transform, &object);
    Ok(TextResult { drawing_id: d.id().as_str().to_owned(), text_id: l.id, bounds, rev: committed_rev(cx)? })
}

/// `draw.textFormat.set` parameters (02 §13.5).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TextFormatParams {
    /// The text object.
    text_id: String,
    /// The drawing holding a local ID.
    #[serde(default)]
    drawing_id: Option<String>,
    /// First code point.
    #[serde(default)]
    #[schemars(range(min = 0, max = 100_000))]
    start: u32,
    /// Number of code points (default: to the end).
    #[serde(default)]
    #[schemars(range(min = 0, max = 100_000))]
    length: Option<u32>,
    /// Font family.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    font: Option<String>,
    /// Size (du).
    #[serde(default)]
    #[schemars(range(min = 1.0, max = 512.0))]
    size: Option<f64>,
    /// Bold.
    #[serde(default)]
    bold: Option<bool>,
    /// Italic.
    #[serde(default)]
    italic: Option<bool>,
    /// Baseline.
    #[serde(default)]
    baseline: Option<Baseline>,
    /// Manual kerning (1/100 em).
    #[serde(default)]
    #[schemars(range(min = -60, max = 60))]
    kerning: Option<i32>,
    /// Font kerning pairs.
    #[serde(default)]
    auto_kern: Option<bool>,
    /// Alignment.
    #[serde(default)]
    align: Option<Align>,
    /// Left margin (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1000.0))]
    left_margin: Option<f64>,
    /// Right margin (du).
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1000.0))]
    right_margin: Option<f64>,
    /// First-line indent (du).
    #[serde(default)]
    #[schemars(range(min = -99.0, max = 1000.0))]
    indent: Option<f64>,
    /// Additional line spacing (du).
    #[serde(default)]
    #[schemars(range(min = -99.0, max = 1000.0))]
    line_spacing: Option<f64>,
    /// Fill colour.
    #[serde(default)]
    colour_id: Option<String>,
    /// Outline colour: only `null` (SI-055).
    #[serde(default)]
    outline_colour_id: Option<String>,
    /// The drawing revision the caller read (02 §3.12).
    #[serde(default)]
    expect_rev: Option<u64>,
}

/// `{textId, rev}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextFormatResult {
    /// The text object.
    text_id: String,
    /// Its bounds after the change.
    bounds: Bounds,
    /// The drawing revision after the change.
    rev: u64,
}

#[command(
    id = "draw.textFormat.set",
    kind = action,
    label = "Format text",
    scopes = [edit],
    undo = "Text Property Modified",
    needs_project,
    example(title = "Make the first two characters bold and larger", params = r#"{"drawingId": "drawingBank:#1/drawing:#1", "textId": "nd_01K742SG0WFAQJFKAZKV4CD56C", "start": 0, "length": 2, "bold": true, "size": 30}"#),
    gui = "Text panel (M4 C4.2)",
)]
pub(crate) fn draw_text_format_set(cx: &mut Cx, p: TextFormatParams) -> Result<TextFormatResult, CommandError> {
    let (d, l, n) = text_node(cx, &p.text_id, p.drawing_id.as_deref())?;
    check_editable(&d, p.expect_rev)?;
    let colour = match &p.colour_id {
        Some(c) => Some(colour_of(cx, Some(c), None, "/colourId")?),
        None => None,
    };
    let f = TextFormat {
        font: p.font.clone(),
        size: p.size,
        bold: p.bold,
        italic: p.italic,
        baseline: p.baseline,
        kerning: p.kerning,
        auto_kern: p.auto_kern,
        align: p.align,
        left_margin: p.left_margin,
        right_margin: p.right_margin,
        indent: p.indent,
        line_spacing: p.line_spacing,
        outline_colour_id: p.outline_colour_id.clone(),
    };
    let fmt = format_of(&f, colour, "")?;
    if fmt == Format::default() {
        return Err(err(codes::ARG_MISSING, "give at least one format field", json!({ "pointer": "/font" })));
    }
    let mut c = vd::text_content(&n).ok_or_else(|| CommandError::internal("text node without content"))?;
    let len = c.chars.len();
    if p.start as usize > len {
        return Err(err(codes::ARG_INVALID, format!("the text has {len} characters"), json!({ "pointer": "/start" })));
    }
    let count = p.length.map_or(len - p.start as usize, |l| l as usize);
    if p.start as usize + count > len {
        return Err(err(codes::ARG_INVALID, format!("the text has {len} characters"), json!({ "pointer": "/length" })));
    }
    c.format(p.start as usize, count, &fmt);
    let raw = cx.document()?.entry_raw_json(&EntryKey::Drawing(d.id().clone()))?;
    let base = l.pointer.clone();
    let mut ops = Vec::new();
    if raw.pointer(&format!("{base}/runs")).is_some() {
        ops.push(op("replace", &format!("{base}/runs"), Some(json!(c.runs))));
        ops.push(op("replace", &format!("{base}/paragraphs"), Some(json!(c.paras))));
    }
    cx.patch_entries(vec![(EntryKey::Drawing(d.id().clone()), ops)])?;
    let width = if *n.width_mode() == Some(WidthMode::Fixed) { *n.width() } else { None };
    let transform = n.transform().unwrap_or(tt_model::value::IDENTITY);
    let object = format!("{}/{}", d.id().as_str(), l.id);
    let bounds = bounds_and_fonts(cx, &c, width, transform, &object);
    Ok(TextFormatResult { text_id: l.id, bounds, rev: committed_rev(cx)? })
}
