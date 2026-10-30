//! `doc.*` (10 §10): the generic object-model commands. C1.2 implements reading (`doc.object.get`, `.find`,
//! `.resolve`), the generic patch (`doc.object.patch`, AG-02), schemas (`doc.schema.list`, `.get`), document state and
//! diffs. C1.3 adds the package, asset and load-report commands (`doc_io`), validation and repair (`doc_lint`) and
//! the interchange bundle (`bundle`).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};
use tt_model::change::{self, OwnChange, PatchOp};
use tt_model::document::{Document, RenderOptions, Target, slots_of};
use tt_model::{Id, ObjectType};
use tt_registry::{CommandError, codes, command};

use super::common::{NoParams, event_not_invocable, invalid};
use super::derived;
use crate::cx::Cx;
use crate::session::Back;
use crate::types::ChangeRecord;

/// Largest inline result (11 §6.5 `maxInlineBytes` default).
pub(crate) const MAX_RESULT_BYTES: usize = 52_428_800;

fn some<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

// ----- doc.object.get -------------------------------------------------------------------------------------------

/// What to add to returned objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Include {
    /// Derived properties (10 §2.7, AG-04).
    Derived,
    /// Size statistics, returned beside the object (ADR-0013 §9; SI-015).
    Stats,
}

/// Paging of one large array inside the object (AG-05).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Page {
    /// JSON Pointer of the array inside the object.
    pointer: String,
    /// First item.
    #[serde(default)]
    #[schemars(range(min = 0, max = 100_000_000))]
    offset: usize,
    /// Items.
    #[serde(default = "thousand")]
    #[schemars(range(min = 1, max = 100_000))]
    limit: usize,
}

fn thousand() -> usize {
    1000
}

fn one_i32() -> i32 {
    1
}

/// `doc.object.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    /// One object (id, compound id, path or special reference).
    #[serde(default, rename = "ref")]
    reference: Option<String>,
    /// Several objects (`@selection` expands to the selection).
    #[serde(default)]
    #[schemars(length(min = 1, max = 1000))]
    refs: Option<Vec<String>>,
    /// Object levels in full: 1 = children as stubs, 0 = the object as a stub, -1 = everything (SI-011).
    #[serde(default = "one_i32")]
    #[schemars(range(min = -1, max = 20))]
    depth: i32,
    /// Top-level properties to keep (`id` and `type` are always kept).
    #[serde(default)]
    fields: Option<Vec<String>>,
    /// Extra data.
    #[serde(default)]
    include: Vec<Include>,
    /// Page through one large array.
    #[serde(default)]
    page: Option<Page>,
}

/// Paging information.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageInfo {
    /// The paged array.
    pointer: String,
    /// Items in the array.
    total: usize,
    /// Offset of the next page (`null` at the end).
    next_offset: Option<usize>,
}

/// `doc.object.get` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct GetResult {
    /// The object (single `ref`).
    #[serde(skip_serializing_if = "Option::is_none")]
    object: Option<Map<String, Value>>,
    /// The objects (`refs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    objects: Option<Vec<Map<String, Value>>>,
    /// Statistics per object (`include: ["stats"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<Vec<Value>>,
    /// Paging of the array named by `page.pointer`, per object.
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<Vec<PageInfo>>,
}

fn count_objects(v: &Value, by_type: &mut BTreeMap<String, u64>) {
    match v {
        Value::Object(m) => {
            if let Some(t) = m.get("type").and_then(Value::as_str).filter(|t| ObjectType::parse(t).is_some())
                && m.contains_key("rev")
            {
                *by_type.entry(t.to_owned()).or_default() += 1;
            }
            m.values().for_each(|x| count_objects(x, by_type));
        }
        Value::Array(a) => a.iter().for_each(|x| count_objects(x, by_type)),
        _ => {}
    }
}

#[command(
    id = "doc.object.get",
    kind = query,
    label = "Get object",
    scopes = [read],
    needs_project,
    example(title = "The first scene with its elements as stubs", params = r#"{"ref": "scene:#1", "depth": 1}"#),
    example(title = "The current element with derived properties", params = r#"{"ref": "@current", "include": ["derived"]}"#),
    example(title = "Only names and types of two objects", params = r#"{"refs": ["scene:#1", "palette:#1"], "fields": ["name"]}"#),
    agent_only = "agents read the object model; the GUI shows it in its panels (11 §7.4 get_object)",
)]
pub(crate) fn doc_object_get(cx: &mut Cx, p: GetParams) -> Result<GetResult, CommandError> {
    let refs: Vec<String> = match (&p.reference, &p.refs) {
        (Some(r), None) => vec![r.clone()],
        (None, Some(rs)) => rs.clone(),
        (None, None) => {
            return Err(CommandError::with_details(
                codes::ARG_MISSING,
                "give `ref` or `refs`",
                json!({ "pointer": "/ref" }),
            ));
        }
        (Some(_), Some(_)) => return Err(invalid("/refs", "give either `ref` or `refs`, not both")),
    };
    let targets = cx.resolve_objects(&refs)?;
    let doc = cx.document()?.clone();
    let depth = if p.depth < 0 { None } else { Some(u32::try_from(p.depth).unwrap_or(1)) };
    let mut objects = Vec::with_capacity(targets.len());
    let mut stats = Vec::new();
    let mut pages = Vec::new();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for (ty, key) in &targets {
        let mut v = doc.object_json(key, RenderOptions { depth, path: true })?;
        if p.include.contains(&Include::Stats) {
            let mut by_type = BTreeMap::new();
            count_objects(&v, &mut by_type);
            let bytes = serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0);
            stats.push(json!({ "id": key, "jsonBytes": bytes, "objectsByType": by_type }));
        }
        if let Some(pg) = &p.page {
            let tokens = tt_model::pointer::parse(&pg.pointer)?;
            let mut at: Option<&mut Value> = Some(&mut v);
            for t in &tokens {
                at = at.and_then(|x| match x {
                    Value::Object(m) => m.get_mut(t.as_str()),
                    Value::Array(a) => t.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
                    _ => None,
                });
            }
            let Some(Value::Array(items)) = at else {
                return Err(invalid("/page/pointer", format!("`{}` is not an array of `{key}`", pg.pointer)));
            };
            let total = items.len();
            let end = pg.offset.saturating_add(pg.limit).min(total);
            let slice: Vec<Value> = items.get(pg.offset.min(total)..end).map(<[Value]>::to_vec).unwrap_or_default();
            *items = slice;
            pages.push(PageInfo { pointer: pg.pointer.clone(), total, next_offset: (end < total).then_some(end) });
        }
        let Value::Object(mut m) = v else { return Err(CommandError::internal("object JSON is not an object")) };
        if p.include.contains(&Include::Derived) && depth != Some(0) {
            for (name, owner) in derived::add(&doc, *ty, key, &mut m) {
                missing.insert(format!("{}.{name} ({owner})", ty.as_str()));
            }
        }
        if let Some(fields) = &p.fields {
            m.retain(|k, _| k == "id" || k == "colourId" || k == "type" || fields.iter().any(|f| f == k));
        }
        objects.push(m);
    }
    if !missing.is_empty() {
        cx.warn(
            "doc.derivedUnavailable",
            format!("derived properties not computed yet: {}", missing.into_iter().collect::<Vec<_>>().join(", ")),
            targets.iter().map(|(_, k)| k.clone()).collect(),
        );
    }
    let bytes: usize = objects.iter().map(|o| serde_json::to_string(o).map(|s| s.len()).unwrap_or(0)).sum();
    if bytes > MAX_RESULT_BYTES {
        return Err(CommandError::with_details(
            codes::LIMIT_EXCEEDED,
            format!("the result would be {bytes} bytes (limit {MAX_RESULT_BYTES})"),
            json!({ "bytes": bytes, "limit": MAX_RESULT_BYTES }),
        )
        .hint("use depth, fields or page (10 AG-05)"));
    }
    cx.set_targets(json!({ "ids": targets.iter().map(|(_, k)| k).collect::<Vec<_>>() }));
    let stats = (!stats.is_empty()).then_some(stats);
    let page = (!pages.is_empty()).then_some(pages);
    Ok(if p.reference.is_some() {
        GetResult { object: objects.into_iter().next(), objects: None, stats, page }
    } else {
        GetResult { object: None, objects: Some(objects), stats, page }
    })
}

