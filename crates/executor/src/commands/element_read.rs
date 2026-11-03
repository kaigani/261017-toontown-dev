//! Element queries and element-level UI commands (04 §5, §9.3, §11.8, §17.4–§17.5; M3 C3.1): `element.list`,
//! `element.get` (the 04 §17.5 read model, a projection of the element object with its derived fields, 10 AG-01),
//! `element.hierarchy.get`, `element.typeColour.get/.set`, `element.select`, `element.hierarchy.select`,
//! `element.current.set` and `element.current.move`. Queries and UI commands never create an undo step.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::exposure as x;
use tt_anim::hierarchy as h;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::{Element, ElementType, Role};
use tt_registry::{CommandError, command};

use super::common::{Direction, NoParams};
use super::timing::{
    Budget, CATEGORY_COLOURS, as_u32, category, element, element_id, err, internal, scene_id, scene_of, to_json,
};
use crate::cx::Cx;

// ----- element.list ---------------------------------------------------------------------------------------------------

/// List order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ListOrder {
    /// Timeline order (parents before children).
    #[default]
    Timeline,
    /// Exposure Sheet column order (cell-based elements only).
    Xsheet,
}

fn yes() -> bool {
    true
}

/// `element.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ListParams {
    /// Scene (default: the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// Timeline or Exposure Sheet order.
    #[serde(default)]
    order: ListOrder,
    /// Only these element types.
    #[serde(default)]
    types: Option<Vec<ElementType>>,
    /// Only the descendants (or, with `recursive: false`, the children) of this element.
    #[serde(default)]
    parent_id: Option<String>,
    /// Include every level below `parentId` (default) or only its children.
    #[serde(default = "yes")]
    recursive: bool,
}

/// One listed element.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListedElement {
    /// ID.
    id: String,
    /// Name.
    name: String,
    /// Element type.
    #[serde(rename = "type")]
    ty: String,
    /// Parent (`null` at the root level).
    parent_id: Option<String>,
    /// Depth in the hierarchy (0 = root level).
    depth: u32,
    /// Index among its siblings (Timeline order) or column index (Exposure Sheet order).
    index: u32,
}

/// `{elements}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ListResult {
    /// The elements in the requested order.
    elements: Vec<ListedElement>,
}

#[command(
    id = "element.list",
    kind = query,
    label = "List elements",
    scopes = [read],
    needs_project,
    example(title = "Elements of the Intro scene in Timeline order", params = r#"{"sceneId": "scene:\"Intro\""}"#),
    example(title = "Exposure Sheet columns of the Intro scene", params = r#"{"sceneId": "scene:\"Intro\"", "order": "xsheet"}"#),
    example(title = "Direct children of the peg Bob-P", params = r#"{"parentId": "scene:\"Intro\"/element:\"Bob-P\"", "recursive": false}"#),
    agent_only = "the Timeline and Exposure Sheet show the elements directly",
)]
pub(crate) fn element_list(cx: &mut Cx, p: ListParams) -> Result<ListResult, CommandError> {
    let parent = match &p.parent_id {
        Some(r) => Some(element_id(cx, r, "/parentId")?),
        None => None,
    };
    let doc = cx.document()?;
    let scene = match (&p.scene_id, &parent) {
        (Some(s), _) => scene_id(cx, Some(s))?,
        (None, Some(e)) => doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?,
        (None, None) => scene_id(cx, None)?,
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("listing elements");
    budget.spend(h::count(sc.tree()))?;
    let placed = h::placed(sc.tree());
    let under = |id: &Id, depth: usize| -> bool {
        match &parent {
            None => true,
            Some(pid) if p.recursive => id != pid && h::is_within(sc.tree(), id, pid),
            Some(pid) => placed.iter().any(|q| q.id == *id && q.parent.as_ref() == Some(pid)) && depth > 0,
        }
    };
    let typed = |e: &Element| p.types.as_ref().is_none_or(|t| t.contains(e.element_type()));
    let mut out = Vec::new();
    match p.order {
        ListOrder::Timeline => {
            for q in &placed {
                let Some(e) = doc.element(&q.id) else { continue };
                if under(&q.id, q.depth) && typed(e) {
                    out.push(listed(e, q.parent.as_ref(), q.depth, q.index));
                }
            }
        }
        ListOrder::Xsheet => {
            for (i, c) in sc.column_order().iter().enumerate() {
                let Some(e) = doc.element(c) else { continue };
                let Some(q) = placed.iter().find(|q| q.id == *c) else { continue };
                if under(c, q.depth) && typed(e) {
                    out.push(listed(e, q.parent.as_ref(), q.depth, i));
                }
            }
        }
    }
    Ok(ListResult { elements: out })
}

fn listed(e: &Element, parent: Option<&Id>, depth: usize, index: usize) -> ListedElement {
    ListedElement {
        id: e.id().as_str().to_owned(),
        name: e.name().clone(),
        ty: e.element_type().as_str().to_owned(),
        parent_id: parent.map(|p| p.as_str().to_owned()),
        depth: as_u32(depth),
        index: as_u32(index),
    }
}

// ----- element.get ------------------------------------------------------------------------------------------------------

/// `element.get` / `element.hierarchy.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ElementParams {
    /// The element (ID or path).
    element_id: String,
}

/// The media summary of a media element.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaInfo {
    /// The linked file (or library item) of the first sequence's asset.
    path: Option<String>,
    /// First frame of the first sequence.
    start_frame: u32,
    /// Frames covered by the sequences.
    length: u32,
    /// Whether the linked source is missing (as last checked; media checks arrive with M8 C8.3).
    missing: bool,
}

