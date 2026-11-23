//! `timeline.*` (04 §11, §17.4; M5 C5.1a; `reviews/M5-design-timeline-kernel.md`): the current frame, the playback
//! range, the Timeline read model, frame labels, the Timeline view, the Timeline's display toggles and the structured
//! equivalents of its preferences.
//!
//! **One state each (SI-202, SI-203, rulings E-4, E-6).** The current frame is `AppContext.frame`, whatever command
//! writes it; the playback range of a scene is one record ([`PlaybackRange`]) written by `timeline.playbackRange.set`
//! and the `playback.range.*` commands through [`set_range`]; the time display, the Timeline background, single-cell
//! selection and the category colours are the 01 §7.3 preference keys, written as `prefs.set` writes them. The events
//! come from `Core::settle`, which compares the session before and after every invocation, so commands that write the
//! same state raise the same events.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::hierarchy as h;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::ElementType;
use tt_registry::{CommandError, codes, command};

use super::anim_common::{ChannelEdit, data_of, frame_of};
use super::common::NoParams;
use super::timing::{Budget, as_u32, element, element_id, err, internal, scene_id};
use crate::cx::Cx;
use crate::rules;

// ----- state ----------------------------------------------------------------------------------------------------------

/// The playback range of one scene (09 §2.8; SI-204: saved in `session.json`, outside revisions and undo).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PlaybackRange {
    /// Free (user-defined) rather than automatic.
    pub free: bool,
    /// The free start and end (meaningful while `free`).
    pub start: u32,
    pub end: u32,
    /// The last free values, restored when the range becomes free again (09 line 188).
    pub last_free: Option<(u32, u32)>,
}

/// The Timeline view of one scene (04 §11.5, §17.4 `timeline.view.set`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimelineView {
    /// Index into the zoom ladder of `timeline-layout.json`.
    pub zoom: u32,
    /// The first frame shown.
    pub first_frame: u32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self { zoom: crate::timeline_layout::layout().default_zoom_level, first_frame: 1 }
    }
}

/// Range mode (09 §2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RangeMode {
    /// Automatic fit: frame 1 to the scene's last frame.
    Auto,
    /// Free: user-defined start and end.
    Free,
}

/// A scene's effective playback range: `(mode, start, end)`, the free values clamped to the present scene length.
pub(crate) fn effective_range(ranges: &BTreeMap<String, PlaybackRange>, scene: &Id, len: u32) -> (RangeMode, u32, u32) {
    let len = len.max(1);
    match ranges.get(scene.as_str()) {
        Some(r) if r.free => {
            let end = r.end.clamp(1, len);
            (RangeMode::Free, r.start.clamp(1, end), end)
        }
        _ => (RangeMode::Auto, 1, len),
    }
}

/// The scene length of `scene` (at least 1).
pub(crate) fn length_of(doc: &Document, scene: &Id) -> u32 {
    doc.scene(scene).map_or(1, |s| u32::try_from(doc.scene_length(s)).unwrap_or(u32::MAX).max(1))
}

/// The current scene and its length.
pub(crate) fn current(cx: &Cx) -> Result<(Id, u32), CommandError> {
    let scene = scene_id(cx, None)?;
    let len = length_of(cx.document()?, &scene);
    Ok((scene, len))
}

/// A scene named by a command that acts on the current scene only (scene switching is 08's).
fn current_scene_only(cx: &Cx, given: Option<&str>, pointer: &str) -> Result<(Id, u32), CommandError> {
    let (scene, len) = current(cx)?;
    if let Some(r) = given {
        let named = scene_id(cx, Some(r))?;
        if named != scene {
            return Err(err(
                codes::ARG_INVALID,
                "the Timeline acts on the current scene; make that scene current first (08)",
                json!({ "pointer": pointer, "reason": "notCurrentScene" }),
            ));
        }
    }
    Ok((scene, len))
}

/// The outcome of [`set_range`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RangeSet {
    pub mode: RangeMode,
    pub start: u32,
    pub end: u32,
    /// A given value was moved (the other end passed, or clamped to the scene).
    pub adjusted: bool,
}

