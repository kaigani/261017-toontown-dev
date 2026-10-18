//! # tt-engine — engine facade
//!
//! The one entry point the shells (desktop host, headless binary, MCP server) use to reach the engine. In later
//! milestones it owns the application state, settings, virtual views (11 §3), the jobs runner, the events bus and
//! the command implementations of areas 01–09, all running behind the serialized executor (`tt-executor`).
//!
//! **Status: M0.** The facade exists so the shells are wired the way M1 needs them: they construct an [`Engine`]
//! with a [`RunMode`], read [`EngineInfo`], and call engine functions (today only [`Engine::ping`]); they never
//! reach into the document crates directly. [`Engine::selftest`] exercises the facade without any display or GPU.
//!
//! Boundary rule (plan §3.2, 11 P2): the facade will expose document changes **only** as command invocations routed
//! to the executor. No method on `Engine` may hand out mutable access to document state.
//!
//! Spec sections it will implement: 00 §4 (reference architecture); 01 (app state, settings); 11 §2–§3 (run modes,
//! virtual views); area command implementations of 01–09. Milestone: M1 onwards.

mod components;
mod determinism;
mod error;
mod ping;
mod selftest;
pub mod wire;

use std::time::Instant;

use schemars::JsonSchema;
use serde::Serialize;

pub use components::{Component, components};
pub use error::{EngineError, ErrorInfo};
pub use ping::{MAX_PING_PAYLOAD_BYTES, PingRequest, PingResponse};
pub use selftest::{CheckResult, SelftestReport};

/// Crate name as linked into the shells.
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Spec sections this crate is responsible for.
pub const SPEC_SECTIONS: &[&str] = &["00 §4", "01 (app state, settings)", "11 §2–§3", "areas 01–09 (commands)"];

/// Milestone(s) in which this crate receives its implementation (plan §4).
pub const MILESTONE: &str = "M1 onwards";

/// Application display name (working name, plan §6.8).
pub const APP_NAME: &str = "Toontown";

/// Application version (the workspace version).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// How the engine was started (11 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RunMode {
    /// GUI running; agents attach to the live session.
    Attached,
    /// No window; the full engine runs offscreen with virtual views.
    Headless,
    /// Headless replay of a batch file, then exit.
    BatchScript,
}

/// Configuration used to construct an [`Engine`].
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Run mode of the hosting process.
    pub run_mode: RunMode,
}

/// Internal bootstrap information about the running engine: the engine-level facts the MCP `initialize` server info
/// needs (11 §4: name, version, registry version, document schema version, run mode) plus platform and architecture.
///
/// This is **not** the result of the spec query `app.info.get` (01 §12), which also reports `locale` and
/// `languages[]`; those depend on settings and localisation, which do not exist yet. `app.info.get` is a registered
/// query from C1.4 (ticket in `plan/traceability.csv`) and will be built on this type, not replace it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineInfo {
    /// Application name.
    pub name: &'static str,
    /// Application version.
    pub version: &'static str,
    /// Operating system the engine was compiled for (`windows`, `macos`, `linux`).
    pub platform: &'static str,
    /// CPU architecture (`x86_64`, `aarch64`).
    pub arch: &'static str,
    /// Project format / document schema version (10 §6.6).
    #[schemars(regex(pattern = "^[0-9]+[.][0-9]+$"))]
    pub format_version: &'static str,
    /// Command-registry version (11 §4).
    #[schemars(regex(pattern = "^[0-9]+[.][0-9]+[.][0-9]+$"))]
    pub registry_version: &'static str,
    /// Run mode.
    pub run_mode: RunMode,
}

/// The engine facade. Cheap to share: the desktop host keeps one in Tauri state, the headless binary owns one.
#[derive(Debug)]
pub struct Engine {
    config: EngineConfig,
    started: Instant,
}

impl Engine {
    /// Constructs the engine. Never touches a display, GPU or window system.
    ///
    /// # Errors
    /// Infallible in M0; the `Result` is part of the contract because M1 loads settings and the registry here.
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        Ok(Self { config, started: Instant::now() })
    }

    /// Static information about this engine instance.
    #[must_use]
    pub fn info(&self) -> EngineInfo {
        EngineInfo {
            name: APP_NAME,
            version: APP_VERSION,
            platform: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            format_version: tt_model::FORMAT_VERSION,
            registry_version: tt_registry::REGISTRY_VERSION,
            run_mode: self.config.run_mode,
        }
    }

    /// Time since the engine was constructed, in nanoseconds (saturating).
    #[must_use]
    pub fn uptime_ns(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Round-trip probe used by the IPC latency readout and by the self-test. Echoes `seq` and the payload.
    ///
    /// # Errors
    /// [`EngineError::LimitExceeded`] when the payload is larger than [`MAX_PING_PAYLOAD_BYTES`].
    pub fn ping(&self, request: PingRequest) -> Result<PingResponse, EngineError> {
        ping::ping(self, request)
    }

    /// Runs the engine self-test (used by `toontown-headless --selftest`). Needs no display or GPU.
    #[must_use]
    pub fn selftest(&self) -> SelftestReport {
        selftest::run(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless() -> Engine {
        Engine::new(EngineConfig { run_mode: RunMode::Headless }).unwrap()
    }

    #[test]
    fn info_reports_versions_and_mode() {
        let info = headless().info();
        assert_eq!(info.name, "Toontown");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.format_version, tt_model::FORMAT_VERSION);
        assert_eq!(info.registry_version, tt_registry::REGISTRY_VERSION);
        assert_eq!(info.run_mode, RunMode::Headless);
    }

    #[test]
    fn info_serializes_camel_case() {
        let json = serde_json::to_value(headless().info()).unwrap();
        assert_eq!(json["runMode"], "headless");
        assert!(json.get("formatVersion").is_some());
        assert!(json.get("registryVersion").is_some());
    }

    #[test]
    fn engine_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine>();
    }
}
