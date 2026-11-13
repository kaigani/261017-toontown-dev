//! The preference catalogue (01 §7.2-§7.3) and per-key validation (M4 C4.1).
//!
//! Every preference is declared once, in `prefs.json`: key, type, default, range or allowed values, when a change
//! applies (`live`, `nextLaunch`, `restart`), the Preferences window category and a description. `prefs.*` read and
//! write values by key through this table; the settings file stores only values that differ from the defaults
//! (01 §11). A stored value that no longer validates (a hand-edited file, a newer version's key) is ignored and the
//! default applies, so a bad entry never breaks a read.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Value, json};
use tt_registry::CommandError;

/// Error codes of preferences (01 §12.1 rule 7).
pub(crate) mod codes {
    /// A key that is not in the catalogue.
    pub const UNKNOWN_KEY: &str = "prefs.unknownKey";
    /// A value of the wrong type or not among the allowed values.
    pub const INVALID_VALUE: &str = "prefs.invalidValue";
    /// A number outside its range.
    pub const OUT_OF_RANGE: &str = "prefs.outOfRange";
}

/// The type of a preference (01 §7.2, plus `object` for the remembered option sets of 01 §7.3 *Edit*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PrefType {
    /// `true` / `false`.
    Bool,
    /// An integer.
    Int,
    /// A number.
    Number,
    /// A string.
    String,
    /// One of `values`.
    Enum,
    /// `#RRGGBBAA`.
    Colour,
    /// An absolute file-system path (empty: the platform default).
    Path,
    /// An object whose members are the default's members.
    Object,
}

impl PrefType {
    /// The name `prefs.list` reports.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PrefType::Bool => "bool",
            PrefType::Int => "int",
            PrefType::Number => "number",
            PrefType::String => "string",
            PrefType::Enum => "enum",
            PrefType::Colour => "colour",
            PrefType::Path => "path",
            PrefType::Object => "object",
        }
    }
}

/// When a change takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Applies {
    /// At once.
    Live,
    /// At the next launch.
    NextLaunch,
    /// After a restart.
    Restart,
}

impl Applies {
    /// The name `prefs.list` reports.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Applies::Live => "live",
            Applies::NextLaunch => "nextLaunch",
            Applies::Restart => "restart",
        }
    }
}

/// One catalogue entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrefDef {
    pub key: String,
    #[serde(rename = "type")]
    pub ty: PrefType,
    pub default: Value,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// Integers inside this inclusive range are refused although inside `min..max` (`display.bitmapPreviewMaxSize`:
    /// 0 or 256-8192).
    #[serde(default)]
    pub excluded_range: Option<[i64; 2]>,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// The enum also accepts installed user theme ids (`interface.theme`, 01 §5.8).
    #[serde(default)]
    pub user_themes: bool,
    /// An empty string means the theme's colour (`interface.background.timeline`).
    #[serde(default)]
    pub theme_default: bool,
    #[serde(default)]
    pub max_length: Option<usize>,
    /// The type of every member of an `object` preference (documentation; validation follows the default's members).
    #[serde(default, rename = "objectOf")]
    pub _object_of: Option<String>,
    pub applies: Applies,
    pub category: String,
    pub description: String,
}

/// A Preferences window category.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Category {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueFile {
    #[serde(rename = "$comment")]
    _comment: String,
    version: u32,
    categories: Vec<Category>,
    prefs: Vec<PrefDef>,
}

/// The catalogue.
#[derive(Debug, Default)]
pub(crate) struct Catalogue {
    pub categories: Vec<Category>,
    pub prefs: Vec<PrefDef>,
    index: BTreeMap<String, usize>,
    /// Why the bundled catalogue did not load (a build defect; `c41_prefs` proves it loads).
    pub error: Option<String>,
}

impl Catalogue {
    /// The entry of a key.
    pub(crate) fn get(&self, key: &str) -> Option<&PrefDef> {
        self.index.get(key).and_then(|i| self.prefs.get(*i))
    }
}

fn parse(text: &str) -> Catalogue {
    match serde_json::from_str::<CatalogueFile>(text) {
        Ok(f) if f.version == 1 => {
            let index = f.prefs.iter().enumerate().map(|(i, p)| (p.key.clone(), i)).collect();
            Catalogue { categories: f.categories, prefs: f.prefs, index, error: None }
        }
        Ok(f) => {
            Catalogue { error: Some(format!("unsupported catalogue version {}", f.version)), ..Catalogue::default() }
        }
        Err(e) => Catalogue { error: Some(e.to_string()), ..Catalogue::default() },
    }
}

