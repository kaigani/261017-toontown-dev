//! The manifest (10 §6.4) and format versions (10 §6.6 VER-01/VER-02).

use std::fmt;

use serde_json::{Map, Value, json};
use tt_model::ModelError;

use crate::codes;

/// Media type of project packages (10 §6.1; D14 placeholder).
pub const PACKAGE_MIMETYPE: &str = "application/x-animpkg+zip";
/// Media type of templates.
pub const TEMPLATE_MIMETYPE: &str = "application/x-animtpl+zip";
/// Media type of palette archives.
pub const PALETTE_MIMETYPE: &str = "application/x-animpal+zip";
/// Application name written as the manifest generator.
pub const GENERATOR: &str = "Toontown";

/// A `MAJOR.MINOR` format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
}

impl Version {
    /// Parses `MAJOR.MINOR` (digits only, no leading `+`, at most 9 digits each).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (a, b) = s.split_once('.')?;
        let digits = |x: &str| !x.is_empty() && x.len() <= 9 && x.bytes().all(|c| c.is_ascii_digit());
        if !digits(a) || !digits(b) {
            return None;
        }
        Some(Self { major: a.parse().ok()?, minor: b.parse().ok()? })
    }

    /// The version this build reads and writes ([`tt_model::FORMAT_VERSION`]).
    #[must_use]
    pub fn current() -> Self {
        Self::parse(tt_model::FORMAT_VERSION).unwrap_or(Self { major: 1, minor: 0 })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// One entry of the integrity list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Entry path.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256.
    pub sha256: String,
}

/// A parsed manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// `animpkg`, `animtpl`.
    pub format: String,
    /// Writer's format version.
    pub format_version: Version,
    /// Lowest reader version that loads the file without losing meaning.
    pub min_reader_version: Version,
    /// Features a reader must understand.
    pub required_features: Vec<String>,
    /// Informational generator record.
    pub generator: Value,
    /// Project ID.
    pub project_id: Option<String>,
    /// `documentRev` at save time.
    pub document_rev: Option<u64>,
    /// Save time.
    pub saved_at: Option<String>,
    /// Integrity list (package form).
    pub entries: Option<Vec<ManifestEntry>>,
    /// Properties this version does not know (kept and written back).
    pub unknown: Map<String, Value>,
}

/// How a file's version relates to this reader (VER-02, VER-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionRelation {
    /// Same version.
    Same,
    /// Older: migrate in memory.
    Older(Version),
    /// Same major, newer minor: open with a warning, preserve unknown content.
    NewerMinor(Version),
}

fn bad(message: impl Into<String>) -> ModelError {
    ModelError::with_details(codes::INVALID_FORMAT, message, json!({ "reason": "manifest", "entry": "manifest.json" }))
}

impl Manifest {
    /// A manifest for a document written now by this build.
    #[must_use]
    pub fn new(format: &str, project_id: Option<String>, document_rev: Option<u64>, saved_at: String) -> Self {
        Self {
            format: format.to_owned(),
            format_version: Version::current(),
            min_reader_version: Version::current(),
            required_features: Vec::new(),
            generator: generator(),
            project_id,
            document_rev,
            saved_at: Some(saved_at),
            entries: None,
            unknown: Map::new(),
        }
    }

