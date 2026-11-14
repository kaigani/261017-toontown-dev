//! Drawing aids (02 §10, §13.5; M4 C4.2): grid, onion skin and its style, the auto and static light tables, light-table
//! shading, Show Strokes, current drawing on top, Camera-view selection display and the Drawing-view background. The
//! aids are user-scope drawing settings ([`crate::drawing`], persisted with them, never undoable); the static light
//! table is per-scene session state shared with the Exposure Sheet's light table (SI-085, SI-126).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::Id;
use tt_model::entities::element::ElementType;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timing::LightItem;
use crate::app::AppContext;
use crate::cx::Cx;
use crate::drawing::Aids;

fn err(code: &str, message: impl Into<String>, details: Value) -> CommandError {
    CommandError::with_details(code, message, details)
}

fn aid_json(a: &Aids, aid: &str) -> Value {
    serde_json::to_value(a).ok().and_then(|v| v.get(aid).cloned()).unwrap_or(Value::Null)
}

/// Applies `f` to the aids and announces `drawAid.changed` for `aid` when it changed something.
fn change(cx: &mut Cx, aid: &str, f: impl FnOnce(&mut Aids)) -> Value {
    let before = cx.ui().drawing().aids.clone();
    f(&mut cx.ui_mut().drawing_mut().aids);
    let after = cx.ui().drawing().aids.clone();
    let state = aid_json(&after, aid);
    if after != before {
        cx.emit("drawAid.changed", json!({ "aid": aid, "state": state }));
    }
    state
}

fn colour(v: &str, pointer: &str) -> Result<String, CommandError> {
    tt_model::value::Colour::parse(v)
        .map(|c| c.to_string())
        .map_err(|e| err(codes::ARG_INVALID, e.message().to_owned(), json!({ "pointer": pointer })))
}

// ----- event ----------------------------------------------------------------------------------------------------------

/// `drawAid.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct AidChanged {
    /// The aid (`grid`, `onionSkin`, `onionSkinStyle`, `autoLightTable`, `staticLightTable`, `lightTableShade`,
    /// `staticLightTablePanel`, `strokes`, `currentOnTop`, `selectionDisplay`, `background`).
    aid: String,
    /// Its state now.
    state: Value,
}

#[command(
    id = "drawAid.changed",
    kind = event,
    label = "A drawing aid setting changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn draw_aid_changed(_cx: &mut Cx, _p: NoParams) -> Result<AidChanged, CommandError> {
    Err(super::common::event_not_invocable("drawAid.changed"))
}

// ----- state ----------------------------------------------------------------------------------------------------------

/// `drawAid.state.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AidState {
    /// Grid.
    grid: crate::drawing::Grid,
    /// Onion skin.
    onion_skin: crate::drawing::OnionSkin,
    /// Onion-skin appearance.
    onion_skin_style: crate::drawing::OnionSkinStyle,
    /// Auto light table.
    auto_light_table: bool,
    /// The current scene's static light table.
    static_light_table: Vec<Entry>,
    /// Light-table shading.
    light_table_shade: crate::drawing::Shade,
    /// Show Strokes.
    strokes: bool,
    /// Current drawing on top.
    current_on_top: bool,
    /// Selection display.
    selection_display: crate::drawing::SelectionDisplay,
    /// Background.
    background: crate::drawing::Background,
}

#[command(
    id = "drawAid.state.get",
    kind = query,
    label = "Get drawing aids",
    scopes = [read],
    needs_project,
    example(title = "The state of every drawing aid", params = "{}"),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_state_get(cx: &mut Cx, _p: NoParams) -> Result<AidState, CommandError> {
    let a = cx.ui().drawing().aids.clone();
    let scene = cx.app().scene.clone();
    Ok(AidState {
        grid: a.grid,
        onion_skin: a.onion_skin,
        onion_skin_style: a.onion_skin_style,
        auto_light_table: a.auto_light_table,
        static_light_table: scene.map(|s| entries(cx, &s)).unwrap_or_default(),
        light_table_shade: a.light_table_shade,
        strokes: a.strokes,
        current_on_top: a.current_on_top,
        selection_display: a.selection_display,
        background: a.background,
    })
}