/// Sets a scene's playback range: the one rule of `timeline.playbackRange.set`, `playback.range.set` and
/// `playback.range.mode.set` (SI-202). A free value given while automatic makes the range free; a start past the end
/// moves the end to it and an end before the start moves the start (09 lines 186-187); then both are clamped to 1..scene
/// length (04 line 669). Automatic keeps the last free values; becoming free without values restores them (09 line 188).
pub(crate) fn set_range(
    cx: &mut Cx,
    scene: &Id,
    len: u32,
    mode: Option<RangeMode>,
    start: Option<i64>,
    end: Option<i64>,
) -> RangeSet {
    let len = len.max(1);
    let mut r = cx.session().timeline.ranges.get(scene.as_str()).copied().unwrap_or_default();
    let wants_free = match mode {
        Some(m) => m == RangeMode::Free,
        None => r.free || start.is_some() || end.is_some(),
    };
    let base = if r.free { Some((r.start, r.end)) } else { r.last_free };
    let (mut s, mut e) = base.map_or((1_i64, i64::from(len)), |(a, b)| (i64::from(a), i64::from(b)));
    let mut adjusted = false;
    if let Some(v) = start {
        s = v;
        if s > e {
            e = s;
            adjusted = end.is_none();
        }
    }
    if let Some(v) = end {
        e = v;
        if e < s {
            s = e;
            adjusted = true;
        }
    }
    let clamp = |v: i64| u32::try_from(v.clamp(1, i64::from(len))).unwrap_or(1);
    let (cs, ce) = (clamp(s), clamp(e));
    adjusted |= i64::from(cs) != s || i64::from(ce) != e;
    if start.is_none() && end.is_none() {
        adjusted = false;
    }
    if wants_free {
        r = PlaybackRange { free: true, start: cs, end: ce, last_free: Some((cs, ce)) };
    } else {
        let remembered = if start.is_some() || end.is_some() { Some((cs, ce)) } else { base };
        r = PlaybackRange { free: false, start: r.start, end: r.end, last_free: remembered };
    }
    cx.session_mut().timeline.ranges.insert(scene.as_str().to_owned(), r);
    let (mode, start, end) = effective_range(&cx.session().timeline.ranges, scene, len);
    RangeSet { mode, start, end, adjusted }
}

/// Marks how the current frame is about to change (`timeline.currentFrame.changed`'s `source`, 04 line 1466): read and
/// cleared by `Core::settle`.
pub(crate) fn frame_source(cx: &mut Cx, source: &'static str) {
    cx.session_mut().timeline.frame_source = Some(source);
}

/// Marks a navigation command (09 §2.4: First, Previous, Next, Last, the drawing steps, Change Current Frame, in every
/// command form, SI-202): playback stops at settle even when the frame does not change (09 line 138; ruling E-22).
pub(crate) fn navigated(cx: &mut Cx) {
    cx.session_mut().timeline.navigated = true;
}

// ----- timeline.currentFrame.* ------------------------------------------------------------------------------------------

/// `timeline.currentFrame.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentFrameSetParams {
    /// Frame: any integer, clamped to 1..scene length (09 line 139; ruling E-27).
    frame: i64,
    /// The scene (the current scene only).
    #[serde(default)]
    scene_id: Option<String>,
}

/// `{frame, clamped}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct CurrentFrameSetResult {
    /// The current frame.
    #[schemars(range(min = 1))]
    frame: u32,
    /// The given frame was outside 1..scene length.
    clamped: bool,
}

/// Sets the current frame (the state of `app.frame.set` too, SI-202).
pub(crate) fn put_frame(cx: &mut Cx, frame: u32) {
    let app = cx.app_mut();
    if app.frame != frame {
        app.frame = frame;
        app.drawing = None;
    }
}

#[command(
    id = "timeline.currentFrame.set",
    kind = ui,
    label = "Set current frame",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Go to frame 12", params = r#"{"frame": 12}"#),
    example(title = "Beyond the scene end: clamped", params = r#"{"frame": 100000}"#),
    gui = "Timeline: click or drag in the frame ruler, drag the playhead (M5 C5.1)",
)]
pub(crate) fn timeline_current_frame_set(
    cx: &mut Cx,
    p: CurrentFrameSetParams,
) -> Result<CurrentFrameSetResult, CommandError> {
    let (_, len) = current_scene_only(cx, p.scene_id.as_deref(), "/sceneId")?;
    let frame = u32::try_from(p.frame.clamp(1, i64::from(len))).unwrap_or(1);
    frame_source(cx, "command");
    navigated(cx);
    put_frame(cx, frame);
    Ok(CurrentFrameSetResult { frame, clamped: i64::from(frame) != p.frame })
}

/// Direction of `timeline.currentFrame.move`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveDirection {
    /// The first frame (of the playback range with `withinRange`).
    First,
    /// Back.
    Previous,
    /// Forward.
    Next,
    /// The last frame (of the playback range with `withinRange`).
    Last,
}

/// Unit of `timeline.currentFrame.move`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveUnit {
    /// Frames.
    #[default]
    Frame,
    /// Drawing changes of an element.
    Drawing,
}

fn one() -> i64 {
    1
}

/// `timeline.currentFrame.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentFrameMoveParams {
    /// Where to.
    direction: MoveDirection,
    /// Frames or drawing changes.
    #[serde(default)]
    unit: MoveUnit,
    /// The element whose drawing changes count (default the current element; with none, every visible cell-based
    /// element, 09 line 132).
    #[serde(default)]
    element_id: Option<String>,
    /// How many steps (spec A: `int = 1`; below 1 counts as 1, the observation of round 1).
    #[serde(default = "one")]
    count: i64,
    /// Limit movement to the playback range.
    #[serde(default)]
    within_range: bool,
}

/// `{frame, moved}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct CurrentFrameMoveResult {
    /// The current frame.
    #[schemars(range(min = 1))]
    frame: u32,
    /// The frame changed.
    moved: bool,
}

