//! macOS native listener: an AppKit local event monitor for tablet and mouse events (**spike code**).
//!
//! `octotablet` 0.1.0 has no macOS backend, so the native path on macOS reads `NSEvent` directly: tablet-point and
//! tablet-proximity events, and mouse events whose subtype says they come from a tablet. The monitor returns every
//! event unchanged, so the webview still receives it and both paths can be compared on the same strokes.
//!
//! Only accessors that are valid for each event type are called: AppKit raises an Objective-C exception for invalid
//! ones (for example `pressure` on a proximity event), which would abort the process.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventSubtype, NSEventType, NSPointingDeviceType};
use tauri::AppHandle;
use tauri::ipc::Channel;

use super::{NativeBatch, NativePacket, NativeStartInfo, epoch_ms};

thread_local! {
    static MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

const FLUSH_INTERVAL: Duration = Duration::from_millis(4);
const MAX_BUFFERED: usize = 100_000;

pub(super) fn start(
    app: &AppHandle,
    channel: Channel<NativeBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, String> {
    let buffer: Arc<Mutex<Vec<NativePacket>>> = Arc::default();
    let sink = Arc::clone(&buffer);
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    app.run_on_main_thread(move || {
        let mask = NSEventMask::TabletPoint
            | NSEventMask::TabletProximity
            | NSEventMask::LeftMouseDown
            | NSEventMask::LeftMouseUp
            | NSEventMask::LeftMouseDragged
            | NSEventMask::MouseMoved;
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit passes a valid NSEvent for the duration of the handler.
            #[allow(unsafe_code)]
            let e = unsafe { event.as_ref() };
            if let Some(packet) = convert(e) {
                if let Ok(mut b) = sink.lock() {
                    if b.len() < MAX_BUFFERED {
                        b.push(packet);
                    }
                }
            }
            // Return the event unchanged so the webview still receives it.
            event.as_ptr()
        });
        // SAFETY: called on the main thread; the handler always returns the (valid) event it was given.
        #[allow(unsafe_code)]
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        let result = match monitor {
            Some(m) => {
                MONITOR.with_borrow_mut(|slot| *slot = Some(m));
                Ok(())
            }
            None => Err("addLocalMonitorForEventsMatchingMask returned nil".to_owned()),
        };
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("main thread did not answer: {e}"))??;

    let app = app.clone();
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(FLUSH_INTERVAL);
            let packets = buffer.lock().map(|mut b| std::mem::take(&mut *b)).unwrap_or_default();
            if !packets.is_empty() {
                let _ = channel.send(NativeBatch { packets });
            }
        }
        let _ = app.run_on_main_thread(|| {
            MONITOR.with_borrow_mut(|slot| {
                if let Some(m) = slot.take() {
                    // SAFETY: `m` is the monitor object returned by addLocalMonitorForEventsMatchingMask_handler.
                    #[allow(unsafe_code)]
                    unsafe {
                        NSEvent::removeMonitor(&m);
                    }
                }
            });
        });
    });
    Ok(NativeStartInfo {
        backend: "appkit-nsevent",
        tablets: Vec::new(),
        // NSEvent.timestamp is a double in seconds since boot; AppKit documents no coarser granularity.
        timestamp_granularity_ms: None,
    })
}

fn device_name(kind: NSPointingDeviceType) -> &'static str {
    match kind {
        NSPointingDeviceType::Pen => "pen",
        NSPointingDeviceType::Eraser => "eraser",
        NSPointingDeviceType::Cursor => "cursor",
        _ => "unknown",
    }
}

fn convert(e: &NSEvent) -> Option<NativePacket> {
    let t = e.r#type();
    let base = |kind: &'static str| NativePacket {
        kind,
        host_epoch_ms: epoch_ms(),
        device_ms: Some(e.timestamp() * 1000.0),
        x: 0.0,
        y: 0.0,
        pressure: None,
        tilt_x: None,
        tilt_y: None,
        twist: None,
        tool: "mouse".to_owned(),
    };
    if t == NSEventType::TabletProximity {
        let entering = e.isEnteringProximity();
        return Some(NativePacket {
            tool: device_name(e.pointingDeviceType()).to_owned(),
            ..base(if entering { "in" } else { "out" })
        });
    }
    let is_mouse = t == NSEventType::LeftMouseDown
        || t == NSEventType::LeftMouseUp
        || t == NSEventType::LeftMouseDragged
        || t == NSEventType::MouseMoved;
    if !(t == NSEventType::TabletPoint || is_mouse) {
        return None;
    }
    let subtype = e.subtype();
    if is_mouse && subtype == NSEventSubtype::TabletProximity {
        return Some(base("proximity"));
    }
    let from_tablet = t == NSEventType::TabletPoint || subtype == NSEventSubtype::TabletPoint;
    let kind = if t == NSEventType::LeftMouseDown {
        "down"
    } else if t == NSEventType::LeftMouseUp {
        "up"
    } else {
        "pose"
    };
    let at = e.locationInWindow();
    let mut packet = NativePacket { x: at.x as f32, y: at.y as f32, pressure: Some(e.pressure()), ..base(kind) };
    if from_tablet {
        // tilt is -1..1 on each axis; report degrees like the web path (approximately asin * 90/pi*2).
        let tilt = e.tilt();
        packet.tilt_x = Some((tilt.x as f32).clamp(-1.0, 1.0).asin().to_degrees());
        packet.tilt_y = Some((tilt.y as f32).clamp(-1.0, 1.0).asin().to_degrees());
        packet.twist = Some(e.rotation());
        packet.tool = "pen".to_owned();
    }
    Some(packet)
}
