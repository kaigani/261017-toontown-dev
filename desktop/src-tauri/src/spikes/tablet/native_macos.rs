//! macOS native listener: an AppKit local event monitor for tablet and mouse events (**spike code**).
//!
//! `octotablet` 0.1.0 has no macOS backend, so the native path on macOS reads `NSEvent` directly: tablet-point and
//! tablet-proximity events, and left/right/other mouse events (including those whose subtype says they come from a
//! tablet). The monitor returns every event unchanged, so the webview still receives it and both paths can be
//! compared on the same strokes.
//!
//! Accessor preconditions (C02-R1-08). AppKit raises an Objective-C exception, which would abort the process, when an
//! accessor is read on an event type it does not document. Every read below is gated on the documented combination:
//! * `subtype` — mouse events only (never on native tablet-point/proximity events);
//! * `pressure` — mouse down/up/dragged and tablet-point events (not plain `MouseMoved`);
//! * `tilt`, `rotation` — tablet-point events and mouse events whose subtype is tablet-point;
//! * `pointingDeviceType`, `isEnteringProximity` — tablet-proximity events;
//! * `deviceID` — tablet-point/proximity events and mouse events with a tablet subtype.
//!
//! Tilt: Apple documents `tilt` as two axes scaled -1..1. The converted value keeps the earlier `asin` → degrees
//! mapping; the raw value travels next to it (`rawTiltX/Y`) so the user's run can settle the calibration.
//! The tool reported by the last proximity event (pen/eraser/cursor) is carried into later poses.
//!
//! The section from the `device_name` function to the end of this file is compiled by the reviewers' contract test
//! (`tests/adversarial/c02-r1-macos-accessors.rs`) against contract-enforcing doubles; keep it to the API that prelude
//! provides. Right/other buttons, device ids and raw tilt are therefore handled in [`capture`], above that section.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventSubtype, NSEventType, NSPointingDeviceType};
use tauri::AppHandle;
use tauri::ipc::Channel;

use super::{NativePacket, NativeStartInfo, Sequencer, WireBatch, WirePacket, epoch_ms, window_geometry};