/// The element read model (04 §17.5).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ElementInfo {
    /// ID.
    id: String,
    /// Canonical path (10 AG-03).
    path: String,
    /// Name.
    name: String,
    /// Element type.
    element_type: String,
    /// Scene.
    scene_id: String,
    /// Parent (`null` at the root level).
    parent_id: Option<String>,
    /// Children in Timeline order.
    child_ids: Vec<String>,
    /// Depth (0 = root level).
    depth: u32,
    /// Position in Timeline order (pre-order index).
    timeline_index: u32,
    /// Exposure Sheet column index (`null` for non-column elements).
    xsheet_index: Option<u32>,
    /// First frame of the bar (cell-based: first exposed frame, 1 when empty).
    start: u32,
    /// Bar length (cell-based: the exposed extent, 0 when empty).
    duration: u32,
    /// Loops.
    loops: u32,
    /// Annotation element.
    annotation: bool,
    /// Composite role.
    role: Role,
    /// Deformable by bones.
    deformable: bool,
    /// Shown in the scene.
    visible_in_scene: bool,
    /// Hidden because an ancestor is hidden (04 §10.1).
    hidden_by_parent: bool,
    /// Shown in the Exposure Sheet (`null` for non-column elements).
    visible_in_xsheet: Option<bool>,
    /// Locked in scene-planning views.
    locked: bool,
    /// Collapsed in the Timeline.
    collapsed: bool,
    /// Marker colour (`null` = category default).
    marker_colour: Option<String>,
    /// Background colour (`null` = category default).
    background_colour: Option<String>,
    /// The marker colour in effect.
    effective_marker_colour: String,
    /// Element note.
    note: String,
    /// Exposure Sheet column width.
    column_width: u32,
    /// Clone group (the shared drawing bank) when the bank is shared, else `null`.
    clone_group_id: Option<String>,
    /// The other elements of the clone group.
    clone_ids: Vec<String>,
    /// Drawings in the bank.
    drawing_count: u32,
    /// Media summary (media elements), else `null`.
    media: Option<MediaInfo>,
}

/// The effective colour of a category (session preference, else 04 §5.1's recommended default).
pub(crate) fn category_colour(cx: &Cx, cat: &str) -> String {
    cx.session()
        .timeline
        .type_colours
        .get(cat)
        .cloned()
        .or_else(|| CATEGORY_COLOURS.iter().find(|(c, _)| *c == cat).map(|(_, v)| (*v).to_owned()))
        .unwrap_or_else(|| "#BFBFBFFF".to_owned())
}

fn colour_str(v: &Option<tt_model::value::Colour>) -> Option<String> {
    v.as_ref().map(ToString::to_string)
}

/// The exposed extent of a cell-based element: `(first, last)` exposed frames.
fn exposed_extent(e: &Element) -> Option<(usize, usize)> {
    let f = x::expand(e.exposure().as_deref()?);
    let first = f.iter().position(Option::is_some)?;
    Some((first + 1, x::last_exposed(&f)))
}

