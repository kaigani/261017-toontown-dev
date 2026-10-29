//! `doc.*` file and media commands (10 §10.2; C1.3): the load report, binary entries, assets (list, relink, embed,
//! export, remove), package inspection and export, and the asset-status and changed-on-disk events.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_model::ObjectType;
use tt_model::change::PatchOp;
use tt_model::document::{EntryKey, Target};
use tt_model::entities::asset::{AssetStorage, LinkKind};
use tt_persist::container::{BlobData, Form};
use tt_persist::{b64, fsx};
use tt_registry::{CommandError, command};

use super::common::{Job, NoParams, OnExists, event_not_invocable, invalid};
use crate::cx::Cx;
use crate::persistence;

// ----- load report -------------------------------------------------------------------------------------------------

/// An issue (10 §10.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueOut {
    /// `is_<n>`.
    pub issue_id: String,
    /// Code.
    pub code: String,
    /// `info`, `warning`, `error`.
    pub severity: String,
    /// The object concerned.
    pub object_id: Option<String>,
    /// Its canonical path.
    pub path: Option<String>,
    /// The package entry concerned (load issues).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Message.
    pub message: String,
    /// Details.
    pub details: Value,
    /// Machine-applicable fixes.
    pub fixes: Vec<Value>,
}

/// `LoadReport` (10 §7.3, §10.4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadReportOut {
    /// Project id.
    project_id: String,
    /// The path opened (`null` for a project that was not opened from a file).
    path: Option<String>,
    /// `package` or `folder` (`null` likewise).
    form: Option<String>,
    /// The file's format version.
    format_version: String,
    /// The version migrated from.
    migrated_from: Option<String>,
    /// Open read-only.
    read_only: bool,
    /// Load time.
    duration_ms: u64,
    /// Issues found and repaired.
    issues: Vec<IssueOut>,
    /// Repairs applied.
    repaired_count: u64,
}

#[command(
    id = "doc.loadReport.get",
    kind = query,
    label = "Load report",
    scopes = [read],
    needs_project,
    example(title = "Repairs and issues of the last load", params = "{}"),
    gui = "Load report window (M4)",
)]
pub(crate) fn doc_load_report_get(cx: &mut Cx, _p: NoParams) -> Result<LoadReportOut, CommandError> {
    let p = cx.project()?;
    if let Some(r) = &p.store.load_report
        && let Ok(mut out) = serde_json::from_value::<LoadReportOut>(r.clone())
    {
        out.read_only = p.read_only;
        return Ok(out);
    }
    // A project that was created, or installed from memory: an empty report.
    Ok(LoadReportOut {
        project_id: p.doc.project().id().as_str().to_owned(),
        path: p.path.clone(),
        form: p.form.clone(),
        format_version: p.store.format_version.clone(),
        migrated_from: p.store.migrated_from.clone(),
        read_only: p.read_only,
        duration_ms: 0,
        issues: Vec::new(),
        repaired_count: 0,
    })
}

// ----- blobs -------------------------------------------------------------------------------------------------------

/// `doc.blob.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct BlobParams {
    /// An object holding a binary payload: an asset, a bitmap drawing node, a library item (its thumbnail, else its
    /// file).
    #[serde(default, rename = "ref")]
    reference: Option<String>,
    /// A package entry.
    #[serde(default)]
    entry: Option<String>,
    /// Largest payload returned.
    #[serde(default = "eight_mib")]
    #[schemars(range(min = 1, max = 67_108_864))]
    max_bytes: u64,
}

fn eight_mib() -> u64 {
    8_388_608
}

/// A binary payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlobResult {
    /// IANA media type.
    media_type: String,
    /// Size in bytes.
    size: u64,
    /// SHA-256 (lower-case hex).
    sha256: String,
    /// The bytes, base64.
    base64: String,
}

/// The media type of an entry or file name.
pub(crate) fn media_type(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or_default();
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "json" => "application/json",
        "animtpl" => "application/x-animtpl+zip",
        "animpkg" => "application/x-animpkg+zip",
        _ => "application/octet-stream",
    }
}

/// Where the bytes of an asset are: its entry (embedded) or its resolved file (linked).
enum Source {
    Entry(String),
    File(PathBuf),
}

fn asset_source(cx: &Cx, asset_key: &str) -> Result<(Source, String), CommandError> {
    let p = cx.project()?;
    let a = cx.document()?.project().assets().iter().find(|a| a.id().as_str() == asset_key).ok_or_else(|| {
        CommandError::with_details("ref.notFound", format!("no asset `{asset_key}`"), json!({ "ref": asset_key }))
    })?;
    match a.storage() {
        AssetStorage::Embedded => {
            let entry = a.entry().clone().unwrap_or_default();
            Ok((Source::Entry(entry), a.media_type().clone()))
        }
        AssetStorage::Linked => match p.store.linked.get(asset_key) {
            Some(f) => Ok((Source::File(f.clone()), a.media_type().clone())),
            None => Err(CommandError::with_details(
                tt_persist::codes::MEDIA_MISSING,
                format!("the linked file of asset `{}` is missing", a.name()),
                json!({ "assetId": asset_key, "link": a.link() }),
            )
            .hint("relink it with doc.asset.relink")),
        },
    }
}