// ----- doc.object.find ------------------------------------------------------------------------------------------

/// Sort order of `doc.object.find`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Sort {
    /// Canonical path (AG-06).
    #[default]
    Path,
    /// Name.
    Name,
    /// Revision.
    Rev,
    /// Creation time.
    CreatedAt,
}

fn hundred() -> usize {
    100
}

/// `doc.object.find` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FindParams {
    /// Object type (or alias).
    #[serde(default, rename = "type")]
    ty: Option<String>,
    /// Only descendants of this object.
    #[serde(default)]
    within: Option<String>,
    /// Name: exact, or a glob with `*` and `?`.
    #[serde(default)]
    name: Option<String>,
    /// Predicates (10 §10.3).
    #[serde(default, rename = "where")]
    predicate: Option<Map<String, Value>>,
    /// Page size.
    #[serde(default = "hundred")]
    #[schemars(range(min = 1, max = 1000))]
    limit: usize,
    /// First item.
    #[serde(default)]
    #[schemars(range(min = 0, max = 100_000_000))]
    offset: usize,
    /// Extra top-level properties per item.
    #[serde(default)]
    fields: Option<Vec<String>>,
    /// Order.
    #[serde(default)]
    sort: Sort,
}

/// `doc.object.find` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindResult {
    /// Items `{id, type, name, path, rev, …fields}`.
    items: Vec<Map<String, Value>>,
    /// Matches in total.
    total: usize,
    /// Offset of the next page (`null` at the end).
    next_offset: Option<usize>,
}

/// `*` and `?` glob, case-sensitive.
pub(crate) fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn get_dotted<'a>(obj: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(obj, |cur, seg| match cur {
        Value::Object(m) => m.get(seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    })
}

