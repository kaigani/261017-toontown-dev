//! The timing-gesture interpreter (M5 C5.1a; `reviews/M5-design-timeline-kernel.md` §5; rulings E-7, E-10): every
//! press, drag and release in the Timeline's layer list, tracks and frame ruler is turned into its structured command
//! (04 §11.6, §11.8, §11.10, §11.13; §17.1 item 5), as `gesture.rs` does for the views.
//!
//! Product code under the usual rules: the window calls it through the host at the press and at the release
//! ([`run`]), and the headless replay calls the same [`plan`] with the window's recorded samples and layout (E-7). The
//! layout constants come from `timeline-layout.json` ([`crate::timeline_layout`]); the visible rows are derived from
//! the document, never sent by the window.
//!
//! **A change under the drag (E-10, E-23).** The press keeps a snapshot of the document and of the selection state. At
//! the release the plan is computed twice, against the snapshot (what the preview showed) and against the present
//! state, and each is resolved to its outcome in absolute values ([`outcome`]); the present plan is committed only when
//! the two outcomes are equal, otherwise the gesture is cancelled with `timeline.targetChanged`, which the window shows
//! on the status line. Escape, a lost capture or an engine restart
//! abort the grab: nothing is sent (01 line 495).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::hierarchy as h;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::ElementType;
use tt_registry::CommandError;

use crate::commands::timeline::{RangeMode, effective_range, length_of};
use crate::timeline_layout::{Layout, layout};

/// The surface a gesture is made on: the Timeline's, or the Exposure Sheet's (M5 C5.2a, [`crate::xsheet_gesture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    /// The layer list (row names).
    Layers,
    /// The tracks.
    Tracks,
    /// The frame ruler.
    Ruler,
    /// The Exposure Sheet's grid of cells (frames down, columns across).
    XsheetGrid,
    /// The Exposure Sheet's column headers.
    XsheetHeaders,
}

/// A gesture's layout, captured at the press (design note §5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GestureLayout {
    /// The surface.
    pub surface: Surface,
    /// Zoom level (an index into the ladder).
    pub zoom_level: u32,
    /// The first frame shown.
    pub first_frame: u32,
    /// The rows' vertical scroll in pixels (the sheet: the frames').
    #[serde(default)]
    pub scroll_top: f64,
    /// The sheet's horizontal scroll in pixels (its columns; the Timeline scrolls by `first_frame`).
    #[serde(default)]
    pub scroll_left: f64,
    /// The sheet's static light-table panel, where it is shown (04 §15.2: cells dragged onto it are sent to it), in
    /// the surface's own pixels as the samples are (not scrolled with the grid).
    #[serde(default)]
    pub light_table: Option<DropRect>,
    /// `windows`, `macos` or `linux` (the modifier roles, SI-211).
    pub platform: String,
}

/// A drop target's rectangle in a surface's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DropRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl DropRect {
    /// Whether a point is inside.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// One pointer sample in the surface's own pixels (tracks and layers: y from the top of the first row; ruler: y within
/// the ruler), with the modifiers held.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
    /// `shift`, `alt`, `ctrl`, `meta`.
    #[serde(default)]
    pub modifiers: Vec<String>,
}

/// The selection state a gesture reads (from the session at the press and at the release).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// The current scene.
    pub scene: Option<Id>,
    /// Rows selected in the Timeline.
    pub rows: Vec<Id>,
    /// The cell selection: elements, first and last frame.
    pub cells: Option<(Vec<Id>, u32, u32)>,
    /// The current element and frame.
    pub current_element: Option<Id>,
    pub current_frame: u32,
    /// The single-cell selection preference (04 line 687).
    pub single_cell: bool,
    /// The current scene's effective playback range.
    pub range: (u32, u32),
    pub range_free: bool,
    /// The effect-parameter setting (ruling E-40): per effect element, and the default for every effect.
    pub parameter_rows: BTreeMap<String, bool>,
    pub parameter_rows_hidden: bool,
    /// The drag-and-drop option set in effect (04's names): a cell drag carries the set resolved at its press, inline
    /// in its `cell.range.move` (ruling E-32).
    pub drag_options: serde_json::Map<String, Value>,
    /// The Exposure Sheet's column widths by element (M5 C5.2a: its geometry).
    pub column_widths: BTreeMap<String, u32>,
    /// The Exposure Sheet shows thumbnails (taller rows, 04 §12.3).
    pub thumbnails: bool,
}

impl State {
    /// The state of a session.
    pub(crate) fn of(doc: &Document, s: &crate::session::Session) -> Self {
        let sel = crate::commands::timing::cell_selection_state(&s.app);
        let ids = |v: &Value| -> Vec<Id> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().and_then(|k| Id::parse(k).ok())).collect())
                .unwrap_or_default()
        };
        let elements = ids(&sel["elementIds"]);
        let rows_selected = sel["rowsSelected"].as_u64().unwrap_or(0) > 0;
        let from = sel["from"].as_u64().and_then(|f| u32::try_from(f).ok()).unwrap_or(0);
        let to = sel["to"].as_u64().and_then(|f| u32::try_from(f).ok()).unwrap_or(0);
        let (rows, cells) =
            if rows_selected || from == 0 { (elements, None) } else { (Vec::new(), Some((elements, from, to))) };
        let scene = s.app.scene.clone();
        let (range, range_free) = match &scene {
            Some(sc) => {
                let (mode, a, b) = effective_range(&s.timeline.ranges, sc, length_of(doc, sc));
                ((a, b), mode == RangeMode::Free)
            }
            None => ((1, 1), false),
        };
        let single = s.env.pref(crate::commands::timeline::SINGLE_CELL_KEY).and_then(|v| v.as_bool()).unwrap_or(true);
        Self {
            scene,
            rows,
            cells,
            current_element: s.app.element.clone(),
            current_frame: s.app.frame,
            single_cell: single,
            range,
            range_free,
            parameter_rows: s.timeline.parameter_rows.clone(),
            parameter_rows_hidden: s.timeline.parameter_rows_hidden,
            drag_options: crate::commands::transfer_options::cell_base_in(
                s,
                crate::commands::transfer_options::Operation::DragDrop,
            ),
            column_widths: s.timeline.column_widths.clone(),
            thumbnails: s.ui.drawing().xsheet.thumbnails,
        }
    }
}

