//! Text objects (02 §3.7, §7.6; 10 §4.9): content edits on code points with their character runs and paragraphs,
//! formatting of ranges, and layout for the text box bounds.
//!
//! Persisted form (10 §4.9): `text`; `runs` = `[{start, length, font, size, bold, italic, script, kerning,
//! autoKerning, colourId}]` covering the text consecutively; `paragraphs` = `[{start, length, alignment, marginLeft,
//! marginRight, indent, lineSpacing}]`, one per line of the text (a paragraph includes its terminating `\n`). Sizes,
//! margins, indent and line spacing are drawing units and kerning is 1/100 em (02 §7.6.1, §14 decision 16; SI-055).
//! Layout: the transform's origin is the top-left corner of the box; lines run downwards (−y in drawing space).

use serde::{Deserialize, Serialize};

use crate::fonts;
use crate::geom::{Affine, P, Rect};

/// The default font name of new text: the generic sans-serif family, resolved to the platform's default sans-serif
/// face at layout, so the persisted name is the same on every OS (02 §7.6.1 "platform default sans-serif").
pub const DEFAULT_FONT: &str = "sans-serif";

/// Most code points in one text object (10 §4.9).
pub const MAX_TEXT: usize = 100_000;

/// A character run (persisted form).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// First code point.
    pub start: u32,
    /// Number of code points.
    pub length: u32,
    /// Font family (kept when missing).
    pub font: String,
    /// Size (du).
    pub size: f64,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// `normal`, `subscript` or `superscript`.
    pub script: String,
    /// Manual kerning (1/100 em).
    pub kerning: f64,
    /// Font kerning pairs.
    pub auto_kerning: bool,
    /// Fill colour.
    pub colour_id: Option<String>,
}

/// A paragraph (persisted form).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Para {
    /// First code point.
    pub start: u32,
    /// Number of code points (with the terminating newline).
    pub length: u32,
    /// `left`, `centre`, `right` or `justify`.
    pub alignment: String,
    /// Left margin (du).
    pub margin_left: f64,
    /// Right margin (du).
    pub margin_right: f64,
    /// First-line indent (du).
    pub indent: f64,
    /// Additional spacing between lines (du).
    pub line_spacing: f64,
}

/// Formatting fields of 02 §13.5 `draw.textFormat.set` / the Text panel (command names; `None` = unchanged).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Format {
    /// Font family.
    pub font: Option<String>,
    /// Size (du, 1–512).
    pub size: Option<f64>,
    /// Bold.
    pub bold: Option<bool>,
    /// Italic.
    pub italic: Option<bool>,
    /// Baseline (`normal` | `subscript` | `superscript`; persisted as `script`).
    pub baseline: Option<String>,
    /// Kerning (1/100 em, −60–60).
    pub kerning: Option<f64>,
    /// Automatic kerning.
    pub auto_kern: Option<bool>,
    /// Fill colour (`Some(None)` clears it).
    pub colour_id: Option<Option<String>>,
    /// Paragraph alignment (`left` | `centre` | `right` | `justify`).
    pub align: Option<String>,
    /// Left margin (du, 0–1000).
    pub left_margin: Option<f64>,
    /// Right margin (du, 0–1000).
    pub right_margin: Option<f64>,
    /// First-line indent (du, −99–1000).
    pub indent: Option<f64>,
    /// Line spacing (du, −99–1000).
    pub line_spacing: Option<f64>,
}

impl Format {
    /// Whether any character field is set.
    #[must_use]
    pub fn has_character(&self) -> bool {
        self.font.is_some()
            || self.size.is_some()
            || self.bold.is_some()
            || self.italic.is_some()
            || self.baseline.is_some()
            || self.kerning.is_some()
            || self.auto_kern.is_some()
            || self.colour_id.is_some()
    }

