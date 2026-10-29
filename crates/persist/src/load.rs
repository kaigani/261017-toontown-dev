//! The load sequence (10 §7.1): container, manifest and versions, integrity, migration, parse and validate, resolve,
//! repair, report. The result is a **candidate** document with its load report; installing it is the executor's.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};
use tt_model::document::{Document, EntryKey};
use tt_model::overlay::Overlay;
use tt_model::{IdGenerator, ModelError};

use crate::codes;
use crate::container::{Blobs, Container, Form};
use crate::layout::{self, Kind};
use crate::manifest::{Manifest, PACKAGE_MIMETYPE, Version, VersionRelation};
use crate::migrate::{self, Entries, Migration};
use crate::repair::{self, FrozenEntry, Work};
use crate::report::{Issues, LoadReport};
use crate::zip::{Limits, invalid};

/// Known required features (VER-02): none in 1.0.
pub const KNOWN_FEATURES: &[&str] = &[];

/// Default bound of one JSON entry (ZIP-06 global limit for object entries).
pub const JSON_LIMIT: u64 = 1 << 30;
/// Margin over a manifest size before decompression stops (ZIP-06).
pub const SIZE_MARGIN: u64 = 64 * 1024;

/// Load options.
pub struct LoadOptions<'a> {
    /// Open read-only (an interrupted folder save is then reported, not completed).
    pub read_only: bool,
    /// Container limits.
    pub limits: Limits,
    /// Bound of one JSON entry.
    pub json_limit: u64,
    /// Migration chain.
    pub migrations: &'a [Migration],
    /// Progress (fraction, step text); returns `false` to cancel.
    pub progress: &'a (dyn Fn(f64, &str) -> bool + Sync),
}

impl Default for LoadOptions<'_> {
    fn default() -> Self {
        Self {
            read_only: false,
            limits: Limits::default(),
            json_limit: JSON_LIMIT,
            migrations: migrate::CHAIN,
            progress: &|_, _| true,
        }
    }
}

/// What the load keeps beside the document so a save writes it back (VER-02, ZIP-04).
#[derive(Debug, Clone, Default)]
pub struct Preserved {
    /// Unknown properties of newer-minor entries, by entry.
    pub overlays: BTreeMap<EntryKey, Overlay>,
    /// Read-only entries with unknown values, written back as read.
    pub frozen: BTreeMap<EntryKey, FrozenEntry>,
    /// Entries outside the reserved names (copied unchanged on save).
    pub unknown_entries: Vec<String>,
    /// The file's format version when newer than this reader's (written back so newer readers keep the meaning).
    pub newer_version: Option<(Version, Version)>,
    /// Unknown manifest properties.
    pub manifest_unknown: serde_json::Map<String, Value>,
}

/// A linked or embedded file the project references and that cannot be found (MED-03).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingMedia {
    /// Referencing object (asset, drawing, palette, library item).
    pub object_id: String,
    /// Entry path (embedded) or link path (linked).
    pub location: String,
    /// `embedded`, `linked`, `pixels`, `texture`, `libraryFile`.
    pub kind: String,
}

/// A loaded project.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The candidate document.
    pub doc: Document,
    /// The load report.
    pub report: LoadReport,
    /// Binary entries.
    pub blobs: Blobs,
    /// Preserved data.
    pub preserved: Preserved,
    /// The version migrated from.
    pub migrated_from: Option<Version>,
    /// Whether repairs changed the content (REP-01).
    pub repaired: bool,
    /// `documentRev` of the file (the persisted `savedRev`).
    pub saved_rev: u64,
    /// The file's save time (manifest).
    pub saved_at: Option<String>,
    /// `session.json`, when valid.
    pub session: Option<Value>,
    /// The form.
    pub form: Form,
    /// The path.
    pub path: PathBuf,
    /// Missing media.
    pub missing: Vec<MissingMedia>,
    /// Resolved locations of linked assets (asset ID → absolute path).
    pub linked: BTreeMap<String, PathBuf>,
}

