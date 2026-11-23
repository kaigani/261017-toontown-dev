//! Fonts for text layout (02 §3.7) and the `missingFonts` lint (`doc.validate`).
//!
//! The system font list is read once per process (`fontdb`, the platform's font folders) and font files are parsed
//! with `skrifa` for the metrics layout needs: units per em, ascender, descender, line gap, horizontal advances
//! and `kern`-table pairs. A run whose font is not installed keeps its requested name (§3.7) and is laid out with the
//! platform's default sans-serif face; when no face can be read at all, a fixed fallback metric (advance 0.5 em,
//! ascender 0.8 em, descender 0.2 em) keeps layout defined. Layout bounds are derived data: they depend on the font
//! files of the machine, never the persisted text (ADR-0016). Same-font layout controls (C21-R1-11) register a fixture
//! font file through the dev-only `test-hooks` feature ([`register_test_font`]); products never have it.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::{Arc, Condvar, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

/// Metrics of one face, scaled to 1 em = 1.0.
#[derive(Debug, Clone)]
pub struct FaceMetrics {
    /// Family name of the face actually used.
    pub family: String,
    /// Ascender (positive, em).
    pub ascender: f64,
    /// Descender (positive distance below the baseline, em).
    pub descender: f64,
    /// Line gap (em).
    pub line_gap: f64,
    data: Option<(Arc<Vec<u8>>, u32)>,
    units_per_em: f64,
}

/// The font a run resolves to.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The face metrics.
    pub face: Arc<FaceMetrics>,
    /// Whether the requested family is installed (`false`: a fallback is used, 02 §3.7).
    pub installed: bool,
}

impl FaceMetrics {
    /// The fixed fallback metric (no readable face on the machine).
    #[must_use]
    pub fn fallback() -> Self {
        Self { family: String::new(), ascender: 0.8, descender: 0.2, line_gap: 0.0, data: None, units_per_em: 1.0 }
    }