pub(crate) fn info(cx: &Cx, doc: &Document, id: &Id) -> Result<ElementInfo, CommandError> {
    let e = element(doc, id)?;
    let sc = scene_of(doc, id)?;
    let placed = h::placed(sc.tree());
    let (pi, q) = placed.iter().enumerate().find(|(_, q)| q.id == *id).ok_or_else(|| internal("element not in the tree"))?;
    let ps = h::parents(sc.tree());
    let mut hidden_by_parent = false;
    let mut cur = q.parent.clone();
    while let Some(p) = cur {
        if doc.element(&p).is_some_and(|x| !*x.visible_in_scene()) {
            hidden_by_parent = true;
            break;
        }
        cur = ps.get(&p).cloned().flatten();
    }
    let (start, duration) = match (e.start(), e.duration()) {
        (Some(s), Some(d)) => (*s, *d),
        _ => match exposed_extent(e) {
            Some((a, b)) => (as_u32(a), as_u32(b - a + 1)),
            None => (1, 0),
        },
    };
    let bank = e.drawing_bank_id().clone();
    let users: Vec<Id> = bank.as_ref().map(|b| doc.bank_users(b).to_vec()).unwrap_or_default();
    let media = if *e.element_type() == ElementType::Media {
        let seqs = e.sequences().clone().unwrap_or_default();
        let path = seqs.first().and_then(|s| doc.object_raw_json(s.asset_id().as_str()).ok()).and_then(|(_, a)| {
            a.pointer("/link/path")
                .or_else(|| a.pointer("/sourcePath"))
                .or_else(|| a.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
        Some(MediaInfo {
            path,
            start_frame: seqs.first().map_or(1, |s| *s.start_frame()),
            length: seqs.iter().map(|s| s.stop_frame().saturating_sub(*s.start_frame()) + 1).sum(),
            missing: false,
        })
    } else {
        None
    };
    Ok(ElementInfo {
        id: id.as_str().to_owned(),
        path: doc.canonical_path(id.as_str()).unwrap_or_default(),
        name: e.name().clone(),
        element_type: e.element_type().as_str().to_owned(),
        scene_id: sc.id().as_str().to_owned(),
        parent_id: q.parent.as_ref().map(|p| p.as_str().to_owned()),
        child_ids: h::level_ids(sc.tree(), Some(id)).iter().map(|c| c.as_str().to_owned()).collect(),
        depth: as_u32(q.depth),
        timeline_index: as_u32(pi),
        xsheet_index: sc.column_order().iter().position(|c| c == id).map(as_u32),
        start,
        duration,
        loops: *e.loops(),
        annotation: *e.element_type() == ElementType::Annotation,
        role: *e.role(),
        deformable: *e.deformable(),
        visible_in_scene: *e.visible_in_scene(),
        hidden_by_parent,
        visible_in_xsheet: *e.visible_in_xsheet(),
        locked: *e.locked(),
        collapsed: *e.collapsed(),
        marker_colour: colour_str(e.marker_colour()),
        background_colour: colour_str(e.background_colour()),
        effective_marker_colour: colour_str(e.marker_colour())
            .unwrap_or_else(|| category_colour(cx, category(*e.element_type()))),
        note: e.note().clone(),
        column_width: *e.column_width(),
        clone_group_id: bank.as_ref().filter(|_| users.len() > 1).map(|b| b.as_str().to_owned()),
        clone_ids: if users.len() > 1 { users.iter().filter(|u| *u != id).map(|u| u.as_str().to_owned()).collect() } else { Vec::new() },
        drawing_count: bank.as_ref().and_then(|b| doc.bank(b)).map_or(0, |b| as_u32(b.drawings().ids().len())),
        media,
    })
}

#[command(
    id = "element.get",
    kind = query,
    label = "Get element",
    scopes = [read],
    needs_project,
    example(title = "Bob's state", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    agent_only = "the Timeline, Exposure Sheet and Properties view show an element's state directly",
)]
pub(crate) fn element_get(cx: &mut Cx, p: ElementParams) -> Result<ElementInfo, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?;
    let mut budget = Budget::new("reading an element");
    budget.spend(doc.scene_of(&id).and_then(|s| doc.scene(s)).map_or(1, |s| h::count(s.tree())))?;
    info(cx, doc, &id)
}

// ----- element.hierarchy.get ----------------------------------------------------------------------------------------------

/// `{parentId, childIds, previousSiblingId, nextSiblingId, depth}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HierarchyResult {
    /// Parent (`null` at the root level).
    parent_id: Option<String>,
    /// Children, top to bottom.
    child_ids: Vec<String>,
    /// The sibling above (`null` when first).
    previous_sibling_id: Option<String>,
    /// The sibling below (`null` when last).
    next_sibling_id: Option<String>,
    /// Depth (0 = root level).
    depth: u32,
}