    fn apply_run(&self, r: &mut Run) {
        if let Some(v) = &self.font {
            r.font.clone_from(v);
        }
        if let Some(v) = self.size {
            r.size = v;
        }
        if let Some(v) = self.bold {
            r.bold = v;
        }
        if let Some(v) = self.italic {
            r.italic = v;
        }
        if let Some(v) = &self.baseline {
            r.script.clone_from(v);
        }
        if let Some(v) = self.kerning {
            r.kerning = v;
        }
        if let Some(v) = self.auto_kern {
            r.auto_kerning = v;
        }
        if let Some(v) = &self.colour_id {
            r.colour_id.clone_from(v);
        }
    }

    fn apply_para(&self, p: &mut Para) {
        if let Some(v) = &self.align {
            p.alignment.clone_from(v);
        }
        if let Some(v) = self.left_margin {
            p.margin_left = v;
        }
        if let Some(v) = self.right_margin {
            p.margin_right = v;
        }
        if let Some(v) = self.indent {
            p.indent = v;
        }
        if let Some(v) = self.line_spacing {
            p.line_spacing = v;
        }
    }
}

/// The Text panel defaults (02 §7.6.1) with an optional fill colour.
#[must_use]
pub fn default_run(colour_id: Option<String>) -> Run {
    Run {
        start: 0,
        length: 0,
        font: DEFAULT_FONT.to_owned(),
        size: 36.0,
        bold: false,
        italic: false,
        script: "normal".to_owned(),
        kerning: 0.0,
        auto_kerning: true,
        colour_id,
    }
}

/// The paragraph defaults (02 §7.6.1).
#[must_use]
pub fn default_para() -> Para {
    Para {
        start: 0,
        length: 0,
        alignment: "left".to_owned(),
        margin_left: 0.0,
        margin_right: 0.0,
        indent: 0.0,
        line_spacing: 0.0,
    }
}

/// Text content: characters, runs and one paragraph format per line.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    /// Code points.
    pub chars: Vec<char>,
    /// Runs covering `chars` consecutively (one zero-length run when empty).
    pub runs: Vec<Run>,
    /// One paragraph per line (`start`/`length` recomputed by [`Content::normalize`]).
    pub paras: Vec<Para>,
}

