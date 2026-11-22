//! The exposure sheet as JSON (04 §17.2; M3 C3.1): `xsheet.exposures.get` (run-length columns over a frame window,
//! Exposure Sheet order, hidden columns included) and `xsheet.exposures.set` (validated, atomic, one undo step,
//! `expectRev`, `dryRun`), plus `xsheet.lightTable.list` and the `xsheet.lightTable.changed` event.
//!
//! The sheet revision (`rev`) is the highest revision of the scene and its cell-based elements: it changes whenever
//! any cell of the scene changes (and on other edits of those objects).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_anim::exposure as x;
use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::ElementType;
use tt_model::entities::scene::Scene;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timing::{
    Budget, CellChange, Changes, Column, add_drawing, as_u32, element, element_id, err, internal, scene_id, to_json,
    write_column,
};
use crate::cx::Cx;

/// The sheet revision of a scene: the highest `rev` of the scene and its cell-based elements.
pub(crate) fn sheet_rev(doc: &Document, sc: &Scene) -> u64 {
    sc.column_order().iter().filter_map(|e| doc.element(e)).map(|e| *e.rev()).fold(*sc.rev(), u64::max)
}

// ----- xsheet.exposures.get -------------------------------------------------------------------------------------------

/// `xsheet.exposures.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct GetParams {
    /// Scene (default: the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// Only these columns (default: every cell-based element, Exposure Sheet order).
    #[serde(default)]
    element_ids: Option<Vec<String>>,
    /// First frame of the window.
    #[serde(default = "first")]
    from: i64,
    /// Last frame of the window (default: the scene length).
    #[serde(default)]
    to: Option<i64>,
    /// Optional parts: `notes`, `mouths`, `thumbnails`.
    #[serde(default)]
    include: Option<Vec<Include>>,
}

fn first() -> i64 {
    1
}

/// Optional parts of the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Include {
    /// Cell notes.
    Notes,
    /// Lip-sync mouths of sound columns.
    Mouths,
    /// 64-pixel PNG thumbnails of the exposed drawings.
    Thumbnails,
}

/// One exposed run.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Run {
    /// Drawing name (media: the media item's name).
    drawing: String,
    /// Drawing ID (media: the sequence ID).
    drawing_id: String,
    /// First frame inside the window.
    from: u32,
    /// Last frame inside the window.
    to: u32,
    /// The run continues outside the window: `before`, `after` or `both`.
    #[serde(skip_serializing_if = "Option::is_none")]
    continues: Option<String>,
}

/// One cell note.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Note {
    /// Frame.
    frame: u32,
    /// Text.
    text: String,
}

/// One sound clip.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Clip {
    /// The placement.
    clip_id: String,
    /// The sound asset's name.
    file: String,
    /// First frame inside the window.
    from: u32,
    /// Last frame inside the window.
    to: u32,
    /// Continuation outside the window.
    #[serde(skip_serializing_if = "Option::is_none")]
    continues: Option<String>,
}

/// One mouth run.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Mouth {
    /// `A`–`G` or `X`.
    mouth: String,
    /// First frame.
    from: u32,
    /// Last frame.
    to: u32,
}

/// One column.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColumnOut {
    /// The element.
    element_id: String,
    /// Its name.
    name: String,
    /// Its type.
    element_type: String,
    /// Shown in the Exposure Sheet.
    visible: bool,
    /// Exposed runs (drawing, image, annotation and media columns).
    #[serde(skip_serializing_if = "Option::is_none")]
    exposures: Option<Vec<Run>>,
    /// Cell notes in the window (with `include: ["notes"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<Vec<Note>>,
    /// Sound clips (sound columns).
    #[serde(skip_serializing_if = "Option::is_none")]
    clips: Option<Vec<Clip>>,
    /// Mouth runs (sound columns, with `include: ["mouths"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    mouths: Option<Vec<Mouth>>,
    /// Thumbnails by drawing ID (with `include: ["thumbnails"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnails: Option<Map<String, Value>>,
}

/// The exposure-sheet object (04 §17.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sheet {
    /// The scene.
    scene_id: String,
    /// Sheet revision.
    rev: u64,
    /// Scene length.
    scene_length: u32,
    /// Window start.
    from: u32,
    /// Window end.
    to: u32,
    /// Columns in Exposure Sheet order.
    columns: Vec<ColumnOut>,
}