fn cmp_json(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

/// Evaluates a `where` object (AND of its keys; `anyOf` is OR).
fn matches(obj: &Value, pred: &Map<String, Value>, at: &str) -> Result<bool, CommandError> {
    for (k, cond) in pred {
        if k == "anyOf" {
            let Value::Array(alts) = cond else {
                return Err(invalid(&format!("{at}/anyOf"), "anyOf takes an array of predicates"));
            };
            let mut any = false;
            for (i, a) in alts.iter().enumerate() {
                let Value::Object(m) = a else {
                    return Err(invalid(&format!("{at}/anyOf/{i}"), "a predicate is an object"));
                };
                if matches(obj, m, &format!("{at}/anyOf/{i}"))? {
                    any = true;
                    break;
                }
            }
            if !any {
                return Ok(false);
            }
            continue;
        }
        let actual = get_dotted(obj, k);
        let (op, value) = match cond {
            Value::Object(m) if m.contains_key("op") => (
                m.get("op").and_then(Value::as_str).unwrap_or_default(),
                m.get("value").cloned().unwrap_or(Value::Null),
            ),
            other => ("eq", other.clone()),
        };
        let here = format!("{at}/{}", tt_model::pointer::escape(k));
        let ok = match op {
            "eq" => actual == Some(&value),
            "ne" => actual != Some(&value),
            "lt" | "lte" | "gt" | "gte" => actual.and_then(|a| cmp_json(a, &value)).is_some_and(|o| match op {
                "lt" => o.is_lt(),
                "lte" => o.is_le(),
                "gt" => o.is_gt(),
                _ => o.is_ge(),
            }),
            "in" | "nin" => {
                let Value::Array(set) = &value else { return Err(invalid(&here, "`in`/`nin` take an array")) };
                let inside = actual.is_some_and(|a| set.contains(a));
                if op == "in" { inside } else { !inside }
            }
            "glob" => match (actual, &value) {
                (Some(Value::String(a)), Value::String(p)) => glob(p, a),
                (_, Value::String(_)) => false,
                _ => return Err(invalid(&here, "`glob` takes a string")),
            },
            "regex" => {
                let Value::String(p) = &value else { return Err(invalid(&here, "`regex` takes a string")) };
                let re = regex::RegexBuilder::new(p)
                    .size_limit(1 << 20)
                    .build()
                    .map_err(|e| invalid(&here, format!("invalid regex: {e}")))?;
                matches!(actual, Some(Value::String(a)) if re.is_match(a))
            }
            "exists" => actual.is_some() == value.as_bool().unwrap_or(true),
            "contains" => matches!(actual, Some(Value::Array(a)) if a.contains(&value)),
            other => {
                return Err(CommandError::with_details(
                    codes::ARG_INVALID,
                    format!("unknown predicate operator `{other}`"),
                    json!({ "pointer": here, "allowed": { "enum": ["eq", "ne", "lt", "lte", "gt", "gte", "in", "nin", "glob", "regex", "exists", "contains"] } }),
                ));
            }
        };
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A match of `doc.object.find`: key, type, rev and (when needed) its JSON.
type Found = (String, ObjectType, u64, Option<Value>);

fn is_within(doc: &Document, key: &str, ancestor: &str) -> bool {
    let mut cur = doc.locate(key).and_then(|l| l.parent.clone());
    let mut guard = 0;
    while let Some(k) = cur {
        if k == ancestor {
            return true;
        }
        guard += 1;
        if guard > 256 {
            return false;
        }
        cur = doc.locate(&k).and_then(|l| l.parent.clone());
    }
    false
}

fn parse_type(name: &str, pointer: &str) -> Result<ObjectType, CommandError> {
    ObjectType::parse(name).ok_or_else(|| {
        CommandError::with_details(
            codes::ARG_INVALID,
            format!("unknown object type `{name}`"),
            json!({ "pointer": pointer, "allowed": { "enum": ObjectType::ALL.iter().map(|t| t.as_str()).collect::<Vec<_>>() } }),
        )
    })
}

#[command(
    id = "doc.object.find",
    kind = query,
    label = "Find objects",
    scopes = [read],
    needs_project,
    example(title = "Every drawing element", params = r#"{"type": "element", "where": {"elementType": {"op": "in", "value": ["drawing", "image"]}}}"#),
    example(title = "Strokes of the first drawing", params = r#"{"type": "stroke", "within": "drawingBank:#1/drawing:#1", "limit": 10}"#),
    example(title = "Objects whose name starts with B", params = r#"{"name": "B*"}"#),
    agent_only = "agents query the object model (11 §7.4 find_objects)",
)]
pub(crate) fn doc_object_find(cx: &mut Cx, p: FindParams) -> Result<FindResult, CommandError> {
    let ty = p.ty.as_deref().map(|t| parse_type(t, "/type")).transpose()?;
    let within = match &p.within {
        Some(w) => match cx.resolve(w)? {
            Target::Object { key, .. } => Some(key),
            Target::Cell { .. } => return Err(invalid("/within", "`within` names an object")),
        },
        None => None,
    };
    let doc = cx.document()?.clone();
    let needs_json = p.name.is_some()
        || p.predicate.is_some()
        || p.fields.is_some()
        || matches!(p.sort, Sort::Name | Sort::CreatedAt);
    let mut found: Vec<Found> = Vec::new();
    for loc in doc.locations() {
        if ty.is_some_and(|t| t != loc.ty) {
            continue;
        }
        if within.as_ref().is_some_and(|w| !is_within(&doc, &loc.key, w)) {
            continue;
        }
        if p.name.is_some() && !loc.ty.has_name() {
            continue;
        }
        let json = if needs_json { Some(doc.object_raw_json(&loc.key)?.1) } else { None };
        if let (Some(n), Some(j)) = (&p.name, &json) {
            let Some(name) = j.get("name").and_then(Value::as_str) else { continue };
            let hit = if n.contains(['*', '?']) { glob(n, name) } else { name == n };
            if !hit {
                continue;
            }
        }
        if let (Some(pred), Some(j)) = (&p.predicate, &json)
            && !matches(j, pred, "/where")?
        {
            continue;
        }
        found.push((loc.key.clone(), loc.ty, loc.rev, json));
    }
    // Canonical paths of every match in one pass (C12-R1-23: no sibling scan per object).
    let paths: Vec<Option<String>> = if p.sort == Sort::Path {
        doc.canonical_paths(found.iter().map(|f| f.0.as_str()))
    } else {
        vec![None; found.len()]
    };
    let mut keyed: Vec<(String, Found)> = found
        .into_iter()
        .zip(paths)
        .map(|(f, path)| {
            let sort_key = match p.sort {
                Sort::Path => path.unwrap_or_default(),
                Sort::Name => {
                    f.3.as_ref().and_then(|j| j.get("name")).and_then(Value::as_str).unwrap_or_default().to_owned()
                }
                Sort::Rev => format!("{:020}", f.2),
                Sort::CreatedAt => {
                    f.3.as_ref().and_then(|j| j.get("createdAt")).and_then(Value::as_str).unwrap_or_default().to_owned()
                }
            };
            (sort_key, f)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.0.cmp(&b.1.0)));
    let total = keyed.len();
    let end = p.offset.saturating_add(p.limit).min(total);
    let mut items = Vec::new();
    for (sort_key, (key, ty, rev, json)) in keyed.into_iter().skip(p.offset).take(p.limit) {
        let mut m = Map::new();
        m.insert("id".into(), json!(key));
        m.insert("type".into(), json!(ty.as_str()));
        let name = match &json {
            Some(j) => j.get("name").cloned(),
            None if ty.has_name() => doc.object_raw_json(&key)?.1.get("name").cloned(),
            None => None,
        };
        m.insert("name".into(), name.unwrap_or(Value::Null));
        let path = if p.sort == Sort::Path { Some(sort_key) } else { doc.canonical_path(&key) };
        m.insert("path".into(), json!(path));
        m.insert("rev".into(), json!(rev));
        if let (Some(fields), Some(j)) = (&p.fields, &json) {
            for f in fields {
                if let Some(v) = j.get(f) {
                    m.entry(f.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        items.push(m);
    }
    Ok(FindResult { items, total, next_offset: (end < total).then_some(end) })
}

// ----- doc.object.resolve ---------------------------------------------------------------------------------------

/// `doc.object.resolve` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveParams {
    /// References (ids, compound ids, paths, special references).
    #[schemars(length(min = 1, max = 1000))]
    refs: Vec<String>,
}

/// `doc.object.resolve` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ResolveResult {
    /// One item per reference: `{ref, id, type, name, path, rev}` or `{ref, error}`.
    items: Vec<Map<String, Value>>,
}

#[command(
    id = "doc.object.resolve",
    kind = query,
    label = "Resolve references",
    scopes = [read],
    needs_project,
    example(title = "Resolve a path and the current element", params = r#"{"refs": ["scene:#1", "@current"]}"#),
    agent_only = "agents translate paths and IDs (10 AG-03)",
)]
pub(crate) fn doc_object_resolve(cx: &mut Cx, p: ResolveParams) -> Result<ResolveResult, CommandError> {
    let doc = cx.document()?.clone();
    let mut items = Vec::with_capacity(p.refs.len());
    for r in &p.refs {
        let mut m = Map::new();
        m.insert("ref".into(), json!(r));
        match cx.resolve(r) {
            Ok(Target::Object { ty, key }) => {
                let name = if ty.has_name() { doc.object_raw_json(&key)?.1.get("name").cloned() } else { None };
                m.insert("id".into(), json!(key));
                m.insert("type".into(), json!(ty.as_str()));
                m.insert("name".into(), name.unwrap_or(Value::Null));
                m.insert("path".into(), json!(doc.canonical_path(&key)));
                m.insert("rev".into(), json!(doc.locate(&key).map_or(0, |l| l.rev)));
            }
            Ok(Target::Cell { element, frame }) => {
                m.insert("id".into(), json!(element.as_str()));
                m.insert("type".into(), json!("cell"));
                m.insert("frame".into(), json!(frame));
            }
            Err(e) => {
                m.insert("error".into(), serde_json::to_value(&e).unwrap_or(Value::Null));
            }
        }
        items.push(m);
    }
    Ok(ResolveResult { items })
}

// ----- doc.object.patch -----------------------------------------------------------------------------------------

/// RFC 6902 operation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OpKind {
    /// add.
    Add,
    /// remove.
    Remove,
    /// replace.
    Replace,
    /// move.
    Move,
    /// copy.
    Copy,
    /// test.
    Test,
}

impl OpKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Replace => "replace",
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Test => "test",
        }
    }
}

/// One RFC 6902 operation, pointers relative to the object.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpParam {
    /// Operation.
    op: OpKind,
    /// Target pointer.
    #[schemars(length(max = 65_536))]
    path: String,
    /// Value (`add`, `replace`, `test`).
    #[serde(default, deserialize_with = "some")]
    value: Option<Value>,
    /// Source pointer (`move`, `copy`).
    #[serde(default)]
    from: Option<String>,
}

