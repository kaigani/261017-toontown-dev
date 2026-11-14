//! The user's drawing settings (02 §5, §6.2-6.3, §7.7, §10, §12.3; M4 C4.2, reviews/M4-design-gestures.md §1): the
//! active tool and the last-used tool per group, per-tool options, the global drawing toggles, symmetry, tablet input
//! settings and the drawing aids. User scope: kept in [`crate::ui::UiState`] (so rollback and savepoints carry it), not
//! undoable (02 §12.1), and written to the `drawing` settings file whenever it changes, under the settings files'
//! version, shape and corrupt-file rules (ADR-0021 decision 13; ruling D-7d). Pens and the current pens are project
//! scope and live elsewhere (the document; the project's `session.json`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Tool groups in palette order with their tools in order (02 §5.1).
pub const GROUPS: &[(&str, &[&str])] = &[
    ("select", &["select", "contourEditor", "perspective", "repositionAll"]),
    ("brush", &["brush", "pencil", "rectangle", "ellipse", "polyline", "line"]),
    ("paint", &["paint", "unpaint", "paintUnpainted", "dropper", "closeGap", "stroke", "editTexture"]),
    ("eraser", &["eraser", "cutter", "scissor"]),
    ("navigate", &["zoom", "grabber"]),
    ("text", &["text"]),
];

/// The group of a tool.
#[must_use]
pub fn group_of(tool: &str) -> Option<&'static str> {
    GROUPS.iter().find(|(_, tools)| tools.contains(&tool)).map(|(g, _)| *g)
}

/// Whether `tool` is a tool id.
#[must_use]
pub fn is_tool(tool: &str) -> bool {
    group_of(tool).is_some()
}

/// The options a tool has, with their defaults (02 §7.7; the option commands of 02 §13.2). Tools without options have
/// none (02 §1.3: "a tool with no options says so").
#[must_use]
pub fn default_options(tool: &str) -> Map<String, Value> {
    let v = match tool {
        "select" => json!({ "rectRule": "inside" }),
        "pencil" | "rectangle" | "ellipse" | "line" => json!({ "thicknessSource": "penMax", "thickness": null }),
        "polyline" => json!({ "snap": true, "snapRadius": 8, "thicknessSource": "penMax", "thickness": null }),
        "text" => json!({ "font": null, "size": 36, "bold": false, "italic": false, "baseline": "normal", "kerning": 0,
                          "autoKern": true, "align": "left", "leftMargin": 0, "rightMargin": 0, "indent": 0,
                          "lineSpacing": 0, "outlineColourId": null }),
        _ => json!({}),
    };
    v.as_object().cloned().unwrap_or_default()
}

/// Checks one option value of a tool; `Err` names the reason (`tool.notApplicable` for an unknown option,
/// `tool.valueOutOfRange` for a value outside its range).
///
/// # Errors
/// `(code, message)`.
pub fn check_option(tool: &str, key: &str, v: &Value) -> Result<(), (&'static str, String)> {
    let defaults = default_options(tool);
    if !defaults.contains_key(key) {
        return Err(("tool.notApplicable", format!("the {tool} tool has no option `{key}`")));
    }
    let num = |lo: f64, hi: f64| v.as_f64().is_some_and(|x| x.is_finite() && (lo..=hi).contains(&x));
    let ok = match (tool, key) {
        ("select", "rectRule") => matches!(v.as_str(), Some("inside" | "intersect")),
        (_, "thicknessSource") => matches!(v.as_str(), Some("penMax" | "fixed")),
        (_, "thickness") => v.is_null() || num(0.0, 100.0),
        ("polyline", "snap") | ("text", "bold" | "italic" | "autoKern") => v.is_boolean(),
        ("polyline", "snapRadius") => num(1.0, 50.0),
        ("text", "font") => v.is_null() || v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256),
        ("text", "size") => num(1.0, 512.0),
        ("text", "baseline") => matches!(v.as_str(), Some("normal" | "subscript" | "superscript")),
        ("text", "kerning") => v.as_i64().is_some_and(|k| (-60..=60).contains(&k)),
        ("text", "align") => matches!(v.as_str(), Some("left" | "centre" | "right" | "justify")),
        // SI-131: the spec gives no range for margins, indent and line spacing; ±10,000 du is accepted.
        ("text", "leftMargin" | "rightMargin" | "indent" | "lineSpacing") => num(-10_000.0, 10_000.0),
        ("text", "outlineColourId") => v.is_null() || v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256),
        _ => true,
    };
    if ok { Ok(()) } else { Err(("tool.valueOutOfRange", format!("`{key}` of the {tool} tool is out of range"))) }
}