#[command(
    id = "element.hierarchy.get",
    kind = query,
    label = "Get hierarchy",
    scopes = [read],
    needs_project,
    example(title = "Where Bob hangs", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    agent_only = "the Timeline shows the hierarchy by indentation",
)]
pub(crate) fn element_hierarchy_get(cx: &mut Cx, p: ElementParams) -> Result<HierarchyResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?;
    let sc = scene_of(doc, &id)?;
    let mut budget = Budget::new("reading the hierarchy");
    budget.spend(h::count(sc.tree()))?;
    let placed = h::placed(sc.tree());
    let q = placed.iter().find(|q| q.id == id).ok_or_else(|| internal("element not in the tree"))?;
    let level = h::level_ids(sc.tree(), q.parent.as_ref());
    let s = |i: Option<usize>| i.and_then(|i| level.get(i)).map(|x| x.as_str().to_owned());
    Ok(HierarchyResult {
        parent_id: q.parent.as_ref().map(|p| p.as_str().to_owned()),
        child_ids: h::level_ids(sc.tree(), Some(&id)).iter().map(|c| c.as_str().to_owned()).collect(),
        previous_sibling_id: s(q.index.checked_sub(1)),
        next_sibling_id: s(Some(q.index + 1)),
        depth: as_u32(q.depth),
    })
}

// ----- category colours -------------------------------------------------------------------------------------------------

/// `{colours}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TypeColoursResult {
    /// Colour by category (`drawings`, `images`, `sounds`, `pegs`, `skeletons`, `media`, `cameras`, `effects`,
    /// `effectParameters`).
    colours: Map<String, Value>,
}

#[command(
    id = "element.typeColour.get",
    kind = query,
    label = "Get category colours",
    scopes = [read],
    example(title = "Default colours of all element categories", params = "{}"),
    agent_only = "the preferences dialog shows the category colours directly",
)]
pub(crate) fn element_type_colour_get(cx: &mut Cx, _p: NoParams) -> Result<TypeColoursResult, CommandError> {
    let mut colours = Map::new();
    for (c, _) in CATEGORY_COLOURS {
        colours.insert(c.to_owned(), json!(category_colour(cx, c)));
    }
    Ok(TypeColoursResult { colours })
}

/// Element categories (04 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Category {
    /// Drawing and annotation elements.
    Drawings,
    /// Image elements.
    Images,
    /// Sound elements.
    Sounds,
    /// Pegs.
    Pegs,
    /// Skeletons and bones.
    Skeletons,
    /// Media elements.
    Media,
    /// Cameras.
    Cameras,
    /// Effects.
    Effects,
    /// Effect parameters.
    EffectParameters,
}

/// `element.typeColour.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TypeColourSetParams {
    /// The category.
    category: Category,
    /// `#RRGGBB` or `#RRGGBBAA`.
    #[schemars(regex(pattern = r"^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$"))]
    colour: String,
}

/// `{category, colour}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TypeColourSetResult {
    /// The category.
    category: Category,
    /// Its colour (`#RRGGBBAA`).
    #[schemars(regex(pattern = r"^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$"))]
    colour: String,
}

#[command(
    id = "element.typeColour.set",
    kind = ui,
    label = "Set category colour",
    scopes = [ui],
    example(title = "Pegs in purple", params = r##"{"category": "pegs", "colour": "#8064A2"}"##),
    gui = "Preferences: Timeline category colours (M4 C4.1)",
)]
pub(crate) fn element_type_colour_set(cx: &mut Cx, p: TypeColourSetParams) -> Result<TypeColourSetResult, CommandError> {
    let c = tt_model::value::Colour::parse(&p.colour)
        .map_err(|e| err(tt_registry::codes::ARG_INVALID, e.message().to_owned(), json!({ "pointer": "/colour" })))?
        .to_string();
    let key = to_json(&p.category)?.as_str().unwrap_or_default().to_owned();
    cx.session_mut().timeline.type_colours.insert(key, c.clone());
    Ok(TypeColourSetResult { category: p.category, colour: c })
}

// ----- selection and current element ----------------------------------------------------------------------------------------

