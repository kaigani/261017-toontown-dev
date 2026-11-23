//! The one gate between the engine and the system font list (ruling E-43, `reviews/M5-design-font-scan.md`).
//!
//! Text layout (a text node's bounds, its glyphs, the Text tool's carets) and the missing-fonts check need the list of
//! the machine's fonts, read once per process on a background thread from the engine's start (`tt_vector::fonts`).
//! Every product path into that list goes through this module: a path that involves no text never waits; one that does
//! waits for the list at most `tt_vector::fonts::WAIT_BOUND` (20 s) and then answers `command.timeout` with
//! `details.reason: "fontsLoading"` (SI-215) before it changes anything, so a later retry succeeds. Once the list is
//! read every call returns at once. Text is never laid out with a partial list. A static test
//! (`fonts_gate::tests::no_product_file_reads_the_font_list_past_the_gate`) keeps the product files from calling the
//! layout functions directly.

use serde_json::json;
use tt_model::entities::drawing::{Drawing, DrawingNode, NodeKind};
use tt_registry::{CommandError, codes};
use tt_vector::drawing as vd;
use tt_vector::geom::Rect;
use tt_vector::text;

/// The `details.reason` of a command that waited for the font list past the bound (ruling E-43, SI-215).
pub(crate) const FONTS_LOADING: &str = "fontsLoading";

/// The answer past the bound.
pub(crate) fn loading() -> CommandError {
    CommandError::with_details(
        codes::COMMAND_TIMEOUT,
        "the system fonts are still being read",
        json!({ "reason": FONTS_LOADING, "boundMs": u64::try_from(tt_vector::fonts::wait_bound().as_millis()).unwrap_or(u64::MAX) }),
    )
    .hint("the system fonts are still being read; retry in a few seconds (nothing was changed)")
}

/// Waits for the font list within the bound.
///
/// # Errors
///
/// `command.timeout` with reason `fontsLoading` past the bound.
pub(crate) fn ready() -> Result<(), CommandError> {
    tt_vector::fonts::ready_within(tt_vector::fonts::wait_bound()).map_err(|_| loading())
}

/// Whether a node is or holds a text node.
pub(crate) fn node_has_text(n: &DrawingNode) -> bool {
    match n.kind() {
        NodeKind::Text => true,
        NodeKind::Group => n.children().as_ref().is_some_and(|cs| cs.iter().any(node_has_text)),
        NodeKind::Layer | NodeKind::Bitmap => false,
    }
}

/// Whether a drawing holds a text node.
pub(crate) fn drawing_has_text(d: &Drawing) -> bool {
    d.nodes().iter().any(node_has_text)
}

/// Waits for the font list if the node involves text.
///
/// # Errors
///
/// As [`ready`].
pub(crate) fn for_node(n: &DrawingNode) -> Result<(), CommandError> {
    if node_has_text(n) { ready() } else { Ok(()) }
}

/// Waits for the font list if the drawing involves text.
///
/// # Errors
///
/// As [`ready`].
pub(crate) fn for_drawing(d: &Drawing) -> Result<(), CommandError> {
    if drawing_has_text(d) { ready() } else { Ok(()) }
}

/// `tt_vector::drawing::drawing_bounds` behind the gate.
///
/// # Errors
///
/// As [`ready`], when the drawing holds text.
pub(crate) fn drawing_bounds(d: &Drawing, include_invisible: bool) -> Result<Option<Rect>, CommandError> {
    for_drawing(d)?;
    Ok(vd::drawing_bounds(d, include_invisible))
}

/// `tt_vector::drawing::node_bounds` behind the gate.
///
/// # Errors
///
/// As [`ready`], when the node holds text.
pub(crate) fn node_bounds(n: &DrawingNode, include_invisible: bool) -> Result<Option<Rect>, CommandError> {
    for_node(n)?;
    Ok(vd::node_bounds(n, include_invisible))
}

/// Proof that the font list is read (ruling E-43): text layout, carets, glyphs and the installed-font check are its
/// methods, so no product path lays out text without having waited for the list ([`ready`]) or found it read ([`now`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Read(());

/// Waits for the font list within the bound.
///
/// # Errors
///
/// `command.timeout` with reason `fontsLoading` past the bound.
pub(crate) fn read() -> Result<Read, CommandError> {
    ready().map(|()| Read(()))
}

/// The proof without waiting, when the list is already read (paths that must never wait: the window's state reads and
/// the document thread's housekeeping after a call; they skip text until the list is read).
pub(crate) fn now() -> Option<Read> {
    tt_vector::fonts::is_ready().then_some(Read(()))
}

impl Read {
    /// `tt_vector::text::layout`.
    pub(crate) fn layout(self, c: &text::Content, fixed_width: Option<f64>) -> text::Layout {
        text::layout(c, fixed_width)
    }

    /// `tt_vector::text::carets`.
    pub(crate) fn carets(self, c: &text::Content, fixed_width: Option<f64>) -> Vec<text::Caret> {
        text::carets(c, fixed_width)
    }

    /// `tt_vector::text::caret_at`.
    pub(crate) fn caret_at(self, c: &text::Content, fixed_width: Option<f64>, local: tt_vector::geom::P) -> usize {
        text::caret_at(c, fixed_width, local)
    }

