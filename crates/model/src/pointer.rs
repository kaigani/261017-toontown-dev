//! JSON Pointers (RFC 6901) used by patches and change sets.

use serde_json::Value;

use crate::error::ModelError;

/// Escapes one reference token (`~` → `~0`, `/` → `~1`).
#[must_use]
pub fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Builds a pointer from tokens.
#[must_use]
pub fn join<I: IntoIterator<Item = S>, S: AsRef<str>>(tokens: I) -> String {
    tokens.into_iter().fold(String::new(), |mut acc, t| {
        acc.push('/');
        acc.push_str(&escape(t.as_ref()));
        acc
    })
}

/// Splits a pointer into unescaped tokens. `""` is the whole document.
///
/// # Errors
/// `arg.invalid` when the pointer does not start with `/` or contains an invalid `~` escape.
pub fn parse(pointer: &str) -> Result<Vec<String>, ModelError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    let rest = pointer
        .strip_prefix('/')
        .ok_or_else(|| ModelError::invalid(pointer, format!("JSON pointer `{pointer}` must start with `/`")))?;
    rest.split('/')
        .map(|t| {
            let mut out = String::with_capacity(t.len());
            let mut chars = t.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => out.push('~'),
                        Some('1') => out.push('/'),
                        _ => return Err(ModelError::invalid(pointer, format!("invalid `~` escape in `{pointer}`"))),
                    }
                } else {
                    out.push(c);
                }
            }
            Ok(out)
        })
        .collect()
}

/// Parses an array index token (no leading zeros, no sign; RFC 6901 §4).
#[must_use]
pub fn array_index(token: &str) -> Option<usize> {
    if token == "0" || (!token.is_empty() && !token.starts_with('0') && token.bytes().all(|b| b.is_ascii_digit())) {
        token.parse().ok()
    } else {
        None
    }
}

/// The value at `tokens`, if any.
#[must_use]
pub fn get<'a>(root: &'a Value, tokens: &[String]) -> Option<&'a Value> {
    tokens.iter().try_fold(root, |v, t| match v {
        Value::Object(m) => m.get(t),
        Value::Array(a) => array_index(t).and_then(|i| a.get(i)),
        _ => None,
    })
}

/// The mutable value at `tokens`, if any.
pub fn get_mut<'a>(root: &'a mut Value, tokens: &[String]) -> Option<&'a mut Value> {
    tokens.iter().try_fold(root, |v, t| match v {
        Value::Object(m) => m.get_mut(t),
        Value::Array(a) => array_index(t).and_then(|i| a.get_mut(i)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_escapes() {
        let p = join(["a/b", "c~d", "0"]);
        assert_eq!(p, "/a~1b/c~0d/0");
        assert_eq!(parse(&p).unwrap(), vec!["a/b", "c~d", "0"]);
        assert!(parse("a").is_err());
        assert!(parse("/a~2").is_err());
        assert_eq!(parse("").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn indexes_are_strict() {
        assert_eq!(array_index("0"), Some(0));
        assert_eq!(array_index("12"), Some(12));
        for bad in ["01", "-1", "+1", "", "1a", "-"] {
            assert_eq!(array_index(bad), None, "{bad}");
        }
        let v = json!({"a": [10, {"b": 2}]});
        assert_eq!(get(&v, &parse("/a/1/b").unwrap()), Some(&json!(2)));
        assert_eq!(get(&v, &parse("/a/01").unwrap()), None);
    }
}
