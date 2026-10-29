//! Saving (10 §8.1): the entry tree of a document snapshot, the atomic package writer (SAVE-01), the journaled folder
//! writer (SAVE-02), and the recovery of an interrupted folder save on open.
//!
//! **The entry tree** (10 §6.3, FMT-01): every object entry as canonical JSON (10 §6.5) — the model's persisted entry
//! form with the newer-minor properties of the file merged back (VER-02), frozen entries written as read —, the
//! referenced binary entries (MED-04: unreferenced embedded media are not written), the unknown entries of the source
//! (ZIP-04), `session.json`, and `manifest.json`. Both forms write the same entries with the same bytes; the package
//! manifest lists every entry with size and SHA-256, the folder manifest omits the list (10 §6.4).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tt_model::ModelError;
use tt_model::document::{Document, EntryKey};
use tt_model::overlay;

use crate::codes;
use crate::container::{Blobs, Container, FolderIndex, Form};
use crate::fsx::{self, sha256};
use crate::journal;
use crate::layout::{self, Kind};
use crate::load::{self, Preserved};
use crate::manifest::{Manifest, ManifestEntry, PACKAGE_MIMETYPE, Version};
use crate::report::Issues;
use crate::zip::{Limits, Method, ZipArchive, ZipWriter};

/// What one entry holds.
#[derive(Debug, Clone)]
pub enum Content {
    /// Bytes (JSON text).
    Bytes(Vec<u8>),
    /// A binary entry read from the project's blob store.
    Blob {
        /// Bytes expected (when known).
        size: Option<u64>,
    },
}

/// One entry to write.
#[derive(Debug, Clone)]
pub struct Planned {
    /// Entry name.
    pub name: String,
    /// Content.
    pub content: Content,
}

/// The inputs of a save.
pub struct SaveInput<'a> {
    /// The snapshot.
    pub doc: &'a Document,
    /// Data preserved from the file.
    pub preserved: &'a Preserved,
    /// Binary entries.
    pub blobs: &'a Blobs,
    /// `session.json` content.
    pub session: Option<&'a Value>,
    /// Save time.
    pub saved_at: String,
}

/// The canonical text of an object entry, as written (overlay merged, frozen entries as read, `library.json` in the
/// `animlib` form).
///
/// # Errors
/// Serialisation errors.
pub fn entry_text(doc: &Document, preserved: &Preserved, key: &EntryKey) -> Result<String, ModelError> {
    let value = if let Some(f) = preserved.frozen.get(key) {
        f.raw.clone()
    } else {
        let v = doc.entry_json(key)?;
        match preserved.overlays.get(key) {
            Some(o) => overlay::merge(v, o),
            None => v,
        }
    };
    let value = if *key == EntryKey::Library { crate::library::to_animlib(value) } else { value };
    tt_model::canonical::to_canonical_string(&value)
}

/// The entries a snapshot is saved as (manifest excluded), sorted by name with `mimetype` first.
///
/// # Errors
/// Serialisation errors; `internal` for an entry without a location (an element outside every scene).
pub fn plan(input: &SaveInput<'_>) -> Result<Vec<Planned>, ModelError> {
    let mut out: BTreeMap<String, Planned> = BTreeMap::new();
    for key in input.doc.entry_keys() {
        let name = layout::entry_path(input.doc, &key)
            .ok_or_else(|| ModelError::new(codes::INTERNAL, format!("entry {key:?} has no location")))?;
        let text = entry_text(input.doc, input.preserved, &key)?;
        out.insert(name.clone(), Planned { name, content: Content::Bytes(text.into_bytes()) });
    }
    for (entry, _, size, _) in load::saved_blob_refs(input.doc) {
        if input.blobs.contains(&entry) && !out.contains_key(&entry) {
            out.insert(entry.clone(), Planned { name: entry, content: Content::Blob { size } });
        }
    }
    // The project thumbnail is derived (regenerated on save, 10 §6.3); until its generator exists (M3) the existing one
    // is kept (C13-R1-13).
    if input.blobs.contains(layout::THUMBNAIL) {
        out.insert(
            layout::THUMBNAIL.to_owned(),
            Planned { name: layout::THUMBNAIL.to_owned(), content: Content::Blob { size: None } },
        );
    }
    for u in &input.preserved.unknown_entries {
        if input.blobs.contains(u) && !out.contains_key(u) {
            out.insert(u.clone(), Planned { name: u.clone(), content: Content::Blob { size: None } });
        }
    }
    if let Some(s) = input.session
        && let Ok(t) = tt_model::canonical::to_canonical_string(s)
    {
        out.insert(
            layout::SESSION.to_owned(),
            Planned { name: layout::SESSION.to_owned(), content: Content::Bytes(t.into_bytes()) },
        );
    }
    Ok(out.into_values().collect())
}