    /// `tt_vector::text::caret_x`.
    pub(crate) fn caret_x(self, c: &text::Content, fixed_width: Option<f64>, caret: usize) -> f64 {
        text::caret_x(c, fixed_width, caret)
    }

    /// `tt_vector::text::line_move`.
    pub(crate) fn line_move(
        self,
        c: &text::Content,
        fixed_width: Option<f64>,
        caret: usize,
        goal_x: f64,
        dir: i32,
    ) -> usize {
        text::line_move(c, fixed_width, caret, goal_x, dir)
    }

    /// `tt_vector::ops::glyphs::shapes`.
    pub(crate) fn shapes(self, c: &text::Content, fixed_width: Option<f64>) -> Vec<tt_vector::ops::glyphs::GlyphShape> {
        tt_vector::ops::glyphs::shapes(c, fixed_width)
    }

    /// `tt_vector::fonts::is_installed`.
    pub(crate) fn is_installed(self, family: &str) -> bool {
        tt_vector::fonts::is_installed(family)
    }

    /// `tt_vector::drawing::text_layout`.
    pub(crate) fn text_layout(self, n: &DrawingNode) -> Option<text::Layout> {
        vd::text_layout(n)
    }

    /// `tt_vector::drawing::drawing_bounds`.
    pub(crate) fn drawing_bounds(self, d: &Drawing, include_invisible: bool) -> Option<Rect> {
        vd::drawing_bounds(d, include_invisible)
    }

    /// `tt_vector::drawing::node_bounds`.
    pub(crate) fn node_bounds(self, n: &DrawingNode, include_invisible: bool) -> Option<Rect> {
        vd::node_bounds(n, include_invisible)
    }
}

/// A drawing's bounds without waiting: `None` while the list is not read and the drawing holds text (paths that must
/// never wait skip it until the list is read).
pub(crate) fn drawing_bounds_now(d: &Drawing, include_invisible: bool) -> Option<Option<Rect>> {
    if drawing_has_text(d) {
        now().map(|r| r.drawing_bounds(d, include_invisible))
    } else {
        Some(vd::drawing_bounds(d, include_invisible))
    }
}

/// A node's bounds without waiting (as [`drawing_bounds_now`]).
pub(crate) fn node_bounds_now(n: &DrawingNode, include_invisible: bool) -> Option<Option<Rect>> {
    if node_has_text(n) {
        now().map(|r| r.node_bounds(n, include_invisible))
    } else {
        Some(vd::node_bounds(n, include_invisible))
    }
}

/// `tt_vector::drawing::node_json` behind the gate (its bounds lay out text).
///
/// # Errors
///
/// As [`ready`], when bounds are included and the node holds text.
pub(crate) fn node_json(n: &DrawingNode, inc: vd::Include) -> Result<serde_json::Value, CommandError> {
    if inc.bounds {
        for_node(n)?;
    }
    Ok(vd::node_json(n, inc))
}

/// `tt_vector::drawing::flat_json` behind the gate.
///
/// # Errors
///
/// As [`ready`], when bounds are included and the drawing holds text.
pub(crate) fn flat_json(d: &Drawing, inc: vd::Include) -> Result<Vec<serde_json::Value>, CommandError> {
    if inc.bounds {
        for_drawing(d)?;
    }
    Ok(vd::flat_json(d, inc))
}

#[cfg(test)]
mod tests {
    /// Ruling E-43: no product file reads the font list past the gate (the calls that lay out text, compute text bounds
    /// or ask whether a font is installed go through this module), so no caller can wait without the bound.
    #[test]
    fn no_product_file_reads_the_font_list_past_the_gate() -> Result<(), String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let banned = [
            "drawing_bounds(",
            "node_bounds(",
            "text_layout(",
            "node_json(",
            "flat_json(",
            "text::layout(",
            "text::layout_placed(",
            "text::layout_lines(",
            "text::carets(",
            "text::caret_at(",
            "text::caret_x(",
            "text::line_move(",
            "glyphs::shapes(",
            "fonts::is_installed(",
            "fonts::installed_families(",
            "fonts::resolve(",
        ];
        let mut found = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|x| x != "rs") || path.ends_with("fonts_gate.rs") {
                    continue;
                }
                let src = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                // Test modules may call the layout directly (they run after the list is read).
                let product = src.split("#[cfg(test)]").next().unwrap_or(&src);
                for (i, line) in product.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or(line);
                    for b in banned {
                        for (at, _) in code.match_indices(b) {
                            let before = &code[..at];
                            // A method of the gate's `Read` proof (`fonts.layout(`), a longer name (`text_node_json(`),
                            // a definition (`fn x(`) or the gate itself (`fonts_gate::x(`) is not a direct call.
                            let prev = before.chars().next_back();
                            if prev.is_some_and(|c| c == '.' || c == '_' || c.is_alphanumeric())
                                || before.ends_with("fn ")
                                || before.ends_with("fonts_gate::")
                            {
                                continue;
                            }
                            found.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
        if found.is_empty() { Ok(()) } else { Err(format!("past the font gate:\n{}", found.join("\n"))) }
    }
}
