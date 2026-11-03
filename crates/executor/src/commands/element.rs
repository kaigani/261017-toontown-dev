//! Element structure commands (04 §3–§11, §17.4; M3 C3.1): creation (batch, parent peg), rename, clone, duplicate,
//! delete, stacking order (Timeline levels and Exposure Sheet columns), parenting, colours, notes, composite
//! properties, bar timing, child exposure, media length, and the visibility, lock, annotation, collapse and solo
//! toggles.
//!
//! Every document change is staged through the executor as one undo step; names follow 04 §3.2/§4.2 (`Name`,
//! `Name_1`, … skipping used names; at most 64 characters); placement follows 04 §3.2 (Timeline: above the selected
//! row at its level, else top of the root level; Exposure Sheet: right of the selected column, else rightmost).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_anim::exposure as x;
use tt_anim::hierarchy as h;
use tt_model::document::{Document, EntryKey};
use tt_model::entities::element::{Element, ElementInit, ElementType, ParamSlot};
use tt_model::entities::scene::TreeNode;
use tt_model::value::Timestamp;
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timing::{
    Budget, CellChange, Changes, Column, as_u32, duplicate_name, element, element_id, element_name, emit_cells,
    emit_state, err, internal, op, parentable, scene_id, scene_names, to_json, tree_json, write_column,
};
use crate::app::{Mode, mode_of_view};
use crate::cx::Cx;
use crate::types::Warning;

fn one() -> u32 {
    1
}

/// Element types `element.create` accepts (04 §17.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CreateType {
    /// Vector drawing element.
    Drawing,
    /// Bitmap image element.
    Image,
    /// Sound element.
    Sound,
    /// Media element.
    Media,
    /// Annotation element.
    Annotation,
    /// Peg.
    Peg,
    /// Camera.
    Camera,
    /// Skeleton effect.
    Skeleton,
    /// Colour transform effect.
    ColourTransform,
    /// Clipping effect.
    Clipping,
    /// Drop shadow effect.
    DropShadow,
    /// Blur effect.
    Blur,
    /// Shake effect.
    Shake,
    /// Natural-phenomena particle effect.
    ParticleNature,
    /// Action/impact particle effect.
    ParticleImpact,
}

impl CreateType {
    const fn element_type(self) -> ElementType {
        match self {
            Self::Drawing => ElementType::Drawing,
            Self::Image => ElementType::Image,
            Self::Sound => ElementType::Sound,
            Self::Media => ElementType::Media,
            Self::Annotation => ElementType::Annotation,
            Self::Peg => ElementType::Peg,
            Self::Camera => ElementType::Camera,
            Self::Skeleton => ElementType::Skeleton,
            Self::ColourTransform => ElementType::ColourTransform,
            Self::Clipping => ElementType::Clipping,
            Self::DropShadow => ElementType::DropShadow,
            Self::Blur => ElementType::Blur,
            Self::Shake => ElementType::Shake,
            Self::ParticleNature => ElementType::ParticleNature,
            Self::ParticleImpact => ElementType::ParticleImpact,
        }
    }
}

/// The default base name of a type (04 §3.3).
fn default_base(t: ElementType) -> &'static str {
    match t {
        ElementType::Drawing => "Drawing",
        ElementType::Annotation => "Annotation",
        ElementType::Image => "Image",
        ElementType::Sound => "Sound",
        ElementType::Media => "Media",
        ElementType::Peg => "Peg",
        ElementType::Camera => "Camera",
        ElementType::Skeleton => "Skeleton",
        ElementType::Bone => "Bone",
        ElementType::EffectParameter => "Parameter",
        ElementType::ColourTransform => "Colour-Transform",
        ElementType::Clipping => "Clipping",
        ElementType::DropShadow => "Drop-Shadow",
        ElementType::Blur => "Blur",
        ElementType::Shake => "Shake",
        ElementType::ParticleNature => "Particles-Nature",
        ElementType::ParticleImpact => "Particles-Impact",
    }
}

/// `count` default names of a type in a scene: `Name`, `Name_1`, …; sound elements a sound-track style zero-padded
/// counter (`Sound-01`, `Sound-02`, …; 04 §3.3).
fn default_names(t: ElementType, taken: &BTreeSet<String>, count: usize) -> Vec<String> {
    if t == ElementType::Sound {
        let mut out = Vec::new();
        let mut n = 1u32;
        while out.len() < count {
            let name = format!("Sound-{n:02}");
            if !taken.contains(&name) {
                out.push(name);
            }
            n += 1;
        }
        return out;
    }
    x::unique_names(default_base(t), &|n| taken.contains(n), count, false)
}

/// One element to add: its JSON, where it goes in the tree, and whether it is an Exposure Sheet column.
struct NewElement {
    id: Id,
    json: Value,
    column: bool,
}

/// Builds a new element (and, for cell-based types, its new drawing bank). Effects that need parameter elements get
/// them as children (04 §2.9, §2.10; 10 §4.6: clipping → mask, particles → sprites with a sprite drawing element,
/// skeleton → bones and skin); returns the element's tree node with those children.
fn build(
    cx: &mut Cx,
    changes: &mut Changes,
    t: ElementType,
    name: String,
    bar: (u32, u32),
    taken: &mut BTreeSet<String>,
    extra: &mut Vec<NewElement>,
) -> Result<(NewElement, TreeNode), CommandError> {
    let id = cx.new_id(ObjectType::Element)?;
    let bank = if t.specific_properties().contains(&"drawingBankId") {
        let b = cx.new_id(ObjectType::DrawingBank)?;
        changes.push(
            EntryKey::Project,
            vec![op("add", "/drawingBanks/-", Some(json!({ "id": b.as_str(), "type": "drawingBank" })))],
        );
        changes.push(
            EntryKey::Bank(b.clone()),
            vec![op("add", "", Some(to_json(&tt_model::entities::drawing::DrawingBank::new(b.clone(), Vec::new()))?))],
        );
        Some(b)
    } else {
        None
    };
    taken.insert(name.clone());
    let base = name.clone();
    let el = Element::new(ElementInit {
        id: id.clone(),
        element_type: t,
        name,
        created_at: Timestamp::now(),
        drawing_bank_id: bank,
        bar,
        slot: None,
    });
    let mut node = TreeNode { id: id.clone(), children: Vec::new() };
    let params: &[(ParamSlot, &str)] = match t {
        ElementType::Clipping => &[(ParamSlot::Mask, "mask")],
        ElementType::ParticleNature | ElementType::ParticleImpact => &[(ParamSlot::Sprites, "sprites")],
        ElementType::Skeleton => &[(ParamSlot::Bones, "bones"), (ParamSlot::Skin, "skin")],
        _ => &[],
    };
    for (slot, suffix) in params {
        let pname = x::unique_names(&format!("{base}-{suffix}"), &|n| taken.contains(n), 1, false).concat();
        taken.insert(pname.clone());
        let pid = cx.new_id(ObjectType::Element)?;
        let p = Element::new(ElementInit {
            id: pid.clone(),
            element_type: ElementType::EffectParameter,
            name: pname,
            created_at: Timestamp::now(),
            drawing_bank_id: None,
            bar,
            slot: Some(*slot),
        });
        let mut pnode = TreeNode { id: pid.clone(), children: Vec::new() };
        if *slot == ParamSlot::Sprites {
            // The auto-created sprite drawing element holding the particle artwork (04 §2.9).
            let sname = x::unique_names(&format!("{base}-sprite"), &|n| taken.contains(n), 1, false).concat();
            let (sprite, snode) = build(cx, changes, ElementType::Drawing, sname, bar, taken, extra)?;
            pnode.children.push(snode);
            extra.push(sprite);
        }
        node.children.push(pnode);
        extra.push(NewElement { id: pid, json: to_json(&p)?, column: false });
    }
    Ok((NewElement { id, json: to_json(&el)?, column: t.is_cell_based() }, node))
}

/// Where new elements go: the Timeline level (`parent`) and position (`index`), and the Exposure Sheet column they
/// follow (`after`).
#[derive(Clone, Copy)]
struct Slot<'a> {
    parent: Option<&'a Id>,
    index: Option<usize>,
    after: Option<&'a Id>,
}

/// Stages new elements into a scene: entries, stubs, tree placement and columns.
fn place(
    doc: &Document,
    changes: &mut Changes,
    scene: &Id,
    news: &[NewElement],
    nodes: Vec<TreeNode>,
    slot: Slot<'_>,
) -> Result<(), CommandError> {
    let Slot { parent, index, after } = slot;
    let sc = doc.scene(scene).ok_or_else(|| internal("scene"))?;
    let mut tree = sc.tree().clone();
    for (k, n) in nodes.into_iter().enumerate() {
        if !h::attach(&mut tree, parent, index.map(|i| i + k), n) {
            return Err(internal("parent not in the tree"));
        }
    }
    let mut columns = sc.column_order().clone();
    let mut at = match after {
        Some(a) => columns.iter().position(|c| c == a).map_or(columns.len(), |i| i + 1),
        None => columns.len(),
    };
    let mut ops = Vec::new();
    for n in news {
        changes.push(EntryKey::Element(n.id.clone()), vec![op("add", "", Some(n.json.clone()))]);
        ops.push(op("add", "/elements/-", Some(json!({ "id": n.id.as_str(), "type": "element" }))));
        if n.column {
            columns.insert(at, n.id.clone());
            at += 1;
        }
    }
    ops.push(op("replace", "/tree", Some(tree_json(&tree)?)));
    ops.push(op("replace", "/columnOrder", Some(to_json(&columns)?)));
    changes.push(EntryKey::Scene(scene.clone()), ops);
    Ok(())
}

/// The first selected Timeline row (element) of a scene.
fn selected_row(cx: &Cx, doc: &Document, scene: &Id) -> Option<Id> {
    cx.app()
        .selection_of("timeline")
        .iter()
        .chain(cx.app().selection_of("staging").iter())
        .filter_map(|k| Id::parse(k).ok())
        .find(|id| doc.scene_of(id) == Some(scene))
}

/// The first selected Exposure Sheet column of a scene.
fn selected_column(cx: &Cx, doc: &Document, scene: &Id) -> Option<Id> {
    let cells = super::timing::selected_cells(cx).map(|s| s.0).unwrap_or_default();
    cells
        .into_iter()
        .find(|id| doc.scene_of(id) == Some(scene) && doc.element(id).is_some_and(|e| e.element_type().is_cell_based()))
}

fn scene_len(doc: &Document, scene: &Id) -> u32 {
    doc.scene(scene).map_or(1, |s| as_u32(usize::try_from(doc.scene_length(s)).unwrap_or(1)))
}

/// Warns that pegs, cameras and effects are hidden in drawing mode (04 §1.4).
fn warn_drawing_mode(cx: &mut Cx, t: ElementType, ids: &[String]) {
    let staging_only = matches!(t, ElementType::Peg | ElementType::Camera | ElementType::Skeleton) || t.is_effect();
    if staging_only && mode_of_view(cx.ui().central_panel()) == Mode::Drawing {
        cx.warn(
            "element.hiddenInDrawingMode",
            "the new elements cannot be seen or modified in drawing mode; switch to scene-planning mode to see them",
            ids.to_vec(),
        );
    }
}

// ----- element.create ---------------------------------------------------------------------------------------------

/// `element.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreateParams {
    /// Element type.
    #[serde(rename = "type")]
    ty: CreateType,
    /// Base name (default: the type's default name); unique in the scene, at most 64 characters.
    #[serde(default)]
    name: Option<String>,
    /// How many elements (1-99; `element.countOutOfRange` otherwise).
    #[serde(default = "one")]
    count: u32,
    /// Scene (default: the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// Parent (peg, effect, skeleton or camera) to create the elements under.
    #[serde(default)]
    parent_id: Option<String>,
    /// Element to insert directly above, at its level (default: above the selected row, else top of the root level).
    #[serde(default)]
    above: Option<String>,
    /// Exposure Sheet column to insert right of (default: right of the selected column, else rightmost).
    #[serde(default)]
    xsheet_after: Option<String>,
}

