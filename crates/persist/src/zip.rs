//! The zip container of packages, templates and palette archives (10 §6.2, PKWARE APPNOTE 6.3 with ZIP64).
//!
//! **Writing** is deterministic: entries in the order given, the language-encoding (UTF-8) flag on every entry, a fixed
//! DOS timestamp (1980-01-01 00:00), no data descriptors (sizes and CRC are patched into the local header after the
//! data), no extra fields except ZIP64 ones, and ZIP64 records exactly when a size, an offset or the entry count needs
//! them (or always, for tests). The first entry of a package is `mimetype`, stored, without extra fields (ZIP-02).
//!
//! **Reading** trusts nothing (ZIP-01, ZIP-05, ZIP-06): the end records are located and bounds-checked; every central
//! directory record and every local header is checked against the file length and against each other; entry names
//! must be UTF-8, relative, `/`-separated, without empty, `.` or `..` segments, drive letters, backslashes or control
//! characters, and unique; encrypted entries and compression methods other than stored and deflate are refused; entry
//! data ranges must not overlap; decompression is bounded by a caller-given limit and must produce exactly the declared
//! size with the declared CRC. Violations of the container rules are errors before anything is extracted.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde_json::json;
use tt_model::ModelError;

use crate::codes;

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD64_SIG: u32 = 0x0606_4b50;
const LOCATOR64_SIG: u32 = 0x0706_4b50;
const FLAG_UTF8: u16 = 0x0800;
/// DOS date 1980-01-01, time 00:00:00.
const DOS_DATE: u16 = (1 << 5) | 1;
const DOS_TIME: u16 = 0;
/// Above this many uncompressed bytes an entry is written with ZIP64 sizes (deflate may expand incompressible data by
/// a few hundred kilobytes per 4 GiB; the margin is far larger).
const ZIP64_SIZE_THRESHOLD: u64 = 0xF000_0000;

/// Compression method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Stored (0).
    Stored,
    /// Deflate (8).
    Deflate,
}

impl Method {
    fn code(self) -> u16 {
        match self {
            Method::Stored => 0,
            Method::Deflate => 8,
        }
    }

    /// The method for an entry name (ZIP-03): JSON is deflated; already-compressed media (PNG, JPEG, MP3, AAC, MP4,
    /// MOV, M4A, nested packages) is stored; anything else is deflated.
    #[must_use]
    pub fn for_name(name: &str) -> Method {
        let lower = name.to_ascii_lowercase();
        let stored = [
            ".png", ".jpg", ".jpeg", ".mp3", ".aac", ".m4a", ".mp4", ".mov", ".animtpl", ".animpkg", ".animpal", ".gz",
            ".zip", ".webp", ".ogg", ".opus",
        ];
        if name == "mimetype" || stored.iter().any(|s| lower.ends_with(s)) { Method::Stored } else { Method::Deflate }
    }
}

/// A container-level problem: `project.invalidFormat` with `details.reason` (`unsafeEntry`, `sizeLimit`, `corrupt`,
/// `encrypted`, `unsupportedCompression`, `noMimetype`, `wrongMimetype`, `duplicateEntry`, `notZip`).
#[must_use]
pub fn invalid(reason: &str, message: impl Into<String>, entry: Option<&str>) -> ModelError {
    let mut details = json!({ "reason": reason });
    if let (Some(e), Some(m)) = (entry, details.as_object_mut()) {
        m.insert("entry".into(), json!(e));
    }
    ModelError::with_details(codes::INVALID_FORMAT, message, details)
}

/// Whether an entry name is safe (ZIP-01): non-empty, relative, `/`-separated, no empty/`.`/`..` segments, no backslash,
/// no control characters, at most 1024 bytes and 64 segments, and every segment a portable file name, so that the folder
/// form can hold the same entry on every platform (FMT-01) and no segment can leave its folder (C13-R1-10): no `:` in any
/// segment (drive prefixes such as `C:` or `D:x`, NTFS streams), none of `*?"<>|`, no segment ending in `.` or a space,
/// no Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`, with or without an extension), and
/// no component longer than [`MAX_COMPONENT_BYTES`].
#[must_use]
pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 1024
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.chars().any(|c| c.is_control())
        && name.split('/').count() <= 64
        && name.split('/').all(safe_segment)
}

/// The longest entry-name component, in UTF-8 bytes (C13-R2-05). File systems allow 255 per component (APFS: UTF-8
/// bytes; NTFS: UTF-16 units, never more than the UTF-8 bytes), and a folder save writes hidden siblings of each
/// entry (`.<name>.<12 hex digits>.tmp` / `.old`, 18 bytes longer); 200 leaves room for both.
pub const MAX_COMPONENT_BYTES: usize = 200;