// ----- grid, onion skin -----------------------------------------------------------------------------------------------

/// `drawAid.grid.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GridParams {
    /// Shown.
    #[serde(default)]
    visible: Option<bool>,
    /// The grid type.
    #[serde(default, rename = "type")]
    ty: Option<GridType>,
    /// Drawn over the drawings.
    #[serde(default)]
    overlay: Option<bool>,
}

/// Grid type (02 §10.1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GridType {
    /// Square grid.
    Normal,
    /// 12-field.
    Field12,
    /// 16-field.
    Field16,
}

fn name_of<T: Serialize>(v: T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

#[command(
    id = "drawAid.grid.set",
    kind = ui,
    label = "Grid",
    scopes = [ui],
    needs_project,
    example(title = "Show the 12-field grid", params = r#"{"visible": true, "type": "field12"}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_grid_set(cx: &mut Cx, p: GridParams) -> Result<Value, CommandError> {
    Ok(change(cx, "grid", |a| {
        if let Some(v) = p.visible {
            a.grid.visible = v;
        }
        if let Some(t) = p.ty {
            a.grid.ty = name_of(t);
        }
        if let Some(o) = p.overlay {
            a.grid.overlay = o;
        }
    }))
}

/// `drawAid.onionSkin.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OnionParams {
    /// Master switch.
    #[serde(default)]
    enabled: Option<bool>,
    /// Previous drawings shown (0-3).
    #[serde(default)]
    #[schemars(range(min = 0, max = 3))]
    previous: Option<i64>,
    /// Next drawings shown (0-3).
    #[serde(default)]
    #[schemars(range(min = 0, max = 3))]
    next: Option<i64>,
}

fn count(v: Option<i64>, pointer: &str) -> Result<Option<i64>, CommandError> {
    match v {
        Some(n) if !(0..=3).contains(&n) => {
            Err(err("tool.valueOutOfRange", "onion-skin counts are 0-3", json!({ "pointer": pointer })))
        }
        _ => Ok(v),
    }
}

#[command(
    id = "drawAid.onionSkin.set",
    kind = ui,
    label = "Onion skin",
    scopes = [ui],
    needs_project,
    example(title = "Onion skin with two previous drawings", params = r#"{"enabled": true, "previous": 2}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_onion_skin_set(cx: &mut Cx, p: OnionParams) -> Result<Value, CommandError> {
    let prev = count(p.previous, "/previous")?;
    let next = count(p.next, "/next")?;
    Ok(change(cx, "onionSkin", |a| {
        if let Some(e) = p.enabled {
            a.onion_skin.enabled = e;
        }
        if let Some(n) = prev {
            a.onion_skin.previous = n;
        }
        if let Some(n) = next {
            a.onion_skin.next = n;
        }
    }))
}

/// `drawAid.onionSkinStyle.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StyleParams {
    /// Outlines only.
    #[serde(default)]
    outline: Option<bool>,
    /// Semi-transparent.
    #[serde(default)]
    transparency: Option<bool>,
    /// Tinted.
    #[serde(default)]
    colour_shading: Option<bool>,
    /// Tint of previous drawings (`#RRGGBB` or `#RRGGBBAA`).
    #[serde(default)]
    previous_colour: Option<String>,
    /// Tint of next drawings.
    #[serde(default)]
    next_colour: Option<String>,
}

/// `{style}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StyleResult {
    /// The appearance now.
    style: Value,
}