fn read_source(cx: &Cx, src: &Source, max: u64) -> Result<Vec<u8>, CommandError> {
    let too_large = |size: u64| {
        CommandError::with_details(
            "doc.blobTooLarge",
            format!("the payload has {size} bytes, more than maxBytes {max}"),
            json!({ "size": size, "maxBytes": max }),
        )
        .hint("raise maxBytes, or export the file with doc.asset.export")
    };
    match src {
        Source::Entry(e) => {
            let blobs = &cx.project()?.store.blobs;
            let size = blobs.size(e).ok_or_else(|| {
                CommandError::with_details(
                    tt_persist::codes::MEDIA_MISSING,
                    format!("entry `{e}` is missing"),
                    json!({ "entry": e }),
                )
            })?;
            if size > max {
                return Err(too_large(size));
            }
            Ok(blobs.read(e, max)?)
        }
        Source::File(f) => {
            let size = std::fs::metadata(f)
                .map(|m| m.len())
                .map_err(|e| CommandError::from(fsx::fs_error(&e, f, "cannot read")))?;
            if size > max {
                return Err(too_large(size));
            }
            fsx::read_limited(f, max).map_err(|e| CommandError::from(fsx::fs_error(&e, f, "cannot read")))
        }
    }
}

#[command(
    id = "doc.blob.get",
    kind = query,
    label = "Read blob",
    scopes = [read],
    needs_project,
    example(title = "The embedded sound of the sample", params = r#"{"ref": "as_01K742SG0G5HPQ0RBF5X4DSR0W"}"#),
    agent_only = "agents read binary package entries",
)]
pub(crate) fn doc_blob_get(cx: &mut Cx, p: BlobParams) -> Result<BlobResult, CommandError> {
    let (src, mt) = match (&p.reference, &p.entry) {
        (Some(r), None) => {
            let key = match cx.resolve(r)? {
                Target::Object { key, .. } => key,
                Target::Cell { .. } => return Err(invalid("/ref", "a cell holds no binary payload")),
            };
            let doc = cx.document()?;
            let loc = doc.locate(&key).ok_or_else(|| invalid("/ref", "unknown object"))?;
            match loc.ty {
                ObjectType::Asset => asset_source(cx, &key)?,
                ObjectType::DrawingNode | ObjectType::LibraryItem => {
                    let (_, v) = doc.object_raw_json(&key)?;
                    let entry = v
                        .pointer("/image/entry")
                        .or_else(|| v.pointer("/thumbnail/entry"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| v.get("file").and_then(Value::as_str).map(|f| format!("library/files/{f}")))
                        .ok_or_else(|| invalid("/ref", "the object holds no binary payload"))?;
                    let mt = v
                        .pointer("/image/mediaType")
                        .or_else(|| v.pointer("/thumbnail/mediaType"))
                        .and_then(Value::as_str)
                        .map_or_else(|| media_type(&entry).to_owned(), str::to_owned);
                    (Source::Entry(entry), mt)
                }
                other => {
                    return Err(CommandError::with_details(
                        "ref.wrongType",
                        format!("a {other} holds no binary payload"),
                        json!({ "ref": r, "type": other.as_str() }),
                    ));
                }
            }
        }
        (None, Some(e)) => {
            if !tt_persist::zip::safe_name(e) {
                return Err(invalid("/entry", "not a package entry name"));
            }
            (Source::Entry(e.clone()), media_type(e).to_owned())
        }
        _ => return Err(invalid("/ref", "give either `ref` or `entry`")),
    };
    let bytes = read_source(cx, &src, p.max_bytes)?;
    Ok(BlobResult {
        media_type: mt,
        size: bytes.len() as u64,
        sha256: fsx::sha256(&bytes),
        base64: b64::encode(&bytes),
    })
}

// ----- assets ------------------------------------------------------------------------------------------------------

/// Asset kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AssetKind {
    /// Sound.
    Sound,
    /// Image.
    Image,
    /// Movie.
    Movie,
    /// Vector file.
    Vector,
}

/// Asset filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AssetStatus {
    /// Every asset.
    #[default]
    All,
    /// Missing files.
    Missing,
    /// Linked files whose content changed.
    Changed,
    /// Linked assets.
    Linked,
    /// Embedded assets.
    Embedded,
    /// Assets referenced by nothing.
    Unused,
}

/// `doc.asset.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssetListParams {
    /// Kind filter.
    #[serde(default)]
    kind: Option<AssetKind>,
    /// Status filter.
    #[serde(default)]
    status: AssetStatus,
}

/// One asset (10 §10.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssetEntry {
    /// Asset id.
    id: String,
    /// Name.
    name: String,
    /// `sound`, `image`, `movie`, `vector`.
    kind: String,
    /// `embedded`, `linked`.
    storage: String,
    /// `ok`, `missing`, `changed`.
    status: String,
    /// The entry (embedded) or the resolved file (linked; the link path when missing).
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// Recorded size.
    size: u64,
    /// Number of references (placements, sequences).
    refs: u64,
}

/// `{assets}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct AssetsResult {
    /// Assets.
    assets: Vec<AssetEntry>,
}

