//! Canonical JSON (10 §6.5, JSON-01…JSON-05; ADR-0011).
//!
//! - **JSON-01** UTF-8 without BOM, `\n` line endings, two-space indentation, final newline.
//! - **JSON-02** Object keys are written in the order of the value given. Model objects are serialised from typed
//!   structs whose field order is their schema order (envelope first, then the type's properties, then `meta`,
//!   `ext`), and `serde_json` is built with `preserve_order`; unknown keys preserved from newer minor versions are
//!   appended in lexicographic order by [`crate::overlay`].
//! - **JSON-03** Numbers use the ECMAScript `Number::toString` form (the shortest digits that round-trip to the same
//!   double, `-0` as `0`, integers without a fraction, exponent form outside 1e-7 ≤ |x| < 1e21), which is also what
//!   `JSON.stringify` writes, so the front end and the engine agree byte for byte. Integers held as `i64`/`u64` are
//!   written exactly. NaN and infinities are rejected.
//! - **Tuples.** An array of 1–8 scalars (numbers, strings, booleans, null) is written on one line as `[a, b, c]`:
//!   stroke points, exposure runs, pressure samples, matrices and other small tuples (JSON-03). Empty arrays and
//!   objects are written `[]` and `{}`.
//! - Strings escape only `"`, `\` and control characters (`\b \f \n \r \t`, otherwise `\u00XX` lower-case hex);
//!   everything else, including non-ASCII, is written as UTF-8.

use serde::Serialize;
use serde_json::{Number, Value};

use crate::error::{ModelError, codes};

/// Maximum number of scalar elements an array may have to be written on one line.
pub const TUPLE_MAX: usize = 8;

/// Formats a finite double exactly as ECMAScript `Number.prototype.toString()` does.
///
/// # Errors
/// `arg.invalid` for NaN or an infinity (10 §1.1: "NaN and infinities are invalid").
pub fn format_f64(x: f64) -> Result<String, ModelError> {
    if !x.is_finite() {
        return Err(ModelError::new(codes::ARG_INVALID, "NaN and infinities are not valid JSON numbers"));
    }
    if x == 0.0 {
        return Ok("0".to_owned());
    }
    let mut buf = ryu::Buffer::new();
    let s = buf.format_finite(x.abs());
    // Decompose ryu's shortest representation into digits and a decimal exponent: value = 0.d1d2…dk × 10^n.
    let (mantissa, exp10) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (s, 0),
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let frac_part = frac_part.trim_end_matches('0');
    let mut digits: String = format!("{int_part}{frac_part}");
    let mut n: i32 = i32::try_from(int_part.len()).unwrap_or(0) + exp10;
    // Strip leading zeros (e.g. "0.001" -> digits "1", n = -2).
    let leading = digits.len() - digits.trim_start_matches('0').len();
    digits.drain(..leading);
    n -= i32::try_from(leading).unwrap_or(0);
    let digits = digits.trim_end_matches('0').to_owned();
    let k = i32::try_from(digits.len()).unwrap_or(1);
    let sign = if x < 0.0 { "-" } else { "" };
    let out = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat(usize::try_from(n - k).unwrap_or(0)))
    } else if 0 < n && n <= 21 {
        let (a, b) = digits.split_at(usize::try_from(n).unwrap_or(0));
        format!("{a}.{b}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(usize::try_from(-n).unwrap_or(0)))
    } else {
        let e = n - 1;
        let esign = if e >= 0 { "+" } else { "-" };
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() {
            format!("{first}e{esign}{}", e.abs())
        } else {
            format!("{first}.{rest}e{esign}{}", e.abs())
        }
    };
    Ok(format!("{sign}{out}"))
}

/// Formats a JSON number canonically: integers exactly, doubles per [`format_f64`].
///
/// # Errors
/// `arg.invalid` for a non-finite double.
pub fn format_number(n: &Number) -> Result<String, ModelError> {
    if let Some(i) = n.as_i64() {
        return Ok(i.to_string());
    }
    if let Some(u) = n.as_u64() {
        return Ok(u.to_string());
    }
    format_f64(n.as_f64().unwrap_or(f64::NAN))
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if c <= '\u{1F}' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Array(_) | Value::Object(_))
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_value(out: &mut String, v: &Value, level: usize, depth: usize) -> Result<(), ModelError> {
    if depth > crate::MAX_JSON_DEPTH {
        return Err(ModelError::new(
            codes::LIMIT_EXCEEDED,
            format!("JSON nested deeper than {}", crate::MAX_JSON_DEPTH),
        ));
    }
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&format_number(n)?),
        Value::String(s) => write_string(out, s),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) if items.len() <= TUPLE_MAX && items.iter().all(is_scalar) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, item, level, depth + 1)?;
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                indent(out, level + 1);
                write_value(out, item, level + 1, depth + 1)?;
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            indent(out, level);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            out.push_str("{\n");
            for (i, (k, item)) in map.iter().enumerate() {
                indent(out, level + 1);
                write_string(out, k);
                out.push_str(": ");
                write_value(out, item, level + 1, depth + 1)?;
                out.push_str(if i + 1 < map.len() { ",\n" } else { "\n" });
            }
            indent(out, level);
            out.push('}');
        }
    }
    Ok(())
}

/// Writes a JSON value in canonical form (JSON-01…JSON-03), keeping the key order of `value`.
///
/// # Errors
/// `arg.invalid` for a non-finite number; `doc.limitExceeded` for nesting deeper than [`crate::MAX_JSON_DEPTH`].
pub fn to_canonical_string(value: &Value) -> Result<String, ModelError> {
    let mut out = String::new();
    write_value(&mut out, value, 0, 0)?;
    out.push('\n');
    Ok(out)
}