#[command(
    id = "drawAid.onionSkinStyle.set",
    kind = ui,
    label = "Onion-skin appearance",
    scopes = [ui],
    needs_project,
    example(title = "Outlined onion skins", params = r#"{"outline": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_onion_skin_style_set(cx: &mut Cx, p: StyleParams) -> Result<StyleResult, CommandError> {
    let prev = p.previous_colour.as_deref().map(|c| colour(c, "/previousColour")).transpose()?;
    let next = p.next_colour.as_deref().map(|c| colour(c, "/nextColour")).transpose()?;
    let style = change(cx, "onionSkinStyle", |a| {
        let s = &mut a.onion_skin_style;
        if let Some(v) = p.outline {
            s.outline = v;
        }
        if let Some(v) = p.transparency {
            s.transparency = v;
        }
        if let Some(v) = p.colour_shading {
            s.colour_shading = v;
        }
        if let Some(c) = prev {
            s.previous_colour = c;
        }
        if let Some(c) = next {
            s.next_colour = c;
        }
    });
    Ok(StyleResult { style })
}

/// `drawAid.onionSkinCycle.run` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CycleParams {
    /// Which count to cycle.
    direction: Direction,
}

/// Onion-skin direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Direction {
    /// Previous drawings.
    Previous,
    /// Next drawings.
    Next,
}

/// `{previous, next}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Counts {
    /// Previous drawings shown.
    previous: i64,
    /// Next drawings shown.
    next: i64,
}

#[command(
    id = "drawAid.onionSkinCycle.run",
    kind = ui,
    label = "Cycle onion skin",
    scopes = [ui],
    needs_project,
    example(title = "One more previous drawing (3 wraps to 0)", params = r#"{"direction": "previous"}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_onion_skin_cycle_run(cx: &mut Cx, p: CycleParams) -> Result<Counts, CommandError> {
    change(cx, "onionSkin", |a| {
        let o = &mut a.onion_skin;
        let slot = if p.direction == Direction::Previous { &mut o.previous } else { &mut o.next };
        *slot = ((*slot).clamp(0, 3) + 1) % 4;
    });
    let o = &cx.ui().drawing().aids.onion_skin;
    Ok(Counts { previous: o.previous, next: o.next })
}

// ----- toggles --------------------------------------------------------------------------------------------------------

/// `{enabled}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Enabled {
    /// On.
    enabled: bool,
}

/// `{visible}`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Visible {
    /// Shown.
    visible: bool,
}

#[command(
    id = "drawAid.autoLightTable.set",
    kind = ui,
    label = "Auto light table",
    scopes = [ui],
    needs_project,
    example(title = "Show the other elements' drawings", params = r#"{"enabled": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_auto_light_table_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    change(cx, "autoLightTable", |a| a.auto_light_table = p.enabled);
    Ok(p)
}

/// `drawAid.lightTableShade.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShadeParams {
    /// Shade the auto light table.
    #[serde(default)]
    auto: Option<bool>,
    /// Shade static light-table entries.
    #[serde(default, rename = "static")]
    stat: Option<bool>,
}

#[command(
    id = "drawAid.lightTableShade.set",
    kind = ui,
    label = "Light-table shading",
    scopes = [ui],
    needs_project,
    example(title = "Unshaded static entries", params = r#"{"static": false}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_light_table_shade_set(cx: &mut Cx, p: ShadeParams) -> Result<Value, CommandError> {
    Ok(change(cx, "lightTableShade", |a| {
        if let Some(v) = p.auto {
            a.light_table_shade.auto = v;
        }
        if let Some(v) = p.stat {
            a.light_table_shade.stat = v;
        }
    }))
}

#[command(
    id = "drawAid.staticLightTablePanel.set",
    kind = ui,
    label = "Static light-table panel",
    scopes = [ui],
    needs_project,
    example(title = "Show the panel", params = r#"{"visible": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_static_light_table_panel_set(cx: &mut Cx, p: Visible) -> Result<Visible, CommandError> {
    change(cx, "staticLightTablePanel", |a| a.static_light_table_panel = p.visible);
    Ok(p)
}