fn u(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Content {
    /// New content in one run and default paragraphs.
    #[must_use]
    pub fn new(text: &str, run: Run, para: Para) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let lines = chars.iter().filter(|c| **c == '\n').count() + 1;
        let mut c = Content { chars, runs: vec![run], paras: vec![para; lines] };
        c.runs[0].start = 0;
        c.runs[0].length = u(c.chars.len());
        c.normalize();
        c
    }

    /// From the persisted fields (runs and paragraphs as stored; paragraphs mapped to lines in order).
    #[must_use]
    pub fn from_parts(text: &str, runs: Vec<Run>, paras: Vec<Para>) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let lines = chars.iter().filter(|c| **c == '\n').count() + 1;
        let mut ps: Vec<Para> = paras;
        let fallback = ps.last().cloned().unwrap_or_else(default_para);
        ps.resize(lines, fallback);
        let runs = if runs.is_empty() { vec![default_run(None)] } else { runs };
        let mut c = Content { chars, runs, paras: ps };
        c.normalize();
        c
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    fn line_of(&self, at: usize) -> usize {
        self.chars[..at.min(self.chars.len())].iter().filter(|c| **c == '\n').count()
    }

    /// Recomputes run starts, drops empty runs (keeping one), merges equal neighbours, and recomputes paragraph
    /// ranges from the lines. The text is first put in NFC (10 §1.1), with run lengths following the composition.
    pub fn normalize(&mut self) {
        self.compose();
        let template = self.runs_template();
        let mut runs: Vec<Run> = Vec::with_capacity(self.runs.len());
        for r in self.runs.drain(..) {
            if r.length == 0 {
                continue;
            }
            if let Some(last) = runs.last_mut()
                && same_format(last, &r)
            {
                last.length += r.length;
                continue;
            }
            runs.push(r);
        }
        if runs.is_empty() {
            let mut keep = template;
            keep.length = 0;
            runs.push(keep);
        }
        let mut pos = 0u32;
        for r in &mut runs {
            r.start = pos;
            pos += r.length;
        }
        self.runs = runs;
        let mut start = 0usize;
        let n = self.chars.len();
        let mut line = 0usize;
        for i in 0..=n {
            if i == n || self.chars[i] == '\n' {
                let end = if i == n { n } else { i + 1 };
                if let Some(p) = self.paras.get_mut(line) {
                    p.start = u(start);
                    p.length = u(end - start);
                }
                start = end;
                line += 1;
            }
        }
        self.paras.truncate(line);
    }

    /// Normalises the text to NFC (10 §1.1) and keeps every code point's format: each code point of the normalised
    /// text comes from one code point of the input (see [`nfc_with_origins`]) and takes that code point's own run. A
    /// composite takes the run of its starter, so a combining mark (or a vowel sign of class 0, as in Bengali) typed
    /// after a base joins the base's run while unrelated characters keep theirs (C21-R1-09, C21-R2-02). The runs are
    /// rebuilt in output order from each code point's source run, so a mark that canonical ordering moves keeps its
    /// own format: a source run is split, and revisited, as often as needed (C21-R3-03).
    fn compose(&mut self) {
        let text: String = self.chars.iter().collect();
        if unicode_normalization::is_nfc(&text) {
            return;
        }
        // The run of every code point.
        let mut owner: Vec<usize> = Vec::with_capacity(self.chars.len());
        for (i, r) in self.runs.iter().enumerate() {
            owner.extend(std::iter::repeat_n(i, r.length as usize));
        }
        owner.resize(self.chars.len(), self.runs.len().saturating_sub(1));
        let mapped = nfc_with_origins(&self.chars);
        let template = self.runs_template();
        let mut runs: Vec<Run> = Vec::with_capacity(self.runs.len() + 2);
        let mut current: Option<usize> = None;
        for (_, origin) in &mapped {
            let source = owner.get(*origin).copied().unwrap_or(0);
            match (current, runs.last_mut()) {
                (Some(c), Some(last)) if c == source => last.length += 1,
                _ => {
                    let mut r = self.runs.get(source).cloned().unwrap_or_else(|| template.clone());
                    r.length = 1;
                    runs.push(r);
                    current = Some(source);
                }
            }
        }
        // `normalize` merges neighbours of equal format and recomputes the starts.
        self.runs = runs;
        self.chars = mapped.into_iter().map(|(c, _)| c).collect();
    }

    fn runs_template(&self) -> Run {
        self.runs.first().cloned().unwrap_or_else(|| default_run(None))
    }

    /// Splits the runs so a boundary falls at `at`.
    fn split_at(&mut self, at: usize) {
        let mut pos = 0usize;
        for i in 0..self.runs.len() {
            let len = self.runs[i].length as usize;
            if at > pos && at < pos + len {
                let mut tail = self.runs[i].clone();
                self.runs[i].length = u(at - pos);
                tail.length = u(pos + len - at);
                self.runs.insert(i + 1, tail);
                return;
            }
            pos += len;
        }
    }

    /// Inserts text at code point `at` (typing continues the format of the character before, or the first run).
    pub fn insert(&mut self, at: usize, s: &str) {
        let at = at.min(self.chars.len());
        let new: Vec<char> = s.chars().collect();
        // Paragraphs: the line holding `at` splits at every inserted newline, each part keeping its format.
        let line = self.line_of(at);
        let breaks = new.iter().filter(|c| **c == '\n').count();
        if let Some(p) = self.paras.get(line).cloned() {
            for _ in 0..breaks {
                self.paras.insert(line, p.clone());
            }
        }
        // Runs: the run containing the character before `at` grows.
        let mut pos = 0usize;
        let mut grown = false;
        for r in &mut self.runs {
            let len = r.length as usize;
            if (at > pos && at <= pos + len) || (at == 0 && pos == 0) {
                r.length += u(new.len());
                grown = true;
                break;
            }
            pos += len;
        }
        if !grown && let Some(r) = self.runs.last_mut() {
            r.length += u(new.len());
        }
        self.chars.splice(at..at, new);
        self.normalize();
    }

    /// Deletes `len` code points from `start` (paragraphs whose newline is deleted merge into the first).
    pub fn delete(&mut self, start: usize, len: usize) {
        let start = start.min(self.chars.len());
        let end = (start + len).min(self.chars.len());
        if end <= start {
            return;
        }
        let line = self.line_of(start);
        let removed_breaks = self.chars[start..end].iter().filter(|c| **c == '\n').count();
        for _ in 0..removed_breaks {
            if line + 1 < self.paras.len() {
                self.paras.remove(line + 1);
            }
        }
        self.split_at(start);
        self.split_at(end);
        let mut pos = 0usize;
        for r in &mut self.runs {
            let l = r.length as usize;
            if pos >= start && pos + l <= end {
                r.length = 0;
            }
            pos += l;
        }
        let template = self.runs_template();
        self.chars.drain(start..end);
        if self.runs.iter().all(|r| r.length == 0) {
            let mut t = template;
            t.length = 0;
            self.runs = vec![t];
        }
        self.normalize();
    }

    /// Applies a format to `start .. start + len`: character fields to the characters in range (to the only run when
    /// the text is empty), paragraph fields to every paragraph the range touches.
    pub fn format(&mut self, start: usize, len: usize, f: &Format) {
        let start = start.min(self.chars.len());
        let end = (start + len).min(self.chars.len());
        if f.has_character() {
            if self.chars.is_empty() {
                for r in &mut self.runs {
                    f.apply_run(r);
                }
            } else {
                self.split_at(start);
                self.split_at(end);
                let mut pos = 0usize;
                for r in &mut self.runs {
                    let l = r.length as usize;
                    if pos >= start && pos + l <= end && l > 0 {
                        f.apply_run(r);
                    }
                    pos += l;
                }
            }
        }
        let first = self.line_of(start);
        let last = if end > start { self.line_of(end - 1) } else { first };
        for p in self.paras.iter_mut().take(last + 1).skip(first) {
            f.apply_para(p);
        }
        self.normalize();
    }

    /// The run holding code point `i` (the last run at the end).
    #[must_use]
    pub fn run_at(&self, i: usize) -> &Run {
        let mut pos = 0usize;
        for r in &self.runs {
            if i < pos + r.length as usize {
                return r;
            }
            pos += r.length as usize;
        }
        &self.runs[self.runs.len() - 1]
    }
}

