//! Synthetic pen injection for measuring the web and native paths without tablet hardware (**spike code**, Windows).
//!
//! Uses `CreateSyntheticPointerDevice(PT_PEN)` + `InjectSyntheticPointerInput` (Windows 10 1809+). This is a
//! **simulation**: it exercises the OS pointer stack, WebView2 and Windows Ink with known values, but it is not
//! evidence about real tablet hardware or drivers.
//!
//! Safety of the user's session: input is injected only inside this app's own canvas rectangle, and only at screen
//! points where the topmost window is this app's main window (`WindowFromPoint` -> root ancestor). The check runs for
//! every frame; the stroke is lifted and aborted as soon as any other window covers the point. Requiring keyboard focus
//! instead does not work for a process started in the background (Windows' focus-stealing protection), and is not
//! needed: pointer input goes to the window under the point. The window is kept always-on-top only for the stroke.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager as _};
use tt_engine::ErrorInfo;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Controls::{
    CreateSyntheticPointerDevice, DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE, POINTER_FEEDBACK_DEFAULT,
    POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
};
use windows::Win32::UI::Input::Pointer::{
    InjectSyntheticPointerInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_UP,
    POINTER_FLAG_UPDATE, POINTER_FLAGS, POINTER_INFO, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, PEN_MASK_PRESSURE, PEN_MASK_ROTATION, PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_PEN,
    WindowFromPoint,
};

use super::{PenSimReport, PenSimRequest, epoch_ms, error};

/// Hover frames before contact and after lifting, so the pen enters and leaves range like a real one.
const HOVER_FRAMES: u32 = 10;

/// Pen state for contact frame `i` of `n`: pressure steps 1..=1024 (each value once per 1024 frames), tilt sweeps
/// -60..=60 degrees, rotation 0..359.
fn pen_state(i: u32, n: u32) -> (u32, i32, i32, u32) {
    let pressure = 1 + (i % 1024);
    let phase = f64::from(i) / f64::from(n.max(1));
    let tilt_x = (-60.0 + 120.0 * phase).round() as i32;
    let tilt_y = (60.0 - 120.0 * phase).round() as i32;
    (pressure, tilt_x, tilt_y, (i * 7) % 360)
}

/// Screen position for frame `i` of `n` inside the rectangle: a sine wave from left to right.
fn position(rect: (i32, i32, i32, i32), i: u32, n: u32) -> POINT {
    let (left, top, width, height) = rect;
    let phase = f64::from(i) / f64::from(n.max(1));
    let x = f64::from(left) + 8.0 + phase * f64::from(width - 16);
    let y =
        f64::from(top) + f64::from(height) / 2.0 + (phase * std::f64::consts::TAU * 3.0).sin() * f64::from(height / 3);
    POINT { x: x.round() as i32, y: y.round() as i32 }
}

struct Device(HSYNTHETICPOINTERDEVICE);

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateSyntheticPointerDevice and is destroyed exactly once.
        #[allow(unsafe_code)]
        unsafe {
            DestroySyntheticPointerDevice(self.0);
        }
    }
}

fn frame(flags: POINTER_FLAGS, at: POINT, pressure: u32, tilt_x: i32, tilt_y: i32, rotation: u32) -> POINTER_TYPE_INFO {
    POINTER_TYPE_INFO {
        r#type: PT_PEN,
        Anonymous: POINTER_TYPE_INFO_0 {
            penInfo: POINTER_PEN_INFO {
                pointerInfo: POINTER_INFO {
                    pointerType: PT_PEN,
                    pointerFlags: flags,
                    ptPixelLocation: at,
                    ..POINTER_INFO::default()
                },
                penFlags: 0,
                penMask: PEN_MASK_PRESSURE | PEN_MASK_ROTATION | PEN_MASK_TILT_X | PEN_MASK_TILT_Y,
                pressure,
                rotation,
                tiltX: tilt_x,
                tiltY: tilt_y,
            },
        },
    }
}

fn inject(device: &Device, info: &POINTER_TYPE_INFO) -> Result<(), ErrorInfo> {
    // SAFETY: `info` is a fully initialised POINTER_TYPE_INFO for a pen device created by this module.
    #[allow(unsafe_code)]
    unsafe { InjectSyntheticPointerInput(device.0, std::slice::from_ref(info)) }
        .map_err(|e| error("spike.injectFailed", format!("InjectSyntheticPointerInput: {e}")))
}

pub(super) fn run(app: &AppHandle, request: &PenSimRequest) -> Result<PenSimReport, ErrorInfo> {
    let window = app.get_webview_window("main").ok_or_else(|| error("internal", "no main window".into()))?;
    let hwnd: HWND = window.hwnd().map_err(|e| error("internal", e.to_string()))?;
    let _ = window.set_always_on_top(true);
    std::thread::sleep(Duration::from_millis(300));
    let result = inject_stroke(&window, hwnd, request);
    let _ = window.set_always_on_top(false);
    result
}