/// `{elementIds, names, warnings}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateResult {
    /// The created elements (parameter elements of effects excluded), first = current.
    element_ids: Vec<String>,
    /// Their names.
    names: Vec<String>,
    /// Non-blocking warnings (`element.hiddenInDrawingMode`).
    warnings: Vec<Warning>,
}

#[command(
    id = "element.create",
    kind = action,
    label = "New elements",
    scopes = [edit],
    undo = "New Elements",
    needs_project,
    example(title = "Add a drawing element to the current scene", params = r#"{"type": "drawing"}"#),
    example(title = "Three pegs named Leg, Leg_1, Leg_2 under Bob-P", params = r#"{"type": "peg", "name": "Leg", "count": 3, "parentId": "scene:\"Intro\"/element:\"Bob-P\""}"#),
    example(title = "A clipping effect with its mask row", params = r#"{"type": "clipping", "sceneId": "scene:\"Intro\""}"#),
    gui = "Element menu: New Elements / Add <type>; Timeline layer list add control; Exposure Sheet quick-add (M5)",
)]
pub(crate) fn element_create(cx: &mut Cx, p: CreateParams) -> Result<CreateResult, CommandError> {
    if !(1..=99).contains(&p.count) {
        return Err(err(
            "element.countOutOfRange",
            format!("count {} is outside 1-99", p.count),
            json!({ "pointer": "/count", "count": p.count, "minimum": 1, "maximum": 99 }),
        ));
    }
    let t = p.ty.element_type();
    let doc = cx.document()?.clone();
    let parent = match &p.parent_id {
        Some(r) => Some(element_id(cx, r, "/parentId")?),
        None => None,
    };
    let above = match &p.above {
        Some(r) => Some(element_id(cx, r, "/above")?),
        None => None,
    };
    let scene = match (&p.scene_id, &parent, &above) {
        (Some(s), _, _) => scene_id(cx, Some(s))?,
        (None, Some(e), _) | (None, None, Some(e)) => doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?,
        (None, None, None) => scene_id(cx, None)?,
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    for (e, ptr) in [(&parent, "/parentId"), (&above, "/above")] {
        if let Some(e) = e
            && doc.scene_of(e) != Some(&scene)
        {
            return Err(err(codes::ARG_INVALID, "the element is in another scene", json!({ "pointer": ptr })));
        }
    }
    if let Some(pid) = &parent {
        let pt = *element(&doc, pid)?.element_type();
        if !parentable(pt) || pt == ElementType::EffectParameter {
            return Err(not_parentable(&doc, pid));
        }
    }
    let mut taken = scene_names(&doc, sc);
    let names = match &p.name {
        Some(n) => {
            let base = element_name(n, "/name")?;
            if taken.contains(&base) {
                return Err(duplicate_name(&base, "/name"));
            }
            let mut v = vec![base.clone()];
            v.extend(x::unique_names(&base, &|n| taken.contains(n) || n == base, p.count as usize - 1, true));
            if v.iter().any(|n| n.chars().count() > 64) {
                return Err(err(
                    "element.nameTooLong",
                    "a derived name exceeds 64 characters",
                    json!({ "pointer": "/name" }),
                ));
            }
            v
        }
        None => default_names(t, &taken, p.count as usize),
    };
    // Placement (04 §3.2).
    let (tree_parent, index) = match (&parent, &above) {
        (Some(pid), None) => (Some(pid.clone()), Some(0)),
        (pp, Some(a)) => {
            let ps = h::parents(sc.tree());
            let lvl = ps.get(a).cloned().flatten();
            if pp.is_some() && pp.as_ref() != lvl.as_ref() {
                return Err(err(
                    codes::ARG_INVALID,
                    "`above` is not a child of `parentId`",
                    json!({ "pointer": "/above" }),
                ));
            }
            let i = h::level_ids(sc.tree(), lvl.as_ref()).iter().position(|x| x == a);
            (lvl, i)
        }
        (None, None) => match selected_row(cx, &doc, &scene) {
            Some(row) if p.scene_id.is_none() => {
                let lvl = h::parents(sc.tree()).get(&row).cloned().flatten();
                let i = h::level_ids(sc.tree(), lvl.as_ref()).iter().position(|x| *x == row);
                (lvl, i)
            }
            _ => (None, Some(0)),
        },
    };
    if let Some(pid) = &tree_parent
        && !parentable(*element(&doc, pid)?.element_type())
    {
        return Err(not_parentable(&doc, pid));
    }
    let after = match &p.xsheet_after {
        Some(r) => {
            let a = element_id(cx, r, "/xsheetAfter")?;
            if !sc.column_order().contains(&a) {
                return Err(err(
                    "element.notCellBased",
                    "`xsheetAfter` is not an Exposure Sheet column",
                    json!({ "pointer": "/xsheetAfter" }),
                ));
            }
            Some(a)
        }
        None if p.scene_id.is_none() => selected_column(cx, &doc, &scene),
        None => None,
    };
    let mut budget = Budget::new("creating elements");
    let bar = (1, scene_len(&doc, &scene));
    let mut changes = Changes::default();
    let mut news = Vec::new();
    let mut nodes = Vec::new();
    let mut extra = Vec::new();
    for name in &names {
        // Each element costs its tree placement (and parameter rows) before anything is staged.
        budget.spend(h::count(sc.tree()) + 4)?;
        let (n, node) = build(cx, &mut changes, t, name.clone(), bar, &mut taken, &mut extra)?;
        news.push(n);
        nodes.push(node);
    }
    let ids: Vec<String> = news.iter().map(|n| n.id.as_str().to_owned()).collect();
    news.extend(extra);
    let slot = Slot { parent: tree_parent.as_ref(), index, after: after.as_ref() };
    place(&doc, &mut changes, &scene, &news, nodes, slot)?;
    changes.stage(cx)?;
    // The new elements become the selection; the first becomes current (04 §3.2).
    cx.app_mut().set_selection("timeline", ids.clone());
    if let Some(first) = ids.first().and_then(|i| Id::parse(i).ok()) {
        cx.app_mut().scene = Some(scene.clone());
        cx.app_mut().element = Some(first);
        cx.app_mut().drawing = None;
    }
    warn_drawing_mode(cx, t, &ids);
    emit_state(cx, &scene, &ids, "created");
    let warnings = cx.peek_warnings();
    Ok(CreateResult { element_ids: ids, names, warnings })
}

fn not_parentable(doc: &Document, id: &Id) -> CommandError {
    let (name, t) = doc.element(id).map_or((String::new(), ""), |e| (e.name().clone(), e.element_type().as_str()));
    err(
        "element.notParentable",
        format!("{t} element `{name}` cannot have children (parents are pegs, effects, skeletons, bones and cameras)"),
        json!({ "elementId": id.as_str(), "elementType": t }),
    )
}

// ----- element.parentPeg.create --------------------------------------------------------------------------------------

/// `element.parentPeg.create` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ParentPegParams {
    /// Elements to attach (default: the selected rows, else the current element); one scene.
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Peg name (default: `<first element>-P`, unique).
    #[serde(default)]
    name: Option<String>,
}

/// `{pegId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParentPegResult {
    /// The new peg.
    peg_id: String,
    /// Its name.
    name: String,
}

/// The elements of an `elementIds?` parameter: given, else the selected rows, else the current element.
fn targets(cx: &Cx, given: Option<&[String]>) -> Result<Vec<Id>, CommandError> {
    match given {
        Some(list) => {
            let mut out = Vec::new();
            for (i, r) in list.iter().enumerate() {
                let id = element_id(cx, r, &format!("/elementIds/{i}"))?;
                if !out.contains(&id) {
                    out.push(id);
                }
            }
            Ok(out)
        }
        None => {
            let doc = cx.document()?;
            let mut rows: Vec<Id> = cx
                .app()
                .selection_of("timeline")
                .iter()
                .filter_map(|k| Id::parse(k).ok())
                .filter(|id| doc.element(id).is_some())
                .collect();
            if rows.is_empty() {
                rows.extend(super::timing::selected_cells(cx).map(|s| s.0).unwrap_or_default());
            }
            if rows.is_empty() {
                rows.extend(cx.app().element.clone());
            }
            if rows.is_empty() {
                return Err(err(
                    "element.notFound",
                    "no element was given and none is selected or current",
                    json!({ "pointer": "/elementIds" }),
                )
                .hint("pass elementIds, or select elements (element.select)"));
            }
            Ok(rows)
        }
    }
}

fn one_scene(doc: &Document, ids: &[Id]) -> Result<Id, CommandError> {
    let mut scene: Option<Id> = None;
    for e in ids {
        let s = doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?;
        match &scene {
            Some(x) if *x != s => {
                return Err(err(
                    codes::ARG_INVALID,
                    "the elements are in different scenes",
                    json!({ "pointer": "/elementIds" }),
                ));
            }
            _ => scene = Some(s),
        }
    }
    scene.ok_or_else(|| err("element.notFound", "no elements", json!({ "pointer": "/elementIds" })))
}

/// The frame range an element occupies (bar or exposed cells); `None` when empty.
fn extent(doc: &Document, id: &Id) -> Option<(u32, u32)> {
    let e = doc.element(id)?;
    if let (Some(s), Some(d)) = (e.start(), e.duration()) {
        return Some((*s, s.saturating_add(*d).saturating_sub(1)));
    }
    if let Some(r) = e.exposure() {
        let f = x::expand(r);
        let first = f.iter().position(Option::is_some)?;
        return Some((as_u32(first + 1), as_u32(x::last_exposed(&f))));
    }
    let mut lo = u32::MAX;
    let mut hi = 0;
    for p in e.placements().iter().flatten() {
        lo = lo.min(*p.start_frame());
        hi = hi.max(*p.stop_frame());
    }
    for s in e.sequences().iter().flatten() {
        lo = lo.min(*s.start_frame());
        hi = hi.max(*s.stop_frame());
    }
    (hi > 0).then_some((lo, hi))
}

#[command(
    id = "element.parentPeg.create",
    kind = action,
    label = "Add parent peg",
    scopes = [edit],
    undo = "Add Parent Peg",
    needs_project,
    example(title = "Put the Background image under a new peg", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Background\""]}"#),
    gui = "Element menu and Timeline quick-add: Add Parent Peg (M5 C5.1)",
)]
pub(crate) fn element_parent_peg_create(cx: &mut Cx, p: ParentPegParams) -> Result<ParentPegResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let scene = one_scene(&doc, &ids)?;
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("adding a parent peg");
    budget.spend(h::count(sc.tree()) * (ids.len() + 1))?;
    let mut taken = scene_names(&doc, sc);
    let name = match &p.name {
        Some(n) => {
            let n = element_name(n, "/name")?;
            if taken.contains(&n) {
                return Err(duplicate_name(&n, "/name"));
            }
            n
        }
        None => {
            let first = doc.element(&ids[0]).map(|e| e.name().clone()).unwrap_or_default();
            let base: String = format!("{first}-P").chars().take(64).collect();
            x::unique_names(&base, &|n| taken.contains(n), 1, false).concat()
        }
    };
    // The peg goes where the first element (in Timeline order) was; its bar covers the children's combined range.
    let placed = h::placed(sc.tree());
    let mut sorted: Vec<&h::Placed> = placed.iter().filter(|q| ids.contains(&q.id)).collect();
    sorted.sort_by_key(|q| placed.iter().position(|r| r.id == q.id));
    let first = sorted.first().ok_or_else(|| internal("placed"))?;
    let (level, index) = (first.parent.clone(), first.index);
    let (mut lo, mut hi) = (u32::MAX, 0);
    for e in &ids {
        if let Some((a, b)) = extent(&doc, e) {
            lo = lo.min(a);
            hi = hi.max(b);
        }
    }
    let bar = if hi == 0 { (1, scene_len(&doc, &scene)) } else { (lo, hi - lo + 1) };
    let mut changes = Changes::default();
    let mut extra = Vec::new();
    let (peg, mut node) = build(cx, &mut changes, ElementType::Peg, name.clone(), bar, &mut taken, &mut extra)?;
    let mut tree = sc.tree().clone();
    for q in &sorted {
        if let Some(n) = h::detach(&mut tree, &q.id) {
            node.children.push(n);
        }
    }
    // Insert at the first element's position (adjusted when earlier siblings moved under the peg).
    let siblings_before = sorted.iter().filter(|q| q.parent == level && q.index < index).count();
    if !h::attach(&mut tree, level.as_ref(), Some(index - siblings_before), node) {
        return Err(internal("level"));
    }
    changes.push(EntryKey::Element(peg.id.clone()), vec![op("add", "", Some(peg.json.clone()))]);
    changes.push(
        EntryKey::Scene(scene.clone()),
        vec![
            op("add", "/elements/-", Some(json!({ "id": peg.id.as_str(), "type": "element" }))),
            op("replace", "/tree", Some(tree_json(&tree)?)),
        ],
    );
    changes.stage(cx)?;
    warn_drawing_mode(cx, ElementType::Peg, &[peg.id.as_str().to_owned()]);
    emit_state(cx, &scene, &[peg.id.as_str().to_owned()], "created");
    emit_state(cx, &scene, &ids.iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>(), "reparented");
    Ok(ParentPegResult { peg_id: peg.id.as_str().to_owned(), name })
}

