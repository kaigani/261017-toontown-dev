//! The Timeline's geometry (M5 C5.1; `reviews/M5-design-timeline-kernel.md` §5.1): row height, ruler height, the zoom
//! ladder, handle widths, the drag threshold, drop zones and the modifier roles (SI-211). One source,
//! `timeline-layout.json`, read here by the kernel (the timing-gesture interpreter, `timeline.view.set`) and by the
//! window (`desktop/src/timeline/geometry.ts`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// One step of the zoom ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomLevel {
    /// Width of one cell in pixels.
    pub cell_width: u32,
    /// Frames one cell covers (1 above the narrowest readable width; 2, 5 or 10 below it, 04 line 650).
    pub frames_per_cell: u32,
}

/// The modifier roles of one platform (SI-211): each role is the set of modifiers that must be held.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modifiers {
    /// Extend a row or cell selection.
    pub extend: Vec<String>,
    /// Toggle a row in the selection.
    pub toggle: Vec<String>,
    /// Select one cell while single-cell selection is off.
    pub single_cell: Vec<String>,
    /// Copy instead of move when dragging cells.
    pub copy: Vec<String>,
    /// Link instead of move when dragging cells.
    pub link: Vec<String>,
}

/// The Timeline's layout constants.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// File version.
    pub version: u32,
    /// Height of one row.
    pub row_height: u32,
    /// Height of the frame ruler.
    pub ruler_height: u32,
    /// The zoom ladder, narrowest first.
    pub zoom_levels: Vec<ZoomLevel>,
    /// The zoom level of a new Timeline.
    pub default_zoom_level: u32,
    /// The viewport width of a session without a window.
    pub headless_viewport_width: u32,
    /// Width of a bar's start and end handles.
    pub bar_handle_width: u32,
    /// Width of a bar's loop handle.
    pub loop_handle_width: u32,
    /// Radius of a keyframe glyph's hit area.
    pub keyframe_radius: u32,
    /// Pointer travel below which a press and release is a click.
    pub drag_threshold: u32,
    /// The share of a row's height, at its top and at its bottom, that drops between rows.
    pub drop_zone_between: f64,
    /// Empty frames shown past the scene end (04 line 676).
    pub empty_frames_past_end: u32,
    /// Half the hit width of a playback-range marker.
    pub range_marker_half_width: u32,
    /// Modifier roles by platform (`windows`, `linux`, `macos`).
    pub modifiers: BTreeMap<String, Modifiers>,
}

impl Default for Layout {
    /// The values of `timeline-layout.json` (used only if the bundled file failed to parse, which a test rules out).
    fn default() -> Self {
        let z = |cell_width, frames_per_cell| ZoomLevel { cell_width, frames_per_cell };
        Self {
            version: 1,
            row_height: 22,
            ruler_height: 24,
            zoom_levels: vec![
                z(1, 10),
                z(2, 5),
                z(4, 2),
                z(6, 1),
                z(8, 1),
                z(12, 1),
                z(16, 1),
                z(24, 1),
                z(32, 1),
                z(48, 1),
            ],
            default_zoom_level: 6,
            headless_viewport_width: 960,
            bar_handle_width: 5,
            loop_handle_width: 6,
            keyframe_radius: 5,
            drag_threshold: 4,
            drop_zone_between: 0.25,
            empty_frames_past_end: 100,
            range_marker_half_width: 5,
            modifiers: BTreeMap::new(),
        }
    }
}

fn parsed() -> &'static Result<Layout, String> {
    static FILE: OnceLock<Result<Layout, String>> = OnceLock::new();
    FILE.get_or_init(|| serde_json::from_str::<Layout>(include_str!("timeline-layout.json")).map_err(|e| e.to_string()))
}

/// The bundled layout.
#[must_use]
pub fn layout() -> &'static Layout {
    static FALLBACK: OnceLock<Layout> = OnceLock::new();
    match parsed() {
        Ok(l) => l,
        Err(_) => FALLBACK.get_or_init(Layout::default),
    }
}

/// Why the bundled file did not parse (`None` when it did; a test keeps it `None`).
#[must_use]
pub fn problem() -> Option<&'static str> {
    parsed().as_ref().err().map(String::as_str)
}

impl Layout {
    /// The zoom level at `index`, clamped to the ladder.
    #[must_use]
    pub fn zoom(&self, index: u32) -> ZoomLevel {
        let last = self.zoom_levels.len().saturating_sub(1);
        let i = usize::try_from(index).unwrap_or(usize::MAX).min(last);
        self.zoom_levels.get(i).copied().unwrap_or(ZoomLevel { cell_width: 16, frames_per_cell: 1 })
    }

    /// The highest zoom level index.
    #[must_use]
    pub fn max_zoom(&self) -> u32 {
        u32::try_from(self.zoom_levels.len().saturating_sub(1)).unwrap_or(0)
    }

    /// Frames visible in a viewport `width` pixels wide at zoom level `index` (at least one cell).
    #[must_use]
    pub fn visible_frames(&self, width: u32, index: u32) -> u32 {
        let z = self.zoom(index);
        (width / z.cell_width.max(1)).max(1).saturating_mul(z.frames_per_cell.max(1))
    }

    /// The modifier roles of `platform` (the Windows table when the platform is unknown).
    #[must_use]
    pub fn modifiers(&self, platform: &str) -> Modifiers {
        self.modifiers.get(platform).or_else(|| self.modifiers.get("windows")).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_layout_parses_and_matches_the_fallback() {
        assert_eq!(problem(), None);
        let l = layout();
        let d = Layout::default();
        assert_eq!((l.row_height, l.ruler_height, &l.zoom_levels), (d.row_height, d.ruler_height, &d.zoom_levels));
        assert_eq!(l.default_zoom_level, d.default_zoom_level);
        assert!(l.zoom_levels.windows(2).all(|w| w[0].cell_width < w[1].cell_width));
        for p in ["windows", "linux", "macos"] {
            let m = l.modifiers(p);
            assert!(!m.extend.is_empty() && !m.copy.is_empty() && !m.link.is_empty(), "{p}");
        }
        assert_eq!(l.visible_frames(960, 6), 60);
        assert_eq!(l.visible_frames(960, 0), 9600);
    }
}