/// Symmetry (02 §6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SymmetrySetting {
    /// `none`, `mirror` or `kaleidoscope`.
    pub mode: String,
    /// Angle, 0-360°.
    pub angle: f64,
    /// Mirrors, 2-16.
    pub mirrors: i64,
}

impl Default for SymmetrySetting {
    fn default() -> Self {
        Self { mode: "none".into(), angle: 0.0, mirrors: 6 }
    }
}

/// Tablet input settings (02 §6.3).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct TabletSetting {
    /// The tablet acts as a mouse.
    pub ignore_tablet: bool,
    /// Portrait-mode correction.
    pub portrait_correction: bool,
    /// Compatibility handling.
    pub compatibility_mode: bool,
    /// The input interface (`null`: the platform default).
    pub input_interface: Option<String>,
}

/// The grid (02 §10.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Grid {
    /// Shown.
    pub visible: bool,
    /// `normal`, `field12` or `field16`.
    #[serde(rename = "type")]
    pub ty: String,
    /// Drawn over the drawings.
    pub overlay: bool,
}

impl Default for Grid {
    fn default() -> Self {
        Self { visible: false, ty: "normal".into(), overlay: false }
    }
}

/// Onion skin (02 §10.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct OnionSkin {
    /// Master switch.
    pub enabled: bool,
    /// Previous drawings, 0-3.
    pub previous: i64,
    /// Next drawings, 0-3.
    pub next: i64,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self { enabled: false, previous: 1, next: 0 }
    }
}

/// Onion-skin appearance (02 §10.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct OnionSkinStyle {
    /// Outlines only.
    pub outline: bool,
    /// Semi-transparent.
    pub transparency: bool,
    /// Tinted.
    pub colour_shading: bool,
    /// Tint of previous drawings.
    pub previous_colour: String,
    /// Tint of next drawings.
    pub next_colour: String,
}

impl Default for OnionSkinStyle {
    fn default() -> Self {
        Self {
            outline: false,
            transparency: true,
            colour_shading: true,
            previous_colour: "#FF0000FF".into(),
            next_colour: "#00C000FF".into(),
        }
    }
}

/// Light-table shading (02 §10.3-10.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Shade {
    /// The auto light table is washed out.
    pub auto: bool,
    /// Static light-table entries are washed out.
    #[serde(rename = "static")]
    pub stat: bool,
}

impl Default for Shade {
    fn default() -> Self {
        Self { auto: true, stat: true }
    }
}

/// Camera-view selection display (02 §10.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SelectionDisplay {
    /// Outline on selection.
    pub outline_on_selection: bool,
    /// Box highlighting.
    pub box_highlighting: bool,
    /// Selection always visible (SI-117: 02 §10.7's default off is kept).
    pub keep_selection_visible: bool,
}

impl Default for SelectionDisplay {
    fn default() -> Self {
        Self { outline_on_selection: false, box_highlighting: true, keep_selection_visible: false }
    }
}

/// Drawing View background (02 §10.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Background {
    /// Colour.
    pub colour: String,
    /// Use the scene's background colour instead.
    pub use_scene_colour: bool,
}

impl Default for Background {
    fn default() -> Self {
        Self { colour: "#F0F0F0FF".into(), use_scene_colour: false }
    }
}

/// The drawing aids (02 §10).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Aids {
    /// Grid.
    pub grid: Grid,
    /// Onion skin.
    pub onion_skin: OnionSkin,
    /// Onion-skin appearance.
    pub onion_skin_style: OnionSkinStyle,
    /// Auto light table on.
    pub auto_light_table: bool,
    /// Light-table shading.
    pub light_table_shade: Shade,
    /// The static light-table panel shown.
    pub static_light_table_panel: bool,
    /// Show Strokes.
    pub strokes: bool,
    /// Show current drawing on top (Camera View).
    pub current_on_top: bool,
    /// Selection display.
    pub selection_display: SelectionDisplay,
    /// Background.
    pub background: Background,
}