fn safe_segment(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_COMPONENT_BYTES || s == "." || s == ".." || s.ends_with('.') || s.ends_with(' ') {
        return false;
    }
    if s.chars().any(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return false;
    }
    let stem = s.split('.').next().unwrap_or(s).to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    !device
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn sat32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone)]
struct Written {
    name: String,
    method: Method,
    crc: u32,
    compressed: u64,
    uncompressed: u64,
    offset: u64,
    zip64_local: bool,
}

/// Counts bytes written through it.
struct Counting<W> {
    inner: W,
    n: u64,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let k = self.inner.write(buf)?;
        self.n += k as u64;
        Ok(k)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A zip writer over a seekable sink.
pub struct ZipWriter<W: Write + Seek> {
    w: W,
    written: Vec<Written>,
    names: std::collections::HashSet<String>,
    force_zip64: bool,
}

impl<W: Write + Seek> ZipWriter<W> {
    /// A writer at the sink's current position (normally 0).
    #[must_use]
    pub fn new(w: W) -> Self {
        Self { w, written: Vec::new(), names: std::collections::HashSet::new(), force_zip64: false }
    }

    /// Writes ZIP64 records for every entry and the end of the archive (tests of the ZIP64 paths).
    #[must_use]
    pub fn force_zip64(mut self, on: bool) -> Self {
        self.force_zip64 = on;
        self
    }