/// Selection mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SelectMode {
    /// Replace.
    #[default]
    Replace,
    /// Add.
    Add,
    /// Remove.
    Remove,
    /// Toggle.
    Toggle,
}

/// `element.select` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SelectParams {
    /// Elements (rows / columns).
    element_ids: Vec<String>,
    /// How to combine with the current selection.
    #[serde(default)]
    mode: SelectMode,
}

/// `{selectedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedResult {
    /// The selected elements.
    selected_ids: Vec<String>,
}

fn set_rows(cx: &mut Cx, ids: Vec<String>) {
    cx.app_mut().set_selection("timeline", ids.clone());
    cx.app_mut().selections.insert("staging".to_owned(), ids);
    let current = cx.app().element.as_ref().map(|e| e.as_str().to_owned());
    let frame = cx.app().frame;
    let rows = cx.app().selection_of("timeline");
    cx.emit(
        "cell.selection.changed",
        json!({ "elementIds": rows, "from": 0, "to": 0, "currentElementId": current, "currentFrame": frame }),
    );
}

#[command(
    id = "element.select",
    kind = ui,
    label = "Select elements",
    scopes = [ui],
    needs_project,
    example(title = "Select Bob", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""]}"#),
    gui = "Timeline: click row names (with extend/toggle modifiers); Exposure Sheet: click a column header (M5)",
)]
pub(crate) fn element_select(cx: &mut Cx, p: SelectParams) -> Result<SelectedResult, CommandError> {
    let mut ids = Vec::new();
    for (i, r) in p.element_ids.iter().enumerate() {
        let id = element_id(cx, r, &format!("/elementIds/{i}"))?.as_str().to_owned();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut sel: Vec<String> = cx
        .app()
        .selection_of("timeline")
        .into_iter()
        .filter(|k| crate::app::parse_cell_ref(k).is_none())
        .collect();
    match p.mode {
        SelectMode::Replace => sel = ids,
        SelectMode::Add => sel.extend(ids.into_iter().filter(|i| !sel.contains(i)).collect::<Vec<_>>()),
        SelectMode::Remove => sel.retain(|i| !ids.contains(i)),
        SelectMode::Toggle => {
            for i in ids {
                match sel.iter().position(|s| *s == i) {
                    Some(k) => {
                        sel.remove(k);
                    }
                    None => sel.push(i),
                }
            }
        }
    }
    set_rows(cx, sel.clone());
    Ok(SelectedResult { selected_ids: sel })
}

/// Hierarchy relations (04 §9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Relation {
    /// The parent.
    Parent,
    /// The element and every element attached to it.
    Children,
    /// The first child.
    FirstChild,
    /// The sibling above.
    PreviousSibling,
    /// The sibling below.
    NextSibling,
    /// The root of the current skeleton.
    SkeletonRoot,
}

/// `element.hierarchy.select` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct HierarchySelectParams {
    /// Relation to select.
    relation: Relation,
    /// From this element (default: the current element).
    #[serde(default)]
    element_id: Option<String>,
}

