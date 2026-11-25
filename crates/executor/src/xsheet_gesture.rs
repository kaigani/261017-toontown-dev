//! The Exposure Sheet's gestures in the timing-gesture interpreter (M5 C5.2a; `reviews/M5-design-xsheet-view.md` §3;
//! rulings E-7, E-10): the sheet is a second surface family beside the Timeline's, frames down and columns across
//! (04 §12.1). The window calls the same interpreter at the press and the release ([`crate::timing_gesture::run`]), so
//! the press's snapshot and the release's comparison (E-10) apply as they do to the Timeline; this module gives the
//! sheet's geometry, what is under a press and the commands of a gesture (04 §12.5, §12.9):
//!
//! | Gesture | Command |
//! |---|---|
//! | Click a cell | `cell.select` of the cell, `xsheet.currentCell.set` (extend: the rectangle from the current cell) |
//! | Drag from a cell outside the selection | `cell.select` of the rectangle (frames × columns), `xsheet.currentCell.set` |
//! | Drag the selected cells | `cell.range.move` (move, copy or link by the modifiers; another column: `targetElementId`) |
//! | Drag an exposure's bottom edge | `cell.exposure.add` / `cell.exposure.remove` of the frames dragged (following cells shift) |
//! | Click a column header | `element.select`, `element.current.set` (extend and toggle as the Timeline's rows) |
//! | Drag a column header | `element.order.set {order: "xsheet"}` |
//!
//! The columns are derived from the document (the scene's cell-based elements in Exposure Sheet order, those shown in
//! the sheet) and their widths from the session, never sent by the window; the geometry's constants come from
//! `xsheet-layout.json`.

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::json;
use tt_model::Id;
use tt_model::document::Document;

use crate::app::AppContext;
use crate::timing_gesture::{GestureLayout, Plan, Sample, State, Target};

/// The sheet's layout constants.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XsheetLayout {
    /// File version.
    pub version: u32,
    /// Height of one frame's row.
    pub row_height: u32,
    /// Height of one frame's row with thumbnails shown (04 §12.3: rows become taller).
    pub row_height_thumbnails: u32,
    /// A column's width when it has none of its own.
    pub column_width: u32,
    /// How near an exposure's bottom edge a press grabs it.
    pub exposure_edge: u32,
}

impl Default for XsheetLayout {
    /// The values of `xsheet-layout.json` (used only if the bundled file failed to parse, which a test rules out).
    fn default() -> Self {
        Self { version: 1, row_height: 20, row_height_thumbnails: 52, column_width: 80, exposure_edge: 4 }
    }
}

/// The bundled layout.
#[must_use]
pub fn xsheet_layout() -> &'static XsheetLayout {
    static L: OnceLock<XsheetLayout> = OnceLock::new();
    L.get_or_init(|| serde_json::from_str(include_str!("xsheet-layout.json")).unwrap_or_default())
}

/// The bundled layout parsed strictly (a test checks the file).
///
/// # Errors
/// The parse error.
pub fn parse_bundled() -> Result<XsheetLayout, serde_json::Error> {
    serde_json::from_str(include_str!("xsheet-layout.json"))
}

/// One column the sheet shows: its element, left edge and width (unscrolled).
#[derive(Debug, Clone)]
pub(crate) struct Column {
    pub id: Id,
    pub x: f64,
    pub width: f64,
}

/// The columns of the scene, left to right.
pub(crate) fn columns(doc: &Document, scene: &Id, state: &State) -> Vec<Column> {
    let l = xsheet_layout();
    let Some(sc) = doc.scene(scene) else { return Vec::new() };
    let mut x = 0.0;
    let mut out = Vec::new();
    for e in sc.column_order() {
        let Some(el) = doc.element(e) else { continue };
        if !el.element_type().is_cell_based() || !el.visible_in_xsheet().unwrap_or(true) {
            continue;
        }
        let width = f64::from(state.column_widths.get(e.as_str()).copied().unwrap_or(l.column_width).max(1));
        out.push(Column { id: e.clone(), x, width });
        x += width;
    }
    out
}

/// The row height in force.
fn row_height(state: &State) -> f64 {
    let l = xsheet_layout();
    f64::from(if state.thumbnails { l.row_height_thumbnails } else { l.row_height }.max(1))
}

/// The frame under `y` (from the top of frame 1, scrolled).
#[must_use]
pub(crate) fn frame_at(state: &State, g: &GestureLayout, y: f64) -> u32 {
    let r = ((y + g.scroll_top) / row_height(state)).floor();
    let r = if r.is_finite() { r as i64 } else { 0 };
    u32::try_from(r.saturating_add(1).clamp(1, i64::from(u32::MAX))).unwrap_or(1)
}

/// The column under `x` (scrolled), `None` right of the last.
fn column_at(cols: &[Column], g: &GestureLayout, x: f64) -> Option<usize> {
    let x = x + g.scroll_left;
    cols.iter().position(|c| x >= c.x && x < c.x + c.width)
}

