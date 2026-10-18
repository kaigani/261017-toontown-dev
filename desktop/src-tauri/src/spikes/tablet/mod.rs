//! C0.2 tablet-input spike, host side (**spike code**).
//!
//! Two input paths are compared side by side on the spike page:
//! * **web**: pointer events in the webview (measured entirely in the front end, `desktop/src/spikes/tablet/`);
//! * **native**: a platform listener in this host streaming packets to the front end over a Tauri channel —
//!   `octotablet` (Windows Ink RealTimeStylus) on Windows; AppKit `NSEvent` tablet events on macOS, because
//!   `octotablet` 0.1.0 has no macOS backend (its README lists macOS as "Help needed").
//!
//! On Windows without tablet hardware, [`simulate_pen`] injects a synthetic pen stroke (`InjectSyntheticPointerInput`)
//! into this app's own window only, so both paths can be exercised and their results compared with known input.

#[cfg(target_os = "macos")]
mod native_macos;
#[cfg(windows)]
mod native_windows;
#[cfg(windows)]
mod probe_windows;
#[cfg(windows)]
mod simulate_windows;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tt_engine::ErrorInfo;

/// Spike launch settings (`--spike tablet [--spike-log <file>]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpikeSettings {
    /// Spike name; only `tablet` exists.
    pub name: String,
    /// Where `spike_save_log` writes the JSON log.
    pub log_path: Option<PathBuf>,
    /// `--spike-auto`: the page runs the synthetic-pen scenarios, saves the log and the app exits.
    pub auto: bool,
}

/// One input packet as produced by a platform converter. The field set is pinned by the reviewers' replay tests
/// (`tests/adversarial/c02-r1-*.rs` compile the converters against it); per-packet extras travel in [`WirePacket`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePacket {
    /// `pose`, `down`, `up`, `in`, `out`, `frame`, `proximity`.
    pub kind: &'static str,
    /// Host wall-clock time when the packet was read, ms since the Unix epoch.
    pub host_epoch_ms: f64,
    /// Device/OS timestamp in ms, on the backend's own clock (only differences are meaningful).
    pub device_ms: Option<f64>,
    /// Position in window logical pixels (macOS: window points, origin bottom-left).
    pub x: f32,
    /// See `x`.
    pub y: f32,
    /// Normalised pressure 0..1.
    pub pressure: Option<f32>,
    /// Tilt in degrees.
    pub tilt_x: Option<f32>,
    /// Tilt in degrees.
    pub tilt_y: Option<f32>,
    /// Barrel rotation in degrees.
    pub twist: Option<f32>,
    /// Tool description.
    pub tool: String,
}

/// Packets produced by one converter pass (internal; see [`WireBatch`] for what crosses the channel).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBatch {
    /// Packets in arrival order.
    pub packets: Vec<NativePacket>,
}

/// A packet as sent over the channel: the converter's packet plus a gap-free host sequence number (so the page can
/// count channel loss against a reference) and platform extras.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePacket {
    /// Host sequence number, 0-based, contiguous for the listener's lifetime.
    pub seq: u64,
    /// The converted packet.
    #[serde(flatten)]
    pub packet: NativePacket,
    /// Raw tilt as reported by the OS (macOS: `NSEvent.tilt`, scaled -1..1), next to the converted degrees.
    pub raw_tilt_x: Option<f64>,
    /// See `raw_tilt_x`.
    pub raw_tilt_y: Option<f64>,
    /// Device identity when the OS provides one (macOS: `deviceID` of tablet/proximity events).
    pub device_id: Option<String>,
    /// macOS: `NSEvent.windowNumber` (0 = no window); conversion input for `locationInWindow`.
    pub window_number: Option<i64>,
}

/// A batch on the channel, with cumulative host-side loss counters.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireBatch {
    /// Packets in arrival order.
    pub packets: Vec<WirePacket>,
    /// Packets the host discarded because its buffer was full (cumulative).
    pub host_dropped: u64,
    /// Channel sends that failed (cumulative; their packets never reached the page).
    pub send_failures: u64,
}

/// Wraps converter packets into [`WirePacket`]s with sequence numbers.
#[derive(Debug, Default)]
pub struct Sequencer {
    next: std::sync::atomic::AtomicU64,
}

impl Sequencer {
    /// Wraps `packet` with the next sequence number and no extras.
    pub fn wrap(&self, packet: NativePacket) -> WirePacket {
        WirePacket {
            seq: self.next.fetch_add(1, Ordering::SeqCst),
            packet,
            raw_tilt_x: None,
            raw_tilt_y: None,
            device_id: None,
            window_number: None,
        }
    }
}

