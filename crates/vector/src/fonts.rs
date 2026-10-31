//! Fonts for text layout (02 §3.7) and the `missingFonts` lint (`doc.validate`).
//!
//! The system font list is read once per process (`fontdb`, the platform's font folders) and font files are parsed
//! with `skrifa` for the metrics layout needs: units per em, ascender, descender, line gap, horizontal advances
//! and `kern`-table pairs. A run whose font is not installed keeps its requested name (§3.7) and is laid out with the
//! platform's default sans-serif face; when no face can be read at all, a fixed fallback metric (advance 0.5 em,
//! ascender 0.8 em, descender 0.2 em) keeps layout defined. Layout bounds are derived data: they depend on the font
//! files of the machine, never the persisted text (ADR-0016).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

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

struct Fonts {
    db: fontdb::Database,
    cache: Mutex<BTreeMap<(String, bool, bool), Arc<FaceMetrics>>>,
}

fn fonts() -> &'static Fonts {
    static F: OnceLock<Fonts> = OnceLock::new();
    F.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Fonts { db, cache: Mutex::new(BTreeMap::new()) }
    })
}

/// Starts reading the system font list on a background thread, so the first text command does not pay for it.
pub fn warm() {
    std::thread::spawn(|| {
        let _ = fonts();
    });
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
