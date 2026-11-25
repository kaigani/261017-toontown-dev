//! The Exposure Sheet's view-state commands (04 §17.4; M5): `xsheet.currentCell.set` (C5.1a, the Timeline's and the
//! Exposure Sheet's cell click). C5.2 adds `xsheet.display.set`, `xsheet.columnWidth.set`, `xsheet.panel.set` and the
//! light-table commands here.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::ElementType;
use tt_registry::{CommandError, codes, command};

use super::timing::{LightItem, element_id, err};
use crate::app::AppContext;
use crate::cx::Cx;

// ----- xsheet.currentCell.set (M5; the Timeline's cell click, 04 line 662) -----------------------------------------------

/// `xsheet.currentCell.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentCellParams {
    /// The element.
    element_id: String,
    /// The frame (frames past the scene end may be current: 04 line 676).
    #[schemars(range(min = 1, max = 100_000))]
    frame: i64,
}

/// `{elementId, frame}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentCellResult {
    element_id: String,
    #[schemars(range(min = 1))]
    frame: u32,
}

#[command(
    id = "xsheet.currentCell.set",
    kind = ui,
    label = "Set current cell",
    scopes = [ui],
    needs_project,
    example(title = "Bob at frame 3", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 3}"#),
    gui = "Timeline and Exposure Sheet: click a cell (M5 C5.1, C5.2)",
)]
pub(crate) fn xsheet_current_cell_set(cx: &mut Cx, p: CurrentCellParams) -> Result<CurrentCellResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let frame = u32::try_from(p.frame).ok().filter(|f| (1..=100_000).contains(f)).ok_or_else(|| {
        err(
            "cell.frameOutOfRange",
            format!("frame {} is outside 1 to 100000", p.frame),
            json!({ "pointer": "/frame", "allowed": { "minimum": 1, "maximum": 100_000 } }),
        )
    })?;
    super::element_read::put_current_element(cx, &id)?;
    super::timeline::frame_source(cx, "command");
    super::timeline::navigated(cx);
    super::timeline::put_frame(cx, frame);
    Ok(CurrentCellResult { element_id: id.as_str().to_owned(), frame })
}

// ----- the view state: display options, panels, column widths (04 §12.2, §12.4; M5 C5.2a) ---------------------------

/// The Exposure Sheet's view state the window follows (the host-only notice `shell.xsheetView`): the display options,
/// the panels' shown state and the column widths. An agent reads it through the commands that set it.
fn view_state(cx: &Cx) -> Value {
    let d = cx.ui().drawing();
    json!({
        "thumbnails": d.xsheet.thumbnails,
        "exposureText": d.xsheet.exposure_text,
        "elementNames": d.xsheet.element_names,
        "notes": d.xsheet.notes,
        "elementList": d.xsheet.element_list,
        "lightTable": d.aids.static_light_table_panel,
        "toolbar": d.xsheet.toolbar,
        "columnWidths": cx.session().timeline.column_widths,
    })
}

/// Announces the view state when it differs from `before` (an event only for an effective change).
fn announce_view(cx: &mut Cx, before: &Value) {
    let after = view_state(cx);
    if &after != before {
        cx.emit("shell.xsheetView", after);
    }
}

/// `xsheet.display.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DisplayParams {
    /// Thumbnails in cells.
    #[serde(default)]
    thumbnails: Option<bool>,
    /// The drawing name in every held cell.
    #[serde(default)]
    exposure_text: Option<bool>,
    /// The element-name prefix in cell labels.
    #[serde(default)]
    element_names: Option<bool>,
    /// Note indicators.
    #[serde(default)]
    notes: Option<bool>,
}

/// `{thumbnails, exposureText, elementNames, notes}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DisplayResult {
    thumbnails: bool,
    exposure_text: bool,
    element_names: bool,
    notes: bool,
}