/// Elements referencing each asset (placements and media sequences).
pub(crate) fn asset_refs(doc: &tt_model::document::Document) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for e in doc.elements() {
        for pl in e.placements().iter().flatten() {
            out.entry(pl.asset_id().as_str().to_owned()).or_default().push(e.id().as_str().to_owned());
        }
        for sq in e.sequences().iter().flatten() {
            out.entry(sq.asset_id().as_str().to_owned()).or_default().push(e.id().as_str().to_owned());
        }
    }
    out
}

fn status_of(cx: &Cx, id: &str, storage: AssetStorage, entry: Option<&String>) -> Result<String, CommandError> {
    let p = cx.project()?;
    Ok(match storage {
        AssetStorage::Embedded => {
            if entry.is_some_and(|e| p.store.blobs.contains(e)) {
                "ok".into()
            } else {
                "missing".into()
            }
        }
        AssetStorage::Linked => p
            .store
            .asset_status
            .get(id)
            .cloned()
            .unwrap_or_else(|| if p.store.linked.contains_key(id) { "ok".into() } else { "missing".into() }),
    })
}

#[command(
    id = "doc.asset.list",
    kind = query,
    label = "Assets",
    scopes = [read],
    needs_project,
    example(title = "Missing media", params = r#"{"status": "missing"}"#),
    gui = "Project › Media (M8)",
)]
pub(crate) fn doc_asset_list(cx: &mut Cx, p: AssetListParams) -> Result<AssetsResult, CommandError> {
    let doc = cx.document()?.clone();
    let refs = asset_refs(&doc);
    let mut out = Vec::new();
    for a in doc.project().assets() {
        let id = a.id().as_str().to_owned();
        let kind = serde_json::to_value(a.kind()).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        if let Some(k) = p.kind {
            let matches = matches!(
                (k, kind.as_str()),
                (AssetKind::Sound, "sound")
                    | (AssetKind::Image, "image")
                    | (AssetKind::Movie, "movie")
                    | (AssetKind::Vector, "vector")
            );
            if !matches {
                continue;
            }
        }
        let storage = *a.storage();
        let status = status_of(cx, &id, storage, a.entry().as_ref())?;
        let n = refs.get(&id).map_or(0, Vec::len) as u64;
        let keep = match p.status {
            AssetStatus::All => true,
            AssetStatus::Missing => status == "missing",
            AssetStatus::Changed => status == "changed",
            AssetStatus::Linked => storage == AssetStorage::Linked,
            AssetStatus::Embedded => storage == AssetStorage::Embedded,
            AssetStatus::Unused => n == 0,
        };
        if !keep {
            continue;
        }
        let path = match storage {
            AssetStorage::Embedded => a.entry().clone(),
            AssetStorage::Linked => cx
                .project()?
                .store
                .linked
                .get(&id)
                .map(|f| f.to_string_lossy().into_owned())
                .or_else(|| a.link().as_ref().and_then(|l| l.path.clone())),
        };
        out.push(AssetEntry {
            id,
            name: a.name().clone(),
            kind,
            storage: if storage == AssetStorage::Embedded { "embedded".into() } else { "linked".into() },
            status,
            path,
            size: *a.size(),
            refs: n,
        });
    }
    Ok(AssetsResult { assets: out })
}

fn asset_patch(cx: &mut Cx, id: &str, ops: Vec<PatchOp>) -> Result<(), CommandError> {
    cx.patch_objects(vec![(id.to_owned(), ops)])
}

fn op(o: &'static str, path: &str, value: Value) -> PatchOp {
    PatchOp { op: o, from: None, path: path.into(), range: None, value: Some(value), old_value: None, elided: false }
}

/// `doc.asset.relink` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AssetRelinkParams {
    /// One linked asset.
    #[serde(default)]
    asset_id: Option<String>,
    /// Its new file (absolute).
    #[serde(default)]
    path: Option<String>,
    /// A folder to search for missing linked assets (MED-02 step 4).
    #[serde(default)]
    search_folder: Option<String>,
    /// Search subfolders.
    #[serde(default = "yes")]
    recursive: bool,
    /// How a candidate matches.
    #[serde(default)]
    match_by: MatchBy,
}

fn yes() -> bool {
    true
}

/// How a search candidate matches a missing asset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MatchBy {
    /// Same file name.
    Name,
    /// Same name and size.
    #[default]
    NameAndSize,
    /// Same SHA-256.
    Hash,
}

/// A relinked asset.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Relinked {
    /// Asset id.
    asset_id: String,
    /// Its file.
    path: String,
}

/// `doc.asset.relink` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelinkResult {
    /// Relinked assets.
    relinked: Vec<Relinked>,
    /// Linked assets still missing.
    still_missing: Vec<String>,
}

/// Patches an asset's link to a file (relative to the folder containing the project when there is one).
fn link_to(cx: &mut Cx, id: &str, file: &Path) -> Result<(), CommandError> {
    let (sha, size) = std::fs::File::open(file)
        .and_then(fsx::sha256_reader)
        .map_err(|e| CommandError::from(fsx::fs_error(&e, file, "cannot read")))?;
    let folder = cx.project()?.path.as_ref().and_then(|p| Path::new(p).parent().map(Path::to_path_buf));
    let rel = folder.as_ref().and_then(|f| relative(file, f)).unwrap_or_else(|| file.to_string_lossy().into_owned());
    asset_patch(
        cx,
        id,
        vec![
            op("replace", "/link/path", json!(rel)),
            op("replace", "/link/absolutePath", json!(file.to_string_lossy())),
            op("replace", "/size", json!(size)),
            op("replace", "/sha256", json!(sha)),
        ],
    )?;
    if let Ok(p) = cx.project_mut() {
        p.store.linked.insert(id.to_owned(), file.to_path_buf());
        p.store.asset_status.insert(id.to_owned(), "ok".into());
    }
    Ok(())
}