/// Frames at which the exposed drawing of one of `elements` changes, between `lo` and `hi` (both inclusive).
fn changes(doc: &Document, elements: &[Id], lo: u32, hi: u32) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for e in elements {
        let mut prev = if lo > 1 { crate::app::AppContext::exposed_drawing(doc, e, lo - 1) } else { None };
        for f in lo..=hi {
            let d = crate::app::AppContext::exposed_drawing(doc, e, f);
            if d != prev && (f > lo || lo == 1 || d.is_some() || prev.is_some()) {
                out.insert(f);
            }
            prev = d;
        }
    }
    out
}

/// The frame `count` drawing changes before or after `now` within `range` (09 §2.4 Previous / Next Drawing): changes of
/// `element` (default the current element; with none, of every visible cell-based element, 09 line 132); the frame
/// stays where no further change exists (never wraps). The one rule of `timeline.currentFrame.move` and
/// `app.frame.move` (SI-202).
pub(crate) fn drawing_step(
    cx: &Cx,
    scene: &Id,
    element: Option<&str>,
    next: bool,
    count: u32,
    (lo, hi): (u32, u32),
    now: u32,
) -> Result<u32, CommandError> {
    let doc = cx.document()?.clone();
    let len = length_of(&doc, scene);
    let elements: Vec<Id> = match (element, cx.app().element.clone()) {
        (Some(r), _) => vec![element_id(cx, r, "/elementId")?],
        (None, Some(e)) => vec![e],
        (None, None) => doc
            .scene(scene)
            .map(|s| {
                s.column_order()
                    .iter()
                    .filter(|e| doc.element(e).is_some_and(|x| *x.visible_in_scene()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
    };
    let mut budget = Budget::new("finding drawing changes");
    budget.spend(elements.len().saturating_mul(usize::try_from(len).unwrap_or(usize::MAX)))?;
    let marks = changes(&doc, &elements, 1, len);
    let mut f = now;
    for _ in 0..count {
        // At or past the end (or the start) nothing follows: the frame stays (09 line 135; ruling E-21). The guard comes
        // before `range`, which panics on a start after its end.
        let step = if next {
            if f >= hi { None } else { marks.range((Bound::Excluded(f), Bound::Included(hi))).next().copied() }
        } else if f <= lo {
            None
        } else {
            marks.range(lo..f).next_back().copied()
        };
        match step {
            Some(s) => f = s,
            None => break,
        }
    }
    Ok(f)
}

#[command(
    id = "timeline.currentFrame.move",
    kind = ui,
    label = "Go to first, previous, next or last frame",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "First frame of the playback range", params = r#"{"direction": "first", "withinRange": true}"#),
    example(title = "Next drawing change of Bob", params = r#"{"direction": "next", "unit": "drawing", "elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    gui = "Play menu and the main toolbar: First, Previous, Next, Last Frame (M5 C5.1)",
)]
pub(crate) fn timeline_current_frame_move(
    cx: &mut Cx,
    p: CurrentFrameMoveParams,
) -> Result<CurrentFrameMoveResult, CommandError> {
    let (scene, len) = current(cx)?;
    let (lo, hi) = if p.within_range {
        let (_, s, e) = effective_range(&cx.session().timeline.ranges, &scene, len);
        (s, e)
    } else {
        (1, len)
    };
    let now = cx.app().frame.clamp(1, len);
    let count = u32::try_from(p.count.clamp(1, i64::from(u32::MAX))).unwrap_or(1);
    let frame = match (p.direction, p.unit) {
        (MoveDirection::First, _) => lo,
        (MoveDirection::Last, _) => hi,
        (MoveDirection::Next, MoveUnit::Frame) => now.saturating_add(count).min(hi).max(now.min(hi)),
        (MoveDirection::Previous, MoveUnit::Frame) => now.saturating_sub(count).max(lo).min(now.max(lo)),
        (dir, MoveUnit::Drawing) => {
            drawing_step(cx, &scene, p.element_id.as_deref(), dir == MoveDirection::Next, count, (lo, hi), now)?
        }
    };
    frame_source(cx, "navigation");
    navigated(cx);
    put_frame(cx, frame);
    Ok(CurrentFrameMoveResult { frame, moved: frame != now })
}

/// `timeline.currentFrame.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentFrameChanged {
    /// The scene.
    scene_id: String,
    /// The current frame.
    #[schemars(range(min = 1))]
    frame: u32,
    /// `playback`, `navigation` or `command`.
    source: String,
}

#[command(
    id = "timeline.currentFrame.changed",
    kind = event,
    label = "Current frame changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn timeline_current_frame_changed(_cx: &mut Cx, _p: NoParams) -> Result<CurrentFrameChanged, CommandError> {
    Err(super::common::event_not_invocable("timeline.currentFrame.changed"))
}

// ----- timeline.playbackRange.* -------------------------------------------------------------------------------------------

/// `timeline.playbackRange.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaybackRangeSetParams {
    /// Automatic or free (a start or end given makes it free).
    #[serde(default)]
    mode: Option<RangeMode>,
    /// Free start (clamped).
    #[serde(default)]
    start: Option<i64>,
    /// Free end (clamped).
    #[serde(default)]
    end: Option<i64>,
}