fn same_format(a: &Run, b: &Run) -> bool {
    a.font == b.font
        && a.size == b.size
        && a.bold == b.bold
        && a.italic == b.italic
        && a.script == b.script
        && a.kerning == b.kerning
        && a.auto_kerning == b.auto_kerning
        && a.colour_id == b.colour_id
}

/// The result of laying out a text object.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Box width (du, local).
    pub width: f64,
    /// Box height (du, local).
    pub height: f64,
    /// Number of laid-out lines.
    pub lines: usize,
    /// Requested fonts that are not installed (02 §3.7: displayed with a fallback), sorted.
    pub missing_fonts: Vec<String>,
}

/// Lays out the content: `width = Some(w)` wraps at `w` (fixed width), `None` is auto width.
#[must_use]
pub fn layout(c: &Content, fixed_width: Option<f64>) -> Layout {
    let mut missing: Vec<String> = Vec::new();
    // Per character: advance (du) and line height contribution.
    let n = c.chars.len();
    let mut adv = vec![0.0; n];
    let mut height = vec![0.0; n];
    let mut i = 0;
    let mut kern_prev: Vec<f64> = vec![0.0; n];
    for r in &c.runs {
        let len = r.length as usize;
        let res = fonts::resolve(&r.font, r.bold, r.italic);
        if !res.installed && !missing.contains(&r.font) {
            missing.push(r.font.clone());
        }
        let size = if r.script == "normal" { r.size } else { r.size * 0.6 };
        let slice: Vec<char> = c.chars[i..(i + len).min(n)].to_vec();
        let (a, k) = res.face.advances(&slice);
        let lh = (res.face.ascender + res.face.descender + res.face.line_gap) * size;
        for j in 0..slice.len() {
            adv[i + j] = a[j] * size + r.kerning / 100.0 * size;
            height[i + j] = lh;
            if r.auto_kerning && j > 0 {
                kern_prev[i + j] = k[j - 1] * size;
            }
        }
        i += len;
    }
    missing.sort();
    let mut box_w: f64 = 0.0;
    let mut box_h = 0.0;
    let mut lines = 0usize;
    for p in &c.paras {
        let s = p.start as usize;
        let e = (p.start + p.length) as usize;
        let body: Vec<usize> = (s..e.min(n)).filter(|k| c.chars[*k] != '\n').collect();
        let empty_h = c.run_at(s.min(n.saturating_sub(1))).size;
        let empty_h = {
            let r = c.run_at(s.min(n.saturating_sub(1)));
            let res = fonts::resolve(&r.font, r.bold, r.italic);
            (res.face.ascender + res.face.descender + res.face.line_gap)
                * if r.script == "normal" { empty_h } else { empty_h * 0.6 }
        };
        // Greedy word wrap.
        let mut line_start = 0usize;
        let mut first = true;
        loop {
            let indent = if first { p.indent } else { 0.0 };
            let avail = fixed_width.map(|w| w - p.margin_left - p.margin_right - indent);
            let mut w = 0.0;
            let mut h: f64 = 0.0;
            let mut end = line_start;
            let mut last_break: Option<usize> = None;
            while end < body.len() {
                let k = body[end];
                let step = adv[k] + if end > line_start { kern_prev[k] } else { 0.0 };
                if let Some(a) = avail
                    && w + step > a
                    && end > line_start
                {
                    break;
                }
                w += step;
                h = h.max(height[k]);
                if c.chars[k] == ' ' {
                    last_break = Some(end + 1);
                }
                end += 1;
            }
            if end < body.len()
                && let Some(b) = last_break
                && b > line_start
            {
                end = b;
                w = (line_start..end).map(|q| adv[body[q]]).sum();
                h = (line_start..end).map(|q| height[body[q]]).fold(0.0, f64::max);
            }
            if end == line_start && body.is_empty() {
                h = empty_h;
            }
            let line_w = w + p.margin_left + p.margin_right + indent;
            box_w = box_w.max(line_w);
            box_h += h.max(if body.is_empty() { empty_h } else { 0.0 }) + p.line_spacing;
            lines += 1;
            first = false;
            if end >= body.len() {
                break;
            }
            line_start = end;
        }
    }
    let width = fixed_width.unwrap_or(box_w).max(0.0);
    Layout { width, height: box_h.max(0.0), lines, missing_fonts: missing }
}