/// `file` relative to `base` with `/` separators, when it is inside `base` or a sibling of it.
fn relative(file: &Path, base: &Path) -> Option<String> {
    let f = fsx::resolve(file);
    let b = fsx::resolve(base);
    let rest = f.strip_prefix(&b).ok()?;
    Some(rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
}

fn walk_files(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 32 || out.len() > 100_000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_file() {
            out.push(e.path());
        } else if ft.is_dir() && recursive {
            walk_files(&e.path(), recursive, out, depth + 1);
        }
    }
}

#[command(
    id = "doc.asset.relink",
    kind = action,
    label = "Relink Media",
    scopes = [edit, files_read],
    undo = "Relink Media",
    needs_project,
    example(title = "Search a folder for missing media", params = r#"{"searchFolder": "/tmp/media"}"#),
    gui = "Project › Media › Relink (M8)",
)]
pub(crate) fn doc_asset_relink(cx: &mut Cx, p: AssetRelinkParams) -> Result<RelinkResult, CommandError> {
    let doc = cx.document()?.clone();
    let linked: Vec<_> = doc
        .project()
        .assets()
        .iter()
        .filter(|a| *a.storage() == AssetStorage::Linked && a.link().as_ref().is_some_and(|l| l.kind == LinkKind::File))
        .cloned()
        .collect();
    let mut relinked = Vec::new();
    match (&p.asset_id, &p.path, &p.search_folder) {
        (Some(id), Some(path), None) => {
            let key = cx.resolve_as(id, ObjectType::Asset)?;
            let file = persistence::path_param(cx, path, "/path")?;
            if !linked.iter().any(|a| a.id().as_str() == key) {
                return Err(invalid("/assetId", "only linked assets are relinked"));
            }
            if !file.is_file() {
                return Err(CommandError::with_details(
                    "asset.relinkFailed",
                    format!("`{}` is not a file", file.display()),
                    json!({ "assetId": key, "path": file.to_string_lossy() }),
                ));
            }
            link_to(cx, &key, &file)?;
            relinked.push(Relinked { asset_id: key, path: file.to_string_lossy().into_owned() });
        }
        (None, None, Some(folder)) => {
            let folder = persistence::path_param(cx, folder, "/searchFolder")?;
            let mut files = Vec::new();
            walk_files(&folder, p.recursive, &mut files, 0);
            for a in &linked {
                let id = a.id().as_str().to_owned();
                if cx.project()?.store.linked.contains_key(&id) {
                    continue;
                }
                let name = a
                    .link()
                    .as_ref()
                    .and_then(|l| l.path.clone())
                    .and_then(|x| Path::new(&x).file_name().map(|n| n.to_string_lossy().into_owned()));
                let hit = files.iter().find(|f| {
                    let same_name =
                        name.as_deref().is_some_and(|n| f.file_name().is_some_and(|x| x.to_string_lossy() == n));
                    match p.match_by {
                        MatchBy::Name => same_name,
                        MatchBy::NameAndSize => same_name && std::fs::metadata(f).is_ok_and(|m| m.len() == *a.size()),
                        MatchBy::Hash => {
                            std::fs::metadata(f).is_ok_and(|m| m.len() == *a.size())
                                && std::fs::File::open(f)
                                    .ok()
                                    .and_then(|x| fsx::sha256_reader(x).ok())
                                    .is_some_and(|(h, _)| h == *a.sha256())
                        }
                    }
                });
                if let Some(f) = hit.cloned() {
                    link_to(cx, &id, &f)?;
                    relinked.push(Relinked { asset_id: id, path: f.to_string_lossy().into_owned() });
                }
            }
        }
        _ => return Err(invalid("/assetId", "give `assetId` with `path`, or `searchFolder`")),
    }
    let still_missing = linked
        .iter()
        .map(|a| a.id().as_str().to_owned())
        .filter(|id| !relinked.iter().any(|r| &r.asset_id == id))
        .filter(|id| cx.project().is_ok_and(|p| !p.store.linked.contains_key(id)))
        .collect();
    Ok(RelinkResult { relinked, still_missing })
}

/// `doc.asset.embed` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AssetEmbedParams {
    /// Assets to embed.
    #[serde(default)]
    asset_ids: Option<Vec<String>>,
    /// Every linked asset.
    #[serde(default)]
    all: bool,
}

/// `doc.asset.embed` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EmbedResult {
    /// Embedded assets.
    embedded: Vec<String>,
    /// Bytes added to the package.
    bytes: u64,
}