fn continues(before: bool, after: bool) -> Option<String> {
    match (before, after) {
        (true, true) => Some("both".to_owned()),
        (true, false) => Some("before".to_owned()),
        (false, true) => Some("after".to_owned()),
        (false, false) => None,
    }
}

#[command(
    id = "xsheet.exposures.get",
    kind = query,
    label = "Get exposure sheet",
    scopes = [read],
    needs_project,
    example(title = "The Intro scene's exposure sheet, frames 1-24", params = r#"{"sceneId": "scene:\"Intro\"", "from": 1, "to": 24}"#),
    example(title = "Bob's column with notes", params = r#"{"elementIds": ["scene:\"Intro\"/element:\"Bob\""], "include": ["notes"]}"#),
    agent_only = "the Exposure Sheet shows the cells directly",
)]
pub(crate) fn xsheet_exposures_get(cx: &mut Cx, p: GetParams) -> Result<Sheet, CommandError> {
    let picked = match &p.element_ids {
        Some(list) => {
            let mut out = Vec::new();
            for (i, r) in list.iter().enumerate() {
                out.push(element_id(cx, r, &format!("/elementIds/{i}"))?);
            }
            Some(out)
        }
        None => None,
    };
    let doc = cx.document()?.clone();
    let scene = match (&p.scene_id, picked.as_ref().and_then(|v| v.first())) {
        (Some(s), _) => scene_id(cx, Some(s))?,
        (None, Some(e)) => doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?,
        (None, None) => scene_id(cx, None)?,
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?.clone();
    let length = as_u32(usize::try_from(doc.scene_length(&sc)).unwrap_or(1));
    let from = super::timing::frame(p.from, "/from")?;
    let to = match p.to {
        Some(t) => super::timing::frame(t, "/to")?,
        None => (length as usize).max(from),
    };
    if to < from {
        return Err(err(codes::ARG_INVALID, "`to` is before `from`", json!({ "pointer": "/to" })));
    }
    let include = p.include.clone().unwrap_or_default();
    let mut budget = Budget::new("reading the exposure sheet");
    let mut columns = Vec::new();
    for e in sc.column_order() {
        if picked.as_ref().is_some_and(|v| !v.contains(e)) {
            continue;
        }
        let el = element(&doc, e)?;
        let t = *el.element_type();
        let mut c = ColumnOut {
            element_id: e.as_str().to_owned(),
            name: el.name().clone(),
            element_type: t.as_str().to_owned(),
            visible: el.visible_in_xsheet().unwrap_or(true),
            exposures: None,
            notes: None,
            clips: None,
            mouths: None,
            thumbnails: None,
        };
        match t {
            ElementType::Sound => {
                let mut clips = Vec::new();
                let mut mouths = Vec::new();
                for pl in el.placements().iter().flatten() {
                    budget.spend(1)?;
                    let (a, b) = (*pl.start_frame() as usize, *pl.stop_frame() as usize);
                    if b < from || a > to {
                        continue;
                    }
                    let file = doc
                        .object_raw_json(pl.asset_id().as_str())
                        .ok()
                        .and_then(|(_, v)| v.get("name").and_then(Value::as_str).map(str::to_owned))
                        .unwrap_or_default();
                    clips.push(Clip {
                        clip_id: pl.id().as_str().to_owned(),
                        file,
                        from: as_u32(a.max(from)),
                        to: as_u32(b.min(to)),
                        continues: continues(a < from, b > to),
                    });
                    if include.contains(&Include::Mouths)
                        && let Some(track) = pl.mouth_track()
                    {
                        let letters: Vec<Option<char>> = track.mouths.chars().map(Some).collect();
                        budget.spend(letters.len())?;
                        for r in x::runs(&letters) {
                            let (ra, rb) = (a + r.from - 1, a + r.to() - 1);
                            if rb < from || ra > to {
                                continue;
                            }
                            if let Some(m) = r.value {
                                mouths.push(Mouth {
                                    mouth: m.to_string(),
                                    from: as_u32(ra.max(from)),
                                    to: as_u32(rb.min(to)),
                                });
                            }
                        }
                    }
                }
                c.clips = Some(clips);
                if include.contains(&Include::Mouths) {
                    c.mouths = Some(mouths);
                }
            }
            ElementType::Media => {
                let mut runs = Vec::new();
                for s in el.sequences().iter().flatten() {
                    budget.spend(1)?;
                    let (a, b) = (*s.start_frame() as usize, *s.stop_frame() as usize);
                    if b < from || a > to {
                        continue;
                    }
                    let name = doc
                        .object_raw_json(s.asset_id().as_str())
                        .ok()
                        .and_then(|(_, v)| v.get("name").and_then(Value::as_str).map(str::to_owned))
                        .unwrap_or_default();
                    runs.push(Run {
                        drawing: name,
                        drawing_id: s.id().as_str().to_owned(),
                        from: as_u32(a.max(from)),
                        to: as_u32(b.min(to)),
                        continues: continues(a < from, b > to),
                    });
                }
                c.exposures = Some(runs);
            }
            _ => {
                let col = Column::read(&doc, e, &mut budget)?;
                budget.spend(to - from + 1)?;
                let mut runs = Vec::new();
                let mut thumbs = Map::new();
                for w in x::window(&col.frames, from, to) {
                    let Some(d) = col.drawing(w.value) else { continue };
                    let name = doc.drawing(d).map(|x| x.name().clone()).unwrap_or_default();
                    if include.contains(&Include::Thumbnails) && !thumbs.contains_key(d.as_str()) {
                        budget.spend(64)?;
                        let params: super::draw_render::RenderParams =
                            serde_json::from_value(json!({ "drawingId": d.as_str(), "width": 64, "height": 64 }))
                                .map_err(internal)?;
                        let r = to_json(&super::draw_render::draw_drawing_render(cx, params)?)?;
                        thumbs.insert(d.as_str().to_owned(), r.get("png").cloned().unwrap_or(Value::Null));
                    }
                    runs.push(Run {
                        drawing: name,
                        drawing_id: d.as_str().to_owned(),
                        from: as_u32(w.from),
                        to: as_u32(w.to),
                        continues: w.continues.map(|c| c.as_str().to_owned()),
                    });
                }
                c.exposures = Some(runs);
                if include.contains(&Include::Thumbnails) {
                    c.thumbnails = Some(thumbs);
                }
                if include.contains(&Include::Notes) {
                    c.notes = Some(
                        el.cell_notes()
                            .iter()
                            .filter(|(f, _)| (from..=to).contains(&(**f as usize)))
                            .map(|(f, t)| Note { frame: *f, text: t.clone() })
                            .collect(),
                    );
                }
            }
        }
        columns.push(c);
    }
    Ok(Sheet {
        scene_id: scene.as_str().to_owned(),
        rev: sheet_rev(&doc, &sc),
        scene_length: length,
        from: as_u32(from),
        to: as_u32(to),
        columns,
    })
}

// ----- xsheet.exposures.set --------------------------------------------------------------------------------------------

/// One run to write.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RunIn {
    /// Drawing name in the column's bank.
    #[serde(default)]
    drawing: Option<String>,
    /// Drawing ID (or path).
    #[serde(default)]
    drawing_id: Option<String>,
    /// First frame.
    from: i64,
    /// Last frame.
    to: i64,
}