/// The main window's geometry when the listener started (conversion inputs for native coordinates).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    /// Backing scale factor (physical pixels per logical pixel).
    pub scale_factor: f64,
    /// Client area origin on screen, physical pixels.
    pub inner_x: i32,
    /// See `inner_x`.
    pub inner_y: i32,
    /// Client area size, physical pixels.
    pub inner_width: u32,
    /// See `inner_width`.
    pub inner_height: u32,
}

/// Host facts recorded in the log (the page adds what the webview knows).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeEnvironment {
    /// `windows`, `macos`, `linux`.
    pub os: &'static str,
    /// CPU architecture.
    pub arch: &'static str,
    /// OS version string (Windows: `ver`; macOS: `sw_vers`), if it could be read.
    pub os_version: Option<String>,
    /// WebView2 runtime version (Windows) or WebKit framework version (macOS), from Tauri.
    pub webview_version: Option<String>,
    /// Logical CPUs available to the process.
    pub cpu_count: Option<u32>,
    /// Main window geometry now.
    pub window: Option<WindowGeometry>,
}

/// Result of starting the native listener.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStartInfo {
    /// `octotablet-ink` or `appkit-nsevent`.
    pub backend: &'static str,
    /// Tablets the backend reported at start (names).
    pub tablets: Vec<String>,
    /// Backend timestamp granularity in ms, if known.
    pub timestamp_granularity_ms: Option<f64>,
    /// Clock domain of `deviceMs` (only differences within one domain are meaningful).
    pub device_clock: &'static str,
    /// Main window geometry at start.
    pub window: Option<WindowGeometry>,
}

/// What the front end needs to know about the spike launch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeMode {
    /// Spike name.
    pub name: String,
    /// Log destination, if `--spike-log` was given.
    pub log_path: Option<String>,
    /// Native backend available on this platform, if any.
    pub native_backend: Option<&'static str>,
    /// Whether synthetic pen injection is available (Windows).
    pub pen_simulation: bool,
    /// Automated run (`--spike-auto`).
    pub auto: bool,
}

/// Parameters of a synthetic pen stroke (Windows).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PenSimRequest {
    /// Target rectangle in the webview's CSS pixels (the canvas).
    pub left: f64,
    /// See `left`.
    pub top: f64,
    /// See `left`.
    pub width: f64,
    /// See `left`.
    pub height: f64,
    /// Frames per second to inject (1..=1000).
    pub hz: u32,
    /// Number of contact frames (1..=10000). Pressure steps through 1..=1024 once per 1024 frames.
    pub frames: u32,
    /// Path: `sine` (default), `zigzag` (at least 2 px between frames, so no two adjacent frames share a pixel) or
    /// `stationary` (one fixed point, pressure-only changes).
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Report of an injected stroke.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PenSimReport {
    /// `InjectSyntheticPointerInput` (synthetic pen, not hardware).
    pub method: &'static str,
    /// Contact frames injected.
    pub frames: u32,
    /// Requested rate.
    pub hz: u32,
    /// Achieved rate over the contact frames.
    pub achieved_hz: f64,
    /// Host wall-clock time taken immediately **before** each contact frame's inject call, ms since the Unix epoch.
    pub inject_epoch_ms: Vec<f64>,
    /// Host wall-clock time when each inject call returned.
    pub inject_return_epoch_ms: Vec<f64>,
    /// Screen position of each contact frame, physical pixels.
    pub screen_x: Vec<i32>,
    /// See `screen_x`.
    pub screen_y: Vec<i32>,
    /// Target rectangle on screen `[left, top, width, height]`, physical pixels.
    pub screen_rect: [i32; 4],
    /// Path pattern actually used.
    pub pattern: String,
    /// What was targeted: `webview` (this app's canvas) or `nativeWindow` (the probe window).
    pub target: &'static str,
    /// Pressure injected in each contact frame (0..1024).
    pub pressure: Vec<u32>,
    /// Tilt X injected in each contact frame (degrees).
    pub tilt_x: Vec<i32>,
    /// Tilt Y injected in each contact frame (degrees).
    pub tilt_y: Vec<i32>,
    /// Rotation injected in each contact frame (degrees).
    pub rotation: Vec<u32>,
}

/// Saved-log result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedLog {
    /// Absolute path written.
    pub path: String,
    /// Bytes written.
    pub bytes: u64,
}