/// `{mode, start, end}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RangeResult {
    /// `auto` or `free`.
    pub mode: RangeMode,
    /// First frame of the range.
    pub start: u32,
    /// Last frame of the range.
    pub end: u32,
}

#[command(
    id = "timeline.playbackRange.set",
    kind = ui,
    label = "Set playback range",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Play frames 10 to 30 only", params = r#"{"mode": "free", "start": 10, "end": 30}"#),
    example(title = "Back to the whole scene", params = r#"{"mode": "auto"}"#),
    gui = "Timeline: drag a playback-range marker; ruler context: Playback Range, Set Playback Start / End Frame (M5 C5.1)",
)]
pub(crate) fn timeline_playback_range_set(cx: &mut Cx, p: PlaybackRangeSetParams) -> Result<RangeResult, CommandError> {
    let (scene, len) = current(cx)?;
    let r = set_range(cx, &scene, len, p.mode, p.start, p.end);
    Ok(RangeResult { mode: r.mode, start: r.start, end: r.end })
}

#[command(
    id = "timeline.playbackRange.get",
    kind = query,
    label = "Get playback range",
    scopes = [read],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "The current scene's playback range", params = "{}"),
    gui = "Timeline: the frame ruler's playback-range markers (M5 C5.1)",
)]
pub(crate) fn timeline_playback_range_get(cx: &mut Cx, _p: NoParams) -> Result<RangeResult, CommandError> {
    let (scene, len) = current(cx)?;
    let (mode, start, end) = effective_range(&cx.session().timeline.ranges, &scene, len);
    Ok(RangeResult { mode, start, end })
}

#[command(
    id = "timeline.playbackRange.changed",
    kind = event,
    label = "Playback range changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn timeline_playback_range_changed(_cx: &mut Cx, _p: NoParams) -> Result<RangeResult, CommandError> {
    Err(super::common::event_not_invocable("timeline.playbackRange.changed"))
}

// ----- timeline.rows.list ---------------------------------------------------------------------------------------------------

/// `timeline.rows.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RowsListParams {
    /// The scene (default the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// First frame of the window keyframes are listed in (default 1).
    #[serde(default)]
    #[schemars(range(min = 1))]
    from: Option<i64>,
    /// Last frame of the window (default the scene length).
    #[serde(default)]
    #[schemars(range(min = 1))]
    to: Option<i64>,
}

/// A row's bar.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RowBar {
    /// First frame (the first exposed frame of a cell-based row).
    start: u32,
    /// Last frame.
    end: u32,
    /// Loops (1 = not looped).
    loops: u32,
}

/// A keyframe marker.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RowKeyframe {
    /// Scene frame.
    frame: u32,
    /// Keyed transform channels: `position`, `scale`, `rotation`, `skew`.
    channels: Vec<String>,
    /// `constant` when every channel keyed here starts a constant segment, else `interpolated`.
    segment_after: String,
}

/// One Timeline row (04 §17.5).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Row {
    element_id: String,
    name: String,
    #[serde(rename = "type")]
    ty: String,
    depth: u32,
    parent_id: Option<String>,
    collapsed: bool,
    visible_in_scene: bool,
    hidden_by_parent: bool,
    locked: bool,
    /// Whether the Timeline shows the row: not under a collapsed parent, and not an effect-parameter row its effect's
    /// setting hides (04 §2.9, §9.4; the rows the timing-gesture interpreter hit-tests, in the same order).
    shown_in_timeline: bool,
    /// `null` for a cell-based row with no exposed cell.
    bar: Option<RowBar>,
    keyframes: Vec<RowKeyframe>,
    child_keyframe_frames: Vec<u32>,
    /// Sound clips (M8; empty until then).
    #[serde(skip_serializing_if = "Option::is_none")]
    clips: Option<Vec<Value>>,
}

/// `timeline.rows.list` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RowsResult {
    scene_id: String,
    scene_length: u32,
    current_frame: u32,
    playback_range: RangeResult,
    rows: Vec<Row>,
}

/// The transform channel a stored channel's keys count for in the Timeline's glyphs (04 line 627), if any.
fn glyph_channel(name: &str) -> Option<&'static str> {
    match name {
        "position" => Some("position"),
        "rotation" => Some("rotation"),
        "skew" => Some("skew"),
        n if n == "scale" || n.starts_with("scale.") => Some("scale"),
        _ => None,
    }
}