#[command(
    id = "doc.asset.embed",
    kind = action,
    label = "Embed Media",
    scopes = [edit, files_read],
    undo = "Embed Media",
    needs_project,
    example(title = "Embed every linked file", params = r#"{"all": true}"#),
    gui = "Project › Media › Embed (M8)",
)]
pub(crate) fn doc_asset_embed(cx: &mut Cx, p: AssetEmbedParams) -> Result<EmbedResult, CommandError> {
    let doc = cx.document()?.clone();
    let wanted: Vec<String> = match (&p.asset_ids, p.all) {
        (Some(ids), false) => ids.iter().map(|r| cx.resolve_as(r, ObjectType::Asset)).collect::<Result<_, _>>()?,
        (None, true) => doc
            .project()
            .assets()
            .iter()
            .filter(|a| *a.storage() == AssetStorage::Linked)
            .map(|a| a.id().as_str().to_owned())
            .collect(),
        _ => return Err(invalid("/assetIds", "give `assetIds` or `all: true`")),
    };
    let cache = cx.session().env.cache.clone();
    let mut embedded = Vec::new();
    let mut bytes = 0u64;
    // MED-01 (C13-R1-16): identical content is stored once and shared, by SHA-256 — with an asset already embedded,
    // or with one embedded earlier in this call.
    let mut by_hash: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for a in doc.project().assets() {
        if let (AssetStorage::Embedded, Some(e)) = (a.storage(), a.entry())
            && cx.project()?.store.blobs.contains(e)
        {
            by_hash.entry(a.sha256().clone()).or_insert_with(|| e.clone());
        }
    }
    for id in wanted {
        let Some(a) = doc.project().assets().iter().find(|a| a.id().as_str() == id) else { continue };
        if *a.storage() != AssetStorage::Linked {
            continue;
        }
        let Some(file) = cx.project()?.store.linked.get(&id).cloned() else {
            cx.warn(
                tt_persist::codes::MEDIA_MISSING,
                format!("the linked file of `{}` is missing; it was not embedded", a.name()),
                vec![id.clone()],
            );
            continue;
        };
        let ext = file.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "bin".into());
        let (sha, size) = std::fs::File::open(&file)
            .and_then(fsx::sha256_reader)
            .map_err(|e| CommandError::from(fsx::fs_error(&e, &file, "cannot read")))?;
        let shared = by_hash.get(&sha).cloned();
        let entry = shared.clone().unwrap_or_else(|| format!("media/{id}.{ext}"));
        // The bytes are frozen in the session cache now (MED-01: original bytes, no re-encoding).
        let frozen = cache.join(fsx::random_token(8)).join(format!("{id}.{ext}"));
        if !cx.dry_run() && shared.is_none() {
            fsx::copy_file(&file, &frozen).map_err(|e| CommandError::from(fsx::fs_error(&e, &file, "cannot copy")))?;
        }
        by_hash.entry(sha.clone()).or_insert_with(|| entry.clone());
        asset_patch(
            cx,
            &id,
            vec![
                op("replace", "/storage", json!("embedded")),
                op("replace", "/entry", json!(entry)),
                op("replace", "/link", Value::Null),
                op("replace", "/size", json!(size)),
                op("replace", "/sha256", json!(sha)),
            ],
        )?;
        if !cx.dry_run()
            && shared.is_none()
            && let Ok(p) = cx.project_mut()
        {
            p.store.blobs.insert(entry.clone(), BlobData::File(frozen));
        }
        if shared.is_none() {
            bytes += size;
        }
        embedded.push(id);
    }
    Ok(EmbedResult { embedded, bytes })
}

/// `doc.asset.export` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct AssetExportParams {
    /// The asset.
    asset_id: String,
    /// Absolute target file.
    path: String,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// `{path, bytes}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PathBytes {
    /// Written file.
    path: String,
    /// Bytes written.
    bytes: u64,
}

fn exists_error(path: &Path) -> CommandError {
    CommandError::with_details(
        tt_persist::codes::FS_EXISTS,
        format!("`{}` exists", path.display()),
        json!({ "path": path.to_string_lossy() }),
    )
    .hint("pass onExists: \"overwrite\"")
}

#[command(
    id = "doc.asset.export",
    kind = query,
    label = "Export media file",
    scopes = [read, files_write],
    needs_project,
    example(title = "Write the sample's sound to a file", params = r#"{"assetId": "as_01K742SG0G5HPQ0RBF5X4DSR0W", "path": "/tmp/line1.wav", "onExists": "overwrite"}"#),
    gui = "Project › Media › Export (M8)",
)]
pub(crate) fn doc_asset_export(cx: &mut Cx, p: AssetExportParams) -> Result<PathBytes, CommandError> {
    let key = cx.resolve_as(&p.asset_id, ObjectType::Asset)?;
    let target = persistence::path_param(cx, &p.path, "/path")?;
    if target.exists() && p.on_exists == OnExists::Fail {
        return Err(exists_error(&target));
    }
    let (src, _) = asset_source(cx, &key)?;
    let bytes = read_source(cx, &src, 1 << 40)?;
    if !cx.dry_run() {
        fsx::write_atomic(&target, &bytes, fsx::fs_error)?;
    }
    Ok(PathBytes { path: target.to_string_lossy().into_owned(), bytes: bytes.len() as u64 })
}

/// `doc.asset.remove` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssetRemoveParams {
    /// Assets to remove.
    #[serde(default, rename = "assetIds")]
    asset_ids: Option<Vec<String>>,
    /// Every asset referenced by nothing.
    #[serde(default)]
    unused: bool,
    /// Also remove referenced assets, detaching their references.
    #[serde(default)]
    force: bool,
}