/// `expectRev`: one revision for every target, or per reference.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum ExpectRev {
    /// Applies to every target.
    All(u64),
    /// Reference → revision.
    Each(BTreeMap<String, u64>),
}

/// `doc.object.patch` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PatchParams {
    /// Objects to patch (ids, paths, `@selection`).
    #[schemars(length(min = 1, max = 1000))]
    targets: Vec<String>,
    /// JSON merge patch (RFC 7386) applied to every target.
    #[serde(default)]
    props: Option<Map<String, Value>>,
    /// JSON Patch operations (RFC 6902), pointers relative to each target.
    #[serde(default)]
    #[schemars(length(max = 10_000))]
    ops: Option<Vec<OpParam>>,
    /// Optimistic concurrency (REV-05).
    #[serde(default)]
    expect_rev: Option<ExpectRev>,
    /// Undo label (default "Set <property>").
    #[serde(default)]
    #[schemars(length(min = 1, max = 200))]
    label: Option<String>,
    /// Report the changes without applying them (the executor reads it before the handler runs).
    #[serde(default)]
    #[allow(dead_code, reason = "read by the executor from the raw parameters")]
    dry_run: bool,
}

/// `doc.object.patch` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PatchResult {
    /// The changes (also in the envelope).
    changes: Vec<ChangeRecord>,
    /// `documentRev` after the patch (the revision it would get for a dry run).
    document_rev: u64,
}

fn without_nulls(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            Value::Object(m.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), without_nulls(x))).collect())
        }
        other => other.clone(),
    }
}

/// RFC 7386 as RFC 6902 operations against the object's current JSON.
fn merge_ops(target: &Value, patch: &Map<String, Value>, at: &str, out: &mut Vec<PatchOp>) {
    for (k, v) in patch {
        let path = format!("{at}/{}", tt_model::pointer::escape(k));
        let cur = target.get(k);
        let op = |op: &'static str, value: Option<Value>| PatchOp {
            op,
            from: None,
            path: path.clone(),
            range: None,
            value,
            old_value: None,
            elided: false,
        };
        match (v, cur) {
            (Value::Null, Some(_)) => out.push(op("remove", None)),
            (Value::Null, None) => {}
            (Value::Object(m), Some(c @ Value::Object(_))) => merge_ops(c, m, &path, out),
            (v, Some(_)) => out.push(op("replace", Some(without_nulls(v)))),
            (v, None) => out.push(op("add", Some(without_nulls(v)))),
        }
    }
}

fn flag(s: &Value, k: &str) -> bool {
    s.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// The first `x-command` inside a schema subtree (following `$ref`s).
fn nested_command(schema: &Value, depth: usize, seen: &mut BTreeSet<String>) -> Option<String> {
    if depth > 24 {
        return None;
    }
    match schema {
        Value::Object(m) => {
            if let Some(c) = m.get("x-command").and_then(Value::as_str) {
                return Some(c.to_owned());
            }
            if let Some(r) = m.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/"))
                && seen.insert(r.to_owned())
                && let Some(d) = tt_model::schema::definitions().get(r)
                && let Some(c) = nested_command(d, depth + 1, seen)
            {
                return Some(c);
            }
            m.iter().filter(|(k, _)| k.as_str() != "$ref").find_map(|(_, v)| nested_command(v, depth + 1, seen))
        }
        Value::Array(a) => a.iter().find_map(|v| nested_command(v, depth + 1, seen)),
        _ => None,
    }
}

/// The area command owning the child collections of a type (for `doc.structuralChange`).
fn structural_command(ty: ObjectType, slot: &str) -> &'static str {
    match (ty, slot) {
        (ObjectType::Drawing | ObjectType::DrawingNode, _) => "draw.* (02)",
        (ObjectType::Element, "channels") => "anim.* (05)",
        (ObjectType::Element, _) => "element.* (04) / sound.* (07) / fx.* (06)",
        (ObjectType::Scene, _) => "element.create (04)",
        (ObjectType::Palette, _) => "colour.* (03)",
        (ObjectType::DrawingBank, _) => "element.drawing.* (04)",
        (ObjectType::Channel, _) => "anim.key.* (05)",
        (ObjectType::Library | ObjectType::LibraryFolder, _) => "library.* (08)",
        (ObjectType::Project, _) => "storyline.* / colour.* / draw.pen.* (08, 03, 02)",
        _ => "the owning area's commands",
    }
}