    fn with_face<T>(&self, f: impl FnOnce(&skrifa::FontRef<'_>) -> T) -> Option<T> {
        let (data, index) = self.data.as_ref()?;
        let face = skrifa::FontRef::from_index(data, *index).ok()?;
        Some(f(&face))
    }

    /// Horizontal advances (em) of each character, and the automatic kerning (em) between each pair of neighbours
    /// (`kern` table pairs, horizontal, non-variable subtables).
    #[must_use]
    pub fn advances(&self, chars: &[char]) -> (Vec<f64>, Vec<f64>) {
        use skrifa::MetadataProvider;
        use skrifa::raw::TableProvider;
        use skrifa::raw::tables::kern::SubtableKind;
        let upem = self.units_per_em;
        let r = self.with_face(|face| {
            let charmap = face.charmap();
            let metrics =
                face.glyph_metrics(skrifa::prelude::Size::unscaled(), skrifa::prelude::LocationRef::default());
            let glyphs: Vec<Option<skrifa::GlyphId>> = chars.iter().map(|c| charmap.map(*c)).collect();
            let adv: Vec<f64> = glyphs
                .iter()
                .map(|g| g.and_then(|g| metrics.advance_width(g)).map_or(0.5, |a| f64::from(a) / upem))
                .collect();
            let mut kern = vec![0.0; chars.len().saturating_sub(1)];
            if let Ok(table) = face.kern() {
                let subtables: Vec<_> = table
                    .subtables()
                    .filter_map(Result::ok)
                    .filter(|s| s.is_horizontal() && !s.is_variable() && !s.is_cross_stream())
                    .filter_map(|s| s.kind().ok())
                    .collect();
                for (i, k) in kern.iter_mut().enumerate() {
                    let (Some(l), Some(r)) = (glyphs[i], glyphs[i + 1]) else { continue };
                    let found = subtables.iter().find_map(|s| match s {
                        SubtableKind::Format0(t) => t.kerning(l, r),
                        SubtableKind::Format2(t) => t.kerning(l, r),
                        SubtableKind::Format3(t) => t.kerning(l, r),
                        SubtableKind::Format1(_) => None,
                    });
                    if let Some(v) = found {
                        *k = f64::from(v) / upem;
                    }
                }
            }
            (adv, kern)
        });
        r.unwrap_or_else(|| (vec![0.5; chars.len()], vec![0.0; chars.len().saturating_sub(1)]))
    }
}

/// Collects a glyph outline as cubic contours (quadratics raised exactly to cubics), in em units, y up.
#[derive(Default)]
struct Contours {
    done: Vec<Vec<[f64; 8]>>,
    open: Vec<[f64; 8]>,
    at: (f64, f64),
    start: (f64, f64),
    scale: f64,
}

impl Contours {
    fn finish(&mut self) {
        if self.at != self.start && !self.open.is_empty() {
            let (a, b) = (self.at, self.start);
            self.open.push([a.0, a.1, a.0, a.1, b.0, b.1, b.0, b.1]);
        }
        if !self.open.is_empty() {
            self.done.push(std::mem::take(&mut self.open));
        }
    }
}

impl skrifa::outline::OutlinePen for Contours {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        let p = (f64::from(x) * self.scale, f64::from(y) * self.scale);
        self.at = p;
        self.start = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = (self.at, (f64::from(x) * self.scale, f64::from(y) * self.scale));
        if a != b {
            self.open.push([a.0, a.1, a.0, a.1, b.0, b.1, b.0, b.1]);
        }
        self.at = b;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let a = self.at;
        let c = (f64::from(cx0) * self.scale, f64::from(cy0) * self.scale);
        let b = (f64::from(x) * self.scale, f64::from(y) * self.scale);
        let c1 = (a.0 + 2.0 / 3.0 * (c.0 - a.0), a.1 + 2.0 / 3.0 * (c.1 - a.1));
        let c2 = (b.0 + 2.0 / 3.0 * (c.0 - b.0), b.1 + 2.0 / 3.0 * (c.1 - b.1));
        self.open.push([a.0, a.1, c1.0, c1.1, c2.0, c2.1, b.0, b.1]);
        self.at = b;
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let a = self.at;
        let s = self.scale;
        let b = (f64::from(x) * s, f64::from(y) * s);
        self.open.push([
            a.0,
            a.1,
            f64::from(cx0) * s,
            f64::from(cy0) * s,
            f64::from(cx1) * s,
            f64::from(cy1) * s,
            b.0,
            b.1,
        ]);
        self.at = b;
    }

    fn close(&mut self) {
        self.finish();
    }
}

impl FaceMetrics {
    /// The outline of a character's glyph as closed contours of cubic segments `[x0, y0, x1, y1, x2, y2, x3, y3]` in
    /// em units (y up, origin on the baseline), unhinted at the default location. `None` without a readable face or a
    /// glyph for the character; an empty list for a blank glyph (space).
    #[must_use]
    pub fn outline(&self, ch: char) -> Option<Vec<Vec<[f64; 8]>>> {
        use skrifa::MetadataProvider;
        let upem = self.units_per_em;
        self.with_face(|face| {
            let gid = face.charmap().map(ch)?;
            let glyph = face.outline_glyphs().get(gid)?;
            let mut pen = Contours { scale: 1.0 / upem, ..Contours::default() };
            let settings = skrifa::outline::DrawSettings::unhinted(
                skrifa::prelude::Size::unscaled(),
                skrifa::prelude::LocationRef::default(),
            );
            glyph.draw(settings, &mut pen).ok()?;
            pen.finish();
            Some(pen.done)
        })
        .flatten()
    }
}

struct Fonts {
    db: fontdb::Database,
    cache: Mutex<BTreeMap<(String, bool, bool), Arc<FaceMetrics>>>,
}

/// Font files registered by tests before the font list is first read (dev-only).
#[cfg(feature = "test-hooks")]
static TEST_FONTS: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