thread_local! {
    static MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

const FLUSH_INTERVAL: Duration = Duration::from_millis(4);
const MAX_BUFFERED: usize = 100_000;

pub(super) fn start(
    app: &AppHandle,
    channel: Channel<WireBatch>,
    stop: Arc<AtomicBool>,
) -> Result<NativeStartInfo, String> {
    let buffer: Arc<Mutex<Vec<WirePacket>>> = Arc::default();
    let dropped = Arc::new(AtomicU64::new(0));
    let sequencer = Arc::new(Sequencer::default());
    let (sink, sink_dropped, seq) = (Arc::clone(&buffer), Arc::clone(&dropped), Arc::clone(&sequencer));
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    app.run_on_main_thread(move || {
        let mask = NSEventMask::TabletPoint
            | NSEventMask::TabletProximity
            | NSEventMask::LeftMouseDown
            | NSEventMask::LeftMouseUp
            | NSEventMask::LeftMouseDragged
            | NSEventMask::RightMouseDown
            | NSEventMask::RightMouseUp
            | NSEventMask::RightMouseDragged
            | NSEventMask::OtherMouseDown
            | NSEventMask::OtherMouseUp
            | NSEventMask::OtherMouseDragged
            | NSEventMask::MouseMoved;
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit passes a valid NSEvent for the duration of the handler.
            #[allow(unsafe_code)]
            let e = unsafe { event.as_ref() };
            if let Some(packet) = capture(e, &seq) {
                match sink.lock() {
                    Ok(mut b) if b.len() < MAX_BUFFERED => b.push(packet),
                    _ => {
                        sink_dropped.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

    let app_for_stop = app.clone();
    std::thread::spawn(move || {
        let failures = AtomicU64::new(0);
        while !stop.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(FLUSH_INTERVAL);
            let packets = buffer.lock().map(|mut b| std::mem::take(&mut *b)).unwrap_or_default();
            if !packets.is_empty() {
                let batch = WireBatch {
                    packets,
                    host_dropped: dropped.load(std::sync::atomic::Ordering::SeqCst),
                    send_failures: failures.load(std::sync::atomic::Ordering::SeqCst),
                };
                if channel.send(batch).is_err() {
                    failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        }
        let _ = app_for_stop.run_on_main_thread(|| {
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
        device_clock: "NSEvent.timestamp, ms since system boot",
        window: window_geometry(app),
    })
}

fn is_mouse_type(t: NSEventType) -> bool {
    [
        NSEventType::LeftMouseDown,
        NSEventType::LeftMouseUp,
        NSEventType::LeftMouseDragged,
        NSEventType::RightMouseDown,
        NSEventType::RightMouseUp,
        NSEventType::RightMouseDragged,
        NSEventType::OtherMouseDown,
        NSEventType::OtherMouseUp,
        NSEventType::OtherMouseDragged,
        NSEventType::MouseMoved,
    ]
    .contains(&t)
}

/// Whether tablet accessors (`deviceID`) are documented for this event: native tablet events, or mouse events whose
/// subtype is tablet-point/proximity. `subtype` is read only for mouse events.
fn carries_tablet_data(e: &NSEvent) -> bool {
    let t = e.r#type();
    if t == NSEventType::TabletPoint || t == NSEventType::TabletProximity {
        return true;
    }
    is_mouse_type(t) && {
        let s = e.subtype();
        s == NSEventSubtype::TabletPoint || s == NSEventSubtype::TabletProximity
    }
}

/// Converts right/other-button mouse events (not modelled by the reviewers' contract doubles, so not in [`convert`]).
fn convert_button(e: &NSEvent) -> Option<NativePacket> {
    let t = e.r#type();
    let (kind, button) = if t == NSEventType::RightMouseDown {
        ("down", "right")
    } else if t == NSEventType::RightMouseUp {
        ("up", "right")
    } else if t == NSEventType::RightMouseDragged {
        ("pose", "right")
    } else if t == NSEventType::OtherMouseDown {
        ("down", "other")
    } else if t == NSEventType::OtherMouseUp {
        ("up", "other")
    } else if t == NSEventType::OtherMouseDragged {
        ("pose", "other")
    } else {
        return None;
    };
    let at = e.locationInWindow();
    // Mouse down/up/dragged: pressure and subtype are documented.
    let mut packet = NativePacket {
        kind,
        host_epoch_ms: epoch_ms(),
        device_ms: Some(e.timestamp() * 1000.0),
        x: at.x as f32,
        y: at.y as f32,
        pressure: Some(e.pressure()),
        tilt_x: None,
        tilt_y: None,
        twist: None,
        tool: format!("mouse ({button} button)"),
    };
    if e.subtype() == NSEventSubtype::TabletPoint {
        fill_tablet(e, &mut packet);
        packet.tool =
            format!("{} ({button} button)", tool_label(CURRENT_TOOL.load(std::sync::atomic::Ordering::SeqCst)));
    }
    Some(packet)
}

/// Everything the monitor records for one event: the converted packet plus extras the doubles do not model.
fn capture(e: &NSEvent, seq: &Sequencer) -> Option<WirePacket> {
    let packet = convert(e).or_else(|| convert_button(e))?;
    let has_tilt = packet.tilt_x.is_some();
    let mut wire = seq.wrap(packet);
    if has_tilt {
        // `tilt` was valid for this event (convert only reads it when documented).
        let raw = e.tilt();
        wire.raw_tilt_x = Some(raw.x);
        wire.raw_tilt_y = Some(raw.y);
    }
    if carries_tablet_data(e) {
        wire.device_id = Some(e.deviceID().to_string());
    }
    wire.window_number = i64::try_from(e.windowNumber()).ok();
    Some(wire)
}

fn device_name(kind: NSPointingDeviceType) -> &'static str {
    match kind {
        NSPointingDeviceType::Pen => "pen",
        NSPointingDeviceType::Eraser => "eraser",
        NSPointingDeviceType::Cursor => "cursor",
        _ => "unknown",
    }
}

const TOOL_PEN: u8 = 0;
const TOOL_ERASER: u8 = 1;
const TOOL_CURSOR: u8 = 2;
const TOOL_UNKNOWN: u8 = 3;

/// Tool of the most recent proximity event, carried into later tablet poses.
static CURRENT_TOOL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(TOOL_PEN);

fn tool_code(name: &str) -> u8 {
    match name {
        "pen" => TOOL_PEN,
        "eraser" => TOOL_ERASER,
        "cursor" => TOOL_CURSOR,
        _ => TOOL_UNKNOWN,
    }
}

fn tool_label(code: u8) -> &'static str {
    match code {
        TOOL_PEN => "pen",
        TOOL_ERASER => "eraser",
        TOOL_CURSOR => "cursor",
        _ => "unknown",
    }
}

/// Reads the tablet-only axes. Call only for tablet-point events or mouse events with a tablet-point subtype.
fn fill_tablet(e: &NSEvent, packet: &mut NativePacket) {
    let tilt = e.tilt();
    packet.tilt_x = Some((tilt.x as f32).clamp(-1.0, 1.0).asin().to_degrees());
    packet.tilt_y = Some((tilt.y as f32).clamp(-1.0, 1.0).asin().to_degrees());
    packet.twist = Some(e.rotation());
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
        // Proximity: device type and direction are documented; pressure and subtype are not read.
        let name = device_name(e.pointingDeviceType());
        CURRENT_TOOL.store(tool_code(name), std::sync::atomic::Ordering::SeqCst);
        return Some(NativePacket {
            tool: name.to_owned(),
            ..base(if e.isEnteringProximity() { "in" } else { "out" })
        });
    }
    if t == NSEventType::TabletPoint {
        // Native tablet-point: pressure, tilt and rotation are documented; `subtype` is NOT (not a mouse event).
        let at = e.locationInWindow();
        let mut packet = NativePacket {
            x: at.x as f32,
            y: at.y as f32,
            pressure: Some(e.pressure()),
            tool: tool_label(CURRENT_TOOL.load(std::sync::atomic::Ordering::SeqCst)).to_owned(),
            ..base("pose")
        };
        fill_tablet(e, &mut packet);
        return Some(packet);
    }
    let pressed =
        t == NSEventType::LeftMouseDown || t == NSEventType::LeftMouseUp || t == NSEventType::LeftMouseDragged;
    if !(pressed || t == NSEventType::MouseMoved) {
        return None;
    }
    // Mouse events: `subtype` is documented.
    let subtype = e.subtype();
    if subtype == NSEventSubtype::TabletProximity {
        return Some(base("proximity"));
    }
    let kind = if t == NSEventType::LeftMouseDown {
        "down"
    } else if t == NSEventType::LeftMouseUp {
        "up"
    } else {
        "pose"
    };
    let at = e.locationInWindow();
    let mut packet = NativePacket { x: at.x as f32, y: at.y as f32, ..base(kind) };
    if pressed {
        // Pressure is documented for mouse down/up/dragged, not for plain mouse movement (hover).
        packet.pressure = Some(e.pressure());
    }
    if subtype == NSEventSubtype::TabletPoint {
        fill_tablet(e, &mut packet);
        packet.tool = tool_label(CURRENT_TOOL.load(std::sync::atomic::Ordering::SeqCst)).to_owned();
    }
    Some(packet)
}