#[command(
    id = "drawAid.strokes.set",
    kind = ui,
    label = "Show Strokes",
    scopes = [ui],
    needs_project,
    example(title = "Show invisible strokes and open ends", params = r#"{"visible": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_strokes_set(cx: &mut Cx, p: Visible) -> Result<Visible, CommandError> {
    change(cx, "strokes", |a| a.strokes = p.visible);
    Ok(p)
}

#[command(
    id = "drawAid.currentOnTop.set",
    kind = ui,
    label = "Current drawing on top",
    scopes = [ui],
    needs_project,
    example(title = "Edit with the current drawing on top", params = r#"{"enabled": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_current_on_top_set(cx: &mut Cx, p: Enabled) -> Result<Enabled, CommandError> {
    change(cx, "currentOnTop", |a| a.current_on_top = p.enabled);
    Ok(p)
}

/// `drawAid.selectionDisplay.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectionDisplayParams {
    /// Outline on selection.
    #[serde(default)]
    outline_on_selection: Option<bool>,
    /// Box highlighting.
    #[serde(default)]
    box_highlighting: Option<bool>,
    /// Keep the selection visible.
    #[serde(default)]
    keep_selection_visible: Option<bool>,
}

/// `{settings}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SettingsResult {
    /// The selection display now.
    settings: Value,
}

#[command(
    id = "drawAid.selectionDisplay.set",
    kind = ui,
    label = "Selection display",
    scopes = [ui],
    needs_project,
    example(title = "Outline selected elements", params = r#"{"outlineOnSelection": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_selection_display_set(
    cx: &mut Cx,
    p: SelectionDisplayParams,
) -> Result<SettingsResult, CommandError> {
    let settings = change(cx, "selectionDisplay", |a| {
        let s = &mut a.selection_display;
        if let Some(v) = p.outline_on_selection {
            s.outline_on_selection = v;
        }
        if let Some(v) = p.box_highlighting {
            s.box_highlighting = v;
        }
        if let Some(v) = p.keep_selection_visible {
            s.keep_selection_visible = v;
        }
    });
    Ok(SettingsResult { settings })
}

/// `drawAid.background.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BackgroundParams {
    /// Colour (`#RRGGBB` or `#RRGGBBAA`).
    #[serde(default)]
    colour: Option<String>,
    /// Use the scene's background colour.
    #[serde(default)]
    use_scene_colour: Option<bool>,
}

#[command(
    id = "drawAid.background.set",
    kind = ui,
    label = "Drawing View background",
    scopes = [ui],
    needs_project,
    example(title = "A white background", params = r##"{"colour": "#FFFFFF"}"##),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_background_set(cx: &mut Cx, p: BackgroundParams) -> Result<Value, CommandError> {
    let c = p.colour.as_deref().map(|c| colour(c, "/colour")).transpose()?;
    Ok(change(cx, "background", |a| {
        if let Some(c) = c {
            a.background.colour = c;
        }
        if let Some(u) = p.use_scene_colour {
            a.background.use_scene_colour = u;
        }
    }))
}

// ----- static light table ---------------------------------------------------------------------------------------------

/// Overlay or underlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Placement {
    /// Over the current drawing.
    Overlay,
    /// Under it.
    Underlay,
}

impl Placement {
    /// The stored form shared with the Exposure Sheet's light table (`over` / `under`, SI-085).
    fn stored(self) -> &'static str {
        match self {
            Placement::Overlay => "over",
            Placement::Underlay => "under",
        }
    }

    fn of(s: &str) -> Self {
        if s == "over" { Placement::Overlay } else { Placement::Underlay }
    }
}

/// One static light-table entry.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    /// Entry id.
    entry_id: String,
    /// The source drawing.
    source_id: String,
    /// `drawing` or `image` (by the source element's type).
    source_kind: String,
    /// Shown.
    visible: bool,
    /// Overlay or underlay.
    placement: Placement,
}