/// `doc.asset.remove` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoveResult {
    /// Removed assets.
    removed: Vec<String>,
    /// Bytes no longer stored (embedded files leave the package at the next save, MED-04).
    bytes: u64,
    /// Placements and media sequences removed with them (`force`).
    detached_refs: Vec<String>,
}

/// Removes assets (and with `force`, the placements and sequences using them) as one structural change.
pub(crate) fn remove_assets(cx: &mut Cx, ids: &[String], force: bool) -> Result<RemoveResult, CommandError> {
    let doc = cx.document()?.clone();
    let refs = asset_refs(&doc);
    let mut removed = Vec::new();
    let mut bytes = 0;
    let mut detached = Vec::new();
    let mut changes: Vec<(EntryKey, Vec<PatchOp>)> = Vec::new();
    let project_json = doc.entry_raw_json(&EntryKey::Project)?;
    let mut assets: Vec<Value> = project_json.get("assets").and_then(Value::as_array).cloned().unwrap_or_default();
    for id in ids {
        let used = refs.get(id).is_some_and(|r| !r.is_empty());
        if used && !force {
            return Err(CommandError::with_details(
                "asset.inUse",
                format!("asset `{id}` is referenced; pass force: true to remove it with its references"),
                json!({ "assetId": id, "refs": refs.get(id) }),
            ));
        }
        let Some(pos) = assets.iter().position(|a| a.get("id").and_then(Value::as_str) == Some(id)) else {
            return Err(CommandError::with_details("ref.notFound", format!("no asset `{id}`"), json!({ "ref": id })));
        };
        let a = assets.remove(pos);
        if a.get("storage").and_then(Value::as_str) == Some("embedded") {
            bytes += a.get("size").and_then(Value::as_u64).unwrap_or(0);
        }
        removed.push(id.clone());
    }
    changes.push((EntryKey::Project, vec![op("replace", "/assets", Value::Array(assets))]));
    if force {
        for e in doc.elements() {
            let ej = doc.entry_raw_json(&EntryKey::Element(e.id().clone()))?;
            let mut ops = Vec::new();
            for field in ["placements", "sequences"] {
                let Some(list) = ej.get(field).and_then(Value::as_array) else { continue };
                let keep: Vec<Value> = list
                    .iter()
                    .filter(|x| {
                        let hit =
                            x.get("assetId").and_then(Value::as_str).is_some_and(|a| removed.iter().any(|r| r == a));
                        if hit && let Some(i) = x.get("id").and_then(Value::as_str) {
                            detached.push(i.to_owned());
                        }
                        !hit
                    })
                    .cloned()
                    .collect();
                if keep.len() != list.len() {
                    ops.push(op("replace", &format!("/{field}"), Value::Array(keep)));
                }
            }
            if !ops.is_empty() {
                changes.push((EntryKey::Element(e.id().clone()), ops));
            }
        }
    }
    cx.patch_entries(changes)?;
    for id in &removed {
        if let Ok(p) = cx.project_mut() {
            p.store.linked.remove(id);
            p.store.asset_status.remove(id);
        }
    }
    Ok(RemoveResult { removed, bytes, detached_refs: detached })
}

#[command(
    id = "doc.asset.remove",
    kind = action,
    label = "Remove Media",
    scopes = [edit],
    undo = "Remove Media",
    needs_project,
    example(title = "Remove media nothing uses", params = r#"{"unused": true}"#),
    gui = "Project › Media › Remove (M8)",
)]
pub(crate) fn doc_asset_remove(cx: &mut Cx, p: AssetRemoveParams) -> Result<RemoveResult, CommandError> {
    let doc = cx.document()?.clone();
    let ids: Vec<String> = match (&p.asset_ids, p.unused) {
        (Some(ids), false) => ids.iter().map(|r| cx.resolve_as(r, ObjectType::Asset)).collect::<Result<_, _>>()?,
        (None, true) => {
            let refs = asset_refs(&doc);
            doc.project()
                .assets()
                .iter()
                .map(|a| a.id().as_str().to_owned())
                .filter(|id| !refs.contains_key(id))
                .collect()
        }
        _ => return Err(invalid("/assetIds", "give `assetIds` or `unused: true`")),
    };
    if ids.is_empty() {
        return Ok(RemoveResult { removed: Vec::new(), bytes: 0, detached_refs: Vec::new() });
    }
    remove_assets(cx, &ids, p.force)
}

// ----- packages ----------------------------------------------------------------------------------------------------

/// `doc.package.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageGetParams {
    /// Absolute path of a package, folder, template, palette file or library folder.
    path: String,
    /// Load and validate it completely (10 §7) and report its issues.
    #[serde(default)]
    validate: bool,
}

/// What a file on disk is (10 §10.2 `doc.package.get`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageInfo {
    /// `project`, `template`, `palette`, `library`.
    kind: String,
    /// `package`, `folder` or `file`.
    form: String,
    /// Format version.
    format_version: Option<String>,
    /// Lowest reader version.
    min_reader_version: Option<String>,
    /// Whether this version can read it.
    readable: bool,
    /// Project id (projects).
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
    /// Name.
    name: Option<String>,
    /// Save time.
    saved_at: Option<String>,
    /// Number of entries.
    entry_count: u64,
    /// Issues found (with `validate`; container and version problems always).
    issues: Vec<Value>,
}