/// The drawing-space bounds of a text box laid out as `l` under `transform` (origin = top-left corner).
#[must_use]
pub fn bounds(l: &Layout, transform: &Affine) -> Rect {
    let corners = [P::new(0.0, 0.0), P::new(l.width, 0.0), P::new(l.width, -l.height), P::new(0.0, -l.height)];
    let mut r = Rect::of(transform.apply(corners[0]));
    for c in &corners[1..] {
        r = r.with(transform.apply(*c));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(t: &str) -> Content {
        Content::new(t, default_run(Some("col_ink".into())), default_para())
    }

    #[test]
    fn new_text_has_one_run_and_one_paragraph_per_line() {
        let c = content("Hello\nworld");
        assert_eq!(c.runs.len(), 1);
        assert_eq!(c.runs[0].length, 11);
        assert_eq!(c.paras.len(), 2);
        assert_eq!((c.paras[0].start, c.paras[0].length), (0, 6));
        assert_eq!((c.paras[1].start, c.paras[1].length), (6, 5));
    }

    #[test]
    fn format_splits_and_merges_runs() {
        let mut c = content("Hello");
        c.format(1, 2, &Format { bold: Some(true), ..Format::default() });
        assert_eq!(
            c.runs.iter().map(|r| (r.start, r.length, r.bold)).collect::<Vec<_>>(),
            vec![(0, 1, false), (1, 2, true), (3, 2, false)]
        );
        c.format(0, 5, &Format { bold: Some(false), ..Format::default() });
        assert_eq!(c.runs.len(), 1);
    }

    #[test]
    fn typing_and_deleting_keep_runs_covering_the_text() {
        let mut c = content("Hello");
        c.format(0, 5, &Format { italic: Some(true), ..Format::default() });
        c.insert(5, " there\nfriend");
        assert_eq!(c.text(), "Hello there\nfriend");
        assert_eq!(c.runs.iter().map(|r| r.length).sum::<u32>(), 18);
        assert!(c.runs[0].italic);
        assert_eq!(c.paras.len(), 2);
        c.delete(5, 7);
        assert_eq!(c.text(), "Hellofriend");
        assert_eq!(c.paras.len(), 1);
        c.delete(0, 11);
        assert_eq!(c.runs.len(), 1);
        assert_eq!(c.runs[0].length, 0);
        assert!(c.runs[0].italic, "the format survives on the empty run");
    }

    #[test]
    fn paragraph_format_follows_split_lines() {
        let mut c = content("ab");
        c.format(0, 0, &Format { align: Some("centre".into()), ..Format::default() });
        c.insert(1, "\n");
        assert_eq!(c.paras.iter().map(|p| p.alignment.as_str()).collect::<Vec<_>>(), ["centre", "centre"]);
    }

    #[test]
    fn layout_wraps_at_a_fixed_width() {
        let c = content("one two three four five six");
        let auto = layout(&c, None);
        assert_eq!(auto.lines, 1);
        let fixed = layout(&c, Some(auto.width / 3.0));
        assert!(fixed.lines >= 3, "{fixed:?}");
        assert!(fixed.height > auto.height);
    }
}

/// NFC with the origin of every output code point (UAX #15: canonical decomposition, canonical ordering, canonical
/// composition). Each input code point is decomposed on its own (canonical decomposition is per code point), the
/// non-starter sequences are stably ordered by combining class, and each code point is composed into the last
/// starter when it is not blocked; a composite keeps its starter's origin. The output equals
/// `unicode_normalization`'s NFC of the same text (property-tested), and every output code point names the input
/// code point it came from.
#[must_use]
pub fn nfc_with_origins(chars: &[char]) -> Vec<(char, usize)> {
    use unicode_normalization::char::{canonical_combining_class as ccc, compose, decompose_canonical};
    let mut d: Vec<(char, usize)> = Vec::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        decompose_canonical(*c, |x| d.push((x, i)));
    }
    // Canonical ordering: every maximal run of non-starters, stably by combining class.
    let mut i = 0;
    while i < d.len() {
        if ccc(d[i].0) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < d.len() && ccc(d[i].0) != 0 {
            i += 1;
        }
        d[start..i].sort_by_key(|(c, _)| ccc(*c));
    }
    // Canonical composition.
    let mut out: Vec<(char, usize)> = Vec::with_capacity(d.len());
    let mut starter: Option<usize> = None;
    // Combining class of the last code point appended after the starter (`None`: adjacent to it).
    let mut last: Option<u8> = None;
    for (c, origin) in d {
        let class = ccc(c);
        if let Some(sp) = starter {
            let blocked = last.is_some_and(|l| l == 0 || l >= class);
            if !blocked && let Some(composite) = compose(out[sp].0, c) {
                out[sp].0 = composite;
                continue;
            }
        }
        if class == 0 {
            starter = Some(out.len());
            last = None;
        } else {
            last = Some(class);
        }
        out.push((c, origin));
    }
    out
}