/// Registers a font file for same-font layout tests (dev-only `test-hooks`; C21-R1-11). It joins the font list when the
/// list is first read, so a test registers it before any text is laid out in its process ([`is_installed`] then
/// confirms it).
#[cfg(feature = "test-hooks")]
pub fn register_test_font(data: Vec<u8>) {
    if let Ok(mut list) = TEST_FONTS.lock() {
        list.push(data);
    }
}

/// The font list, read once per process.
static FONTS: OnceLock<Fonts> = OnceLock::new();
/// Set once the list is read, with the condition variable a bounded wait sleeps on (ruling E-43).
static READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
/// The background read is started once per process.
static STARTED: Once = Once::new();
/// How long the read took.
static SCAN_TIME: OnceLock<Duration> = OnceLock::new();
/// Set when a caller found the list unread (it waited, or skipped text): only then is the read announced, so a process
/// in which nothing missed the list sees no event from it.
static MISSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Called with `true` when a caller starts waiting for the list and `false` when it stops (the window's status line).
type Observer = Box<dyn Fn(bool) + Send + Sync>;
static OBSERVERS: Mutex<Vec<Observer>> = Mutex::new(Vec::new());

/// How long a command waits for the font list before it answers that the fonts are still being read (ruling E-43).
pub const WAIT_BOUND: Duration = Duration::from_secs(20);

/// The font list was not read within the bound (ruling E-43): the caller answers that the system fonts are still being
/// read, before it changes anything, and a later retry succeeds. Text is never laid out with a partial list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontsLoading;

#[cfg(feature = "test-hooks")]
mod hold {
    //! Dev-only (`test-hooks`): holds the font read until released, and shortens the bounded wait, so tests can reach
    //! every caller while the list is not ready (ruling E-43). Products never have it (the release-features gate).
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    pub(super) static HELD: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
    pub(super) static BOUND: Mutex<Option<Duration>> = Mutex::new(None);
    pub(super) static EAGER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub(super) fn wait_released() {
        let (lock, cv) = &HELD;
        let Ok(mut held) = lock.lock() else { return };
        while *held {
            match cv.wait(held) {
                Ok(g) => held = g,
                Err(_) => return,
            }
        }
    }
}

/// Holds the font read (dev-only `test-hooks`): a read that has not started yet waits until [`release_scan`]. Call it
/// before anything in the process reads the list.
#[cfg(feature = "test-hooks")]
pub fn hold_scan() {
    if let Ok(mut held) = hold::HELD.0.lock() {
        *held = true;
    }
}

/// Releases a held font read (dev-only `test-hooks`).
#[cfg(feature = "test-hooks")]
pub fn release_scan() {
    let (lock, cv) = &hold::HELD;
    if let Ok(mut held) = lock.lock() {
        *held = false;
        cv.notify_all();
    }
}

/// Replaces [`WAIT_BOUND`] for tests (dev-only `test-hooks`); `None` restores it.
#[cfg(feature = "test-hooks")]
pub fn set_wait_bound(bound: Option<Duration>) {
    if let Ok(mut b) = hold::BOUND.lock() {
        *b = bound;
    }
}

/// Makes the render list compute every entry's text bounds eagerly, as before ruling E-43 (b) (dev-only `test-hooks`),
/// so a test compares the two.
#[cfg(feature = "test-hooks")]
pub fn set_eager_text_bounds(on: bool) {
    hold::EAGER.store(on, std::sync::atomic::Ordering::Release);
}

/// Whether the render list computes every entry's text bounds eagerly ([`set_eager_text_bounds`]); always `false` in a
/// product (the dev-only `test-hooks` feature is absent, which the release-features gate proves).
#[must_use]
pub fn eager_text_bounds() -> bool {
    #[cfg(feature = "test-hooks")]
    {
        hold::EAGER.load(std::sync::atomic::Ordering::Acquire)
    }
    #[cfg(not(feature = "test-hooks"))]
    {
        false
    }
}

/// The bound a wait uses: [`WAIT_BOUND`], or a test's.
#[must_use]
pub fn wait_bound() -> Duration {
    #[cfg(feature = "test-hooks")]
    if let Ok(b) = hold::BOUND.lock()
        && let Some(d) = *b
    {
        return d;
    }
    WAIT_BOUND
}

fn scan() -> Fonts {
    #[cfg(feature = "test-hooks")]
    hold::wait_released();
    let started = Instant::now();
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    #[cfg(feature = "test-hooks")]
    if let Ok(list) = TEST_FONTS.lock() {
        for data in list.iter() {
            db.load_font_data(data.clone());
        }
    }
    let elapsed = started.elapsed();
    let _ = SCAN_TIME.set(elapsed);
    // The scan's duration, for the application log (the host's stderr; ruling E-43 item 4).
    let _ =
        writeln!(std::io::stderr(), "toontown: system fonts read in {} ms ({} faces)", elapsed.as_millis(), db.len());
    Fonts { db, cache: Mutex::new(BTreeMap::new()) }
}

/// The font list, reading it on the caller's thread if no read has finished: callers that may run before the list is
/// read wait for it with [`ready_within`] first, which bounds the wait.
fn fonts() -> &'static Fonts {
    let f = FONTS.get_or_init(scan);
    mark_ready();
    f
}

