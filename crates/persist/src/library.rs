//! Libraries (10 §6.11), templates (10 §6.9) and palette exchange files (10 §6.10).
//!
//! **`library.json`** is `{ "format": "animlib", "formatVersion", "id", "name", "kind", "root": [...] }`: the model's
//! library object (envelope, `kind`, `formatVersion`, `meta`, `ext`) with its `children` written as `root`, so the
//! project-local library under `library/` and a library folder on disk use one format (ADR-0014 §6). The hierarchy
//! exists only in `library.json`; files are flat under `files/`.
//!
//! **Templates** are zip archives (ZIP-01…ZIP-06 apply) with `mimetype`, a manifest of format `animtpl`,
//! `template.json`, `content.json` (an interchange bundle, 10 §6.8) and the binary entries `content.json` references.
//! **Palette files** are an archive with `palettes.json` and `textures/` (the `mimetype` entry is written, not
//! required) or a bare JSON file with the same content.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use tt_model::ModelError;

use crate::codes;
use crate::container::{BlobData, Blobs};
use crate::fsx;
use crate::manifest::{Manifest, ManifestEntry, PALETTE_MIMETYPE, TEMPLATE_MIMETYPE};
use crate::zip::{Limits, Method, ZipArchive, ZipWriter, invalid};

/// The model library object → `animlib` text form.
#[must_use]
pub fn to_animlib(library: Value) -> Value {
    let Value::Object(m) = library else { return library };
    let mut out = Map::new();
    out.insert("format".into(), json!("animlib"));
    for (k, v) in m {
        if k == "children" {
            out.insert("root".into(), v);
        } else {
            out.insert(k, v);
        }
    }
    Value::Object(out)
}

/// The `animlib` form → the model library object (a file without `format` is read as the model object).
#[must_use]
pub fn from_animlib(file: Value) -> Value {
    let Value::Object(m) = file else { return file };
    if m.get("format").and_then(Value::as_str) != Some("animlib") {
        return Value::Object(m);
    }
    let mut out = Map::new();
    for (k, v) in m {
        match k.as_str() {
            "format" => {}
            "root" => {
                out.insert("children".into(), v);
            }
            _ => {
                out.insert(k, v);
            }
        }
    }
    if !out.contains_key("type") {
        out.insert("type".into(), json!("library"));
    }
    Value::Object(out)
}

/// A library folder read from disk.
#[derive(Debug, Clone)]
pub struct LibraryFolder {
    /// The library object (model form).
    pub library: tt_model::entities::library::Library,
    /// Items whose file is missing or unreadable (`broken`, 08 §1.1): item ID → file.
    pub broken: BTreeMap<String, String>,
}

/// Reads a library folder (10 §6.11).
///
/// # Errors
/// `fs.ioError` when `library.json` cannot be read; `project.invalidFormat` (`reason: "library"`) when it is not a
/// valid library.
pub fn read_library_folder(folder: &Path) -> Result<LibraryFolder, ModelError> {
    let index = folder.join("library.json");
    let bytes = fsx::read_limited(&index, 256 << 20).map_err(|e| fsx::fs_error(&e, &index, "cannot read"))?;
    let bad = |m: String| {
        ModelError::with_details(
            codes::INVALID_FORMAT,
            m,
            json!({ "reason": "library", "path": index.to_string_lossy() }),
        )
    };
    let text = String::from_utf8(bytes).map_err(|_| bad("library.json is not UTF-8".into()))?;
    let v: Value = tt_model::json_text::parse(&text).map_err(|e| bad(e.message().to_owned()))?;
    if let Some(fv) = v.get("formatVersion").and_then(Value::as_str) {
        let version = crate::manifest::Version::parse(fv).ok_or_else(|| bad(format!("formatVersion `{fv}`")))?;
        if version.major > crate::manifest::Version::current().major {
            return Err(ModelError::with_details(
                codes::UNSUPPORTED_VERSION,
                format!("the library was written by format {version}"),
                json!({ "formatVersion": fv }),
            ));
        }
    }
    let library: tt_model::entities::library::Library = tt_model::object::from_json(&from_animlib(v))
        .map_err(|e| bad(e.first().map(|x| x.message().to_owned()).unwrap_or_default()))?;
    let mut broken = BTreeMap::new();
    let raw = tt_model::object::to_json(&library)?;
    collect_items(&raw, &mut |id, file| {
        let ok = crate::zip::safe_name(file) && folder.join("files").join(file).is_file();
        if !ok {
            broken.insert(id.to_owned(), file.to_owned());
        }
    });
    Ok(LibraryFolder { library, broken })
}

fn collect_items(v: &Value, f: &mut dyn FnMut(&str, &str)) {
    for c in v.get("children").and_then(Value::as_array).into_iter().flatten() {
        if c.get("type").and_then(Value::as_str) == Some("libraryItem")
            && let (Some(id), Some(file)) = (c.get("id").and_then(Value::as_str), c.get("file").and_then(Value::as_str))
        {
            f(id, file);
        }
        collect_items(c, f);
    }
}