/// Serialises a typed value and writes it canonically.
///
/// # Errors
/// As [`to_canonical_string`], plus `internal` if serialisation fails.
pub fn to_canonical<T: Serialize + ?Sized>(value: &T) -> Result<String, ModelError> {
    let v = serde_json::to_value(value)
        .map_err(|e| ModelError::new(codes::INTERNAL, format!("serialisation failed: {e}")))?;
    to_canonical_string(&v)
}

/// Writes a JSON value compactly (no white space) with canonical number formatting and the given key order. Used
/// for wire payloads and hashing where indentation is not wanted.
///
/// # Errors
/// As [`to_canonical_string`].
pub fn to_compact_string(value: &Value) -> Result<String, ModelError> {
    fn go(out: &mut String, v: &Value, depth: usize) -> Result<(), ModelError> {
        if depth > crate::MAX_JSON_DEPTH {
            return Err(ModelError::new(
                codes::LIMIT_EXCEEDED,
                format!("JSON nested deeper than {}", crate::MAX_JSON_DEPTH),
            ));
        }
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&format_number(n)?),
            Value::String(s) => write_string(out, s),
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    go(out, item, depth + 1)?;
                }
                out.push(']');
            }
            Value::Object(map) => {
                out.push('{');
                for (i, (k, item)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    go(out, item, depth + 1)?;
                }
                out.push('}');
            }
        }
        Ok(())
    }
    let mut out = String::new();
    go(&mut out, value, 0)?;
    Ok(out)
}

/// Normalises numbers in a JSON value received from outside: every finite double with an integral value that fits
/// an `i64` becomes an integer (JSON has one number type; `1.0` and `1` are the same value, and integer-typed
/// properties accept both, as JSON Schema's `integer` does). `-0.0` becomes `0`.
pub fn normalize_numbers(value: &mut Value) {
    normalize_numbers_at(value, 0);
}

fn normalize_numbers_at(value: &mut Value, depth: usize) {
    if depth > crate::MAX_JSON_DEPTH {
        return; // deeper values are rejected by every later stage (schema, typed parse, writer)
    }
    match value {
        Value::Number(n) => {
            if n.is_f64()
                && let Some(f) = n.as_f64()
                // i64 range check without precision loss: |f| < 2^63.
                && f.fract() == 0.0
                && f.abs() < 9_223_372_036_854_775_808.0
            {
                #[allow(clippy::cast_possible_truncation)]
                let i = f as i64;
                *n = Number::from(i);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| normalize_numbers_at(v, depth + 1)),
        Value::Object(map) => map.values_mut().for_each(|v| normalize_numbers_at(v, depth + 1)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_match_ecmascript() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (23.976, "23.976"),
            (29.97, "29.97"),
            (59.94, "59.94"),
            (41.1121, "41.1121"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123_456_789_012_345_680_000.0, "123456789012345680000"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (0.000_123, "0.000123"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (9_007_199_254_740_993.0, "9007199254740992"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
            (100.0, "100"),
            (12_345.678, "12345.678"),
        ];
        for (x, s) in cases {
            assert_eq!(format_f64(*x).unwrap(), *s, "{x:e}");
            assert_eq!(s.parse::<f64>().unwrap(), if *x == 0.0 { 0.0 } else { *x });
        }
        assert!(format_f64(f64::NAN).is_err());
        assert!(format_f64(f64::INFINITY).is_err());
    }

    #[test]
    fn layout_is_two_space_with_tuples_on_one_line() {
        let v = json!({
            "id": "el_1", "type": "element", "exposure": [["dr_1", 6], [null, 2]],
            "points": [[0, 0, 0, 0, 0.5, 0], [1, 1, 0, 0, 0, 0]], "empty": [], "obj": {},
            "long": [1, 2, 3, 4, 5, 6, 7, 8, 9], "s": "a\"b\\c\n\u{1}é"
        });
        let s = to_canonical_string(&v).unwrap();
        let expected = "{\n  \"id\": \"el_1\",\n  \"type\": \"element\",\n  \"exposure\": [\n    [\"dr_1\", 6],\n    [null, 2]\n  ],\n  \"points\": [\n    [0, 0, 0, 0, 0.5, 0],\n    [1, 1, 0, 0, 0, 0]\n  ],\n  \"empty\": [],\n  \"obj\": {},\n  \"long\": [\n    1,\n    2,\n    3,\n    4,\n    5,\n    6,\n    7,\n    8,\n    9\n  ],\n  \"s\": \"a\\\"b\\\\c\\n\\u0001é\"\n}\n";
        assert_eq!(s, expected);
        // It is valid JSON that parses back to the same value.
        assert_eq!(serde_json::from_str::<Value>(&s).unwrap(), v);
    }

    #[test]
    fn integral_doubles_normalise_to_integers() {
        let mut v = json!({"a": 1.0, "b": -0.0, "c": 1.5, "d": [2.0, 1e300]});
        normalize_numbers(&mut v);
        assert_eq!(to_compact_string(&v).unwrap(), "{\"a\":1,\"b\":0,\"c\":1.5,\"d\":[2,1e+300]}");
        assert!(v["a"].is_i64());
    }

    #[test]
    fn large_integers_are_exact() {
        let v = json!({"u": u64::MAX, "i": i64::MIN});
        assert_eq!(to_compact_string(&v).unwrap(), "{\"u\":18446744073709551615,\"i\":-9223372036854775808}");
    }
}