fn cancelled() -> ModelError {
    ModelError::with_details("job.cancelled", "the open was cancelled", json!({}))
}

fn step(opts: &LoadOptions<'_>, f: f64, text: &str) -> Result<(), ModelError> {
    if (opts.progress)(f, text) { Ok(()) } else { Err(cancelled()) }
}

/// Opens a project package or folder and returns the repaired candidate (10 §7.1).
///
/// # Errors
/// The fatal problems of 10 §7.2 (`project.notFound`, `project.permissionDenied`, `project.invalidFormat`,
/// `project.unsupportedVersion`) and `job.cancelled` when the progress callback cancels.
#[allow(clippy::too_many_lines)]
pub fn load(path: &Path, opts: &LoadOptions<'_>) -> Result<Loaded, ModelError> {
    let started = Instant::now();
    let mut issues = Issues::default();
    // 1. Container.
    step(opts, 0.0, "Container")?;
    if path.is_dir() {
        crate::save::recover_journal(path, opts.read_only, &mut issues)?;
    }
    let container = Container::open(path, PACKAGE_MIMETYPE, opts.limits)?;
    let form = container.form();
    // 2. Manifest.
    step(opts, 0.05, "Manifest")?;
    let manifest = if container.contains(layout::MANIFEST) {
        Some(Manifest::parse(&container.read(layout::MANIFEST, 64 << 20)?)?)
    } else if form == Form::Package {
        return Err(invalid("noManifest", "the package has no manifest.json", Some(layout::MANIFEST)));
    } else {
        issues.warning(
            codes::HASH_MISMATCH,
            None,
            Some(layout::MANIFEST),
            "the folder has no manifest.json; format 1.0 is assumed",
            json!({ "reason": "noManifest" }),
        );
        None
    };
    let relation = match &manifest {
        Some(m) => m.check("animpkg", KNOWN_FEATURES)?,
        None => VersionRelation::Same,
    };
    let file_version = manifest.as_ref().map_or_else(Version::current, |m| m.format_version);
    let names = container.names();
    let mut preserved = Preserved::default();
    for n in &names {
        if layout::classify(n) == Kind::Unknown {
            preserved.unknown_entries.push(n.clone());
        }
    }
    if let VersionRelation::NewerMinor(v) = relation {
        let min = manifest.as_ref().map_or(v, |m| m.min_reader_version);
        preserved.newer_version = Some((v, min));
        issues.warning(
            codes::NEWER_MINOR,
            None,
            None,
            format!("written by format {v}, newer than {}; unknown content is preserved", Version::current()),
            json!({ "formatVersion": v.to_string(), "readerVersion": Version::current().to_string() }),
        );
    }
    if let Some(m) = &manifest {
        preserved.manifest_unknown = m.unknown.clone();
    }
    // Integrity (package form): sizes and hashes of the listed entries.
    let listed: BTreeMap<String, (u64, String)> = manifest
        .as_ref()
        .and_then(|m| m.entries.as_ref())
        .map(|es| es.iter().map(|e| (e.path.clone(), (e.size, e.sha256.clone()))).collect())
        .unwrap_or_default();
    if form == Form::Package {
        let total = names.len().max(1);
        for (i, n) in names.iter().enumerate() {
            if i % 64 == 0 {
                #[allow(clippy::cast_precision_loss, reason = "progress fraction")]
                step(opts, 0.05 + 0.25 * (i as f64 / total as f64), "Integrity")?;
            }
            let kind = layout::classify(n);
            if !kind.listed() {
                continue;
            }
            let Some((size, sha)) = listed.get(n) else {
                issues.warning(
                    codes::HASH_MISMATCH,
                    None,
                    Some(n),
                    format!("entry `{n}` is not listed in the manifest"),
                    json!({ "reason": "unlisted" }),
                );
                continue;
            };
            let limit = size.saturating_add(SIZE_MARGIN);
            let actual = container
                .reader(n, limit)
                .and_then(|r| crate::fsx::sha256_reader(r).map_err(|e| crate::zip::entry_error(n, &e)));
            match actual {
                Ok((h, len)) if h == *sha && len == *size => {}
                Ok((h, len)) => issues.warning(
                    codes::HASH_MISMATCH,
                    None,
                    Some(n),
                    format!("entry `{n}` differs from its manifest record"),
                    json!({ "expectedSize": size, "size": len, "expectedSha256": sha, "sha256": h }),
                ),
                Err(e) if e.details().get("reason").and_then(Value::as_str) == Some("sizeLimit") => return Err(e),
                Err(e) => issues.warning(
                    codes::HASH_MISMATCH,
                    None,
                    Some(n),
                    format!("entry `{n}` cannot be read: {}", e.message()),
                    json!({ "reason": e.details().get("reason") }),
                ),
            }
        }
        for n in listed.keys() {
            if !container.contains(n) {
                issues.warning(
                    codes::HASH_MISMATCH,
                    None,
                    Some(n),
                    format!("entry `{n}` is listed in the manifest but missing"),
                    json!({ "reason": "missing" }),
                );
            }
        }
    }
    // 3–4. Read object entries, migrate, parse.
    step(opts, 0.3, "Properties")?;
    let mut entries: Entries = BTreeMap::new();
    let mut unreadable: BTreeMap<String, Value> = BTreeMap::new();
    for n in &names {
        let kind = layout::classify(n);
        if !kind.is_object() {
            continue;
        }
        let limit = listed.get(n).map_or(opts.json_limit, |(s, _)| s.saturating_add(SIZE_MARGIN).min(opts.json_limit));
        let bytes = match container.read(n, limit) {
            Ok(b) => b,
            Err(e) if e.details().get("reason").and_then(Value::as_str) == Some("sizeLimit") => return Err(e),
            Err(e) => {
                unreadable.insert(n.clone(), json!({ "unreadable": e.message() }));
                continue;
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                unreadable.insert(n.clone(), json!(String::from_utf8_lossy(e.as_bytes())));
                continue;
            }
        };
        match tt_model::json_text::parse::<Value>(&text) {
            Ok(v) => {
                entries.insert(n.clone(), v);
            }
            Err(_) => {
                unreadable.insert(n.clone(), json!(text));
            }
        }
    }
    let migrated_from = match relation {
        VersionRelation::Older(v) => {
            step(opts, 0.35, "Migrate")?;
            entries = migrate::migrate(entries, v, Version::current(), opts.migrations)?;
            Some(v)
        }
        _ => None,
    };
    // The project entry is required.
    let Some(project_json) = entries.remove(layout::PROJECT) else {
        let reason = if unreadable.contains_key(layout::PROJECT) {
            "project.json is not valid JSON"
        } else {
            "project.json is missing"
        };
        return Err(ModelError::with_details(
            codes::INVALID_FORMAT,
            reason,
            json!({ "reason": "project", "entry": layout::PROJECT }),
        ));
    };
    if !project_json.is_object() || project_json.get("type").and_then(Value::as_str) != Some("project") {
        return Err(ModelError::with_details(
            codes::INVALID_FORMAT,
            "project.json does not hold a project object",
            json!({ "reason": "project", "entry": layout::PROJECT }),
        ));
    }
    // Deterministic IDs for repairs (LOAD-03): seeded with the project entry and the entry list.
    let seed = {
        let h = crate::fsx::sha256(format!("{}{:?}", project_json, names).as_bytes());
        u64::from_str_radix(h.get(..16).unwrap_or("0"), 16).unwrap_or(0)
    };
    let stamp =
        manifest.as_ref().and_then(|m| m.saved_at.clone()).filter(|s| tt_model::value::Timestamp::parse(s).is_ok());
    let mut work = Work {
        project: repair::item_for(&Kind::Project, layout::PROJECT, project_json)
            .ok_or_else(|| ModelError::new(codes::INTERNAL, "project item"))?,
        palettes: Vec::new(),
        banks: Vec::new(),
        drawings: Vec::new(),
        scenes: Vec::new(),
        elements: Vec::new(),
        library: None,
        issues,
        ids: IdGenerator::seeded(seed, 1_760_486_400_000),
        stamp: stamp.unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_owned()),
        newer_minor: matches!(relation, VersionRelation::NewerMinor(_)),
    };
    // Unreadable entries become items carrying their raw text (the type is that of the entry name).
    for (n, raw) in unreadable {
        let kind = layout::classify(&n);
        if let Some(mut item) = repair::item_for(&kind, &n, Value::Null) {
            item.value = json!({ "type": "unreadable", "raw": raw });
            file_item(&mut work, item);
        }
    }
    for (n, v) in entries {
        let kind = layout::classify(&n);
        let v = if kind == Kind::LibraryIndex { crate::library::from_animlib(v) } else { v };
        if let Some(item) = repair::item_for(&kind, &n, v) {
            file_item(&mut work, item);
        }
    }
    // Remember which entries were frozen by the user's raw JSON (unknown values) before validation changes them.
    step(opts, 0.45, "Palettes")?;
    work.validate_entries()?;
    // 5–6. Resolve and repair.
    step(opts, 0.6, "Scenes")?;
    work.entry_duplicates();
    work.membership();
    work.inline_duplicates();
    work.names();
    work.references();
    step(opts, 0.75, "Drawings")?;
    let file_rev = manifest.as_ref().and_then(|m| m.document_rev).unwrap_or(0).max(work.max_rev()).max(1);
    let mut doc_rev = file_rev;
    let mut doc = None;
    let mut repaired = false;
    for _ in 0..8 {
        repaired = work.any_touched();
        if repaired && file_rev == u64::MAX {
            // REV-03 / C13-R1-09: a repair is a new revision, and there is none left; never install it unstamped.
            return Err(ModelError::with_details(
                codes::INVALID_FORMAT,
                "the project needs repairs, but its document revision is exhausted (2^64 - 1)",
                json!({ "reason": "revisionExhausted", "documentRev": file_rev }),
            ));
        }
        doc_rev = if repaired { file_rev + 1 } else { file_rev };
        if repaired {
            work.stamp(doc_rev);
        }
        let parts = work.parts(doc_rev).map_err(|errors| fatal_parts(&errors))?;
        let assembled = Document::assemble(parts.clone());
        if assembled.issues().iter().all(|i| i.severity != tt_model::document::Severity::Error) {
            doc = Some(Document::from_parts(parts).map_err(|errors| fatal_parts(&errors))?);
            break;
        }
        if !work.drop_failing(&assembled) {
            let errors: Vec<ModelError> = assembled
                .issues()
                .into_iter()
                .filter(|i| i.severity == tt_model::document::Severity::Error)
                .map(|i| i.to_error())
                .collect();
            return Err(fatal_parts(&errors));
        }
        work.membership();
        work.references();
    }
    let Some(doc) = doc else {
        return Err(ModelError::with_details(
            codes::INVALID_FORMAT,
            "the project cannot be repaired",
            json!({ "reason": "unrepairable" }),
        ));
    };
    // 7. Media.
    step(opts, 0.9, "Media")?;
    let blobs = Blobs::from_container(container.clone());
    let (missing, linked) = check_media(&doc, &blobs, path);
    if !missing.is_empty() {
        work.issues.warning(
            codes::MEDIA_MISSING,
            None,
            None,
            format!("{} media file(s) referenced by the project are missing", missing.len()),
            json!({ "missing": missing }),
        );
    }
    let (overlays, frozen) = work.preserved();
    preserved.overlays = overlays;
    preserved.frozen = frozen;
    let session = container
        .read(layout::SESSION, 1 << 20)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|t| tt_model::json_text::parse::<Value>(&t).ok())
        .filter(Value::is_object);
    // 8. Report.
    let mut list = work.issues.clone().into_list();
    for i in &mut list {
        if let Some(id) = &i.object_id
            && let Some(p) = doc.canonical_path(id)
        {
            i.path = Some(p);
        }
    }
    let repaired_count = list.iter().filter(|i| i.repaired).count();
    let report = LoadReport {
        project_id: doc.project().id().as_str().to_owned(),
        path: path.to_string_lossy().into_owned(),
        form: form.as_str().to_owned(),
        format_version: file_version.to_string(),
        migrated_from: migrated_from.map(|v| v.to_string()),
        read_only: opts.read_only,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        issues: list,
        repaired_count,
    };
    step(opts, 1.0, "Finalisation")?;
    Ok(Loaded {
        doc,
        report,
        blobs,
        preserved,
        migrated_from,
        repaired,
        saved_rev: file_rev,
        saved_at: manifest.and_then(|m| m.saved_at),
        session,
        form,
        path: path.to_path_buf(),
        missing,
        linked,
    })
}