    fn check_name(&mut self, name: &str) -> io::Result<()> {
        if !safe_name(name) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("unsafe entry name `{name}`")));
        }
        if !self.names.insert(name.to_owned()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("duplicate entry `{name}`")));
        }
        Ok(())
    }

    fn local_header(&mut self, name: &str, method: Method, zip64: bool) -> io::Result<u64> {
        let offset = self.w.stream_position()?;
        let mut h = Vec::with_capacity(30 + name.len() + 20);
        put32(&mut h, LOCAL_SIG);
        put16(&mut h, if zip64 { 45 } else { 20 });
        put16(&mut h, FLAG_UTF8);
        put16(&mut h, method.code());
        put16(&mut h, DOS_TIME);
        put16(&mut h, DOS_DATE);
        put32(&mut h, 0); // crc, patched
        put32(&mut h, if zip64 { u32::MAX } else { 0 });
        put32(&mut h, if zip64 { u32::MAX } else { 0 });
        put16(&mut h, u16::try_from(name.len()).unwrap_or(u16::MAX));
        put16(&mut h, if zip64 { 20 } else { 0 });
        h.extend_from_slice(name.as_bytes());
        if zip64 {
            put16(&mut h, 0x0001);
            put16(&mut h, 16);
            put64(&mut h, 0); // uncompressed, patched
            put64(&mut h, 0); // compressed, patched
        }
        self.w.write_all(&h)?;
        Ok(offset)
    }

    fn patch(&mut self, e: &Written) -> io::Result<()> {
        let end = self.w.stream_position()?;
        self.w.seek(SeekFrom::Start(e.offset + 14))?;
        let mut p = Vec::with_capacity(12);
        put32(&mut p, e.crc);
        if e.zip64_local {
            put32(&mut p, u32::MAX);
            put32(&mut p, u32::MAX);
            self.w.write_all(&p)?;
            let name_len = e.name.len() as u64;
            self.w.seek(SeekFrom::Start(e.offset + 30 + name_len + 4))?;
            let mut q = Vec::with_capacity(16);
            put64(&mut q, e.uncompressed);
            put64(&mut q, e.compressed);
            self.w.write_all(&q)?;
        } else {
            if e.compressed > u64::from(u32::MAX) || e.uncompressed > u64::from(u32::MAX) {
                return Err(io::Error::other("entry grew beyond 4 GiB without ZIP64 sizes"));
            }
            put32(&mut p, sat32(e.compressed));
            put32(&mut p, sat32(e.uncompressed));
            self.w.write_all(&p)?;
        }
        self.w.seek(SeekFrom::Start(end))?;
        Ok(())
    }

    /// Adds an entry from bytes.
    ///
    /// # Errors
    /// I/O errors; `InvalidInput` for an unsafe or duplicate name.
    pub fn add_bytes(&mut self, name: &str, data: &[u8], method: Method) -> io::Result<()> {
        self.add_reader(name, data, data.len() as u64, method)
    }

    /// Adds an entry from a reader of `size_hint` bytes (the hint decides the ZIP64 local header; the actual length is
    /// what is recorded).
    ///
    /// # Errors
    /// I/O errors; `InvalidInput` for an unsafe or duplicate name.
    pub fn add_reader(&mut self, name: &str, mut data: impl Read, size_hint: u64, method: Method) -> io::Result<()> {
        self.check_name(name)?;
        // ZIP-02: `mimetype` has no extra field.
        let zip64 = name != "mimetype" && (self.force_zip64 || size_hint >= ZIP64_SIZE_THRESHOLD);
        let offset = self.local_header(name, method, zip64)?;
        let start = self.w.stream_position()?;
        let mut crc = crc32fast::Hasher::new();
        let mut uncompressed = 0u64;
        let mut buf = vec![0u8; 1 << 16];
        let compressed = match method {
            Method::Stored => {
                loop {
                    let k = data.read(&mut buf)?;
                    if k == 0 {
                        break;
                    }
                    crc.update(&buf[..k]);
                    uncompressed += k as u64;
                    self.w.write_all(&buf[..k])?;
                }
                uncompressed
            }
            Method::Deflate => {
                let mut enc = DeflateEncoder::new(Counting { inner: &mut self.w, n: 0 }, Compression::new(6));
                loop {
                    let k = data.read(&mut buf)?;
                    if k == 0 {
                        break;
                    }
                    crc.update(&buf[..k]);
                    uncompressed += k as u64;
                    enc.write_all(&buf[..k])?;
                }
                let c = enc.finish()?;
                c.n
            }
        };
        debug_assert_eq!(self.w.stream_position().unwrap_or(start + compressed), start + compressed);
        let e = Written {
            name: name.to_owned(),
            method,
            crc: crc.finalize(),
            compressed,
            uncompressed,
            offset,
            zip64_local: zip64,
        };
        self.patch(&e)?;
        self.written.push(e);
        Ok(())
    }

    /// Adds an entry from its raw (already compressed) bytes, copied without recompression (SAVE-01 MAY).
    ///
    /// # Errors
    /// I/O errors; `InvalidInput` for an unsafe or duplicate name.
    pub fn add_raw(
        &mut self,
        name: &str,
        mut raw: impl Read,
        method: Method,
        crc: u32,
        compressed: u64,
        uncompressed: u64,
    ) -> io::Result<()> {
        self.check_name(name)?;
        let zip64 = name != "mimetype" && (self.force_zip64 || uncompressed >= ZIP64_SIZE_THRESHOLD);
        let offset = self.local_header(name, method, zip64)?;
        let n = io::copy(&mut (&mut raw).take(compressed), &mut self.w)?;
        if n != compressed {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, format!("raw entry `{name}` is truncated")));
        }
        let e = Written { name: name.to_owned(), method, crc, compressed, uncompressed, offset, zip64_local: zip64 };
        self.patch(&e)?;
        self.written.push(e);
        Ok(())
    }

    /// Writes the central directory and the end records; returns the sink.
    ///
    /// # Errors
    /// I/O errors.
    pub fn finish(mut self) -> io::Result<W> {
        let cd_start = self.w.stream_position()?;
        let mut cd = Vec::new();
        for e in &self.written {
            let big_u = self.force_zip64 && e.name != "mimetype" || e.uncompressed > u64::from(u32::MAX) - 1;
            let big_c = self.force_zip64 && e.name != "mimetype" || e.compressed > u64::from(u32::MAX) - 1;
            let big_o = self.force_zip64 && e.name != "mimetype" || e.offset > u64::from(u32::MAX) - 1;
            let mut extra = Vec::new();
            if big_u || big_c || big_o {
                let mut body = Vec::new();
                if big_u {
                    put64(&mut body, e.uncompressed);
                }
                if big_c {
                    put64(&mut body, e.compressed);
                }
                if big_o {
                    put64(&mut body, e.offset);
                }
                put16(&mut extra, 0x0001);
                put16(&mut extra, u16::try_from(body.len()).unwrap_or(0));
                extra.extend_from_slice(&body);
            }
            let zip64 = !extra.is_empty() || e.zip64_local;
            put32(&mut cd, CENTRAL_SIG);
            put16(&mut cd, if zip64 { 45 } else { 20 }); // made by: MS-DOS / FAT, spec version
            put16(&mut cd, if zip64 { 45 } else { 20 });
            put16(&mut cd, FLAG_UTF8);
            put16(&mut cd, e.method.code());
            put16(&mut cd, DOS_TIME);
            put16(&mut cd, DOS_DATE);
            put32(&mut cd, e.crc);
            put32(&mut cd, if big_c { u32::MAX } else { sat32(e.compressed) });
            put32(&mut cd, if big_u { u32::MAX } else { sat32(e.uncompressed) });
            put16(&mut cd, u16::try_from(e.name.len()).unwrap_or(u16::MAX));
            put16(&mut cd, u16::try_from(extra.len()).unwrap_or(0));
            put16(&mut cd, 0); // comment
            put16(&mut cd, 0); // disk
            put16(&mut cd, 0); // internal attributes
            put32(&mut cd, 0); // external attributes
            put32(&mut cd, if big_o { u32::MAX } else { sat32(e.offset) });
            cd.extend_from_slice(e.name.as_bytes());
            cd.extend_from_slice(&extra);
        }
        self.w.write_all(&cd)?;
        let cd_size = cd.len() as u64;
        let count = self.written.len() as u64;
        let need64 = self.force_zip64
            || count >= u64::from(u16::MAX)
            || cd_size >= u64::from(u32::MAX)
            || cd_start >= u64::from(u32::MAX);
        let mut end = Vec::new();
        if need64 {
            let eocd64 = self.w.stream_position()?;
            put32(&mut end, EOCD64_SIG);
            put64(&mut end, 44);
            put16(&mut end, 45);
            put16(&mut end, 45);
            put32(&mut end, 0);
            put32(&mut end, 0);
            put64(&mut end, count);
            put64(&mut end, count);
            put64(&mut end, cd_size);
            put64(&mut end, cd_start);
            put32(&mut end, LOCATOR64_SIG);
            put32(&mut end, 0);
            put64(&mut end, eocd64);
            put32(&mut end, 1);
        }
        put32(&mut end, EOCD_SIG);
        put16(&mut end, 0);
        put16(&mut end, 0);
        let c16 = if need64 { u16::MAX } else { u16::try_from(count).unwrap_or(u16::MAX) };
        put16(&mut end, c16);
        put16(&mut end, c16);
        put32(&mut end, if need64 { u32::MAX } else { sat32(cd_size) });
        put32(&mut end, if need64 { u32::MAX } else { sat32(cd_start) });
        put16(&mut end, 0);
        self.w.write_all(&end)?;
        self.w.flush()?;
        Ok(self.w)
    }
}