// ----- element.rename ------------------------------------------------------------------------------------------------

/// `element.rename` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RenameParams {
    /// The element.
    element_id: String,
    /// The new name (unique in the scene, at most 64 characters, none of `\ / : * ? < > " |` or `.`).
    name: String,
}

/// `{elementId, name}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameResult {
    /// The element.
    element_id: String,
    /// Its name.
    name: String,
}

#[command(
    id = "element.rename",
    kind = action,
    label = "Rename element",
    scopes = [edit],
    undo = "Rename Element",
    needs_project,
    example(title = "Rename Bob to Robert", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "name": "Robert"}"#),
    gui = "Timeline: double-click a layer name; Exposure Sheet: double-click a column header; Element menu: Rename (M5)",
)]
pub(crate) fn element_rename(cx: &mut Cx, p: RenameParams) -> Result<RenameResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let name = element_name(&p.name, "/name")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    if *e.name() == name {
        return Ok(RenameResult { element_id: id.as_str().to_owned(), name });
    }
    let sc = doc.scene_of(&id).and_then(|s| doc.scene(s)).ok_or_else(|| internal("scene"))?;
    if scene_names(&doc, sc).contains(&name) {
        return Err(duplicate_name(&name, "/name"));
    }
    let mut changes = Changes::default();
    changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/name", Some(json!(name)))]);
    changes.stage(cx)?;
    cx.set_label(format!("Rename {} Element", type_label(*e.element_type())));
    emit_state(cx, sc.id(), &[id.as_str().to_owned()], "renamed");
    Ok(RenameResult { element_id: id.as_str().to_owned(), name })
}

/// The undo-label name of an element type (04 §4.1: "the undo label names the element type").
fn type_label(t: ElementType) -> &'static str {
    match t {
        ElementType::Drawing => "Drawing",
        ElementType::Annotation => "Annotation",
        ElementType::Image => "Image",
        ElementType::Sound => "Sound",
        ElementType::Media => "Media",
        ElementType::Peg => "Peg",
        ElementType::Camera => "Camera",
        ElementType::Skeleton => "Skeleton",
        ElementType::Bone => "Bone",
        ElementType::EffectParameter => "Effect Parameter",
        _ => "Effect",
    }
}

// ----- element.clone / element.duplicate --------------------------------------------------------------------------------

/// `element.clone` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CloneParams {
    /// A cell-based element.
    element_id: String,
    /// Name (default: the original name with a unique suffix).
    #[serde(default)]
    name: Option<String>,
}

/// `{elementId, name, cloneGroupId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloneResult {
    /// The clone.
    element_id: String,
    /// Its name.
    name: String,
    /// The clone group (the shared drawing bank).
    clone_group_id: String,
}

/// A copy of an element's JSON under a new ID and name: channels, keys and motion points get new IDs (they are
/// addressable objects); opaque `meta`/`ext` and every other property are copied verbatim.
fn copied_element(cx: &mut Cx, doc: &Document, id: &Id, name: &str) -> Result<(Id, Value), CommandError> {
    let mut v = doc.entry_raw_json(&EntryKey::Element(id.clone())).map_err(internal)?;
    let new = cx.new_id(ObjectType::Element)?;
    v["id"] = json!(new.as_str());
    v["rev"] = json!(0);
    v["name"] = json!(name);
    v["createdAt"] = to_json(&Timestamp::now())?;
    let mut created = Vec::new();
    if let Some(Value::Object(m)) = v.get_mut("channels") {
        for c in m.values_mut() {
            let types = [ObjectType::Channel, ObjectType::Key, ObjectType::MotionPoint];
            super::edit_ops::regenerate(cx, c, &types, &mut created)?;
        }
    }
    for slot in ["placements", "sequences", "blocks"] {
        if let Some(Value::Array(a)) = v.get_mut(slot) {
            for item in a {
                super::edit_ops::regenerate(
                    cx,
                    item,
                    &[ObjectType::SoundPlacement, ObjectType::MediaSequence, ObjectType::ParticleBlock],
                    &mut created,
                )?;
            }
        }
    }
    Ok((new, v))
}

/// Inserts copies directly above their originals (Timeline) and right of them (Exposure Sheet).
fn place_copies(
    doc: &Document,
    changes: &mut Changes,
    scene: &Id,
    pairs: &[(Id, Id, Value)],
) -> Result<(), CommandError> {
    let sc = doc.scene(scene).ok_or_else(|| internal("scene"))?;
    let mut tree = sc.tree().clone();
    let mut columns = sc.column_order().clone();
    let mut ops = Vec::new();
    for (orig, new, json) in pairs {
        let ps = h::parents(&tree);
        let lvl = ps.get(orig).cloned().flatten();
        let i = h::level_ids(&tree, lvl.as_ref()).iter().position(|x| x == orig);
        h::attach(&mut tree, lvl.as_ref(), i, TreeNode { id: new.clone(), children: Vec::new() });
        if let Some(c) = columns.iter().position(|c| c == orig) {
            columns.insert(c + 1, new.clone());
        }
        changes.push(EntryKey::Element(new.clone()), vec![op("add", "", Some(json.clone()))]);
        ops.push(op("add", "/elements/-", Some(json!({ "id": new.as_str(), "type": "element" }))));
    }
    ops.push(op("replace", "/tree", Some(tree_json(&tree)?)));
    ops.push(op("replace", "/columnOrder", Some(to_json(&columns)?)));
    changes.push(EntryKey::Scene(scene.clone()), ops);
    Ok(())
}

#[command(
    id = "element.clone",
    kind = action,
    label = "Clone element",
    scopes = [edit],
    undo = "Clone Element",
    needs_project,
    example(title = "Clone Bob (shared drawings, own timing)", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\""}"#),
    gui = "Element menu, Timeline layer context, Exposure Sheet column header: Clone (M5)",
)]
pub(crate) fn element_clone(cx: &mut Cx, p: CloneParams) -> Result<CloneResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    let bank = match (e.element_type().has_exposure(), e.drawing_bank_id()) {
        (true, Some(b)) => b.clone(),
        _ => {
            return Err(err(
                "element.notCloneable",
                format!(
                    "{} element `{}` cannot be cloned: only drawing, image and annotation elements share a bank",
                    e.element_type().as_str(),
                    e.name()
                ),
                json!({ "elementId": id.as_str(), "elementType": e.element_type().as_str() }),
            ));
        }
    };
    let scene = doc.scene_of(&id).cloned().ok_or_else(|| internal("scene"))?;
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let taken = scene_names(&doc, sc);
    let name = match &p.name {
        Some(n) => {
            let n = element_name(n, "/name")?;
            if taken.contains(&n) {
                return Err(duplicate_name(&n, "/name"));
            }
            n
        }
        None => x::unique_names(e.name(), &|n| taken.contains(n), 1, true).concat(),
    };
    let mut budget = Budget::new("cloning an element");
    budget.spend(h::count(sc.tree()) + x::runs_len(e.exposure().as_deref().unwrap_or(&[])))?;
    let (new, json) = copied_element(cx, &doc, &id, &name)?;
    let mut changes = Changes::default();
    place_copies(&doc, &mut changes, &scene, &[(id.clone(), new.clone(), json)])?;
    changes.stage(cx)?;
    emit_state(cx, &scene, &[new.as_str().to_owned()], "created");
    Ok(CloneResult { element_id: new.as_str().to_owned(), name, clone_group_id: bank.as_str().to_owned() })
}

/// `element.duplicate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DuplicateParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
}

/// `{elementIds, names}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicateResult {
    /// The copies, in the order of the originals.
    element_ids: Vec<String>,
    /// Their names.
    names: Vec<String>,
}

