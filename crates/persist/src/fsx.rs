//! File-system helpers: atomic replacement, flushing, typed I/O errors, hashing.
//!
//! **Atomic replacement** (10 SAVE-01, §6.11, 01 §11): the new content is written to a temporary sibling, flushed to
//! stable storage (`sync_all`), then renamed over the target. `std::fs::rename` replaces an existing file atomically on
//! POSIX (`rename(2)`) and on Windows (`MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`). On POSIX the parent directory
//! is flushed afterwards so the rename itself is durable; Windows offers no directory flush without native calls, and
//! the write-through flag of `MoveFileExW` is not reachable from safe Rust (ADR-0014 §3).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::json;
use sha2::{Digest, Sha256};
use tt_model::ModelError;

use crate::codes;

/// Lower-case hex of bytes.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// SHA-256 (lower-case hex) of bytes.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// SHA-256 (lower-case hex) and length of a stream.
///
/// # Errors
/// The stream's I/O error.
pub fn sha256_reader(mut r: impl Read) -> io::Result<(String, u64)> {
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut n = 0u64;
    loop {
        let k = r.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((hex(&h.finalize()), n))
}

/// A random lower-case hex token of `bytes` random bytes.
#[must_use]
pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::fill(&mut buf).is_err() {
        // Uniqueness, not secrecy, is what temporary names need: fall back to time and a counter.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return format!("{:x}{n:x}", crate::time::now_ms());
    }
    hex(&buf)
}

/// Why an I/O operation failed, as the `details.reason` of 10 SAVE-05.
#[must_use]
pub fn reason(e: &io::Error) -> &'static str {
    match e.kind() {
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => "diskFull",
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => "permissionDenied",
        // ERROR_SHARING_VIOLATION (32) and ERROR_LOCK_VIOLATION (33) on Windows; EBUSY / ETXTBSY elsewhere.
        _ if matches!(e.raw_os_error(), Some(32 | 33)) => "fileInUse",
        io::ErrorKind::ResourceBusy | io::ErrorKind::ExecutableFileBusy => "fileInUse",
        io::ErrorKind::NotFound => "notFound",
        _ => "ioError",
    }
}

/// A save failure (SAVE-05): `project.permissionDenied` for permission problems, else `project.saveFailed` with
/// `details.reason`.
#[must_use]
pub fn save_error(e: &io::Error, path: &Path, what: &str) -> ModelError {
    let r = reason(e);
    let code = if r == "permissionDenied" { codes::PERMISSION_DENIED } else { codes::SAVE_FAILED };
    ModelError::with_details(
        code,
        format!("{what} `{}`: {e}", path.display()),
        json!({ "path": path.to_string_lossy(), "reason": if r == "permissionDenied" { "permissionDenied" } else { r } }),
    )
}

/// A read failure of a project path: `project.notFound`, `project.permissionDenied` or `project.invalidFormat`.
#[must_use]
pub fn open_error(e: &io::Error, path: &Path) -> ModelError {
    let p = path.to_string_lossy();
    match reason(e) {
        "notFound" => ModelError::with_details(codes::NOT_FOUND, format!("`{p}` does not exist"), json!({ "path": p })),
        "permissionDenied" => ModelError::with_details(
            codes::PERMISSION_DENIED,
            format!("`{p}` cannot be read: {e}"),
            json!({ "path": p, "reason": "permissionDenied" }),
        ),
        r => ModelError::with_details(
            codes::INVALID_FORMAT,
            format!("`{p}` cannot be read: {e}"),
            json!({ "path": p, "reason": r }),
        ),
    }
}

/// A generic file error (`fs.ioError`) for non-project files.
#[must_use]
pub fn fs_error(e: &io::Error, path: &Path, what: &str) -> ModelError {
    ModelError::with_details(
        codes::FS_IO,
        format!("{what} `{}`: {e}", path.display()),
        json!({ "path": path.to_string_lossy(), "reason": reason(e) }),
    )
}

/// A temporary sibling name for `target`: `.<name>.<token>.tmp` in the same folder (10 SAVE-01).
#[must_use]
pub fn temp_sibling(target: &Path) -> PathBuf {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
    dir.join(format!(".{name}.{}.tmp", random_token(6)))
}