/// What a gesture produces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Plan {
    /// Nothing (a click on empty space, a drag that moved nothing).
    Nothing,
    /// Commands run one after another (`ui` selection commands, or one action).
    Commands {
        /// `[command, params]` pairs.
        commands: Vec<(String, Value)>,
    },
    /// One atomic batch: one undo step (a row moved to another level of the root).
    Batch {
        /// The batch's undo label.
        label: String,
        /// `[command, params]` pairs.
        steps: Vec<(String, Value)>,
    },
}

fn one(command: &str, params: Value) -> Plan {
    Plan::Commands { commands: vec![(command.to_owned(), params)] }
}

/// What is under a press.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Target {
    /// Nothing.
    Nothing,
    /// The ruler (current frame).
    Ruler {
        frame: u32,
    },
    /// A playback-range marker.
    RangeStart,
    RangeEnd,
    /// A row's name.
    Row {
        row: usize,
        element: String,
    },
    /// A bar's body, start handle, end handle or loop handle.
    BarBody {
        row: usize,
        element: String,
        frame: u32,
    },
    BarStart {
        row: usize,
        element: String,
    },
    BarEnd {
        row: usize,
        element: String,
    },
    LoopHandle {
        row: usize,
        element: String,
    },
    /// A keyframe symbol.
    Keyframe {
        row: usize,
        element: String,
        frame: u32,
    },
    /// A cell: inside the cell selection or not.
    Cell {
        row: usize,
        element: String,
        frame: u32,
        selected: bool,
    },
    /// An Exposure Sheet cell (M5 C5.2a).
    SheetCell {
        column: usize,
        element: String,
        frame: u32,
        selected: bool,
    },
    /// The bottom edge of an exposure's last cell in the sheet (lengthen or shorten, 04 §12.9).
    ExposureEdge {
        column: usize,
        element: String,
        frame: u32,
    },
    /// An Exposure Sheet column header.
    Header {
        column: usize,
        element: String,
    },
}

// ----- geometry ----------------------------------------------------------------------------------------------------------

/// The frame under `x` (the first frame of the cell under it).
#[must_use]
pub fn frame_at(l: &Layout, g: &GestureLayout, x: f64) -> u32 {
    let z = l.zoom(g.zoom_level);
    let cell = (x / f64::from(z.cell_width.max(1))).floor();
    // `as` saturates a finite float at the integer range; the arithmetic after it saturates too, so no coordinate can
    // wrap (ruling E-31; the entry points also refuse coordinates beyond [`COORDINATE_BOUND`]).
    let cell = if cell.is_finite() { cell as i64 } else { 0 };
    let f = i64::from(g.first_frame).saturating_add(cell.saturating_mul(i64::from(z.frames_per_cell.max(1))));
    u32::try_from(f.clamp(1, i64::from(u32::MAX))).unwrap_or(1)
}

/// The left edge of the cell that holds `frame`.
#[must_use]
pub fn x_of(l: &Layout, g: &GestureLayout, frame: u32) -> f64 {
    let z = l.zoom(g.zoom_level);
    let cells = (i64::from(frame) - i64::from(g.first_frame)).div_euclid(i64::from(z.frames_per_cell.max(1)));
    cells as f64 * f64::from(z.cell_width)
}

/// The visible row under `y` (`None` below the last row or above the first).
fn row_at(l: &Layout, g: &GestureLayout, y: f64, rows: usize) -> Option<usize> {
    let r = ((y + g.scroll_top) / f64::from(l.row_height.max(1))).floor();
    (r >= 0.0 && r.is_finite() && (r as usize) < rows).then_some(r as usize)
}

/// Whether every modifier of a role is held.
fn held(role: &[String], s: &Sample) -> bool {
    !role.is_empty() && role.iter().all(|m| s.modifiers.iter().any(|x| x.eq_ignore_ascii_case(m)))
}

/// One visible row.
#[derive(Debug, Clone)]
pub(crate) struct VisibleRow {
    pub id: Id,
    pub parent: Option<Id>,
    pub index: usize,
    pub ty: ElementType,
}

/// The rows the Timeline shows, as indices into rows given in Timeline order as `(depth, collapsed)` (ruling E-37): a row
/// is shown unless one of its ancestors is collapsed. The rule reads only members `timeline.rows.list` declares (`depth`,
/// `collapsed`), so the window draws the rows the interpreter hit-tests by the same rule
/// (`desktop/src/timeline/rows.ts`); both check the shared case table `timeline-rows-cases.json`. Effect-parameter rows
/// (04 §2.9) are rows like any other until effect elements exist (M7, with `timeline.effectParameterRows.set`).
pub(crate) fn shown_rows(rows: impl IntoIterator<Item = (usize, bool)>) -> Vec<usize> {
    let mut out = Vec::new();
    let mut hidden_under: Option<usize> = None;
    for (i, (depth, collapsed)) in rows.into_iter().enumerate() {
        if let Some(d) = hidden_under {
            if depth > d {
                continue;
            }
            hidden_under = None;
        }
        if collapsed {
            hidden_under = Some(depth);
        }
        out.push(i);
    }
    out
}

/// Whether the effect-parameter setting shows an effect-parameter row (ruling E-40; 04 line 74, §2.9): its effect's own
/// setting, else the setting for every effect. Other rows are never hidden by it. Parameter rows are leaves (nothing is
/// parented under them), so leaving one out hides exactly that row.
pub(crate) fn parameter_row_shown(
    rows: &BTreeMap<String, bool>,
    hidden_by_default: bool,
    ty: ElementType,
    parent: Option<&Id>,
) -> bool {
    ty != ElementType::EffectParameter
        || parent.and_then(|p| rows.get(p.as_str()).copied()).unwrap_or(!hidden_by_default)
}

/// The Timeline's visible rows in order: the rows `timeline.rows.list` lists (without the effect-parameter rows the
/// setting hides, ruling E-40), by the rule of [`shown_rows`].
pub(crate) fn visible_rows(doc: &Document, scene: &Id, state: &State) -> Vec<VisibleRow> {
    let Some(sc) = doc.scene(scene) else { return Vec::new() };
    let rows: Vec<(h::Placed, ElementType, bool)> = h::placed(sc.tree())
        .into_iter()
        .filter_map(|q| doc.element(&q.id).map(|e| (*e.element_type(), *e.collapsed())).map(|(t, c)| (q, t, c)))
        .filter(|(q, t, _)| {
            parameter_row_shown(&state.parameter_rows, state.parameter_rows_hidden, *t, q.parent.as_ref())
        })
        .collect();
    let shown = shown_rows(rows.iter().map(|(q, _, c)| (q.depth, *c)));
    shown
        .into_iter()
        .filter_map(|i| rows.get(i))
        .map(|(q, ty, _)| VisibleRow { id: q.id.clone(), parent: q.parent.clone(), index: q.index, ty: *ty })
        .collect()
}

