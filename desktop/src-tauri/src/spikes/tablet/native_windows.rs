//! Windows native listener: `octotablet` (Windows Ink RealTimeStylus) attached to the main window (**spike code**).
//!
//! The `Manager` lives on the main (UI) thread, where Ink delivers its asynchronous-plugin callbacks. A helper thread
//! asks the main thread to pump it every 2 ms and forwards the converted packets over the Tauri channel.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use octotablet::events::{Event, ToolEvent};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _};

use super::{NativeBatch, NativePacket, NativeStartInfo, epoch_ms};

thread_local! {
    static MANAGER: RefCell<Option<octotablet::Manager>> = const { RefCell::new(None) };
}

const PUMP_INTERVAL: Duration = Duration::from_millis(2);

pub(super) fn start(
    app: &AppHandle,
    channel: Channel<NativeBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, String> {
    let window = app.get_webview_window("main").ok_or("no main window")?;
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
            };
            MANAGER.with_borrow_mut(|m| *m = Some(manager));
            info
        });
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    let info = rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("main thread did not answer: {e}"))??;

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
        let mut frame_ms = None;
        for event in events {
            if let Event::Tool { tool, event } = event {
                let tool_name = tool.name.clone().unwrap_or_else(|| "pen".into());
                let device_ms = frame_ms;
                let packet = |kind| NativePacket {
                    kind,
                    host_epoch_ms: now,
                    device_ms,
                    x: 0.0,
                    y: 0.0,
                    pressure: None,
                    tilt_x: None,
                    tilt_y: None,
                    twist: None,
                    tool: tool_name.clone(),
                };
                match event {
                    ToolEvent::Pose(pose) => packets.push(NativePacket {
                        x: pose.position[0],
                        y: pose.position[1],
                        pressure: pose.pressure.get(),
                        tilt_x: pose.tilt.map(|t| t[0].to_degrees()),
                        tilt_y: pose.tilt.map(|t| t[1].to_degrees()),
                        twist: pose.roll.get().map(f32::to_degrees),
                        ..packet("pose")
                    }),
                    ToolEvent::Frame(ts) => {
                        frame_ms = ts.map(|t| (t - octotablet::events::FrameTimestamp::epoch()).as_secs_f64() * 1000.0);
                        packets.push(NativePacket { device_ms: frame_ms, ..packet("frame") });
                    }
                    ToolEvent::Down => packets.push(packet("down")),
                    ToolEvent::Up => packets.push(packet("up")),
                    ToolEvent::In { .. } => packets.push(packet("in")),
                    ToolEvent::Out => packets.push(packet("out")),
                    _ => {}
                }
            }
        }
    });
    if !packets.is_empty() {
        let _ = channel.send(NativeBatch { packets });
    }
}
