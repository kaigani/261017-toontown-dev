//! `doc.object.refs`, `doc.schema.validate`, `doc.validate` and `doc.repair` (10 §10.2, §10.3, AG-08; C1.3).
//!
//! The lint checks of 10 §10.3 run on the document as the invocation sees it. Checks that need computations owned by
//! later milestones (zones, fonts, lip-sync detection, storyline timing, library versions) are reported in
//! `unavailable` with their owner instead of being guessed (the C1.2 readiness ruling on derived fields).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::document::{Document, EntryKey, Severity, SlotKind, Target, slots_of};
use tt_model::entities::element::ElementType;
use tt_model::{ObjectType, object};
use tt_registry::{CommandError, codes, command};

use crate::cx::Cx;
use crate::types::ChangeRecord;

// ----- doc.object.refs ---------------------------------------------------------------------------------------------

/// `doc.object.refs` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefsParams {
    /// The object.
    #[serde(rename = "ref")]
    reference: String,
    /// `incoming`: objects that reference it; `outgoing`: objects it references.
    #[serde(default)]
    direction: RefDirection,
    /// Largest number of references returned.
    #[serde(default = "five_hundred")]
    #[schemars(range(min = 1, max = 100_000))]
    limit: usize,
}

fn five_hundred() -> usize {
    500
}

/// Reference direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RefDirection {
    /// Objects referencing it.
    #[default]
    Incoming,
    /// Objects it references.
    Outgoing,
}

/// One reference.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct RefEntry {
    /// The other object.
    id: String,
    /// Its type.
    #[serde(rename = "type")]
    ty: String,
    /// Its canonical path.
    path: Option<String>,
    /// The referencing property (JSON Pointer into the referencing object).
    property: String,
}

/// `{refs, total}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RefsResult {
    /// References.
    refs: Vec<RefEntry>,
    /// Total before the limit.
    total: usize,
}