    /// Parses manifest text (strict JSON, 10 §6.4 fields with their types).
    ///
    /// # Errors
    /// `project.invalidFormat` with `details.reason: "manifest"`.
    pub fn parse(text: &[u8]) -> Result<Self, ModelError> {
        let text = std::str::from_utf8(text).map_err(|_| bad("manifest.json is not UTF-8"))?;
        let v: Value = tt_model::json_text::parse(text).map_err(|e| bad(format!("manifest.json: {}", e.message())))?;
        let Value::Object(mut m) = v else { return Err(bad("manifest.json is not an object")) };
        let format = match m.remove("format") {
            Some(Value::String(s)) => s,
            _ => return Err(bad("manifest.json has no `format`")),
        };
        let version = |m: &mut Map<String, Value>, k: &str| -> Result<Option<Version>, ModelError> {
            match m.remove(k) {
                None => Ok(None),
                Some(Value::String(s)) => {
                    Version::parse(&s).map(Some).ok_or_else(|| bad(format!("manifest `{k}` is not MAJOR.MINOR: `{s}`")))
                }
                Some(_) => Err(bad(format!("manifest `{k}` is not a string"))),
            }
        };
        let format_version =
            version(&mut m, "formatVersion")?.ok_or_else(|| bad("manifest.json has no `formatVersion`"))?;
        let min_reader_version = version(&mut m, "minReaderVersion")?.unwrap_or(format_version);
        let required_features = match m.remove("requiredFeatures") {
            None => Vec::new(),
            Some(Value::Array(a)) => a
                .into_iter()
                .map(|x| x.as_str().map(str::to_owned).ok_or_else(|| bad("`requiredFeatures` holds strings")))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(bad("`requiredFeatures` is not an array")),
        };
        let generator = m.remove("generator").unwrap_or(Value::Null);
        let project_id = match m.remove("projectId") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s),
            Some(_) => return Err(bad("`projectId` is not a string")),
        };
        let document_rev = match m.remove("documentRev") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_u64().ok_or_else(|| bad("`documentRev` is not a non-negative integer"))?),
        };
        let saved_at = match m.remove("savedAt") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s),
            Some(_) => return Err(bad("`savedAt` is not a string")),
        };
        let entries = match m.remove("entries") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) => {
                let mut out = Vec::with_capacity(a.len());
                for e in a {
                    let path = e.get("path").and_then(Value::as_str).ok_or_else(|| bad("an entry has no `path`"))?;
                    let size = e.get("size").and_then(Value::as_u64).ok_or_else(|| bad("an entry has no `size`"))?;
                    let sha = e.get("sha256").and_then(Value::as_str).ok_or_else(|| bad("an entry has no `sha256`"))?;
                    if sha.len() != 64 || !sha.bytes().all(|c| c.is_ascii_hexdigit()) {
                        return Err(bad(format!("entry `{path}` has a malformed SHA-256")));
                    }
                    out.push(ManifestEntry { path: path.to_owned(), size, sha256: sha.to_ascii_lowercase() });
                }
                Some(out)
            }
            Some(_) => return Err(bad("`entries` is not an array")),
        };
        Ok(Self {
            format,
            format_version,
            min_reader_version,
            required_features,
            generator,
            project_id,
            document_rev,
            saved_at,
            entries,
            unknown: m,
        })
    }

    /// The manifest as canonical JSON text (10 §6.5; field order of §6.4; unknown properties last, sorted).
    ///
    /// # Errors
    /// Serialisation errors (non-finite numbers in preserved properties).
    pub fn to_canonical(&self) -> Result<String, ModelError> {
        let mut m = Map::new();
        m.insert("format".into(), json!(self.format));
        m.insert("formatVersion".into(), json!(self.format_version.to_string()));
        m.insert("minReaderVersion".into(), json!(self.min_reader_version.to_string()));
        m.insert("requiredFeatures".into(), json!(self.required_features));
        m.insert("generator".into(), self.generator.clone());
        if let Some(p) = &self.project_id {
            m.insert("projectId".into(), json!(p));
        }
        if let Some(r) = self.document_rev {
            m.insert("documentRev".into(), json!(r));
        }
        if let Some(s) = &self.saved_at {
            m.insert("savedAt".into(), json!(s));
        }
        if let Some(entries) = &self.entries {
            let list: Vec<Value> =
                entries.iter().map(|e| json!({ "path": e.path, "size": e.size, "sha256": e.sha256 })).collect();
            m.insert("entries".into(), Value::Array(list));
        }
        let mut unknown: Vec<(&String, &Value)> = self.unknown.iter().collect();
        unknown.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in unknown {
            m.insert(k.clone(), v.clone());
        }
        tt_model::canonical::to_canonical_string(&Value::Object(m))
    }

    /// VER-02: whether this reader may open the file, and how its version relates.
    ///
    /// # Errors
    /// `project.unsupportedVersion` naming the versions or features; `project.invalidFormat` for another format.
    pub fn check(&self, expected_format: &str, known_features: &[&str]) -> Result<VersionRelation, ModelError> {
        if self.format != expected_format {
            return Err(ModelError::with_details(
                codes::INVALID_FORMAT,
                format!("the manifest format is `{}`, expected `{expected_format}`", self.format),
                json!({ "reason": "wrongFormat", "format": self.format }),
            ));
        }
        let reader = Version::current();
        let unknown: Vec<&String> =
            self.required_features.iter().filter(|f| !known_features.contains(&f.as_str())).collect();
        if self.format_version.major > reader.major || self.min_reader_version > reader || !unknown.is_empty() {
            return Err(ModelError::with_details(
                codes::UNSUPPORTED_VERSION,
                format!(
                    "written by format {} (needs reader {}{}); this version reads {reader}",
                    self.format_version,
                    self.min_reader_version,
                    if unknown.is_empty() { String::new() } else { format!(", features {unknown:?}") }
                ),
                json!({
                    "formatVersion": self.format_version.to_string(),
                    "minReaderVersion": self.min_reader_version.to_string(),
                    "readerVersion": reader.to_string(),
                    "unknownFeatures": unknown,
                }),
            ));
        }
        Ok(match self.format_version.cmp(&reader) {
            std::cmp::Ordering::Equal => VersionRelation::Same,
            std::cmp::Ordering::Less => VersionRelation::Older(self.format_version),
            std::cmp::Ordering::Greater => VersionRelation::NewerMinor(self.format_version),
        })
    }
}