#[command(
    id = "element.duplicate",
    kind = action,
    label = "Duplicate element",
    scopes = [edit],
    undo = "Duplicate Element",
    needs_project,
    example(title = "Duplicate Bob with copies of his drawings", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""]}"#),
    gui = "Element menu, Timeline layer context, Exposure Sheet column header: Duplicate (M5)",
)]
pub(crate) fn element_duplicate(cx: &mut Cx, p: DuplicateParams) -> Result<DuplicateResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("duplicating elements");
    // The whole subtree is copied (SI-094): mandatory parameter rows (masks, sprites, bones, skin) and ordinary
    // children alike, so an element given with one of its ancestors is copied once, with that ancestor. A parameter
    // row alone is refused: it never becomes an unowned root (10 §4.6; C31-R1-06).
    let mut roots: Vec<Id> = Vec::new();
    for id in &ids {
        let sc = super::timing::scene_of(&doc, id)?;
        let covered = ids.iter().any(|o| o != id && h::is_within(sc.tree(), id, o));
        if covered {
            continue;
        }
        let e = element(&doc, id)?;
        if *e.element_type() == ElementType::EffectParameter {
            return Err(err(
                codes::ARG_INVALID,
                format!("parameter element `{}` is duplicated only with its effect or skeleton", e.name()),
                json!({ "elementId": id.as_str() }),
            ));
        }
        roots.push(id.clone());
    }
    let mut changes = Changes::default();
    let mut out_ids = Vec::new();
    let mut names = Vec::new();
    let mut taken_by: BTreeMap<Id, BTreeSet<String>> = BTreeMap::new();
    // Banks shared inside the copied subtrees stay shared between the copies (clones of each other), once.
    let mut banks: BTreeMap<Id, (Id, BTreeMap<String, String>)> = BTreeMap::new();
    let mut by_scene: BTreeMap<Id, Vec<(Id, TreeNode, Vec<(Id, Id)>, Vec<(Id, Value)>)>> = BTreeMap::new();
    for root in &roots {
        let scene = doc.scene_of(root).cloned().ok_or_else(|| internal("scene"))?;
        let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
        let taken = taken_by.entry(scene.clone()).or_insert_with(|| scene_names(&doc, sc));
        let subtree: Vec<Id> = std::iter::once(root.clone()).chain(h::descendants(sc.tree(), root)).collect();
        budget.spend(h::count(sc.tree()) + subtree.len())?;
        let mut idmap: BTreeMap<String, String> = BTreeMap::new();
        let mut copies: Vec<(Id, Value)> = Vec::new();
        let mut columns: Vec<(Id, Id)> = Vec::new();
        for e_id in &subtree {
            let e = element(&doc, e_id)?;
            let name = x::unique_names(e.name(), &|n| taken.contains(n), 1, true).concat();
            taken.insert(name.clone());
            let (new, mut json) = copied_element(cx, &doc, e_id, &name)?;
            if let Some(bank) = e.drawing_bank_id() {
                if !banks.contains_key(bank) {
                    let made = copy_bank(cx, &doc, &mut changes, &mut budget, bank)?;
                    banks.insert(bank.clone(), made);
                }
                if let Some((nb, dmap)) = banks.get(bank) {
                    json["drawingBankId"] = json!(nb.as_str());
                    if let Some(Value::Array(runs)) = json.get_mut("exposure") {
                        budget.spend(runs.len())?;
                        for r in runs.iter_mut() {
                            if let Some(d) = r.get(0).and_then(Value::as_str).and_then(|d| dmap.get(d)) {
                                r[0] = json!(d);
                            }
                        }
                    }
                }
            }
            if sc.column_order().contains(e_id) {
                columns.push((e_id.clone(), new.clone()));
            }
            idmap.insert(e_id.as_str().to_owned(), new.as_str().to_owned());
            if e_id == root {
                out_ids.push(new.as_str().to_owned());
                names.push(name.clone());
            }
            copies.push((new, json));
        }
        // References between copied elements follow the copies (`x-ref: element` properties).
        for (_, json) in &mut copies {
            super::edit_ops::remap_element_refs(json, &idmap, "");
        }
        let original = h::find(sc.tree(), root).cloned().ok_or_else(|| internal("tree"))?;
        let node = renamed_node(&original, &idmap);
        by_scene.entry(scene).or_default().push((root.clone(), node, columns, copies));
    }
    for (scene, groups) in &by_scene {
        let sc = doc.scene(scene).ok_or_else(|| internal("scene"))?;
        let mut tree = sc.tree().clone();
        let mut columns = sc.column_order().clone();
        let mut ops = Vec::new();
        for (root, node, cols, copies) in groups {
            // The copy goes directly above its original, at its level (Timeline); columns right of theirs.
            let ps = h::parents(&tree);
            let lvl = ps.get(root).cloned().flatten();
            let i = h::level_ids(&tree, lvl.as_ref()).iter().position(|x| x == root);
            h::attach(&mut tree, lvl.as_ref(), i, node.clone());
            for (orig, new) in cols {
                if let Some(c) = columns.iter().position(|c| c == orig) {
                    columns.insert(c + 1, new.clone());
                }
            }
            for (new, json) in copies {
                changes.push(EntryKey::Element(new.clone()), vec![op("add", "", Some(json.clone()))]);
                ops.push(op("add", "/elements/-", Some(json!({ "id": new.as_str(), "type": "element" }))));
            }
        }
        ops.push(op("replace", "/tree", Some(tree_json(&tree)?)));
        ops.push(op("replace", "/columnOrder", Some(to_json(&columns)?)));
        changes.push(EntryKey::Scene(scene.clone()), ops);
    }
    changes.stage(cx)?;
    for (scene, groups) in &by_scene {
        let created: Vec<String> =
            groups.iter().flat_map(|(_, _, _, copies)| copies.iter().map(|(n, _)| n.as_str().to_owned())).collect();
        emit_state(cx, scene, &created, "created");
        // The copies' new banks announce their drawings to the copies using them.
        for (_, _, _, copies) in groups {
            for (new, json) in copies {
                if let Some(nb) = json.get("drawingBankId").and_then(Value::as_str)
                    && let Some((_, (_, dmap))) = banks.iter().find(|(_, (b, _))| b.as_str() == nb)
                    && !dmap.is_empty()
                {
                    let drawings: Vec<String> = dmap.values().cloned().collect();
                    cx.emit(
                        "element.drawing.changed",
                        json!({ "elementId": new.as_str(), "drawingIds": drawings, "change": "created" }),
                    );
                }
            }
        }
    }
    Ok(DuplicateResult { element_ids: out_ids, names })
}

/// A copy of a bank with a verbatim copy of every drawing (lock states kept): `(new bank, old → new drawing ID)`.
fn copy_bank(
    cx: &mut Cx,
    doc: &Document,
    changes: &mut Changes,
    budget: &mut Budget,
    bank: &Id,
) -> Result<(Id, BTreeMap<String, String>), CommandError> {
    let nb = cx.new_id(ObjectType::DrawingBank)?;
    changes.push(
        EntryKey::Project,
        vec![op("add", "/drawingBanks/-", Some(json!({ "id": nb.as_str(), "type": "drawingBank" })))],
    );
    changes.push(
        EntryKey::Bank(nb.clone()),
        vec![op("add", "", Some(to_json(&tt_model::entities::drawing::DrawingBank::new(nb.clone(), Vec::new()))?))],
    );
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let drawings = doc.bank(bank).map(|b| b.drawings().ids().to_vec()).unwrap_or_default();
    budget.spend(drawings.len())?;
    for d in drawings {
        let Some(dr) = doc.drawing(&d) else { continue };
        let dj = doc.entry_raw_json(&EntryKey::Drawing(d.clone())).map_err(internal)?;
        budget.spend(dr.nodes().len())?;
        let nd = super::timing::add_drawing(cx, changes, &nb, dr.name(), Some(&dj))?;
        if *dr.locked() {
            changes.push(EntryKey::Drawing(nd.clone()), vec![op("replace", "/locked", Some(json!(true)))]);
        }
        map.insert(d.as_str().to_owned(), nd.as_str().to_owned());
    }
    Ok((nb, map))
}

/// A tree node with every ID mapped.
fn renamed_node(n: &TreeNode, map: &BTreeMap<String, String>) -> TreeNode {
    TreeNode {
        id: map.get(n.id.as_str()).and_then(|s| Id::parse(s).ok()).unwrap_or_else(|| n.id.clone()),
        children: n.children.iter().map(|c| renamed_node(c, map)).collect(),
    }
}

// ----- element.delete ---------------------------------------------------------------------------------------------------

/// `element.delete` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DeleteParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
}

/// `{deletedIds, reparentedIds, deletedDrawingCount}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteResult {
    /// Deleted elements (with the parameter elements of deleted effects).
    deleted_ids: Vec<String>,
    /// Children re-attached to the deleted element's parent.
    reparented_ids: Vec<String>,
    /// Drawings deleted with banks no remaining element uses.
    deleted_drawing_count: u32,
}

#[command(
    id = "element.delete",
    kind = action,
    label = "Delete elements",
    scopes = [edit],
    undo = "Delete Elements",
    needs_project,
    example(title = "Delete the peg Bob-P; Bob moves to its level", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob-P\""]}"#),
    gui = "Element menu, Timeline layer context, Exposure Sheet column header: Delete (M5)",
)]
pub(crate) fn element_delete(cx: &mut Cx, p: DeleteParams) -> Result<DeleteResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("deleting elements");
    // Parameter elements are created and deleted only with their effect (10 §4.6).
    let mut gone: BTreeSet<Id> = BTreeSet::new();
    for id in &ids {
        let e = element(&doc, id)?;
        if *e.element_type() == ElementType::EffectParameter {
            return Err(err(
                codes::ARG_INVALID,
                format!("parameter element `{}` is deleted only with its effect", e.name()),
                json!({ "elementId": id.as_str() }),
            ));
        }
        gone.insert(id.clone());
        let sc = super::timing::scene_of(&doc, id)?;
        if let Some(n) = h::find(sc.tree(), id) {
            for c in &n.children {
                if doc.element(&c.id).is_some_and(|x| *x.element_type() == ElementType::EffectParameter) {
                    gone.insert(c.id.clone());
                }
            }
        }
    }
    let mut changes = Changes::default();
    let mut reparented = Vec::new();
    let mut by_scene: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for id in &gone {
        by_scene.entry(doc.scene_of(id).cloned().ok_or_else(|| internal("scene"))?).or_default().push(id.clone());
    }
    for (scene, list) in &by_scene {
        let sc = doc.scene(scene).ok_or_else(|| internal("scene"))?;
        budget.spend(h::count(sc.tree()) + sc.elements().ids().len())?;
        for id in list {
            if let Some(n) = h::find(sc.tree(), id) {
                for c in &n.children {
                    if !gone.contains(&c.id) {
                        reparented.push(c.id.as_str().to_owned());
                    }
                }
            }
        }
        let tree = h::without(sc.tree(), &|i| gone.contains(i));
        let stubs: Vec<Value> = sc
            .elements()
            .ids()
            .iter()
            .filter(|e| !gone.contains(*e))
            .map(|e| json!({ "id": e.as_str(), "type": "element" }))
            .collect();
        let columns: Vec<&Id> = sc.column_order().iter().filter(|c| !gone.contains(*c)).collect();
        let mut ops = vec![
            op("replace", "/elements", Some(Value::Array(stubs))),
            op("replace", "/tree", Some(tree_json(&tree)?)),
            op("replace", "/columnOrder", Some(to_json(&columns)?)),
        ];
        if sc.current_camera_id().as_ref().is_some_and(|c| gone.contains(c)) {
            let next = h::placed(&tree)
                .into_iter()
                .find(|q| doc.element(&q.id).is_some_and(|e| *e.element_type() == ElementType::Camera))
                .map(|q| q.id.as_str().to_owned());
            ops.push(op("replace", "/currentCameraId", Some(json!(next))));
        }
        changes.push(EntryKey::Scene(scene.clone()), ops);
        for id in list {
            changes.push(EntryKey::Element(id.clone()), vec![op("remove", "", None)]);
        }
        // References from the remaining elements (lip-sync mappings and other `x-ref: element` properties).
        let names: BTreeSet<String> = gone.iter().map(|i| i.as_str().to_owned()).collect();
        for other in sc.elements().ids() {
            if gone.contains(other) {
                continue;
            }
            let key = EntryKey::Element(other.clone());
            let fixes = super::edit_ops::element_reference_fixes(&doc.entry_raw_json(&key).map_err(internal)?, &names);
            changes.push(key, fixes);
        }
    }
    // Banks no remaining element uses go with their drawings (10 §4.8).
    let mut deleted_drawings = 0u32;
    let banks: BTreeSet<Id> =
        gone.iter().filter_map(|e| doc.element(e).and_then(|x| x.drawing_bank_id().clone())).collect();
    let mut bank_stubs_removed = Vec::new();
    for b in &banks {
        if doc.bank_users(b).iter().all(|u| gone.contains(u)) {
            let drawings = doc.bank(b).map(|x| x.drawings().ids().to_vec()).unwrap_or_default();
            budget.spend(drawings.len())?;
            for d in &drawings {
                changes.push(EntryKey::Drawing(d.clone()), vec![op("remove", "", None)]);
            }
            deleted_drawings += as_u32(drawings.len());
            changes.push(EntryKey::Bank(b.clone()), vec![op("remove", "", None)]);
            bank_stubs_removed.push(b.clone());
        }
    }
    if !bank_stubs_removed.is_empty() {
        let stubs: Vec<Value> = doc
            .project()
            .drawing_banks()
            .ids()
            .iter()
            .filter(|b| !bank_stubs_removed.contains(b))
            .map(|b| json!({ "id": b.as_str(), "type": "drawingBank" }))
            .collect();
        changes.push(EntryKey::Project, vec![op("replace", "/drawingBanks", Some(Value::Array(stubs)))]);
    }
    changes.stage(cx)?;
    for (scene, list) in &by_scene {
        emit_state(cx, scene, &list.iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>(), "deleted");
    }
    Ok(DeleteResult {
        deleted_ids: gone.iter().map(|i| i.as_str().to_owned()).collect(),
        reparented_ids: reparented,
        deleted_drawing_count: deleted_drawings,
    })
}

// ----- order ----------------------------------------------------------------------------------------------------------------