/// An automated run (`--spike-auto`) that has not saved its log after this long exits with status 2.
pub const AUTO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Exits the app with status 2 if an automated run never saves its log (for example, the webview failed to load).
pub fn arm_auto_timeout(app: &AppHandle, settings: &SpikeSettings) {
    if !settings.auto {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(AUTO_TIMEOUT);
        app.exit(2);
    });
}

/// Managed state of the tablet spike.
#[derive(Debug, Default)]
pub struct TabletSpike {
    settings: Option<SpikeSettings>,
    native: Mutex<Option<Arc<AtomicBool>>>,
}

impl TabletSpike {
    /// State for the given launch settings (`None` = spike disabled).
    #[must_use]
    pub fn new(settings: Option<SpikeSettings>) -> Self {
        Self { settings: settings.filter(|s| s.name == "tablet"), native: Mutex::new(None) }
    }

    fn require(&self) -> Result<&SpikeSettings, ErrorInfo> {
        self.settings.as_ref().ok_or_else(|| error("spike.notEnabled", "start the app with --spike tablet".into()))
    }
}

pub(crate) fn error(code: &str, message: String) -> ErrorInfo {
    ErrorInfo { code: code.to_owned(), message, details: serde_json::json!({}) }
}

pub(crate) fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

const NATIVE_BACKEND: Option<&str> = if cfg!(windows) {
    Some("octotablet-ink")
} else if cfg!(target_os = "macos") {
    Some("appkit-nsevent")
} else {
    None
};

