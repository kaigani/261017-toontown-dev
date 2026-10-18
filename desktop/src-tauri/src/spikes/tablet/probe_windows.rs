//! Windows listener probe for review C02-R1-04 (**spike code**): which layers see the synthetic pen at all?
//!
//! The same injector ([`super::simulate_windows::inject_stroke`]) is aimed at a plain same-process native window
//! (a topmost popup laid over the canvas) that records, together:
//! * `WM_POINTER*` messages with the full pen history of each (`GetPointerPenInfoHistory`);
//! * an `octotablet` RealTimeStylus attached to that window (the same manager the webview listener uses);
//! * raw input for the HID digitizer usage page 0x0D (`WM_INPUT`, `RIDEV_INPUTSINK`).
//!
//! For the webview half of the matrix, [`raw_sink_start`]/[`raw_sink_stop`] run the raw-input listener on a hidden
//! window while the ordinary webview simulation runs (with the RTS listener on the main window). `WM_POINTER` of the
//! webview is not observable from this process: WebView2 renders and receives input in a child window owned by the
//! `msedgewebview2` process.
//!
//! The probe reports counts and raw values only; it draws no conclusion about real tablets.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use octotablet::events::{Event, ToolEvent};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use serde::Serialize;
use tauri::{AppHandle, Manager as _};
use tt_engine::ErrorInfo;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::Pointer::{GetPointerPenInfoHistory, POINTER_FLAG_INCONTACT, POINTER_PEN_INFO};
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, HRAWINPUT, RAWHID, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT,
    RIDEV_INPUTSINK, RIDEV_PAGEONLY, RIDEV_REMOVE, RIDI_DEVICENAME, RIM_TYPEHID, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, MsgWaitForMultipleObjects, PM_REMOVE,
    PeekMessageW, QS_ALLINPUT, RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
    WM_INPUT, WM_POINTERDOWN, WM_POINTERENTER, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE, WNDCLASSW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::w;

use super::simulate_windows::{Rect, canvas_rect, inject_stroke};
use super::{PenSimRequest, error};

/// Entries kept per list (the counters keep counting past it).
const MAX_ENTRIES: usize = 20_000;
/// HID digitizer usage page.
const DIGITIZER_PAGE: u16 = 0x0D;

/// One pen-history entry seen through `WM_POINTER*`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerEntry {
    /// Message that carried it: `down`, `update`, `up`.
    message: &'static str,
    /// Index of this entry in its message's history (0 = the message's own, newest).
    history_index: u32,
    pointer_id: u32,
    frame_id: u32,
    x: i32,
    y: i32,
    pressure: u32,
    tilt_x: i32,
    tilt_y: i32,
    rotation: u32,
    in_contact: bool,
}

/// What `WM_POINTER*` delivered to the probe window.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerCounts {
    down: u64,
    update: u64,
    up: u64,
    enter: u64,
    leave: u64,
    /// Sum of history lengths over all down/update/up messages.
    history_entries: u64,
    /// Messages whose history could not be read.
    history_errors: u64,
    /// Entries in chronological order (oldest first), up to `MAX_ENTRIES`.
    entries: Vec<PointerEntry>,
    truncated: u64,
}

/// What the RealTimeStylus (`octotablet`) on the probe window delivered.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RtsCounts {
    /// `None` when the manager could not be built (see `error`).
    tablets: Option<Vec<String>>,
    error: Option<String>,
    poses: u64,
    downs: u64,
    ups: u64,
    frames: u64,
    proximity_in: u64,
    proximity_out: u64,
    /// Normalised pressure of each pose (0..1), up to `MAX_ENTRIES`.
    pressures: Vec<f32>,
    truncated: u64,
}

/// What raw input (HID digitizer page) delivered.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawCounts {
    registered: bool,
    error: Option<String>,
    /// `WM_INPUT` messages.
    messages: u64,
    /// HID reports (a message can carry several).
    hid_reports: u64,
    /// Non-HID raw input (should stay 0: only the digitizer page is registered).
    other: u64,
    /// Distinct `hDevice` values.
    devices: u64,
    report_bytes: u64,
    /// Device interface names of the distinct devices (`RIDI_DEVICENAME`).
    device_names: Vec<String>,
    /// Each HID report as hex, in arrival order, up to `MAX_ENTRIES` (for offline decoding).
    reports: Vec<String>,
    truncated: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Listeners {
    wm_pointer: Option<PointerCounts>,
    rts: Option<RtsCounts>,
    raw_input: Option<RawCounts>,
}