/// Limits applied while reading (ZIP-06).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum total of declared uncompressed sizes.
    pub max_total: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_entries: 1_000_000, max_total: 1 << 40 }
    }
}

/// One entry of an archive, as validated from its central-directory record and local header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Entry name.
    pub name: String,
    /// Compression method.
    pub method: Method,
    /// Declared CRC-32.
    pub crc: u32,
    /// Declared compressed size.
    pub compressed: u64,
    /// Declared uncompressed size.
    pub uncompressed: u64,
    /// Offset of the local header.
    pub header_offset: u64,
    /// Offset of the entry data.
    pub data_offset: u64,
    /// Whether the local header has no extra field.
    pub local_extra_empty: bool,
}

/// A validated archive on disk. Reads open the file again, so readers on different threads do not interfere.
#[derive(Debug, Clone)]
pub struct ZipArchive {
    path: PathBuf,
    len: u64,
    entries: Vec<ZipEntry>,
    by_name: HashMap<String, usize>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8).map(|s| {
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        u64::from_le_bytes(a)
    })
}

fn corrupt(message: impl Into<String>) -> ModelError {
    invalid("corrupt", message, None)
}

fn read_at(f: &mut File, at: u64, len: usize) -> Result<Vec<u8>, ModelError> {
    let mut buf = vec![0u8; len];
    f.seek(SeekFrom::Start(at)).map_err(|e| corrupt(format!("cannot seek: {e}")))?;
    f.read_exact(&mut buf).map_err(|_| corrupt("the archive is truncated"))?;
    Ok(buf)
}

/// The end of the central directory: entry count, size and offset of the directory, and where the end records start.
struct End {
    count: u64,
    cd_size: u64,
    cd_offset: u64,
    end_start: u64,
}

fn find_end(f: &mut File, len: u64) -> Result<End, ModelError> {
    if len < 22 {
        return Err(invalid("notZip", "the file is too short to be a zip archive", None));
    }
    let tail_len = len.min(22 + 65_535);
    let tail = read_at(f, len - tail_len, usize::try_from(tail_len).unwrap_or(0))?;
    // The last EOCD signature whose comment ends exactly at the end of the file.
    let mut found = None;
    for i in (0..=tail.len().saturating_sub(22)).rev() {
        if u32_at(&tail, i) == Some(EOCD_SIG) {
            let comment = usize::from(u16_at(&tail, i + 20).unwrap_or(u16::MAX));
            if i + 22 + comment == tail.len() {
                found = Some(i);
                break;
            }
        }
    }
    let Some(i) = found else {
        return Err(invalid("notZip", "no end-of-central-directory record (not a zip archive, or truncated)", None));
    };
    let eocd_at = len - tail_len + i as u64;
    let disk = u16_at(&tail, i + 4).unwrap_or(1);
    let cd_disk = u16_at(&tail, i + 6).unwrap_or(1);
    let count16 = u16_at(&tail, i + 10).unwrap_or(0);
    let size32 = u32_at(&tail, i + 12).unwrap_or(0);
    let off32 = u32_at(&tail, i + 16).unwrap_or(0);
    if (disk != 0 && disk != u16::MAX) || (cd_disk != 0 && cd_disk != u16::MAX) {
        return Err(corrupt("multi-disk archives are not supported"));
    }
    if count16 == u16::MAX || size32 == u32::MAX || off32 == u32::MAX {
        // ZIP64: the locator sits right before the EOCD.
        if eocd_at < 20 {
            return Err(corrupt("ZIP64 locator missing"));
        }
        let loc = read_at(f, eocd_at - 20, 20)?;
        if u32_at(&loc, 0) != Some(LOCATOR64_SIG) {
            return Err(corrupt("ZIP64 locator missing"));
        }
        let rec_at = u64_at(&loc, 8).unwrap_or(u64::MAX);
        if rec_at.checked_add(56).is_none_or(|e| e > eocd_at - 20) {
            return Err(corrupt("ZIP64 end record out of bounds"));
        }
        let rec = read_at(f, rec_at, 56)?;
        if u32_at(&rec, 0) != Some(EOCD64_SIG) {
            return Err(corrupt("ZIP64 end record missing"));
        }
        let count = u64_at(&rec, 32).unwrap_or(0);
        let cd_size = u64_at(&rec, 40).unwrap_or(0);
        let cd_offset = u64_at(&rec, 48).unwrap_or(0);
        return Ok(End { count, cd_size, cd_offset, end_start: rec_at });
    }
    Ok(End { count: u64::from(count16), cd_size: u64::from(size32), cd_offset: u64::from(off32), end_start: eocd_at })
}