/// The keyed scene frames of one element with their glyph channels and constant flags.
fn keys_of(
    doc: &Document,
    id: &Id,
    budget: &mut Budget,
) -> Result<BTreeMap<u32, (BTreeSet<&'static str>, bool)>, CommandError> {
    let data = data_of(doc, id, budget)?;
    let edit = ChannelEdit::read(doc, id)?;
    let mut map: BTreeMap<u32, (BTreeSet<&'static str>, bool)> = BTreeMap::new();
    let mut put = |local: u32, ch: &'static str, constant: bool| {
        let scene = data.bar.scene(f64::from(local));
        if scene >= 1.0 && scene <= f64::from(u32::MAX) {
            let e = map.entry(scene as u32).or_insert((BTreeSet::new(), true));
            e.0.insert(ch);
            e.1 &= constant;
        }
    };
    for q in edit.points() {
        budget.spend(1)?;
        if let Some(f) = frame_of(&q) {
            put(f, "position", q["constantSegment"] == json!(true));
        }
    }
    for name in edit.channels.keys() {
        let Some(ch) = glyph_channel(name) else { continue };
        if ch == "position" {
            continue;
        }
        for key in edit.keys(name) {
            budget.spend(1)?;
            if let Some(f) = frame_of(&key) {
                put(f, ch, key["constantSegment"] == json!(true));
            }
        }
    }
    Ok(map)
}

/// The keyed scene frames of an element (the Timeline's keyframe symbols; the timing-gesture interpreter's hit test).
pub(crate) fn keyed_frames(doc: &Document, id: &Id, budget: &mut Budget) -> Result<Vec<u32>, CommandError> {
    Ok(keys_of(doc, id, budget)?.into_keys().collect())
}

#[command(
    id = "timeline.rows.list",
    kind = query,
    label = "List Timeline rows",
    scopes = [read],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "The current scene's rows", params = "{}"),
    example(title = "Keyframes of frames 1 to 24 only", params = r#"{"from": 1, "to": 24}"#),
    gui = "Timeline: the layer list and tracks (M5 C5.1)",
)]
pub(crate) fn timeline_rows_list(cx: &mut Cx, p: RowsListParams) -> Result<RowsResult, CommandError> {
    let scene = scene_id(cx, p.scene_id.as_deref())?;
    let doc = cx.document()?.clone();
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene not in the document"))?;
    let len = u32::try_from(doc.scene_length(sc)).unwrap_or(u32::MAX).max(1);
    let from = u32::try_from(p.from.unwrap_or(1).max(1)).unwrap_or(u32::MAX);
    let to = match p.to {
        Some(t) => u32::try_from(t.max(1)).unwrap_or(u32::MAX),
        None => len.max(from),
    };
    // A reversed window is refused as `xsheet.exposures.get` refuses it (04 line 26: a range `from..to` is inclusive;
    // line 1306 item 11: a failed command changes nothing; ruling E-26). The check also keeps `range` below from a
    // start after its end, which panics.
    if to < from {
        return Err(CommandError::with_details(
            codes::ARG_INVALID,
            "`to` is before `from`",
            json!({ "pointer": "/to" }),
        ));
    }
    let mut budget = Budget::new("listing Timeline rows");
    let gesture_state = crate::timing_gesture::State::of(&doc, cx.session());
    let shown: BTreeSet<Id> =
        crate::timing_gesture::visible_rows(&doc, &scene, &gesture_state).into_iter().map(|r| r.id).collect();
    let placed = h::placed(sc.tree());
    budget.spend(placed.len())?;
    let mut hidden: BTreeMap<Id, bool> = BTreeMap::new();
    let mut parent_of: BTreeMap<Id, Option<Id>> = BTreeMap::new();
    let mut keyed: BTreeMap<Id, Vec<u32>> = BTreeMap::new();
    let mut rows = Vec::with_capacity(placed.len());
    for q in &placed {
        let e = element(&doc, &q.id)?;
        let parent_hidden = q.parent.as_ref().is_some_and(|p| hidden.get(p).copied().unwrap_or(false));
        hidden.insert(q.id.clone(), parent_hidden || !*e.visible_in_scene());
        parent_of.insert(q.id.clone(), q.parent.clone());
        let bar = match (e.start(), e.duration()) {
            (Some(s), Some(d)) => {
                Some(RowBar { start: *s, end: (*s).saturating_add((*d).max(1)) - 1, loops: *e.loops() })
            }
            _ => super::element_read::exposed_extent(e).map(|(a, b)| RowBar {
                start: as_u32(a),
                end: as_u32(b),
                loops: *e.loops(),
            }),
        };
        let keys = keys_of(&doc, &q.id, &mut budget)?;
        keyed.insert(q.id.clone(), keys.keys().copied().collect());
        let keyframes = keys
            .range(from..=to)
            .map(|(f, (chs, constant))| RowKeyframe {
                frame: *f,
                channels: ["position", "scale", "rotation", "skew"]
                    .iter()
                    .filter(|c| chs.contains(*c))
                    .map(|c| (*c).to_owned())
                    .collect(),
                segment_after: if *constant { "constant" } else { "interpolated" }.to_owned(),
            })
            .collect();
        rows.push(Row {
            element_id: q.id.as_str().to_owned(),
            name: e.name().clone(),
            ty: e.element_type().as_str().to_owned(),
            depth: as_u32(q.depth),
            parent_id: q.parent.as_ref().map(|p| p.as_str().to_owned()),
            collapsed: *e.collapsed(),
            visible_in_scene: *e.visible_in_scene(),
            hidden_by_parent: parent_hidden,
            locked: *e.locked(),
            shown_in_timeline: shown.contains(&q.id),
            bar,
            keyframes,
            child_keyframe_frames: Vec::new(),
            clips: (*e.element_type() == ElementType::Sound).then(Vec::new),
        });
    }
    // A parent's child-keyframe markers: every frame a descendant has a key at (04 line 630).
    let mut child: BTreeMap<Id, BTreeSet<u32>> = BTreeMap::new();
    for (id, frames) in &keyed {
        let mut up = parent_of.get(id).cloned().flatten();
        while let Some(p) = up {
            budget.spend(frames.len())?;
            child.entry(p.clone()).or_default().extend(frames.iter().copied().filter(|f| (from..=to).contains(f)));
            up = parent_of.get(&p).cloned().flatten();
        }
    }
    for r in &mut rows {
        if let Ok(id) = Id::parse(&r.element_id)
            && let Some(set) = child.remove(&id)
        {
            r.child_keyframe_frames = set.into_iter().collect();
        }
    }
    let (mode, start, end) = effective_range(&cx.session().timeline.ranges, &scene, len);
    Ok(RowsResult {
        scene_id: scene.as_str().to_owned(),
        scene_length: len,
        current_frame: cx.app().frame.clamp(1, len),
        playback_range: RangeResult { mode, start, end },
        rows,
    })
}

