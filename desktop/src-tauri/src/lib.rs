//! # tt-desktop — Toontown desktop host (Tauri 2)
//!
//! Hosts the engine in-process (attached run mode, 11 §3), builds the native menu bar from the UI-bindings
//! manifest, and exposes a small set of typed commands to the web front end (React + dockview in `desktop/src`).
//!
//! The host holds no document state of its own: from M1 every command it receives is routed to the engine's
//! serialized executor (11 P2). Shell features still to come are in spec 01 (M4).
//!
//! **M0 diagnostics** (`--ipc-bench`, `--spike …` and their commands) exist only in builds with the Cargo feature
//! `m0-diagnostics` (C0.4). The default build registers the product commands alone; [`COMMANDS`] lists them.

pub mod bindings;
pub mod diagnostics;
pub mod ipc;
pub mod menu;
// Spike code is compiled into a build only with the feature; unit tests always compile it so its tests keep running.
#[cfg(any(test, feature = "m0-diagnostics"))]
pub mod spikes;

use std::sync::Arc;

use tt_engine::{Engine, EngineConfig, EngineError, RunMode};

/// Declares the command registry from one list: [`COMMANDS`] (the paths, for the gating tests) and the Tauri invoke
/// handler are generated from the same tokens, so they cannot disagree.
macro_rules! command_registry {
    ($($($segment:ident)::+),+ $(,)?) => {
        /// Every command this build registers with Tauri, as its Rust path (the command name is the last segment).
        pub const COMMANDS: &[&str] = &[$(stringify!($($segment)::+)),+];

        fn invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$($($segment)::+),+]
        }
    };
}

#[cfg(not(feature = "m0-diagnostics"))]
command_registry![ipc::ping, ipc::app_info];

#[cfg(feature = "m0-diagnostics")]
command_registry![
    ipc::ping,
    ipc::app_info,
    diagnostics::bench_plan,
    diagnostics::bench_report,
    spikes::tablet::spike_mode,
    spikes::tablet::spike_tablet_native_start,
    spikes::tablet::spike_tablet_native_stop,
    spikes::tablet::spike_pen_simulate,
    spikes::tablet::spike_save_log,
    spikes::tablet::spike_environment,
    spikes::tablet::spike_probe_native_window,
    spikes::tablet::spike_raw_input_sink,
    spikes::canvas::spike_canvas_scene,
    spikes::canvas::spike_canvas_pixels,
];

/// Errors that stop the desktop host from starting.
#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    /// The engine failed to start.
    #[error("engine failed to start ({code}): {0}", code = .0.code())]
    Engine(#[from] EngineError),
    /// The UI-bindings manifest is invalid (a build defect; covered by unit tests).
    #[error(transparent)]
    Bindings(#[from] bindings::BindingsError),
    /// The command line is invalid.
    #[error(transparent)]
    Args(#[from] diagnostics::ArgsError),
    /// An M0 diagnostic flag was given to a build without the `m0-diagnostics` feature.
    #[error(
        "`{0}` is an M0 diagnostic and this build has none; build with `--config src-tauri/tauri.diagnostics.conf.json` (docs/dev-setup.md)"
    )]
    DiagnosticsNotBuilt(&'static str),
    /// Tauri failed.
    #[error("window system error: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
/// [`DesktopError`] when the arguments, the engine, the menu manifest or the window system fails.
pub fn run() -> Result<(), DesktopError> {
    let launch = diagnostics::parse_args(std::env::args().skip(1))?;
    #[cfg(not(feature = "m0-diagnostics"))]
    if let Some(flag) = launch.requested_diagnostic() {
        return Err(DesktopError::DiagnosticsNotBuilt(flag));
    }
    let engine = Engine::new(EngineConfig { run_mode: RunMode::Attached })?;
    let manifest = Arc::new(bindings::Manifest::embedded()?);
    let menu_manifest = Arc::clone(&manifest);
    let builder = tauri::Builder::default()
        .manage(engine)
        .menu(move |app| menu::build(app, &menu_manifest))
        .on_menu_event(move |app, event| menu::on_event(app, &manifest, event.id().as_ref()));
    #[cfg(feature = "m0-diagnostics")]
    let builder = with_m0_diagnostics(builder, &launch);
    builder.invoke_handler(invoke_handler()).run(tauri::generate_context!())?;
    Ok(())
}

/// Managed state and start-up hooks of the M0 diagnostics (benchmark and spikes).
#[cfg(feature = "m0-diagnostics")]
fn with_m0_diagnostics(
    builder: tauri::Builder<tauri::Wry>,
    launch: &diagnostics::LaunchOptions,
) -> tauri::Builder<tauri::Wry> {
    let bench = launch.bench.clone();
    let spike = launch.spike.clone();
    builder
        .manage(diagnostics::Diagnostics { bench: bench.clone() })
        .manage(spikes::tablet::TabletSpike::new(spike.clone()))
        .manage(spikes::canvas::CanvasSpike::new(spike.clone()))
        .setup(move |app| {
            if let Some(settings) = &bench {
                diagnostics::arm_timeout(app.handle(), settings);
            }
            if let Some(settings) = &spike {
                spikes::tablet::arm_auto_timeout(app.handle(), settings);
                if settings.name == "canvas" {
                    // Measure on the largest canvas the display allows (the page records the actual size).
                    if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                        let _ = window.maximize();
                    }
                }
            }
            Ok(())
        })
}

#[cfg(test)]
mod tests {
    use super::COMMANDS;

    /// Command names (the last path segment) this build registers.
    fn names() -> Vec<&'static str> {
        COMMANDS.iter().map(|path| path.rsplit("::").next().unwrap_or(path).trim()).collect()
    }

    #[test]
    fn tauri_config_matches_workspace_and_plan() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"), "tauri.conf.json version drifted from Cargo");
        assert_eq!(conf["identifier"], "dev.toontown.app");
        assert_eq!(conf["mainBinaryName"], "toontown");
        assert_eq!(conf["productName"], "Toontown");
        assert_eq!(conf["app"]["windows"][0]["label"], "main");
    }

    /// C0.4: the default build registers the product commands only; no spike or benchmark command exists in it.
    #[cfg(not(feature = "m0-diagnostics"))]
    #[test]
    fn default_build_registers_no_diagnostic_commands() {
        assert_eq!(names(), ["ping", "app_info"]);
        for path in COMMANDS {
            assert!(!path.contains("spike") && !path.contains("bench") && !path.contains("diagnostics"), "{path}");
        }
    }

    /// The diagnostics build registers every benchmark and spike command next to the product ones.
    #[cfg(feature = "m0-diagnostics")]
    #[test]
    fn diagnostics_build_registers_the_m0_commands() {
        let names = names();
        assert_eq!(&names[..2], ["ping", "app_info"]);
        assert!(names.contains(&"bench_plan") && names.contains(&"bench_report"));
        assert_eq!(names.iter().filter(|n| n.starts_with("spike_")).count(), 10);
    }

    /// The diagnostics overlay config enables the feature and the matching front-end build, and nothing else.
    #[test]
    fn diagnostics_overlay_config_enables_both_halves() {
        let overlay: serde_json::Value = serde_json::from_str(include_str!("../tauri.diagnostics.conf.json")).unwrap();
        assert_eq!(overlay["build"]["features"], serde_json::json!(["m0-diagnostics"]));
        assert_eq!(overlay["build"]["beforeBuildCommand"], "pnpm build:diagnostics");
        assert_eq!(overlay.as_object().map(|o| o.len()), Some(2), "only $schema and build");
    }
}