impl ZipArchive {
    /// Opens and validates an archive. `expect_mimetype`: the first entry must be an uncompressed `mimetype` without
    /// extra fields holding exactly this text (ZIP-02).
    ///
    /// # Errors
    /// `project.invalidFormat` (`details.reason`) for anything that is not a well-formed, safe archive; the I/O error
    /// of opening the file.
    #[allow(clippy::too_many_lines)]
    pub fn open(path: &Path, expect_mimetype: Option<&str>, limits: Limits) -> Result<Self, ModelError> {
        let mut f = File::open(path).map_err(|e| crate::fsx::open_error(&e, path))?;
        let len = f.metadata().map_err(|e| crate::fsx::open_error(&e, path))?.len();
        let end = find_end(&mut f, len)?;
        if end.count > limits.max_entries as u64 {
            return Err(invalid(
                "sizeLimit",
                format!("the archive declares {} entries (limit {})", end.count, limits.max_entries),
                None,
            ));
        }
        if end.cd_offset.checked_add(end.cd_size).is_none_or(|e| e > end.end_start)
            || end.count.saturating_mul(46) > end.cd_size
        {
            return Err(corrupt("the central directory is out of bounds"));
        }
        let cd = read_at(&mut f, end.cd_offset, usize::try_from(end.cd_size).map_err(|_| corrupt("huge directory"))?)?;
        let mut entries: Vec<ZipEntry> = Vec::with_capacity(usize::try_from(end.count).unwrap_or(0));
        let mut by_name: HashMap<String, usize> = HashMap::new();
        let mut at = 0usize;
        let mut total: u64 = 0;
        for _ in 0..end.count {
            if u32_at(&cd, at) != Some(CENTRAL_SIG) {
                return Err(corrupt("bad central directory record"));
            }
            let flags = u16_at(&cd, at + 8).ok_or_else(|| corrupt("short record"))?;
            let method = u16_at(&cd, at + 10).ok_or_else(|| corrupt("short record"))?;
            let crc = u32_at(&cd, at + 16).ok_or_else(|| corrupt("short record"))?;
            let mut csize = u64::from(u32_at(&cd, at + 20).ok_or_else(|| corrupt("short record"))?);
            let mut usize_ = u64::from(u32_at(&cd, at + 24).ok_or_else(|| corrupt("short record"))?);
            let name_len = usize::from(u16_at(&cd, at + 28).ok_or_else(|| corrupt("short record"))?);
            let extra_len = usize::from(u16_at(&cd, at + 30).ok_or_else(|| corrupt("short record"))?);
            let comment_len = usize::from(u16_at(&cd, at + 32).ok_or_else(|| corrupt("short record"))?);
            let disk = u16_at(&cd, at + 34).ok_or_else(|| corrupt("short record"))?;
            let mut offset = u64::from(u32_at(&cd, at + 42).ok_or_else(|| corrupt("short record"))?);
            let name_bytes = cd.get(at + 46..at + 46 + name_len).ok_or_else(|| corrupt("short record"))?;
            let extra =
                cd.get(at + 46 + name_len..at + 46 + name_len + extra_len).ok_or_else(|| corrupt("short record"))?;
            at = at + 46 + name_len + extra_len + comment_len;
            if at > cd.len() {
                return Err(corrupt("short record"));
            }
            let name = String::from_utf8(name_bytes.to_vec())
                .map_err(|_| invalid("unsafeEntry", "an entry name is not valid UTF-8", None))?;
            if flags & 0x0001 != 0 || flags & 0x0040 != 0 {
                return Err(invalid("encrypted", format!("entry `{name}` is encrypted (ZIP-05)"), Some(&name)));
            }
            // ZIP64 extra: the 0xFFFFFFFF fields, in order.
            let (need_u, need_c, need_o) =
                (usize_ == u64::from(u32::MAX), csize == u64::from(u32::MAX), offset == u64::from(u32::MAX));
            if need_u || need_c || need_o || disk == u16::MAX {
                let mut k = 0usize;
                let mut z = None;
                while k + 4 <= extra.len() {
                    let id = u16_at(extra, k).unwrap_or(0);
                    let size = usize::from(u16_at(extra, k + 2).unwrap_or(0));
                    if id == 0x0001 {
                        z = extra.get(k + 4..k + 4 + size);
                        break;
                    }
                    k += 4 + size;
                }
                let z = z.ok_or_else(|| corrupt(format!("entry `{name}` lacks its ZIP64 sizes")))?;
                let mut p = 0usize;
                let mut next = || -> Result<u64, ModelError> {
                    let v = u64_at(z, p).ok_or_else(|| corrupt("short ZIP64 field"))?;
                    p += 8;
                    Ok(v)
                };
                if need_u {
                    usize_ = next()?;
                }
                if need_c {
                    csize = next()?;
                }
                if need_o {
                    offset = next()?;
                }
            }
            let is_dir = name.ends_with('/');
            let check = name.strip_suffix('/').unwrap_or(&name);
            if !safe_name(check) {
                return Err(invalid("unsafeEntry", format!("unsafe entry name `{name}` (ZIP-01)"), Some(&name)));
            }
            let method = match method {
                0 => Method::Stored,
                8 => Method::Deflate,
                m => {
                    return Err(invalid(
                        "unsupportedCompression",
                        format!("entry `{name}` uses compression method {m}"),
                        Some(&name),
                    ));
                }
            };
            if is_dir {
                if usize_ != 0 {
                    return Err(corrupt(format!("directory entry `{name}` has data")));
                }
                continue;
            }
            if method == Method::Stored && csize != usize_ {
                return Err(invalid("sizeLimit", format!("stored entry `{name}` declares two sizes"), Some(&name)));
            }
            total = total.saturating_add(usize_);
            if total > limits.max_total {
                return Err(invalid("sizeLimit", "the archive's declared sizes exceed the global limit", Some(&name)));
            }
            if by_name.insert(name.clone(), entries.len()).is_some() {
                return Err(invalid("duplicateEntry", format!("entry `{name}` occurs twice"), Some(&name)));
            }
            entries.push(ZipEntry {
                name,
                method,
                crc,
                compressed: csize,
                uncompressed: usize_,
                header_offset: offset,
                data_offset: 0,
                local_extra_empty: true,
            });
        }
        // Local headers: present, consistent, inside the data area, not overlapping.
        for e in &mut entries {
            if e.header_offset.checked_add(30).is_none_or(|x| x > end.cd_offset) {
                return Err(corrupt(format!("entry `{}` starts outside the data area", e.name)));
            }
            let h = read_at(&mut f, e.header_offset, 30)?;
            if u32_at(&h, 0) != Some(LOCAL_SIG) {
                return Err(corrupt(format!("entry `{}` has no local header", e.name)));
            }
            let lflags = u16_at(&h, 6).unwrap_or(0);
            if lflags & 0x0001 != 0 {
                return Err(invalid("encrypted", format!("entry `{}` is encrypted", e.name), Some(&e.name)));
            }
            let lmethod = u16_at(&h, 8).unwrap_or(0);
            let ln = u64::from(u16_at(&h, 26).unwrap_or(0));
            let le = u64::from(u16_at(&h, 28).unwrap_or(0));
            if lmethod != e.method.code() {
                return Err(corrupt(format!("entry `{}`: local and central compression differ", e.name)));
            }
            let name_at = e.header_offset + 30;
            if name_at.checked_add(ln + le).is_none_or(|x| x > end.cd_offset) {
                return Err(corrupt(format!("entry `{}`: local header out of bounds", e.name)));
            }
            let lname = read_at(&mut f, name_at, usize::try_from(ln).unwrap_or(0))?;
            if lname != e.name.as_bytes() {
                return Err(invalid(
                    "unsafeEntry",
                    format!("entry `{}`: the local header names a different entry", e.name),
                    Some(&e.name),
                ));
            }
            e.data_offset = name_at + ln + le;
            e.local_extra_empty = le == 0;
            if e.data_offset.checked_add(e.compressed).is_none_or(|x| x > end.cd_offset) {
                return Err(invalid(
                    "sizeLimit",
                    format!("entry `{}` declares more data than the archive holds", e.name),
                    Some(&e.name),
                ));
            }
        }
        let mut ranges: Vec<(u64, u64, &str)> =
            entries.iter().map(|e| (e.header_offset, e.data_offset + e.compressed, e.name.as_str())).collect();
        ranges.sort_unstable();
        for w in ranges.windows(2) {
            if w[1].0 < w[0].1 {
                return Err(invalid(
                    "unsafeEntry",
                    format!("entries `{}` and `{}` overlap", w[0].2, w[1].2),
                    Some(w[1].2),
                ));
            }
        }
        let archive = Self { path: path.to_path_buf(), len, entries, by_name };
        if let Some(mt) = expect_mimetype {
            archive.check_mimetype(mt)?;
        }
        Ok(archive)
    }