/// Which order a reorder changes (04 §1.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OrderKind {
    /// Timeline rows within a hierarchy level.
    #[default]
    Timeline,
    /// Exposure Sheet columns (left = back, right = front).
    Xsheet,
}

/// Stacking direction (04 §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum To {
    /// Above all others of the level / rightmost column.
    Front,
    /// One up / one right.
    Forward,
    /// One down / one left.
    Backward,
    /// Below all others / leftmost.
    Back,
}

/// `element.order.move` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OrderMoveParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Direction.
    to: To,
    /// Timeline level or Exposure Sheet columns.
    #[serde(default)]
    order: OrderKind,
}

/// `{changed, order}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct OrderMoveResult {
    /// Whether anything moved (false at the boundary).
    changed: bool,
    /// The resulting order of the affected level or of the columns.
    order: Vec<String>,
}

/// Moves `picked` within `list` (index 0 = front for the Timeline, back for the Exposure Sheet).
fn moved(list: &[Id], picked: &BTreeSet<Id>, to: To, front_first: bool) -> Vec<Id> {
    let (sel, rest): (Vec<Id>, Vec<Id>) = list.iter().cloned().partition(|i| picked.contains(i));
    let to_front = matches!(to, To::Front | To::Forward);
    match (to, front_first) {
        (To::Front, true) | (To::Back, false) => sel.into_iter().chain(rest).collect(),
        (To::Back, true) | (To::Front, false) => rest.into_iter().chain(sel).collect(),
        _ => {
            // One step: towards index 0 when (forward and front-first) or (backward and back-first).
            let towards_zero = to_front == front_first;
            let mut v = list.to_vec();
            if towards_zero {
                for i in 1..v.len() {
                    if picked.contains(&v[i]) && !picked.contains(&v[i - 1]) {
                        v.swap(i, i - 1);
                    }
                }
            } else {
                for i in (0..v.len().saturating_sub(1)).rev() {
                    if picked.contains(&v[i]) && !picked.contains(&v[i + 1]) {
                        v.swap(i, i + 1);
                    }
                }
            }
            v
        }
    }
}

#[command(
    id = "element.order.move",
    kind = action,
    label = "Arrange",
    scopes = [edit],
    undo = "Arrange",
    needs_project,
    example(title = "Bring the Background image to the front of its Timeline level", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Background\""], "to": "front"}"#),
    example(title = "Move Bob's column one step to the right in the Exposure Sheet", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "to": "forward", "order": "xsheet"}"#),
    gui = "Element menu, Timeline layer context, camera view context, Exposure Sheet column header: Arrange (M5)",
)]
pub(crate) fn element_order_move(cx: &mut Cx, p: OrderMoveParams) -> Result<OrderMoveResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let scene = one_scene(&doc, &ids)?;
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("arranging elements");
    budget.spend(h::count(sc.tree()) * 2)?;
    let picked: BTreeSet<Id> = ids.iter().cloned().collect();
    let mut changes = Changes::default();
    let (changed, order) = match p.order {
        OrderKind::Xsheet => {
            for id in &ids {
                if !sc.column_order().contains(id) {
                    return Err(err(
                        "element.notCellBased",
                        "only cell-based elements have Exposure Sheet columns",
                        json!({ "elementId": id.as_str() }),
                    ));
                }
            }
            let after = moved(sc.column_order(), &picked, p.to, false);
            let changed = after != *sc.column_order();
            if changed {
                changes
                    .push(EntryKey::Scene(scene.clone()), vec![op("replace", "/columnOrder", Some(to_json(&after)?))]);
            }
            (changed, after)
        }
        OrderKind::Timeline => {
            let ps = h::parents(sc.tree());
            let lvl = ps.get(&ids[0]).cloned().flatten();
            if ids.iter().any(|i| ps.get(i).cloned().flatten() != lvl) {
                return Err(err(
                    codes::ARG_INVALID,
                    "the elements are on different hierarchy levels",
                    json!({ "pointer": "/elementIds" }),
                ));
            }
            let level = h::level_ids(sc.tree(), lvl.as_ref());
            let after = moved(&level, &picked, p.to, true);
            let changed = after != level;
            if changed {
                let mut tree = sc.tree().clone();
                h::reorder(&mut tree, lvl.as_ref(), &after);
                changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/tree", Some(tree_json(&tree)?))]);
            }
            (changed, after)
        }
    };
    changes.stage(cx)?;
    if changed {
        emit_state(cx, &scene, &ids.iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>(), "reordered");
    }
    Ok(OrderMoveResult { changed, order: order.iter().map(|i| i.as_str().to_owned()).collect() })
}

/// `element.order.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OrderSetParams {
    /// Timeline level or Exposure Sheet columns.
    order: OrderKind,
    /// The complete new order (a permutation of the level or of the columns).
    element_ids: Vec<String>,
    /// The Timeline level (its parent; default the root level).
    #[serde(default)]
    parent_id: Option<String>,
    /// Scene (default: the scene of the elements, else the current scene).
    #[serde(default)]
    scene_id: Option<String>,
}

/// `{order}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct OrderSetResult {
    /// The order.
    order: Vec<String>,
}

#[command(
    id = "element.order.set",
    kind = action,
    label = "Set order",
    scopes = [edit],
    undo = "Reorder Elements",
    needs_project,
    example(title = "Reorder the Exposure Sheet columns", params = r#"{"order": "xsheet", "elementIds": ["scene:\"Intro\"/element:\"Background\"", "scene:\"Intro\"/element:\"Bob\"", "scene:\"Intro\"/element:\"Notes\"", "scene:\"Intro\"/element:\"Dialogue\"", "scene:\"Intro\"/element:\"Reference\""]}"#),
    gui = "Timeline: drag rows between rows; Exposure Sheet: drag column headers; element list panel drag (M5)",
)]
pub(crate) fn element_order_set(cx: &mut Cx, p: OrderSetParams) -> Result<OrderSetResult, CommandError> {
    let mut ids = Vec::new();
    for (i, r) in p.element_ids.iter().enumerate() {
        ids.push(element_id(cx, r, &format!("/elementIds/{i}"))?);
    }
    let doc = cx.document()?.clone();
    let parent = match &p.parent_id {
        Some(r) => Some(element_id(cx, r, "/parentId")?),
        None => None,
    };
    let scene = match (&p.scene_id, ids.first(), &parent) {
        (Some(s), _, _) => scene_id(cx, Some(s))?,
        (None, Some(e), _) | (None, None, Some(e)) => doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?,
        (None, None, None) => scene_id(cx, None)?,
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("reordering elements");
    budget.spend(h::count(sc.tree()) + ids.len())?;
    let not_perm = |what: &str| {
        err(
            codes::ARG_INVALID,
            format!("elementIds must list every {what} exactly once"),
            json!({ "pointer": "/elementIds" }),
        )
    };
    let mut changes = Changes::default();
    match p.order {
        OrderKind::Xsheet => {
            let mut a: Vec<&Id> = ids.iter().collect();
            let mut b: Vec<&Id> = sc.column_order().iter().collect();
            a.sort();
            b.sort();
            if a != b {
                return Err(not_perm("Exposure Sheet column of the scene"));
            }
            if ids != *sc.column_order() {
                changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/columnOrder", Some(to_json(&ids)?))]);
            }
        }
        OrderKind::Timeline => {
            let mut tree = sc.tree().clone();
            if !h::reorder(&mut tree, parent.as_ref(), &ids) {
                return Err(not_perm("element of the Timeline level"));
            }
            if tree != *sc.tree() {
                changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/tree", Some(tree_json(&tree)?))]);
            }
        }
    }
    let changed = !changes.is_empty();
    changes.stage(cx)?;
    if changed {
        emit_state(cx, &scene, &ids.iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>(), "reordered");
    }
    Ok(OrderSetResult { order: ids.iter().map(|i| i.as_str().to_owned()).collect() })
}

// ----- parenting --------------------------------------------------------------------------------------------------------

/// `element.parent.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ParentSetParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// The parent: a peg, effect, skeleton, bone or camera.
    #[serde(default)]
    parent_id: Option<String>,
    /// Attach to the nearest peg or effect above at the same level (attach to next object).
    #[serde(default)]
    nearest: bool,
    /// Position among the parent's children (default: the bottom).
    #[serde(default)]
    index: Option<u32>,
}

/// `{elementIds, parentId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParentResult {
    /// The elements.
    element_ids: Vec<String>,
    /// The parent (`null` at the root level).
    parent_id: Option<String>,
}

#[command(
    id = "element.parent.set",
    kind = action,
    label = "Attach to parent",
    scopes = [edit],
    undo = "Attach",
    needs_project,
    example(title = "Attach the Background image to Bob-P", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Background\""], "parentId": "scene:\"Intro\"/element:\"Bob-P\""}"#),
    example(title = "Attach to the nearest peg or effect above", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Background\""], "nearest": true}"#),
    gui = "Timeline: drop rows onto a peg/effect row; layer context: Attach to Next Object (M5 C5.1)",
)]
pub(crate) fn element_parent_set(cx: &mut Cx, p: ParentSetParams) -> Result<ParentResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let scene = one_scene(&doc, &ids)?;
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("attaching elements");
    budget.spend(h::count(sc.tree()) * (ids.len() + 1))?;
    let parent = match (&p.parent_id, p.nearest) {
        (Some(r), false) => element_id(cx, r, "/parentId")?,
        (None, true) => {
            // The nearest peg or effect above the first element at its level.
            let ps = h::parents(sc.tree());
            let lvl = ps.get(&ids[0]).cloned().flatten();
            let level = h::level_ids(sc.tree(), lvl.as_ref());
            let i = level.iter().position(|x| *x == ids[0]).unwrap_or(0);
            level[..i]
                .iter()
                .rev()
                .find(|e| {
                    doc.element(e)
                        .is_some_and(|x| *x.element_type() == ElementType::Peg || x.element_type().is_effect())
                })
                .cloned()
                .ok_or_else(|| {
                    err(
                        "element.notParentable",
                        "there is no peg or effect above at the same level",
                        json!({ "pointer": "/nearest" }),
                    )
                })?
        }
        _ => {
            return Err(err(
                codes::ARG_INVALID,
                "give exactly one of `parentId` and `nearest: true`",
                json!({ "pointer": "/parentId" }),
            ));
        }
    };
    if doc.scene_of(&parent) != Some(&scene) {
        return Err(err(codes::ARG_INVALID, "the parent is in another scene", json!({ "pointer": "/parentId" })));
    }
    let pt = *element(&doc, &parent)?.element_type();
    if !parentable(pt) || pt == ElementType::EffectParameter {
        return Err(not_parentable(&doc, &parent));
    }
    let mut tree = sc.tree().clone();
    for id in &ids {
        if h::is_within(&tree, &parent, id) {
            return Err(err(
                "element.hierarchyCycle",
                "an element cannot be attached to itself or to one of its descendants",
                json!({ "elementId": id.as_str(), "parentId": parent.as_str() }),
            ));
        }
        if doc.element(id).is_some_and(|e| *e.element_type() == ElementType::EffectParameter) {
            return Err(err(
                codes::ARG_INVALID,
                "parameter elements stay under their effect",
                json!({ "elementId": id.as_str() }),
            ));
        }
    }
    let mut index = p.index.map(|i| i as usize);
    for id in &ids {
        if let Some(n) = h::detach(&mut tree, id) {
            h::attach(&mut tree, Some(&parent), index, n);
            index = index.map(|i| i + 1);
        }
    }
    let mut changes = Changes::default();
    if tree != *sc.tree() {
        changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/tree", Some(tree_json(&tree)?))]);
    }
    changes.stage(cx)?;
    let strs: Vec<String> = ids.iter().map(|i| i.as_str().to_owned()).collect();
    emit_state(cx, &scene, &strs, "reparented");
    Ok(ParentResult { element_ids: strs, parent_id: Some(parent.as_str().to_owned()) })
}

