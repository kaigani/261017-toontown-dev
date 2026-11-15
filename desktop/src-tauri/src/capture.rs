//! The window's own pixels (M4 C4.2b; design note 5 §2, ruling D-6): the platform webview's snapshot, as PNG bytes.
//! - Windows: `ICoreWebView2::CapturePreview` into a memory stream;
//! - macOS: `WKWebView takeSnapshotWithConfiguration:completionHandler:`, its image encoded as PNG.
//!
//! Both run on the main thread (the webview's own), dispatched by [`tauri::WebviewWindow::with_webview`], and answer
//! through a channel the caller waits on within its budget; the caller is never the main thread (an executor caller's
//! thread or an MCP worker). The platform calls are foreign-function calls, which Rust marks `unsafe`; this file is
//! the only place in the host that makes them for a product feature.
#![allow(unsafe_code, reason = "the webview snapshot APIs are foreign (COM, Objective-C) calls (ruling D-6)")]

use std::sync::mpsc;
use std::time::Duration;

/// The PNG of what `webview` shows, within `budget`.
///
/// # Errors
/// A message when the platform refuses, the capture fails, or the budget passes.
pub fn snapshot(webview: &tauri::WebviewWindow, budget: Duration) -> Result<Vec<u8>, String> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, String>>();
    let fail = tx.clone();
    webview
        .with_webview(move |pw| {
            if let Err(e) = platform::start(&pw, tx) {
                let _ = fail.send(Err(e));
            }
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(budget).map_err(|_| format!("no capture within {} ms", budget.as_millis()))?
}

#[cfg(windows)]
mod platform {
    use std::sync::mpsc::Sender;

    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};

    fn read(stream: &IStream) -> windows::core::Result<Vec<u8>> {
        // SAFETY: `stream` is a live COM stream owned by this capture; the buffer is sized from its own Stat.
        unsafe {
            stream.Seek(0, STREAM_SEEK_SET, None)?;
            let mut stat = STATSTG::default();
            stream.Stat(&mut stat, STATFLAG_NONAME)?;
            let n = usize::try_from(stat.cbSize).unwrap_or(0);
            let mut buf = vec![0u8; n];
            let mut got = 0u32;
            stream.Read(buf.as_mut_ptr().cast(), u32::try_from(n).unwrap_or(u32::MAX), Some(&mut got)).ok()?;
            buf.truncate(got as usize);
            Ok(buf)
        }
    }

    pub fn start(pw: &tauri::webview::PlatformWebview, tx: Sender<Result<Vec<u8>, String>>) -> Result<(), String> {
        let controller = pw.controller();
        // SAFETY: COM calls on the webview's own thread (with_webview runs this on the main thread); the stream and
        // the handler are reference-counted COM objects kept alive by the call until the handler runs.
        unsafe {
            let core = controller.CoreWebView2().map_err(|e| e.to_string())?;
            let stream: IStream = CreateStreamOnHGlobal(HGLOBAL::default(), true).map_err(|e| e.to_string())?;
            let own = stream.clone();
            let handler = CapturePreviewCompletedHandler::create(Box::new(move |done: windows::core::Result<()>| {
                let _ = tx.send(done.and_then(|()| read(&own)).map_err(|e| e.to_string()));
                Ok(())
            }));
            core.CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, &stream, &handler)
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::Mutex;
    use std::sync::mpsc::Sender;

    use block2::RcBlock;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError};
    use objc2_web_kit::WKWebView;

    fn png(image: &NSImage) -> Option<Vec<u8>> {
        let tiff = image.TIFFRepresentation()?;
        let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
        // SAFETY: an empty properties dictionary of the declared key and value types.
        let data = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
        Some(data.to_vec())
    }

    pub fn start(pw: &tauri::webview::PlatformWebview, tx: Sender<Result<Vec<u8>, String>>) -> Result<(), String> {
        let raw = pw.inner().cast::<WKWebView>();
        if raw.is_null() {
            return Err("no WKWebView".into());
        }
        // The completion handler may be called once; the sender is taken on the first call.
        let tx = Mutex::new(Some(tx));
        let block = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
            // SAFETY: WebKit passes a valid image or null for the duration of the call.
            let result = unsafe { image.as_ref() }.and_then(png).ok_or_else(|| "the snapshot failed".to_owned());
            if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(result);
            }
        });
        // SAFETY: `raw` is the window's live WKWebView (wry's), used on the main thread where with_webview runs this.
        unsafe { (*raw).takeSnapshotWithConfiguration_completionHandler(None, &block) };
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use std::sync::mpsc::Sender;

    pub fn start(_pw: &tauri::webview::PlatformWebview, _tx: Sender<Result<Vec<u8>, String>>) -> Result<(), String> {
        Err("window captures are implemented on Windows and macOS".into())
    }
}
