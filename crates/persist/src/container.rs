//! Reading the entries of either form (10 §6.1 FMT-01): a package archive or a folder holding the same entries as
//! plain files. Both expose the same entry tree; folder control files (`.lock`, `.save-journal.json`, `.git/`,
//! `.gitignore`, …) are not entries. Folder entries must be regular files: symbolic links and other special files are
//! refused (`project.invalidFormat`, `unsafeEntry`), so no entry can escape the folder.
//!
//! [`Blobs`] is the binary-entry store of an open project: the container it was loaded from, plus entries added in the
//! session (embedded media, files carried across a save that replaced the container).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use tt_model::ModelError;

use crate::codes;
use crate::layout;
use crate::zip::{self, Limits, ZipArchive, invalid};

/// The two equivalent forms (10 §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Form {
    /// One zip file.
    Package,
    /// A folder of plain files.
    Folder,
}

impl Form {
    /// `package` or `folder`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Form::Package => "package",
            Form::Folder => "folder",
        }
    }
}

/// The files of a folder-form project: entry name → size.
#[derive(Debug, Clone)]
pub struct FolderIndex {
    root: PathBuf,
    files: BTreeMap<String, u64>,
}

impl FolderIndex {
    /// Walks a folder. Control files are skipped; symbolic links and special files are refused.
    ///
    /// # Errors
    /// `project.invalidFormat` (`unsafeEntry`, `sizeLimit`), or the I/O error of reading the folder.
    pub fn scan(root: &Path, limits: Limits) -> Result<Self, ModelError> {
        let mut files = BTreeMap::new();
        let mut stack = vec![(root.to_path_buf(), String::new(), 0usize)];
        while let Some((dir, prefix, depth)) = stack.pop() {
            if depth > 64 {
                return Err(invalid("unsafeEntry", "folders nested deeper than 64 levels", Some(&prefix)));
            }
            let rd = fs::read_dir(&dir).map_err(|e| crate::fsx::open_error(&e, &dir))?;
            for item in rd {
                let item = item.map_err(|e| crate::fsx::open_error(&e, &dir))?;
                let name = item.file_name();
                let Some(name) = name.to_str() else {
                    return Err(invalid("unsafeEntry", "a file name is not valid Unicode", None));
                };
                let entry = if prefix.is_empty() { name.to_owned() } else { format!("{prefix}/{name}") };
                if prefix.is_empty() && layout::is_folder_control(&entry) {
                    continue;
                }
                let ft = item.file_type().map_err(|e| crate::fsx::open_error(&e, &dir))?;
                if ft.is_symlink() {
                    return Err(invalid(
                        "unsafeEntry",
                        format!("`{entry}` is a symbolic link; folder entries are regular files"),
                        Some(&entry),
                    ));
                }
                if !zip::safe_name(&entry) {
                    return Err(invalid("unsafeEntry", format!("unsafe entry name `{entry}`"), Some(&entry)));
                }
                if ft.is_dir() {
                    stack.push((item.path(), entry, depth + 1));
                } else if ft.is_file() {
                    let len = item.metadata().map_err(|e| crate::fsx::open_error(&e, &item.path()))?.len();
                    files.insert(entry, len);
                    if files.len() > limits.max_entries {
                        return Err(invalid("sizeLimit", "the folder holds too many files", None));
                    }
                } else {
                    return Err(invalid("unsafeEntry", format!("`{entry}` is not a regular file"), Some(&entry)));
                }
            }
        }
        Ok(Self { root: root.to_path_buf(), files })
    }

    /// The folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Entry name → size.
    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, u64> {
        &self.files
    }

    fn file(&self, name: &str) -> Result<PathBuf, ModelError> {
        if !self.files.contains_key(name) || !zip::safe_name(name) {
            return Err(invalid("corrupt", format!("no entry `{name}`"), Some(name)));
        }
        Ok(name.split('/').fold(self.root.clone(), |p, s| p.join(s)))
    }
}

/// An opened container.
#[derive(Debug, Clone)]
pub enum Container {
    /// A package.
    Package(Arc<ZipArchive>),
    /// A folder.
    Folder(Arc<FolderIndex>),
}

impl Container {
    /// Opens a project path: a file is a package (checked against `mimetype`), a folder is the folder form.
    ///
    /// # Errors
    /// `project.notFound`, `project.permissionDenied`, `project.invalidFormat`.
    pub fn open(path: &Path, mimetype: &str, limits: Limits) -> Result<Self, ModelError> {
        let meta = fs::metadata(path).map_err(|e| crate::fsx::open_error(&e, path))?;
        if meta.is_dir() {
            Ok(Container::Folder(Arc::new(FolderIndex::scan(path, limits)?)))
        } else {
            Ok(Container::Package(Arc::new(ZipArchive::open(path, Some(mimetype), limits)?)))
        }
    }

    /// The form.
    #[must_use]
    pub fn form(&self) -> Form {
        match self {
            Container::Package(_) => Form::Package,
            Container::Folder(_) => Form::Folder,
        }
    }