/// `element.parent.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ParentRemoveParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
}

#[command(
    id = "element.parent.remove",
    kind = action,
    label = "Detach from parent",
    scopes = [edit],
    undo = "Detach",
    needs_project,
    example(title = "Detach Bob from Bob-P; Bob moves directly above it", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""]}"#),
    gui = "Timeline layer context: Detach from Parent (M5 C5.1)",
)]
pub(crate) fn element_parent_remove(cx: &mut Cx, p: ParentRemoveParams) -> Result<ParentResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let scene = one_scene(&doc, &ids)?;
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let mut budget = Budget::new("detaching elements");
    budget.spend(h::count(sc.tree()) * (ids.len() + 1))?;
    let mut tree = sc.tree().clone();
    let mut new_parent: Option<Id> = None;
    for id in &ids {
        if doc.element(id).is_some_and(|e| *e.element_type() == ElementType::EffectParameter) {
            return Err(err(
                codes::ARG_INVALID,
                "parameter elements stay under their effect",
                json!({ "elementId": id.as_str() }),
            ));
        }
        let ps = h::parents(&tree);
        let Some(Some(parent)) = ps.get(id).cloned() else { continue };
        let grand = ps.get(&parent).cloned().flatten();
        // It moves to the parent's level, directly above the parent (04 §9.2).
        if let Some(n) = h::detach(&mut tree, id) {
            let i = h::level_ids(&tree, grand.as_ref()).iter().position(|x| *x == parent);
            h::attach(&mut tree, grand.as_ref(), i, n);
        }
        new_parent = grand;
    }
    let mut changes = Changes::default();
    if tree != *sc.tree() {
        changes.push(EntryKey::Scene(scene.clone()), vec![op("replace", "/tree", Some(tree_json(&tree)?))]);
    }
    changes.stage(cx)?;
    let strs: Vec<String> = ids.iter().map(|i| i.as_str().to_owned()).collect();
    emit_state(cx, &scene, &strs, "reparented");
    Ok(ParentResult { element_ids: strs, parent_id: new_parent.map(|p| p.as_str().to_owned()) })
}

// ----- colours, notes, properties ---------------------------------------------------------------------------------------

/// `element.colour.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ColourParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Marker colour; an empty string reverts to the category default.
    #[serde(default)]
    #[schemars(regex(pattern = r"^(#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?)?$"))]
    marker: Option<String>,
    /// Timeline background colour; an empty string reverts to the category default.
    #[serde(default)]
    #[schemars(regex(pattern = r"^(#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?)?$"))]
    background: Option<String>,
}

/// `{elementIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdsResult {
    /// The elements.
    element_ids: Vec<String>,
}

fn colour_value(v: &str, pointer: &str) -> Result<Value, CommandError> {
    if v.is_empty() {
        return Ok(Value::Null);
    }
    let c = tt_model::value::Colour::parse(v)
        .map_err(|e| err(codes::ARG_INVALID, e.message().to_owned(), json!({ "pointer": pointer })))?;
    Ok(json!(c.to_string()))
}

#[command(
    id = "element.colour.set",
    kind = action,
    label = "Set element colour",
    scopes = [edit],
    undo = "Set Element Colour",
    needs_project,
    example(title = "Mark Bob red", params = r##"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "marker": "#FF0000"}"##),
    example(title = "Revert Bob's marker to the category default", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "marker": ""}"#),
    gui = "Element menu, Timeline layer context, Exposure Sheet column header: Marker / Background / Default Colour (M5)",
)]
pub(crate) fn element_colour_set(cx: &mut Cx, p: ColourParams) -> Result<IdsResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let marker = p.marker.as_deref().map(|m| colour_value(m, "/marker")).transpose()?;
    let background = p.background.as_deref().map(|m| colour_value(m, "/background")).transpose()?;
    let doc = cx.document()?.clone();
    let mut changes = Changes::default();
    let mut changed = Vec::new();
    for id in &ids {
        let e = element(&doc, id)?;
        let mut ops = Vec::new();
        if let Some(m) = &marker
            && to_json(e.marker_colour())? != *m
        {
            ops.push(op("replace", "/markerColour", Some(m.clone())));
        }
        if let Some(b) = &background
            && to_json(e.background_colour())? != *b
        {
            ops.push(op("replace", "/backgroundColour", Some(b.clone())));
        }
        if !ops.is_empty() {
            changed.push(id.clone());
        }
        changes.push(EntryKey::Element(id.clone()), ops);
    }
    changes.stage(cx)?;
    state_by_scene(cx, &doc, &changed, "properties");
    Ok(IdsResult { element_ids: ids.iter().map(|i| i.as_str().to_owned()).collect() })
}

fn state_by_scene(cx: &mut Cx, doc: &Document, ids: &[Id], change: &str) {
    let mut by: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    for i in ids {
        if let Some(s) = doc.scene_of(i) {
            by.entry(s.clone()).or_default().push(i.as_str().to_owned());
        }
    }
    for (s, l) in by {
        emit_state(cx, &s, &l, change);
    }
}

/// `element.note.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct NoteParams {
    /// The element.
    element_id: String,
    /// The note (empty removes it; at most 10 000 characters).
    #[schemars(length(max = 10_000))]
    text: String,
}

/// `{elementId}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdResult {
    /// The element.
    element_id: String,
}

#[command(
    id = "element.note.set",
    kind = action,
    label = "Set element note",
    scopes = [edit],
    undo = "Set Element Note",
    needs_project,
    example(title = "Note on Bob", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "text": "redraw the hands"}"#),
    gui = "Element menu, Timeline layer context, Exposure Sheet column header: Set Element Note (M5)",
)]
pub(crate) fn element_note_set(cx: &mut Cx, p: NoteParams) -> Result<IdResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    if *e.note() != p.text {
        let mut changes = Changes::default();
        let o = if p.text.is_empty() {
            op("remove", "/note", None)
        } else if e.note().is_empty() {
            op("add", "/note", Some(json!(p.text)))
        } else {
            op("replace", "/note", Some(json!(p.text)))
        };
        changes.push(EntryKey::Element(id.clone()), vec![o]);
        changes.stage(cx)?;
        state_by_scene(cx, &doc, std::slice::from_ref(&id), "properties");
    }
    Ok(IdResult { element_id: id.as_str().to_owned() })
}

/// Composite role (04 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RoleParam {
    /// Normal.
    Normal,
    /// In front of all normal elements.
    Foreground,
    /// Behind all normal elements; the last exposure extends to the scene length.
    Background,
    /// Deformation envelope (skin).
    Envelope,
}

/// `element.properties.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PropertiesParams {
    /// The element.
    element_id: String,
    /// Composite role.
    #[serde(default)]
    role: Option<RoleParam>,
    /// Deformable by skeleton bones.
    #[serde(default)]
    deformable: Option<bool>,
}

#[command(
    id = "element.properties.set",
    kind = action,
    label = "Set element properties",
    scopes = [edit],
    undo = "Set Element Properties",
    needs_project,
    example(title = "Make Bob a foreground element", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "role": "foreground"}"#),
    gui = "Properties view: Element panel, Type and Deformable (M5 C5.2)",
)]
pub(crate) fn element_properties_set(cx: &mut Cx, p: PropertiesParams) -> Result<IdResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    let mut ops = Vec::new();
    if let Some(r) = p.role {
        let v = to_json(&match r {
            RoleParam::Normal => tt_model::entities::element::Role::Normal,
            RoleParam::Foreground => tt_model::entities::element::Role::Foreground,
            RoleParam::Background => tt_model::entities::element::Role::Background,
            RoleParam::Envelope => tt_model::entities::element::Role::Envelope,
        })?;
        if to_json(e.role())? != v {
            ops.push(op("replace", "/role", Some(v)));
        }
    }
    if let Some(d) = p.deformable
        && *e.deformable() != d
    {
        ops.push(op("replace", "/deformable", Some(json!(d))));
    }
    if !ops.is_empty() {
        let mut changes = Changes::default();
        changes.push(EntryKey::Element(id.clone()), ops);
        changes.stage(cx)?;
        state_by_scene(cx, &doc, std::slice::from_ref(&id), "properties");
    }
    Ok(IdResult { element_id: id.as_str().to_owned() })
}

// ----- timing --------------------------------------------------------------------------------------------------------

/// Which end a duration change keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Anchor {
    /// Keep the start (stretch from the end).
    #[default]
    Start,
    /// Keep the end (stretch from the start).
    End,
}

/// What happens to keyframes when the duration changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum KeyMode {
    /// Keys keep their local frames; keys beyond the new end are removed.
    #[default]
    Keep,
    /// Keys are re-timed proportionally (05 §8.4).
    Scale,
}

/// `element.timing.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TimingParams {
    /// Bar elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// New start frame (1 … 100 000).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    start: Option<i64>,
    /// Move the start by this many frames (−100 000 … 100 000).
    #[serde(default)]
    #[schemars(range(min = -100_000, max = 100_000))]
    start_delta: Option<i64>,
    /// New duration (1 … 100 000).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    duration: Option<i64>,
    /// Which end a duration change keeps.
    #[serde(default)]
    anchor: Anchor,
    /// Keep or scale keyframes.
    #[serde(default)]
    keyframes: KeyMode,
    /// Loops (1 = not looped; at most 100 000).
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    loops: Option<i64>,
}

/// One element's bar.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Bar {
    /// The element.
    id: String,
    /// Start frame.
    #[schemars(range(min = 1))]
    start: u32,
    /// Duration.
    #[schemars(range(min = 1))]
    duration: u32,
    /// Loops.
    #[schemars(range(min = 1))]
    loops: u32,
}

/// `{elements}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TimingResult {
    /// The bars after the change.
    elements: Vec<Bar>,
}

/// Whether an element type loops (05 §8.4, 06 §2.6: pegs, skeletons, bones and the non-particle effects).
const fn loopable(t: ElementType) -> bool {
    matches!(
        t,
        ElementType::Peg
            | ElementType::Skeleton
            | ElementType::Bone
            | ElementType::ColourTransform
            | ElementType::Clipping
            | ElementType::DropShadow
            | ElementType::Blur
            | ElementType::Shake
    )
}

/// Re-times the keys of one element's channels (local frames, 05 §2.10) from a cycle of `old` frames to `new`:
/// `keep` removes keys beyond `new`; `scale` maps local frame k to round(1 + (k − 1)(new − 1)/(old − 1)) with the
/// later key winning a collision (05 §8.4). Motion-path keyframes follow the same rule; control points between
/// removed keyframes go with them. Returns the patch of the `channels` map, if anything changed.
fn retimed_channels(
    channels: &Value,
    old: u32,
    new: u32,
    mode: KeyMode,
    budget: &mut Budget,
) -> Result<Option<Value>, CommandError> {
    let Value::Object(map) = channels else { return Ok(None) };
    let map_frame = |k: u64| -> u64 {
        match mode {
            KeyMode::Keep => k,
            KeyMode::Scale if old <= 1 => 1,
            KeyMode::Scale => {
                // round(1 + (k − 1)(new − 1)/(old − 1)), half up, in exact integers.
                let num = (k.saturating_sub(1)) * u64::from(new - 1) * 2 + u64::from(old - 1);
                1 + num / (u64::from(old - 1) * 2)
            }
        }
    };
    let mut out = serde_json::Map::new();
    let mut changed = false;
    for (name, c) in map {
        let mut c = c.clone();
        if let Some(Value::Array(keys)) = c.get_mut("keys") {
            budget.spend(keys.len())?;
            let mut by_frame: BTreeMap<u64, Value> = BTreeMap::new();
            for k in keys.iter() {
                let f = k.get("frame").and_then(Value::as_u64).unwrap_or(1);
                let nf = map_frame(f);
                if nf > u64::from(new) {
                    continue;
                }
                let mut k = k.clone();
                k["frame"] = json!(nf);
                // Keys arrive sorted, so a later key replaces an earlier one at the same frame.
                by_frame.insert(nf, k);
            }
            let next: Vec<Value> = by_frame.into_values().collect();
            if next != *keys {
                changed = true;
                *keys = next;
            }
            if keys.is_empty() {
                changed = true;
                continue;
            }
        }
        if let Some(Value::Array(points)) = c.get_mut("points") {
            budget.spend(points.len())?;
            let mut next: Vec<Value> = Vec::new();
            let mut last_key: Option<u64> = None;
            for pt in points.iter() {
                match pt.get("frame").and_then(Value::as_u64) {
                    Some(f) => {
                        let nf = map_frame(f);
                        if nf > u64::from(new) {
                            break;
                        }
                        if last_key == Some(nf) {
                            // A collision: the later keyframe wins; the earlier one and the control points after it go.
                            while next.last().is_some_and(|p| p.get("frame").is_none_or(Value::is_null)) {
                                next.pop();
                            }
                            next.pop();
                        }
                        let mut pt = pt.clone();
                        pt["frame"] = json!(nf);
                        last_key = Some(nf);
                        next.push(pt);
                    }
                    None => next.push(pt.clone()),
                }
            }
            // A path ends on a keyframe (10 §4.12).
            while next.last().is_some_and(|p| p.get("frame").is_none_or(Value::is_null)) {
                next.pop();
            }
            if next != *points {
                changed = true;
                *points = next;
            }
            if points.is_empty() {
                changed = true;
                continue;
            }
        }
        out.insert(name.clone(), c);
    }
    Ok(changed.then_some(Value::Object(out)))
}