/// The manifest of a snapshot.
#[must_use]
pub fn manifest_for(input: &SaveInput<'_>, entries: Option<Vec<ManifestEntry>>) -> Manifest {
    let mut m = Manifest::new(
        "animpkg",
        Some(input.doc.project().id().as_str().to_owned()),
        Some(input.doc.document_rev()),
        input.saved_at.clone(),
    );
    // VER-02: a file of a newer minor version keeps its version, so newer readers keep its meaning.
    if let Some((v, min)) = input.preserved.newer_version
        && v > Version::current()
    {
        m.format_version = v;
        m.min_reader_version = min;
    }
    m.unknown.clone_from(&input.preserved.manifest_unknown);
    m.entries = entries;
    m
}

/// Hashes what it writes.
struct HashingReader<R> {
    inner: R,
    hasher: sha2::Sha256,
    n: u64,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use sha2::Digest;
        let k = self.inner.read(buf)?;
        self.hasher.update(&buf[..k]);
        self.n += k as u64;
        Ok(k)
    }
}

fn blob_limit(size: Option<u64>) -> u64 {
    size.map_or(1 << 40, |s| s.saturating_add(load::SIZE_MARGIN))
}

/// Writes a package to `out` (a new file). Returns the manifest written.
///
/// # Errors
/// I/O errors (`project.saveFailed` with `details.reason`), blob read errors.
pub fn write_package(
    out: &Path,
    input: &SaveInput<'_>,
    planned: &[Planned],
    force_zip64: bool,
) -> Result<Manifest, ModelError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| fsx::save_error(&e, out, "cannot create"))?;
    let mut w = ZipWriter::new(BufWriter::new(file)).force_zip64(force_zip64);
    let io = |e: io::Error| fsx::save_error(&e, out, "cannot write");
    w.add_bytes(layout::MIMETYPE, PACKAGE_MIMETYPE.as_bytes(), Method::Stored).map_err(io)?;
    let mut listed = Vec::new();
    for p in planned {
        let method = Method::for_name(&p.name);
        match &p.content {
            Content::Bytes(b) => {
                w.add_bytes(&p.name, b, method).map_err(io)?;
                if layout::classify(&p.name).listed() {
                    listed.push(ManifestEntry { path: p.name.clone(), size: b.len() as u64, sha256: sha256(b) });
                }
            }
            Content::Blob { size } => {
                use sha2::Digest;
                let r = input.blobs.reader(&p.name, blob_limit(*size))?;
                let mut h = HashingReader { inner: r, hasher: sha2::Sha256::new(), n: 0 };
                let hint = input.blobs.size(&p.name).unwrap_or(0);
                w.add_reader(&p.name, &mut h, hint, method).map_err(|e| {
                    ModelError::with_details(
                        codes::SAVE_FAILED,
                        format!("cannot copy entry `{}`: {e}", p.name),
                        json!({ "reason": fsx::reason(&e), "entry": p.name }),
                    )
                })?;
                let sha = fsx::hex(&h.hasher.finalize());
                if layout::classify(&p.name).listed() {
                    listed.push(ManifestEntry { path: p.name.clone(), size: h.n, sha256: sha });
                }
            }
        }
    }
    let manifest = manifest_for(input, Some(listed));
    w.add_bytes(layout::MANIFEST, manifest.to_canonical()?.as_bytes(), Method::Deflate).map_err(io)?;
    let buf = w.finish().map_err(io)?;
    let file = buf.into_inner().map_err(|e| fsx::save_error(e.error(), out, "cannot write"))?;
    file.sync_all().map_err(|e| fsx::save_error(&e, out, "cannot flush"))?;
    Ok(manifest)
}

