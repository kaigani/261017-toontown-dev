//! Synthetic pen injection for measuring the web and native paths without tablet hardware (**spike code**, Windows).
//!
//! Uses `CreateSyntheticPointerDevice(PT_PEN)` + `InjectSyntheticPointerInput` (Windows 10 1809+). This is a
//! **simulation**: it exercises the OS pointer stack, WebView2 and Windows Ink with known values, but it is not
//! evidence about real tablet hardware or drivers.
//!
//! Safety of the user's session: input is injected only inside a rectangle of one of this app's own windows, and only
//! at screen points where the topmost window is that window (`WindowFromPoint` -> root ancestor). The check runs for
//! every frame; the stroke is lifted and aborted as soon as any other window covers the point. Requiring keyboard focus
//! instead does not work for a process started in the background (Windows' focus-stealing protection), and is not
//! needed: pointer input goes to the window under the point. The main window is always-on-top only for the stroke.
//!
//! Timing (C02-R1-01): each contact frame's wall-clock time is taken immediately **before** its inject call (and the
//! return time after it), so event-minus-injection latency is not shortened by the call itself.

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
    GA_ROOT, GUITHREADINFO, GetAncestor, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowThreadProcessId, PEN_MASK_PRESSURE, PEN_MASK_ROTATION, PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_PEN,
    WindowFromPoint,
};

use super::{FocusSnapshot, PenSimReport, PenSimRequest, epoch_ms, error, monotonic_ns};

/// Hover frames before contact and after lifting, so the pen enters and leaves range like a real one.
const HOVER_FRAMES: u32 = 10;

/// Screen rectangle `(left, top, width, height)` in physical pixels.
pub(super) type Rect = (i32, i32, i32, i32);

/// Fixed tilt X / tilt Y / rotation of the `pressureOnly` pattern (non-zero, so fidelity remains checkable).
const PRESSURE_ONLY_POSE: (i32, i32, u32) = (20, -10, 90);

/// Pen state for contact frame `i` of `n`: pressure steps 1..=1024 (each value once per 1024 frames). Except for
/// `pressureOnly`, tilt sweeps -60..=60 degrees and rotation 0..359; `pressureOnly` holds tilt and rotation fixed.
fn pen_state(pattern: &str, i: u32, n: u32) -> (u32, i32, i32, u32) {
    let pressure = 1 + (i % 1024);
    if pattern == "pressureOnly" {
        let (tilt_x, tilt_y, rotation) = PRESSURE_ONLY_POSE;
        return (pressure, tilt_x, tilt_y, rotation);
    }
    let phase = f64::from(i) / f64::from(n.max(1));
    let tilt_x = (-60.0 + 120.0 * phase).round() as i32;
    let tilt_y = (60.0 - 120.0 * phase).round() as i32;
    (pressure, tilt_x, tilt_y, (i * 7) % 360)
}