#[command(
    id = "element.timing.set",
    kind = action,
    label = "Change timing",
    scopes = [edit],
    undo = "Change Timing",
    needs_project,
    example(title = "Move the peg Bob-P to start at frame 5", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob-P\""], "start": 5}"#),
    example(title = "Stretch Bob-P to 24 frames, scaling its keys", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob-P\""], "duration": 24, "keyframes": "scale"}"#),
    example(title = "Loop the Fade effect twice", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Fade\""], "loops": 2}"#),
    gui = "Timeline: drag a bar, its ends or its loop handle; layer context: Change Start Frame / Duration / Loops (M5 C5.1)",
)]
pub(crate) fn element_timing_set(cx: &mut Cx, p: TimingParams) -> Result<TimingResult, CommandError> {
    if p.start.is_some() && p.start_delta.is_some() {
        return Err(err(
            codes::ARG_INVALID,
            "give `start` or `startDelta`, not both",
            json!({ "pointer": "/startDelta" }),
        ));
    }
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("changing element timing");
    let mut changes = Changes::default();
    let mut bars = Vec::new();
    let mut changed = Vec::new();
    for id in &ids {
        let e = element(&doc, id)?;
        let t = *e.element_type();
        let (Some(s), Some(d)) = (e.start().map(i64::from), e.duration().map(i64::from)) else {
            return Err(err(
                "cell.notApplicable",
                format!(
                    "{} element `{}` has no bar: its timing is its exposure (use the cell commands)",
                    t.as_str(),
                    e.name()
                ),
                json!({ "elementId": id.as_str(), "elementType": t.as_str() }),
            )
            .hint("move exposures with cell.range.move"));
        };
        let loops = i64::from(*e.loops()).max(1);
        if p.loops.is_some() && !loopable(t) {
            return Err(err(
                "element.notLoopable",
                format!("{} elements cannot loop", t.as_str()),
                json!({ "elementId": id.as_str(), "elementType": t.as_str() }),
            ));
        }
        // Checked arithmetic throughout (C31-R1-10): any overflow is the typed out-of-range refusal, before staging.
        let out_of_range = || {
            err(
                "cell.frameOutOfRange",
                format!("the requested bar lies outside frames 1-{}", x::MAX_FRAMES),
                json!({ "elementId": id.as_str(), "start": p.start, "startDelta": p.start_delta, "duration": p.duration, "loops": p.loops }),
            )
        };
        let end = s.checked_add(d).and_then(|v| v.checked_sub(1)).ok_or_else(out_of_range)?;
        let mut ns = match (p.start, p.start_delta) {
            (Some(v), _) => v,
            (None, Some(dv)) => s.checked_add(dv).ok_or_else(out_of_range)?,
            _ => s,
        };
        let mut nd = p.duration.unwrap_or(d);
        if p.anchor == Anchor::End {
            match (p.duration, p.start.is_some() || p.start_delta.is_some()) {
                (Some(v), _) => ns = end.checked_sub(v).and_then(|x| x.checked_add(1)).ok_or_else(out_of_range)?,
                (None, true) => nd = end.checked_sub(ns).and_then(|x| x.checked_add(1)).ok_or_else(out_of_range)?,
                _ => {}
            }
        }
        // Loops (05 §8.4): with `count` the bar is loops × L; with `fill` the bar stays and L follows.
        let nl = p.loops.unwrap_or(loops);
        let cycle = (d / loops).max(1);
        let count = *e.loop_type() == tt_model::entities::element::LoopType::Count;
        if p.loops.is_some() && p.duration.is_none() && count {
            nd = cycle.checked_mul(nl).ok_or_else(out_of_range)?;
        }
        let last = ns.checked_add(nd).and_then(|v| v.checked_sub(1)).ok_or_else(out_of_range)?;
        if ns < 1 || nd < 1 || nl < 1 || last > x::MAX_FRAMES as i64 || nl > x::MAX_FRAMES as i64 {
            return Err(err(
                "cell.frameOutOfRange",
                format!("the bar would span frames {ns}-{last}; bars lie within 1-{}", x::MAX_FRAMES),
                json!({ "elementId": id.as_str(), "start": ns, "duration": nd, "loops": nl }),
            ));
        }
        let (ns, nd, nl) = (
            as_u32(usize::try_from(ns).unwrap_or(1)),
            as_u32(usize::try_from(nd).unwrap_or(1)),
            as_u32(usize::try_from(nl).unwrap_or(1)),
        );
        let mut ops = Vec::new();
        if i64::from(ns) != s {
            ops.push(op("replace", "/start", Some(json!(ns))));
        }
        if i64::from(nd) != d {
            ops.push(op("replace", "/duration", Some(json!(nd))));
            let old_cycle = as_u32(usize::try_from(cycle).unwrap_or(1));
            let new_cycle = (nd / nl.max(1)).max(1);
            if let Some(ch) = doc.entry_raw_json(&EntryKey::Element(id.clone())).map_err(internal)?.get("channels")
                && let Some(next) = retimed_channels(ch, old_cycle, new_cycle, p.keyframes, &mut budget)?
            {
                ops.push(op("replace", "/channels", Some(next)));
            }
        }
        if i64::from(nl) != loops {
            ops.push(op("replace", "/loops", Some(json!(nl))));
        }
        if !ops.is_empty() {
            changed.push(id.clone());
        }
        changes.push(EntryKey::Element(id.clone()), ops);
        bars.push(Bar { id: id.as_str().to_owned(), start: ns, duration: nd, loops: nl });
    }
    changes.stage(cx)?;
    state_by_scene(cx, &doc, &changed, "timing");
    Ok(TimingResult { elements: bars })
}

/// `element.childExposure.apply` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ChildExposureParams {
    /// The peg.
    peg_id: String,
    /// Last frame to expose the children's drawings in.
    #[schemars(range(min = 1))]
    to_frame: i64,
}

/// `{changed}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChangedResult {
    /// Changed windows.
    changed: Vec<CellChange>,
}

#[command(
    id = "element.childExposure.apply",
    kind = action,
    label = "Extend children exposure",
    scopes = [edit],
    undo = "Extend Children Exposure",
    needs_project,
    example(title = "Extend the exposure of Bob-P's children up to frame 60", params = r#"{"pegId": "scene:\"Intro\"/element:\"Bob-P\"", "toFrame": 60}"#),
    gui = "Timeline cell context on a peg: Extend Children Exposure (M5 C5.1)",
)]
pub(crate) fn element_child_exposure_apply(cx: &mut Cx, p: ChildExposureParams) -> Result<ChangedResult, CommandError> {
    let peg = element_id(cx, &p.peg_id, "/pegId")?;
    // "Extend children exposure": the target frame of 13.5 (1 … 9999).
    let to = super::timing::target_frame(p.to_frame, x::MAX_COMMAND_FRAMES as usize, "/toFrame")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &peg)?;
    if *e.element_type() != ElementType::Peg {
        return Err(err("cell.notApplicable", format!("`{}` is not a peg", e.name()), json!({ "pointer": "/pegId" })));
    }
    let sc = super::timing::scene_of(&doc, &peg)?;
    let mut budget = Budget::new("extending children exposure");
    let mut changes = Changes::default();
    let mut out = Vec::new();
    // The cell-based elements attached to the peg (directly or through child pegs).
    for c in h::descendants(sc.tree(), &peg) {
        let Some(ce) = doc.element(&c) else { continue };
        if !ce.element_type().has_exposure() {
            continue;
        }
        let col = Column::read(&doc, &c, &mut budget)?;
        let last = x::last_exposed(&col.frames);
        if last == 0 || to <= last {
            continue;
        }
        budget.spend(to)?;
        if let Ok(after) = x::fill(&col.frames, last, to) {
            write_column(&col, &after, &mut changes, &mut out)?;
        }
    }
    changes.stage(cx)?;
    emit_cells(cx, &out)?;
    Ok(ChangedResult { changed: out })
}

/// `element.mediaLength.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct MediaLengthParams {
    /// The media element.
    element_id: String,
    /// New length of the element's (last) sequence (1 … 100 000); omitted: the media's natural length.
    #[serde(default)]
    #[schemars(range(min = 1, max = 100_000))]
    length: Option<i64>,
}

/// `{elementId, from, to}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaLengthResult {
    /// The element.
    element_id: String,
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
}

#[command(
    id = "element.mediaLength.set",
    kind = action,
    label = "Reset media",
    scopes = [edit],
    undo = "Reset Media",
    needs_project,
    example(title = "Set the Reference media to 30 frames", params = r#"{"elementId": "scene:\"Intro\"/element:\"Reference\"", "length": 30}"#),
    gui = "Exposure Sheet media cell context: Reset Media (M5 C5.2)",
)]
pub(crate) fn element_media_length_set(cx: &mut Cx, p: MediaLengthParams) -> Result<MediaLengthResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    if *e.element_type() != ElementType::Media {
        return Err(err(
            "cell.notApplicable",
            format!("`{}` is not a media element", e.name()),
            json!({ "pointer": "/elementId" }),
        ));
    }
    let seqs = e.sequences().clone().unwrap_or_default();
    let Some(last_i) = seqs.len().checked_sub(1) else {
        return Err(err(
            "drawing.fileMissing",
            "the media element links no media",
            json!({ "elementId": id.as_str() }),
        ));
    };
    let Some(length) = p.length else {
        // The natural length is the linked source's (derived `sourceLength`), read by the media decoder of M8 C8.3.
        return Err(err(
            codes::COMMAND_DISABLED,
            "the media's natural length is unknown until media decoding exists (M8 C8.3); pass `length`",
            json!({ "elementId": id.as_str(), "reason": "sourceLengthUnknown", "owner": "M8 C8.3", "specIssue": "SI-088" }),
        ));
    };
    let s = &seqs[last_i];
    let from = *s.start_frame();
    let to = i64::from(from).checked_add(length).and_then(|v| v.checked_sub(1)).unwrap_or(i64::MAX);
    if to > x::MAX_FRAMES as i64 {
        return Err(err(
            "doc.limitExceeded",
            "the media would pass the 100 000-frame scene limit",
            json!({ "pointer": "/length" }),
        ));
    }
    let to = as_u32(usize::try_from(to).unwrap_or(1));
    if *s.stop_frame() != to {
        let mut changes = Changes::default();
        changes.push(
            EntryKey::Element(id.clone()),
            vec![op("replace", &format!("/sequences/{last_i}/stopFrame"), Some(json!(to)))],
        );
        changes.stage(cx)?;
        emit_cells(cx, &[CellChange { element_id: id.as_str().to_owned(), from, to }])?;
    }
    Ok(MediaLengthResult { element_id: id.as_str().to_owned(), from: *seqs[0].start_frame(), to })
}