/// SAVE-01 verification: re-open the package and check every listed entry's size and SHA-256.
///
/// # Errors
/// `project.saveFailed` with `details.reason: "verifyFailed"`.
pub fn verify_package(path: &Path) -> Result<(), ModelError> {
    let fail = |m: String| {
        ModelError::with_details(
            codes::SAVE_FAILED,
            m,
            json!({ "reason": "verifyFailed", "path": path.to_string_lossy() }),
        )
    };
    let z = ZipArchive::open(path, Some(PACKAGE_MIMETYPE), Limits::default())
        .map_err(|e| fail(format!("re-opening failed: {}", e.message())))?;
    let m = Manifest::parse(&z.read(layout::MANIFEST, 64 << 20).map_err(|e| fail(e.message().to_owned()))?)
        .map_err(|e| fail(e.message().to_owned()))?;
    for e in m.entries.iter().flatten() {
        let r = z.reader(&e.path, e.size).map_err(|x| fail(x.message().to_owned()))?;
        let (h, n) = fsx::sha256_reader(r).map_err(|x| fail(format!("`{}`: {x}", e.path)))?;
        if h != e.sha256 || n != e.size {
            return Err(fail(format!("entry `{}` does not match the manifest", e.path)));
        }
    }
    Ok(())
}

/// A previous version staged in the backups folder, not yet recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedBackup {
    /// The backup file.
    pub path: PathBuf,
}

/// Where the previous version goes when a save replaces it (BAK-01/BAK-02). Staging never touches the previous version:
/// a package is hard-linked into the backups folder (so the final rename-over *moves* it there) or copied across
/// volumes; a folder is snapshotted as a package. Only a successful save commits the backup (index, pruning); a failed
/// save discards it.
pub trait BackupSink {
    /// Stages the previous package file.
    ///
    /// # Errors
    /// I/O errors; the save then fails and nothing is replaced.
    fn stage_package(&self, previous: &Path) -> Result<Option<StagedBackup>, ModelError>;
    /// Stages a package snapshot of a previous folder-form version.
    ///
    /// # Errors
    /// I/O errors.
    fn stage_folder(&self, previous: &Container) -> Result<Option<StagedBackup>, ModelError>;
    /// Records a staged backup after a successful save; returns its ID.
    ///
    /// # Errors
    /// I/O errors of the backup index (the save itself succeeded).
    fn commit(&self, staged: StagedBackup) -> Result<String, ModelError>;
    /// Removes a staged backup after a failed save.
    fn discard(&self, staged: StagedBackup);
}

/// The outcome of a save.
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    /// The manifest written.
    pub manifest: Manifest,
    /// Entries written (folder form: changed entries only).
    pub written: Vec<String>,
    /// Entries removed (folder form).
    pub removed: Vec<String>,
    /// Total bytes of the result.
    pub bytes: u64,
    /// The backup of the previous version.
    pub backup_id: Option<String>,
}