fn issue_json(code: &str, severity: &str, message: &str, details: Value) -> Value {
    json!({ "code": code, "severity": severity, "message": message, "details": details })
}

fn inspect(path: &Path, validate: bool, migrations: &'static [tt_persist::migrate::Migration]) -> PackageInfo {
    let mut info = PackageInfo {
        kind: "project".into(),
        form: if path.is_dir() { "folder".into() } else { "package".into() },
        format_version: None,
        min_reader_version: None,
        readable: false,
        project_id: None,
        name: None,
        saved_at: None,
        entry_count: 0,
        issues: Vec::new(),
    };
    let fail = |info: &mut PackageInfo, e: &tt_model::ModelError| {
        info.issues.push(issue_json(e.code(), "error", e.message(), e.details().clone()));
    };
    // Library folders and palette or template files first.
    if path.is_dir() && path.join("library.json").is_file() && !path.join(tt_persist::layout::PROJECT).is_file() {
        info.kind = "library".into();
        match tt_persist::library::read_library_folder(path) {
            Ok(l) => {
                info.readable = true;
                let v = tt_model::object::to_json(&l.library).unwrap_or(Value::Null);
                info.name = v.get("name").and_then(Value::as_str).map(str::to_owned);
                info.format_version = v.get("formatVersion").and_then(Value::as_str).map(str::to_owned);
                info.entry_count = l.broken.len() as u64;
                for (id, file) in l.broken {
                    info.issues.push(issue_json(
                        "brokenLibraryItems",
                        "warning",
                        "the item's file is missing",
                        json!({ "itemId": id, "file": file }),
                    ));
                }
            }
            Err(e) => fail(&mut info, &e),
        }
        return info;
    }
    if path.is_file() {
        // The type is in the first entry (`mimetype`, ZIP-02): read a bounded header only (C13-R1-14).
        let mut buf = [0u8; 70];
        let n = std::fs::File::open(path).and_then(|mut f| read_up_to(&mut f, &mut buf)).unwrap_or(0);
        let header = &buf[..n];
        let head = header.get(30..38).map(<[u8]>::to_vec).unwrap_or_default();
        let mime_at = header.get(38..).map(|x| String::from_utf8_lossy(x).into_owned()).unwrap_or_default();
        if head == b"mimetype" && mime_at.starts_with(tt_persist::manifest::TEMPLATE_MIMETYPE) {
            info.kind = "template".into();
            match tt_persist::library::read_template(path) {
                Ok(t) => {
                    info.readable = true;
                    info.format_version = Some(t.manifest.format_version.to_string());
                    info.min_reader_version = Some(t.manifest.min_reader_version.to_string());
                    info.name = t.meta.get("name").and_then(Value::as_str).map(str::to_owned);
                    info.saved_at = t.manifest.saved_at;
                    info.entry_count = t.manifest.entries.map_or(0, |e| e.len() as u64);
                }
                Err(e) => fail(&mut info, &e),
            }
            return info;
        }
        let palette = (head == b"mimetype" && mime_at.starts_with(tt_persist::manifest::PALETTE_MIMETYPE))
            || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("animpal"));
        if palette {
            info.kind = "palette".into();
            info.form = "file".into();
            match tt_persist::library::read_palette_file(path) {
                Ok(p) => {
                    info.readable = true;
                    info.format_version = p.content.get("formatVersion").and_then(Value::as_str).map(str::to_owned);
                    info.entry_count =
                        p.content.get("palettes").and_then(Value::as_array).map_or(0, |a| a.len() as u64);
                    info.name = p
                        .content
                        .get("palettes")
                        .and_then(|a| a.get(0))
                        .and_then(|x| x.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                }
                Err(e) => fail(&mut info, &e),
            }
            return info;
        }
    }
    match tt_persist::Container::open(path, tt_persist::manifest::PACKAGE_MIMETYPE, Default::default()) {
        Err(e) => fail(&mut info, &e),
        Ok(c) => {
            info.entry_count = c.names().len() as u64;
            if let Some(m) = c
                .read(tt_persist::layout::MANIFEST, 64 << 20)
                .ok()
                .and_then(|b| tt_persist::manifest::Manifest::parse(&b).ok())
            {
                info.format_version = Some(m.format_version.to_string());
                info.min_reader_version = Some(m.min_reader_version.to_string());
                info.project_id.clone_from(&m.project_id);
                info.saved_at.clone_from(&m.saved_at);
                info.readable = m.check("animpkg", tt_persist::load::KNOWN_FEATURES).is_ok();
                if let Err(e) = m.check("animpkg", tt_persist::load::KNOWN_FEATURES) {
                    fail(&mut info, &e);
                }
            } else if c.form() == Form::Folder {
                info.readable = true;
            }
            info.name = c
                .read(tt_persist::layout::PROJECT, 64 << 20)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_owned));
            if validate && info.readable {
                match tt_persist::load(
                    path,
                    &tt_persist::LoadOptions { read_only: true, migrations, ..Default::default() },
                ) {
                    Ok(l) => {
                        info.project_id = Some(l.doc.project().id().as_str().to_owned());
                        for i in &l.report.issues {
                            info.issues.push(serde_json::to_value(i).unwrap_or(Value::Null));
                        }
                    }
                    Err(e) => {
                        info.readable = false;
                        fail(&mut info, &e);
                    }
                }
            }
        }
    }
    info
}