fn file_item(work: &mut Work, item: repair::Item) {
    match item.ty {
        tt_model::ObjectType::Palette => work.palettes.push(item),
        tt_model::ObjectType::DrawingBank => work.banks.push(item),
        tt_model::ObjectType::Drawing => work.drawings.push(item),
        tt_model::ObjectType::Scene => work.scenes.push(item),
        tt_model::ObjectType::Element => work.elements.push(item),
        tt_model::ObjectType::Library => work.library = Some(item),
        _ => {}
    }
}

fn fatal_parts(errors: &[ModelError]) -> ModelError {
    ModelError::with_details(
        codes::INVALID_FORMAT,
        format!(
            "the project cannot be repaired: {}",
            errors.first().map(ModelError::message).unwrap_or("integrity errors remain")
        ),
        json!({
            "reason": "unrepairable",
            "errors": errors.iter().take(50).map(|e| json!({ "code": e.code(), "message": e.message(), "details": e.details() })).collect::<Vec<_>>(),
        }),
    )
}

/// Every binary entry the document references, with the object referencing it and its declared size (MED-05, MED-01,
/// textures, library files and thumbnails).
#[must_use]
pub fn blob_refs(doc: &Document) -> Vec<(String, String, Option<u64>, &'static str)> {
    let mut out = Vec::new();
    for a in doc.project().assets() {
        if let Some(e) = a.entry() {
            out.push((e.clone(), a.id().as_str().to_owned(), Some(*a.size()), "embedded"));
        }
    }
    for d in doc.drawings() {
        if let Ok(v) = doc.entry_raw_json(&EntryKey::Drawing(d.id().clone())) {
            collect_blobs(&v, d.id().as_str(), &mut out, 0);
        }
    }
    for p in doc.palettes() {
        if let Ok(v) = doc.entry_raw_json(&EntryKey::Palette(p.id().clone())) {
            collect_textures(&v, p.id().as_str(), &mut out, 0);
        }
    }
    if let Ok(v) = doc.entry_raw_json(&EntryKey::Library) {
        collect_library(&v, &mut out, 0);
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn collect_blobs(v: &Value, owner: &str, out: &mut Vec<(String, String, Option<u64>, &'static str)>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            if let (Some(Value::String(e)), Some(Value::String(_)), Some(size)) =
                (m.get("entry"), m.get("sha256"), m.get("size").and_then(Value::as_u64))
                && m.contains_key("mediaType")
            {
                out.push((e.clone(), owner.to_owned(), Some(size), "pixels"));
            }
            for (k, x) in m {
                if k != "meta" && k != "ext" {
                    collect_blobs(x, owner, out, depth + 1);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_blobs(x, owner, out, depth + 1)),
        _ => {}
    }
}

fn collect_textures(v: &Value, owner: &str, out: &mut Vec<(String, String, Option<u64>, &'static str)>, depth: usize) {
    if depth > 200 {
        return;
    }
    match v {
        Value::Object(m) => {
            if let Some(Value::String(img)) = m.get("image")
                && img.starts_with("textures/")
            {
                out.push((img.clone(), owner.to_owned(), None, "texture"));
            }
            for (k, x) in m {
                if k != "meta" && k != "ext" {
                    collect_textures(x, owner, out, depth + 1);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_textures(x, owner, out, depth + 1)),
        _ => {}
    }
}

fn collect_library(v: &Value, out: &mut Vec<(String, String, Option<u64>, &'static str)>, depth: usize) {
    if depth > 200 {
        return;
    }
    if let Value::Object(m) = v {
        if m.get("type").and_then(Value::as_str) == Some("libraryItem") {
            let id = m.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            if let Some(Value::String(f)) = m.get("file") {
                out.push((format!("library/files/{f}"), id.clone(), None, "libraryFile"));
            }
            if let Some(Value::Object(t)) = m.get("thumbnail")
                && let Some(Value::String(e)) = t.get("entry")
            {
                out.push((e.clone(), id, t.get("size").and_then(Value::as_u64), "libraryFile"));
            }
        }
        for x in m.get("children").and_then(Value::as_array).into_iter().flatten() {
            collect_library(x, out, depth + 1);
        }
    }
}

/// Resolves a linked asset (MED-02): (1) the path relative to the folder containing the project, (2) the absolute path
/// at link time, (3) a file with the same name and size in the project's folder.
#[must_use]
pub fn resolve_link(
    project_path: Option<&Path>,
    link_path: &str,
    absolute: Option<&str>,
    size: u64,
) -> Option<PathBuf> {
    let folder = project_path.and_then(Path::parent);
    if let Some(f) = folder {
        let p = f.join(link_path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(a) = absolute {
        let p = PathBuf::from(a);
        if p.is_absolute() && p.is_file() {
            return Some(p);
        }
    }
    let name = Path::new(link_path).file_name()?;
    let f = folder?;
    let p = f.join(name);
    (p.is_file() && std::fs::metadata(&p).is_ok_and(|m| m.len() == size)).then_some(p)
}

fn check_media(doc: &Document, blobs: &Blobs, path: &Path) -> (Vec<MissingMedia>, BTreeMap<String, PathBuf>) {
    let mut missing = Vec::new();
    for (entry, owner, _, kind) in blob_refs(doc) {
        if !blobs.contains(&entry) {
            missing.push(MissingMedia { object_id: owner, location: entry, kind: kind.to_owned() });
        }
    }
    let mut linked = BTreeMap::new();
    for a in doc.project().assets() {
        let Some(link) = a.link() else { continue };
        if link.kind == tt_model::entities::asset::LinkKind::File {
            let p = link.path.as_deref().unwrap_or_default();
            match resolve_link(Some(path), p, link.absolute_path.as_deref(), *a.size()) {
                Some(found) => {
                    linked.insert(a.id().as_str().to_owned(), found);
                }
                None => missing.push(MissingMedia {
                    object_id: a.id().as_str().to_owned(),
                    location: p.to_owned(),
                    kind: "linked".to_owned(),
                }),
            }
        }
    }
    (missing, linked)
}

/// Reads a whole entry of a loaded project as text (helpers for tools and tests).
///
/// # Errors
/// As [`Blobs::read`].
pub fn read_text(blobs: &Blobs, name: &str, limit: u64) -> Result<String, ModelError> {
    let mut s = String::new();
    blobs.reader(name, limit)?.read_to_string(&mut s).map_err(|e| crate::zip::entry_error(name, &e))?;
    Ok(s)
}

/// The entry names a document is stored under, by entry key (for tests and the save path).
#[must_use]
pub fn entry_names(doc: &Document) -> BTreeSet<String> {
    doc.entry_keys().iter().filter_map(|k| layout::entry_path(doc, k)).collect()
}