/// AG-02 classification of one operation pointer of a `ty` object (ADR-0013 §2).
fn classify(ty: ObjectType, pointer: &str, at: &str, test: bool) -> Result<(), CommandError> {
    if pointer.is_empty() {
        return Err(CommandError::with_details(
            "doc.structuralChange",
            "replacing or removing a whole object is a structural change",
            json!({ "pointer": at, "command": structural_command(ty, ""), "reason": "wholeObject" }),
        )
        .hint("patch individual properties (props or ops on /<property>)"));
    }
    let tokens = tt_model::pointer::parse(pointer)?;
    let Some(first) = tokens.first() else { return Ok(()) };
    if slots_of(ty).iter().any(|(f, _)| f == first) {
        if test {
            return Ok(());
        }
        return Err(CommandError::with_details(
            "doc.structuralChange",
            format!("`{first}` is a child collection of a {ty}"),
            json!({ "pointer": at, "property": first, "command": structural_command(ty, first) }),
        ));
    }
    let mut prefix = String::new();
    let mut last: Option<Value> = None;
    for (i, t) in tokens.iter().enumerate() {
        prefix.push('/');
        prefix.push_str(&tt_model::pointer::escape(t));
        let Some(s) = tt_model::schema::property_schema(ty, &prefix) else {
            if i == 0 {
                return Err(CommandError::with_details(
                    "doc.schemaViolation",
                    format!("a {ty} has no property `{t}`"),
                    json!({ "pointer": at, "property": t, "reason": "unknownProperty" }),
                ));
            }
            break;
        };
        if test {
            return Ok(());
        }
        if flag(&s, "readOnly") || flag(&s, "x-derived") || flag(&s, "x-transient") {
            return Err(CommandError::with_details(
                "doc.readOnlyProperty",
                format!("`{prefix}` of a {ty} is read-only, derived or transient"),
                json!({ "pointer": at, "property": prefix }),
            ));
        }
        if let Some(c) = s.get("x-command").and_then(Value::as_str) {
            return Err(x_command(c, &prefix, at));
        }
        last = Some(s);
    }
    if let Some(s) = last
        && let Some(c) = nested_command(&s, 0, &mut BTreeSet::new())
    {
        return Err(x_command(&c, pointer, at));
    }
    Ok(())
}

/// A property with `x-command` follows that command's semantics (AG-02); until the command is implemented the patch
/// is refused with the command and its owner named (ADR-0013 §2).
fn x_command(command: &str, property: &str, at: &str) -> CommandError {
    let owner = tt_registry::owner_of(command);
    CommandError::with_details(
        codes::COMMAND_NOT_IMPLEMENTED,
        format!("`{property}` is changed with the semantics of `{command}`, which is implemented in {owner}"),
        json!({ "commandId": command, "milestone": owner, "property": property, "pointer": at }),
    )
    .hint(format!("use `{command}` once available"))
}

/// The drawing holding a drawing-local object, or the drawing itself.
fn drawing_of(doc: &Document, ty: ObjectType, key: &str) -> Option<Id> {
    match ty {
        ObjectType::Drawing => Id::parse(key).ok(),
        ObjectType::DrawingNode | ObjectType::Stroke => key.split_once('/').and_then(|(d, _)| Id::parse(d).ok()),
        _ => None,
    }
    .filter(|d| doc.drawing(d).is_some())
}

