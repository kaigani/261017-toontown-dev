//! `ping`: an engine round trip with no side effects, used to measure IPC latency and in the self-test.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::{Engine, EngineError};

/// Largest payload `ping` echoes (1 MiB). Bulk transfers use binary IPC (C0.3), not `ping`.
pub const MAX_PING_PAYLOAD_BYTES: usize = 1 << 20;

/// Parameters of a ping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PingRequest {
    /// Caller-chosen sequence number, echoed back.
    pub seq: u64,
    /// Optional payload, echoed back (lets the caller measure size-dependent cost).
    #[serde(default)]
    pub payload: Option<String>,
}

/// Result of a ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    /// The request's sequence number.
    pub seq: u64,
    /// The request's payload.
    pub payload: Option<String>,
    /// Engine uptime when the ping was handled, in nanoseconds.
    pub engine_uptime_ns: u64,
    /// Time spent inside the engine handling the ping, in nanoseconds.
    pub handler_ns: u64,
}

pub(crate) fn ping(engine: &Engine, request: PingRequest) -> Result<PingResponse, EngineError> {
    let start = Instant::now();
    let len = request.payload.as_ref().map_or(0, String::len);
    if len > MAX_PING_PAYLOAD_BYTES {
        return Err(EngineError::LimitExceeded {
            what: "ping payload",
            limit: MAX_PING_PAYLOAD_BYTES as u64,
            value: len as u64,
        });
    }
    let engine_uptime_ns = engine.uptime_ns();
    let handler_ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
    Ok(PingResponse { seq: request.seq, payload: request.payload, engine_uptime_ns, handler_ns })
}

#[cfg(test)]
mod tests {
    use crate::{Engine, EngineConfig, EngineError, RunMode};

    use super::*;

    fn engine() -> Engine {
        Engine::new(EngineConfig { run_mode: RunMode::Attached }).unwrap()
    }

    #[test]
    fn echoes_seq_and_payload() {
        let r = engine().ping(PingRequest { seq: 42, payload: Some("abc".into()) }).unwrap();
        assert_eq!(r.seq, 42);
        assert_eq!(r.payload.as_deref(), Some("abc"));
    }

    #[test]
    fn payload_limit_is_inclusive() {
        let ok = "x".repeat(MAX_PING_PAYLOAD_BYTES);
        assert!(engine().ping(PingRequest { seq: 1, payload: Some(ok) }).is_ok());
        let too_big = "x".repeat(MAX_PING_PAYLOAD_BYTES + 1);
        let err = engine().ping(PingRequest { seq: 1, payload: Some(too_big) }).unwrap_err();
        assert!(matches!(err, EngineError::LimitExceeded { .. }));
        assert_eq!(err.code(), "limit.exceeded");
    }

    #[test]
    fn request_rejects_unknown_fields() {
        let bad = serde_json::from_str::<PingRequest>(r#"{"seq":1,"extra":true}"#);
        assert!(bad.is_err());
        let good: PingRequest = serde_json::from_str(r#"{"seq":7}"#).unwrap();
        assert_eq!(good, PingRequest { seq: 7, payload: None });
    }
}