#[command(
    id = "xsheet.display.set",
    kind = ui,
    label = "Exposure Sheet display options",
    scopes = [ui],
    needs_project,
    example(title = "Thumbnails on", params = r#"{"thumbnails": true}"#),
    example(title = "Repeat the drawing name in held cells, without the element name", params = r#"{"exposureText": true, "elementNames": false}"#),
    gui = "Exposure Sheet: toolbar thumbnails toggle; grid context display options (04 §12.4; M5 C5.2)",
)]
pub(crate) fn xsheet_display_set(cx: &mut Cx, p: DisplayParams) -> Result<DisplayResult, CommandError> {
    let before = view_state(cx);
    {
        let x = &mut cx.ui_mut().drawing_mut().xsheet;
        if let Some(v) = p.thumbnails {
            x.thumbnails = v;
        }
        if let Some(v) = p.exposure_text {
            x.exposure_text = v;
        }
        if let Some(v) = p.element_names {
            x.element_names = v;
        }
        if let Some(v) = p.notes {
            x.notes = v;
        }
    }
    announce_view(cx, &before);
    let x = &cx.ui().drawing().xsheet;
    Ok(DisplayResult {
        thumbnails: x.thumbnails,
        exposure_text: x.exposure_text,
        element_names: x.element_names,
        notes: x.notes,
    })
}

/// `xsheet.panel.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PanelParams {
    /// The element list panel.
    #[serde(default)]
    element_list: Option<bool>,
    /// The static light table panel (one state with `drawAid.staticLightTablePanel.set`, ruling E-57).
    #[serde(default)]
    light_table: Option<bool>,
    /// The toolbar.
    #[serde(default)]
    toolbar: Option<bool>,
}

/// `{elementList, lightTable, toolbar}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PanelResult {
    element_list: bool,
    light_table: bool,
    toolbar: bool,
}

#[command(
    id = "xsheet.panel.set",
    kind = ui,
    label = "Exposure Sheet panels",
    scopes = [ui],
    needs_project,
    example(title = "Show the static light table panel", params = r#"{"lightTable": true}"#),
    example(title = "Hide the toolbar, show the element list", params = r#"{"toolbar": false, "elementList": true}"#),
    gui = "Exposure Sheet toolbar: the element list and light table panel toggles; the window commands (04 §12.2; M5 C5.2)",
)]
pub(crate) fn xsheet_panel_set(cx: &mut Cx, p: PanelParams) -> Result<PanelResult, CommandError> {
    let before = view_state(cx);
    {
        let x = &mut cx.ui_mut().drawing_mut().xsheet;
        if let Some(v) = p.element_list {
            x.element_list = v;
        }
        if let Some(v) = p.toolbar {
            x.toolbar = v;
        }
    }
    if let Some(v) = p.light_table {
        // The aids' state, announced as `drawAid.staticLightTablePanel.set` announces it (one state, E-57).
        super::draw_aid::change(cx, "staticLightTablePanel", |a| a.static_light_table_panel = v);
    }
    announce_view(cx, &before);
    let d = cx.ui().drawing();
    Ok(PanelResult {
        element_list: d.xsheet.element_list,
        light_table: d.aids.static_light_table_panel,
        toolbar: d.xsheet.toolbar,
    })
}

/// `xsheet.columnWidth.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ColumnWidthParams {
    /// The column's element (default: the current element).
    #[serde(default)]
    element_id: Option<String>,
    /// The width in pixels.
    #[serde(default)]
    #[schemars(range(min = 20, max = 1000))]
    width: Option<u32>,
    /// Return every column to the default width (04 §12.4 "Reset column width").
    #[serde(default)]
    reset: bool,
}

/// `{widths}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ColumnWidthResult {
    /// The columns with a width of their own, by element ID (the others have the default width).
    widths: BTreeMap<String, u32>,
}