#[command(
    id = "doc.object.patch",
    kind = action,
    label = "Set properties",
    scopes = [edit],
    undo = "Set properties",
    needs_project,
    example(title = "Annotate a scene in meta", params = r#"{"targets": ["scene:#1"], "props": {"meta": {"status": "layout"}}}"#),
    example(title = "Mark the first drawing as mouth shape A", params = r#"{"targets": ["drawingBank:#1/drawing:#1"], "ops": [{"op": "replace", "path": "/mouthShape", "value": "A"}], "label": "Set mouth shape"}"#),
    example(title = "Set a pen's persisted tip fields (persisted names `tip` and `tipAngle`, SI-004)", params = r#"{"targets": ["pen:#1"], "props": {"tip": "square", "tipAngle": 30}, "dryRun": true}"#),
    example(title = "Persisted element role (superset enum, SI-003)", params = r#"{"targets": ["scene:#1/element:#1"], "props": {"role": "foreground"}, "dryRun": true}"#),
    agent_only = "agents patch the object model (11 §7.4 set_properties); the GUI uses the area commands",
)]
pub(crate) fn doc_object_patch(cx: &mut Cx, p: PatchParams) -> Result<PatchResult, CommandError> {
    if p.props.is_none() && p.ops.is_none() {
        return Err(CommandError::with_details(
            codes::ARG_MISSING,
            "give `props` or `ops`",
            json!({ "pointer": "/props" }),
        ));
    }
    let targets = cx.resolve_objects(&p.targets)?;
    let doc = cx.document()?.clone();
    // REV-05 (C12-R1-07): an integer guards every target; a map guards every object it lists, targets or not.
    if let Some(expect) = &p.expect_rev {
        let pairs: Vec<(String, u64)> = match expect {
            ExpectRev::All(r) => targets.iter().map(|(_, k)| (k.clone(), *r)).collect(),
            ExpectRev::Each(m) => m.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        };
        crate::run::check_expect_rev(cx, &pairs)?;
    }
    let mut edits: Vec<(String, Vec<PatchOp>)> = Vec::with_capacity(targets.len());
    let mut first_property: Option<String> = None;
    for (ty, key) in &targets {
        if let Some(d) = drawing_of(&doc, *ty, key)
            && doc.drawing(&d).is_some_and(|x| *x.locked())
        {
            return Err(CommandError::with_details(
                "drawing.locked",
                format!("drawing `{d}` is locked"),
                json!({ "drawingId": d.as_str(), "id": key }),
            ));
        }
        let mut ops: Vec<PatchOp> = Vec::new();
        if let Some(props) = &p.props {
            let (_, current) = doc.object_raw_json(key)?;
            merge_ops(&current, props, "", &mut ops);
        }
        for o in p.ops.iter().flatten() {
            ops.push(PatchOp {
                op: o.op.as_str(),
                from: o.from.clone(),
                path: o.path.clone(),
                range: None,
                value: o.value.clone(),
                old_value: None,
                elided: false,
            });
        }
        for (i, op) in ops.iter().enumerate() {
            let at = format!("/ops/{i}");
            if matches!(op.op, "add" | "replace" | "test") && op.value.is_none() {
                return Err(CommandError::with_details(
                    codes::ARG_MISSING,
                    format!("`{}` needs a value", op.op),
                    json!({ "pointer": format!("{at}/value") }),
                ));
            }
            if matches!(op.op, "move" | "copy") && op.from.is_none() {
                return Err(CommandError::with_details(
                    codes::ARG_MISSING,
                    format!("`{}` needs `from`", op.op),
                    json!({ "pointer": format!("{at}/from") }),
                ));
            }
            classify(*ty, &op.path, &at, op.op == "test")?;
            if let Some(f) = &op.from {
                classify(*ty, f, &format!("{at}/from"), op.op == "copy")?;
            }
            if first_property.is_none() && op.op != "test" {
                first_property = tt_model::pointer::parse(&op.path).ok().and_then(|t| t.into_iter().next());
            }
        }
        edits.push((key.clone(), ops));
    }
    let label = p.label.clone().unwrap_or_else(|| match &first_property {
        Some(f) => format!("Set {f}"),
        None => "Set properties".to_owned(),
    });
    cx.set_label(label);
    cx.patch_objects(edits)?;
    // 02 §13.6: patched strokes go through the same validation as `draw.stroke.set` (a brush outline is swept again
    // from an edited spine; an outline edited directly discards the spine).
    let strokes: Vec<String> =
        targets.iter().filter(|(t, _)| *t == ObjectType::Stroke).map(|(_, k)| k.clone()).collect();
    super::draw_stroke::normalise_patched_strokes(cx, &doc, &strokes)?;
    let (changes, rev) = cx.preview_changes()?;
    cx.set_targets(json!({ "ids": targets.iter().map(|(_, k)| k).collect::<Vec<_>>() }));
    let document_rev = if changes.is_empty() { doc.document_rev() } else { rev };
    Ok(PatchResult { changes, document_rev })
}

/// Stages a property merge onto elements (pasting element properties, `edit.paste`): the rules of
/// `doc.object.patch` `props`. Returns the modified element keys.
pub(crate) fn stage_props(
    cx: &mut Cx,
    refs: &[String],
    props: &Map<String, Value>,
) -> Result<Vec<String>, CommandError> {
    let targets = cx.resolve_objects(refs)?;
    let doc = cx.document()?.clone();
    let mut edits = Vec::with_capacity(targets.len());
    for (ty, key) in &targets {
        if *ty != ObjectType::Element {
            return Err(CommandError::with_details(
                tt_model::codes::REF_WRONG_TYPE,
                format!("element properties paste onto elements, not a {ty}"),
                json!({ "id": key }),
            ));
        }
        let (_, current) = doc.object_raw_json(key)?;
        let mut ops = Vec::new();
        merge_ops(&current, props, "", &mut ops);
        for (i, op) in ops.iter().enumerate() {
            classify(*ty, &op.path, &format!("/props/{i}"), false)?;
        }
        edits.push((key.clone(), ops));
    }
    cx.patch_objects(edits)?;
    Ok(targets.into_iter().map(|(_, k)| k).collect())
}

// ----- schemas ----------------------------------------------------------------------------------------------------

/// One type in `doc.schema.list`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeInfo {
    /// Type name.
    #[serde(rename = "type")]
    ty: String,
    /// Path aliases.
    aliases: Vec<String>,
    /// Possible parent types.
    parents: Vec<String>,
    /// Addressable by quoted name in paths.
    name_addressable: bool,
    /// Description.
    description: String,
}

/// `doc.schema.list` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaListResult {
    /// Format version.
    format_version: String,
    /// Types.
    types: Vec<TypeInfo>,
}

#[command(
    id = "doc.schema.list",
    kind = query,
    label = "List object types",
    scopes = [read],
    example(title = "Every object type with its parents and addressing", params = "{}"),
    agent_only = "schema discovery for agents (10 §2.8)",
)]
pub(crate) fn doc_schema_list(_cx: &mut Cx, _p: NoParams) -> Result<SchemaListResult, CommandError> {
    Ok(SchemaListResult {
        format_version: tt_model::FORMAT_VERSION.to_owned(),
        types: ObjectType::ALL
            .iter()
            .map(|t| TypeInfo {
                ty: t.as_str().to_owned(),
                aliases: t.aliases().iter().map(|a| (*a).to_owned()).collect(),
                parents: t.parents().iter().map(|p| p.as_str().to_owned()).collect(),
                name_addressable: t.name_addressable(),
                description: t.description().to_owned(),
            })
            .collect(),
    })
}

/// `doc.schema.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaGetParams {
    /// Object type (or alias).
    #[serde(rename = "type")]
    ty: String,
    /// JSON Pointer of one property.
    #[serde(default)]
    property: Option<String>,
}

/// `doc.schema.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaGetResult {
    /// Type name.
    #[serde(rename = "type")]
    ty: String,
    /// Format version.
    format_version: String,
    /// The JSON Schema (draft 2020-12).
    schema: Map<String, Value>,
}

#[command(
    id = "doc.schema.get",
    kind = query,
    label = "Get schema",
    scopes = [read],
    example(title = "The element schema", params = r#"{"type": "element"}"#),
    example(title = "The schema of a pen's tip", params = r#"{"type": "pen", "property": "/tip"}"#),
    agent_only = "schema discovery for agents (10 §2.8)",
)]
pub(crate) fn doc_schema_get(_cx: &mut Cx, p: SchemaGetParams) -> Result<SchemaGetResult, CommandError> {
    let ty = parse_type(&p.ty, "/type")?;
    let schema = match &p.property {
        None => tt_model::schema::object_schema(ty),
        Some(ptr) => tt_model::schema::property_schema(ty, ptr)
            .ok_or_else(|| invalid("/property", format!("a {ty} has no property at `{ptr}`")))?,
    };
    let Value::Object(schema) = schema else { return Err(CommandError::internal("schema is not an object")) };
    Ok(SchemaGetResult { ty: ty.as_str().to_owned(), format_version: tt_model::FORMAT_VERSION.to_owned(), schema })
}

