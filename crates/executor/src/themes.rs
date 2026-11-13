//! Themes (01 §5.8; M4 C4.1): the built-in `light` and `dark` themes, the `system` choice, and user theme files in
//! `<settings>/Themes/*.json`.
//!
//! A user theme file is `{format: "toontown-theme", formatVersion: "1.0", themeId, name, dark, colours}`, where
//! `colours` maps tokens of [`TOKENS`] to `#RRGGBBAA`. Invalid files are ignored with a logged warning (they are listed
//! with `valid: false` and the reason, never offered for selection). The front end applies a theme as CSS custom
//! properties; the built-in themes' values live there.

use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

/// The colour tokens a theme defines (the front end's CSS custom properties `--tt-<token>`).
pub const TOKENS: &[&str] = &[
    "background",
    "surface",
    "surfaceAlt",
    "border",
    "text",
    "textMuted",
    "accent",
    "accentText",
    "selection",
    "focus",
    "danger",
    "warning",
    "success",
    "tooltipBackground",
    "tooltipText",
    "tabActive",
    "tabInactive",
];

/// The largest theme file read (64 KiB).
const MAX_BYTES: u64 = 64 * 1024;
/// The most theme files read from the folder.
const MAX_FILES: usize = 64;

/// A theme as `ui.theme.list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInfo {
    /// Theme id (the value of `interface.theme`).
    pub theme_id: String,
    /// Display name.
    pub name: String,
    /// Shipped with the application.
    pub built_in: bool,
    /// A dark theme.
    pub dark: bool,
    /// The file parsed and validated (user themes).
    #[serde(skip)]
    pub valid: bool,
    /// Why the file was ignored.
    #[serde(skip)]
    pub problem: Option<String>,
    /// The colour tokens of a user theme (`null` for built-in themes, whose colours are the front end's).
    #[serde(skip)]
    pub colours: Option<Map<String, Value>>,
}

/// The built-in themes (01 §5.8: light, dark, follow system).
#[must_use]
pub fn built_in() -> Vec<ThemeInfo> {
    let t = |id: &str, name: &str, dark: bool| ThemeInfo {
        theme_id: id.to_owned(),
        name: name.to_owned(),
        built_in: true,
        dark,
        valid: true,
        problem: None,
        colours: None,
    };
    vec![t("system", "Follow system", false), t("light", "Light", false), t("dark", "Dark", true)]
}

fn colour(s: &str) -> bool {
    let hex = s.strip_prefix('#').unwrap_or("");
    s.starts_with('#') && hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !built_in().iter().any(|b| b.theme_id == id)
}

/// Validates the text of one theme file.
fn read_one(text: &str, file_stem: &str) -> ThemeInfo {
    let bad = |why: String| ThemeInfo {
        theme_id: file_stem.to_owned(),
        name: file_stem.to_owned(),
        built_in: false,
        dark: false,
        valid: false,
        problem: Some(why),
        colours: None,
    };
    let Ok(v) = tt_model::json_text::parse::<Value>(text) else { return bad("not JSON".into()) };
    if v.get("format").and_then(Value::as_str) != Some("toontown-theme") {
        return bad("format is not toontown-theme".into());
    }
    if v.get("formatVersion").and_then(Value::as_str) != Some("1.0") {
        return bad("unsupported formatVersion".into());
    }
    let Some(id) = v.get("themeId").and_then(Value::as_str).filter(|i| id_ok(i)) else {
        return bad("themeId must be 1-64 letters, digits, - or _ and not a built-in id".into());
    };
    let Some(name) = v.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty() && n.len() <= 100) else {
        return bad("name must be 1-100 characters".into());
    };
    let Some(dark) = v.get("dark").and_then(Value::as_bool) else { return bad("dark must be a boolean".into()) };
    let Some(colours) = v.get("colours").and_then(Value::as_object) else {
        return bad("colours must be an object".into());
    };
    for (k, c) in colours {
        if !TOKENS.contains(&k.as_str()) {
            return bad(format!("unknown colour token `{k}`"));
        }
        if !c.as_str().is_some_and(colour) {
            return bad(format!("colour `{k}` must be #RRGGBBAA"));
        }
    }
    if let Some(extra) = v.as_object().and_then(|m| {
        m.keys().find(|k| !["format", "formatVersion", "themeId", "name", "dark", "colours"].contains(&k.as_str()))
    }) {
        return bad(format!("unknown member `{extra}`"));
    }
    ThemeInfo {
        theme_id: id.to_owned(),
        name: name.to_owned(),
        built_in: false,
        dark,
        valid: true,
        problem: None,
        colours: Some(colours.clone()),
    }
}

/// Reads the user themes of a folder (missing folder: none). Invalid files are returned with `valid: false` and are
/// logged once to stderr by the caller's host; duplicates of an id keep the first file in name order.
#[must_use]
pub fn scan(dir: &Path) -> Vec<ThemeInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<_> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")) && p.is_file())
        .collect();
    files.sort();
    files.truncate(MAX_FILES);
    let mut out: Vec<ThemeInfo> = Vec::new();
    for p in files {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let info = match std::fs::metadata(&p) {
            Ok(m) if m.len() <= MAX_BYTES => match std::fs::read(&p).ok().and_then(|b| String::from_utf8(b).ok()) {
                Some(text) => read_one(&text, &stem),
                None => read_one("", &stem),
            },
            _ => ThemeInfo {
                theme_id: stem.clone(),
                name: stem,
                built_in: false,
                dark: false,
                valid: false,
                problem: Some("larger than 64 KiB".into()),
                colours: None,
            },
        };
        if info.valid && out.iter().any(|o| o.valid && o.theme_id == info.theme_id) {
            continue;
        }
        out.push(info);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_files_are_validated() {
        let ok = r##"{"format":"toontown-theme","formatVersion":"1.0","themeId":"night","name":"Night","dark":true,"colours":{"background":"#101010FF"}}"##;
        let t = read_one(ok, "night");
        assert!(t.valid, "{:?}", t.problem);
        assert!(t.dark);
        assert!(!read_one(&ok.replace("background", "nope"), "x").valid);
        assert!(!read_one(&ok.replace("#101010FF", "red"), "x").valid);
        assert!(!read_one(&ok.replace("night", "dark"), "x").valid, "a built-in id");
        assert!(!read_one("{", "x").valid);
    }
}
