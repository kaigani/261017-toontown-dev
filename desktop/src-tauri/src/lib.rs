//! # tt-desktop — Toontown desktop host (Tauri 2)
//!
//! Hosts the engine in-process (attached run mode, 11 §3), builds the native menu bar from the UI-bindings
//! manifest, and exposes a small set of typed commands to the web front end (React + dockview in `desktop/src`).
//!
//! The host holds no document state of its own: from M1 every command it receives is routed to the engine's
//! serialized executor (11 P2). Shell features still to come are in spec 01 (M4).

pub mod bindings;
pub mod diagnostics;
pub mod ipc;
pub mod menu;
pub mod spikes;

use std::sync::Arc;

use tt_engine::{Engine, EngineConfig, EngineError, RunMode};

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
    /// Tauri failed.
    #[error("window system error: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Starts the desktop application and blocks until it exits.
///
/// # Errors
/// [`DesktopError`] when the engine, the menu manifest or the window system fails.
pub fn run() -> Result<(), DesktopError> {
    let launch = diagnostics::parse_args(std::env::args().skip(1))?;
    let bench = launch.bench;
    let engine = Engine::new(EngineConfig { run_mode: RunMode::Attached })?;
    let manifest = Arc::new(bindings::Manifest::embedded()?);
    let menu_manifest = Arc::clone(&manifest);
    tauri::Builder::default()
        .manage(engine)
        .manage(diagnostics::Diagnostics { bench: bench.clone() })
        .manage(spikes::tablet::TabletSpike::new(launch.spike.clone()))
        .manage(spikes::canvas::CanvasSpike::new(launch.spike.clone()))
        .menu(move |app| menu::build(app, &menu_manifest))
        .on_menu_event(move |app, event| menu::on_event(app, &manifest, event.id().as_ref()))
        .setup(move |app| {
            if let Some(settings) = &bench {
                diagnostics::arm_timeout(app.handle(), settings);
            }
            if let Some(settings) = &launch.spike {
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
        .invoke_handler(tauri::generate_handler![
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
            spikes::canvas::spike_canvas_pixels
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tauri_config_matches_workspace_and_plan() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"), "tauri.conf.json version drifted from Cargo");
        assert_eq!(conf["identifier"], "dev.toontown.app");
        assert_eq!(conf["mainBinaryName"], "toontown");
        assert_eq!(conf["productName"], "Toontown");
        assert_eq!(conf["app"]["windows"][0]["label"], "main");
    }
}
