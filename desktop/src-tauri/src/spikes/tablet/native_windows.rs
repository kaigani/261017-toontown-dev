//! Windows native listener: `octotablet` (Windows Ink RealTimeStylus) attached to the main window (**spike code**).
//!
//! The `Manager` lives on the main (UI) thread, where Ink delivers its asynchronous-plugin callbacks. A helper thread
//! asks the main thread to pump it every 2 ms and forwards the converted packets over the Tauri channel.
//!
//! Timestamps (C02-R1-02): in octotablet 0.1.0, `ToolEvent::Frame(t)` *terminates* the preceding group of events
//! (its Ink backend emits `Pose` then `Frame(t)` per packet). [`pump`] therefore holds every packet until the frame
//! that closes its group arrives, possibly in a later pump, and labels the whole group with that frame's time.
//! `deviceMs` is on octotablet's own clock (arbitrary epoch); `hostEpochMs` is host wall time. The two are never mixed.
//!
//! The section from the `pump` function to the end of this file is compiled by the reviewers' replay test
//! (`tests/adversarial/c02-r1-native-timestamps.rs`) against doubles; keep it to the types that prelude provides.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use octotablet::events::{Event, ToolEvent};
use tauri::{AppHandle, Manager as _};

use super::{NativeBatch, NativePacket, NativeStartInfo, Sequencer, WireBatch, epoch_ms, window_geometry};

thread_local! {
    static MANAGER: RefCell<Option<octotablet::Manager>> = const { RefCell::new(None) };
}

const PUMP_INTERVAL: Duration = Duration::from_millis(2);

/// The production channel handed to [`pump`]: numbers packets ([`Sequencer`]) and counts failed sends before they
/// reach the Tauri channel. Named `Channel` so that `pump` reads the same in production and in the replay test.
pub(super) struct Channel<T> {
    inner: tauri::ipc::Channel<WireBatch>,
    seq: Arc<Sequencer>,
    failures: Arc<AtomicU64>,
    _batch: PhantomData<fn(T)>,
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            seq: Arc::clone(&self.seq),
            failures: Arc::clone(&self.failures),
            _batch: PhantomData,
        }
    }
}

impl Channel<NativeBatch> {
    fn new(inner: tauri::ipc::Channel<WireBatch>) -> Self {
        Self { inner, seq: Arc::default(), failures: Arc::default(), _batch: PhantomData }
    }

    fn send(&self, batch: NativeBatch) -> Result<(), ()> {
        let packets = batch.packets.into_iter().map(|p| self.seq.wrap(p)).collect();
        let wire = WireBatch { packets, host_dropped: 0, send_failures: self.failures.load(Ordering::SeqCst) };
        self.inner.send(wire).map_err(|_| {
            self.failures.fetch_add(1, Ordering::SeqCst);
        })
    }
}

pub(super) fn start(
    app: &AppHandle,
    channel: tauri::ipc::Channel<WireBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, String> {
    let window = app.get_webview_window("main").ok_or("no main window")?;
    let geometry = window_geometry(app);
    let (tx, rx) = mpsc::channel::<Result<NativeStartInfo, String>>();
    app.run_on_main_thread(move || {
        // SAFETY: `build_raw` requires the window handle to stay valid while the Manager lives. The Manager is kept in
        // a main-thread thread-local and dropped by `stop` (or at process exit); the main window lives until then.
        #[allow(unsafe_code)]
        let built = unsafe { octotablet::Builder::new().emulate_tool_from_mouse(false).build_raw(&window) };
        let result = built.map_err(|e| format!("octotablet: {e}")).map(|manager| {
            let info = NativeStartInfo {
                backend: "octotablet-ink",
                tablets: manager.tablets().iter().map(|t| t.name.clone().unwrap_or_else(|| "unnamed".into())).collect(),
                timestamp_granularity_ms: manager.timestamp_granularity().map(|d| d.as_secs_f64() * 1000.0),
                device_clock: "octotablet FrameTimestamp, ms since an arbitrary epoch",
                window: geometry,
            };
            MANAGER.with_borrow_mut(|m| *m = Some(manager));
            info
        });
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    let info = rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("main thread did not answer: {e}"))??;

    let channel = Channel::new(channel);
    let app = app.clone();
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            let channel = channel.clone();
            let _ = app.run_on_main_thread(move || pump(&channel));
            std::thread::sleep(PUMP_INTERVAL);
        }
        let _ = app.run_on_main_thread(|| MANAGER.with_borrow_mut(|m| *m = None));
    });
    Ok(info)
}

fn pump(channel: &Channel<NativeBatch>) {
    let mut packets = Vec::new();
    MANAGER.with_borrow_mut(|m| {
        let Some(manager) = m.as_mut() else { return };
        let Ok(events) = manager.pump();
        let now = epoch_ms();
        for event in events {
            if let Event::Tool { tool, event } = event {
                let tool_name = tool.name.clone().unwrap_or_else(|| "pen".into());
                let packet = |kind| NativePacket {
                    kind,
                    host_epoch_ms: now,
                    device_ms: None,
                    x: 0.0,
                    y: 0.0,
                    pressure: None,
                    tilt_x: None,
                    tilt_y: None,
                    twist: None,
                    tool: tool_name.clone(),
                };
                match event {
                    ToolEvent::Pose(pose) => hold(NativePacket {
                        x: pose.position[0],
                        y: pose.position[1],
                        pressure: pose.pressure.get(),
                        tilt_x: pose.tilt.map(|t| t[0].to_degrees()),
                        tilt_y: pose.tilt.map(|t| t[1].to_degrees()),
                        twist: pose.roll.get().map(f32::to_degrees),
                        ..packet("pose")
                    }),
                    ToolEvent::Frame(ts) => {
                        // The frame closes the group held so far: its time labels every packet of that group.
                        let device_ms =
                            ts.map(|t| (t - octotablet::events::FrameTimestamp::epoch()).as_secs_f64() * 1000.0);
                        for mut held in release() {
                            held.device_ms = device_ms;
                            packets.push(held);
                        }
                        packets.push(NativePacket { device_ms, ..packet("frame") });
                    }
                    ToolEvent::Down => hold(packet("down")),
                    ToolEvent::Up => hold(packet("up")),
                    ToolEvent::In { .. } => hold(packet("in")),
                    ToolEvent::Out => hold(packet("out")),
                    _ => {}
                }
            }
        }
    });
    if !packets.is_empty() {
        let _ = channel.send(NativeBatch { packets });
    }
}

thread_local! {
    /// Packets of the group whose terminating `Frame` has not arrived yet (it may arrive in a later pump).
    static HELD: RefCell<Vec<NativePacket>> = const { RefCell::new(Vec::new()) };
}

fn hold(packet: NativePacket) {
    HELD.with_borrow_mut(|held| held.push(packet));
}

fn release() -> Vec<NativePacket> {
    HELD.with_borrow_mut(std::mem::take)
}
