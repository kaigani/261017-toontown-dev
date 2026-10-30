//! Names (10 §1.3): normalisation and the per-entity rules.

use unicode_normalization::UnicodeNormalization;

use crate::error::{ModelError, codes};
use crate::types::ObjectType;

/// Characters forbidden in file-system-like names (10 §1.3).
pub const FS_FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// The naming rule of one entity kind (10 §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameRule {
    /// Characters forbidden in addition to control characters.
    pub forbidden: &'static [char],
    /// Whether `.` is forbidden anywhere (elements, drawings).
    pub no_dot: bool,
    /// Whether a trailing `.` is forbidden (projects).
    pub no_trailing_dot: bool,
    /// Maximum length in Unicode scalar values.
    pub max_len: usize,
}

const GENERIC: NameRule = NameRule { forbidden: &[], no_dot: false, no_trailing_dot: false, max_len: 255 };

impl NameRule {
    /// The rule for the `name` of an object type (10 §1.3). Types without a row in the table (assets, drawing nodes,
    /// channels, libraries) use the generic rule: non-empty, no control characters, at most 255 characters.
    #[must_use]
    pub const fn for_type(ty: ObjectType) -> NameRule {
        match ty {
            ObjectType::Project => {
                NameRule { forbidden: FS_FORBIDDEN, no_dot: false, no_trailing_dot: true, max_len: 100 }
            }
            ObjectType::Scene | ObjectType::Palette | ObjectType::PaletteStyle => {
                NameRule { forbidden: FS_FORBIDDEN, no_dot: false, no_trailing_dot: false, max_len: 128 }
            }
            ObjectType::Element | ObjectType::Drawing => {
                NameRule { forbidden: FS_FORBIDDEN, no_dot: true, no_trailing_dot: false, max_len: 64 }
            }
            ObjectType::Swatch => NameRule { forbidden: &[], no_dot: false, no_trailing_dot: false, max_len: 128 },
            ObjectType::Pen => NameRule { forbidden: FS_FORBIDDEN, no_dot: false, no_trailing_dot: false, max_len: 64 },
            ObjectType::LibraryFolder | ObjectType::LibraryItem | ObjectType::Library => {
                NameRule { forbidden: FS_FORBIDDEN, no_dot: false, no_trailing_dot: false, max_len: 128 }
            }
            _ => GENERIC,
        }
    }

    /// The JSON Schema `pattern` that expresses the character rules (length is expressed with `maxLength`).
    #[must_use]
    pub fn pattern(&self) -> String {
        let mut class = String::from("\\u0000-\\u001F\\u007F");
        for c in self.forbidden {
            match c {
                '\\' => class.push_str("\\\\"),
                '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => class.push(*c),
                _ => class.push(*c),
            }
        }
        if self.no_dot {
            class.push('.');
        }
        // Non-empty, no leading/trailing white space (names are trimmed), none of the forbidden characters.
        let body = format!("[^{class}]");
        let tail = if self.no_trailing_dot { format!("[^{class}\\s.]") } else { format!("[^{class}\\s]") };
        format!("^(?:[^{class}\\s]|[^{class}\\s]{body}*{tail})$")
    }
}

/// Normalises a name as every input path does: NFC, then trim leading and trailing white space (10 §1.1, §1.3).
#[must_use]
pub fn normalize(input: &str) -> String {
    input.nfc().collect::<String>().trim().to_owned()
}

/// Checks an already normalised name against a rule.
///
/// # Errors
/// `arg.invalid` naming the violated rule (`details.rule`: `empty`, `control`, `forbidden`, `trailingDot`, `tooLong`).
pub fn check(rule: &NameRule, name: &str) -> Result<(), ModelError> {
    let fail = |r: &str, msg: String| {
        Err(ModelError::with_details(
            codes::ARG_INVALID,
            msg,
            serde_json::json!({ "rule": r, "maxLength": rule.max_len }),
        ))
    };
    if name.is_empty() {
        return fail("empty", "name must not be empty".to_owned());
    }
    if name.chars().any(|c| c.is_control() && (c <= '\u{1F}' || c == '\u{7F}')) {
        return fail("control", "name must not contain control characters".to_owned());
    }
    if let Some(c) = name.chars().find(|c| rule.forbidden.contains(c) || (rule.no_dot && *c == '.')) {
        return fail("forbidden", format!("name must not contain `{c}`"));
    }
    if rule.no_trailing_dot && name.ends_with('.') {
        return fail("trailingDot", "name must not end with `.`".to_owned());
    }
    if name.chars().count() > rule.max_len {
        return fail("tooLong", format!("name is longer than {} characters", rule.max_len));
    }
    if name != name.trim() {
        return fail("whitespace", "name must not start or end with white space".to_owned());
    }
    Ok(())
}

/// Normalises then checks a name for an object type.
///
/// # Errors
/// As [`check`].
pub fn normalize_and_check(ty: ObjectType, input: &str) -> Result<String, ModelError> {
    let n = normalize(input);
    check(&NameRule::for_type(ty), &n)?;
    Ok(n)
}

/// The smallest positive integer `n` such that `format(n)` is not in `taken` (10 §1.3 default patterns).
#[must_use]
pub fn next_free<F: Fn(u32) -> String>(format: F, taken: &dyn Fn(&str) -> bool) -> String {
    let mut n = 1u32;
    loop {
        let candidate = format(n);
        if !taken(&candidate) || n == u32::MAX {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_nfc_and_trims() {
        assert_eq!(normalize("  Bob \t"), "Bob");
        assert_eq!(normalize("e\u{301}"), "\u{e9}");
    }

    #[test]
    fn per_entity_rules() {
        let el = NameRule::for_type(ObjectType::Element);
        assert!(check(&el, "Bob").is_ok());
        assert!(check(&el, "Bob.1").is_err());
        assert!(check(&el, &"x".repeat(65)).is_err());
        assert!(check(&el, &"x".repeat(64)).is_ok());
        let pr = NameRule::for_type(ObjectType::Project);
        assert!(check(&pr, "Short film").is_ok());
        assert!(check(&pr, "Short.film").is_ok());
        assert!(check(&pr, "Short.").is_err());
        assert!(check(&pr, "a/b").is_err());
        let sw = NameRule::for_type(ObjectType::Swatch);
        assert!(check(&sw, "a/b").is_ok());
        assert!(check(&sw, "a\u{7}b").is_err());
        assert!(check(&sw, "").is_err());
    }

    #[test]
    fn next_free_uses_smallest_unused() {
        let taken = ["Scene-1", "Scene-3"];
        assert_eq!(next_free(|n| format!("Scene-{n}"), &|s| taken.contains(&s)), "Scene-2");
    }
}