/// A scene's entries whose sources still exist (02 §10.4: a deleted source removes its entry).
fn entries(cx: &Cx, scene: &Id) -> Vec<Entry> {
    let Ok(doc) = cx.document() else { return Vec::new() };
    cx.session()
        .timeline
        .light_tables
        .get(scene.as_str())
        .map(|items| {
            items
                .iter()
                .filter_map(|it| {
                    doc.drawing(&Id::parse(&it.drawing_id).ok()?)?;
                    let e = doc.element(&Id::parse(&it.element_id).ok()?)?;
                    let kind = if *e.element_type() == ElementType::Image { "image" } else { "drawing" };
                    Some(Entry {
                        entry_id: it.item_id.clone(),
                        source_id: it.drawing_id.clone(),
                        source_kind: kind.to_owned(),
                        visible: it.visible,
                        placement: Placement::of(&it.placement),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Drops entries whose sources are gone, then announces the scene's light table.
fn announce(cx: &mut Cx, scene: &Id) -> Vec<Entry> {
    let live = entries(cx, scene);
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        items.retain(|it| live.iter().any(|e| e.entry_id == it.item_id));
    }
    cx.emit("drawAid.changed", json!({ "aid": "staticLightTable", "state": live }));
    live
}

/// `drawAid.staticLightTable.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AddParams {
    /// The drawing (default: the drawing at `elementId` and `frame`, else the current drawing).
    #[serde(default)]
    drawing_id: Option<String>,
    /// The element whose cell is sent (default: the current element).
    #[serde(default)]
    element_id: Option<String>,
    /// The frame of that cell (default: the current frame).
    #[serde(default)]
    frame: Option<u32>,
    /// Overlay or underlay.
    #[serde(default = "underlay")]
    placement: Placement,
    /// Shown.
    #[serde(default = "yes")]
    visible: bool,
}

fn underlay() -> Placement {
    Placement::Underlay
}

fn yes() -> bool {
    true
}

/// `{entryId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryIdResult {
    /// The new entry.
    entry_id: String,
}

/// The upper bound on entries per scene (SI-131).
pub(crate) const MAX_ENTRIES: usize = 256;

/// `drawAid.staticLightTable.add`: without `drawingId` or `elementId`, the current element's cell at the frame must
/// expose a drawing (O(1): one exposure lookup).
pub(crate) fn precheck_add(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    if v.get("drawingId").is_some() || v.get("elementId").is_some() {
        return Ok(());
    }
    let doc = cx.document()?;
    let frame = v.get("frame").and_then(Value::as_u64).and_then(|f| u32::try_from(f).ok()).unwrap_or(cx.app().frame);
    let exposed = cx.app().element.as_ref().and_then(|e| AppContext::exposed_drawing(doc, e, frame.max(1)));
    if exposed.is_none() {
        return Err(err(
            "drawing.noTarget",
            "there is no current drawing to send to the static light table",
            json!({}),
        ));
    }
    Ok(())
}

/// The light-table edits act on the current scene; `list` on `sceneId` or the current scene.
pub(crate) fn precheck_scene(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    super::timing::scene_id(cx, v.get("sceneId").and_then(Value::as_str)).map(|_| ())
}

/// `remove` and `set` name their entries (`entryIds`, `all`, or for `set` `onlyVisibleEntryId`) and act on the
/// current scene: a bare reference is refused as the handler refuses it.
pub(crate) fn precheck_entries(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let named = v.get("entryIds").is_some_and(|x| !x.is_null())
        || v.get("all") == Some(&Value::Bool(true))
        || v.get("onlyVisibleEntryId").is_some_and(|x| !x.is_null());
    if !named {
        return Err(err(codes::ARG_INVALID, "pass entryIds or all", json!({ "pointer": "/entryIds" })));
    }
    precheck_scene(cx, v)
}

#[command(
    id = "drawAid.staticLightTable.add",
    precheck = precheck_add,
    kind = action,
    label = "Send to static light table",
    scopes = [ui],
    needs_project,
    example(title = "Bob's frame 1 under the work", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 1}"#),
    example(title = "Bob's frame 3 over the work, hidden", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 3, "placement": "overlay", "visible": false}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_static_light_table_add(cx: &mut Cx, p: AddParams) -> Result<EntryIdResult, CommandError> {
    let doc = cx.document()?.clone();
    let (drawing, element) = if let Some(d) = &p.drawing_id {
        let d = super::timing::drawing_id(cx, d, "/drawingId")?;
        let element = match &p.element_id {
            Some(e) => super::timing::element_id(cx, e, "/elementId")?,
            None => doc.bank_of(&d).and_then(|b| doc.bank_users(b).first().cloned()).ok_or_else(|| {
                err("drawing.notFound", "the drawing has no element", json!({ "pointer": "/drawingId" }))
            })?,
        };
        (d, element)
    } else {
        let element = match &p.element_id {
            Some(e) => super::timing::element_id(cx, e, "/elementId")?,
            None => cx.app().element.clone().ok_or_else(|| {
                err("drawing.noTarget", "there is no current element and no drawing or cell was given", json!({}))
                    .hint("pass drawingId, or elementId and frame")
            })?,
        };
        let frame = p.frame.unwrap_or(cx.app().frame).max(1);
        let d = AppContext::exposed_drawing(&doc, &element, frame).ok_or_else(|| {
            err("drawing.noTarget", format!("the cell at frame {frame} is empty"), json!({ "pointer": "/frame" }))
        })?;
        (d, element)
    };
    let scene = doc
        .scene_of(&element)
        .cloned()
        .ok_or_else(|| err("element.notFound", "the element has no scene", json!({ "pointer": "/elementId" })))?;
    let table = cx.session_mut().timeline.light_tables.entry(scene.as_str().to_owned()).or_default();
    if table.len() >= MAX_ENTRIES {
        return Err(err(
            codes::LIMIT_EXCEEDED,
            format!("a scene's static light table holds at most {MAX_ENTRIES} entries"),
            json!({ "limit": "staticLightTableEntries", "max": MAX_ENTRIES }),
        ));
    }
    let n = table.iter().filter_map(|it| it.item_id.strip_prefix("lt")?.parse::<u64>().ok()).max().unwrap_or(0) + 1;
    let entry_id = format!("lt{n}");
    table.push(LightItem {
        item_id: entry_id.clone(),
        element_id: element.as_str().to_owned(),
        drawing_id: drawing.as_str().to_owned(),
        visible: p.visible,
        placement: p.placement.stored().to_owned(),
    });
    announce(cx, &scene);
    Ok(EntryIdResult { entry_id })
}

/// The current scene (light-table edits act on it).
fn current_scene(cx: &Cx) -> Result<Id, CommandError> {
    super::timing::scene_id(cx, None)
}

/// Resolves `entryIds` / `all` to the entries of the current scene.
fn chosen(cx: &Cx, scene: &Id, ids: Option<&[String]>, all: bool) -> Result<Vec<String>, CommandError> {
    let live = entries(cx, scene);
    match (ids, all) {
        (_, true) => Ok(live.into_iter().map(|e| e.entry_id).collect()),
        (Some(ids), false) => {
            for (i, id) in ids.iter().enumerate() {
                if !live.iter().any(|e| &e.entry_id == id) {
                    return Err(err(
                        tt_model::codes::REF_NOT_FOUND,
                        format!("no static light-table entry `{id}`"),
                        json!({ "pointer": format!("/entryIds/{i}"), "ref": id }),
                    ));
                }
            }
            Ok(ids.to_vec())
        }
        (None, false) => Err(err(codes::ARG_INVALID, "pass entryIds or all", json!({ "pointer": "/entryIds" }))),
    }
}

/// `drawAid.staticLightTable.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RemoveParams {
    /// Entries to delete.
    #[serde(default)]
    #[schemars(length(max = 256))]
    entry_ids: Option<Vec<String>>,
    /// Delete every entry.
    #[serde(default)]
    all: bool,
}

/// `{removedEntryIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Removed {
    /// The entries removed (sources untouched).
    removed_entry_ids: Vec<String>,
}

#[command(
    id = "drawAid.staticLightTable.remove",
    precheck = precheck_entries,
    kind = action,
    label = "Delete static light-table entries",
    scopes = [ui],
    needs_project,
    example(title = "Delete every entry", params = r#"{"all": true}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_static_light_table_remove(cx: &mut Cx, p: RemoveParams) -> Result<Removed, CommandError> {
    let scene = current_scene(cx)?;
    let ids = chosen(cx, &scene, p.entry_ids.as_deref(), p.all)?;
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        items.retain(|it| !ids.contains(&it.item_id));
    }
    announce(cx, &scene);
    Ok(Removed { removed_entry_ids: ids })
}

/// `drawAid.staticLightTable.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetParams {
    /// Entries to change.
    #[serde(default)]
    #[schemars(length(max = 256))]
    entry_ids: Option<Vec<String>>,
    /// Change every entry.
    #[serde(default)]
    all: bool,
    /// Show or hide them.
    #[serde(default)]
    visible: Option<bool>,
    /// Overlay or underlay them.
    #[serde(default)]
    placement: Option<Placement>,
    /// Show only this entry (hide all others).
    #[serde(default)]
    only_visible_entry_id: Option<String>,
}