/// `xsheet.columnWidth.set`: `width` or `reset`, not both, as the handler refuses them; a width needs a column (the
/// given element or the current one); `reset` needs nothing.
pub(crate) fn precheck_column_width(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let reset = v.get("reset") == Some(&Value::Bool(true));
    let width = v.get("width").is_some_and(|w| !w.is_null());
    if reset == width {
        let message = if reset { "pass width or reset, not both" } else { "pass width, or reset" };
        return Err(err(codes::ARG_INVALID, message, json!({ "pointer": "/width" })));
    }
    if reset || v.get("elementId").is_some() {
        return Ok(());
    }
    if cx.app().element.is_none() {
        return Err(err(
            "element.noCurrent",
            "there is no current element and no elementId was given",
            json!({ "pointer": "/elementId" }),
        ));
    }
    Ok(())
}

#[command(
    id = "xsheet.columnWidth.set",
    precheck = precheck_column_width,
    kind = ui,
    label = "Set column width",
    scopes = [ui],
    needs_project,
    example(title = "Bob's column 120 pixels wide", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "width": 120}"#),
    example(title = "Reset every column width", params = r#"{"reset": true}"#),
    gui = "Exposure Sheet: drag a column header's border; grid context Reset Column Width (04 §12.2, §12.4; M5 C5.2)",
)]
pub(crate) fn xsheet_column_width_set(cx: &mut Cx, p: ColumnWidthParams) -> Result<ColumnWidthResult, CommandError> {
    let before = view_state(cx);
    match (p.reset, p.width) {
        (true, Some(_)) => {
            return Err(err(
                codes::ARG_INVALID,
                "pass width to set one column, or reset to reset them all, not both",
                json!({ "pointer": "/width" }),
            ));
        }
        (true, None) => cx.session_mut().timeline.column_widths.clear(),
        (false, None) => {
            return Err(err(codes::ARG_INVALID, "pass width, or reset", json!({ "pointer": "/width" })));
        }
        (false, Some(width)) => {
            let id = match &p.element_id {
                Some(e) => element_id(cx, e, "/elementId")?,
                None => cx.app().element.clone().ok_or_else(|| {
                    err("element.noCurrent", "there is no current element", json!({ "pointer": "/elementId" }))
                })?,
            };
            let doc = cx.document()?;
            let column = doc.element(&id).is_some_and(|e| e.element_type().is_cell_based());
            if !column {
                return Err(err(
                    codes::ARG_INVALID,
                    "the element is not an Exposure Sheet column (only cell-based elements are, 04 §12.1)",
                    json!({ "pointer": "/elementId", "elementId": id.as_str() }),
                ));
            }
            cx.session_mut().timeline.column_widths.insert(id.as_str().to_owned(), width);
        }
    }
    announce_view(cx, &before);
    Ok(ColumnWidthResult { widths: cx.session().timeline.column_widths.clone() })
}

// ----- the static light table (04 §15; ruling E-57: one state with drawAid.staticLightTable.*) ----------------------

/// One drawing to send (SI-043, the shape 04 §13.18 and §15.1 give: a drawing of an element, chosen by its cell or
/// directly): `elementId` with `frame` (the drawing exposed there), or `drawingId` (a drawing of that element's bank),
/// or neither (the element's drawing at the current frame, as from a column header).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LightAddItem {
    /// The element.
    element_id: String,
    /// The frame of its cell.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    frame: Option<u32>,
    /// A drawing of the element's bank.
    #[serde(default)]
    drawing_id: Option<String>,
}

/// `xsheet.lightTable.add` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LightAddParams {
    /// The drawings to send (omitted: the drawings of the selected cells).
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    items: Option<Vec<LightAddItem>>,
}

/// `{itemIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightAddResult {
    /// The new items, in the order sent.
    item_ids: Vec<String>,
}

