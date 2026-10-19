//! Error and issue codes produced by persistence (10 §7.2, §8, §9; 01 §3.2, §3.6, §12.1).

/// Not a package, no `mimetype`, unsafe entry names, unreadable container, missing or invalid `project.json`.
pub const INVALID_FORMAT: &str = "project.invalidFormat";
/// Written by a newer major format, `minReaderVersion` too new or an unknown required feature (VER-02).
pub const UNSUPPORTED_VERSION: &str = "project.unsupportedVersion";
/// The first save of a migrated project needs `allowFormatUpgrade` (VER-03).
pub const FORMAT_UPGRADE: &str = "project.formatUpgrade";
/// The path does not exist.
pub const NOT_FOUND: &str = "project.notFound";
/// The location is not readable or writable.
pub const PERMISSION_DENIED: &str = "project.permissionDenied";
/// Writing failed (`details.reason`: `diskFull`, `fileInUse`, `verifyFailed`, `ioError`; SAVE-05).
pub const SAVE_FAILED: &str = "project.saveFailed";
/// The project was opened read-only (RO-01, LOCK-02).
pub const READ_ONLY: &str = "project.readOnly";
/// The destination exists and `onExists` is `fail`.
pub const EXISTS: &str = "project.exists";
/// Save As into the current project package.
pub const PATH_INSIDE_PROJECT: &str = "project.pathInsideProject";
/// Manifest hash or size differs from the entry (warning).
pub const HASH_MISMATCH: &str = "integrity.hashMismatch";
/// The file was written by a newer minor format version (warning).
pub const NEWER_MINOR: &str = "project.newerMinorVersion";
/// A folder-form save was interrupted (warning; SAVE-02).
pub const SAVE_INTERRUPTED: &str = "project.saveInterrupted";
/// The project is locked by someone else (warning; LOCK-02).
pub const LOCKED_BY_OTHER: &str = "project.lockedByOther";
/// The project's file changed on disk (warning; RO-02).
pub const CHANGED_ON_DISK: &str = "project.changedOnDisk";
/// The original file was saved after the recovery copy (warning; REC-02).
pub const RECOVERY_OLDER: &str = "project.recoveryOlderThanFile";
/// Media referenced by the project is missing (one summary warning; MED-03).
pub const MEDIA_MISSING: &str = "project.mediaMissing";
/// An entry is not valid JSON or fails its schema (10 §7.2).
pub const SCHEMA_VIOLATION: &str = "doc.schemaViolation";
/// A drawing entry is missing or unreadable (10 §7.2).
pub const CONTENT_MISSING: &str = "drawing.contentMissing";
/// Two objects share an ID (10 §7.2).
pub const ID_DUPLICATE: &str = "id.duplicate";
/// A reference does not resolve (10 §7.2).
pub const REF_DANGLING: &str = "ref.dangling";
/// The scene tree is invalid (10 §7.2).
pub const TREE_INVALID: &str = "tree.invalid";
/// An exposure is not canonical or names foreign drawings (10 §7.2).
pub const EXPOSURE_INVALID: &str = "exposure.invalid";
/// A value was out of range and was clamped (10 §7.2).
pub const VALUE_CLAMPED: &str = "doc.valueClamped";
/// A name was invalid or duplicated and was repaired (10 §7.2).
pub const NAME_REPAIRED: &str = "doc.nameRepaired";
/// An unreadable transition was loaded as `none` (10 §4.4).
pub const TRANSITION_INVALID: &str = "storyline.transitionInvalid";
/// An object carries values of a newer minor version and is read-only (VER-02).
pub const UNKNOWN_VALUE: &str = "doc.unknownValue";
/// A path parameter is relative or malformed (01 §12.1 rule 2).
pub const FS_INVALID_PATH: &str = "fs.invalidPath";
/// A file target exists and `onExists` is `fail` (11 §6.2).
pub const FS_EXISTS: &str = "fs.exists";
/// A file could not be read or written (generic I/O).
pub const FS_IO: &str = "fs.ioError";
/// Settings files are corrupt; defaults are used (01 §11).
pub const PREFS_CORRUPT: &str = "prefs.corrupt";
/// Unexpected failure.
pub const INTERNAL: &str = "internal";