#[derive(Default)]
struct ThreadState {
    pointer: PointerCounts,
    raw: RawCounts,
    raw_devices: BTreeSet<isize>,
}

thread_local! {
    static STATE: RefCell<ThreadState> = RefCell::new(ThreadState::default());
}

/// Window handle for `octotablet::Builder::build_raw`.
struct Handle(NonZeroIsize);

impl HasWindowHandle for Handle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
        // SAFETY: the window outlives the manager built from this handle (the manager is dropped before
        // `DestroyWindow` in `listen`).
        #[allow(unsafe_code)]
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for Handle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

fn message_name(msg: u32) -> &'static str {
    match msg {
        WM_POINTERDOWN => "down",
        WM_POINTERUP => "up",
        _ => "update",
    }
}

fn record_pointer(state: &mut ThreadState, msg: u32, wparam: WPARAM) {
    let counts = &mut state.pointer;
    match msg {
        WM_POINTERDOWN => counts.down += 1,
        WM_POINTERUP => counts.up += 1,
        WM_POINTERUPDATE => counts.update += 1,
        WM_POINTERENTER => {
            counts.enter += 1;
            return;
        }
        _ => {
            counts.leave += 1;
            return;
        }
    }
    let pointer_id = (wparam.0 & 0xFFFF) as u32;
    let mut len = 0_u32;
    // SAFETY: querying the length only (no buffer).
    #[allow(unsafe_code)]
    let sized = unsafe { GetPointerPenInfoHistory(pointer_id, &mut len, None) };
    if sized.is_err() || len == 0 {
        counts.history_errors += 1;
        return;
    }
    let mut history = vec![POINTER_PEN_INFO::default(); len as usize];
    // SAFETY: `history` has room for `len` entries, as the call requires.
    #[allow(unsafe_code)]
    let read = unsafe { GetPointerPenInfoHistory(pointer_id, &mut len, Some(history.as_mut_ptr())) };
    if read.is_err() {
        counts.history_errors += 1;
        return;
    }
    history.truncate(len as usize);
    counts.history_entries += u64::from(len);
    // The history is newest first; record oldest first.
    for (index, info) in history.iter().enumerate().rev() {
        if counts.entries.len() >= MAX_ENTRIES {
            counts.truncated += 1;
            continue;
        }
        let p = &info.pointerInfo;
        counts.entries.push(PointerEntry {
            message: message_name(msg),
            history_index: u32::try_from(index).unwrap_or(u32::MAX),
            pointer_id: p.pointerId,
            frame_id: p.frameId,
            x: p.ptPixelLocation.x,
            y: p.ptPixelLocation.y,
            pressure: info.pressure,
            tilt_x: info.tiltX,
            tilt_y: info.tiltY,
            rotation: info.rotation,
            in_contact: p.pointerFlags.contains(POINTER_FLAG_INCONTACT),
        });
    }
}