/// Main-window geometry (physical pixels), if the window exists.
pub(crate) fn window_geometry(app: &AppHandle) -> Option<WindowGeometry> {
    use tauri::Manager as _;
    let window = app.get_webview_window("main")?;
    let origin = window.inner_position().ok()?;
    let size = window.inner_size().ok()?;
    Some(WindowGeometry {
        scale_factor: window.scale_factor().ok()?,
        inner_x: origin.x,
        inner_y: origin.y,
        inner_width: size.width,
        inner_height: size.height,
    })
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn os_version() -> Option<String> {
    if cfg!(windows) {
        command_output("cmd", &["/C", "ver"])
    } else if cfg!(target_os = "macos") {
        let name = command_output("sw_vers", &["-productVersion"])?;
        let build = command_output("sw_vers", &["-buildVersion"]).unwrap_or_default();
        Some(format!("macOS {name} ({build})"))
    } else {
        None
    }
}

/// Host environment for the log: OS version, webview runtime version, CPU count, window geometry.
///
/// # Errors
/// `spike.notEnabled`.
#[tauri::command]
pub fn spike_environment(app: AppHandle, spike: State<'_, TabletSpike>) -> Result<SpikeEnvironment, ErrorInfo> {
    spike.require()?;
    Ok(SpikeEnvironment {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        os_version: os_version(),
        webview_version: tauri::webview_version().ok(),
        cpu_count: std::thread::available_parallelism().ok().and_then(|n| u32::try_from(n.get()).ok()),
        window: window_geometry(&app),
    })
}

/// The spike launch, or `null` when the app was not started with `--spike tablet`.
#[tauri::command]
#[must_use]
pub fn spike_mode(spike: State<'_, TabletSpike>) -> Option<SpikeMode> {
    spike.settings.as_ref().map(|s| SpikeMode {
        name: s.name.clone(),
        log_path: s.log_path.as_ref().map(|p| p.display().to_string()),
        native_backend: NATIVE_BACKEND,
        pen_simulation: cfg!(windows),
        auto: s.auto,
    })
}

/// Starts the native listener, streaming [`NativeBatch`]es over `channel` until [`spike_tablet_native_stop`].
///
/// # Errors
/// `spike.notEnabled`, `spike.busy` (already running), `spike.unsupported`, `spike.nativeFailed`.
#[tauri::command]
pub fn spike_tablet_native_start(
    app: AppHandle,
    spike: State<'_, TabletSpike>,
    channel: Channel<WireBatch>,
) -> Result<NativeStartInfo, ErrorInfo> {
    spike.require()?;
    let mut slot = spike.native.lock().map_err(|_| error("internal", "spike state poisoned".into()))?;
    if slot.is_some() {
        return Err(error("spike.busy", "the native listener is already running".into()));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let info = start_native(&app, channel, Arc::clone(&stop))?;
    *slot = Some(stop);
    Ok(info)
}

#[cfg(windows)]
fn start_native(
    app: &AppHandle,
    channel: Channel<WireBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, ErrorInfo> {
    native_windows::start(app, channel, stop).map_err(|e| error("spike.nativeFailed", e))
}

#[cfg(target_os = "macos")]
fn start_native(
    app: &AppHandle,
    channel: Channel<WireBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, ErrorInfo> {
    native_macos::start(app, channel, stop).map_err(|e| error("spike.nativeFailed", e))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn start_native(_: &AppHandle, _: Channel<WireBatch>, _: Arc<AtomicBool>) -> Result<NativeStartInfo, ErrorInfo> {
    Err(error("spike.unsupported", "no native tablet backend on this platform".into()))
}

/// Stops the native listener (no-op if it is not running).
///
/// # Errors
/// `spike.notEnabled`.
#[tauri::command]
pub fn spike_tablet_native_stop(spike: State<'_, TabletSpike>) -> Result<(), ErrorInfo> {
    spike.require()?;
    let mut slot = spike.native.lock().map_err(|_| error("internal", "spike state poisoned".into()))?;
    if let Some(stop) = slot.take() {
        stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Injects a synthetic pen stroke into this app's window (Windows only; refuses unless the window is in front).
///
/// # Errors
/// `spike.notEnabled`, `spike.unsupported` (not Windows), `arg.invalid`, `spike.notForeground`, `spike.injectFailed`.
#[tauri::command]
pub async fn spike_pen_simulate(
    app: AppHandle,
    spike: State<'_, TabletSpike>,
    request: PenSimRequest,
) -> Result<PenSimReport, ErrorInfo> {
    spike.require()?;
    validate(&request)?;
    simulate_pen(app, request).await
}

fn validate(r: &PenSimRequest) -> Result<(), ErrorInfo> {
    let finite = [r.left, r.top, r.width, r.height].iter().all(|v| v.is_finite());
    if !finite || r.width < 10.0 || r.height < 10.0 || r.left < 0.0 || r.top < 0.0 {
        return Err(error(
            "arg.invalid",
            "the target rectangle must be finite, non-negative and at least 10x10".into(),
        ));
    }
    if !(1..=1000).contains(&r.hz) || !(1..=10_000).contains(&r.frames) {
        return Err(error("arg.invalid", "hz must be 1..=1000 and frames 1..=10000".into()));
    }
    if !matches!(r.pattern.as_deref(), None | Some("sine" | "zigzag" | "stationary")) {
        return Err(error("arg.invalid", "pattern must be sine, zigzag or stationary".into()));
    }
    Ok(())
}

#[cfg(windows)]
async fn simulate_pen(app: AppHandle, request: PenSimRequest) -> Result<PenSimReport, ErrorInfo> {
    tauri::async_runtime::spawn_blocking(move || simulate_windows::run(&app, &request))
        .await
        .map_err(|e| error("internal", format!("simulation task failed: {e}")))?
}

#[cfg(not(windows))]
async fn simulate_pen(_: AppHandle, _: PenSimRequest) -> Result<PenSimReport, ErrorInfo> {
    Err(error("spike.unsupported", "synthetic pen injection exists only on Windows".into()))
}

/// Windows: the discriminating experiment of review C02-R1-04. Injects the same synthetic stroke into a plain
/// same-process native window (recording WM_POINTER with pen history, an `octotablet` RealTimeStylus on that window,
/// and raw HID input), then returns. The webview half of the matrix is the ordinary simulation with
/// [`spike_raw_input_sink`] running.
///
/// # Errors
/// `spike.notEnabled`, `spike.unsupported` (not Windows), `arg.invalid`, `spike.notForeground`, `spike.injectFailed`.
#[tauri::command]
pub async fn spike_probe_native_window(
    app: AppHandle,
    spike: State<'_, TabletSpike>,
    request: PenSimRequest,
) -> Result<serde_json::Value, ErrorInfo> {
    spike.require()?;
    validate(&request)?;
    probe_native(app, request).await
}

#[cfg(windows)]
async fn probe_native(app: AppHandle, request: PenSimRequest) -> Result<serde_json::Value, ErrorInfo> {
    tauri::async_runtime::spawn_blocking(move || probe_windows::native_window_run(&app, &request))
        .await
        .map_err(|e| error("internal", format!("probe task failed: {e}")))?
}

#[cfg(not(windows))]
async fn probe_native(_: AppHandle, _: PenSimRequest) -> Result<serde_json::Value, ErrorInfo> {
    Err(error("spike.unsupported", "the native-window probe exists only on Windows".into()))
}

/// Windows: starts (`true`) or stops (`false`) a raw-input sink for HID digitizer collections (usage page 0x0D)
/// on a hidden same-process window; stopping returns what it received. Used during the webview simulation.
///
/// # Errors
/// `spike.notEnabled`, `spike.unsupported` (not Windows), `spike.probeFailed`.
#[tauri::command]
pub fn spike_raw_input_sink(spike: State<'_, TabletSpike>, start: bool) -> Result<serde_json::Value, ErrorInfo> {
    spike.require()?;
    raw_sink(start)
}

#[cfg(windows)]
fn raw_sink(start: bool) -> Result<serde_json::Value, ErrorInfo> {
    if start { probe_windows::raw_sink_start() } else { probe_windows::raw_sink_stop() }
}

#[cfg(not(windows))]
fn raw_sink(_: bool) -> Result<serde_json::Value, ErrorInfo> {
    Err(error("spike.unsupported", "raw input exists only on Windows".into()))
}

/// Writes the front end's JSON log to the `--spike-log` path (and exits the app after an automated run).
///
/// # Errors
/// `spike.notEnabled`, `spike.noLogPath`, `fs.unwritable`.
#[tauri::command]
pub fn spike_save_log(
    app: AppHandle,
    spike: State<'_, TabletSpike>,
    log: serde_json::Value,
) -> Result<SavedLog, ErrorInfo> {
    let settings = spike.require()?;
    let Some(path) = settings.log_path.as_ref() else {
        return Err(error("spike.noLogPath", "start the app with --spike-log <file> to save the log".into()));
    };
    let text = serde_json::to_string_pretty(&log).map_err(|e| error("internal", e.to_string()))? + "\n";
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| error("fs.unwritable", format!("{}: {e}", dir.display())))?;
    }
    std::fs::write(path, &text).map_err(|e| error("fs.unwritable", format!("{}: {e}", path.display())))?;
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
    let saved = SavedLog { path: absolute.display().to_string(), bytes: text.len() as u64 };
    if settings.auto {
        app.exit(0);
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> PenSimRequest {
        PenSimRequest { left: 0.0, top: 0.0, width: 100.0, height: 100.0, hz: 240, frames: 100, pattern: None }
    }

    #[test]
    fn disabled_unless_tablet() {
        assert!(TabletSpike::new(None).require().is_err());
        let other = SpikeSettings { name: "canvas".into(), log_path: None, auto: false };
        assert!(TabletSpike::new(Some(other)).require().is_err());
        let tablet = SpikeSettings { name: "tablet".into(), log_path: None, auto: false };
        assert!(TabletSpike::new(Some(tablet)).require().is_ok());
    }

    #[test]
    fn validates_simulation_requests() {
        assert!(validate(&req()).is_ok());
        assert!(validate(&PenSimRequest { hz: 0, ..req() }).is_err());
        assert!(validate(&PenSimRequest { frames: 20_000, ..req() }).is_err());
        assert!(validate(&PenSimRequest { width: 5.0, ..req() }).is_err());
        assert!(validate(&PenSimRequest { left: f64::NAN, ..req() }).is_err());
        assert!(validate(&PenSimRequest { top: -1.0, ..req() }).is_err());
        assert!(validate(&PenSimRequest { pattern: Some("zigzag".into()), ..req() }).is_ok());
        assert!(validate(&PenSimRequest { pattern: Some("spiral".into()), ..req() }).is_err());
    }

    #[test]
    fn request_rejects_unknown_fields() {
        let bad = r#"{"left":0,"top":0,"width":100,"height":100,"hz":240,"frames":10,"x":1}"#;
        assert!(serde_json::from_str::<PenSimRequest>(bad).is_err());
    }

    #[test]
    fn sequencer_is_gap_free_and_wire_packets_flatten() {
        let seq = Sequencer::default();
        let p = NativePacket {
            kind: "pose",
            host_epoch_ms: 1.0,
            device_ms: Some(2.0),
            x: 0.0,
            y: 0.0,
            pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            tool: "pen".into(),
        };
        let a = seq.wrap(p.clone());
        let b = seq.wrap(p);
        assert_eq!((a.seq, b.seq), (0, 1));
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["seq"], 1);
        assert_eq!(v["deviceMs"], 2.0);
        assert!(v.get("rawTiltX").is_some() && v.get("packet").is_none());
    }

    #[test]
    fn packet_serializes_camel_case() {
        let p = NativePacket {
            kind: "pose",
            host_epoch_ms: 1.0,
            device_ms: None,
            x: 1.0,
            y: 2.0,
            pressure: Some(0.5),
            tilt_x: None,
            tilt_y: None,
            twist: None,
            tool: "pen".into(),
        };
        let v = serde_json::to_value(p).unwrap();
        assert_eq!(v["hostEpochMs"], 1.0);
        assert!(v.get("tiltX").is_some());
    }
}
