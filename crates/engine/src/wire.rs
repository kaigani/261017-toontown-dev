//! IPC wire types and their published schemas (ADR-0012).
//!
//! Every value the hosts send to the front end crosses as **JSON text** written by [`to_wire_json`] (canonical
//! number formatting, integers exact at any magnitude), and every request arrives as JSON text read by
//! [`from_wire_json`]. The front end parses the text with its own lossless parser, so 64-bit integers survive in
//! both directions, and validates it against the schemas generated here ([`wire_definitions`]), published as
//! `desktop/src/ipc/generated/wire.ts` (kept in sync by `crates/engine/tests/frontend_schemas.rs`).

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{EngineInfo, ErrorInfo, PingRequest, PingResponse};

/// `namespace.object[.verb]`: the command-ID pattern (matches `is_command_id` in the desktop host).
pub const COMMAND_ID_PATTERN: &str = "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$";

/// Who handles a forwarded menu command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MenuHandler {
    /// The front end handles it.
    Frontend,
    /// Not implemented yet (the milestone that delivers it is named).
    Pending,
}

/// Payload sent to the front end when a menu item it handles (or a pending one) is chosen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuCommandEvent {
    /// Command id.
    #[schemars(regex(pattern = "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$"))]
    pub command: String,
    /// Fixed parameters from the manifest.
    pub params: Map<String, Value>,
    /// `frontend` or `pending`.
    pub handler: MenuHandler,
    /// Milestone of a pending command.
    pub milestone: Option<String>,
}

/// The wire types' schema definitions (post-processed like the model's: integer bounds, presence).
#[must_use]
pub fn wire_definitions() -> Map<String, Value> {
    tt_model::schema::struct_definitions(|g| {
        let _ = g.subschema_for::<EngineInfo>();
        let _ = g.subschema_for::<ErrorInfo>();
        let _ = g.subschema_for::<PingRequest>();
        let _ = g.subschema_for::<PingResponse>();
        let _ = g.subschema_for::<MenuCommandEvent>();
    })
}

/// Serialises a wire value as compact JSON text with canonical number formatting (exact 64-bit integers).
///
/// # Errors
/// `internal` (as [`ErrorInfo`]) if the value cannot be serialised (non-finite numbers).
pub fn to_wire_json<T: Serialize>(value: &T) -> Result<String, ErrorInfo> {
    let v = serde_json::to_value(value).map_err(|e| internal(&e.to_string()))?;
    tt_model::canonical::to_compact_string(&v).map_err(|e| internal(e.message()))
}

/// Parses a request's JSON text. Unknown fields are rejected where the type denies them.
///
/// # Errors
/// `arg.invalid` with the parser's message.
pub fn from_wire_json<T: DeserializeOwned>(text: &str) -> Result<T, ErrorInfo> {
    serde_json::from_str(text).map_err(|e| ErrorInfo {
        code: "arg.invalid".to_owned(),
        message: format!("invalid request: {e}"),
        details: serde_json::json!({ "line": e.line(), "column": e.column() }),
    })
}

/// The wire text of an error (what a host rejects a call with).
#[must_use]
pub fn error_json(e: &ErrorInfo) -> String {
    to_wire_json(e).unwrap_or_else(|_| "{\"code\":\"internal\",\"message\":\"unserialisable error\",\"details\":{}}".to_owned())
}

fn internal(message: &str) -> ErrorInfo {
    ErrorInfo { code: "internal".to_owned(), message: format!("serialisation failed: {message}"), details: serde_json::json!({}) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u64_values_are_exact_on_the_wire() {
        let r = PingResponse { seq: u64::MAX, payload: None, engine_uptime_ns: 9_007_199_254_740_993, handler_ns: 0 };
        let text = to_wire_json(&r).unwrap();
        assert_eq!(text, "{\"seq\":18446744073709551615,\"payload\":null,\"engineUptimeNs\":9007199254740993,\"handlerNs\":0}");
        let back: PingResponse = from_wire_json(&text).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn requests_reject_unknown_fields_and_bad_json() {
        assert!(from_wire_json::<PingRequest>("{\"seq\":1,\"extra\":2}").is_err());
        assert_eq!(from_wire_json::<PingRequest>("{\"seq\":1").unwrap_err().code, "arg.invalid");
        assert_eq!(from_wire_json::<PingRequest>("{\"seq\":18446744073709551615}").unwrap().seq, u64::MAX);
    }

    #[test]
    fn definitions_cover_every_wire_type() {
        let defs = wire_definitions();
        for name in ["EngineInfo", "ErrorInfo", "PingRequest", "PingResponse", "MenuCommandEvent", "RunMode", "MenuHandler"] {
            assert!(defs.contains_key(name), "{name}");
        }
        assert_eq!(defs["PingResponse"]["properties"]["seq"]["maximum"], serde_json::json!(u64::MAX));
    }
}