fn record_raw(state: &mut ThreadState, lparam: LPARAM) {
    state.raw.messages += 1;
    let handle = HRAWINPUT(lparam.0 as *mut core::ffi::c_void);
    let header = u32::try_from(std::mem::size_of::<RAWINPUTHEADER>()).unwrap_or(u32::MAX);
    let mut size = 0_u32;
    // SAFETY: size query (no buffer).
    #[allow(unsafe_code)]
    let queried = unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header) };
    if queried != 0 || size == 0 {
        return;
    }
    // u64 storage keeps the RAWINPUT header aligned.
    let mut buffer = vec![0_u64; (size as usize).div_ceil(8)];
    // SAFETY: `buffer` holds at least `size` bytes, as reported by the size query.
    #[allow(unsafe_code)]
    let copied = unsafe { GetRawInputData(handle, RID_INPUT, Some(buffer.as_mut_ptr().cast()), &mut size, header) };
    if copied == u32::MAX || (copied as usize) < std::mem::size_of::<RAWINPUTHEADER>() {
        return;
    }
    // SAFETY: the buffer is 8-byte aligned and starts with a RAWINPUTHEADER written by GetRawInputData.
    #[allow(unsafe_code)]
    let input = unsafe { &*buffer.as_ptr().cast::<RAWINPUT>() };
    if state.raw_devices.insert(input.header.hDevice.0 as isize) {
        state.raw.device_names.push(device_name(input.header.hDevice));
    }
    state.raw.devices = state.raw_devices.len() as u64;
    if input.header.dwType == RIM_TYPEHID.0 {
        // SAFETY: dwType says the union holds a RAWHID.
        #[allow(unsafe_code)]
        let hid = unsafe { input.data.hid };
        state.raw.hid_reports += u64::from(hid.dwCount);
        state.raw.report_bytes += u64::from(hid.dwCount) * u64::from(hid.dwSizeHid);
        // The reports follow the RAWHID counters inside the copied buffer.
        let base = buffer.as_ptr().cast::<u8>();
        let start = std::mem::offset_of!(RAWINPUT, data) + std::mem::offset_of!(RAWHID, bRawData);
        // SAFETY: `copied` bytes of `buffer` were written by GetRawInputData; the slice stays inside them.
        #[allow(unsafe_code)]
        let bytes = unsafe { std::slice::from_raw_parts(base, copied as usize) };
        let size = hid.dwSizeHid as usize;
        for i in 0..hid.dwCount as usize {
            let Some(report) = bytes.get(start + i * size..start + (i + 1) * size) else { break };
            if state.raw.reports.len() < MAX_ENTRIES {
                state.raw.reports.push(report.iter().map(|b| format!("{b:02x}")).collect());
            } else {
                state.raw.truncated += 1;
            }
        }
    } else {
        state.raw.other += 1;
    }
}

fn device_name(device: HANDLE) -> String {
    let mut chars = 0_u32;
    // SAFETY: size query (no buffer) for a device handle reported by WM_INPUT.
    #[allow(unsafe_code)]
    unsafe {
        GetRawInputDeviceInfoW(Some(device), RIDI_DEVICENAME, None, &mut chars);
    }
    if chars == 0 {
        return String::new();
    }
    let mut name = vec![0_u16; chars as usize];
    // SAFETY: `name` holds `chars` UTF-16 units, as the size query reported.
    #[allow(unsafe_code)]
    let written =
        unsafe { GetRawInputDeviceInfoW(Some(device), RIDI_DEVICENAME, Some(name.as_mut_ptr().cast()), &mut chars) };
    if written == u32::MAX {
        return String::new();
    }
    String::from_utf16_lossy(&name).trim_end_matches(char::from(0)).to_owned()
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERENTER | WM_POINTERLEAVE => {
            STATE.with_borrow_mut(|s| record_pointer(s, msg, wparam));
        }
        WM_INPUT => STATE.with_borrow_mut(|s| record_raw(s, lparam)),
        _ => {}
    }
    // SAFETY: default processing for our own window (also required after WM_INPUT for cleanup).
    #[allow(unsafe_code)]
    unsafe {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn register_class() -> Result<(), String> {
    static REGISTERED: Mutex<bool> = Mutex::new(false);
    let mut done = REGISTERED.lock().map_err(|_| "class lock poisoned".to_owned())?;
    if *done {
        return Ok(());
    }
    // SAFETY: plain Win32 registration of a class whose window procedure is `window_proc`.
    #[allow(unsafe_code)]
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut core::ffi::c_void),
            lpszClassName: w!("ToontownTabletProbe"),
            ..WNDCLASSW::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(format!("RegisterClassW: {}", windows::core::Error::from_thread()));
        }
    }
    *done = true;
    Ok(())
}