    fn check_mimetype(&self, expected: &str) -> Result<(), ModelError> {
        let first = self.entries.iter().min_by_key(|e| e.header_offset);
        match first {
            Some(e) if e.name == "mimetype" && e.header_offset == 0 => {
                if e.method != Method::Stored || !e.local_extra_empty {
                    return Err(invalid(
                        "noMimetype",
                        "the `mimetype` entry must be stored without extra fields (ZIP-02)",
                        Some("mimetype"),
                    ));
                }
                let data = self.read("mimetype", 255)?;
                if data != expected.as_bytes() {
                    return Err(invalid(
                        "wrongMimetype",
                        format!("`mimetype` is `{}`, expected `{expected}`", String::from_utf8_lossy(&data)),
                        Some("mimetype"),
                    ));
                }
                Ok(())
            }
            _ => Err(invalid("noMimetype", "the first entry must be `mimetype` (ZIP-02)", None)),
        }
    }

    /// The archive file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// File length.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the archive has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries in central-directory order (directories excluded).
    #[must_use]
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// An entry by name.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&ZipEntry> {
        self.by_name.get(name).and_then(|i| self.entries.get(*i))
    }

    /// A reader of an entry's decompressed bytes, bounded: it fails (`InvalidData`) after `limit` bytes or when the
    /// data does not match the declared size and CRC at its end.
    ///
    /// # Errors
    /// `project.invalidFormat` for an unknown entry; the I/O error of opening the file.
    pub fn reader(&self, name: &str, limit: u64) -> Result<EntryReader, ModelError> {
        let e = self.entry(name).ok_or_else(|| invalid("corrupt", format!("no entry `{name}`"), Some(name)))?.clone();
        let mut f = File::open(&self.path).map_err(|err| crate::fsx::open_error(&err, &self.path))?;
        f.seek(SeekFrom::Start(e.data_offset)).map_err(|err| corrupt(format!("cannot seek: {err}")))?;
        let raw = f.take(e.compressed);
        let inner: Box<dyn Read + Send> = match e.method {
            Method::Stored => Box::new(raw),
            Method::Deflate => Box::new(DeflateDecoder::new(raw)),
        };
        Ok(EntryReader { inner, crc: crc32fast::Hasher::new(), n: 0, limit: limit.min(e.uncompressed), entry: e })
    }

    /// Reads an entry completely, bounded by `limit` bytes.
    ///
    /// # Errors
    /// `project.invalidFormat` with `details.reason` `sizeLimit` (more than `limit` bytes or more than declared) or
    /// `corrupt` (fewer bytes than declared, CRC mismatch, inflate error).
    pub fn read(&self, name: &str, limit: u64) -> Result<Vec<u8>, ModelError> {
        let e = self.entry(name).ok_or_else(|| invalid("corrupt", format!("no entry `{name}`"), Some(name)))?;
        if e.uncompressed > limit {
            return Err(invalid(
                "sizeLimit",
                format!("entry `{name}` declares {} bytes (limit {limit})", e.uncompressed),
                Some(name),
            ));
        }
        let mut r = self.reader(name, limit)?;
        let mut out = Vec::with_capacity(usize::try_from(e.uncompressed.min(1 << 26)).unwrap_or(0));
        r.read_to_end(&mut out).map_err(|err| entry_error(name, &err))?;
        Ok(out)
    }

    /// A reader of an entry's raw (compressed) bytes, for copying without recompression.
    ///
    /// # Errors
    /// As [`ZipArchive::reader`].
    pub fn raw_reader(&self, name: &str) -> Result<(ZipEntry, impl Read + use<>), ModelError> {
        let e = self.entry(name).ok_or_else(|| invalid("corrupt", format!("no entry `{name}`"), Some(name)))?.clone();
        let mut f = File::open(&self.path).map_err(|err| crate::fsx::open_error(&err, &self.path))?;
        f.seek(SeekFrom::Start(e.data_offset)).map_err(|err| corrupt(format!("cannot seek: {err}")))?;
        let take = f.take(e.compressed);
        Ok((e, take))
    }
}