/// Every reference the document holds: `(referencing object key, property pointer, referenced key)`. References are the
/// string values of an object's own properties that name another object (child objects and the stubs of split
/// children are containment, not references; `meta` and `ext` are opaque).
fn references(doc: &Document) -> Vec<(String, String, String)> {
    #[allow(clippy::too_many_arguments, reason = "a recursive walk carrying its context")]
    fn own(
        doc: &Document,
        v: &Value,
        ty: ObjectType,
        key: &str,
        container: Option<&str>,
        pointer: &str,
        out: &mut Vec<(String, String, String)>,
        depth: usize,
    ) {
        if depth > 100 {
            return;
        }
        let Value::Object(m) = v else { return };
        let slots: BTreeMap<&str, SlotKind> = slots_of(ty).iter().map(|(f, s)| (*f, *s)).collect();
        for (k, x) in m {
            if k == "meta" || k == "ext" || k == ty.id_field() || k == "type" {
                continue;
            }
            let at = format!("{pointer}/{}", tt_model::pointer::escape(k));
            match slots.get(k.as_str()) {
                Some(SlotKind::Split(_) | SlotKind::SplitOne(_)) => {}
                Some(SlotKind::Inline) => {
                    for c in x.as_array().into_iter().flatten() {
                        child(doc, c, container, out, depth);
                    }
                }
                Some(SlotKind::InlineOne) => child(doc, x, container, out, depth),
                Some(SlotKind::InlineMap) => {
                    for c in x.as_object().into_iter().flatten().map(|(_, c)| c) {
                        child(doc, c, container, out, depth);
                    }
                }
                None => strings(doc, x, key, container, &at, out, 0),
            }
        }
    }
    fn child(
        doc: &Document,
        c: &Value,
        container: Option<&str>,
        out: &mut Vec<(String, String, String)>,
        depth: usize,
    ) {
        let Some(t) = c.get("type").and_then(Value::as_str).and_then(ObjectType::parse) else { return };
        let Some(id) = c.get(t.id_field()).and_then(Value::as_str) else { return };
        let key = match t.local_id_container() {
            Some(_) => format!("{}/{id}", container.unwrap_or_default()),
            None => id.to_owned(),
        };
        let next = if matches!(t, ObjectType::Drawing | ObjectType::Channel) {
            Some(id.to_owned())
        } else {
            container.map(str::to_owned)
        };
        own(doc, c, t, &key, next.as_deref(), "", out, depth + 1);
    }
    fn strings(
        doc: &Document,
        v: &Value,
        key: &str,
        container: Option<&str>,
        at: &str,
        out: &mut Vec<(String, String, String)>,
        depth: usize,
    ) {
        if depth > 50 {
            return;
        }
        match v {
            Value::String(s) => {
                let target = doc
                    .locate(s)
                    .map(|l| l.key.clone())
                    .or_else(|| container.and_then(|c| doc.locate(&format!("{c}/{s}"))).map(|l| l.key.clone()));
                if let Some(t) = target
                    && t != key
                {
                    out.push((key.to_owned(), at.to_owned(), t));
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    strings(doc, x, key, container, &format!("{at}/{i}"), out, depth + 1);
                }
            }
            Value::Object(m) => {
                for (k, x) in m {
                    // Map keys that are IDs (palette styles per palette).
                    if let Some(l) = doc.locate(k)
                        && l.key != key
                    {
                        out.push((key.to_owned(), format!("{at}/{}", tt_model::pointer::escape(k)), l.key.clone()));
                    }
                    strings(doc, x, key, container, &format!("{at}/{}", tt_model::pointer::escape(k)), out, depth + 1);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for k in doc.entry_keys() {
        let Ok(v) = doc.entry_raw_json(&k) else { continue };
        let Some(ty) = v.get("type").and_then(Value::as_str).and_then(ObjectType::parse) else { continue };
        let id = v.get(ty.id_field()).and_then(Value::as_str).unwrap_or_default().to_owned();
        let container = matches!(ty, ObjectType::Drawing).then(|| id.clone());
        own(doc, &v, ty, &id, container.as_deref(), "", &mut out, 0);
    }
    out
}

#[command(
    id = "doc.object.refs",
    kind = query,
    label = "References",
    scopes = [read],
    needs_project,
    example(title = "Who uses the first palette", params = r#"{"ref": "palette:#1"}"#),
    agent_only = "agents inspect references",
)]
pub(crate) fn doc_object_refs(cx: &mut Cx, p: RefsParams) -> Result<RefsResult, CommandError> {
    let key = match cx.resolve(&p.reference)? {
        Target::Object { key, .. } => key,
        Target::Cell { .. } => return Err(super::common::invalid("/ref", "`ref` names an object")),
    };
    let doc = cx.document()?;
    let all = references(doc);
    let mut found: Vec<RefEntry> = Vec::new();
    for (from, prop, to) in all {
        let (other, property) = match p.direction {
            RefDirection::Incoming if to == key => (from, prop),
            RefDirection::Outgoing if from == key => (to, prop),
            _ => continue,
        };
        let ty = doc.locate(&other).map_or("unknown", |l| l.ty.as_str()).to_owned();
        found.push(RefEntry { path: doc.canonical_path(&other), id: other, ty, property });
    }
    let total = found.len();
    found.truncate(p.limit);
    Ok(RefsResult { refs: found, total })
}

// ----- doc.schema.validate -------------------------------------------------------------------------------------------

/// `doc.schema.validate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaValidateParams {
    /// Object type.
    #[serde(rename = "type")]
    ty: String,
    /// The candidate object.
    object: Map<String, Value>,
    /// `create`: a new object (ID and `rev` may be omitted); `replace`: the object with this ID, replaced.
    #[serde(default)]
    mode: ValidateMode,
}

/// Validation mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ValidateMode {
    /// A new object.
    Create,
    /// A replacement of an existing object.
    #[default]
    Replace,
}

/// One validation error.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ValidationError {
    /// JSON Pointer into the candidate.
    pointer: String,
    /// Code.
    code: String,
    /// Message.
    message: String,
}

/// `{valid, errors}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ValidateResult {
    /// Whether the candidate is valid.
    valid: bool,
    /// Errors.
    errors: Vec<ValidationError>,
}

macro_rules! input_as {
    ($ty:expr, $v:expr, $($variant:ident => $t:ty),+ $(,)?) => {
        match $ty {
            $(ObjectType::$variant => object::from_json::<$t>($v).and_then(|o| object::to_json(&o).map_err(|e| vec![e])),)+
        }
    };
}

/// The model's input path for an object of any type (schema and object-local rules); the canonical JSON.
pub(crate) fn input_any(ty: ObjectType, v: &Value) -> Result<Value, Vec<tt_model::ModelError>> {
    use tt_model::entities::{
        asset, channel, drawing, effect, element, library, palette, pen, project, scene, sound, transition,
    };
    input_as!(ty, v,
        Project => project::Project,
        Palette => palette::Palette,
        PaletteStyle => palette::PaletteStyle,
        Swatch => palette::Swatch,
        Pen => pen::Pen,
        Asset => asset::Asset,
        DrawingBank => drawing::DrawingBank,
        Drawing => drawing::Drawing,
        DrawingNode => drawing::DrawingNode,
        Stroke => drawing::Stroke,
        Scene => scene::Scene,
        Transition => transition::Transition,
        Element => element::Element,
        Channel => channel::Channel,
        Key => channel::Key,
        MotionPoint => channel::MotionPoint,
        ParticleBlock => effect::ParticleBlock,
        SoundPlacement => sound::SoundPlacement,
        MediaSequence => sound::MediaSequence,
        Library => library::Library,
        LibraryFolder => library::LibraryFolder,
        LibraryItem => library::LibraryItem,
    )
}