/// A row's bar: `(start, end, loops, duration)` for a bar element, `None` for a cell-based row.
fn bar_of(doc: &Document, id: &Id) -> Option<(u32, u32, u32, u32)> {
    let e = doc.element(id)?;
    match (e.start(), e.duration()) {
        (Some(s), Some(d)) => Some((*s, (*s).saturating_add((*d).max(1)) - 1, (*e.loops()).max(1), (*d).max(1))),
        _ => None,
    }
}

/// Types whose bar can loop (04 §11.10: `element.notLoopable` otherwise).
fn loopable(ty: ElementType) -> bool {
    !ty.is_cell_based() && ty != ElementType::Camera
}

/// Types that can be parents (04 §9.2; the kernel's rule, `timing::parentable`).
fn parentable(ty: ElementType) -> bool {
    crate::commands::timing::parentable(ty)
}

/// The keyed scene frames of an element (the Timeline's keyframe symbols).
fn keyed(doc: &Document, id: &Id) -> Vec<u32> {
    let mut budget = crate::commands::timing::Budget::new("hit-testing keyframes");
    crate::commands::timeline::keyed_frames(doc, id, &mut budget).unwrap_or_default()
}

/// What a press in a row of the tracks hits (ruling E-35; the rule is stated in `timeline-layout.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "zone")]
pub enum Zone {
    /// A keyframe symbol (its scene frame).
    Keyframe {
        /// The keyed frame.
        frame: u32,
    },
    /// The end column's upper half of a loopable bar.
    LoopHandle,
    /// The end column elsewhere.
    BarEnd,
    /// The start handle.
    BarStart,
    /// Strictly between the bar's edges.
    BarBody,
    /// Anywhere else: a cell.
    Cell,
}

/// The zone under `x` in a row (ruling E-35), in priority order, each zone including its edge pixels: a keyframe symbol
/// (`keys` holds the keyed frames in the row's keyframe band, empty outside it; the earliest within `keyframeRadius`),
/// the end column (`endHandleWidth`: the loop handle in the upper half of a loopable bar, else the end handle), the start
/// handle (`startHandleWidth`), the body, a cell. `bar` is the bar's first and last frame (its first cycle).
#[must_use]
pub fn zone_at(
    l: &Layout,
    g: &GestureLayout,
    x: f64,
    upper: bool,
    bar: Option<(u32, u32)>,
    loopable: bool,
    keys: &[u32],
) -> Zone {
    let cw = f64::from(l.zoom(g.zoom_level).cell_width);
    let radius = f64::from(l.keyframe_radius);
    if let Some(k) = keys.iter().find(|k| (x - (x_of(l, g, **k) + cw / 2.0)).abs() <= radius) {
        return Zone::Keyframe { frame: *k };
    }
    if let Some((start, end)) = bar {
        let (xs, xe) = (x_of(l, g, start), x_of(l, g, end) + cw);
        if (x - xe).abs() <= f64::from(l.end_handle_width) {
            return if upper && loopable { Zone::LoopHandle } else { Zone::BarEnd };
        }
        if (x - xs).abs() <= f64::from(l.start_handle_width) {
            return Zone::BarStart;
        }
        if x > xs && x < xe {
            return Zone::BarBody;
        }
    }
    Zone::Cell
}

// ----- the press -----------------------------------------------------------------------------------------------------------

/// What is under a press (design note §5.2).
#[must_use]
pub fn classify(doc: &Document, state: &State, g: &GestureLayout, s: &Sample) -> Target {
    let l = layout();
    let Some(scene) = &state.scene else { return Target::Nothing };
    let z = l.zoom(g.zoom_level);
    let cw = f64::from(z.cell_width);
    match g.surface {
        Surface::XsheetGrid => crate::xsheet_gesture::classify(doc, state, g, s, false),
        Surface::XsheetHeaders => crate::xsheet_gesture::classify(doc, state, g, s, true),
        Surface::Ruler => {
            if state.range_free {
                let half = f64::from(l.range_marker_half_width);
                let centre = |f: u32| x_of(l, g, f) + cw / 2.0;
                if (s.x - centre(state.range.0)).abs() <= half {
                    return Target::RangeStart;
                }
                if (s.x - centre(state.range.1)).abs() <= half {
                    return Target::RangeEnd;
                }
            }
            Target::Ruler { frame: frame_at(l, g, s.x) }
        }
        Surface::Layers => {
            let rows = visible_rows(doc, scene, state);
            match row_at(l, g, s.y, rows.len()) {
                Some(r) => Target::Row { row: r, element: rows[r].id.as_str().to_owned() },
                None => Target::Nothing,
            }
        }
        Surface::Tracks => {
            let rows = visible_rows(doc, scene, state);
            let Some(r) = row_at(l, g, s.y, rows.len()) else { return Target::Nothing };
            let row = &rows[r];
            let element = row.id.as_str().to_owned();
            let frame = frame_at(l, g, s.x);
            let top_half = (s.y + g.scroll_top) - (r as f64) * f64::from(l.row_height) < f64::from(l.row_height) / 2.0;
            let selected =
                state.cells.as_ref().is_some_and(|(els, a, b)| els.contains(&row.id) && (*a..=*b).contains(&frame));
            // A cell-based row draws its keyframe symbols in the upper half, its drawing symbols in the lower half; a
            // press inside the cell selection drags the cells (design note §5.2).
            let key_band = !row.ty.is_cell_based() || (top_half && !selected);
            let keys = if key_band { keyed(doc, &row.id) } else { Vec::new() };
            let bar = bar_of(doc, &row.id).map(|(start, end, ..)| (start, end));
            match zone_at(l, g, s.x, top_half, bar, loopable(row.ty), &keys) {
                Zone::Keyframe { frame: k } => Target::Keyframe { row: r, element, frame: k },
                Zone::LoopHandle => Target::LoopHandle { row: r, element },
                Zone::BarEnd => Target::BarEnd { row: r, element },
                Zone::BarStart => Target::BarStart { row: r, element },
                Zone::BarBody => Target::BarBody { row: r, element, frame },
                Zone::Cell => Target::Cell { row: r, element, frame, selected },
            }
        }
    }
}

// ----- the release -----------------------------------------------------------------------------------------------------------

fn travel(samples: &[Sample]) -> f64 {
    let Some(p) = samples.first() else { return 0.0 };
    samples.iter().map(|s| (s.x - p.x).abs().max((s.y - p.y).abs())).fold(0.0, f64::max)
}