/// `{entries}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntriesResult {
    /// The scene's entries now.
    entries: Vec<Entry>,
}

#[command(
    id = "drawAid.staticLightTable.set",
    precheck = precheck_entries,
    kind = action,
    label = "Change static light-table entries",
    scopes = [ui],
    needs_project,
    example(title = "Overlay all", params = r#"{"all": true, "placement": "overlay"}"#),
    example(title = "Hide all", params = r#"{"all": true, "visible": false}"#),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_static_light_table_set(cx: &mut Cx, p: SetParams) -> Result<EntriesResult, CommandError> {
    let scene = current_scene(cx)?;
    let only = match &p.only_visible_entry_id {
        Some(id) => {
            chosen(cx, &scene, Some(std::slice::from_ref(id)), false)
                .map_err(|e| err(&e.code, e.message, json!({ "pointer": "/onlyVisibleEntryId", "ref": id })))?;
            Some(id.clone())
        }
        None => None,
    };
    let ids = if p.entry_ids.is_none() && !p.all && only.is_some() {
        Vec::new()
    } else {
        chosen(cx, &scene, p.entry_ids.as_deref(), p.all)?
    };
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        for it in items.iter_mut() {
            if ids.contains(&it.item_id) {
                if let Some(v) = p.visible {
                    it.visible = v;
                }
                if let Some(pl) = p.placement {
                    it.placement = pl.stored().to_owned();
                }
            }
            if let Some(o) = &only {
                it.visible = &it.item_id == o;
            }
        }
    }
    Ok(EntriesResult { entries: announce(cx, &scene) })
}

/// `drawAid.staticLightTable.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// The scene (default: the current scene).
    #[serde(default)]
    scene_id: Option<String>,
}

#[command(
    id = "drawAid.staticLightTable.list",
    precheck = precheck_scene,
    kind = query,
    label = "List static light table",
    scopes = [read],
    needs_project,
    example(title = "The current scene's entries", params = "{}"),
    gui = "the Drawing view's aid controls (02 §10); wired in M4 C4.2",
)]
pub(crate) fn draw_aid_static_light_table_list(cx: &mut Cx, p: ListParams) -> Result<EntriesResult, CommandError> {
    let scene = super::timing::scene_id(cx, p.scene_id.as_deref())?;
    Ok(EntriesResult { entries: entries(cx, &scene) })
}