// ----- state and diff ---------------------------------------------------------------------------------------------

/// `doc.state.get` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateResult {
    /// Project id.
    project_id: String,
    /// Package path (`null` until saved).
    path: Option<String>,
    /// `package` or `folder` (`null` until saved).
    form: Option<String>,
    /// Format version.
    format_version: String,
    /// Version migrated from on load (C1.3).
    migrated_from: Option<String>,
    /// Current revision.
    document_rev: u64,
    /// Revision at the last save.
    saved_rev: Option<u64>,
    /// Modified since the last save.
    dirty: bool,
    /// Read-only.
    read_only: bool,
    /// The lock (10 §8 LOCK-01): `{held: true, …our lock}` while this session holds it, `{held: false, …holder}` when
    /// the project opened read-only because someone else holds it, `null` otherwise.
    lock: Option<Map<String, Value>>,
    /// Oldest revision `doc.diff` accepts.
    history_from_rev: u64,
}

#[command(
    id = "doc.state.get",
    kind = query,
    label = "Document state",
    scopes = [read],
    needs_project,
    example(title = "Revision, dirty flag and diff horizon", params = "{}"),
    agent_only = "agents track revisions (10 AG-09)",
)]
pub(crate) fn doc_state_get(cx: &mut Cx, _p: NoParams) -> Result<StateResult, CommandError> {
    let p = cx.project()?;
    Ok(StateResult {
        project_id: p.doc.project().id().as_str().to_owned(),
        path: p.path.clone(),
        form: p.form.clone(),
        format_version: p.store.format_version.clone(),
        migrated_from: p.store.migrated_from.clone(),
        document_rev: p.doc.document_rev(),
        saved_rev: p.saved_rev,
        dirty: p.dirty(),
        read_only: p.read_only,
        lock: match (&p.store.lock, &p.store.locked_by) {
            (Some(l), _) => serde_json::to_value(l.info()).ok().and_then(|v| match v {
                Value::Object(mut m) => {
                    m.insert("held".into(), Value::Bool(true));
                    m.insert("path".into(), json!(l.path().to_string_lossy()));
                    Some(m)
                }
                _ => None,
            }),
            (None, Some(Value::Object(m))) => {
                let mut m = m.clone();
                m.insert("held".into(), Value::Bool(false));
                Some(m)
            }
            _ => None,
        },
        history_from_rev: p.history_from_rev(),
    })
}

/// Detail of `doc.diff`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Detail {
    /// Without patches.
    Summary,
    /// With patches.
    #[default]
    Full,
}

/// `doc.diff` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DiffParams {
    /// Base revision.
    #[serde(default)]
    since_rev: Option<u64>,
    /// Base checkpoint (by name).
    #[serde(default)]
    since_checkpoint: Option<String>,
    /// End revision (default: current).
    #[serde(default)]
    until_rev: Option<u64>,
    /// Only objects inside this one.
    #[serde(default)]
    within: Option<String>,
    /// Only these types.
    #[serde(default)]
    types: Option<Vec<String>>,
    /// With or without patches.
    #[serde(default)]
    detail: Detail,
    /// Maximum changes.
    #[serde(default = "thousand")]
    #[schemars(range(min = 1, max = 100_000))]
    limit: usize,
}

/// `doc.diff` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffResult {
    /// Base revision.
    from_rev: u64,
    /// End revision.
    to_rev: u64,
    /// Net changes.
    changes: Vec<ChangeRecord>,
    /// More changes than `limit`.
    truncated: bool,
}

/// The document at `rev`, rebuilt from the revision log.
fn doc_at(cx: &Cx, rev: u64) -> Result<Document, CommandError> {
    let p = cx.project()?;
    let current = p.doc.document_rev();
    if rev == current {
        return Ok(p.doc.clone());
    }
    let oldest = p.history_from_rev();
    if rev > current || rev < oldest {
        return Err(CommandError::with_details(
            "doc.diffUnavailable",
            format!("revision {rev} is outside the retained history ({oldest}–{current})"),
            json!({ "oldestRev": oldest, "documentRev": current }),
        )
        .hint("re-read the objects instead"));
    }
    let mut doc = p.doc.clone();
    for rec in p.log.iter().rev() {
        if rec.rev <= rev {
            break;
        }
        doc = match (&rec.before, &rec.back) {
            (Some(before), _) => before.clone(),
            (None, Back::Snapshot(d)) => d.clone(),
            (None, Back::Patches(patches)) => {
                // As an undo replay does (C12-R1-08): merge the patches per entry in order, check each merged patch
                // against the entry it applies to, then restore the revisions the transaction stamped.
                let mut merged: Vec<(tt_model::document::EntryKey, Vec<PatchOp>)> = Vec::new();
                for (k, ps) in patches {
                    match merged.iter_mut().find(|(e, _)| e == k) {
                        Some((_, ops)) => ops.extend(ps.iter().cloned()),
                        None => merged.push((k.clone(), ps.clone())),
                    }
                }
                let mut ready = Vec::with_capacity(merged.len());
                for (k, ops) in merged {
                    let checked = crate::align::prepare(&doc, &k, &ops)?;
                    ready.push((k, checked));
                }
                let content = doc.with_patched_entries_undo_only(&ready).map_err(crate::cx::model_error)?.0;
                content.at_earlier_revision(rec.rev.saturating_sub(1), &rec.earlier_revs)?
            }
        };
    }
    Ok(doc)
}