    /// The path opened.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Container::Package(z) => z.path(),
            Container::Folder(f) => f.root(),
        }
    }

    /// Entry names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = match self {
            Container::Package(z) => z.entries().iter().map(|e| e.name.clone()).collect(),
            Container::Folder(f) => f.files.keys().cloned().collect(),
        };
        v.sort();
        v
    }

    /// Whether an entry exists.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.size(name).is_some()
    }

    /// The declared (package) or actual (folder) size of an entry.
    #[must_use]
    pub fn size(&self, name: &str) -> Option<u64> {
        match self {
            Container::Package(z) => z.entry(name).map(|e| e.uncompressed),
            Container::Folder(f) => f.files.get(name).copied(),
        }
    }

    /// A bounded reader of an entry (see [`ZipArchive::reader`]; folder files are bounded the same way).
    ///
    /// # Errors
    /// `project.invalidFormat` for a missing entry or a read failure.
    pub fn reader(&self, name: &str, limit: u64) -> Result<Box<dyn Read + Send>, ModelError> {
        match self {
            Container::Package(z) => Ok(Box::new(z.reader(name, limit)?)),
            Container::Folder(f) => {
                let p = f.file(name)?;
                let file = File::open(&p).map_err(|e| crate::fsx::open_error(&e, &p))?;
                Ok(Box::new(Bounded { inner: file.take(limit.saturating_add(1)), n: 0, limit }))
            }
        }
    }

    /// Reads an entry completely, bounded by `limit`.
    ///
    /// # Errors
    /// `project.invalidFormat` (`sizeLimit`, `corrupt`).
    pub fn read(&self, name: &str, limit: u64) -> Result<Vec<u8>, ModelError> {
        if let Container::Package(z) = self {
            return z.read(name, limit);
        }
        if self.size(name).is_some_and(|s| s > limit) {
            return Err(invalid("sizeLimit", format!("entry `{name}` is larger than {limit} bytes"), Some(name)));
        }
        let mut out = Vec::new();
        self.reader(name, limit)?.read_to_end(&mut out).map_err(|e| zip::entry_error(name, &e))?;
        Ok(out)
    }
}

struct Bounded<R> {
    inner: R,
    n: u64,
    limit: u64,
}

impl<R: Read> Read for Bounded<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let k = self.inner.read(buf)?;
        self.n += k as u64;
        if self.n > self.limit {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "entry exceeds its size limit"));
        }
        Ok(k)
    }
}

/// Bytes of a binary entry held outside the container.
#[derive(Debug, Clone)]
pub enum BlobData {
    /// In memory.
    Bytes(Arc<Vec<u8>>),
    /// A file (session cache, or a linked file being embedded).
    File(PathBuf),
}

/// The binary entries available to an open project (10 MED-01/MED-04/MED-05).
#[derive(Debug, Clone, Default)]
pub struct Blobs {
    base: Option<Container>,
    extra: BTreeMap<String, BlobData>,
}

impl Blobs {
    /// The entries of a container.
    #[must_use]
    pub fn from_container(c: Container) -> Self {
        Self { base: Some(c), extra: BTreeMap::new() }
    }

    /// The container the project was loaded from (or last saved to).
    #[must_use]
    pub fn base(&self) -> Option<&Container> {
        self.base.as_ref()
    }

    /// Adds or replaces an entry held outside the container.
    pub fn insert(&mut self, name: String, data: BlobData) {
        self.extra.insert(name, data);
    }

    /// Entries held outside the container.
    #[must_use]
    pub fn extra(&self) -> &BTreeMap<String, BlobData> {
        &self.extra
    }

    /// Whether an entry is available.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.extra.get(name).is_some_and(|d| match d {
            BlobData::Bytes(_) => true,
            BlobData::File(p) => p.is_file(),
        }) || self.base.as_ref().is_some_and(|b| b.contains(name))
    }

    /// The size of an entry.
    #[must_use]
    pub fn size(&self, name: &str) -> Option<u64> {
        match self.extra.get(name) {
            Some(BlobData::Bytes(b)) => Some(b.len() as u64),
            Some(BlobData::File(p)) => fs::metadata(p).ok().map(|m| m.len()),
            None => self.base.as_ref().and_then(|b| b.size(name)),
        }
    }

    /// A bounded reader of an entry.
    ///
    /// # Errors
    /// `project.invalidFormat` for an entry that is missing or cannot be read.
    pub fn reader(&self, name: &str, limit: u64) -> Result<Box<dyn Read + Send>, ModelError> {
        match self.extra.get(name) {
            Some(BlobData::Bytes(b)) => {
                if b.len() as u64 > limit {
                    return Err(invalid(
                        "sizeLimit",
                        format!("entry `{name}` is larger than {limit} bytes"),
                        Some(name),
                    ));
                }
                Ok(Box::new(io::Cursor::new(b.as_ref().clone())))
            }
            Some(BlobData::File(p)) => {
                let f = File::open(p).map_err(|e| {
                    ModelError::with_details(
                        codes::FS_IO,
                        format!("cannot read `{}`: {e}", p.display()),
                        json!({ "entry": name, "reason": crate::fsx::reason(&e) }),
                    )
                })?;
                Ok(Box::new(Bounded { inner: f.take(limit.saturating_add(1)), n: 0, limit }))
            }
            None => match &self.base {
                Some(b) => b.reader(name, limit),
                None => Err(invalid("corrupt", format!("no entry `{name}`"), Some(name))),
            },
        }
    }

    /// Reads an entry completely.
    ///
    /// # Errors
    /// As [`Blobs::reader`].
    pub fn read(&self, name: &str, limit: u64) -> Result<Vec<u8>, ModelError> {
        let mut out = Vec::new();
        self.reader(name, limit)?.read_to_end(&mut out).map_err(|e| zip::entry_error(name, &e))?;
        Ok(out)
    }
}