fn errors_of(es: &[tt_model::ModelError]) -> Vec<ValidationError> {
    es.iter()
        .map(|e| ValidationError {
            pointer: e.details().get("pointer").and_then(Value::as_str).unwrap_or_default().to_owned(),
            code: e.code().to_owned(),
            message: e.message().to_owned(),
        })
        .collect()
}

#[command(
    id = "doc.schema.validate",
    kind = query,
    label = "Validate candidate",
    scopes = [read],
    example(title = "Check a pen before creating it", params = r#"{"type": "pen", "object": {"name": "Ink", "kind": "brush", "minSize": 1, "maxSize": 4, "smoothness": 3, "tip": "round", "tipAngle": 0, "colourId": null}, "mode": "create"}"#),
    agent_only = "agents check candidates before applying them",
)]
pub(crate) fn doc_schema_validate(cx: &mut Cx, p: SchemaValidateParams) -> Result<ValidateResult, CommandError> {
    let ty = ObjectType::parse(&p.ty).ok_or_else(|| {
        CommandError::with_details(
            codes::ARG_INVALID,
            format!("unknown object type `{}`", p.ty),
            json!({ "pointer": "/type" }),
        )
        .hint("doc.schema.list names the types")
    })?;
    let mut v = Value::Object(p.object);
    let id_field = ty.id_field();
    if p.mode == ValidateMode::Create {
        // A new object: the ID and `rev` are assigned on creation; placeholders stand in for them here.
        if let Some(m) = v.as_object_mut() {
            m.entry("type").or_insert_with(|| json!(ty.as_str()));
            m.entry("rev").or_insert(json!(1));
            if !m.contains_key(id_field) {
                let id = tt_model::IdGenerator::seeded(1, 1).next(ty).map_err(CommandError::from)?;
                m.insert(id_field.into(), json!(id));
            }
        }
    }
    let canonical = match input_any(ty, &v) {
        Ok(c) => c,
        Err(es) => return Ok(ValidateResult { valid: false, errors: errors_of(&es) }),
    };
    if p.mode == ValidateMode::Replace && cx.has_project() {
        // The replacement must satisfy the document's rules too (references, names, trees, limits).
        let id = canonical.get(id_field).and_then(Value::as_str).unwrap_or_default().to_owned();
        let doc = cx.document()?;
        let Some(loc) = doc.locate(&id) else {
            return Ok(ValidateResult {
                valid: false,
                errors: vec![ValidationError {
                    pointer: format!("/{id_field}"),
                    code: "ref.notFound".into(),
                    message: format!("no {ty} `{id}` to replace"),
                }],
            });
        };
        if loc.ty != ty {
            return Ok(ValidateResult {
                valid: false,
                errors: vec![ValidationError {
                    pointer: format!("/{id_field}"),
                    code: "ref.wrongType".into(),
                    message: format!("`{id}` is a {}, not a {ty}", loc.ty),
                }],
            });
        }
        let (entry, _, pointer) = doc.object_pointer(&loc.key)?;
        let mut value = canonical.clone();
        // The entry keeps its stubs for split children (the candidate is the object's own JSON).
        if pointer.is_empty()
            && let (Ok(current), Some(m)) = (doc.entry_raw_json(&entry), value.as_object_mut())
        {
            for (field, slot) in slots_of(ty) {
                if matches!(slot, SlotKind::Split(_) | SlotKind::SplitOne(_))
                    && let Some(x) = current.get(*field)
                {
                    m.insert((*field).into(), x.clone());
                }
            }
        }
        let op = tt_model::change::PatchOp {
            op: "replace",
            from: None,
            path: pointer,
            range: None,
            value: Some(value),
            old_value: None,
            elided: false,
        };
        if let Err(es) = doc.with_patched_entries(&[(entry, vec![op])]) {
            return Ok(ValidateResult { valid: false, errors: errors_of(&es) });
        }
    }
    Ok(ValidateResult { valid: true, errors: Vec::new() })
}

// ----- doc.validate ------------------------------------------------------------------------------------------------

/// Minimum severity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SeverityParam {
    /// Everything.
    #[default]
    Info,
    /// Warnings and errors.
    Warning,
    /// Errors only.
    Error,
}