fn inject_stroke(
    window: &tauri::WebviewWindow,
    hwnd: HWND,
    request: &PenSimRequest,
) -> Result<PenSimReport, ErrorInfo> {
    let scale = window.scale_factor().map_err(|e| error("internal", e.to_string()))?;
    let origin = window.inner_position().map_err(|e| error("internal", e.to_string()))?;
    let size = window.inner_size().map_err(|e| error("internal", e.to_string()))?;
    let px = |v: f64| (v * scale).round() as i32;
    let (w, h) = (i32::try_from(size.width).unwrap_or(i32::MAX), i32::try_from(size.height).unwrap_or(i32::MAX));
    let (left, top) = (px(request.left).min(w), px(request.top).min(h));
    let rect = (origin.x + left, origin.y + top, px(request.width).min(w - left), px(request.height).min(h - top));
    if rect.2 < 10 || rect.3 < 10 {
        return Err(error("arg.invalid", "the target rectangle is outside the window".into()));
    }
    let ours = |at: POINT| {
        // SAFETY: plain Win32 queries with no preconditions.
        #[allow(unsafe_code)]
        let root = unsafe { GetAncestor(WindowFromPoint(at), GA_ROOT) };
        root == hwnd
    };
    let corners = [
        POINT { x: rect.0 + 1, y: rect.1 + 1 },
        POINT { x: rect.0 + rect.2 - 2, y: rect.1 + 1 },
        POINT { x: rect.0 + 1, y: rect.1 + rect.3 - 2 },
        POINT { x: rect.0 + rect.2 - 2, y: rect.1 + rect.3 - 2 },
    ];
    if !corners.iter().all(|&p| ours(p)) {
        let seen: Vec<String> = corners
            .iter()
            .map(|&p| {
                // SAFETY: plain Win32 queries with no preconditions.
                #[allow(unsafe_code)]
                let (w, root) = unsafe { (WindowFromPoint(p), GetAncestor(WindowFromPoint(p), GA_ROOT)) };
                format!("({},{}) -> {:?} root {:?}", p.x, p.y, w.0, root.0)
            })
            .collect();
        return Err(error(
            "spike.notForeground",
            format!(
                "the Toontown canvas must be visible and uncovered to inject pen input (ours {:?}, rect {rect:?}, {})",
                hwnd.0,
                seen.join("; ")
            ),
        ));
    }

    // SAFETY: plain Win32 call; the handle is owned by `Device` and destroyed on drop.
    #[allow(unsafe_code)]
    let device = Device(
        unsafe { CreateSyntheticPointerDevice(PT_PEN, 1, POINTER_FEEDBACK_DEFAULT) }
            .map_err(|e| error("spike.injectFailed", format!("CreateSyntheticPointerDevice: {e}")))?,
    );

    let n = request.frames;
    let period = Duration::from_secs_f64(1.0 / f64::from(request.hz));
    let mut report = PenSimReport {
        method: "InjectSyntheticPointerInput",
        frames: 0,
        hz: request.hz,
        achieved_hz: 0.0,
        inject_epoch_ms: Vec::with_capacity(n as usize),
        pressure: Vec::with_capacity(n as usize),
        tilt_x: Vec::with_capacity(n as usize),
        tilt_y: Vec::with_capacity(n as usize),
        rotation: Vec::with_capacity(n as usize),
    };
    let start_at = position(rect, 0, n);
    let hover = POINTER_FLAG_INRANGE | POINTER_FLAG_UPDATE;
    let mut next = Instant::now();
    // Fixed-rate schedule. After a stall (the thread was descheduled), resume from now instead of injecting the
    // missed frames back to back: a catch-up burst makes the OS coalesce or drop input and would be measured as loss.
    let wait = |next: &mut Instant| {
        *next += period;
        let now = Instant::now();
        if now > *next + period {
            *next = now;
        }
        while Instant::now() < *next {
            std::hint::spin_loop();
        }
    };
    for _ in 0..HOVER_FRAMES {
        inject(&device, &frame(hover, start_at, 0, 0, 0, 0))?;
        wait(&mut next);
    }
    let contact_start = Instant::now();
    let mut last = start_at;
    for i in 0..n {
        let (pressure, tilt_x, tilt_y, rotation) = pen_state(i, n);
        let at = position(rect, i, n);
        if !ours(at) {
            let _ = inject(&device, &frame(POINTER_FLAG_UP, last, 0, 0, 0, 0));
            return Err(error("spike.notForeground", format!("another window covered the canvas after {i} frames")));
        }
        last = at;
        let flags = if i == 0 {
            POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_DOWN
        } else {
            POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_UPDATE
        };
        inject(&device, &frame(flags, last, pressure, tilt_x, tilt_y, rotation))?;
        report.inject_epoch_ms.push(epoch_ms());
        report.pressure.push(pressure);
        report.tilt_x.push(tilt_x);
        report.tilt_y.push(tilt_y);
        report.rotation.push(rotation);
        report.frames += 1;
        wait(&mut next);
    }
    let elapsed = contact_start.elapsed().as_secs_f64();
    inject(&device, &frame(POINTER_FLAG_INRANGE | POINTER_FLAG_UP, last, 0, 0, 0, 0))?;
    for _ in 0..HOVER_FRAMES {
        wait(&mut next);
        inject(&device, &frame(hover, last, 0, 0, 0, 0))?;
    }
    inject(&device, &frame(POINTER_FLAG_UPDATE, last, 0, 0, 0, 0))?;
    report.achieved_hz = if elapsed > 0.0 { f64::from(report.frames) / elapsed } else { 0.0 };
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_steps_through_every_level() {
        let mut seen: Vec<u32> = (0..1024).map(|i| pen_state(i, 1024).0).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 1024);
        assert_eq!((seen[0], seen[1023]), (1, 1024));
    }

    #[test]
    fn tilt_and_rotation_stay_in_range() {
        for i in 0..2000 {
            let (_, tx, ty, r) = pen_state(i, 2000);
            assert!((-60..=60).contains(&tx) && (-60..=60).contains(&ty) && r < 360);
        }
    }

    #[test]
    fn positions_stay_inside_the_rectangle() {
        let rect = (100, 200, 400, 300);
        for i in 0..1000 {
            let p = position(rect, i, 1000);
            assert!(p.x >= 100 && p.x <= 500 && p.y >= 200 && p.y <= 500, "{i}: {p:?}");
        }
    }
}