/// Screen position of contact frame `i` of `n` for a pattern:
/// * `sine`: left to right across the rectangle on a sine wave (adjacent frames can round to the same pixel);
/// * `zigzag`: 2 px per frame, bouncing between the edges, on a 3-row ladder (never the same pixel twice in a row);
/// * `stationary` and `pressureOnly`: the rectangle's centre for every frame.
fn position(rect: Rect, pattern: &str, i: u32, n: u32) -> POINT {
    let (left, top, width, height) = rect;
    match pattern {
        "stationary" | "pressureOnly" => POINT { x: left + width / 2, y: top + height / 2 },
        "zigzag" => {
            let span = (width - 16).max(4);
            let step = (i64::from(i) * 2) % i64::from(2 * span);
            let x = if step < i64::from(span) { step } else { i64::from(2 * span) - step };
            let row = i64::from(i / 7 % 3) - 1;
            let x = i32::try_from(x).unwrap_or(0);
            let y = top + height / 2 + i32::try_from(row * 3).unwrap_or(0);
            POINT { x: left + 8 + x, y }
        }
        _ => {
            let phase = f64::from(i) / f64::from(n.max(1));
            let x = f64::from(left) + 8.0 + phase * f64::from(width - 16);
            let y = f64::from(top)
                + f64::from(height) / 2.0
                + (phase * std::f64::consts::TAU * 3.0).sin() * f64::from(height / 3);
            POINT { x: x.round() as i32, y: y.round() as i32 }
        }
    }
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

/// Whether the topmost window at `at` is `target` (by root ancestor).
pub(super) fn window_at_is(target: HWND, at: POINT) -> bool {
    // SAFETY: plain Win32 queries with no preconditions.
    #[allow(unsafe_code)]
    let root = unsafe { GetAncestor(WindowFromPoint(at), GA_ROOT) };
    root == target
}

fn hwnd_number(h: HWND) -> i64 {
    h.0 as i64
}

fn window_pid_class(h: HWND) -> (u32, String) {
    if h.0.is_null() {
        return (0, String::new());
    }
    let mut pid = 0_u32;
    let mut name = [0_u16; 128];
    // SAFETY: plain Win32 queries on a window handle; the buffers outlive the calls.
    #[allow(unsafe_code)]
    let len = unsafe {
        GetWindowThreadProcessId(h, Some(&mut pid));
        GetClassNameW(h, &mut name)
    };
    let len = usize::try_from(len).unwrap_or(0).min(name.len());
    (pid, String::from_utf16_lossy(&name[..len]))
}

/// Foreground window and the foreground thread's focus/active/capture windows, with owning processes (C02-R2-03).
pub(super) fn focus_snapshot(when: &'static str, target: HWND) -> FocusSnapshot {
    // SAFETY: plain Win32 queries with no preconditions; `info.cbSize` is set as GetGUIThreadInfo requires.
    #[allow(unsafe_code)]
    let (foreground, info) = unsafe {
        let foreground = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO {
            cbSize: u32::try_from(std::mem::size_of::<GUITHREADINFO>()).unwrap_or(0),
            ..GUITHREADINFO::default()
        };
        let ok = GetGUIThreadInfo(thread, &mut info).is_ok();
        (foreground, ok.then_some(info))
    };
    let (foreground_pid, foreground_class) = window_pid_class(foreground);
    let focus = info.map(|i| i.hwndFocus).unwrap_or_default();
    let (focus_pid, focus_class) = window_pid_class(focus);
    FocusSnapshot {
        when,
        own_pid: std::process::id(),
        target_hwnd: hwnd_number(target),
        foreground_hwnd: hwnd_number(foreground),
        foreground_pid,
        foreground_class,
        focus_hwnd: hwnd_number(focus),
        focus_pid,
        focus_class,
        active_hwnd: info.map(|i| hwnd_number(i.hwndActive)).unwrap_or(0),
        capture_hwnd: info.map(|i| hwnd_number(i.hwndCapture)).unwrap_or(0),
    }
}

/// The canvas rectangle of the request (webview CSS pixels) on screen, physical pixels, clamped to the client area.
pub(super) fn canvas_rect(window: &tauri::WebviewWindow, request: &PenSimRequest) -> Result<Rect, ErrorInfo> {
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
    Ok(rect)
}

/// Webview simulation (the `spike_pen_simulate` command): the main window's canvas.
pub(super) fn run(app: &AppHandle, request: &PenSimRequest) -> Result<PenSimReport, ErrorInfo> {
    let window = app.get_webview_window("main").ok_or_else(|| error("internal", "no main window".into()))?;
    let hwnd: HWND = window.hwnd().map_err(|e| error("internal", e.to_string()))?;
    let _ = window.set_always_on_top(true);
    std::thread::sleep(Duration::from_millis(300));
    let result = canvas_rect(&window, request).and_then(|rect| inject_stroke(hwnd, rect, request, "webview"));
    let _ = window.set_always_on_top(false);
    result
}

/// Injects one stroke into `rect`, which must be covered by `target` (checked before every frame).
pub(super) fn inject_stroke(
    target: HWND,
    rect: Rect,
    request: &PenSimRequest,
    target_label: &'static str,
) -> Result<PenSimReport, ErrorInfo> {
    let corners = [
        POINT { x: rect.0 + 1, y: rect.1 + 1 },
        POINT { x: rect.0 + rect.2 - 2, y: rect.1 + 1 },
        POINT { x: rect.0 + 1, y: rect.1 + rect.3 - 2 },
        POINT { x: rect.0 + rect.2 - 2, y: rect.1 + rect.3 - 2 },
    ];
    if !corners.iter().all(|&p| window_at_is(target, p)) {
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
                "the target must be visible and uncovered to inject pen input (target {:?}, rect {rect:?}, {})",
                target.0,
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
    let pattern = request.pattern.clone().unwrap_or_else(|| "sine".to_owned());
    let period = Duration::from_secs_f64(1.0 / f64::from(request.hz));
    let cap = n as usize;
    let mut report = PenSimReport {
        method: "InjectSyntheticPointerInput",
        frames: 0,
        hz: request.hz,
        achieved_hz: 0.0,
        contact_start_ns: 0,
        contact_end_ns: 0,
        contact_start_epoch_ms: 0.0,
        contact_end_epoch_ms: 0.0,
        focus: vec![focus_snapshot("start", target)],
        inject_epoch_ms: Vec::with_capacity(cap),
        inject_return_epoch_ms: Vec::with_capacity(cap),
        screen_x: Vec::with_capacity(cap),
        screen_y: Vec::with_capacity(cap),
        screen_rect: [rect.0, rect.1, rect.2, rect.3],
        pattern: pattern.clone(),
        target: target_label,
        pressure: Vec::with_capacity(cap),
        tilt_x: Vec::with_capacity(cap),
        tilt_y: Vec::with_capacity(cap),
        rotation: Vec::with_capacity(cap),
    };
    let start_at = position(rect, &pattern, 0, n);
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
    report.contact_start_ns = monotonic_ns();
    report.contact_start_epoch_ms = epoch_ms();
    let mut last = start_at;
    for i in 0..n {
        let (pressure, tilt_x, tilt_y, rotation) = pen_state(&pattern, i, n);
        let at = position(rect, &pattern, i, n);
        if !window_at_is(target, at) {
            let _ = inject(&device, &frame(POINTER_FLAG_UP, last, 0, 0, 0, 0));
            return Err(error("spike.notForeground", format!("another window covered the target after {i} frames")));
        }
        last = at;
        let flags = if i == 0 {
            POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_DOWN
        } else {
            POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_UPDATE
        };
        let before = epoch_ms();
        inject(&device, &frame(flags, at, pressure, tilt_x, tilt_y, rotation))?;
        report.inject_epoch_ms.push(before);
        report.inject_return_epoch_ms.push(epoch_ms());
        report.screen_x.push(at.x);
        report.screen_y.push(at.y);
        report.pressure.push(pressure);
        report.tilt_x.push(tilt_x);
        report.tilt_y.push(tilt_y);
        report.rotation.push(rotation);
        report.frames += 1;
        wait(&mut next);
    }
    report.contact_end_ns = monotonic_ns();
    report.contact_end_epoch_ms = epoch_ms();
    inject(&device, &frame(POINTER_FLAG_INRANGE | POINTER_FLAG_UP, last, 0, 0, 0, 0))?;
    for _ in 0..HOVER_FRAMES {
        wait(&mut next);
        inject(&device, &frame(hover, last, 0, 0, 0, 0))?;
    }
    inject(&device, &frame(POINTER_FLAG_UPDATE, last, 0, 0, 0, 0))?;
    let elapsed_ns = report.contact_end_ns.saturating_sub(report.contact_start_ns);
    // Exactly reproducible from the logged endpoints (C02-R2-02).
    report.achieved_hz = if elapsed_ns > 0 { f64::from(report.frames) * 1e9 / elapsed_ns as f64 } else { 0.0 };
    report.focus.push(focus_snapshot("end", target));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_steps_through_every_level() {
        let mut seen: Vec<u32> = (0..1024).map(|i| pen_state("sine", i, 1024).0).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 1024);
        assert_eq!((seen[0], seen[1023]), (1, 1024));
    }

    #[test]
    fn tilt_and_rotation_stay_in_range() {
        for i in 0..2000 {
            let (_, tx, ty, r) = pen_state("sine", i, 2000);
            assert!((-60..=60).contains(&tx) && (-60..=60).contains(&ty) && r < 360);
        }
    }

    #[test]
    fn pressure_only_holds_tilt_and_rotation_and_position() {
        let rect = (8, 58, 1012, 361);
        for i in 0..1024 {
            let (p, tx, ty, r) = pen_state("pressureOnly", i, 1024);
            assert_eq!((p, tx, ty, r), (1 + i, 20, -10, 90));
            assert_eq!(position(rect, "pressureOnly", i, 1024), position(rect, "pressureOnly", 0, 1024));
        }
        // `stationary` keeps the position but still varies tilt and rotation.
        assert_ne!(pen_state("stationary", 0, 1024).1, pen_state("stationary", 1000, 1024).1);
    }

    #[test]
    fn positions_stay_inside_the_rectangle() {
        let rect = (100, 200, 400, 300);
        for pattern in ["sine", "zigzag", "stationary", "pressureOnly"] {
            for i in 0..1000 {
                let p = position(rect, pattern, i, 1000);
                assert!(p.x >= 100 && p.x <= 500 && p.y >= 200 && p.y <= 500, "{pattern} {i}: {p:?}");
            }
        }
    }

    #[test]
    fn zigzag_never_repeats_a_pixel_in_adjacent_frames_and_stationary_always_does() {
        let rect = (8, 58, 1012, 361);
        for i in 1..1024 {
            let (a, b) = (position(rect, "zigzag", i - 1, 1024), position(rect, "zigzag", i, 1024));
            assert!(a != b, "zigzag frame {i} repeats {a:?}");
            assert_eq!(position(rect, "stationary", i, 1024), position(rect, "stationary", 0, 1024));
        }
    }

    #[test]
    fn sine_repeats_pixels_where_the_review_found_missing_frames() {
        // Reviewer's reconstruction of the runner canvas (C0.2 r1): adjacent identical pixels at frames 93, 422,
        // 605 and 934 (1-based; the later frame of each pair) — the frames missing in every run.
        let rect = (8, 58, 1012, 361);
        let repeats: Vec<u32> =
            (1..1024).filter(|&i| position(rect, "sine", i - 1, 1024) == position(rect, "sine", i, 1024)).collect();
        assert_eq!(repeats.iter().map(|i| i + 1).collect::<Vec<_>>(), vec![93, 422, 605, 934]);
    }
}