/// Saves a package atomically (SAVE-01): temporary file in the destination folder, flushed, verified, then renamed
/// over the destination; the previous version goes to the backup sink first. On any failure the temporary file is
/// deleted and the destination is untouched.
///
/// # Errors
/// `project.saveFailed` (`details.reason`: `diskFull`, `fileInUse`, `verifyFailed`, `ioError`),
/// `project.permissionDenied`.
pub fn save_package(
    target: &Path,
    input: &SaveInput<'_>,
    backup: Option<&dyn BackupSink>,
) -> Result<Saved, ModelError> {
    if let Some(dir) = target.parent()
        && !dir.as_os_str().is_empty()
    {
        fs::create_dir_all(dir).map_err(|e| fsx::save_error(&e, dir, "cannot create the folder"))?;
    }
    let planned = plan(input)?;
    let tmp = fsx::temp_sibling(target);
    let mut staged: Option<StagedBackup> = None;
    let result = (|| -> Result<(Manifest, u64), ModelError> {
        let manifest = write_package(&tmp, input, &planned, false)?;
        verify_package(&tmp)?;
        let bytes = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
        if target.is_file()
            && let Some(sink) = backup
        {
            staged = sink.stage_package(target)?;
        }
        fs::rename(&tmp, target).map_err(|e| fsx::save_error(&e, target, "cannot replace"))?;
        if let Some(dir) = target.parent() {
            fsx::sync_dir(dir);
        }
        Ok((manifest, bytes))
    })();
    match result {
        Ok((manifest, bytes)) => {
            let backup_id = match (staged, backup) {
                (Some(st), Some(sink)) => sink.commit(st).ok(),
                _ => None,
            };
            Ok(Saved {
                manifest,
                written: planned.iter().map(|p| p.name.clone()).collect(),
                removed: Vec::new(),
                bytes,
                backup_id,
            })
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            if let (Some(st), Some(sink)) = (staged, backup) {
                sink.discard(st);
            }
            Err(e)
        }
    }
}

fn entry_file(root: &Path, name: &str) -> PathBuf {
    name.split('/').fold(root.to_path_buf(), |p, s| p.join(s))
}

/// FMT-03: a new folder-form project gets a `.gitignore` (if none exists) and a `.gitattributes`.
fn write_git_files(root: &Path) -> Result<(), ModelError> {
    let gi = root.join(".gitignore");
    if !gi.exists() {
        fsx::write_atomic(&gi, b"cache/\nsession.json\n*.lock\n", fsx::save_error)?;
    }
    let ga = root.join(".gitattributes");
    if !ga.exists() {
        fsx::write_atomic(&ga, b"*.png binary\nmedia/** binary\ntextures/** binary\n", fsx::save_error)?;
    }
    Ok(())
}

/// Options of a folder save.
#[derive(Debug, Clone, Copy, Default)]
pub struct FolderOptions {
    /// Write `.gitignore`/`.gitattributes` when the folder is created (FMT-03; not for recovery copies).
    pub git_files: bool,
    /// Stop after writing the temporaries and the journal (tests of SAVE-02 recovery).
    pub crash_after_journal: bool,
    /// Stop after this many renames (tests of SAVE-02 recovery from every point).
    pub crash_after_renames: Option<usize>,
}