fn mark_ready() {
    let (lock, cv) = &READY;
    let first = match lock.lock() {
        Ok(mut ready) if !*ready => {
            *ready = true;
            cv.notify_all();
            true
        }
        _ => false,
    };
    // The list is read: if a caller missed it, observers learn that nothing waits any more (the window reads its views
    // again, recovering what its state reads skipped).
    if first && MISSED.swap(false, std::sync::atomic::Ordering::AcqRel) {
        tell(false);
    }
}

/// Starts reading the system font list on a background thread (once per process), so the first text command does not
/// pay for it. If no thread can be started the list is read on first use.
pub fn warm() {
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("tt-fonts".to_owned()).spawn(|| {
            let _ = fonts();
        });
    });
}

/// Whether the font list has been read.
#[must_use]
pub fn is_ready() -> bool {
    FONTS.get().is_some()
}

/// How long the read took, once it has finished.
#[must_use]
pub fn scan_time() -> Option<Duration> {
    SCAN_TIME.get().copied()
}

/// Records that a caller found the list unread and skipped text without waiting (the window's state reads): the read is
/// announced when it ends, or at once if it ended meanwhile.
pub fn note_skipped() {
    MISSED.store(true, std::sync::atomic::Ordering::Release);
    if is_ready() && MISSED.swap(false, std::sync::atomic::Ordering::AcqRel) {
        tell(false);
    }
}

/// Registers a function told when a caller starts (`true`) and stops (`false`) waiting for the list, and (`false`) when
/// the list has been read after a caller missed it.
pub fn add_wait_observer(f: impl Fn(bool) + Send + Sync + 'static) {
    if let Ok(mut list) = OBSERVERS.lock() {
        list.push(Box::new(f));
    }
}

fn tell(waiting: bool) {
    if let Ok(list) = OBSERVERS.lock() {
        for f in list.iter() {
            f(waiting);
        }
    }
}

/// Waits for the font list at most `bound` (ruling E-43): `Ok` once it is read (at once when it already is), else
/// [`FontsLoading`]. After `Ok` every layout reads the full list without waiting.
///
/// # Errors
///
/// [`FontsLoading`] when the list is not read within `bound`.
pub fn ready_within(bound: Duration) -> Result<(), FontsLoading> {
    if is_ready() {
        return Ok(());
    }
    warm();
    MISSED.store(true, std::sync::atomic::Ordering::Release);
    tell(true);
    let out = wait_ready(bound);
    tell(false);
    out
}

fn wait_ready(bound: Duration) -> Result<(), FontsLoading> {
    let deadline = Instant::now() + bound;
    let (lock, cv) = &READY;
    let Ok(mut ready) = lock.lock() else { return if is_ready() { Ok(()) } else { Err(FontsLoading) } };
    while !*ready {
        let now = Instant::now();
        if now >= deadline {
            return if is_ready() { Ok(()) } else { Err(FontsLoading) };
        }
        match cv.wait_timeout(ready, deadline - now) {
            Ok((g, _)) => ready = g,
            Err(_) => return if is_ready() { Ok(()) } else { Err(FontsLoading) },
        }
    }
    Ok(())
}