/// Everything in this module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DrawingSettings {
    /// The active tool.
    pub active_tool: String,
    /// The tool each group shows (its last-used tool).
    pub last_in_group: BTreeMap<String, String>,
    /// Options per tool (only tools with options; missing keys take their defaults).
    pub options: BTreeMap<String, Map<String, Value>>,
    /// Draw Behind.
    pub place_behind: bool,
    /// Draw on Top Layer.
    pub draw_top_layer: bool,
    /// Snap to Contour.
    pub snap_to_contour: bool,
    /// Symmetry.
    pub symmetry: SymmetrySetting,
    /// Tablet input.
    pub tablet: TabletSetting,
    /// Drawing aids.
    pub aids: Aids,
}

impl Default for DrawingSettings {
    fn default() -> Self {
        Self {
            active_tool: "select".into(),
            last_in_group: BTreeMap::new(),
            options: BTreeMap::new(),
            place_behind: false,
            draw_top_layer: false,
            snap_to_contour: false,
            symmetry: SymmetrySetting::default(),
            tablet: TabletSetting::default(),
            aids: Aids::default(),
        }
    }
}

impl DrawingSettings {
    /// A tool's options: its defaults, overlaid with the stored values that still pass their checks (a stored value that
    /// fails is stale and falls back, as preferences do).
    #[must_use]
    pub fn options_of(&self, tool: &str) -> Map<String, Value> {
        let mut out = default_options(tool);
        if let Some(stored) = self.options.get(tool) {
            for (k, v) in stored {
                if check_option(tool, k, v).is_ok() {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
        out
    }

    /// The tool a group shows (its last-used tool, else its first).
    #[must_use]
    pub fn shown_in_group(&self, group: &str) -> &str {
        let first = GROUPS.iter().find(|(g, _)| *g == group).and_then(|(_, t)| t.first().copied()).unwrap_or("select");
        self.last_in_group.get(group).map_or(first, String::as_str)
    }

    /// Restores stored settings, falling back member by member: a top-level member of the wrong form takes its
    /// default, an unknown tool is dropped, and an out-of-range option falls back when read ([`Self::options_of`]).
    #[must_use]
    pub fn from_stored(v: &Value) -> Self {
        let mut s = Self::default();
        let mut m = serde_json::to_value(&s).ok().and_then(|d| d.as_object().cloned()).unwrap_or_default();
        if let Some(stored) = v.as_object() {
            for (k, x) in stored {
                if m.contains_key(k) {
                    let mut trial = m.clone();
                    trial.insert(k.clone(), x.clone());
                    if serde_json::from_value::<Self>(Value::Object(trial)).is_ok() {
                        m.insert(k.clone(), x.clone());
                    }
                }
            }
        }
        if let Ok(parsed) = serde_json::from_value::<Self>(Value::Object(m)) {
            s = parsed;
        }
        if !is_tool(&s.active_tool) {
            s.active_tool = "select".into();
        }
        s.last_in_group.retain(|g, t| group_of(t) == Some(g.as_str()));
        s.options.retain(|t, _| is_tool(t));
        s
    }

    /// The stored form.
    #[must_use]
    pub fn to_stored(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// The whole-file shape of the `drawing` settings file: an object whose members, when present, are objects or
/// scalars of the right kind (a file that is not this shape is corrupt, ADR-0021 decision 13).
///
/// # Errors
/// What is wrong.
pub fn shape(d: &Value) -> Result<(), String> {
    let Some(m) = d.as_object() else { return Err("not an object".into()) };
    for k in ["lastInGroup", "options", "symmetry", "tablet", "aids"] {
        if m.get(k).is_some_and(|v| !v.is_object()) {
            return Err(format!("`{k}` is not an object"));
        }
    }
    if m.get("activeTool").is_some_and(|v| !v.is_string()) {
        return Err("`activeTool` is not a string".into());
    }
    Ok(())
}