/// The bundled catalogue.
pub(crate) fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| parse(include_str!("prefs.json")))
}

/// The default of a key.
pub(crate) fn default_of(key: &str) -> Option<Value> {
    catalogue().get(key).map(|d| d.default.clone())
}

fn invalid(key: &str, expected: &str) -> CommandError {
    CommandError::with_details(
        codes::INVALID_VALUE,
        format!("`{key}` needs {expected}"),
        json!({ "key": key, "expected": expected }),
    )
}

fn out_of_range(key: &str, def: &PrefDef, value: &Value) -> CommandError {
    let mut details = json!({ "key": key, "value": value });
    if let (Some(m), Some(min)) = (details.as_object_mut(), def.min) {
        m.insert("min".into(), json!(min));
    }
    if let (Some(m), Some(max)) = (details.as_object_mut(), def.max) {
        m.insert("max".into(), json!(max));
    }
    let range = match (def.min, def.max) {
        (Some(a), Some(b)) => format!(" ({a} to {b})"),
        _ => String::new(),
    };
    CommandError::with_details(codes::OUT_OF_RANGE, format!("`{key}` is out of range{range}"), details)
}

/// The key is not in the catalogue.
pub(crate) fn unknown(key: &str) -> CommandError {
    CommandError::with_details(codes::UNKNOWN_KEY, format!("unknown preference `{key}`"), json!({ "key": key }))
        .hint("prefs.list returns every key")
}

