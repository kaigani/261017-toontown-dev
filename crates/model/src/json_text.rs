//! Strict JSON text reading shared by every text boundary of the engine (IPC wire, later the package reader and the
//! MCP transport; ADR-0012, C11-R1-07/R1-17).
//!
//! `serde_json` alone accepts texts the front end's parser rejects, and the reverse: duplicate keys silently keep the
//! last value, integer literals beyond the 64-bit ranges silently become the nearest double, and nesting is limited
//! by a different count. [`parse`] first checks the text with the same rules as the front end's `parseWireJson`
//! (`desktop/src/ipc/wireJson.ts`), then deserialises it. Both sides run the shared corpus
//! `tests/fixtures/wire/hostile-json.json`, so they accept and reject exactly the same texts:
//!
//! - RFC 8259 grammar only (no BOM, no comments, no trailing commas, no `NaN`/`Infinity`, no leading zeros, JSON
//!   white space only); nothing after the value.
//! - Strings: no raw control characters, valid escapes only, no unpaired surrogate (escaped or raw).
//! - Object keys unique after unescaping (`{"a":1,"\u0061":2}` is a duplicate).
//! - Nesting: every value (scalars included) at depth at most [`MAX_TEXT_DEPTH`], the outermost value at depth 1.
//! - Numbers: a fraction/exponent literal must be finite as a double. An **integer literal** denotes an exact
//!   integer: it is accepted when it fits `i64` or `u64`, or when a double holds exactly that integer (e.g. 2^64,
//!   1e20 written in full); any other integer literal would be rounded and is rejected (`arg.invalid`), never
//!   changed (10 §1.1). The canonical writer writes such doubles back as the same integer digits
//!   ([`crate::canonical::format_f64`]), so every accepted text round-trips.

use serde::de::DeserializeOwned;

use crate::error::{ModelError, codes};

/// Maximum depth of a value in JSON text (the outermost value is at depth 1; scalars count). Equal to the front
/// end's `MAX_DEPTH` (`desktop/src/ipc/schema.ts`).
pub const MAX_TEXT_DEPTH: usize = 128;

/// Checks `text` against the shared rules, then deserialises it.
///
/// # Errors
/// `arg.invalid` with `details.offset` (byte offset) for any text the rules reject, or the deserialiser's message
/// with `details.line`/`details.column` when the JSON does not have the shape of `T`.
pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ModelError> {
    check(text)?;
    let mut de = serde_json::Deserializer::from_str(text);
    // Nesting is bounded by `check` (identically to the front end); serde_json's own limit counts differently.
    de.disable_recursion_limit();
    let value = T::deserialize(&mut de).and_then(|v| de.end().map(|()| v)).map_err(|e| {
        ModelError::with_details(
            codes::ARG_INVALID,
            format!("invalid JSON: {e}"),
            serde_json::json!({ "line": e.line(), "column": e.column() }),
        )
    })?;
    Ok(value)
}

/// Checks `text` against the shared rules without building a value.
///
/// # Errors
/// `arg.invalid` with `details.offset` naming the first violation.
pub fn check(text: &str) -> Result<(), ModelError> {
    let mut s = Scanner { b: text.as_bytes(), text, pos: 0 };
    s.value(1)?;
    s.ws();
    if s.pos != s.b.len() {
        return Err(s.fail("trailing characters"));
    }
    Ok(())
}

struct Scanner<'a> {
    b: &'a [u8],
    text: &'a str,
    pos: usize,
}