fn ids_json(ids: &[Id]) -> Value {
    json!(ids.iter().map(Id::as_str).collect::<Vec<_>>())
}

/// The commands of a gesture (design note §5.2; the plan's §6.3 table).
#[must_use]
pub fn plan(doc: &Document, state: &State, g: &GestureLayout, samples: &[Sample]) -> Plan {
    let l = layout();
    let (Some(first), Some(last)) = (samples.first(), samples.last()) else { return Plan::Nothing };
    let Some(scene) = &state.scene else { return Plan::Nothing };
    let mods = l.modifiers(&g.platform);
    let dragged = travel(samples) >= f64::from(l.drag_threshold);
    let len = length_of(doc, scene);
    let target = classify(doc, state, g, first);
    if matches!(g.surface, Surface::XsheetGrid | Surface::XsheetHeaders) {
        return crate::xsheet_gesture::plan(doc, state, g, samples, target, dragged, &mods);
    }
    let rows = visible_rows(doc, scene, state);
    let delta = i64::from(frame_at(l, g, last.x)) - i64::from(frame_at(l, g, first.x));
    match target {
        Target::Nothing => Plan::Nothing,
        Target::Ruler { .. } => {
            let f = frame_at(l, g, last.x).clamp(1, len);
            one("timeline.currentFrame.set", json!({ "frame": f }))
        }
        Target::RangeStart | Target::RangeEnd if !dragged => Plan::Nothing,
        Target::RangeStart => one("timeline.playbackRange.set", json!({ "start": frame_at(l, g, last.x) })),
        Target::RangeEnd => one("timeline.playbackRange.set", json!({ "end": frame_at(l, g, last.x) })),
        Target::Row { row, .. } if !dragged => row_click(&rows, row, state, first, &mods),
        Target::Row { row, .. } => row_drop(doc, &rows, row, state, g, last),
        Target::BarBody { row, frame, .. } if !dragged => cell_click(state, &rows, row, frame, first, &mods, len),
        Target::BarBody { row, .. } => {
            if delta == 0 {
                return Plan::Nothing;
            }
            let pressed = rows[row].id.clone();
            let mut targets: Vec<Id> = if state.rows.contains(&pressed) {
                rows.iter()
                    .filter(|r| state.rows.contains(&r.id) && bar_of(doc, &r.id).is_some())
                    .map(|r| r.id.clone())
                    .collect()
            } else {
                vec![pressed]
            };
            targets.dedup();
            let min_start = targets.iter().filter_map(|t| bar_of(doc, t)).map(|b| b.0).min().unwrap_or(1);
            let delta = delta.max(1 - i64::from(min_start));
            if delta == 0 {
                return Plan::Nothing;
            }
            one("element.timing.set", json!({ "elementIds": ids_json(&targets), "startDelta": delta }))
        }
        Target::BarEnd { row, .. } => {
            let Some((start, end, ..)) = bar_of(doc, &rows[row].id) else { return Plan::Nothing };
            let new_end = (i64::from(end) + delta).max(i64::from(start));
            if !dragged || new_end == i64::from(end) {
                return Plan::Nothing;
            }
            let duration = new_end - i64::from(start) + 1;
            one(
                "element.timing.set",
                json!({ "elementIds": [rows[row].id.as_str()], "duration": duration, "anchor": "start", "keyframes": "scale" }),
            )
        }
        Target::BarStart { row, .. } => {
            let Some((start, end, ..)) = bar_of(doc, &rows[row].id) else { return Plan::Nothing };
            // `max(1)`: a clamp whose maximum is below its minimum panics (ruling E-21's sweep).
            let new_start = (i64::from(start) + delta).clamp(1, i64::from(end).max(1));
            if !dragged || new_start == i64::from(start) {
                return Plan::Nothing;
            }
            let duration = i64::from(end) - new_start + 1;
            one(
                "element.timing.set",
                json!({ "elementIds": [rows[row].id.as_str()], "duration": duration, "anchor": "end", "keyframes": "scale" }),
            )
        }
        Target::LoopHandle { row, .. } => {
            let Some((start, end, loops, duration)) = bar_of(doc, &rows[row].id) else { return Plan::Nothing };
            let cycle = f64::from((duration / loops).max(1));
            let new_end = (i64::from(end) + delta).max(i64::from(start));
            let n = ((new_end - i64::from(start) + 1) as f64 / cycle).round().max(1.0);
            let n = if n.is_finite() { n as i64 } else { 1 };
            if !dragged || n == i64::from(loops) {
                return Plan::Nothing;
            }
            one("element.timing.set", json!({ "elementIds": [rows[row].id.as_str()], "loops": n.min(100_000) }))
        }
        Target::Keyframe { row, frame, .. } if !dragged => cell_click(state, &rows, row, frame, first, &mods, len),
        Target::Keyframe { row, frame, .. } => {
            let to = i64::from(frame) + delta;
            if delta == 0 || to < 1 {
                return Plan::Nothing;
            }
            one("anim.key.move", json!({ "nodeId": rows[row].id.as_str(), "fromFrame": frame, "toFrame": to }))
        }
        Target::Cell { row, frame, .. } if !dragged => cell_click(state, &rows, row, frame, first, &mods, len),
        Target::Cell { row, frame, selected: true, .. } => {
            let Some((els, from, to)) = &state.cells else { return Plan::Nothing };
            let as_i64 = |r: usize| i64::try_from(r).unwrap_or(i64::MAX);
            let drows = row_at(l, g, last.y, usize::MAX).map_or(0, |r| as_i64(r).saturating_sub(as_i64(row)));
            let delta = delta.max(1 - i64::from(*from));
            if delta == 0 && drows == 0 {
                return cell_click(state, &rows, row, frame, first, &mods, len);
            }
            let mode = if held(&mods.link, last) {
                "link"
            } else if held(&mods.copy, last) {
                "copy"
            } else {
                "move"
            };
            // The drag-and-drop set resolved at the press, inline: a change of the stored set while the drag is in
            // progress does not change what the preview showed (ruling E-32).
            let mut params = json!({
                "sourceElementIds": ids_json(els),
                "from": from,
                "to": to,
                "targetFrame": i64::from(*from) + delta,
                "mode": mode,
                "options": Value::Object(state.drag_options.clone()),
            });
            if drows != 0 {
                let base = els.first().and_then(|e| rows.iter().position(|r| &r.id == e)).unwrap_or(row);
                let t = i64::try_from(base).unwrap_or(i64::MAX).saturating_add(drows);
                let Some(target) = usize::try_from(t).ok().and_then(|t| rows.get(t)) else { return Plan::Nothing };
                params["targetElementId"] = json!(target.id.as_str());
            }
            one("cell.range.move", params)
        }
        // The sheet's targets are planned above (they never come from a Timeline surface).
        Target::SheetCell { .. } | Target::ExposureEdge { .. } | Target::Header { .. } => Plan::Nothing,
        Target::Cell { row, frame, selected: false, .. } => {
            let r1 = row_at(l, g, last.y, rows.len()).unwrap_or(row);
            let (a, b) = (row.min(r1), row.max(r1));
            let f1 = frame_at(l, g, last.x);
            let ids: Vec<Id> = rows[a..=b].iter().map(|r| r.id.clone()).collect();
            Plan::Commands {
                commands: vec![
                    (
                        "cell.select".to_owned(),
                        json!({ "elementIds": ids_json(&ids), "from": frame.min(f1), "to": frame.max(f1) }),
                    ),
                    (
                        "xsheet.currentCell.set".to_owned(),
                        json!({ "elementId": rows[row].id.as_str(), "frame": frame }),
                    ),
                ],
            }
        }
    }
}

