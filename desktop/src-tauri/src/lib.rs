//! # tt-desktop — Toontown desktop host (Tauri 2)
//!
//! Hosts the engine in-process (attached run mode, 11 §3), builds the native menu bar from the UI-bindings
//! manifest, and exposes a small set of typed commands to the web front end (React + dockview in `desktop/src`).
//!
//! The host holds no document state of its own (M4 C4.1, ADR-0021): every GUI action reaches the executor through
//! `run_command` as a registered command, exactly as an agent's call does (11 P2); engine events are forwarded to the
//! webviews; native-window changes are applied from the executor's `shell.*` notices; attached mode serves MCP over
//! Streamable HTTP (ADR-0022).
//!
//! **M0 diagnostics** (`--ipc-bench`, `--spike …` and their commands) exist only in builds with the Cargo feature
//! `m0-diagnostics` (C0.4). The default build registers the product commands alone; [`COMMANDS`] lists them.

pub mod bindings;
pub mod diagnostics;
pub mod host;
pub mod instance;
pub mod ipc;
pub mod menu;
pub mod shell;
pub mod watchdog;
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
command_registry![
    ipc::ping,
    ipc::app_info,
    ipc::run_command,
    ipc::cancel_request,
    ipc::report_shell,
    ipc::dispatch_key,
    ipc::availability,
    ipc::pick_path,
    ipc::startup,
    ipc::restart_engine,
];

#[cfg(feature = "m0-diagnostics")]
command_registry![
    ipc::ping,
    ipc::app_info,
    ipc::run_command,
    ipc::cancel_request,
    ipc::report_shell,
    ipc::dispatch_key,
    ipc::availability,
    ipc::pick_path,
    ipc::startup,
    ipc::restart_engine,
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
    /// The executor did not start.
    #[error("the executor did not start ({}): {}", .0.code, .0.message)]
    Executor(tt_executor::CommandError),
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
    let options = host::HostOptions {
        user_dir: launch.user_dir.clone(),
        mcp_http: launch.mcp_http,
        discovery: launch.mcp_discovery.clone(),
        open: launch.open.clone(),
    };
    let host = host::Host::start(options).map_err(DesktopError::Executor)?;
    // 01 §1.1 rule 1: one instance per user session; a second one forwards its documents and exits.
    let settings = host.settings_dir.clone();
    let listener = match instance::claim(&settings, &launch.open) {
        instance::Start::Forwarded => return Ok(()),
        instance::Start::First(listener, key) => listener.map(|l| (l, key)),
    };
    let menu_manifest = Arc::clone(&manifest);
    #[cfg(feature = "m0-diagnostics")]
    let diagnostics_launch = launch.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(engine)
        .manage(host)
        .manage(menu::MenuSlot::default())
        .menu(move |app| menu::build(app, &menu_manifest))
        .on_menu_event(move |app, event| menu::on_event(app, event.id().as_ref()))
        .on_window_event(shell::on_window_event)
        // A reloaded page reports from scratch: what the old page reported ends with it (ruling O-9).
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                use tauri::Manager;
                if let Some(host) = webview.app_handle().try_state::<host::Host>() {
                    host.forget_window_reports(webview.label());
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            shell::start_event_pump(&handle);
            shell::restore_window(&handle);
            watchdog::start(&handle);
            menu::refresh(&handle);
            shell::sync_agent_server(&handle);
            if let Some((listener, key)) = listener {
                let h = handle.clone();
                instance::serve(listener, key, move |paths| shell::open_documents(&h, &paths));
            }
            #[cfg(feature = "m0-diagnostics")]
            m0_setup(&handle, &diagnostics_launch);
            Ok(())
        });
    #[cfg(feature = "m0-diagnostics")]
    let builder = with_m0_diagnostics(builder, &launch);
    builder.invoke_handler(invoke_handler()).run(tauri::generate_context!())?;
    instance::release(&settings);
    Ok(())
}

/// Managed state of the M0 diagnostics (benchmark and spikes).
#[cfg(feature = "m0-diagnostics")]
fn with_m0_diagnostics(
    builder: tauri::Builder<tauri::Wry>,
    launch: &diagnostics::LaunchOptions,
) -> tauri::Builder<tauri::Wry> {
    builder
        .manage(diagnostics::Diagnostics { bench: launch.bench.clone() })
        .manage(spikes::tablet::TabletSpike::new(launch.spike.clone()))
        .manage(spikes::canvas::CanvasSpike::new(launch.spike.clone()))
}

/// Start-up hooks of the M0 diagnostics, run from the application's setup.
#[cfg(feature = "m0-diagnostics")]
fn m0_setup(app: &tauri::AppHandle, launch: &diagnostics::LaunchOptions) {
    if let Some(settings) = &launch.bench {
        diagnostics::arm_timeout(app, settings);
    }
    if let Some(settings) = &launch.spike {
        spikes::tablet::arm_auto_timeout(app, settings);
        if settings.name == "canvas" {
            // Measure on the largest canvas the display allows (the page records the actual size).
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                let _ = window.maximize();
            }
        }
    }
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
        assert_eq!(
            names(),
            [
                "ping",
                "app_info",
                "run_command",
                "cancel_request",
                "report_shell",
                "dispatch_key",
                "pick_path",
                "startup",
                "restart_engine"
            ]
        );
        for path in COMMANDS {
            assert!(!path.contains("spike") && !path.contains("bench") && !path.contains("diagnostics"), "{path}");
        }
    }

    /// The diagnostics build registers every benchmark and spike command next to the product ones.
    #[cfg(feature = "m0-diagnostics")]
    #[test]
    fn diagnostics_build_registers_the_m0_commands() {
        let names = names();
        assert_eq!(
            &names[..9],
            [
                "ping",
                "app_info",
                "run_command",
                "cancel_request",
                "report_shell",
                "dispatch_key",
                "pick_path",
                "startup",
                "restart_engine"
            ]
        );
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