fn is_colour(s: &str) -> bool {
    let hex = s.strip_prefix('#').unwrap_or("");
    s.starts_with('#') && (hex.len() == 8 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validates a value for a key and returns it in its stored form (colours upper-case with alpha; integral numbers as
/// integers for `int`). `user_themes` are the installed user theme ids.
///
/// # Errors
/// `prefs.unknownKey`, `prefs.invalidValue`, `prefs.outOfRange`.
pub(crate) fn validate(key: &str, value: &Value, user_themes: &[String]) -> Result<Value, CommandError> {
    let def = catalogue().get(key).ok_or_else(|| unknown(key))?;
    validate_def(def, value, user_themes)
}

fn validate_def(def: &PrefDef, value: &Value, user_themes: &[String]) -> Result<Value, CommandError> {
    let key = def.key.as_str();
    match def.ty {
        PrefType::Bool => value.as_bool().map(Value::Bool).ok_or_else(|| invalid(key, "a boolean")),
        PrefType::Int => {
            let n = value.as_i64().ok_or_else(|| invalid(key, "an integer"))?;
            let f = n as f64;
            if def.min.is_some_and(|m| f < m)
                || def.max.is_some_and(|m| f > m)
                || def.excluded_range.is_some_and(|[a, b]| (a..=b).contains(&n))
            {
                return Err(out_of_range(key, def, value));
            }
            Ok(json!(n))
        }
        PrefType::Number => {
            let f = value.as_f64().filter(|f| f.is_finite()).ok_or_else(|| invalid(key, "a number"))?;
            if def.min.is_some_and(|m| f < m) || def.max.is_some_and(|m| f > m) {
                return Err(out_of_range(key, def, value));
            }
            Ok(value.clone())
        }
        PrefType::String => {
            let s = value.as_str().ok_or_else(|| invalid(key, "a string"))?;
            if def.max_length.is_some_and(|m| s.chars().count() > m) {
                return Err(CommandError::with_details(
                    codes::OUT_OF_RANGE,
                    format!("`{key}` is longer than {} characters", def.max_length.unwrap_or_default()),
                    json!({ "key": key, "maxLength": def.max_length }),
                ));
            }
            Ok(value.clone())
        }
        PrefType::Enum => {
            let s = value.as_str().ok_or_else(|| invalid(key, "one of the listed values"))?;
            let listed = def.values.as_deref().unwrap_or_default();
            if listed.iter().any(|v| v == s) || (def.user_themes && user_themes.iter().any(|t| t == s)) {
                Ok(value.clone())
            } else {
                let mut allowed: Vec<&str> = listed.iter().map(String::as_str).collect();
                if def.user_themes {
                    allowed.extend(user_themes.iter().map(String::as_str));
                }
                Err(CommandError::with_details(
                    codes::INVALID_VALUE,
                    format!("`{key}` must be one of {}", allowed.join(", ")),
                    json!({ "key": key, "values": allowed }),
                ))
            }
        }
        PrefType::Colour => {
            let s = value.as_str().ok_or_else(|| invalid(key, "a colour #RRGGBBAA"))?;
            if s.is_empty() && def.theme_default {
                return Ok(json!(""));
            }
            if !is_colour(s) {
                return Err(invalid(key, "a colour #RRGGBBAA"));
            }
            let mut c = s.to_ascii_uppercase();
            if c.len() == 7 {
                c.push_str("FF");
            }
            Ok(json!(c))
        }
        PrefType::Path => {
            let s = value.as_str().ok_or_else(|| invalid(key, "an absolute folder path"))?;
            if s.is_empty() {
                return Ok(json!(""));
            }
            let p = std::path::Path::new(s);
            if !p.is_absolute() {
                return Err(invalid(key, "an absolute folder path"));
            }
            match std::fs::metadata(p) {
                Ok(m) if m.is_dir() && !m.permissions().readonly() => Ok(value.clone()),
                _ => Err(CommandError::with_details(
                    codes::INVALID_VALUE,
                    format!("`{key}` must be an existing writable folder"),
                    json!({ "key": key, "expected": "an existing writable folder", "path": s }),
                )),
            }
        }
        PrefType::Object => {
            let obj = value.as_object().ok_or_else(|| invalid(key, "an object"))?;
            let default = def.default.as_object().cloned().unwrap_or_default();
            for (k, v) in obj {
                let Some(d) = default.get(k) else {
                    return Err(CommandError::with_details(
                        codes::INVALID_VALUE,
                        format!("`{key}` has no member `{k}`"),
                        json!({ "key": key, "member": k, "members": default.keys().collect::<Vec<_>>() }),
                    ));
                };
                let same_type = match d {
                    Value::Bool(_) => v.is_boolean(),
                    Value::Number(n) if n.is_i64() || n.is_u64() => v.is_i64() || v.is_u64(),
                    Value::Number(_) => v.is_number(),
                    Value::String(_) => v.is_string(),
                    _ => false,
                };
                if !same_type {
                    return Err(invalid(key, &format!("member `{k}` of the default's type")));
                }
            }
            // Members not given keep the default (a full object is stored, so reads need no merge).
            let mut merged = default;
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
            Ok(Value::Object(merged))
        }
    }
}

/// The effective value of a key from a stored value: the stored one if it still validates, else the default.
pub(crate) fn effective(key: &str, stored: Option<&Value>, user_themes: &[String]) -> Option<Value> {
    let def = catalogue().get(key)?;
    match stored {
        Some(v) => Some(validate_def(def, v, user_themes).unwrap_or_else(|_| def.default.clone())),
        None => Some(def.default.clone()),
    }
}

/// Keys an agent may not set or reset: its own access (01 §7.3 *Agent access*, 11 §15.1 `app` scope).
pub(crate) fn agent_restricted(key: &str) -> bool {
    key.starts_with("agent.")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test")]
    use super::*;

    #[test]
    fn the_bundled_catalogue_loads_and_every_default_validates() {
        let c = catalogue();
        assert!(c.error.is_none(), "{:?}", c.error);
        for d in &c.prefs {
            assert!(c.categories.iter().any(|x| x.id == d.category), "{}", d.key);
            assert_eq!(validate(&d.key, &d.default, &[]).unwrap(), d.default, "{}", d.key);
        }
    }

    #[test]
    fn values_are_typed_and_ranged() {
        assert_eq!(validate("files.backups.count", &json!(21), &[]).unwrap_err().code, codes::OUT_OF_RANGE);
        assert_eq!(validate("files.backups.count", &json!(2.5), &[]).unwrap_err().code, codes::INVALID_VALUE);
        assert_eq!(validate("display.bitmapPreviewMaxSize", &json!(100), &[]).unwrap_err().code, codes::OUT_OF_RANGE);
        assert_eq!(validate("display.bitmapPreviewMaxSize", &json!(256), &[]).unwrap(), json!(256));
        assert_eq!(validate("interface.elementColours.peg", &json!("#ab12cd"), &[]).unwrap(), json!("#AB12CDFF"));
        assert_eq!(validate("interface.theme", &json!("mine"), &["mine".into()]).unwrap(), json!("mine"));
        assert_eq!(validate("interface.theme", &json!("mine"), &[]).unwrap_err().code, codes::INVALID_VALUE);
        assert_eq!(validate("nope.nope", &json!(1), &[]).unwrap_err().code, codes::UNKNOWN_KEY);
        let merged = validate("edit.pasteSpecial.motionPoint", &json!({"x": false}), &[]).unwrap();
        assert_eq!(merged["x"], json!(false));
        assert_eq!(merged["y"], json!(true));
    }
}