/// `doc.validate` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ValidateParams {
    /// Only this object and what it contains.
    #[serde(default)]
    within: Option<String>,
    /// Checks to run (default: all; 10 §10.3).
    #[serde(default)]
    checks: Option<Vec<String>>,
    /// Minimum severity.
    #[serde(default)]
    min_severity: SeverityParam,
    /// Largest number of issues.
    #[serde(default = "five_hundred")]
    #[schemars(range(min = 1, max = 100_000))]
    limit: usize,
}

/// A check that needs a computation owned by a later milestone.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct Unavailable {
    /// The check.
    check: String,
    /// The owning checkpoint.
    owner: String,
    /// Why.
    reason: String,
}

/// `doc.validate` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LintResult {
    /// The revision checked.
    document_rev: u64,
    /// Issues (10 §10.4).
    issues: Vec<super::doc_io::IssueOut>,
    /// Counts by severity.
    counts: BTreeMap<String, usize>,
    /// More issues than `limit`.
    truncated: bool,
    /// Checks not run, with their owner.
    unavailable: Vec<Unavailable>,
}

/// The checks of 10 §10.3, in table order.
pub(crate) const CHECKS: [&str; 19] = [
    "missingMedia",
    "missingDrawingContent",
    "emptyExposure",
    "exposedEmptyDrawing",
    "unusedDrawings",
    "unresolvedColours",
    "unpaintedZones",
    "unusedSwatches",
    "unusedAssets",
    "danglingReferences",
    "duplicateNames",
    "noCamera",
    "staleLipSync",
    "incompleteLipSyncMapping",
    "transitionPadding",
    "keysOutsideDuration",
    "missingFonts",
    "brokenLibraryItems",
    "outdatedTemplateLinks",
];

fn unavailable_owner(check: &str) -> Option<(&'static str, &'static str)> {
    Some(match check {
        "staleLipSync" => ("M8 C8.2", "lip-sync tracks and their timing"),
        "transitionPadding" => ("M9 C9.2", "storyline timing (08 §16.5)"),
        "outdatedTemplateLinks" => ("M9 C9.1", "library template versions"),
        _ => return None,
    })
}

struct Lint<'a> {
    doc: &'a Document,
    issues: Vec<super::doc_io::IssueOut>,
}

impl Lint<'_> {
    fn push(&mut self, code: &str, severity: &str, object: &str, message: String, details: Value, fixes: Vec<Value>) {
        let n = self.issues.len() + 1;
        self.issues.push(super::doc_io::IssueOut {
            issue_id: format!("is_{n}"),
            code: code.into(),
            severity: severity.into(),
            object_id: Some(object.to_owned()),
            path: self.doc.canonical_path(object),
            entry: None,
            message,
            details,
            fixes,
        });
    }
}

fn fix(id: usize, description: &str, command: &str, params: Value, default: bool) -> Value {
    json!({ "fixId": format!("fx_{id}"), "description": description, "command": command, "params": params, "default": default })
}