/// `xsheet.lightTable.add` without items: the cell selection must expose a drawing.
pub(crate) fn precheck_light_add(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    super::timing::scene_id(cx, None)?;
    if let Some(items) = v.get("items").and_then(Value::as_array) {
        // An item's element of a type the light table does not take is refused here as the handler refuses it (04
        // §13.20; prechecks = handlers).
        let doc = cx.document()?;
        for (i, it) in items.iter().enumerate() {
            let Some(r) = it.get("elementId").and_then(Value::as_str) else { continue };
            let Ok(id) = element_id(cx, r, &format!("/items/{i}/elementId")) else { continue };
            if doc.element(&id).is_some_and(|e| {
                !matches!(e.element_type(), ElementType::Drawing | ElementType::Image | ElementType::Annotation)
            }) {
                return Err(err(
                    "cell.notApplicable",
                    "only drawings, images and annotations can go to the static light table (04 §13.20, §15.1)",
                    json!({ "pointer": format!("/items/{i}/elementId") }),
                ));
            }
        }
        return Ok(());
    }
    if selected_drawings(cx)?.is_empty() {
        return Err(err(
            "drawing.noTarget",
            "the selected cells expose no drawing to send to the static light table",
            json!({ "pointer": "/items" }),
        ));
    }
    Ok(())
}

/// The most items one `xsheet.lightTable.add` takes (its schema's bound).
pub(crate) const LIGHT_ADD_MAX: usize = 256;

/// The distinct (element, drawing) pairs the cell selection exposes, in column then frame order.
fn selected_drawings(cx: &Cx) -> Result<Vec<(Id, Id)>, CommandError> {
    let doc = cx.document()?;
    let Some((elements, from, to)) = super::timing::selected_cells(cx) else { return Ok(Vec::new()) };
    let (from, to) = (u32::try_from(from).unwrap_or(u32::MAX), u32::try_from(to).unwrap_or(u32::MAX));
    Ok(light_table_drawings(doc, &elements, from, to))
}

/// The distinct (element, drawing) pairs cells expose that the static light table takes (the drawing, image and
/// annotation columns, 04 §13.20, §15.1), in column then frame order: what the command without items sends for the
/// selection and what cells dragged onto the panel send (the timing-gesture interpreter).
pub(crate) fn light_table_drawings(doc: &Document, elements: &[Id], from: u32, to: u32) -> Vec<(Id, Id)> {
    let mut out: Vec<(Id, Id)> = Vec::new();
    for e in elements {
        let takes = doc.element(e).is_some_and(|el| {
            matches!(el.element_type(), ElementType::Drawing | ElementType::Image | ElementType::Annotation)
        });
        if !takes {
            continue;
        }
        for frame in from..=to {
            if let Some(d) = AppContext::exposed_drawing(doc, e, frame)
                && !out.iter().any(|(oe, od)| oe == e && *od == d)
            {
                out.push((e.clone(), d));
            }
        }
    }
    out
}

/// Resolves one item to its element and drawing.
fn light_source(cx: &Cx, it: &LightAddItem, i: usize) -> Result<(Id, Id), CommandError> {
    let doc = cx.document()?;
    let element = element_id(cx, &it.element_id, &format!("/items/{i}/elementId"))?;
    let e = doc.element(&element).ok_or_else(|| {
        err("element.notFound", "no such element", json!({ "pointer": format!("/items/{i}/elementId") }))
    })?;
    if !matches!(e.element_type(), ElementType::Drawing | ElementType::Image | ElementType::Annotation) {
        // 04 §13.20: a command invoked on an element type it is not available for reports `cell.notApplicable`.
        return Err(err(
            "cell.notApplicable",
            "only drawings, images and annotations can go to the static light table (04 §13.20, §15.1)",
            json!({ "pointer": format!("/items/{i}/elementId") }),
        ));
    }
    let drawing = match (&it.drawing_id, it.frame) {
        (Some(_), Some(_)) => {
            return Err(err(
                codes::ARG_INVALID,
                "pass frame or drawingId, not both",
                json!({ "pointer": format!("/items/{i}/drawingId") }),
            ));
        }
        (Some(d), None) => {
            let d = super::timing::drawing_id(cx, d, &format!("/items/{i}/drawingId"))?;
            if doc.bank_of(&d) != e.drawing_bank_id().as_ref() {
                return Err(err(
                    "drawing.notFound",
                    "the drawing is not in the element's bank",
                    json!({ "pointer": format!("/items/{i}/drawingId") }),
                ));
            }
            d
        }
        (None, frame) => {
            let frame = frame.unwrap_or(cx.app().frame).max(1);
            AppContext::exposed_drawing(doc, &element, frame).ok_or_else(|| {
                err(
                    "drawing.noTarget",
                    format!("the cell at frame {frame} is empty"),
                    json!({ "pointer": format!("/items/{i}/frame") }),
                )
            })?
        }
    };
    Ok((element, drawing))
}