#[cfg(test)]
mod nfc_tests {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    #[test]
    fn decomposed_input_is_composed_and_runs_follow() {
        let c = Content::new("e\u{301}x", default_run(None), default_para());
        assert_eq!(c.text(), "\u{e9}x");
        assert_eq!(c.runs[0].length, 2);
        assert_eq!(c.paras[0].length, 2);
    }

    #[test]
    fn a_combining_mark_typed_after_a_base_joins_the_base_and_its_run() {
        let mut c = Content::new("ab", default_run(None), default_para());
        c.format(0, 1, &Format { bold: Some(true), ..Format::default() });
        c.insert(1, "\u{301}");
        assert_eq!(c.text(), "\u{e1}b");
        assert_eq!(c.runs.iter().map(|r| (r.length, r.bold)).collect::<Vec<_>>(), vec![(1, true), (1, false)]);
        // At a run boundary: a mark inserted at the start of the second run composes with the first run's character.
        let mut d = Content::new("ab", default_run(None), default_para());
        d.format(1, 1, &Format { italic: Some(true), ..Format::default() });
        d.insert(1, "\u{308}");
        assert_eq!(d.text(), "\u{e4}b");
        assert_eq!(d.runs.iter().map(|r| (r.length, r.italic)).collect::<Vec<_>>(), vec![(1, false), (1, true)]);
    }