/// Flushes a directory's metadata (the renames inside it) to stable storage. POSIX only; see the module docs.
pub fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    {
        if let Ok(f) = File::open(dir) {
            let _ = f.sync_all();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
    }
}

/// Writes `bytes` to `target` atomically: temporary sibling, flush, rename over the target, flush the folder. On
/// failure the temporary file is deleted and the target is untouched.
///
/// # Errors
/// The I/O error, mapped by `map`.
pub fn write_atomic(
    target: &Path,
    bytes: &[u8],
    map: impl Fn(&io::Error, &Path, &str) -> ModelError,
) -> Result<(), ModelError> {
    if let Some(dir) = target.parent()
        && !dir.as_os_str().is_empty()
    {
        fs::create_dir_all(dir).map_err(|e| map(&e, dir, "cannot create the folder"))?;
    }
    let tmp = temp_sibling(target);
    let written = (|| -> io::Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, target)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(map(&e, target, "cannot write"));
    }
    if let Some(dir) = target.parent() {
        sync_dir(dir);
    }
    Ok(())
}

/// Reads a whole file, refusing files larger than `limit` bytes.
///
/// # Errors
/// The I/O error, or `InvalidData` for a file over the limit.
pub fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let f = File::open(path)?;
    let mut out = Vec::new();
    f.take(limit.saturating_add(1)).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("file larger than {limit} bytes")));
    }
    Ok(out)
}

/// Copies a file's bytes to another path (creating parent folders), flushing the copy.
///
/// # Errors
/// The I/O error.
pub fn copy_file(from: &Path, to: &Path) -> io::Result<u64> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut src = File::open(from)?;
    let mut dst = OpenOptions::new().write(true).create(true).truncate(true).open(to)?;
    let n = io::copy(&mut src, &mut dst)?;
    dst.sync_all()?;
    Ok(n)
}

/// Moves a file, falling back to copy-and-delete across volumes.
///
/// # Errors
/// The I/O error.
pub fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_file(from, to)?;
            fs::remove_file(from)
        }
    }
}

/// Whether `inner` is `outer` or inside it (after resolving both as far as they exist; 01 §3.6
/// `project.pathInsideProject`, 11 §6.2 symbolic links resolved before the check).
#[must_use]
pub fn is_within(inner: &Path, outer: &Path) -> bool {
    let a = resolve(inner);
    let b = resolve(outer);
    a.starts_with(&b)
}

/// The path with its longest existing prefix canonicalised (symbolic links resolved) and the rest appended.
#[must_use]
pub fn resolve(p: &Path) -> PathBuf {
    let mut existing = p.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(c) = existing.canonicalize() {
            let mut out = c;
            for r in rest.iter().rev() {
                out.push(r);
            }
            return out;
        }
        match (existing.parent().map(Path::to_path_buf), existing.file_name().map(std::ffi::OsStr::to_os_string)) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                rest.push(name);
                existing = parent;
            }
            _ => return p.to_path_buf(),
        }
    }
}

/// A path parameter must be absolute (01 §12.1 rule 2: relative or malformed paths fail with `fs.invalidPath`).
///
/// # Errors
/// `fs.invalidPath`.
pub fn absolute(path: &str, pointer: &str) -> Result<PathBuf, ModelError> {
    let bad = |why: &str| {
        ModelError::with_details(
            codes::FS_INVALID_PATH,
            format!("`{path}` is not a valid absolute path: {why}"),
            json!({ "path": path, "pointer": pointer }),
        )
    };
    if path.is_empty() {
        return Err(bad("empty"));
    }
    if path.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(bad("control characters"));
    }
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err(bad("relative"));
    }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(bad("`..` segments"));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temporaries() {
        let dir = std::env::temp_dir().join(format!("tt-fsx-{}", random_token(4)));
        let target = dir.join("a/b.json");
        write_atomic(&target, b"one", fs_error).unwrap();
        write_atomic(&target, b"two", fs_error).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(dir.join("a")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absolute_paths_only() {
        assert!(absolute("relative/x", "/path").is_err());
        assert!(absolute("", "/path").is_err());
        #[cfg(windows)]
        assert!(absolute("C:\\x\\..\\y", "/path").is_err());
        #[cfg(windows)]
        assert!(absolute("C:\\x\\y", "/path").is_ok());
        #[cfg(unix)]
        assert!(absolute("/x/y", "/path").is_ok());
    }
}
