//! Typed engine errors and their wire form (11 §10: stable dotted codes `area.reason`).

use serde::Serialize;

/// Errors returned by the engine facade.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// A request exceeded a fixed engine limit.
    #[error("{what} is {value}, above the limit of {limit}")]
    LimitExceeded {
        /// What was measured.
        what: &'static str,
        /// The limit.
        limit: u64,
        /// The offending value.
        value: u64,
    },
}

impl EngineError {
    /// The stable dotted error code (11 §10).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::LimitExceeded { .. } => "limit.exceeded",
        }
    }

    /// The wire form used by every client (IPC, MCP, batch logs).
    #[must_use]
    pub fn to_info(&self) -> ErrorInfo {
        let details = match self {
            Self::LimitExceeded { limit, value, .. } => serde_json::json!({ "limit": limit, "value": value }),
        };
        ErrorInfo { code: self.code().to_owned(), message: self.to_string(), details }
    }
}

/// Serializable error envelope: `{code, message, details}` (11 §10).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ErrorInfo {
    /// Stable dotted code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Structured details (code-specific).
    pub details: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_exceeded_wire_form() {
        let err = EngineError::LimitExceeded { what: "ping payload", limit: 10, value: 11 };
        let info = err.to_info();
        assert_eq!(info.code, "limit.exceeded");
        assert_eq!(info.details["limit"], 10);
        assert_eq!(info.details["value"], 11);
        assert!(info.message.contains("ping payload"));
    }
}