    #[test]
    fn deleting_between_a_base_and_a_mark_composes_them() {
        let mut c = Content::new("ex", default_run(None), default_para());
        c.insert(2, "\u{301}");
        assert_eq!(c.text(), "ex\u{301}");
        c.delete(1, 1);
        assert_eq!(c.text(), "\u{e9}");
        assert_eq!(c.runs[0].length, 1);
    }

    #[test]
    fn a_class_zero_composition_keeps_unrelated_formats() {
        // KA + vowel sign E (run 1) + vowel sign AA + Z (run 2, bold): E + AA compose to O in E's run; Z stays bold.
        let mut a = default_run(None);
        a.length = 2;
        let mut b = a.clone();
        b.start = 2;
        b.bold = true;
        let c = Content::from_parts("\u{995}\u{9c7}\u{9be}Z", vec![a, b], vec![default_para()]);
        assert_eq!(c.text(), "\u{995}\u{9cb}Z");
        assert_eq!(c.runs.iter().map(|r| (r.length, r.bold)).collect::<Vec<_>>(), vec![(2, false), (1, true)]);
    }

    #[test]
    fn origins_follow_reordering_and_blocking() {
        // a + dot below (220) after acute (230): reordered, then a + dot below composes, acute stays.
        let v = nfc_with_origins(&['a', '\u{301}', '\u{323}']);
        let s: String = v.iter().map(|(c, _)| *c).collect();
        assert_eq!(s, "a\u{301}\u{323}".nfc().collect::<String>());
        assert_eq!(v[0].1, 0);
        assert_eq!(v[1], ('\u{301}', 1));
    }

    #[test]
    fn nfc_with_origins_matches_the_library_on_random_text() {
        // A fixed-seed corpus over the code points that compose, reorder or block.
        let pool: Vec<char> = "aeouAEOUnZ\n \u{300}\u{301}\u{302}\u{308}\u{323}\u{327}\u{328}\u{345}\u{1100}\u{1161}\u{11a8}\u{ac00}\u{995}\u{9c7}\u{9be}\u{9d7}\u{9bc}\u{b47}\u{b3e}\u{b56}\u{1b05}\u{1b35}\u{3099}\u{304b}\u{fb1d}\u{5b4}\u{e9}\u{1e0b}\u{212b}\u{f900}"
            .chars()
            .collect();
        let mut seed: u64 = 0x7474_6332_315f_7232;
        for _ in 0..20_000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let n = 1 + (seed >> 59) as usize;
            let mut chars = Vec::with_capacity(n);
            let mut k = seed;
            for _ in 0..n {
                k = k.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                chars.push(pool[(k >> 33) as usize % pool.len()]);
            }
            let ours: String = nfc_with_origins(&chars).iter().map(|(c, _)| *c).collect();
            let theirs: String = chars.iter().collect::<String>().nfc().collect();
            assert_eq!(ours, theirs, "{chars:?}");
        }
    }

    #[test]
    fn hangul_jamo_compose_into_one_syllable() {
        let c = Content::new("\u{1100}\u{1161}\u{11A8}", default_run(None), default_para());
        assert_eq!(c.chars.len(), 1);
        assert_eq!(c.runs[0].length, 1);
    }
}