// ----- timeline.frameLabel.get ---------------------------------------------------------------------------------------------

/// `timeline.frameLabel.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameLabelParams {
    /// Frame numbers.
    #[schemars(length(max = 100_000))]
    frames: Vec<i64>,
}

/// `{labels}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrameLabelResult {
    /// One label per frame, in order.
    labels: Vec<String>,
}

/// A frame as the time display shows it: `20`, or `1+04` in feet and frames (16 frames per foot, 04 line 644).
pub(crate) fn frame_label(frame: i64, feet: bool) -> String {
    if feet && frame >= 0 { format!("{}+{:02}", frame / 16, frame % 16) } else { frame.to_string() }
}

/// The time display preference is feet and frames.
pub(crate) fn feet_and_frames(cx: &Cx) -> bool {
    cx.pref(TIME_KEY).and_then(|v| v.as_str().map(|s| s == "feetAndFrames")).unwrap_or(false)
}

#[command(
    id = "timeline.frameLabel.get",
    kind = query,
    label = "Format frame numbers",
    scopes = [read],
    needs_project,
    example(title = "Frames 1, 16, 20", params = r#"{"frames": [1, 16, 20]}"#),
    agent_only = "agents format frame numbers as the time display shows them (04 §11.4); the window formats its ruler from the same preference",
)]
pub(crate) fn timeline_frame_label_get(cx: &mut Cx, p: FrameLabelParams) -> Result<FrameLabelResult, CommandError> {
    let feet = feet_and_frames(cx);
    Ok(FrameLabelResult { labels: p.frames.iter().map(|f| frame_label(*f, feet)).collect() })
}

// ----- preferences with a structured equivalent (SI-203, ruling E-6) ----------------------------------------------------

/// The time display key (01 §7.3).
pub(crate) const TIME_KEY: &str = "interface.timeMeasurement";
/// The Timeline background key (01 §7.3).
pub(crate) const BACKGROUND_KEY: &str = "interface.background.timeline";
/// The single-cell selection key (01 §7.3).
pub(crate) const SINGLE_CELL_KEY: &str = "timing.timeline.singleCellSelection";

/// Writes one preference key as `prefs.set` does (the same validation, the same write and `prefs.changed`).
pub(crate) fn write_pref(cx: &mut Cx, key: &str, value: Value, pointer: &str) -> Result<Value, CommandError> {
    let themes = cx.session().env.settings.theme_ids();
    let v = crate::prefs::validate(key, &value, &themes).map_err(|mut e| {
        if let Some(m) = e.details.as_object_mut() {
            m.insert("pointer".into(), json!(pointer));
        }
        e
    })?;
    let mut values = Map::new();
    values.insert(key.to_owned(), v.clone());
    super::prefs::stage(cx, values)?;
    Ok(v)
}

/// Time display unit (04 §11.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TimeUnit {
    /// Frame numbers.
    Frames,
    /// Feet and frames.
    FeetFrames,
}

/// `timeline.timeDisplay.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TimeDisplayParams {
    /// The unit.
    unit: TimeUnit,
}

/// `{unit}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TimeDisplayResult {
    unit: TimeUnit,
}