/// The column a drop at `x` lands before (`cols.len()`: after the last).
fn drop_index(cols: &[Column], g: &GestureLayout, x: f64) -> usize {
    let x = x + g.scroll_left;
    cols.iter().position(|c| x < c.x + c.width / 2.0).unwrap_or(cols.len())
}

/// The first and last frame of the exposure that holds `frame` in `element`, when the cell exposes a drawing.
fn exposure_of(doc: &Document, element: &Id, frame: u32) -> Option<(u32, u32)> {
    let d = AppContext::exposed_drawing(doc, element, frame)?;
    let same = |f: u32| AppContext::exposed_drawing(doc, element, f).as_ref() == Some(&d);
    let mut start = frame;
    while start > 1 && same(start - 1) {
        start -= 1;
    }
    let mut end = frame;
    while end < u32::MAX && same(end + 1) {
        end += 1;
    }
    Some((start, end))
}

/// What is under a press on the sheet's grid or headers.
pub(crate) fn classify(doc: &Document, state: &State, g: &GestureLayout, s: &Sample, headers: bool) -> Target {
    let Some(scene) = &state.scene else { return Target::Nothing };
    let cols = columns(doc, scene, state);
    let Some(c) = column_at(&cols, g, s.x) else { return Target::Nothing };
    let element = cols[c].id.as_str().to_owned();
    if headers {
        return Target::Header { column: c, element };
    }
    let frame = frame_at(state, g, s.y);
    let rh = row_height(state);
    let in_row = (s.y + g.scroll_top) - f64::from(frame - 1) * rh;
    if rh - in_row <= f64::from(xsheet_layout().exposure_edge)
        && let Some((_, end)) = exposure_of(doc, &cols[c].id, frame)
        && end == frame
    {
        return Target::ExposureEdge { column: c, element, frame };
    }
    let selected =
        state.cells.as_ref().is_some_and(|(els, a, b)| els.contains(&cols[c].id) && (*a..=*b).contains(&frame));
    Target::SheetCell { column: c, element, frame, selected }
}

fn ids_json(ids: &[Id]) -> serde_json::Value {
    json!(ids.iter().map(Id::as_str).collect::<Vec<_>>())
}

fn one(command: &str, params: serde_json::Value) -> Plan {
    Plan::Commands { commands: vec![(command.to_owned(), params)] }
}

/// The commands of a sheet gesture (the module's table).
pub(crate) fn plan(
    doc: &Document,
    state: &State,
    g: &GestureLayout,
    samples: &[Sample],
    target: Target,
    dragged: bool,
    mods: &crate::timeline_layout::Modifiers,
) -> Plan {
    let (Some(first), Some(last)) = (samples.first(), samples.last()) else { return Plan::Nothing };
    let Some(scene) = &state.scene else { return Plan::Nothing };
    let cols = columns(doc, scene, state);
    let held = |role: &[String], s: &Sample| {
        !role.is_empty() && role.iter().all(|m| s.modifiers.iter().any(|x| x.eq_ignore_ascii_case(m)))
    };
    match target {
        Target::SheetCell { column, frame, .. } if !dragged => click(state, &cols, column, frame, first, mods),
        Target::ExposureEdge { column, frame, .. } if !dragged => click(state, &cols, column, frame, first, mods),
        Target::SheetCell { column, frame, selected: false, .. } => {
            let c1 = column_at(&cols, g, last.x).unwrap_or(column);
            let f1 = frame_at(state, g, last.y);
            let (a, b) = (column.min(c1), column.max(c1));
            let ids: Vec<Id> = cols[a..=b].iter().map(|c| c.id.clone()).collect();
            Plan::Commands {
                commands: vec![
                    (
                        "cell.select".to_owned(),
                        json!({ "elementIds": ids_json(&ids), "from": frame.min(f1), "to": frame.max(f1) }),
                    ),
                    (
                        "xsheet.currentCell.set".to_owned(),
                        json!({ "elementId": cols[column].id.as_str(), "frame": frame }),
                    ),
                ],
            }
        }
        Target::SheetCell { column, frame, selected: true, .. } => {
            let Some((els, from, to)) = &state.cells else { return Plan::Nothing };
            let delta = i64::from(frame_at(state, g, last.y)) - i64::from(frame);
            let delta = delta.max(1 - i64::from(*from));
            let as_i64 = |r: usize| i64::try_from(r).unwrap_or(i64::MAX);
            let dcols = column_at(&cols, g, last.x).map_or(0, |c| as_i64(c).saturating_sub(as_i64(column)));
            if delta == 0 && dcols == 0 {
                return click(state, &cols, column, frame, first, mods);
            }
            let mode = if held(&mods.link, last) {
                "link"
            } else if held(&mods.copy, last) {
                "copy"
            } else {
                "move"
            };
            let mut params = json!({
                "sourceElementIds": ids_json(els),
                "from": from,
                "to": to,
                "targetFrame": i64::from(*from) + delta,
                "mode": mode,
                "options": serde_json::Value::Object(state.drag_options.clone()),
            });
            if dcols != 0 {
                let base = els.first().and_then(|e| cols.iter().position(|c| &c.id == e)).unwrap_or(column);
                let t = as_i64(base).saturating_add(dcols);
                let Some(target) = usize::try_from(t).ok().and_then(|t| cols.get(t)) else { return Plan::Nothing };
                params["targetElementId"] = json!(target.id.as_str());
            }
            one("cell.range.move", params)
        }
        Target::ExposureEdge { column, frame, .. } => {
            let Some((start, end)) = exposure_of(doc, &cols[column].id, frame) else { return Plan::Nothing };
            let delta = i64::from(frame_at(state, g, last.y)) - i64::from(end);
            let id = cols[column].id.as_str();
            if delta > 0 {
                one(
                    "cell.exposure.add",
                    json!({ "elementIds": [id], "from": end, "to": end, "frames": delta.min(9999) }),
                )
            } else if delta < 0 {
                // Shortened to one frame at most (the exposure is not removed by its edge).
                let frames = (-delta).min(i64::from(end - start));
                if frames == 0 {
                    return Plan::Nothing;
                }
                one("cell.exposure.remove", json!({ "elementIds": [id], "from": end, "to": end, "frames": frames }))
            } else {
                Plan::Nothing
            }
        }
        Target::Header { column, .. } if !dragged => header_click(state, &cols, column, first, mods, &held),
        Target::Header { column, .. } => {
            let at = drop_index(&cols, g, last.x);
            if at == column || at == column + 1 {
                return Plan::Nothing;
            }
            let Some(sc) = doc.scene(scene) else { return Plan::Nothing };
            // The complete order (hidden columns kept in place): the dragged column goes before the shown column at the
            // drop, or after the last shown one.
            let dragged_id = cols[column].id.clone();
            let mut order: Vec<Id> = sc.column_order().iter().filter(|e| **e != dragged_id).cloned().collect();
            let at = match cols.get(at) {
                Some(c) => order.iter().position(|e| *e == c.id).unwrap_or(order.len()),
                None => cols.last().and_then(|c| order.iter().position(|e| *e == c.id)).map_or(order.len(), |p| p + 1),
            };
            order.insert(at, dragged_id);
            one("element.order.set", json!({ "order": "xsheet", "elementIds": ids_json(&order) }))
        }
        _ => Plan::Nothing,
    }
}