fn register_raw(hwnd: HWND, on: bool) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        usUsagePage: DIGITIZER_PAGE,
        usUsage: 0,
        dwFlags: if on { RIDEV_INPUTSINK | RIDEV_PAGEONLY } else { RIDEV_REMOVE | RIDEV_PAGEONLY },
        hwndTarget: if on { hwnd } else { HWND::default() },
    };
    let size = u32::try_from(std::mem::size_of::<RAWINPUTDEVICE>()).unwrap_or(u32::MAX);
    // SAFETY: one fully initialised RAWINPUTDEVICE.
    #[allow(unsafe_code)]
    unsafe { RegisterRawInputDevices(&[device], size) }.map_err(|e| format!("RegisterRawInputDevices: {e}"))
}

/// Which listeners a probe thread runs.
#[derive(Clone, Copy)]
struct Config {
    /// Visible topmost window over this rectangle; `None` = hidden window (raw input only).
    rect: Option<Rect>,
    rts: bool,
}

/// A listener window on its own thread.
struct ProbeThread {
    hwnd: isize,
    stop: Arc<AtomicBool>,
    join: std::thread::JoinHandle<Listeners>,
}

impl ProbeThread {
    fn spawn(config: Config) -> Result<Self, ErrorInfo> {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<Result<isize, String>>();
        let flag = Arc::clone(&stop);
        let join = std::thread::spawn(move || listen(config, &flag, &tx));
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(hwnd)) => Ok(Self { hwnd, stop, join }),
            Ok(Err(message)) => {
                let _ = join.join();
                Err(error("spike.probeFailed", message))
            }
            Err(e) => Err(error("spike.probeFailed", format!("probe window did not start: {e}"))),
        }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut core::ffi::c_void)
    }

    fn finish(self) -> Result<Listeners, ErrorInfo> {
        self.stop.store(true, Ordering::SeqCst);
        self.join.join().map_err(|_| error("spike.probeFailed", "probe thread panicked".into()))
    }
}

fn listen(config: Config, stop: &AtomicBool, ready: &mpsc::Sender<Result<isize, String>>) -> Listeners {
    let mut out = Listeners::default();
    // SAFETY: this thread's own COM apartment, uninitialised below (RealTimeStylus is an STA COM object).
    #[allow(unsafe_code)]
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let hwnd = match register_class().and_then(|()| create_window(config.rect)) {
        Ok(hwnd) => hwnd,
        Err(message) => {
            let _ = ready.send(Err(message));
            return out;
        }
    };
    let raw = register_raw(hwnd, true);
    let mut rts = RtsCounts::default();
    let mut manager = if config.rts {
        match NonZeroIsize::new(hwnd.0 as isize) {
            Some(raw_hwnd) => {
                // SAFETY: the window stays valid while the manager lives (dropped before DestroyWindow below).
                #[allow(unsafe_code)]
                let built =
                    unsafe { octotablet::Builder::new().emulate_tool_from_mouse(false).build_raw(Handle(raw_hwnd)) };
                match built {
                    Ok(m) => {
                        rts.tablets = Some(
                            m.tablets().iter().map(|t| t.name.clone().unwrap_or_else(|| "unnamed".into())).collect(),
                        );
                        Some(m)
                    }
                    Err(e) => {
                        rts.error = Some(format!("octotablet: {e}"));
                        None
                    }
                }
            }
            None => None,
        }
    } else {
        None
    };
    let _ = ready.send(Ok(hwnd.0 as isize));

    let mut msg = MSG::default();
    while !stop.load(Ordering::SeqCst) {
        // SAFETY: standard message pump for windows created on this thread.
        #[allow(unsafe_code)]
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if let Some(manager) = manager.as_mut() {
            let Ok(events) = manager.pump();
            for event in events {
                if let Event::Tool { event, .. } = event {
                    match event {
                        ToolEvent::Pose(pose) => {
                            rts.poses += 1;
                            if rts.pressures.len() < MAX_ENTRIES {
                                rts.pressures.push(pose.pressure.get().unwrap_or(-1.0));
                            } else {
                                rts.truncated += 1;
                            }
                        }
                        ToolEvent::Down => rts.downs += 1,
                        ToolEvent::Up => rts.ups += 1,
                        ToolEvent::Frame(_) => rts.frames += 1,
                        ToolEvent::In { .. } => rts.proximity_in += 1,
                        ToolEvent::Out => rts.proximity_out += 1,
                        _ => {}
                    }
                }
            }
        }
        // SAFETY: wait for input or 2 ms, whichever comes first.
        #[allow(unsafe_code)]
        unsafe {
            MsgWaitForMultipleObjects(None::<&[HANDLE]>, false, 2, QS_ALLINPUT);
        }
    }

    drop(manager);
    let _ = register_raw(hwnd, false);
    // SAFETY: our own window, destroyed once on its thread.
    #[allow(unsafe_code)]
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    if com.is_ok() {
        // SAFETY: balances the successful CoInitializeEx above.
        #[allow(unsafe_code)]
        unsafe {
            CoUninitialize();
        }
    }
    STATE.with_borrow_mut(|s| {
        let mut raw_counts = std::mem::take(&mut s.raw);
        raw_counts.registered = raw.is_ok();
        raw_counts.error = raw.err();
        out.raw_input = Some(raw_counts);
        if config.rect.is_some() {
            out.wm_pointer = Some(std::mem::take(&mut s.pointer));
        }
        s.raw_devices.clear();
    });
    if config.rts {
        out.rts = Some(rts);
    }
    out
}