/// Writes `library.json` of a library folder atomically, in canonical JSON (10 §6.11).
///
/// # Errors
/// I/O and serialisation errors.
pub fn write_library_index(folder: &Path, library: &tt_model::entities::library::Library) -> Result<(), ModelError> {
    let v = to_animlib(tt_model::object::to_json(library)?);
    let text = tt_model::canonical::to_canonical_string(&v)?;
    fsx::write_atomic(&folder.join("library.json"), text.as_bytes(), fsx::fs_error)
}

/// A template file's parts (10 §6.9).
#[derive(Debug, Clone)]
pub struct Template {
    /// The manifest (format `animtpl`).
    pub manifest: Manifest,
    /// `template.json`: metadata.
    pub meta: Value,
    /// `content.json`: an interchange bundle.
    pub content: Value,
    /// The binary entries, readable from the archive.
    pub blobs: Blobs,
    /// `thumbnail.png`, when present.
    pub has_thumbnail: bool,
}

/// Writes a template archive.
///
/// # Errors
/// I/O and read errors.
pub fn write_template(
    path: &Path,
    meta: &Value,
    content: &Value,
    binaries: &BTreeMap<String, BlobData>,
    thumbnail: Option<&[u8]>,
) -> Result<(), ModelError> {
    let tmp = fsx::temp_sibling(path);
    let r = (|| -> Result<(), ModelError> {
        let file = File::create(&tmp).map_err(|e| fsx::fs_error(&e, &tmp, "cannot create"))?;
        let mut w = ZipWriter::new(BufWriter::new(file));
        let io = |e: std::io::Error| fsx::fs_error(&e, path, "cannot write");
        w.add_bytes("mimetype", TEMPLATE_MIMETYPE.as_bytes(), Method::Stored).map_err(io)?;
        let mut listed = Vec::new();
        let mut add = |w: &mut ZipWriter<BufWriter<File>>, name: &str, bytes: &[u8]| -> Result<(), ModelError> {
            w.add_bytes(name, bytes, Method::for_name(name)).map_err(io)?;
            listed.push(ManifestEntry { path: name.to_owned(), size: bytes.len() as u64, sha256: fsx::sha256(bytes) });
            Ok(())
        };
        add(&mut w, "template.json", tt_model::canonical::to_canonical_string(meta)?.as_bytes())?;
        add(&mut w, "content.json", tt_model::canonical::to_canonical_string(content)?.as_bytes())?;
        let mut store = Blobs::default();
        for (n, d) in binaries {
            store.insert(n.clone(), d.clone());
        }
        for n in binaries.keys() {
            if !crate::zip::safe_name(n) {
                return Err(invalid("unsafeEntry", format!("unsafe entry name `{n}`"), Some(n)));
            }
            let bytes = store.read(n, 1 << 40)?;
            add(&mut w, n, &bytes)?;
        }
        if let Some(t) = thumbnail {
            add(&mut w, "thumbnail.png", t)?;
        }
        let mut m = Manifest::new(
            "animtpl",
            meta.get("id").and_then(Value::as_str).map(str::to_owned),
            None,
            crate::time::now_iso(),
        );
        m.entries = Some(listed);
        w.add_bytes("manifest.json", m.to_canonical()?.as_bytes(), Method::Deflate).map_err(io)?;
        let buf = w.finish().map_err(io)?;
        let f = buf.into_inner().map_err(|e| fsx::fs_error(e.error(), path, "cannot write"))?;
        f.sync_all().map_err(|e| fsx::fs_error(&e, path, "cannot flush"))?;
        fs::rename(&tmp, path).map_err(|e| fsx::fs_error(&e, path, "cannot replace"))
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Reads a template archive (all container rules apply; the manifest's integrity list is checked).
///
/// # Errors
/// `project.invalidFormat`, `project.unsupportedVersion`, I/O errors.
pub fn read_template(path: &Path) -> Result<Template, ModelError> {
    let z = ZipArchive::open(path, Some(TEMPLATE_MIMETYPE), Limits::default())?;
    let manifest = Manifest::parse(&z.read("manifest.json", 64 << 20)?)?;
    manifest.check("animtpl", crate::load::KNOWN_FEATURES)?;
    for e in manifest.entries.iter().flatten() {
        let r = z.reader(&e.path, e.size.saturating_add(crate::load::SIZE_MARGIN))?;
        let (h, n) = fsx::sha256_reader(r).map_err(|x| crate::zip::entry_error(&e.path, &x))?;
        if h != e.sha256 || n != e.size {
            return Err(invalid(
                "corrupt",
                format!("template entry `{}` does not match its manifest", e.path),
                Some(&e.path),
            ));
        }
    }
    let json_entry = |name: &str| -> Result<Value, ModelError> {
        let b = z.read(name, crate::load::JSON_LIMIT)?;
        let t = String::from_utf8(b).map_err(|_| invalid("corrupt", format!("`{name}` is not UTF-8"), Some(name)))?;
        tt_model::json_text::parse(&t).map_err(|e| invalid("corrupt", format!("`{name}`: {}", e.message()), Some(name)))
    };
    let meta = json_entry("template.json")?;
    let content = json_entry("content.json")?;
    let has_thumbnail = z.entry("thumbnail.png").is_some();
    Ok(Template {
        manifest,
        meta,
        content,
        blobs: Blobs::from_container(crate::container::Container::Package(std::sync::Arc::new(z))),
        has_thumbnail,
    })
}

/// A palette exchange file's content (10 §6.10, 03 §12).
#[derive(Debug, Clone)]
pub struct PaletteFile {
    /// `palettes.json`: `{format: "animpal", formatVersion, author, copyright, palettes}`.
    pub content: Value,
    /// Texture entries (`textures/…`), when the file is an archive.
    pub textures: BTreeMap<String, Vec<u8>>,
}

/// Writes a palette file: an archive when there are textures (or `archive` is set), else bare JSON.
///
/// # Errors
/// I/O and serialisation errors.
pub fn write_palette_file(
    path: &Path,
    content: &Value,
    textures: &BTreeMap<String, Vec<u8>>,
    archive: bool,
) -> Result<(), ModelError> {
    let text = tt_model::canonical::to_canonical_string(content)?;
    if textures.is_empty() && !archive {
        return fsx::write_atomic(path, text.as_bytes(), fsx::fs_error);
    }
    let tmp = fsx::temp_sibling(path);
    let r = (|| -> Result<(), ModelError> {
        let file = File::create(&tmp).map_err(|e| fsx::fs_error(&e, &tmp, "cannot create"))?;
        let mut w = ZipWriter::new(BufWriter::new(file));
        let io = |e: std::io::Error| fsx::fs_error(&e, path, "cannot write");
        w.add_bytes("mimetype", PALETTE_MIMETYPE.as_bytes(), Method::Stored).map_err(io)?;
        w.add_bytes("palettes.json", text.as_bytes(), Method::Deflate).map_err(io)?;
        for (n, b) in textures {
            if !n.starts_with("textures/") || !crate::zip::safe_name(n) {
                return Err(invalid("unsafeEntry", format!("texture entries live under textures/: `{n}`"), Some(n)));
            }
            w.add_bytes(n, b, Method::for_name(n)).map_err(io)?;
        }
        let buf = w.finish().map_err(io)?;
        let f = buf.into_inner().map_err(|e| fsx::fs_error(e.error(), path, "cannot write"))?;
        f.sync_all().map_err(|e| fsx::fs_error(&e, path, "cannot flush"))?;
        fs::rename(&tmp, path).map_err(|e| fsx::fs_error(&e, path, "cannot replace"))
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Reads a palette file: an archive (the `mimetype` entry is not required) or bare JSON.
///
/// # Errors
/// `project.invalidFormat` (`reason: "palette"`), container errors, I/O errors.
pub fn read_palette_file(path: &Path) -> Result<PaletteFile, ModelError> {
    let mut head = [0u8; 4];
    let n = File::open(path).and_then(|mut f| f.read(&mut head)).map_err(|e| fsx::fs_error(&e, path, "cannot read"))?;
    let bad = |m: String| {
        ModelError::with_details(
            codes::INVALID_FORMAT,
            m,
            json!({ "reason": "palette", "path": path.to_string_lossy() }),
        )
    };
    let parse = |b: Vec<u8>| -> Result<Value, ModelError> {
        let t = String::from_utf8(b).map_err(|_| bad("palettes.json is not UTF-8".into()))?;
        let v: Value = tt_model::json_text::parse(&t).map_err(|e| bad(e.message().to_owned()))?;
        if v.get("format").and_then(Value::as_str) != Some("animpal") || !v.get("palettes").is_some_and(Value::is_array)
        {
            return Err(bad("not an animpal palette set".into()));
        }
        Ok(v)
    };
    if n == 4 && head == [0x50, 0x4b, 0x03, 0x04] {
        let z = ZipArchive::open(path, None, Limits::default())?;
        let content = parse(z.read("palettes.json", crate::load::JSON_LIMIT)?)?;
        let mut textures = BTreeMap::new();
        for e in z.entries() {
            if e.name.starts_with("textures/") {
                textures.insert(e.name.clone(), z.read(&e.name, 1 << 30)?);
            }
        }
        Ok(PaletteFile { content, textures })
    } else {
        let b = fsx::read_limited(path, crate::load::JSON_LIMIT).map_err(|e| fsx::fs_error(&e, path, "cannot read"))?;
        Ok(PaletteFile { content: parse(b)?, textures: BTreeMap::new() })
    }
}

/// The folder of a library item's file.
#[must_use]
pub fn item_file(folder: &Path, file: &str) -> PathBuf {
    folder.join("files").join(file)
}