/// The installed family names (as the fonts name them), sorted and unique.
#[must_use]
pub fn installed_families() -> Vec<String> {
    let mut v: Vec<String> = fonts().db.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect();
    v.sort();
    v.dedup();
    v
}

/// The generic CSS families (`sans-serif`, `serif`, `monospace`): always available, resolved to the platform's
/// default face of that kind.
#[must_use]
pub fn generic(family: &str) -> Option<fontdb::Family<'static>> {
    match family {
        "sans-serif" => Some(fontdb::Family::SansSerif),
        "serif" => Some(fontdb::Family::Serif),
        "monospace" => Some(fontdb::Family::Monospace),
        _ => None,
    }
}

/// Whether a family is available: a generic family, or an installed one (exact match first, then case-insensitive,
/// like path names, 10 §2.4).
#[must_use]
pub fn is_installed(family: &str) -> bool {
    generic(family).is_some() || installed_name(family).is_some()
}

fn installed_name(family: &str) -> Option<String> {
    let db = &fonts().db;
    let mut ci = None;
    for f in db.faces() {
        for (n, _) in &f.families {
            if n == family {
                return Some(n.clone());
            }
            if ci.is_none() && n.to_lowercase() == family.to_lowercase() {
                ci = Some(n.clone());
            }
        }
    }
    ci
}

fn load(db: &fontdb::Database, name: &str, bold: bool, italic: bool) -> Option<FaceMetrics> {
    let families = [fontdb::Family::Name(name)];
    let query = fontdb::Query {
        families: &families,
        weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
        stretch: fontdb::Stretch::Normal,
        style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
    };
    let id = db.query(&query)?;
    let (source, index) = db.face_source(id)?;
    let bytes: Vec<u8> = match source {
        fontdb::Source::Binary(b) => (*b).as_ref().to_vec(),
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        fontdb::Source::SharedFile(_, data) => (*data).as_ref().to_vec(),
    };
    let m = {
        use skrifa::MetadataProvider;
        let face = skrifa::FontRef::from_index(&bytes, index).ok()?;
        face.metrics(skrifa::prelude::Size::unscaled(), skrifa::prelude::LocationRef::default())
    };
    let upem = f64::from(m.units_per_em);
    if upem <= 0.0 {
        return None;
    }
    Some(FaceMetrics {
        family: name.to_owned(),
        ascender: f64::from(m.ascent) / upem,
        descender: -f64::from(m.descent) / upem,
        line_gap: f64::from(m.leading) / upem,
        data: Some((Arc::new(bytes), index)),
        units_per_em: upem,
    })
}

/// Resolves a run's font: the installed family, else the default sans-serif face, else the fixed fallback.
#[must_use]
pub fn resolve(family: &str, bold: bool, italic: bool) -> Resolved {
    let f = fonts();
    let installed = match generic(family) {
        Some(g) => Some(f.db.family_name(&g).to_owned()),
        None => installed_name(family),
    };
    let key = (installed.clone().unwrap_or_default(), bold, italic);
    if let Ok(cache) = f.cache.lock()
        && let Some(m) = cache.get(&key)
    {
        return Resolved { face: m.clone(), installed: installed.is_some() };
    }
    let sans = f.db.family_name(&fontdb::Family::SansSerif).to_owned();
    let m = installed
        .as_deref()
        .and_then(|n| load(&f.db, n, bold, italic))
        .or_else(|| load(&f.db, &sans, bold, italic))
        .unwrap_or_else(FaceMetrics::fallback);
    let m = Arc::new(m);
    if let Ok(mut cache) = f.cache.lock() {
        cache.insert(key, m.clone());
    }
    Resolved { face: m, installed: installed.is_some() }
}