/// A click on a row name (04 lines 685-686): replace and make current; extend: the rows between the current element
/// and this one; toggle: toggle this row.
fn row_click(
    rows: &[VisibleRow],
    row: usize,
    state: &State,
    s: &Sample,
    mods: &crate::timeline_layout::Modifiers,
) -> Plan {
    let id = &rows[row].id;
    if held(&mods.toggle, s) && !held(&mods.extend, s) {
        return one("element.select", json!({ "elementIds": [id.as_str()], "mode": "toggle" }));
    }
    let ids: Vec<Id> = match (
        held(&mods.extend, s),
        state.current_element.as_ref().and_then(|c| rows.iter().position(|r| &r.id == c)),
    ) {
        (true, Some(anchor)) => rows[anchor.min(row)..=anchor.max(row)].iter().map(|r| r.id.clone()).collect(),
        _ => vec![id.clone()],
    };
    Plan::Commands {
        commands: vec![
            ("element.select".to_owned(), json!({ "elementIds": ids_json(&ids), "mode": "replace" })),
            ("element.current.set".to_owned(), json!({ "elementId": id.as_str() })),
        ],
    }
}

/// A click on a cell (04 lines 662, 687-689): the cell, or the track while single-cell selection is off (the
/// single-cell modifier selects one cell); extend: the rectangle from the current cell; the clicked cell becomes current.
fn cell_click(
    state: &State,
    rows: &[VisibleRow],
    row: usize,
    frame: u32,
    s: &Sample,
    mods: &crate::timeline_layout::Modifiers,
    len: u32,
) -> Plan {
    let id = &rows[row].id;
    if held(&mods.extend, s)
        && let Some(anchor) = state.current_element.as_ref().and_then(|c| rows.iter().position(|r| &r.id == c))
    {
        let ids: Vec<Id> = rows[anchor.min(row)..=anchor.max(row)].iter().map(|r| r.id.clone()).collect();
        let f0 = state.current_frame.max(1);
        return one("cell.select", json!({ "elementIds": ids_json(&ids), "from": f0.min(frame), "to": f0.max(frame) }));
    }
    let select = if state.single_cell || held(&mods.single_cell, s) {
        json!({ "elementIds": [id.as_str()], "from": frame, "to": frame })
    } else {
        json!({ "elementIds": [id.as_str()], "from": 1, "to": len.max(1), "extent": "track" })
    };
    Plan::Commands {
        commands: vec![
            ("cell.select".to_owned(), select),
            ("xsheet.currentCell.set".to_owned(), json!({ "elementId": id.as_str(), "frame": frame })),
        ],
    }
}

/// A row dropped (04 §9.1 lines 476-479, §11.13): between two rows reorders (another level: reparents), onto a peg,
/// effect, skeleton or camera row attaches.
fn row_drop(doc: &Document, rows: &[VisibleRow], row: usize, state: &State, g: &GestureLayout, s: &Sample) -> Plan {
    let l = layout();
    let Some(scene) = &state.scene else { return Plan::Nothing };
    let Some(sc) = doc.scene(scene) else { return Plan::Nothing };
    let pressed = rows[row].id.clone();
    let moving: Vec<Id> = if state.rows.contains(&pressed) {
        rows.iter().filter(|r| state.rows.contains(&r.id)).map(|r| r.id.clone()).collect()
    } else {
        vec![pressed]
    };
    let moving_set: BTreeSet<&Id> = moving.iter().collect();
    let parents = h::parents(sc.tree());
    let inside_moving = |id: &Id| {
        let mut cur = Some(id.clone());
        while let Some(c) = cur {
            if moving_set.contains(&c) {
                return true;
            }
            cur = parents.get(&c).cloned().flatten();
        }
        false
    };
    let rh = f64::from(l.row_height.max(1));
    let pos = (s.y + g.scroll_top) / rh;
    let t = if pos.is_finite() { pos.floor().max(0.0) as usize } else { 0 };
    let frac = pos - pos.floor();
    let zone = l.drop_zone_between;
    // (level parent, insertion index among the level's present children)
    let (level, index): (Option<Id>, usize) = if t >= rows.len() {
        (None, h::level_ids(sc.tree(), None).len())
    } else {
        let r = &rows[t];
        let onto = parentable(r.ty) && frac > zone && frac < 1.0 - zone;
        if onto {
            if inside_moving(&r.id) {
                return Plan::Nothing;
            }
            if moving.iter().all(|m| parents.get(m).cloned().flatten().as_ref() == Some(&r.id)) {
                return Plan::Nothing;
            }
            return one("element.parent.set", json!({ "elementIds": ids_json(&moving), "parentId": r.id.as_str() }));
        }
        let before = frac < 0.5;
        let expanded_parent = rows.get(t + 1).is_some_and(|n| n.parent.as_ref() == Some(&r.id));
        if before {
            (r.parent.clone(), r.index)
        } else if expanded_parent {
            (Some(r.id.clone()), 0)
        } else {
            (r.parent.clone(), r.index + 1)
        }
    };
    if level.as_ref().is_some_and(&inside_moving) {
        return Plan::Nothing;
    }
    let present = h::level_ids(sc.tree(), level.as_ref());
    let removed_before = present.iter().take(index).filter(|x| moving_set.contains(x)).count();
    let mut order: Vec<Id> = present.iter().filter(|x| !moving_set.contains(x)).cloned().collect();
    let at = (index - removed_before).min(order.len());
    for (k, m) in moving.iter().enumerate() {
        order.insert(at + k, m.clone());
    }
    let same_level = moving.iter().all(|m| parents.get(m).cloned().flatten() == level);
    if same_level {
        if order == present {
            return Plan::Nothing;
        }
        let mut params = json!({ "order": "timeline", "elementIds": ids_json(&order) });
        if let Some(p) = &level {
            params["parentId"] = json!(p.as_str());
        }
        return one("element.order.set", params);
    }
    match &level {
        Some(p) => {
            one("element.parent.set", json!({ "elementIds": ids_json(&moving), "parentId": p.as_str(), "index": at }))
        }
        None => {
            // To the root level from deeper levels: detach level by level, then set the root order (one undo step).
            let depth = |m: &Id| {
                let mut d = 0usize;
                let mut cur = parents.get(m).cloned().flatten();
                while let Some(c) = cur {
                    d += 1;
                    cur = parents.get(&c).cloned().flatten();
                }
                d
            };
            let deepest = moving.iter().map(depth).max().unwrap_or(0);
            let mut steps = Vec::new();
            for k in 0..deepest {
                let up: Vec<Id> = moving.iter().filter(|m| depth(m) > k).cloned().collect();
                steps.push(("element.parent.remove".to_owned(), json!({ "elementIds": ids_json(&up) })));
            }
            let root_now: Vec<Id> =
                h::level_ids(sc.tree(), None).iter().filter(|x| !moving_set.contains(x)).cloned().collect();
            let mut root = root_now;
            let at = at.min(root.len());
            for (k, m) in moving.iter().enumerate() {
                root.insert(at + k, m.clone());
            }
            steps.push(("element.order.set".to_owned(), json!({ "order": "timeline", "elementIds": ids_json(&root) })));
            Plan::Batch { label: "Move Rows".to_owned(), steps }
        }
    }
}