impl Scanner<'_> {
    fn fail(&self, message: &str) -> ModelError {
        ModelError::with_details(
            codes::ARG_INVALID,
            format!("invalid JSON text: {message} at offset {}", self.pos),
            serde_json::json!({ "offset": self.pos }),
        )
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<(), ModelError> {
        if depth > MAX_TEXT_DEPTH {
            return Err(self.fail(&format!("nested deeper than {MAX_TEXT_DEPTH}")));
        }
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(drop),
            Some(b't') => self.literal("true"),
            Some(b'f') => self.literal("false"),
            Some(b'n') => self.literal("null"),
            _ => self.number(),
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), ModelError> {
        if self.b.get(self.pos..self.pos + word.len()) == Some(word.as_bytes()) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(self.fail("invalid literal"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), ModelError> {
        self.pos += 1;
        self.ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(());
        }
        let mut keys = std::collections::HashSet::new();
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.fail("expected a key"));
            }
            let at = self.pos;
            let key = self.string()?;
            if !keys.insert(key) {
                self.pos = at;
                return Err(self.fail("duplicate key"));
            }
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.fail("expected `:`"));
            }
            self.pos += 1;
            self.value(depth + 1)?;
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => return Err(self.fail("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), ModelError> {
        self.pos += 1;
        self.ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.value(depth + 1)?;
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => return Err(self.fail("expected `,` or `]`")),
            }
        }
    }

    /// Reads a string token and returns it decoded (serde_json decodes exactly the token; it rejects control
    /// characters, invalid escapes and unpaired surrogate escapes).
    fn string(&mut self) -> Result<String, ModelError> {
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.peek() {
                None => return Err(self.fail("unterminated string")),
                Some(b'"') => break,
                Some(b'\\') => self.pos += 2,
                Some(_) => self.pos += 1,
            }
        }
        self.pos += 1;
        let token = self.text.get(start..self.pos).ok_or_else(|| self.fail("invalid string"))?;
        serde_json::from_str::<String>(token).map_err(|_| {
            self.pos = start;
            self.fail("invalid string")
        })
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<(), ModelError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.fail("invalid number")),
        }
        let mut integer = true;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            integer = false;
            if self.digits() == 0 {
                return Err(self.fail("invalid number"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            integer = false;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.fail("invalid number"));
            }
        }
        let token = self.text.get(start..self.pos).unwrap_or_default();
        if integer {
            if token.parse::<i64>().is_ok() || token.parse::<u64>().is_ok() || exact_double_integer(token) {
                return Ok(());
            }
            self.pos = start;
            return Err(self.fail("integer literal is outside the 64-bit ranges and not exactly a double"));
        }
        match token.parse::<f64>() {
            Ok(x) if x.is_finite() => Ok(()),
            _ => {
                self.pos = start;
                Err(self.fail("number out of range"))
            }
        }
    }
}

/// Whether an integer literal denotes exactly the value of the nearest double.
fn exact_double_integer(token: &str) -> bool {
    match token.parse::<f64>() {
        Ok(x) if x.is_finite() => format!("{x:.0}") == token,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn rules() {
        for ok in
            ["0", "-0", "[]", "{}", "18446744073709551615", "-9223372036854775808", "18446744073709551616", "1e-400"]
        {
            assert!(check(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            " ",
            "{\"a\":1,\"a\":2}",
            "{\"a\":1,\"\\u0061\":2}",
            "\"\\ud800\"",
            "-18446744073709551615",
            "18446744073709551617",
            "1e400",
            "01",
            "1.",
            ".5",
            "+1",
            "NaN",
            "[1,]",
            "{} x",
            "\u{feff}{}",
        ] {
            assert!(check(bad).is_err(), "{bad}");
        }
        assert!(check(&format!("{}{}", "[".repeat(MAX_TEXT_DEPTH), "]".repeat(MAX_TEXT_DEPTH))).is_ok());
        assert!(check(&format!("{}{}", "[".repeat(MAX_TEXT_DEPTH + 1), "]".repeat(MAX_TEXT_DEPTH + 1))).is_err());
        let v: Value = parse(&format!("{}{}", "[".repeat(MAX_TEXT_DEPTH), "]".repeat(MAX_TEXT_DEPTH))).unwrap();
        assert!(v.is_array());
    }
}