#[command(
    id = "timeline.timeDisplay.set",
    kind = ui,
    label = "Set time display",
    scopes = [ui],
    needs_project,
    example(title = "Feet and frames", params = r#"{"unit": "feetFrames"}"#),
    agent_only = "the structured form of the preference interface.timeMeasurement, which the Preferences dialog edits with prefs.set (ruling E-6)",
)]
pub(crate) fn timeline_time_display_set(cx: &mut Cx, p: TimeDisplayParams) -> Result<TimeDisplayResult, CommandError> {
    let v = match p.unit {
        TimeUnit::Frames => "frames",
        TimeUnit::FeetFrames => "feetAndFrames",
    };
    write_pref(cx, TIME_KEY, json!(v), "/unit")?;
    Ok(TimeDisplayResult { unit: p.unit })
}

/// `timeline.backgroundColour.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackgroundParams {
    /// `#RRGGBB` or `#RRGGBBAA`.
    #[schemars(regex(pattern = r"^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$"))]
    colour: String,
}

/// `{colour}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ColourResult {
    colour: String,
}

#[command(
    id = "timeline.backgroundColour.set",
    kind = ui,
    label = "Set Timeline background colour",
    scopes = [ui],
    needs_project,
    example(title = "A dark grey Timeline", params = r##"{"colour": "#303030"}"##),
    agent_only = "the structured form of the preference interface.background.timeline, which the Preferences dialog edits with prefs.set (ruling E-6)",
)]
pub(crate) fn timeline_background_colour_set(cx: &mut Cx, p: BackgroundParams) -> Result<ColourResult, CommandError> {
    let v = write_pref(cx, BACKGROUND_KEY, json!(p.colour), "/colour")?;
    Ok(ColourResult { colour: v.as_str().map_or(p.colour, str::to_owned) })
}

/// `{enabled}` parameters and result.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// On or off.
    pub enabled: bool,
}

#[command(
    id = "timeline.singleCellSelection.set",
    kind = toggle,
    label = "Single cell selection",
    scopes = [ui],
    needs_project,
    example(title = "Clicks select whole tracks", params = r#"{"enabled": false}"#),
    agent_only = "the structured form of the preference timing.timeline.singleCellSelection, which the Preferences dialog edits with prefs.set (ruling E-6)",
)]
pub(crate) fn timeline_single_cell_selection_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    write_pref(cx, SINGLE_CELL_KEY, json!(p.enabled), "/enabled")?;
    Ok(p)
}

// ----- display toggles (session state) -------------------------------------------------------------------------------------

#[command(
    id = "timeline.soundWaveforms.set",
    kind = toggle,
    label = "Sound waveforms",
    scopes = [ui],
    needs_project,
    example(title = "Solid bars for sound rows", params = r#"{"enabled": false}"#),
    gui = "Timeline row and ruler context: Sound Waveforms (shown with its owner M8 until sound rows exist; ruling E-9)",
)]
pub(crate) fn timeline_sound_waveforms_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    cx.session_mut().timeline.waveforms_off = !p.enabled;
    Ok(p)
}

/// `timeline.effectParameterRows.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ParameterRowsParams {
    /// Shown or hidden.
    visible: bool,
    /// One effect element (omitted: every effect element, and the default for new ones).
    #[serde(default)]
    element_id: Option<String>,
}

/// `{visible}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct VisibleResult {
    visible: bool,
}

#[command(
    id = "timeline.effectParameterRows.set",
    kind = toggle,
    label = "Show effect parameters",
    scopes = [ui],
    needs_project,
    example(title = "Hide every effect's parameter rows", params = r#"{"visible": false}"#),
    gui = "Timeline row context: Show Effect Parameters, Show All / Hide All Effect Parameters (M5 C5.1)",
)]
pub(crate) fn timeline_effect_parameter_rows_set(
    cx: &mut Cx,
    p: ParameterRowsParams,
) -> Result<VisibleResult, CommandError> {
    match &p.element_id {
        Some(r) => {
            let id = element_id(cx, r, "/elementId")?;
            let ty = *element(cx.document()?, &id)?.element_type();
            if !matches!(
                ty,
                ElementType::Clipping
                    | ElementType::ColourTransform
                    | ElementType::DropShadow
                    | ElementType::Blur
                    | ElementType::Shake
                    | ElementType::ParticleNature
                    | ElementType::ParticleImpact
            ) {
                return Err(err(
                    codes::ARG_INVALID,
                    format!("`{}` is not an effect element", id.as_str()),
                    json!({ "pointer": "/elementId", "reason": "notEffect", "elementType": ty.as_str() }),
                ));
            }
            cx.session_mut().timeline.parameter_rows.insert(id.as_str().to_owned(), p.visible);
        }
        None => {
            let t = &mut cx.session_mut().timeline;
            t.parameter_rows.clear();
            t.parameter_rows_hidden = !p.visible;
        }
    }
    Ok(VisibleResult { visible: p.visible })
}

// ----- timeline.view.set --------------------------------------------------------------------------------------------------