/// Maps a bounded-read failure of an entry to `project.invalidFormat`.
#[must_use]
pub fn entry_error(name: &str, err: &io::Error) -> ModelError {
    let reason = if err.to_string().contains("limit") || err.to_string().contains("more than declared") {
        "sizeLimit"
    } else {
        "corrupt"
    };
    invalid(reason, format!("entry `{name}`: {err}"), Some(name))
}

/// A bounded, verifying reader of one entry.
pub struct EntryReader {
    inner: Box<dyn Read + Send>,
    crc: crc32fast::Hasher,
    n: u64,
    limit: u64,
    entry: ZipEntry,
}

impl Read for EntryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Ask for one byte more than allowed, so a stream longer than declared is noticed.
        let room = self.limit.saturating_add(1).saturating_sub(self.n);
        let want = buf.len().min(usize::try_from(room).unwrap_or(usize::MAX));
        if want == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "entry exceeds its size limit"));
        }
        let k = self.inner.read(&mut buf[..want])?;
        self.n += k as u64;
        if self.n > self.limit {
            let what = if self.n > self.entry.uncompressed { "more than declared" } else { "size limit" };
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("entry produces {what} (limit)")));
        }
        self.crc.update(&buf[..k]);
        if k == 0 {
            if self.n != self.entry.uncompressed {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("entry has {} bytes, {} declared", self.n, self.entry.uncompressed),
                ));
            }
            if self.crc.clone().finalize() != self.entry.crc {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "CRC-32 mismatch"));
            }
        }
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;
    use std::io::Cursor;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tt-zip-{}", crate::fsx::random_token(4)));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    fn build(force64: bool) -> Vec<u8> {
        let mut w = ZipWriter::new(Cursor::new(Vec::new())).force_zip64(force64);
        w.add_bytes("mimetype", b"application/x-animpkg+zip", Method::Stored).unwrap();
        w.add_bytes("project.json", b"{\n  \"a\": 1\n}\n", Method::Deflate).unwrap();
        w.add_bytes("media/x.png", &[7u8; 1000], Method::Stored).unwrap();
        w.add_bytes("palettes/p.json", &vec![b'x'; 70_000], Method::Deflate).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trip_plain_and_zip64() {
        for force in [false, true] {
            let p = tmp("a.zip");
            std::fs::write(&p, build(force)).unwrap();
            let z = ZipArchive::open(&p, Some("application/x-animpkg+zip"), Limits::default()).unwrap();
            assert_eq!(z.entries().len(), 4);
            assert_eq!(z.read("project.json", 1000).unwrap(), b"{\n  \"a\": 1\n}\n");
            assert_eq!(z.read("palettes/p.json", 70_000).unwrap(), vec![b'x'; 70_000]);
            assert_eq!(z.read("media/x.png", 1000).unwrap(), vec![7u8; 1000]);
            assert!(z.read("palettes/p.json", 69_999).is_err(), "bounded");
        }
    }

    #[test]
    fn deterministic_bytes() {
        assert_eq!(build(false), build(false));
    }

    #[test]
    fn unsafe_names_are_refused() {
        for n in [
            "../x",
            "/x",
            "a/../b",
            "C:/x",
            "a\\b",
            "a//b",
            "./a",
            "a/.",
            "",
            "a\u{1}b",
            "vendor/C:/x",
            "v/D:rel",
            "a/b:stream",
            "a/b.",
            "a/b ",
            "NUL",
            "a/con.txt",
            "LPT1.log",
            "a/x?y",
        ] {
            assert!(!safe_name(n), "{n:?}");
        }
        for n in ["a", "a/b.json", "media/as_1.wav", "x.y/z", "console.json", "COM0", "LPT10", ".lock"] {
            assert!(safe_name(n), "{n:?}");
        }
        // Component length (C13-R2-05): bytes, not characters; the bound is per component.
        assert!(safe_name(&format!("v/{}", "a".repeat(MAX_COMPONENT_BYTES))));
        assert!(!safe_name(&format!("v/{}", "a".repeat(MAX_COMPONENT_BYTES + 1))));
        assert!(!safe_name(&format!("v/{}", "é".repeat(MAX_COMPONENT_BYTES / 2 + 1))));
        assert!(safe_name(&format!("{0}/{0}/{0}", "a".repeat(MAX_COMPONENT_BYTES))));
    }

    #[test]
    fn mimetype_rules() {
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        w.add_bytes("project.json", b"{}", Method::Deflate).unwrap();
        w.add_bytes("mimetype", b"application/x-animpkg+zip", Method::Stored).unwrap();
        let p = tmp("b.zip");
        std::fs::write(&p, w.finish().unwrap().into_inner()).unwrap();
        let e = ZipArchive::open(&p, Some("application/x-animpkg+zip"), Limits::default()).unwrap_err();
        assert_eq!(e.details()["reason"], "noMimetype");
        let p = tmp("c.zip");
        std::fs::write(&p, build(false)).unwrap();
        let e = ZipArchive::open(&p, Some("application/x-animtpl+zip"), Limits::default()).unwrap_err();
        assert_eq!(e.details()["reason"], "wrongMimetype");
    }
}