/// One column to write.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ColumnIn {
    /// The element.
    element_id: String,
    /// Window start (default 1).
    #[serde(default)]
    from: Option<i64>,
    /// Window end (default: the later of the last run's end and the element's last exposed frame).
    #[serde(default)]
    to: Option<i64>,
    /// Runs, sorted and non-overlapping, inside the window; frames not covered become blank.
    exposures: Vec<RunIn>,
}

/// `xsheet.exposures.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SetParams {
    /// Scene (default: the current scene); every column must belong to it.
    #[serde(default)]
    scene_id: Option<String>,
    /// Columns to write.
    #[schemars(length(min = 1))]
    columns: Vec<ColumnIn>,
    /// Create empty drawings for unknown names (drawing and annotation columns).
    #[serde(default)]
    create_missing_drawings: bool,
    /// The sheet revision the edit is based on (`xsheet.revisionMismatch` otherwise).
    #[serde(default)]
    expect_rev: Option<u64>,
    /// Validate and return the diff without applying it.
    #[serde(default)]
    dry_run: bool,
}

/// A created drawing.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Created {
    /// The column.
    element_id: String,
    /// The drawing.
    drawing_id: String,
    /// Its name.
    name: String,
}

/// `{rev, sceneLength, changed, createdDrawings}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetResult {
    /// The sheet revision after the change.
    rev: u64,
    /// The scene length after the change.
    scene_length: u32,
    /// Changed windows per column.
    changed: Vec<CellChange>,
    /// Drawings created by `createMissingDrawings`.
    created_drawings: Vec<Created>,
}