/// Announces both families' events for a change of the scene's light table: `xsheet.lightTable.changed` with the
/// items it changed, and the Drawing view's `drawAid.changed` (one state, E-57).
fn announce_light(cx: &mut Cx, scene: &Id, before: &[super::draw_aid::Entry], changed: Vec<String>) {
    super::draw_aid::announce(cx, scene, before);
    if !changed.is_empty() {
        cx.emit("xsheet.lightTable.changed", json!({ "itemIds": changed }));
    }
}

#[command(
    id = "xsheet.lightTable.add",
    precheck = precheck_light_add,
    kind = ui,
    label = "Send to static light table",
    scopes = [ui],
    needs_project,
    example(title = "The drawings of the selected cells", params = "{}"),
    example(title = "Bob's drawing at frame 3", params = r#"{"items": [{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 3}]}"#),
    gui = "Exposure Sheet and Timeline: cell and column-header contexts, Send to Static Light Table; cells dragged onto the panel (04 §13.18, §15.2; M5 C5.2)",
)]
pub(crate) fn xsheet_light_table_add(cx: &mut Cx, p: LightAddParams) -> Result<LightAddResult, CommandError> {
    let sources = match &p.items {
        Some(items) => {
            items.iter().enumerate().map(|(i, it)| light_source(cx, it, i)).collect::<Result<Vec<_>, _>>()?
        }
        None => selected_drawings(cx)?,
    };
    let scene = super::timing::scene_id(cx, None)?;
    let doc = cx.document()?.clone();
    if let Some((i, _)) = sources.iter().enumerate().find(|(_, (e, _))| doc.scene_of(e) != Some(&scene)) {
        return Err(err(
            codes::ARG_INVALID,
            "the element is not in the current scene (the light table is the scene's, 04 §15)",
            json!({ "pointer": format!("/items/{i}/elementId") }),
        ));
    }
    let before = super::draw_aid::entries(cx, &scene);
    let table = cx.session_mut().timeline.light_tables.entry(scene.as_str().to_owned()).or_default();
    if table.len() + sources.len() > super::draw_aid::MAX_ENTRIES {
        return Err(err(
            codes::LIMIT_EXCEEDED,
            format!("a scene's static light table holds at most {} items", super::draw_aid::MAX_ENTRIES),
            json!({ "limit": "staticLightTableEntries", "max": super::draw_aid::MAX_ENTRIES }),
        ));
    }
    let mut ids = Vec::new();
    for (element, drawing) in sources {
        let id = super::draw_aid::next_entry_id(table);
        table.push(LightItem {
            item_id: id.clone(),
            element_id: element.as_str().to_owned(),
            drawing_id: drawing.as_str().to_owned(),
            visible: true,
            placement: "under".to_owned(),
        });
        ids.push(id);
    }
    announce_light(cx, &scene, &before, ids.clone());
    Ok(LightAddResult { item_ids: ids })
}

/// The live item ids of the current scene's light table, in stacking order.
fn live_ids(cx: &Cx, scene: &Id) -> Vec<String> {
    super::draw_aid::entries(cx, scene).iter().map(|e| e.id().to_owned()).collect()
}

/// Every id of `ids` names a live item (`ref.notFound` otherwise, at its pointer).
fn check_ids(live: &[String], ids: &[String], at: &str) -> Result<(), CommandError> {
    for (i, id) in ids.iter().enumerate() {
        if !live.contains(id) {
            return Err(err(
                tt_model::codes::REF_NOT_FOUND,
                format!("no static light-table item `{id}`"),
                json!({ "pointer": format!("{at}/{i}"), "ref": id }),
            ));
        }
    }
    Ok(())
}