/// Saves the folder form (SAVE-02): changed entries are written to temporary siblings and flushed, entries about to be
/// replaced get old copies, the journal is written (`journal.rs`), the temporaries are renamed over their targets,
/// removed entries are moved to their old copies, `manifest.json` is written last, the journal is deleted, then the old
/// copies. Unchanged entries are not rewritten (LAY-04).
///
/// # Errors
/// `project.saveFailed`, `project.permissionDenied`.
#[allow(clippy::too_many_lines)]
pub fn save_folder(
    root: &Path,
    input: &SaveInput<'_>,
    backup: Option<&dyn BackupSink>,
    opts: FolderOptions,
) -> Result<Saved, ModelError> {
    let existed = root.is_dir();
    if root.exists() && !existed {
        return Err(ModelError::with_details(
            codes::SAVE_FAILED,
            format!("`{}` exists and is not a folder", root.display()),
            json!({ "reason": "ioError", "path": root.to_string_lossy() }),
        ));
    }
    fs::create_dir_all(root).map_err(|e| fsx::save_error(&e, root, "cannot create the folder"))?;
    if !existed && opts.git_files {
        write_git_files(root)?;
    }
    let existing = if existed { Some(FolderIndex::scan(root, Limits::default())?) } else { None };
    let mut staged = None;
    if let (Some(sink), Some(idx)) = (backup, &existing)
        && idx.files().contains_key(layout::PROJECT)
    {
        staged = sink.stage_folder(&Container::Folder(std::sync::Arc::new(idx.clone())))?;
    }
    let discard = |staged: Option<StagedBackup>| {
        if let (Some(st), Some(sink)) = (staged, backup) {
            sink.discard(st);
        }
    };
    let planned = plan(input)?;
    let mimetype =
        Planned { name: layout::MIMETYPE.to_owned(), content: Content::Bytes(PACKAGE_MIMETYPE.as_bytes().to_vec()) };
    let manifest = manifest_for(input, None);
    let manifest_text = manifest.to_canonical()?;
    let mut replaces: Vec<journal::Replace> = Vec::new();
    let mut written = Vec::new();
    let mut total = 0u64;
    let mut temporaries: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<(), ModelError> {
        // FMT-01 (C13-R1-12): the folder holds the same entries as the package, `mimetype` included.
        let all: Vec<(String, Option<&Planned>)> = std::iter::once((layout::MIMETYPE.to_owned(), Some(&mimetype)))
            .chain(planned.iter().map(|p| (p.name.clone(), Some(p))))
            .chain(std::iter::once((layout::MANIFEST.to_owned(), None)))
            .collect();
        for (name, p) in all {
            let target = entry_file(root, &name);
            let (bytes_sha, size): (Option<(Vec<u8>, String)>, u64) = match p.map(|p| &p.content) {
                Some(Content::Bytes(b)) => {
                    let h = sha256(b);
                    let n = b.len() as u64;
                    (Some((b.clone(), h)), n)
                }
                None => {
                    let b = manifest_text.as_bytes().to_vec();
                    let h = sha256(&b);
                    let n = b.len() as u64;
                    (Some((b, h)), n)
                }
                Some(Content::Blob { .. }) => (None, input.blobs.size(&name).unwrap_or(0)),
            };
            total = total.saturating_add(size);
            // Unchanged entries are left alone (LAY-04).
            let same = match (&bytes_sha, existing.as_ref().and_then(|e| e.files().get(&name))) {
                (Some((_, h)), Some(len)) if *len == size => {
                    File::open(&target).ok().and_then(|f| fsx::sha256_reader(f).ok()).is_some_and(|(x, _)| x == *h)
                }
                (None, Some(len)) if *len == size => {
                    // Blob: compare content with the store's bytes.
                    let a = File::open(&target).ok().and_then(|f| fsx::sha256_reader(f).ok());
                    let b =
                        input.blobs.reader(&name, blob_limit(Some(size))).ok().and_then(|r| fsx::sha256_reader(r).ok());
                    a.is_some() && a == b
                }
                _ => false,
            };
            if same {
                continue;
            }
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir).map_err(|e| fsx::save_error(&e, dir, "cannot create the folder"))?;
            }
            let tmp = fsx::temp_sibling(&target);
            temporaries.push(tmp.clone());
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(|e| fsx::save_error(&e, &tmp, "cannot create"))?;
            let sha = match bytes_sha {
                Some((b, h)) => {
                    f.write_all(&b).map_err(|e| fsx::save_error(&e, &tmp, "cannot write"))?;
                    h
                }
                None => {
                    let mut r = input.blobs.reader(&name, blob_limit(Some(size)))?;
                    let mut h = HashingReader { inner: &mut r, hasher: <sha2::Sha256 as sha2::Digest>::new(), n: 0 };
                    io::copy(&mut h, &mut f).map_err(|e| fsx::save_error(&e, &tmp, "cannot write"))?;
                    fsx::hex(&sha2::Digest::finalize(h.hasher))
                }
            };
            f.sync_all().map_err(|e| fsx::save_error(&e, &tmp, "cannot flush"))?;
            let rel = tmp
                .strip_prefix(root)
                .map(|p| {
                    p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
                })
                .unwrap_or_default();
            let new =
                journal::Version { file: rel, size: fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0), sha256: sha };
            // C13-R1-03: the previous content stays recoverable until the save is complete.
            let old = if target.is_file() {
                let v = journal::keep_old(root, &name)?;
                temporaries.push(journal::path(root, &v.file));
                Some(v)
            } else {
                None
            };
            replaces.push(journal::Replace { target: name.clone(), new, old });
            written.push(name);
        }
        Ok(())
    })();
    if let Err(e) = result {
        for t in &temporaries {
            let _ = fs::remove_file(t);
        }
        discard(staged);
        return Err(e);
    }
    // Entries of deleted objects disappear (LAY-03); entries outside the reserved names stay (ZIP-04).
    let keep: BTreeSet<&str> =
        planned.iter().map(|p| p.name.as_str()).chain(std::iter::once(layout::MIMETYPE)).collect();
    let deletions: Vec<String> = existing
        .as_ref()
        .map(|idx| {
            idx.files()
                .keys()
                .filter(|n| {
                    let k = layout::classify(n);
                    !keep.contains(n.as_str())
                        && !matches!(k, Kind::Unknown | Kind::Cache | Kind::Manifest | Kind::Session)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    // Manifest last.
    replaces.sort_by_key(|r| r.target == layout::MANIFEST);
    let mut removes = Vec::new();
    for d in &deletions {
        let p = entry_file(root, d);
        match File::open(&p).and_then(fsx::sha256_reader) {
            Ok((sha256, size)) => {
                removes.push(journal::Remove {
                    target: d.clone(),
                    old: journal::Version { file: journal::old_name(d), size, sha256 },
                });
            }
            Err(e) => {
                for t in &temporaries {
                    let _ = fs::remove_file(t);
                }
                discard(staged);
                return Err(fsx::save_error(&e, &p, "cannot read"));
            }
        }
    }
    let plan = journal::Journal { replace: replaces, remove: removes };
    let journal = root.join(layout::JOURNAL);
    if !plan.replace.is_empty() || !plan.remove.is_empty() {
        let text = tt_model::canonical::to_canonical_string(&plan.to_json())?;
        if let Err(e) = fsx::write_atomic(&journal, text.as_bytes(), fsx::save_error) {
            for t in &temporaries {
                let _ = fs::remove_file(t);
            }
            discard(staged);
            return Err(e);
        }
        if opts.crash_after_journal {
            return Err(ModelError::with_details(
                codes::SAVE_FAILED,
                "simulated crash after the journal",
                json!({ "reason": "ioError" }),
            ));
        }
        // From here on the journal guarantees completion or rollback on the next open (SAVE-02).
        if !journal::apply(root, &plan, opts.crash_after_renames)? {
            return Err(ModelError::with_details(
                codes::SAVE_FAILED,
                "simulated crash during the renames",
                json!({ "reason": "ioError" }),
            ));
        }
        fs::remove_file(&journal).map_err(|e| fsx::save_error(&e, &journal, "cannot remove the journal"))?;
        fsx::sync_dir(root);
        journal::discard_old(root, &plan);
    }
    let backup_id = match (staged, backup) {
        (Some(st), Some(sink)) => sink.commit(st).ok(),
        _ => None,
    };
    Ok(Saved { manifest, written, removed: deletions, bytes: total, backup_id })
}

/// SAVE-02 on open: a leftover journal means an interrupted save. It is validated before anything changes, then the
/// save is completed or rolled back (`journal.rs`); the open reports `project.saveInterrupted`. A read-only open
/// changes nothing and only reports it.
///
/// # Errors
/// `project.invalidFormat` (`reason`: `journal` for a malformed journal, `unrecoverableSave` when neither version is
/// complete; nothing is changed in either case); I/O errors while completing or rolling back.
pub fn recover_journal(root: &Path, read_only: bool, issues: &mut Issues) -> Result<(), ModelError> {
    let path = root.join(layout::JOURNAL);
    if !path.is_file() {
        return Ok(());
    }
    let text = fsx::read_limited(&path, 64 << 20).map_err(|_| {
        crate::zip::invalid("journal", "the save journal cannot be read; nothing was changed", Some(layout::JOURNAL))
    })?;
    let value = std::str::from_utf8(&text).ok().and_then(|t| tt_model::json_text::parse(t).ok()).ok_or_else(|| {
        crate::zip::invalid("journal", "the save journal is not JSON; nothing was changed", Some(layout::JOURNAL))
    })?;
    let plan = journal::Journal::parse(&value)?;
    if read_only {
        issues.warning(
            codes::SAVE_INTERRUPTED,
            None,
            Some(layout::JOURNAL),
            "a previous save of this folder was interrupted; the project is open read-only, so nothing was completed",
            json!({ "completed": false }),
        );
        return Ok(());
    }
    let outcome = journal::recover(root, &plan)?;
    fs::remove_file(&path).map_err(|e| fsx::save_error(&e, &path, "cannot remove the journal"))?;
    fsx::sync_dir(root);
    let completed = outcome == journal::Outcome::Completed;
    issues.warning(
        codes::SAVE_INTERRUPTED,
        None,
        Some(layout::JOURNAL),
        if completed {
            "a previous save of this folder was interrupted; it was completed"
        } else {
            "a previous save of this folder was interrupted; the previous version was restored"
        },
        json!({ "completed": completed }),
    );
    Ok(())
}

/// Writes any container's entries as a package (backups of folder-form projects, BAK-02; form conversion).
///
/// # Errors
/// I/O and read errors.
pub fn container_to_package(source: &Container, out: &Path) -> Result<u64, ModelError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| fsx::save_error(&e, out, "cannot create"))?;
    let mut w = ZipWriter::new(BufWriter::new(file));
    let io = |e: io::Error| fsx::save_error(&e, out, "cannot write");
    w.add_bytes(layout::MIMETYPE, PACKAGE_MIMETYPE.as_bytes(), Method::Stored).map_err(io)?;
    let mut listed = Vec::new();
    let mut manifest: Option<Manifest> = None;
    for n in source.names() {
        if n == layout::MIMETYPE {
            continue;
        }
        if n == layout::MANIFEST {
            manifest = source.read(&n, 64 << 20).ok().and_then(|b| Manifest::parse(&b).ok());
            continue;
        }
        use sha2::Digest;
        let size = source.size(&n).unwrap_or(0);
        let r = source.reader(&n, size)?;
        let mut h = HashingReader { inner: r, hasher: sha2::Sha256::new(), n: 0 };
        w.add_reader(&n, &mut h, size, Method::for_name(&n)).map_err(io)?;
        if layout::classify(&n).listed() {
            listed.push(ManifestEntry { path: n.clone(), size: h.n, sha256: fsx::hex(&h.hasher.finalize()) });
        }
    }
    let mut m = manifest.unwrap_or_else(|| Manifest::new("animpkg", None, None, crate::time::now_iso()));
    m.entries = Some(listed);
    w.add_bytes(layout::MANIFEST, m.to_canonical()?.as_bytes(), Method::Deflate).map_err(io)?;
    let buf = w.finish().map_err(io)?;
    let file = buf.into_inner().map_err(|e| fsx::save_error(e.error(), out, "cannot write"))?;
    file.sync_all().map_err(|e| fsx::save_error(&e, out, "cannot flush"))?;
    Ok(fs::metadata(out).map(|m| m.len()).unwrap_or(0))
}

/// Saves in a form, at a path (the shared entry point of save, save-as, export and recovery copies).
///
/// # Errors
/// As [`save_package`] and [`save_folder`].
pub fn save(
    target: &Path,
    form: Form,
    input: &SaveInput<'_>,
    backup: Option<&dyn BackupSink>,
    git_files: bool,
) -> Result<Saved, ModelError> {
    match form {
        Form::Package => save_package(target, input, backup),
        Form::Folder => save_folder(target, input, backup, FolderOptions { git_files, ..FolderOptions::default() }),
    }
}