fn create_window(rect: Option<Rect>) -> Result<HWND, String> {
    let (x, y, width, height) = rect.unwrap_or((0, 0, 1, 1));
    let ex_style =
        if rect.is_some() { WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE } else { WINDOW_EX_STYLE(0) };
    // SAFETY: creating a window of the class registered by `register_class` on this thread.
    #[allow(unsafe_code)]
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;
        let hwnd = CreateWindowExW(
            ex_style,
            w!("ToontownTabletProbe"),
            w!("Toontown tablet probe"),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowExW: {e}"))?;
        if rect.is_some() {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(hwnd)
    }
}

/// The native-window half of the C02-R1-04 matrix: a topmost probe window over the canvas rectangle, the same
/// synthetic stroke, and all three listeners on that window.
pub(super) fn native_window_run(
    app: &AppHandle,
    request: &PenSimRequest,
    rts: bool,
) -> Result<serde_json::Value, ErrorInfo> {
    let window = app.get_webview_window("main").ok_or_else(|| error("internal", "no main window".into()))?;
    let rect = canvas_rect(&window, request)?;
    let probe = ProbeThread::spawn(Config { rect: Some(rect), rts })?;
    std::thread::sleep(Duration::from_millis(300));
    let injected = inject_stroke(probe.hwnd(), rect, request, "nativeWindow");
    std::thread::sleep(Duration::from_millis(500));
    let listeners = probe.finish()?;
    let (report, failure) = match injected {
        Ok(report) => (Some(report), None),
        Err(e) => (None, Some(e)),
    };
    serde_json::to_value(serde_json::json!({
        "target": "nativeWindow",
        "rts": rts,
        "report": report,
        "error": failure,
        "listeners": listeners,
    }))
    .map_err(|e| error("internal", e.to_string()))
}

static SINK: Mutex<Option<ProbeThread>> = Mutex::new(None);

/// Starts the raw-input listener on a hidden window (the webview half of the matrix).
pub(super) fn raw_sink_start() -> Result<serde_json::Value, ErrorInfo> {
    let mut sink = SINK.lock().map_err(|_| error("internal", "sink lock poisoned".into()))?;
    if let Some(previous) = sink.take() {
        let _ = previous.finish();
    }
    *sink = Some(ProbeThread::spawn(Config { rect: None, rts: false })?);
    Ok(serde_json::json!({ "started": true }))
}

/// Stops the raw-input listener and returns what it received (`null` listeners if it was not running).
pub(super) fn raw_sink_stop() -> Result<serde_json::Value, ErrorInfo> {
    let running = SINK.lock().map_err(|_| error("internal", "sink lock poisoned".into()))?.take();
    let listeners = match running {
        Some(probe) => Some(probe.finish()?),
        None => None,
    };
    serde_json::to_value(serde_json::json!({ "listeners": listeners })).map_err(|e| error("internal", e.to_string()))
}