// ----- toggles ------------------------------------------------------------------------------------------------------------

/// Which elements a toggle applies to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Apply {
    /// The given elements.
    #[default]
    Given,
    /// Every element of the scene.
    All,
    /// Every element except the given ones.
    Others,
}

/// Visibility scope (04 §10.1–§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Scope {
    /// Scene views, playback, render and export.
    Scene,
    /// Exposure Sheet and Drawing View (cell-based elements).
    Xsheet,
}

/// `element.visibility.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct VisibilityParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Scene or Exposure Sheet visibility.
    scope: Scope,
    /// Shown or hidden.
    visible: bool,
    /// The given elements, all, or all others.
    #[serde(default)]
    apply: Apply,
    /// Only elements of these types (e.g. `["annotation"]` for show/hide all annotation elements).
    #[serde(default)]
    types: Option<Vec<ElementType>>,
}

/// `{changedIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedIds {
    /// Elements whose flag changed.
    changed_ids: Vec<String>,
}

/// The elements a toggle applies to (one scene: of the given elements, else the current scene).
fn toggle_set(
    cx: &Cx,
    given: Option<&[String]>,
    apply: Apply,
    types: Option<&[ElementType]>,
) -> Result<(Id, Vec<Id>), CommandError> {
    let doc = cx.document()?;
    let (scene, picked) = match (apply, given) {
        (Apply::All, None) => (scene_id(cx, None)?, Vec::new()),
        _ => {
            let ids = targets(cx, given)?;
            (one_scene(doc, &ids)?, ids)
        }
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let all = doc.tree_order(sc);
    let set: Vec<Id> = match apply {
        Apply::Given => picked,
        Apply::All => all,
        Apply::Others => all.into_iter().filter(|e| !picked.contains(e)).collect(),
    };
    Ok((
        scene,
        set.into_iter()
            .filter(|e| types.is_none_or(|t| doc.element(e).is_some_and(|x| t.contains(x.element_type()))))
            .collect(),
    ))
}

#[command(
    id = "element.visibility.set",
    kind = toggle,
    label = "Show/hide elements",
    scopes = [edit],
    undo = "Show/Hide Elements",
    needs_project,
    example(title = "Hide Bob in the scene", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "scope": "scene", "visible": false}"#),
    example(title = "Hide all annotation elements", params = r#"{"scope": "scene", "visible": false, "apply": "all", "types": ["annotation"], "elementIds": ["scene:\"Intro\"/element:\"Notes\""]}"#),
    gui = "Timeline layer list visibility control and context; Exposure Sheet column header: Hide / Show All / Hide All Others (M5)",
)]
pub(crate) fn element_visibility_set(cx: &mut Cx, p: VisibilityParams) -> Result<ChangedIds, CommandError> {
    let (scene, set) = toggle_set(cx, p.element_ids.as_deref(), p.apply, p.types.as_deref())?;
    let doc = cx.document()?.clone();
    let mut budget = Budget::new("changing visibility");
    budget.spend(set.len())?;
    let mut changes = Changes::default();
    let mut changed = Vec::new();
    for id in &set {
        let e = element(&doc, id)?;
        let (path, current) = match p.scope {
            Scope::Scene => ("/visibleInScene", Some(*e.visible_in_scene())),
            Scope::Xsheet => ("/visibleInXsheet", *e.visible_in_xsheet()),
        };
        if let Some(c) = current
            && c != p.visible
        {
            changes.push(EntryKey::Element(id.clone()), vec![op("replace", path, Some(json!(p.visible)))]);
            changed.push(id.as_str().to_owned());
        } else if current.is_none() && p.apply == Apply::Given {
            return Err(err(
                "element.notCellBased",
                format!("{} element `{}` has no Exposure Sheet column", e.element_type().as_str(), e.name()),
                json!({ "elementId": id.as_str() }),
            ));
        }
    }
    changes.stage(cx)?;
    // Hiding the current element in the Exposure Sheet moves the current element to the next visible column (04 §10.2).
    if p.scope == Scope::Xsheet
        && !p.visible
        && let Some(cur) = cx.app().element.clone()
        && changed.iter().any(|c| c == cur.as_str())
        && let Some(sc) = cx.document()?.scene(&scene).cloned()
    {
        let doc2 = cx.document()?.clone();
        let cols = sc.column_order();
        let i = cols.iter().position(|c| *c == cur).unwrap_or(0);
        let next = cols
            .iter()
            .skip(i + 1)
            .chain(cols.iter().take(i).rev())
            .find(|c| doc2.element(c).is_some_and(|e| e.visible_in_xsheet().unwrap_or(true)))
            .cloned();
        if let Some(n) = next {
            cx.app_mut().element = Some(n);
            cx.app_mut().drawing = None;
        }
    }
    emit_state(cx, &scene, &changed, "visibility");
    Ok(ChangedIds { changed_ids: changed })
}

/// `element.lock.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LockParams {
    /// Elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Locked or unlocked.
    locked: bool,
    /// The given elements, all, or all others.
    #[serde(default)]
    apply: Apply,
}

#[command(
    id = "element.lock.set",
    kind = toggle,
    label = "Lock elements",
    scopes = [edit],
    undo = "Lock Elements",
    needs_project,
    example(title = "Lock Bob in the scene-planning views", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "locked": true}"#),
    example(title = "Unlock all", params = r#"{"locked": false, "apply": "all"}"#),
    gui = "Element menu, Timeline layer context, camera view context: Lock / Unlock / Lock All Others / Lock All / Unlock All (M5)",
)]
pub(crate) fn element_lock_set(cx: &mut Cx, p: LockParams) -> Result<ChangedIds, CommandError> {
    let (scene, set) = toggle_set(cx, p.element_ids.as_deref(), p.apply, None)?;
    let doc = cx.document()?.clone();
    let mut changes = Changes::default();
    let mut changed = Vec::new();
    for id in &set {
        if *element(&doc, id)?.locked() != p.locked {
            changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/locked", Some(json!(p.locked)))]);
            changed.push(id.as_str().to_owned());
        }
    }
    changes.stage(cx)?;
    cx.set_label(if p.locked { "Lock Elements" } else { "Unlock Elements" });
    emit_state(cx, &scene, &changed, "lock");
    Ok(ChangedIds { changed_ids: changed })
}

/// `element.annotation.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AnnotationParams {
    /// A drawing or annotation element.
    element_id: String,
    /// Annotation element (`true`) or drawing element (`false`).
    annotation: bool,
}

/// `{elementId, type}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnotationResult {
    /// The element.
    element_id: String,
    /// Its type now.
    #[serde(rename = "type")]
    ty: String,
}

#[command(
    id = "element.annotation.set",
    kind = toggle,
    label = "Set as annotation element",
    scopes = [edit],
    undo = "Set as Annotation Element",
    needs_project,
    example(title = "Turn Bob into an annotation element", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "annotation": true}"#),
    gui = "Timeline layer context: Set as Annotation / Set as Drawing Element (M5 C5.1)",
)]
pub(crate) fn element_annotation_set(cx: &mut Cx, p: AnnotationParams) -> Result<AnnotationResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let doc = cx.document()?.clone();
    let e = element(&doc, &id)?;
    let t = *e.element_type();
    if !matches!(t, ElementType::Drawing | ElementType::Annotation) {
        return Err(err(
            "element.notDrawing",
            format!("{} element `{}` is not a drawing or annotation element", t.as_str(), e.name()),
            json!({ "elementId": id.as_str(), "elementType": t.as_str() }),
        ));
    }
    let want = if p.annotation { ElementType::Annotation } else { ElementType::Drawing };
    if want != t {
        let mut changes = Changes::default();
        changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/elementType", Some(json!(want.as_str())))]);
        changes.stage(cx)?;
        cx.set_label(if p.annotation { "Set as Annotation Element" } else { "Set as Drawing Element" });
        state_by_scene(cx, &doc, std::slice::from_ref(&id), "type");
    }
    Ok(AnnotationResult { element_id: id.as_str().to_owned(), ty: want.as_str().to_owned() })
}

/// `element.collapsed.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CollapsedParams {
    /// Parent elements (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// Collapsed or expanded.
    collapsed: bool,
}

#[command(
    id = "element.collapsed.set",
    kind = toggle,
    label = "Collapse / expand",
    scopes = [ui],
    needs_project,
    example(title = "Collapse the peg Bob-P", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob-P\""], "collapsed": true}"#),
    gui = "Timeline: disclosure control left of a parent row; layer context Collapse/Expand (M5 C5.1)",
)]
pub(crate) fn element_collapsed_set(cx: &mut Cx, p: CollapsedParams) -> Result<IdsResult, CommandError> {
    let ids = targets(cx, p.element_ids.as_deref())?;
    let doc = cx.document()?.clone();
    let mut changes = Changes::default();
    let mut changed = Vec::new();
    for id in &ids {
        if *element(&doc, id)?.collapsed() != p.collapsed {
            changes.push(EntryKey::Element(id.clone()), vec![op("replace", "/collapsed", Some(json!(p.collapsed)))]);
            changed.push(id.clone());
        }
    }
    // Saved with the scene but not an undo step (04 §17.4: undo "no"): staged as a system change.
    if !changes.is_empty() {
        changes.stage(cx)?;
        cx.seal_system("Collapse / Expand")?;
    }
    state_by_scene(cx, &doc, &changed, "collapsed");
    Ok(IdsResult { element_ids: ids.iter().map(|i| i.as_str().to_owned()).collect() })
}

/// `element.solo.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SoloParams {
    /// Solo mode on or off.
    enabled: bool,
    /// The elements shown while solo is on (default: the selected rows, else the current element).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
}

/// `{enabled, elementIds}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SoloResult {
    /// Solo mode.
    enabled: bool,
    /// The soloed elements.
    element_ids: Vec<String>,
}

#[command(
    id = "element.solo.set",
    kind = toggle,
    label = "Solo mode",
    scopes = [ui],
    needs_project,
    example(title = "Solo Bob", params = r#"{"enabled": true, "elementIds": ["scene:\"Intro\"/element:\"Bob\""]}"#),
    example(title = "Turn solo mode off", params = r#"{"enabled": false}"#),
    gui = "Timeline toolbar: Solo Mode toggle (M5 C5.1)",
)]
pub(crate) fn element_solo_set(cx: &mut Cx, p: SoloParams) -> Result<SoloResult, CommandError> {
    let ids = if p.enabled { targets(cx, p.element_ids.as_deref())? } else { Vec::new() };
    let strs: Vec<String> = ids.iter().map(|i| i.as_str().to_owned()).collect();
    // A view state: never stored in the document, never undoable (04 §10.3).
    cx.session_mut().timeline.solo = p.enabled.then(|| ids.clone());
    Ok(SoloResult { enabled: p.enabled, element_ids: strs })
}

// ----- event --------------------------------------------------------------------------------------------------------------

/// `element.state.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateChanged {
    /// The scene.
    scene_id: String,
    /// The elements.
    element_ids: Vec<String>,
    /// `created`, `deleted`, `renamed`, `reordered`, `reparented`, `timing`, `visibility`, `lock`, `type`,
    /// `collapsed` or `properties`.
    change: String,
}

#[command(
    id = "element.state.changed",
    kind = event,
    label = "Elements changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn element_state_changed(_cx: &mut Cx, _p: NoParams) -> Result<StateChanged, CommandError> {
    Err(super::common::event_not_invocable("element.state.changed"))
}