fn colour_strings(v: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::String(s) if s.starts_with("col_") => {
            out.insert(s.clone());
        }
        Value::Array(a) => a.iter().for_each(|x| colour_strings(x, out, depth + 1)),
        Value::Object(m) => {
            for (k, x) in m {
                if k != "meta" && k != "ext" && k != "colourRecovery" {
                    colour_strings(x, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
fn run_checks(cx: &Cx, checks: &BTreeSet<&str>) -> Result<Vec<super::doc_io::IssueOut>, CommandError> {
    let doc = cx.document()?;
    let store = &cx.project()?.store;
    let mut l = Lint { doc, issues: Vec::new() };
    let mut fx = 0usize;
    let mut next_fix = || {
        fx += 1;
        fx
    };
    let refs = super::doc_io::asset_refs(doc);
    if checks.contains("missingFonts") {
        // 02 §3.7: a text run whose font is not installed keeps its name and is displayed with a fallback font.
        for d in doc.drawings() {
            for o in tt_vector::drawing::objects(d) {
                let Some(n) = tt_vector::drawing::node_at(d, &o.pointer) else { continue };
                let mut seen: Vec<&str> = Vec::new();
                for r in n.runs().iter().flatten() {
                    if seen.contains(&r.font.as_str()) || tt_vector::fonts::is_installed(&r.font) {
                        continue;
                    }
                    seen.push(&r.font);
                    let object = format!("{}/{}", d.id().as_str(), o.id);
                    l.push(
                        "missingFonts",
                        "warning",
                        &object,
                        format!(
                            "font `{}` of a text object in drawing `{}` is not installed; a fallback font is used",
                            r.font,
                            d.name()
                        ),
                        json!({ "font": r.font, "drawingId": d.id().as_str(), "textId": o.id }),
                        Vec::new(),
                    );
                }
            }
        }
    }
    if checks.contains("unpaintedZones") {
        // 10 §10.3 / 11 §14 E2: closed zones bounded by visible strokes with no fill; the agent paints them by ID.
        let cache = cx.planar_cache();
        for d in doc.drawings() {
            for lz in crate::planar::drawing_zones(&cache, d)? {
                for (z, zone) in lz.map.zones.iter().enumerate() {
                    if !zone.visible_boundary || lz.fills.get(z).cloned().flatten().is_some_and(|f| f.painted()) {
                        continue;
                    }
                    let object = format!("{}/{}", d.id().as_str(), zone.id);
                    let f = fix(
                        next_fix(),
                        "Paint the zone (choose a colourId)",
                        "colour.paint.apply",
                        json!({ "drawingId": d.id().as_str(), "objectIds": [zone.id] }),
                        false,
                    );
                    l.push(
                        "unpaintedZones",
                        "info",
                        &object,
                        format!("a zone of {:.1} du² in drawing `{}` is unpainted", zone.area, d.name()),
                        json!({
                            "drawingId": d.id().as_str(), "layerId": lz.layer.id, "zoneId": zone.id, "area": zone.area,
                            "interiorPoint": { "x": zone.interior.x, "y": zone.interior.y },
                        }),
                        vec![f],
                    );
                }
            }
        }
    }
    if checks.contains("missingMedia") {
        for (entry, owner, _, kind) in tt_persist::load::blob_refs(doc) {
            if !store.blobs.contains(&entry) && kind != "libraryFile" {
                l.push(
                    "missingMedia",
                    "warning",
                    &owner,
                    format!("`{entry}` is missing"),
                    json!({ "entry": entry, "kind": kind }),
                    Vec::new(),
                );
            }
        }
        for a in doc.project().assets() {
            let id = a.id().as_str();
            if a.link().is_some() && !store.linked.contains_key(id) {
                let f = fix(
                    next_fix(),
                    "Search the project's folder for the file",
                    "doc.asset.relink",
                    json!({ "searchFolder": cx.project()?.path.as_ref().and_then(|p| std::path::Path::new(p).parent().map(|x| x.to_string_lossy().into_owned())) }),
                    false,
                );
                l.push(
                    "missingMedia",
                    "warning",
                    id,
                    format!("the linked file of `{}` is missing", a.name()),
                    json!({ "link": a.link() }),
                    vec![f],
                );
            }
        }
    }
    if checks.contains("missingDrawingContent") {
        for d in doc.drawings() {
            let v = doc.entry_raw_json(&EntryKey::Drawing(d.id().clone()))?;
            let ext = v.get("ext");
            if ext.and_then(|e| e.get("app.unreadable")).is_some()
                || ext.and_then(|e| e.get("app.contentMissing")).is_some()
            {
                l.push(
                    "missingDrawingContent",
                    "error",
                    d.id().as_str(),
                    format!("the content of drawing `{}` was missing or unreadable", d.name()),
                    json!({}),
                    Vec::new(),
                );
            }
        }
    }
    // Exposures and drawing usage.
    let mut exposed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut mapped: BTreeSet<String> = BTreeSet::new();
    for e in doc.elements() {
        for (d, _) in e.exposure().iter().flatten() {
            if let Some(d) = d {
                exposed.entry(d.as_str().to_owned()).or_default().insert(e.id().as_str().to_owned());
            }
        }
        for m in e.lip_sync_mappings().iter().flatten() {
            let v = serde_json::to_value(&m.mapping).unwrap_or(Value::Null);
            for x in v.as_object().into_iter().flatten().filter_map(|(_, d)| d.as_str()) {
                mapped.insert(x.to_owned());
            }
        }
    }
    if checks.contains("emptyExposure") {
        for e in doc.elements() {
            let t = *e.element_type();
            let empty = match t {
                ElementType::Drawing | ElementType::Annotation | ElementType::Image => {
                    e.exposure().iter().flatten().all(|(d, _)| d.is_none())
                }
                ElementType::Sound => e.placements().iter().flatten().next().is_none(),
                ElementType::Media => e.sequences().iter().flatten().next().is_none(),
                _ => false,
            };
            if empty {
                let f = fix(
                    next_fix(),
                    "Delete the element",
                    "element.delete",
                    json!({ "elementIds": [e.id().as_str()] }),
                    false,
                );
                l.push(
                    "emptyExposure",
                    "warning",
                    e.id().as_str(),
                    format!("`{}` exposes no cell", e.name()),
                    json!({}),
                    vec![f],
                );
            }
        }
    }
    if checks.contains("exposedEmptyDrawing") {
        for d in doc.drawings() {
            if exposed.contains_key(d.id().as_str()) && d.nodes().is_empty() {
                l.push(
                    "exposedEmptyDrawing",
                    "info",
                    d.id().as_str(),
                    format!("drawing `{}` is exposed but empty", d.name()),
                    json!({}),
                    Vec::new(),
                );
            }
        }
    }
    if checks.contains("unusedDrawings") {
        for b in doc.banks() {
            let users = doc.bank_users(b.id());
            for d in b.drawings().ids() {
                if !exposed.contains_key(d.as_str()) && !mapped.contains(d.as_str()) {
                    let Some(dr) = doc.drawing(d) else { continue };
                    let fixes = users
                        .first()
                        .map(|u| {
                            fix(
                                next_fix(),
                                "Delete the drawing",
                                "element.drawing.delete",
                                json!({ "elementId": u.as_str(), "drawingIds": [d.as_str()] }),
                                true,
                            )
                        })
                        .into_iter()
                        .collect();
                    l.push(
                        "unusedDrawings",
                        "info",
                        d.as_str(),
                        format!("drawing `{}` is not exposed in any cell", dr.name()),
                        json!({ "bankId": b.id().as_str() }),
                        fixes,
                    );
                }
            }
        }
    }
    if checks.contains("unresolvedColours") {
        for i in doc.issues().into_iter().filter(|i| i.code == "colour.unresolved") {
            let object = i.object_id.clone().unwrap_or_default();
            let f = fix(next_fix(), "Run colour recovery", "colour.recovery.run", json!({}), true);
            l.push("unresolvedColours", "error", &object, i.message.clone(), json!({ "pointer": i.pointer }), vec![f]);
        }
    }
    if checks.contains("unusedSwatches") {
        let mut used = BTreeSet::new();
        for d in doc.drawings() {
            colour_strings(&doc.entry_raw_json(&EntryKey::Drawing(d.id().clone()))?, &mut used, 0);
        }
        for p in doc.project().pens() {
            if let Some(c) = p.colour_id() {
                used.insert(c.as_str().to_owned());
            }
        }
        for p in doc.palettes() {
            for s in p.swatches() {
                if !used.contains(s.colour_id().as_str()) {
                    let f = fix(
                        next_fix(),
                        "Delete the swatch",
                        "colour.swatch.delete",
                        json!({ "colourIds": [s.colour_id().as_str()] }),
                        false,
                    );
                    l.push(
                        "unusedSwatches",
                        "info",
                        s.colour_id().as_str(),
                        format!("swatch `{}` is used by no drawing or pen", s.name()),
                        json!({ "paletteId": p.id().as_str() }),
                        vec![f],
                    );
                }
            }
        }
    }
    if checks.contains("unusedAssets") {
        for a in doc.project().assets() {
            if !refs.contains_key(a.id().as_str()) {
                let f =
                    fix(next_fix(), "Remove media nothing uses", "doc.asset.remove", json!({ "unused": true }), true);
                l.push(
                    "unusedAssets",
                    "info",
                    a.id().as_str(),
                    format!("`{}` is referenced by nothing", a.name()),
                    json!({}),
                    vec![f],
                );
            }
        }
    }
    if checks.contains("danglingReferences") {
        for i in doc.issues().into_iter().filter(|i| i.code == tt_model::codes::REF_DANGLING) {
            let sev = match i.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            let object = i.object_id.clone().unwrap_or_default();
            l.push("danglingReferences", sev, &object, i.message.clone(), json!({ "pointer": i.pointer }), Vec::new());
        }
    }
    if checks.contains("noCamera") {
        for s in doc.scenes() {
            let cameras = doc
                .tree_order(s)
                .into_iter()
                .filter(|e| doc.element(e).is_some_and(|x| *x.element_type() == ElementType::Camera))
                .count();
            if cameras == 0 || s.current_camera_id().is_none() {
                let f =
                    fix(next_fix(), "Add a camera", "anim.camera.create", json!({ "sceneId": s.id().as_str() }), false);
                l.push(
                    "noCamera",
                    "warning",
                    s.id().as_str(),
                    format!("scene `{}` has no camera", s.name()),
                    json!({}),
                    vec![f],
                );
            }
        }
    }
    if checks.contains("incompleteLipSyncMapping") {
        for e in doc.elements() {
            for (i, m) in e.lip_sync_mappings().iter().flatten().enumerate() {
                let v = serde_json::to_value(&m.mapping).unwrap_or(Value::Null);
                let missing: Vec<&str> = ["A", "B", "C", "D", "E", "F", "G", "X"]
                    .into_iter()
                    .filter(|s| v.get(*s).is_none_or(Value::is_null))
                    .collect();
                if !missing.is_empty() {
                    let f = fix(
                        next_fix(),
                        "Map the missing mouth shapes",
                        "lipsync.mapping.set",
                        json!({ "elementId": e.id().as_str() }),
                        false,
                    );
                    l.push(
                        "incompleteLipSyncMapping",
                        "info",
                        e.id().as_str(),
                        format!("mapping {} of `{}` leaves {missing:?} unmapped", i + 1, e.name()),
                        json!({ "unmapped": missing }),
                        vec![f],
                    );
                }
            }
        }
    }
    if checks.contains("keysOutsideDuration") {
        for e in doc.elements() {
            let (start, end) = match (e.start(), e.duration()) {
                (Some(s), Some(d)) => (u64::from(*s), u64::from(*s) + u64::from(*d).saturating_sub(1)),
                _ => {
                    let len: u64 = e.exposure().iter().flatten().map(|(_, c)| u64::from(*c)).sum();
                    (1, len.max(1))
                }
            };
            let v = serde_json::to_value(e.channels()).unwrap_or(Value::Null);
            let mut outside = Vec::new();
            for (name, ch) in v.as_object().into_iter().flatten() {
                for k in ch.get("keys").and_then(Value::as_array).into_iter().flatten() {
                    if let Some(f) = k.get("frame").and_then(Value::as_u64)
                        && (f < start || f > end)
                    {
                        outside.push(json!({ "channel": name, "frame": f }));
                    }
                }
            }
            if !outside.is_empty() {
                l.push(
                    "keysOutsideDuration",
                    "warning",
                    e.id().as_str(),
                    format!("`{}` has keys outside its bar ({start}–{end})", e.name()),
                    json!({ "keys": outside }),
                    Vec::new(),
                );
            }
        }
    }
    if checks.contains("brokenLibraryItems") {
        for (entry, owner, _, kind) in tt_persist::load::blob_refs(doc) {
            if kind == "libraryFile" && !store.blobs.contains(&entry) {
                l.push(
                    "brokenLibraryItems",
                    "warning",
                    &owner,
                    format!("the library file `{entry}` is missing"),
                    json!({ "entry": entry }),
                    Vec::new(),
                );
            }
        }
    }
    // duplicateNames: names are unique by the model's integrity rules; an open document cannot hold duplicates (the
    // load repairs them, 10 §7.2), so the check finds nothing by construction.
    Ok(l.issues)
}

fn within_filter(
    cx: &Cx,
    within: Option<&String>,
    issues: Vec<super::doc_io::IssueOut>,
) -> Result<Vec<super::doc_io::IssueOut>, CommandError> {
    let Some(w) = within else { return Ok(issues) };
    let key = match cx.resolve(w)? {
        Target::Object { key, .. } => key,
        Target::Cell { .. } => return Err(super::common::invalid("/within", "`within` names an object")),
    };
    let doc = cx.document()?;
    let prefix = doc.canonical_path(&key).unwrap_or_default();
    Ok(issues
        .into_iter()
        .filter(|i| {
            i.object_id.as_deref() == Some(key.as_str()) || i.path.as_ref().is_some_and(|p| p.starts_with(&prefix))
        })
        .collect())
}

fn selected_checks(p: Option<&Vec<String>>) -> Result<(BTreeSet<&'static str>, Vec<Unavailable>), CommandError> {
    let wanted: Vec<&'static str> = match p {
        None => CHECKS.to_vec(),
        Some(list) => {
            let mut out = Vec::new();
            for (i, c) in list.iter().enumerate() {
                let Some(known) = CHECKS.iter().find(|k| **k == c.as_str()) else {
                    return Err(CommandError::with_details(
                        codes::ARG_INVALID,
                        format!("unknown check `{c}`"),
                        json!({ "pointer": format!("/checks/{i}"), "checks": CHECKS }),
                    ));
                };
                out.push(*known);
            }
            out
        }
    };
    let mut run = BTreeSet::new();
    let mut unavailable = Vec::new();
    for c in wanted {
        match unavailable_owner(c) {
            Some((owner, reason)) => {
                unavailable.push(Unavailable { check: c.into(), owner: owner.into(), reason: reason.into() })
            }
            None => {
                run.insert(c);
            }
        }
    }
    Ok((run, unavailable))
}

#[command(
    id = "doc.validate",
    kind = query,
    label = "Validate project",
    scopes = [read],
    needs_project,
    example(title = "Errors only", params = r#"{"minSeverity": "error"}"#),
    gui = "Project › Check Project (M4)",
)]
pub(crate) fn doc_validate(cx: &mut Cx, p: ValidateParams) -> Result<LintResult, CommandError> {
    let (run, unavailable) = selected_checks(p.checks.as_ref())?;
    let rank = |s: &str| match s {
        "error" => SeverityParam::Error,
        "warning" => SeverityParam::Warning,
        _ => SeverityParam::Info,
    };
    let issues = within_filter(cx, p.within.as_ref(), run_checks(cx, &run)?)?;
    let mut issues: Vec<_> = issues.into_iter().filter(|i| rank(&i.severity) >= p.min_severity).collect();
    let mut counts = BTreeMap::from([("error".to_owned(), 0), ("warning".to_owned(), 0), ("info".to_owned(), 0)]);
    for i in &issues {
        *counts.entry(i.severity.clone()).or_default() += 1;
    }
    let truncated = issues.len() > p.limit;
    issues.truncate(p.limit);
    Ok(LintResult {
        document_rev: cx.predicted_rev().unwrap_or(0).saturating_sub(u64::from(!cx.has_staged())),
        issues,
        counts,
        truncated,
        unavailable,
    })
}

// ----- doc.repair --------------------------------------------------------------------------------------------------

/// `doc.repair` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RepairParams {
    /// Issues (from `doc.validate` of the same revision).
    #[serde(default)]
    issue_ids: Option<Vec<String>>,
    /// Checks or issue codes.
    #[serde(default)]
    codes: Option<Vec<String>>,
    /// Only this object and what it contains.
    #[serde(default)]
    within: Option<String>,
    /// Every issue with a default fix.
    #[serde(default)]
    all: bool,
    /// Report what would be fixed; change nothing.
    #[serde(default)]
    #[allow(dead_code, reason = "read by the executor: a set `dryRun` makes the call a dry run (10 AG-02)")]
    dry_run: bool,
}

/// A skipped issue.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Skipped {
    /// The issue.
    issue_id: String,
    /// Why.
    reason: String,
}

/// `doc.repair` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RepairResult {
    /// Issues fixed.
    fixed: Vec<String>,
    /// Issues not fixed, with the reason.
    skipped: Vec<Skipped>,
    /// The changes made (the envelope carries them too).
    changes: Vec<ChangeRecord>,
}