// ----- the window's path ---------------------------------------------------------------------------------------------------

/// A press the window made, kept until its release (E-10).
#[derive(Debug, Clone)]
pub(crate) struct Grab {
    pub layout: GestureLayout,
    pub press: Sample,
    pub doc: Document,
    pub state: State,
}

/// A request from the window (`timing_gesture`): `press`, `release` or `abort`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "phase", deny_unknown_fields)]
pub enum Request {
    /// A press: classify it and keep a snapshot.
    Press {
        /// The layout at the press.
        layout: GestureLayout,
        /// The press.
        sample: Sample,
    },
    /// The release: every sample from the press to the release.
    Release {
        /// The grab's id.
        #[serde(rename = "grabId")]
        grab_id: u64,
        /// The samples, the press first.
        samples: Vec<Sample>,
    },
    /// Escape, a lost capture, an engine restart: the grab is dropped and nothing runs.
    Abort {
        /// The grab's id.
        #[serde(rename = "grabId")]
        grab_id: u64,
    },
}

/// The largest coordinate a gesture's sample or scroll position may have, in pixels (ruling E-31): 100 000 frames at the
/// widest cell (48 px) is 4.8 million pixels; anything beyond is not a pointer position and is refused (`arg.invalid`).
pub const COORDINATE_BOUND: f64 = 10_000_000.0;

fn bounded(v: f64, pointer: &str) -> Result<(), CommandError> {
    if v.is_finite() && v.abs() <= COORDINATE_BOUND {
        return Ok(());
    }
    Err(CommandError::with_details(
        tt_registry::codes::ARG_INVALID,
        format!("a gesture coordinate must be a number within ±{COORDINATE_BOUND} pixels"),
        json!({ "pointer": pointer, "allowed": { "minimum": -COORDINATE_BOUND, "maximum": COORDINATE_BOUND } }),
    ))
}

/// Refuses a layout or a sample with a coordinate beyond [`COORDINATE_BOUND`] (the window's entry and `timing_replay`).
pub(crate) fn check_input(layout: Option<&GestureLayout>, samples: &[Sample], at: &str) -> Result<(), CommandError> {
    if let Some(g) = layout {
        bounded(g.scroll_top, "/layout/scrollTop")?;
    }
    for (i, s) in samples.iter().enumerate() {
        bounded(s.x, &format!("{at}/{i}/x"))?;
        bounded(s.y, &format!("{at}/{i}/y"))?;
    }
    Ok(())
}

/// The most grabs kept at once (a window has one gesture in progress; older grabs are abandoned).
const MAX_GRABS: usize = 8;

/// Runs a window request on the document thread (the host's `timing_gesture`): the answer of a press is the grab and
/// what is under it; of a release, the commands and their envelopes, or the cancellation and its reason.
pub(crate) fn run(
    core: &mut crate::run::Core,
    origin: &crate::types::Origin,
    req: Request,
) -> Result<Value, CommandError> {
    match req {
        Request::Press { layout: g, sample } => {
            check_input(Some(&g), std::slice::from_ref(&sample), "/sample")?;
            let s = core.session_mut().ok_or_else(crate::cx::no_project)?;
            let p = s.project.as_ref().ok_or_else(crate::cx::no_project)?;
            let doc = p.doc.clone();
            let state = State::of(&doc, s);
            let target = classify(&doc, &state, &g, &sample);
            let t = &mut s.timeline;
            t.grab_seq = t.grab_seq.wrapping_add(1);
            let id = t.grab_seq;
            t.grabs.insert(id, Grab { layout: g, press: sample, doc, state });
            while t.grabs.len() > MAX_GRABS {
                let Some(oldest) = t.grabs.keys().next().copied() else { break };
                t.grabs.remove(&oldest);
            }
            Ok(json!({ "grabId": id, "target": target }))
        }
        Request::Abort { grab_id } => {
            if let Some(s) = core.session_mut() {
                s.timeline.grabs.remove(&grab_id);
            }
            Ok(json!({ "aborted": true }))
        }
        Request::Release { grab_id, samples } => {
            check_input(None, &samples, "/samples")?;
            let s = core.session_mut().ok_or_else(crate::cx::no_project)?;
            let grab = s.timeline.grabs.remove(&grab_id).ok_or_else(|| {
                CommandError::with_details(
                    "timeline.targetChanged",
                    "the gesture's press is no longer known (another gesture began or the engine restarted)",
                    json!({ "reason": "unknownGrab", "grabId": grab_id }),
                )
            })?;
            let p = s.project.as_ref().ok_or_else(crate::cx::no_project)?;
            let doc = p.doc.clone();
            // The samples run from the press (the window sends it first; it is put back if it was left out).
            let mut samples = samples;
            if samples.first() != Some(&grab.press) {
                samples.insert(0, grab.press.clone());
            }
            // Rulings E-23 and E-28: the plan is made once, against the press-time snapshot, so its targets are the ids
            // resolved at the press (never a second hit test of the press position). It commits only when its outcome
            // in absolute values against the present document equals its outcome against the snapshot: the targets
            // still exist, their state and the content grabbed are unchanged.
            let planned = plan(&grab.doc, &grab.state, &grab.layout, &samples);
            let previewed = outcome(&grab.doc, &planned);
            let present = outcome(&doc, &planned);
            if previewed != present {
                return Ok(json!({
                    "cancelled": true,
                    "error": {
                        "code": "timeline.targetChanged",
                        "message": "what the gesture acted on changed while it was in progress; nothing was done",
                        "details": { "plan": planned, "previewed": previewed, "present": present },
                    },
                }));
            }
            Ok(commit(core, origin, &planned))
        }
    }
}