/// A click on a cell (04 §12.5): the cell, made current; extend: the rectangle from the current cell.
fn click(
    state: &State,
    cols: &[Column],
    column: usize,
    frame: u32,
    s: &Sample,
    mods: &crate::timeline_layout::Modifiers,
) -> Plan {
    let id = &cols[column].id;
    let extend =
        !mods.extend.is_empty() && mods.extend.iter().all(|m| s.modifiers.iter().any(|x| x.eq_ignore_ascii_case(m)));
    if extend && let Some(anchor) = state.current_element.as_ref().and_then(|c| cols.iter().position(|x| &x.id == c)) {
        let ids: Vec<Id> = cols[anchor.min(column)..=anchor.max(column)].iter().map(|c| c.id.clone()).collect();
        let f0 = state.current_frame.max(1);
        return one("cell.select", json!({ "elementIds": ids_json(&ids), "from": f0.min(frame), "to": f0.max(frame) }));
    }
    Plan::Commands {
        commands: vec![
            ("cell.select".to_owned(), json!({ "elementIds": [id.as_str()], "from": frame, "to": frame })),
            ("xsheet.currentCell.set".to_owned(), json!({ "elementId": id.as_str(), "frame": frame })),
        ],
    }
}

/// A click on a column header (04 §12.5): the element (whole column), as a click on a Timeline row.
fn header_click(
    state: &State,
    cols: &[Column],
    column: usize,
    s: &Sample,
    mods: &crate::timeline_layout::Modifiers,
    held: &dyn Fn(&[String], &Sample) -> bool,
) -> Plan {
    let id = &cols[column].id;
    if held(&mods.toggle, s) && !held(&mods.extend, s) {
        return one("element.select", json!({ "elementIds": [id.as_str()], "mode": "toggle" }));
    }
    let ids: Vec<Id> = match (
        held(&mods.extend, s),
        state.current_element.as_ref().and_then(|c| cols.iter().position(|x| &x.id == c)),
    ) {
        (true, Some(anchor)) => cols[anchor.min(column)..=anchor.max(column)].iter().map(|c| c.id.clone()).collect(),
        _ => vec![id.clone()],
    };
    Plan::Commands {
        commands: vec![
            ("element.select".to_owned(), json!({ "elementIds": ids_json(&ids), "mode": "replace" })),
            ("element.current.set".to_owned(), json!({ "elementId": id.as_str() })),
        ],
    }
}