/// `xsheet.lightTable.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LightMoveParams {
    /// The item.
    item_id: String,
    /// Its new position in the stacking order (0: the first).
    #[schemars(range(min = 0))]
    index: u32,
}

/// `{order}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LightMoveResult {
    /// The items in stacking order.
    order: Vec<String>,
}

/// `xsheet.lightTable.move`: `itemId` names an item of the current scene's table.
pub(crate) fn precheck_light_move(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let scene = super::timing::scene_id(cx, None)?;
    let id = v.get("itemId").and_then(Value::as_str).unwrap_or_default();
    if live_ids(cx, &scene).iter().any(|x| x == id) {
        return Ok(());
    }
    Err(err(
        tt_model::codes::REF_NOT_FOUND,
        format!("no static light-table item `{id}`"),
        json!({ "pointer": "/itemId", "ref": id }),
    ))
}

#[command(
    id = "xsheet.lightTable.move",
    precheck = precheck_light_move,
    kind = ui,
    label = "Reorder light-table item",
    scopes = [ui],
    needs_project,
    example(title = "Move item lt2 to the top", params = r#"{"itemId": "lt2", "index": 0}"#),
    gui = "Exposure Sheet static light table panel: drag an item (04 §15.3; M5 C5.2)",
)]
pub(crate) fn xsheet_light_table_move(cx: &mut Cx, p: LightMoveParams) -> Result<LightMoveResult, CommandError> {
    precheck_light_move(cx, &json!({ "itemId": p.item_id }))?;
    let scene = super::timing::scene_id(cx, None)?;
    let live = live_ids(cx, &scene);
    let last = live.len().saturating_sub(1);
    let index = usize::try_from(p.index).unwrap_or(usize::MAX);
    if index > last {
        return Err(err(
            codes::ARG_INVALID,
            format!("index {index} is past the last position {last}"),
            json!({ "pointer": "/index", "allowed": { "minimum": 0, "maximum": last } }),
        ));
    }
    let before = super::draw_aid::entries(cx, &scene);
    let mut changed = Vec::new();
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        // Items whose source is gone are not in the live order: drop them before reordering.
        items.retain(|it| live.contains(&it.item_id));
        if let Some(from) = items.iter().position(|it| it.item_id == p.item_id)
            && from != index
        {
            let item = items.remove(from);
            items.insert(index.min(items.len()), item);
            changed.push(p.item_id.clone());
        }
    }
    announce_light(cx, &scene, &before, changed);
    Ok(LightMoveResult { order: live_ids(cx, &scene) })
}

/// `xsheet.lightTable.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LightRemoveParams {
    /// The items to remove.
    #[serde(default)]
    #[schemars(length(max = 256))]
    item_ids: Option<Vec<String>>,
    /// Remove every item.
    #[serde(default)]
    all: bool,
}

/// `{removedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightRemoveResult {
    /// The items removed (their drawings untouched, 04 §15.3).
    removed_ids: Vec<String>,
}

/// `remove` and `set` name their items: `itemIds`, or `all` (`remove`) or `apply: "all"` (`set`).
pub(crate) fn precheck_light_items(cx: &Cx, v: &Value) -> Result<(), CommandError> {
    let named = v.get("itemIds").is_some_and(|x| !x.is_null())
        || v.get("all") == Some(&Value::Bool(true))
        || v.get("apply").and_then(Value::as_str).is_some_and(|a| a == "all");
    if !named {
        return Err(err(codes::ARG_INVALID, "pass itemIds (or all)", json!({ "pointer": "/itemIds" })));
    }
    super::timing::scene_id(cx, None).map(|_| ())
}

