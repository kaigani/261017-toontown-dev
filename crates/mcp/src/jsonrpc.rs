//! JSON-RPC 2.0 framing of the MCP stdio transport: one message per line (UTF-8 JSON without embedded newlines),
//! requests, notifications and responses (MCP basic protocol, revision 2025-06-18).

use serde_json::{Value, json};

/// Parse error.
pub const PARSE_ERROR: i64 = -32700;
/// Not a valid request object.
pub const INVALID_REQUEST: i64 = -32600;
/// Unknown method.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Invalid parameters.
pub const INVALID_PARAMS: i64 = -32602;
/// Internal error.
pub const INTERNAL_ERROR: i64 = -32603;
/// MCP: resource not found.
pub const RESOURCE_NOT_FOUND: i64 = -32002;

/// One incoming message.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// A request (has an id and a method).
    Request {
        /// Request id (string or number, kept as JSON).
        id: Value,
        /// Method.
        method: String,
        /// Parameters (`{}` when absent).
        params: Value,
    },
    /// A notification (a method without an id).
    Notification {
        /// Method.
        method: String,
        /// Parameters.
        params: Value,
    },
    /// A response to a request the server sent.
    Response {
        /// The id of the server's request.
        id: Value,
        /// `Ok(result)` or `Err(error object)`.
        outcome: Result<Value, Value>,
    },
}

/// Parses one line. `Err` holds the error response to send (with a null id when the id is unknown).
///
/// # Errors
/// The JSON-RPC error response for unparseable or malformed messages.
pub fn parse(line: &str) -> Result<Incoming, Value> {
    let v: Value =
        serde_json::from_str(line).map_err(|e| error(&Value::Null, PARSE_ERROR, &format!("parse error: {e}"), None))?;
    let Some(m) = v.as_object() else {
        // JSON-RPC batches were removed from MCP in revision 2025-06-18.
        return Err(error(
            &Value::Null,
            INVALID_REQUEST,
            "a message is one JSON-RPC object (batches are not supported)",
            None,
        ));
    };
    if m.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let id = m.get("id").cloned().unwrap_or(Value::Null);
        return Err(error(&id, INVALID_REQUEST, "`jsonrpc` must be \"2.0\"", None));
    }
    let id = m.get("id").cloned();
    if let Some(id) = &id
        && !(id.is_string() || id.is_i64() || id.is_u64())
    {
        return Err(error(&Value::Null, INVALID_REQUEST, "`id` must be a string or an integer", None));
    }
    match (m.get("method"), id) {
        (Some(Value::String(method)), Some(id)) => Ok(Incoming::Request {
            id,
            method: method.clone(),
            params: m.get("params").cloned().unwrap_or_else(|| json!({})),
        }),
        (Some(Value::String(method)), None) => Ok(Incoming::Notification {
            method: method.clone(),
            params: m.get("params").cloned().unwrap_or_else(|| json!({})),
        }),
        (None, Some(id)) => match (m.get("result"), m.get("error")) {
            (Some(r), None) => Ok(Incoming::Response { id, outcome: Ok(r.clone()) }),
            (None, Some(e)) => Ok(Incoming::Response { id, outcome: Err(e.clone()) }),
            _ => Err(error(&id, INVALID_REQUEST, "a response has exactly one of `result` and `error`", None)),
        },
        (Some(_), _) => Err(error(&Value::Null, INVALID_REQUEST, "`method` must be a string", None)),
        (None, None) => Err(error(&Value::Null, INVALID_REQUEST, "not a request, notification or response", None)),
    }
}

/// A success response.
#[must_use]
pub fn result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An error response.
#[must_use]
pub fn error(id: &Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut e = json!({ "code": code, "message": message });
    if let (Some(d), Some(m)) = (data, e.as_object_mut()) {
        m.insert("data".into(), d);
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": e })
}

/// A notification.
#[must_use]
pub fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// A request from the server.
#[must_use]
pub fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test")]
    use super::*;

    #[test]
    fn messages_are_classified_and_malformed_ones_answered() {
        assert!(matches!(parse(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#), Ok(Incoming::Request { .. })));
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Ok(Incoming::Notification { .. })
        ));
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","id":"a","result":{}}"#),
            Ok(Incoming::Response { outcome: Ok(_), .. })
        ));
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","id":"a","error":{"code":1,"message":"x"}}"#),
            Ok(Incoming::Response { outcome: Err(_), .. })
        ));
        for (line, code) in [
            ("{", PARSE_ERROR),
            ("[]", INVALID_REQUEST),
            (r#"{"id":1,"method":"ping"}"#, INVALID_REQUEST),
            (r#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#, INVALID_REQUEST),
            (r#"{"jsonrpc":"2.0","id":1,"method":7}"#, INVALID_REQUEST),
            (r#"{"jsonrpc":"2.0","id":1}"#, INVALID_REQUEST),
        ] {
            let e = parse(line).unwrap_err();
            assert_eq!(e["error"]["code"], code, "{line}");
        }
    }
}