/// `timeline.view.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ViewSetParams {
    /// Zoom level: an index into the zoom ladder (0 = narrowest cells).
    #[serde(default)]
    #[schemars(range(min = 0))]
    zoom_level: Option<i64>,
    /// First frame shown: any integer, clamped to 1..(scene length + the empty margin).
    #[serde(default)]
    first_frame: Option<i64>,
    /// Scroll so this element's row is shown (Show Timeline Selection).
    #[serde(default)]
    reveal_element_id: Option<String>,
}

/// `{zoomLevel, firstFrame, lastFrame}`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewResult {
    /// Zoom level.
    pub zoom_level: u32,
    /// First frame shown.
    pub first_frame: u32,
    /// Last frame shown.
    pub last_frame: f64,
}

/// The Timeline view of `scene` as `timeline.view.set` answers it.
pub(crate) fn view_of(t: &super::timing::TimelineState, scene: &Id) -> ViewResult {
    let layout = crate::timeline_layout::layout();
    let v = t.views.get(scene.as_str()).copied().unwrap_or_default();
    let width = t.viewport.unwrap_or(layout.headless_viewport_width);
    let visible = layout.visible_frames(width, v.zoom);
    ViewResult {
        zoom_level: v.zoom,
        first_frame: v.first_frame,
        last_frame: f64::from(v.first_frame.saturating_add(visible) - 1),
    }
}

#[command(
    id = "timeline.view.set",
    kind = ui,
    label = "Set Timeline view",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Zoom in and start at frame 48", params = r#"{"zoomLevel": 8, "firstFrame": 48}"#),
    example(title = "Read the view without changing it", params = "{}"),
    gui = "Timeline: zoom control, horizontal scroll, View > Zoom In / Out with the Timeline focused, Show Timeline Selection (M5 C5.1)",
)]
pub(crate) fn timeline_view_set(cx: &mut Cx, p: ViewSetParams) -> Result<ViewResult, CommandError> {
    let (scene, len) = current(cx)?;
    let layout = crate::timeline_layout::layout();
    let mut v = cx.session().timeline.views.get(scene.as_str()).copied().unwrap_or_default();
    if let Some(z) = p.zoom_level {
        let max = layout.max_zoom();
        if !(0..=i64::from(max)).contains(&z) {
            return Err(err(
                codes::ARG_INVALID,
                format!("zoom level {z} is outside the ladder 0 to {max}"),
                json!({ "pointer": "/zoomLevel", "allowed": { "minimum": 0, "maximum": max } }),
            ));
        }
        v.zoom = u32::try_from(z).unwrap_or(0);
    }
    let last_start = len.saturating_add(layout.empty_frames_past_end).max(1);
    if let Some(f) = p.first_frame {
        v.first_frame = u32::try_from(f.clamp(1, i64::from(last_start))).unwrap_or(1);
    }
    if let Some(r) = &p.reveal_element_id {
        let id = element_id(cx, r, "/revealElementId")?;
        let doc = cx.document()?;
        if doc.scene_of(&id) != Some(&scene) {
            return Err(err(
                codes::ARG_INVALID,
                "the element is not in the current scene",
                json!({ "pointer": "/revealElementId", "reason": "notCurrentScene" }),
            ));
        }
        let e = element(doc, &id)?;
        let extent = match (e.start(), e.duration()) {
            (Some(s), Some(d)) => Some((*s, (*s).saturating_add((*d).max(1)) - 1)),
            _ => super::element_read::exposed_extent(e).map(|(a, b)| (as_u32(a), as_u32(b))),
        };
        let width = cx.session().timeline.viewport.unwrap_or(layout.headless_viewport_width);
        let shown_last = v.first_frame.saturating_add(layout.visible_frames(width, v.zoom)) - 1;
        if let Some((a, b)) = extent
            && (b < v.first_frame || a > shown_last)
        {
            v.first_frame = a.max(1);
        }
        let t = &mut cx.session_mut().timeline;
        t.reveal_count = t.reveal_count.wrapping_add(1);
        t.reveal = Some(id.as_str().to_owned());
    }
    cx.session_mut().timeline.views.insert(scene.as_str().to_owned(), v);
    Ok(view_of(&cx.session().timeline, &scene))
}

/// The host-only notice of the Timeline's view state (`shell.timelineView`; SI-210): what `timeline.view.set {}`
/// answers, plus the display toggles. Never sent to an MCP client.
pub(crate) fn view_notice(t: &super::timing::TimelineState, scene: Option<&Id>) -> Value {
    let view = scene.map(|s| view_of(t, s));
    json!({
        "sceneId": scene.map(Id::as_str),
        "view": view,
        "revealElementId": t.reveal,
        "revealCount": t.reveal_count,
        "soundWaveforms": !t.waveforms_off,
        "effectParameterRowsHidden": t.parameter_rows_hidden,
        "effectParameterRows": t.parameter_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feet_and_frames_follow_the_design_decision() {
        assert_eq!(frame_label(20, true), "1+04");
        assert_eq!(frame_label(16, true), "1+00");
        assert_eq!(frame_label(1, true), "0+01");
        assert_eq!(frame_label(9999, true), "624+15");
        assert_eq!(frame_label(20, false), "20");
    }
}