/// The generator record of this build (informational, never used for behaviour; 10 §6.4).
#[must_use]
pub fn generator() -> Value {
    json!({ "name": GENERATOR, "version": env!("CARGO_PKG_VERSION"), "platform": platform() })
}

/// `windows-x64`, `macos-arm64`, `linux-x64`, …
#[must_use]
pub fn platform() -> String {
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(Version::parse("1.0"), Some(Version { major: 1, minor: 0 }));
        for bad in ["1", "1.", ".0", "1.0.0", "+1.0", "1.a", "01234567890.1"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert!(Version::parse("1.10") > Version::parse("1.9"));
    }

    #[test]
    fn ver02() {
        let mut m = Manifest::new("animpkg", None, Some(3), "2026-10-19T00:00:00.000Z".into());
        assert_eq!(m.check("animpkg", &[]).unwrap(), VersionRelation::Same);
        m.format_version = Version { major: 1, minor: 4 };
        assert!(matches!(m.check("animpkg", &[]).unwrap(), VersionRelation::NewerMinor(_)));
        m.min_reader_version = Version { major: 1, minor: 2 };
        assert_eq!(m.check("animpkg", &[]).unwrap_err().code(), codes::UNSUPPORTED_VERSION);
        let mut m = Manifest::new("animpkg", None, None, String::new());
        m.format_version = Version { major: 2, minor: 0 };
        assert_eq!(m.check("animpkg", &[]).unwrap_err().code(), codes::UNSUPPORTED_VERSION);
        let mut m = Manifest::new("animpkg", None, None, String::new());
        m.required_features = vec!["hologram".into()];
        assert_eq!(m.check("animpkg", &[]).unwrap_err().code(), codes::UNSUPPORTED_VERSION);
        assert_eq!(m.check("animtpl", &[]).unwrap_err().code(), codes::INVALID_FORMAT);
    }

    #[test]
    fn parse_round_trip_keeps_unknown_fields() {
        let text = br#"{"format":"animpkg","formatVersion":"1.1","minReaderVersion":"1.0","requiredFeatures":[],
            "generator":{"name":"x"},"projectId":"pr_1","documentRev":7,"savedAt":"2026-10-17T09:30:12.345Z",
            "entries":[{"path":"project.json","size":2,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
            "zFuture":{"b":1}}"#;
        let m = Manifest::parse(text).unwrap();
        let again = Manifest::parse(m.to_canonical().unwrap().as_bytes()).unwrap();
        assert_eq!(m, again);
        assert!(m.to_canonical().unwrap().contains("zFuture"));
        assert!(Manifest::parse(b"{\"format\":1}").is_err());
        assert!(Manifest::parse(b"[]").is_err());
    }
}