fn run_error(code: &str, message: String, column: usize, run: Option<usize>) -> CommandError {
    let pointer = match run {
        Some(r) => format!("/columns/{column}/exposures/{r}"),
        None => format!("/columns/{column}"),
    };
    err(code, message, json!({ "pointer": pointer, "column": column, "run": run }))
}

#[command(
    id = "xsheet.exposures.set",
    precheck = crate::prechecks::xsheet_exposures_set,
    kind = action,
    label = "Set exposure sheet",
    scopes = [edit],
    undo = "Set Exposures",
    needs_project,
    example(title = "Rewrite Bob's frames 1-24", params = r#"{"sceneId": "scene:\"Intro\"", "columns": [{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 24, "exposures": [{"drawing": "1", "from": 1, "to": 6}, {"drawing": "10", "from": 7, "to": 12}, {"drawing": "4", "from": 13, "to": 24}]}], "createMissingDrawings": true}"#),
    example(title = "Preview a change without applying it", params = r#"{"columns": [{"elementId": "scene:\"Intro\"/element:\"Bob\"", "from": 1, "to": 4, "exposures": [{"drawing": "1", "from": 1, "to": 4}]}], "dryRun": true}"#),
    gui = "Exposure Sheet: type labels into cells (M5 C5.2); agents write whole windows atomically",
)]
pub(crate) fn xsheet_exposures_set(cx: &mut Cx, p: SetParams) -> Result<SetResult, CommandError> {
    let doc = cx.document()?.clone();
    // Resolve every column first.
    let mut ids = Vec::new();
    for (i, c) in p.columns.iter().enumerate() {
        ids.push(element_id(cx, &c.element_id, &format!("/columns/{i}/elementId"))?);
    }
    let scene = match (&p.scene_id, ids.first()) {
        (Some(s), _) => scene_id(cx, Some(s))?,
        (None, Some(e)) => doc.scene_of(e).cloned().ok_or_else(|| internal("scene"))?,
        (None, None) => scene_id(cx, None)?,
    };
    let sc = doc.scene(&scene).ok_or_else(|| internal("scene"))?.clone();
    if let Some(want) = p.expect_rev {
        let have = sheet_rev(&doc, &sc);
        if want != have {
            return Err(err(
                "xsheet.revisionMismatch",
                format!("the exposure sheet is at revision {have}, not {want}"),
                json!({ "pointer": "/expectRev", "expected": want, "current": have }),
            )
            .hint("re-read the sheet (xsheet.exposures.get) and retry"));
        }
    }
    let mut budget = Budget::new("writing the exposure sheet");
    let mut changes = Changes::default();
    let mut changed = Vec::new();
    let mut created = Vec::new();
    let mut cols: BTreeMap<Id, (Column, Vec<Option<u32>>)> = BTreeMap::new();
    let mut new_names: BTreeMap<(Id, String), Id> = BTreeMap::new();
    for (ci, (c, id)) in p.columns.iter().zip(ids.iter()).enumerate() {
        if doc.scene_of(id) != Some(&scene) {
            return Err(run_error(codes::ARG_INVALID, "the column is not in the scene".to_owned(), ci, None));
        }
        let el = element(&doc, id)?;
        let t = *el.element_type();
        if !t.is_cell_based() || matches!(t, ElementType::Sound | ElementType::Media) {
            let message = match t {
                ElementType::Sound => "sound columns are written with the sound commands (07)".to_owned(),
                ElementType::Media => "media columns are written with the media commands (M8 C8.3, SI-088)".to_owned(),
                _ => format!("{} elements have no Exposure Sheet column", t.as_str()),
            };
            return Err(run_error("xsheet.columnReadOnly", message, ci, None));
        }
        let bank = el.drawing_bank_id().clone().ok_or_else(|| internal("bank"))?;
        if !cols.contains_key(id) {
            let col = Column::read(&doc, id, &mut budget)?;
            let f = col.frames.clone();
            cols.insert(id.clone(), (col, f));
        }
        let (col, frames) = cols.get_mut(id).ok_or_else(|| internal("column"))?;
        // Validate the runs.
        let mut prev_to = 0i64;
        let mut runs = Vec::with_capacity(c.exposures.len());
        for (ri, r) in c.exposures.iter().enumerate() {
            if r.from < 1 || r.to > x::MAX_FRAMES as i64 || r.from > r.to {
                let code = if r.from > r.to { "xsheet.runInvalid" } else { "xsheet.runOutOfRange" };
                return Err(run_error(code, format!("run {}-{} is invalid", r.from, r.to), ci, Some(ri)));
            }
            if usize::from(r.drawing.is_some()) + usize::from(r.drawing_id.is_some()) != 1 {
                return Err(run_error(
                    "xsheet.runInvalid",
                    "a run names its drawing by `drawing` or `drawingId`".to_owned(),
                    ci,
                    Some(ri),
                ));
            }
            if ri > 0 && r.from <= prev_to {
                let code = if r.from < c.exposures[ri - 1].from { "xsheet.runInvalid" } else { "xsheet.runOverlap" };
                return Err(run_error(
                    code,
                    format!("run {} starts at {} but the previous run ends at {prev_to}", ri, r.from),
                    ci,
                    Some(ri),
                ));
            }
            prev_to = r.to;
            runs.push(r);
        }
        let last_exposed = x::last_exposed(frames) as i64;
        let from = c.from.unwrap_or(1);
        let to = c.to.unwrap_or_else(|| prev_to.max(last_exposed).max(from));
        if from < 1 || to > x::MAX_FRAMES as i64 || to < from {
            return Err(run_error(
                "xsheet.runOutOfRange",
                format!("the window {from}-{to} is outside 1-{}", x::MAX_FRAMES),
                ci,
                None,
            ));
        }
        for (ri, r) in runs.iter().enumerate() {
            if r.from < from || r.to > to {
                return Err(run_error(
                    "xsheet.runOutOfRange",
                    format!("run {}-{} lies outside the window {from}-{to}", r.from, r.to),
                    ci,
                    Some(ri),
                ));
            }
        }
        let (from, to) = (usize::try_from(from).unwrap_or(1), usize::try_from(to).unwrap_or(1));
        budget.spend(to - from + 1)?;
        let mut window: Vec<Option<u32>> = vec![None; to - from + 1];
        for (ri, r) in runs.iter().enumerate() {
            let d = match (&r.drawing, &r.drawing_id) {
                (_, Some(dr)) => {
                    let d = super::timing::drawing_id(cx, dr, &format!("/columns/{ci}/exposures/{ri}/drawingId"))?;
                    if doc.bank_of(&d) != Some(&bank) {
                        return Err(run_error(
                            "cell.drawingNotFound",
                            format!("drawing `{dr}` is not in the column's bank"),
                            ci,
                            Some(ri),
                        ));
                    }
                    d
                }
                // Names are normalised (NFC, trimmed; 10 §1.3) before every lookup, staged ones included, so
                // `" shared "` and `"shared"` are one drawing (C31-R1-08).
                (Some(raw), None) => {
                    let normalised =
                        super::timing::drawing_name(raw, &format!("/columns/{ci}/exposures/{ri}/drawing"))?;
                    let name = &normalised;
                    match super::timing::drawing_named(&doc, &bank, name)
                        .or_else(|| new_names.get(&(bank.clone(), name.clone())).cloned())
                    {
                        Some(d) => d,
                        None if p.create_missing_drawings
                            && matches!(t, ElementType::Drawing | ElementType::Annotation) =>
                        {
                            let n =
                                super::timing::drawing_name(name, &format!("/columns/{ci}/exposures/{ri}/drawing"))?;
                            let d = add_drawing(cx, &mut changes, &bank, &n, None)?;
                            new_names.insert((bank.clone(), name.clone()), d.clone());
                            created.push(Created {
                                element_id: id.as_str().to_owned(),
                                drawing_id: d.as_str().to_owned(),
                                name: n,
                            });
                            d
                        }
                        None => {
                            return Err(run_error(
                                "cell.drawingNotFound",
                                format!("the column's bank has no drawing `{name}`"),
                                ci,
                                Some(ri),
                            ));
                        }
                    }
                }
                (None, None) => return Err(internal("validated run")),
            };
            let i = col.intern(&d);
            let a = usize::try_from(r.from).unwrap_or(1) - from;
            let b = usize::try_from(r.to).unwrap_or(1) - from;
            for cell in &mut window[a..=b] {
                *cell = Some(i);
            }
        }
        *frames = x::overwrite(frames, from, &window);
    }
    for (col, after) in cols.values() {
        budget.spend(after.len())?;
        write_column(col, after, &mut changes, &mut changed)?;
    }
    changes.stage(cx)?;
    let after_doc = cx.document()?.clone();
    let after_sc = after_doc.scene(&scene).ok_or_else(|| internal("scene"))?;
    let scene_length = as_u32(usize::try_from(after_doc.scene_length(after_sc)).unwrap_or(1));
    let rev = if changed.is_empty() && created.is_empty() { sheet_rev(&doc, &sc) } else { cx.predicted_rev()? };
    if !p.dry_run {
        super::timing::emit_cells(cx, &changed)?;
        // Drawing-bank creation events, one per bank with its drawings (C31-R1-03).
        let mut by_bank: BTreeMap<Id, Vec<String>> = BTreeMap::new();
        for c in &created {
            if let Some(bank) =
                Id::parse(&c.element_id).ok().and_then(|e| doc.element(&e).and_then(|x| x.drawing_bank_id().clone()))
            {
                by_bank.entry(bank).or_default().push(c.drawing_id.clone());
            }
        }
        for (bank, ids) in &by_bank {
            super::timing::emit_bank(cx, &doc, bank, ids, "created");
        }
    }
    Ok(SetResult { rev, scene_length, changed, created_drawings: created })
}