/// What a plan does to `doc`, in absolute values for every target (ruling E-23): a bar's resolved start, duration and
/// loops with the element's keyed frames; a keyframe move's frames with the element's keyed frames; a parent's child
/// order after a reparenting; a cell move's destination and the content of the grabbed and destination cells; the
/// absolute parameters of the selection and position commands.
pub(crate) fn outcome(doc: &Document, plan: &Plan) -> Value {
    let steps: Vec<&(String, Value)> = match plan {
        Plan::Nothing => Vec::new(),
        Plan::Commands { commands } => commands.iter().collect(),
        Plan::Batch { steps, .. } => steps.iter().collect(),
    };
    json!(steps.into_iter().map(|(c, p)| resolved(doc, c, p)).collect::<Vec<_>>())
}

fn id_list(v: &Value) -> Vec<Id> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().and_then(|k| Id::parse(k).ok())).collect())
        .unwrap_or_default()
}

/// The element ids a command names that the document no longer holds.
fn missing(doc: &Document, p: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for k in ["elementIds", "elementId", "nodeId", "nodeIds", "sourceElementIds", "targetElementId", "parentId"] {
        let ids: Vec<&str> = match &p[k] {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        out.extend(
            ids.into_iter()
                .filter(|s| Id::parse(s).ok().is_none_or(|id| doc.element(&id).is_none()))
                .map(str::to_owned),
        );
    }
    out
}

fn resolved(doc: &Document, command: &str, p: &Value) -> Value {
    let mut v = resolved_values(doc, command, p);
    if let Some(m) = v.as_object_mut() {
        m.insert("missing".to_owned(), json!(missing(doc, p)));
    }
    v
}

fn resolved_values(doc: &Document, command: &str, p: &Value) -> Value {
    let int = |k: &str| p.get(k).and_then(Value::as_i64);
    match command {
        "element.timing.set" => {
            let targets: Vec<Value> = id_list(&p["elementIds"])
                .iter()
                .map(|id| {
                    let Some(e) = doc.element(id) else { return json!({ "elementId": id.as_str(), "missing": true }) };
                    let start = i64::from(e.start().unwrap_or(1));
                    let duration = i64::from(e.duration().unwrap_or(1));
                    let loops = int("loops").unwrap_or_else(|| i64::from(*e.loops()));
                    let (start, duration) = match (int("startDelta"), int("duration"), p["anchor"].as_str()) {
                        (Some(d), _, _) => (start + d, duration),
                        (None, Some(n), Some("end")) => (start + duration - n, n),
                        (None, Some(n), _) => (start, n),
                        (None, None, _) => (int("start").unwrap_or(start), duration),
                    };
                    json!({
                        "elementId": id.as_str(), "start": start, "duration": duration, "loops": loops,
                        "keyed": keyed(doc, id),
                    })
                })
                .collect();
            json!({ "command": command, "targets": targets })
        }
        "anim.key.move" => {
            let node = p["nodeId"].as_str().and_then(|k| Id::parse(k).ok());
            json!({
                "command": command, "nodeId": p["nodeId"], "fromFrame": p["fromFrame"], "toFrame": p["toFrame"],
                "keyed": node.map(|n| keyed(doc, &n)),
            })
        }
        "element.parent.set" => {
            let moving = id_list(&p["elementIds"]);
            let parent = p["parentId"].as_str().and_then(|k| Id::parse(k).ok());
            let scene = moving.first().and_then(|m| doc.scene_of(m)).and_then(|s| doc.scene(s));
            let children = match (scene, &parent) {
                (Some(sc), Some(par)) => {
                    let mut order: Vec<Id> =
                        h::level_ids(sc.tree(), Some(par)).into_iter().filter(|c| !moving.contains(c)).collect();
                    let at = int("index").and_then(|i| usize::try_from(i).ok()).unwrap_or(order.len()).min(order.len());
                    for (k, m) in moving.iter().enumerate() {
                        order.insert(at + k, m.clone());
                    }
                    ids_json(&order)
                }
                _ => Value::Null,
            };
            json!({ "command": command, "parentId": p["parentId"], "children": children })
        }
        "element.order.set" => {
            // The order names the whole level: the level's present children (as a set) are part of the outcome.
            let ids = id_list(&p["elementIds"]);
            let parent = p["parentId"].as_str().and_then(|k| Id::parse(k).ok());
            let scene = ids.first().and_then(|m| doc.scene_of(m)).and_then(|s| doc.scene(s));
            let mut level: Vec<String> = scene
                .map(|sc| h::level_ids(sc.tree(), parent.as_ref()).iter().map(|x| x.as_str().to_owned()).collect())
                .unwrap_or_default();
            level.sort();
            json!({ "command": command, "params": p, "level": level })
        }
        "cell.range.move" => {
            let sources = id_list(&p["sourceElementIds"]);
            let (from, to) = (int("from").unwrap_or(1).max(1), int("to").unwrap_or(1).max(1));
            let target = int("targetFrame").unwrap_or(1).max(1);
            let frames = |a: i64, b: i64, e: &Id| -> Vec<Option<String>> {
                (a..=b)
                    .filter_map(|f| u32::try_from(f).ok())
                    .map(|f| crate::app::AppContext::exposed_drawing(doc, e, f).map(|d| d.as_str().to_owned()))
                    .collect()
            };
            let span = (to - from).max(0);
            // The content grabbed: the source cells' drawings and their notes (E-28).
            let notes = |e: &Id| -> Vec<(u32, String)> {
                doc.element(e)
                    .map(|x| {
                        x.cell_notes()
                            .iter()
                            .filter(|(f, _)| (from..=to).contains(&i64::from(**f)))
                            .map(|(f, t)| (*f, t.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let grabbed: Vec<Value> =
                sources.iter().map(|e| json!([e.as_str(), frames(from, to, e), notes(e)])).collect();
            let dest_ids: Vec<Id> = match p["targetElementId"].as_str().and_then(|k| Id::parse(k).ok()) {
                Some(t) => vec![t],
                None => sources.clone(),
            };
            let destination: Vec<Value> =
                dest_ids.iter().map(|e| json!([e.as_str(), frames(target, target + span, e)])).collect();
            json!({
                "command": command, "targetFrame": target, "targetElementId": p["targetElementId"],
                "mode": p["mode"], "options": p["options"], "grabbed": grabbed, "destination": destination,
            })
        }
        // `element.order.set` names the whole level's order; the selection and position commands are absolute.
        _ => json!({ "command": command, "params": p }),
    }
}

/// Runs a plan as `origin`: commands one after another (stopping at a refusal), or one atomic batch.
pub(crate) fn commit(core: &mut crate::run::Core, origin: &crate::types::Origin, plan: &Plan) -> Value {
    let mut envelopes = Vec::new();
    match plan {
        Plan::Nothing => {}
        Plan::Commands { commands } => {
            for (command, params) in commands {
                let env = core.invoke(&crate::types::Invocation {
                    command: command.clone(),
                    params: params.clone(),
                    options: crate::types::InvokeOptions::default(),
                    origin: origin.clone(),
                });
                let ok = env.is_ok();
                envelopes.push(serde_json::to_value(&env).unwrap_or(Value::Null));
                if !ok {
                    break;
                }
            }
        }
        Plan::Batch { label, steps } => {
            let req = crate::types::BatchRequest {
                label: label.clone(),
                steps: steps
                    .iter()
                    .map(|(id, params)| crate::types::BatchStep { id: id.clone(), params: params.clone(), name: None })
                    .collect(),
                atomic: true,
                stop_on_error: true,
                dry_run: false,
                expect_rev: BTreeMap::new(),
            };
            let env = core.batch(&req, origin);
            envelopes.push(serde_json::to_value(&env).unwrap_or(Value::Null));
        }
    }
    json!({ "plan": plan, "envelopes": envelopes })
}

/// The plan of a gesture against the session's present state, run or not (the headless replay, E-7).
pub(crate) fn replay(
    core: &mut crate::run::Core,
    origin: &crate::types::Origin,
    g: &GestureLayout,
    samples: &[Sample],
    execute: bool,
) -> Result<Value, CommandError> {
    check_input(Some(g), samples, "/samples")?;
    let s = core.session_mut().ok_or_else(crate::cx::no_project)?;
    let p = s.project.as_ref().ok_or_else(crate::cx::no_project)?;
    let doc = p.doc.clone();
    let state = State::of(&doc, s);
    let planned = plan(&doc, &state, g, samples);
    if execute { Ok(commit(core, origin, &planned)) } else { Ok(json!({ "plan": planned })) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(zoom: u64, first: u64) -> GestureLayout {
        GestureLayout {
            surface: Surface::Tracks,
            zoom_level: u32::try_from(zoom).unwrap_or(0),
            first_frame: u32::try_from(first).unwrap_or(1),
            scroll_top: 0.0,
            scroll_left: 0.0,
            light_table: None,
            platform: "windows".to_owned(),
        }
    }

    /// The rows shown (ruling E-37): every case of the shared table, which the window's `rows.ts` checks too.
    #[test]
    fn the_rows_shown_follow_the_shared_case_table() {
        let table: serde_json::Value =
            serde_json::from_str(include_str!("timeline-rows-cases.json")).unwrap_or_default();
        let cases = table["cases"].as_array().cloned().unwrap_or_default();
        assert!(cases.len() > 1000, "{} cases", cases.len());
        for c in &cases {
            let rows: Vec<(usize, bool)> = c["rows"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| (usize::try_from(r[0].as_u64().unwrap_or(0)).unwrap_or(0), r[1].as_bool().unwrap_or(false)))
                .collect();
            let want: Vec<usize> = c["shown"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|i| usize::try_from(i.as_u64().unwrap_or(0)).unwrap_or(0))
                .collect();
            assert_eq!(shown_rows(rows), want, "{c}");
        }
    }

    /// The shared case table: the window's `geometry.ts` checks the same cases (design note C5.1b §3).
    #[test]
    fn geometry_follows_the_shared_case_table() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("timeline-geometry-cases.json")).unwrap_or_default();
        let l = layout();
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        let f = |v: &Value| v.as_f64().unwrap_or(0.0);
        let frame_cases = cases["frameAt"].as_array().cloned().unwrap_or_default();
        assert!(frame_cases.len() > 100);
        for c in &frame_cases {
            let got = frame_at(l, &g(n(&c["zoomLevel"]), n(&c["firstFrame"])), f(&c["x"]));
            assert_eq!(u64::from(got), n(&c["frame"]), "{c}");
        }
        for c in cases["xOf"].as_array().cloned().unwrap_or_default() {
            let got = x_of(l, &g(n(&c["zoomLevel"]), n(&c["firstFrame"])), u32::try_from(n(&c["frame"])).unwrap_or(1));
            assert!((got - f(&c["xLeft"])).abs() < 1e-9, "{c}");
        }
        // The hit zones (ruling E-35).
        let zone_cases = cases["zones"].as_array().cloned().unwrap_or_default();
        assert!(zone_cases.len() > 40);
        for c in &zone_cases {
            let bar = c["bar"]
                .as_array()
                .map(|b| (u32::try_from(n(&b[0])).unwrap_or(1), u32::try_from(n(&b[1])).unwrap_or(1)));
            let keys: Vec<u32> =
                c["keys"].as_array().into_iter().flatten().filter_map(|k| u32::try_from(n(k)).ok()).collect();
            let got = zone_at(
                l,
                &g(n(&c["zoomLevel"]), n(&c["firstFrame"])),
                f(&c["x"]),
                c["upper"] == true,
                bar,
                c["loopable"] == true,
                &keys,
            );
            assert_eq!(serde_json::to_value(got).unwrap_or_default(), c["zone"], "{c}");
        }
        for c in cases["rowAt"].as_array().cloned().unwrap_or_default() {
            let mut gl = g(6, 1);
            gl.scroll_top = f(&c["scrollTop"]);
            let got = row_at(l, &gl, f(&c["y"]), usize::MAX);
            assert_eq!(got.map(|r| r as u64), Some(n(&c["row"])), "{c}");
        }
    }
}
