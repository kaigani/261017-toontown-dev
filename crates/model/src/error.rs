//! Typed model errors in the `{code, message, details}` wire form (11 §10).

use serde::Serialize;
use serde_json::{Value, json};

/// Stable dotted error and issue codes produced by this crate (11 §10, 10 §9).
pub mod codes {
    /// An object or patch does not satisfy its schema (10 §9).
    pub const SCHEMA_VIOLATION: &str = "doc.schemaViolation";
    /// A parameter value is invalid (11 §10).
    pub const ARG_INVALID: &str = "arg.invalid";
    /// A reference does not resolve (11 §10).
    pub const REF_NOT_FOUND: &str = "ref.notFound";
    /// A reference matches more than one object (11 §10).
    pub const REF_AMBIGUOUS: &str = "ref.ambiguous";
    /// A reference resolves to an object of the wrong type (11 §10).
    pub const REF_WRONG_TYPE: &str = "ref.wrongType";
    /// A reference stored in the document does not resolve (10 §7.2).
    pub const REF_DANGLING: &str = "ref.dangling";
    /// Two objects share an id (10 §7.2).
    pub const ID_DUPLICATE: &str = "id.duplicate";
    /// The scene tree is invalid (10 §7.2).
    pub const TREE_INVALID: &str = "tree.invalid";
    /// An exposure is not canonical or names foreign drawings (10 §7.2).
    pub const EXPOSURE_INVALID: &str = "exposure.invalid";
    /// A global limit of 10 §5 would be exceeded.
    pub const LIMIT_EXCEEDED: &str = "doc.limitExceeded";
    /// A JSON-Patch `test` operation did not match (10 §9).
    pub const PATCH_TEST_FAILED: &str = "doc.patchTestFailed";
    /// Unexpected failure (11 §10).
    pub const INTERNAL: &str = "internal";
}

/// An error of the object model, carrying a stable dotted code, a message and structured details.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize)]
#[error("{code}: {message}")]
pub struct ModelError {
    code: &'static str,
    message: String,
    details: Value,
}

impl ModelError {
    /// Creates an error with empty details.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: json!({}) }
    }

    /// Creates an error with structured details.
    #[must_use]
    pub fn with_details(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self { code, message: message.into(), details }
    }

    /// `doc.schemaViolation` at a JSON pointer.
    #[must_use]
    pub fn schema(pointer: &str, message: impl Into<String>) -> Self {
        Self::with_details(codes::SCHEMA_VIOLATION, message, json!({ "pointer": pointer }))
    }

    /// `arg.invalid` at a JSON pointer.
    #[must_use]
    pub fn invalid(pointer: &str, message: impl Into<String>) -> Self {
        Self::with_details(codes::ARG_INVALID, message, json!({ "pointer": pointer }))
    }

    /// The stable dotted code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The structured details.
    #[must_use]
    pub fn details(&self) -> &Value {
        &self.details
    }

    /// The wire form `{code, message, details}`.
    #[must_use]
    pub fn to_wire(&self) -> Value {
        json!({ "code": self.code, "message": self.message, "details": self.details })
    }

    /// Prefixes the JSON pointer in `details.pointer` (used when an error inside a nested value is reported against
    /// its container).
    #[must_use]
    pub fn at(mut self, prefix: &str) -> Self {
        if let Some(Value::String(p)) = self.details.get_mut("pointer") {
            *p = format!("{prefix}{p}");
        } else if let Value::Object(map) = &mut self.details {
            map.insert("pointer".to_owned(), Value::String(prefix.to_owned()));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_form_has_three_fields() {
        let e = ModelError::schema("/name", "bad").at("/scenes/0");
        assert_eq!(e.code(), "doc.schemaViolation");
        assert_eq!(
            e.to_wire(),
            json!({"code": "doc.schemaViolation", "message": "bad",
            "details": {"pointer": "/scenes/0/name"}})
        );
    }
}