// ----- static light table ------------------------------------------------------------------------------------------------

/// One light-table item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightItemOut {
    /// Item ID.
    item_id: String,
    /// Source element.
    element_id: String,
    /// The drawing.
    drawing_id: String,
    /// `<drawing> in <element>`, following renames (04 §15.3).
    label: String,
    /// Shown.
    visible: bool,
    /// `over` or `under`.
    placement: String,
}

/// `{items}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LightTableResult {
    /// Items in stacking order.
    items: Vec<LightItemOut>,
}

#[command(
    id = "xsheet.lightTable.list",
    kind = query,
    label = "List light table",
    scopes = [read],
    needs_project,
    example(title = "The current scene's static light table", params = "{}"),
    agent_only = "the static light table panel shows its items directly",
)]
pub(crate) fn xsheet_light_table_list(cx: &mut Cx, _p: NoParams) -> Result<LightTableResult, CommandError> {
    let doc = cx.document()?;
    let Some(scene) = cx.app().scene.clone() else { return Ok(LightTableResult { items: Vec::new() }) };
    let items = cx
        .session()
        .timeline
        .light_tables
        .get(scene.as_str())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        // Items track their source: deleted drawings and elements drop out (04 §15.3).
        .filter_map(|it| {
            let d = doc.drawing(&Id::parse(&it.drawing_id).ok()?)?;
            let e = doc.element(&Id::parse(&it.element_id).ok()?)?;
            Some(LightItemOut {
                label: format!("{} in {}", d.name(), e.name()),
                item_id: it.item_id,
                element_id: it.element_id,
                drawing_id: it.drawing_id,
                visible: it.visible,
                placement: it.placement,
            })
        })
        .collect();
    Ok(LightTableResult { items })
}

/// `xsheet.lightTable.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightTableChanged {
    /// The items.
    item_ids: Vec<String>,
}

#[command(
    id = "xsheet.lightTable.changed",
    kind = event,
    label = "Light table changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn xsheet_light_table_changed(_cx: &mut Cx, _p: NoParams) -> Result<LightTableChanged, CommandError> {
    Err(super::common::event_not_invocable("xsheet.lightTable.changed"))
}