#[command(
    id = "xsheet.lightTable.remove",
    precheck = precheck_light_items,
    kind = ui,
    label = "Remove light-table items",
    scopes = [ui],
    needs_project,
    example(title = "Delete all", params = r#"{"all": true}"#),
    example(title = "Delete item lt1", params = r#"{"itemIds": ["lt1"]}"#),
    gui = "Exposure Sheet static light table panel: item and panel contexts, Delete, Delete All (04 §15.4; M5 C5.2)",
)]
pub(crate) fn xsheet_light_table_remove(cx: &mut Cx, p: LightRemoveParams) -> Result<LightRemoveResult, CommandError> {
    let scene = super::timing::scene_id(cx, None)?;
    let live = live_ids(cx, &scene);
    let ids = match (&p.item_ids, p.all) {
        (_, true) => live.clone(),
        (Some(ids), false) => {
            check_ids(&live, ids, "/itemIds")?;
            ids.clone()
        }
        (None, false) => return Err(err(codes::ARG_INVALID, "pass itemIds or all", json!({ "pointer": "/itemIds" }))),
    };
    let before = super::draw_aid::entries(cx, &scene);
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        items.retain(|it| !ids.contains(&it.item_id));
    }
    announce_light(cx, &scene, &before, ids.clone());
    Ok(LightRemoveResult { removed_ids: ids })
}

/// Which items `xsheet.lightTable.set` changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Apply {
    /// The items given.
    #[default]
    Given,
    /// Every item.
    All,
    /// Every item but the ones given (Hide All Others, 04 §15.4).
    Others,
}

/// Over or under the Drawing View's drawings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Placement {
    /// Over.
    Over,
    /// Under.
    Under,
}

/// `xsheet.lightTable.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LightSetParams {
    /// The items (with `apply: given` or `others`).
    #[serde(default)]
    #[schemars(length(max = 256))]
    item_ids: Option<Vec<String>>,
    /// Which items change.
    #[serde(default)]
    apply: Apply,
    /// Show or hide them.
    #[serde(default)]
    visible: Option<bool>,
    /// Over or under the Drawing View's drawings.
    #[serde(default)]
    placement: Option<Placement>,
}

/// `{changedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightSetResult {
    /// The items whose state changed.
    changed_ids: Vec<String>,
}

#[command(
    id = "xsheet.lightTable.set",
    precheck = precheck_light_items,
    kind = ui,
    label = "Change light-table items",
    scopes = [ui],
    needs_project,
    example(title = "Hide all others than lt1", params = r#"{"itemIds": ["lt1"], "apply": "others", "visible": false}"#),
    example(title = "Overlay all", params = r#"{"apply": "all", "placement": "over"}"#),
    gui = "Exposure Sheet static light table panel: an item's visibility and over/under controls; item and panel contexts (04 §15.3, §15.4; M5 C5.2)",
)]
pub(crate) fn xsheet_light_table_set(cx: &mut Cx, p: LightSetParams) -> Result<LightSetResult, CommandError> {
    let scene = super::timing::scene_id(cx, None)?;
    let live = live_ids(cx, &scene);
    let given = p.item_ids.clone().unwrap_or_default();
    if p.apply != Apply::All {
        if p.item_ids.is_none() {
            return Err(err(codes::ARG_INVALID, "pass itemIds", json!({ "pointer": "/itemIds" })));
        }
        check_ids(&live, &given, "/itemIds")?;
    }
    let targets: Vec<String> = match p.apply {
        Apply::Given => given,
        Apply::All => live.clone(),
        Apply::Others => live.iter().filter(|id| !given.contains(id)).cloned().collect(),
    };
    let before = super::draw_aid::entries(cx, &scene);
    let mut changed = Vec::new();
    if let Some(items) = cx.session_mut().timeline.light_tables.get_mut(scene.as_str()) {
        for it in items.iter_mut().filter(|it| targets.contains(&it.item_id)) {
            let was = it.clone();
            if let Some(v) = p.visible {
                it.visible = v;
            }
            if let Some(pl) = p.placement {
                it.placement = match pl {
                    Placement::Over => "over",
                    Placement::Under => "under",
                }
                .to_owned();
            }
            if *it != was {
                changed.push(it.item_id.clone());
            }
        }
    }
    announce_light(cx, &scene, &before, changed.clone());
    Ok(LightSetResult { changed_ids: changed })
}