#[command(
    id = "doc.package.get",
    kind = query,
    label = "Inspect package",
    scopes = [read, files_read],
    example(title = "Inspect a package without opening it", params = r#"{"path": "/tmp/Short.animpkg", "validate": true}"#),
    agent_only = "agents inspect files before opening them",
)]
pub(crate) fn doc_package_get(cx: &mut Cx, p: PackageGetParams) -> Result<PackageInfo, CommandError> {
    let path = persistence::path_param(cx, &p.path, "/path")?;
    if !path.exists() {
        return Err(fsx::open_error(&std::io::Error::from(std::io::ErrorKind::NotFound), &path).into());
    }
    Ok(inspect(&path, p.validate, cx.session().env.migrations))
}

/// Forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FormParam {
    /// One zip file.
    #[default]
    Package,
    /// A folder.
    Folder,
}

/// `doc.package.export` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PackageExportParams {
    /// Absolute target path.
    path: String,
    /// Form of the copy.
    #[serde(default)]
    form: FormParam,
    /// Overwrite policy.
    #[serde(default)]
    on_exists: OnExists,
}

/// The final result of `doc.package.export`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageExported {
    /// Where.
    path: String,
    /// `package` or `folder`.
    form: String,
    /// Bytes written.
    bytes: u64,
    /// Entries written.
    entries: u64,
}

#[command(
    id = "doc.package.export",
    kind = job,
    label = "Export a copy",
    scopes = [read, files_write],
    needs_project,
    example(title = "A folder-form copy for version control", params = r#"{"path": "/tmp/Short-folder", "form": "folder", "onExists": "overwrite"}"#),
    agent_only = "agents copy the project in either form (10 FMT-02)",
)]
pub(crate) fn doc_package_export(cx: &mut Cx, p: PackageExportParams) -> Result<Job<PackageExported>, CommandError> {
    let target = persistence::path_param(cx, &p.path, "/path")?;
    let form = if p.form == FormParam::Folder { Form::Folder } else { Form::Package };
    let current = cx.project()?.path.clone().map(PathBuf::from);
    if current.as_ref().is_some_and(|c| {
        crate::store::normalize(&c.to_string_lossy()) == crate::store::normalize(&target.to_string_lossy())
    }) {
        return Err(invalid("/path", "the copy cannot replace the project itself; use project.save"));
    }
    if target.exists() && p.on_exists == OnExists::Fail {
        return Err(exists_error(&target));
    }
    if target.exists() && form == Form::Package && target.is_dir() {
        return Err(invalid("/path", "a package cannot replace a folder"));
    }
    if cx.dry_run() {
        return Ok(Job::dry());
    }
    // The copy is the document as it is: no `modifiedAt` stamp, no change to the open project (FMT-02).
    let snap = persistence::snapshot(cx, crate::time::now_iso())?;
    let cache = cx.session().env.cache.clone();
    let origin = cx.origin().clone();
    let job = cx.session().jobs.submit("doc.package.export", &origin.session_key(), move |progress| {
        if !progress.commit() {
            return Err(CommandError::with_details("job.cancelled", "cancelled", json!({})));
        }
        progress.report(0.1, "Write");
        // The package writer replaces the destination only once the copy is written and verified (SAVE-01); the
        // previous file stays until then (C13-R1-07).
        let w = persistence::write(&snap, &target, form, None, &cache, form == Form::Folder)?;
        let entries = w.container.as_ref().map_or(w.saved.written.len() as u64, |c| c.names().len() as u64);
        Ok(serde_json::to_value(PackageExported {
            path: target.to_string_lossy().into_owned(),
            form: form.as_str().to_owned(),
            bytes: w.saved.bytes,
            entries,
        })
        .unwrap_or(Value::Null))
    })?;
    Ok(Job::new(job))
}

/// Reads until `buf` is full or the end of the file; returns the bytes read.
fn read_up_to(r: &mut impl std::io::Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

// ----- events ------------------------------------------------------------------------------------------------------

/// `doc.asset.status.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct AssetStatusChanged {
    /// `[{assetId, status}]`.
    assets: Vec<Map<String, Value>>,
}

#[command(
    id = "doc.asset.status.changed",
    kind = event,
    label = "Asset status changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn doc_asset_status_changed(_cx: &mut Cx, _p: NoParams) -> Result<AssetStatusChanged, CommandError> {
    Err(event_not_invocable("doc.asset.status.changed"))
}

/// `doc.package.changedOnDisk` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedOnDisk {
    /// The project file or folder.
    path: String,
    /// When the change was noticed.
    detected_at: String,
}

#[command(
    id = "doc.package.changedOnDisk",
    kind = event,
    label = "Project changed on disk",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn doc_package_changed_on_disk(_cx: &mut Cx, _p: NoParams) -> Result<ChangedOnDisk, CommandError> {
    Err(event_not_invocable("doc.package.changedOnDisk"))
}