#[command(
    id = "doc.diff",
    kind = query,
    label = "Changes since",
    scopes = [read],
    needs_project,
    example(title = "Everything since the project was opened", params = r#"{"sinceRev": 1, "detail": "summary"}"#),
    agent_only = "agents catch up after reconnecting (10 AG-09)",
)]
pub(crate) fn doc_diff(cx: &mut Cx, p: DiffParams) -> Result<DiffResult, CommandError> {
    // Inside an atomic batch the latest state is the transaction's staged one, with the revision it will commit at
    // (11 §7.3: queries see earlier uncommitted steps; C12-R1-09).
    let live = cx.project()?.doc.document_rev();
    let predicted = cx.predicted_rev()?;
    let staged = predicted > live;
    let state_at = |cx: &Cx, rev: u64| -> Result<Document, CommandError> {
        if staged && rev == predicted { Ok(cx.document()?.clone()) } else { doc_at(cx, rev) }
    };
    let until = p.until_rev.unwrap_or(predicted);
    let (from_rev, from_doc) = match (&p.since_rev, &p.since_checkpoint) {
        (Some(r), None) => (*r, state_at(cx, *r)?),
        (None, Some(name)) => {
            let c = cx.project()?.checkpoints.iter().find(|c| &c.name == name).cloned().ok_or_else(|| {
                CommandError::with_details(
                    "checkpoint.notFound",
                    format!("no checkpoint `{name}`"),
                    json!({ "name": name }),
                )
            })?;
            (c.document_rev, c.doc)
        }
        (None, None) => {
            return Err(CommandError::with_details(
                codes::ARG_MISSING,
                "give `sinceRev` or `sinceCheckpoint`",
                json!({ "pointer": "/sinceRev" }),
            ));
        }
        (Some(_), Some(_)) => return Err(invalid("/sinceCheckpoint", "give either `sinceRev` or `sinceCheckpoint`")),
    };
    if until < from_rev {
        return Err(invalid("/untilRev", "untilRev is before the base revision"));
    }
    let until_doc = state_at(cx, until)?;
    let staged_view = staged && until == predicted;
    let within = match &p.within {
        Some(w) => match cx.resolve(w)? {
            Target::Object { key, .. } => Some(key),
            Target::Cell { .. } => return Err(invalid("/within", "`within` names an object")),
        },
        None => None,
    };
    let types: Option<Vec<ObjectType>> = p
        .types
        .as_ref()
        .map(|ts| ts.iter().enumerate().map(|(i, t)| parse_type(t, &format!("/types/{i}"))).collect())
        .transpose()?;
    let mut changes = Vec::new();
    // Drawings holding a changed object, collected before any filter applies (C12-R3-04).
    let mut containers: Vec<String> = Vec::new();
    for (c, o) in change::diff(&from_doc, &until_doc)? {
        if let Some((d, _)) = o.key.split_once('/') {
            containers.push(d.to_owned());
        }
        if types.as_ref().is_some_and(|ts| !ts.contains(&o.ty)) {
            continue;
        }
        if let Some(w) = &within {
            let side = if matches!(c, OwnChange::Deleted) { &from_doc } else { &until_doc };
            if &o.key != w && !is_within(side, &o.key, w) {
                continue;
            }
        }
        let rev = if staged_view { until } else { until_doc.locate(&o.key).map_or(until, |l| l.rev) };
        let patch = match (&c, p.detail) {
            (OwnChange::Modified(ops), Detail::Full) => {
                let mut v = Vec::with_capacity(ops.len());
                for op in ops {
                    let mut j = serde_json::to_value(op).map_err(|e| CommandError::internal(e.to_string()))?;
                    if let Some(m) = j.as_object_mut() {
                        let big = |x: Option<&Value>| x.is_some_and(|x| x.to_string().len() > change::ELIDE_BYTES);
                        if big(m.get("value")) || big(m.get("oldValue")) {
                            m.shift_remove("value");
                            m.shift_remove("oldValue");
                            m.insert("elided".into(), json!(true));
                        }
                    }
                    v.push(j);
                }
                Some(v)
            }
            _ => None,
        };
        let (op, parent_id, index) = match &c {
            OwnChange::Created => ("created", o.parent.clone(), Some(o.index)),
            OwnChange::Deleted => ("deleted", o.parent.clone(), None),
            OwnChange::Modified(_) => ("modified", None, None),
        };
        changes.push(ChangeRecord {
            id: o.key.clone(),
            ty: o.ty.as_str().to_owned(),
            op: op.to_owned(),
            rev,
            parent_id,
            index,
            patch,
        });
    }
    // REV-02 invalidation (C12-R2-12): a drawing whose content changed carries a new revision even when its own
    // properties did not change; report it as the command envelope and `doc.changed` do. Content, not stamps, decides
    // (C12-R4-01): the drawings reported are the containers of the drawing-local objects whose own views differ
    // between the two states, so revision stamps (a staged checkpoint's predicted ones, a replay's new ones) never
    // invalidate unchanged content.
    let wanted = |key: &str| {
        types.as_ref().is_none_or(|ts| ts.contains(&ObjectType::Drawing))
            && within.as_ref().is_none_or(|w| key == w || is_within(&until_doc, key, w))
    };
    let mut invalidated: Vec<String> =
        containers.into_iter().filter(|d| until_doc.locate(d).is_some_and(|l| l.ty == ObjectType::Drawing)).collect();
    invalidated.sort();
    invalidated.dedup();
    for d in invalidated {
        if changes.iter().any(|c| c.id == d) || !wanted(&d) {
            continue;
        }
        let rev = if staged_view { until } else { until_doc.locate(&d).map_or(until, |l| l.rev) };
        changes.push(ChangeRecord {
            id: d,
            ty: ObjectType::Drawing.as_str().to_owned(),
            op: "modified".to_owned(),
            rev,
            parent_id: None,
            index: None,
            patch: matches!(p.detail, Detail::Full).then(Vec::new),
        });
    }
    let truncated = changes.len() > p.limit;
    changes.truncate(p.limit);
    Ok(DiffResult { from_rev, to_rev: until, changes, truncated })
}

#[command(
    id = "doc.changed",
    kind = event,
    label = "Document changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn doc_changed(_cx: &mut Cx, _p: NoParams) -> Result<ChangedEvent, CommandError> {
    Err(event_not_invocable("doc.changed"))
}

/// `doc.changed` payload (10 §2.6).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedEvent {
    /// The step created or replayed (`null` for a system change without an undo step, e.g. the `modifiedAt` stamp of
    /// a save).
    undo_step_id: Option<String>,
    /// The transaction's revision.
    document_rev: u64,
    /// Label.
    label: String,
    /// `user`, `agent:<client>`, `system`, `load`.
    origin: String,
    /// Changes (values over 64 KiB elided).
    changes: Vec<ChangeRecord>,
}