#[command(
    id = "doc.repair",
    kind = action,
    label = "Repair",
    scopes = [edit],
    undo = "Repair Project",
    needs_project,
    example(title = "Remove unused media", params = r#"{"codes": ["unusedAssets"], "dryRun": true}"#),
    gui = "Project › Check Project › Repair (M4)",
)]
pub(crate) fn doc_repair(cx: &mut Cx, p: RepairParams) -> Result<RepairResult, CommandError> {
    let (run, _) = selected_checks(None)?;
    let issues = within_filter(cx, p.within.as_ref(), run_checks(cx, &run)?)?;
    let selected: Vec<_> = issues
        .into_iter()
        .filter(|i| {
            p.all
                || p.issue_ids.as_ref().is_some_and(|ids| ids.contains(&i.issue_id))
                || p.codes.as_ref().is_some_and(|cs| {
                    cs.iter().any(|c| *c == i.code || i.details.get("code").and_then(Value::as_str) == Some(c.as_str()))
                })
        })
        .collect();
    let mut fixed = Vec::new();
    let mut skipped = Vec::new();
    let mut unused_assets = Vec::new();
    for i in selected {
        let Some(f) = i.fixes.iter().find(|f| f.get("default") == Some(&Value::Bool(true))) else {
            skipped.push(Skipped { issue_id: i.issue_id.clone(), reason: "the issue has no default fix".into() });
            continue;
        };
        let command = f.get("command").and_then(Value::as_str).unwrap_or_default();
        match command {
            "doc.asset.remove" => {
                if let Some(id) = &i.object_id {
                    unused_assets.push(id.clone());
                }
                fixed.push(i.issue_id.clone());
            }
            other => {
                let owner = crate::registry().ok().and_then(|r| r.get(other)).map_or_else(
                    || "a later milestone".to_owned(),
                    |e| match e.status() {
                        tt_registry::Status::NotImplemented { owner } => owner.to_owned(),
                        tt_registry::Status::Implemented => "the command itself".to_owned(),
                    },
                );
                skipped.push(Skipped {
                    issue_id: i.issue_id.clone(),
                    reason: format!("its fix `{other}` is implemented in {owner}; run it directly when available"),
                });
            }
        }
    }
    if !unused_assets.is_empty() {
        super::doc_io::remove_assets(cx, &unused_assets, false)?;
    }
    let changes = cx.preview_changes()?.0;
    Ok(RepairResult { fixed, skipped, changes })
}