#[command(
    id = "element.hierarchy.select",
    kind = ui,
    label = "Select in hierarchy",
    scopes = [ui],
    needs_project,
    example(title = "Select Bob's parent", params = r#"{"relation": "parent", "elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    gui = "Element menu, camera view and Timeline layer context: Select Parent / Children / Child / Previous / Next Sibling (M5)",
)]
pub(crate) fn element_hierarchy_select(cx: &mut Cx, p: HierarchySelectParams) -> Result<SelectedResult, CommandError> {
    let id = match &p.element_id {
        Some(r) => element_id(cx, r, "/elementId")?,
        None => cx.app().element.clone().ok_or_else(|| {
            err("element.notFound", "there is no current element and no elementId was given", json!({ "pointer": "/elementId" }))
        })?,
    };
    let doc = cx.document()?.clone();
    let sc = scene_of(&doc, &id)?;
    let placed = h::placed(sc.tree());
    let q = placed.iter().find(|q| q.id == id).ok_or_else(|| internal("element not in the tree"))?;
    let level = h::level_ids(sc.tree(), q.parent.as_ref());
    let none = || {
        err(
            tt_registry::codes::COMMAND_DISABLED,
            format!("the element has no {}", match p.relation {
                Relation::Parent => "parent",
                Relation::Children | Relation::FirstChild => "children",
                Relation::PreviousSibling => "previous sibling",
                Relation::NextSibling => "next sibling",
                Relation::SkeletonRoot => "skeleton",
            }),
            json!({ "elementId": id.as_str(), "reason": "noTarget" }),
        )
    };
    let picked: Vec<Id> = match p.relation {
        Relation::Parent => vec![q.parent.clone().ok_or_else(none)?],
        Relation::Children => {
            let kids = h::descendants(sc.tree(), &id);
            if kids.is_empty() {
                return Err(none());
            }
            std::iter::once(id.clone()).chain(kids).collect()
        }
        Relation::FirstChild => vec![h::level_ids(sc.tree(), Some(&id)).first().cloned().ok_or_else(none)?],
        Relation::PreviousSibling => vec![q.index.checked_sub(1).and_then(|i| level.get(i)).cloned().ok_or_else(none)?],
        Relation::NextSibling => vec![level.get(q.index + 1).cloned().ok_or_else(none)?],
        Relation::SkeletonRoot => {
            let ps = h::parents(sc.tree());
            let mut cur = Some(id.clone());
            let mut root = None;
            while let Some(c) = cur {
                if doc.element(&c).is_some_and(|e| *e.element_type() == ElementType::Skeleton) {
                    root = Some(c.clone());
                    break;
                }
                cur = ps.get(&c).cloned().flatten();
            }
            vec![root.ok_or_else(none)?]
        }
    };
    let strs: Vec<String> = picked.iter().map(|i| i.as_str().to_owned()).collect();
    if let Some(first) = picked.first() {
        cx.app_mut().element = Some(first.clone());
        cx.app_mut().drawing = None;
    }
    set_rows(cx, strs.clone());
    Ok(SelectedResult { selected_ids: strs })
}

/// `{elementId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentResult {
    /// The current element (`null` when the scene has none).
    element_id: Option<String>,
}

#[command(
    id = "element.current.set",
    kind = ui,
    label = "Make element current",
    scopes = [ui],
    needs_project,
    example(title = "Make Bob current", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    gui = "Timeline and Exposure Sheet: click a row name or column header (M5)",
)]
pub(crate) fn element_current_set(cx: &mut Cx, p: ElementParams) -> Result<CurrentResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let scene = cx.document()?.scene_of(&id).cloned();
    let app = cx.app_mut();
    app.scene = scene;
    if app.element.as_ref() != Some(&id) {
        app.drawing = None;
    }
    app.element = Some(id.clone());
    Ok(CurrentResult { element_id: Some(id.as_str().to_owned()) })
}

/// `element.current.move` order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveOrder {
    /// Timeline order.
    Timeline,
    /// Exposure Sheet columns.
    #[default]
    Xsheet,
}

/// `element.current.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentMoveParams {
    /// Previous or next.
    direction: Direction,
    /// Which order.
    #[serde(default)]
    order: MoveOrder,
}

#[command(
    id = "element.current.move",
    kind = ui,
    label = "Previous/next element",
    scopes = [ui],
    needs_project,
    example(title = "Next Exposure Sheet column", params = r#"{"direction": "next"}"#),
    gui = "Keyboard: Previous / Next Element (in the Exposure Sheet) (M5)",
)]
pub(crate) fn element_current_move(cx: &mut Cx, p: CurrentMoveParams) -> Result<CurrentResult, CommandError> {
    let doc = cx.document()?.clone();
    let Some(sc) = cx.app().scene.clone().and_then(|s| doc.scene(&s).cloned()) else {
        return Ok(CurrentResult { element_id: None });
    };
    let order: Vec<Id> = match p.order {
        MoveOrder::Xsheet => sc
            .column_order()
            .iter()
            .filter(|c| doc.element(c).is_some_and(|e| e.visible_in_xsheet().unwrap_or(true)))
            .cloned()
            .collect(),
        MoveOrder::Timeline => doc.tree_order(&sc),
    };
    let cur = cx.app().element.clone();
    let pos = cur.as_ref().and_then(|c| order.iter().position(|x| x == c));
    let next = match (pos, p.direction) {
        (None, _) => order.first().cloned(),
        (Some(i), Direction::Next) => order.get(i + 1).cloned().or(cur),
        (Some(i), Direction::Previous) => i.checked_sub(1).and_then(|j| order.get(j).cloned()).or(cur),
    };
    let app = cx.app_mut();
    if app.element != next {
        app.drawing = None;
    }
    app.element.clone_from(&next);
    Ok(CurrentResult { element_id: next.map(|e| e.as_str().to_owned()) })
}
