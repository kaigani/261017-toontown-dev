//! The window's own pixels (M4 C4.2b; design note 5 §2, ruling D-6): the platform webview's snapshot, as PNG bytes.
//! - Windows: `ICoreWebView2::CapturePreview` into a memory stream;
//! - macOS: `WKWebView takeSnapshotWithConfiguration:completionHandler:`, its image encoded as PNG.
//!
//! Both run on the main thread (the webview's own), dispatched by [`tauri::WebviewWindow::with_webview`], and answer
//! through a channel the caller waits on within its budget; the caller is never the main thread (an executor caller's
//! thread or an MCP worker). The platform calls are foreign-function calls, which Rust marks `unsafe`; this file is
//! the only place in the host that makes them for a product feature.
//!
//! The webview's own menus and keys ([`harden`], Windows; the requirement is D-70 item 2): the default context menu is
//! shown only on editable content, with its text-editing items only; browser accelerator keys (F5, Ctrl+R, Ctrl+P,
//! Ctrl+F, zoom, F12) and autofill are off. The page's own rules (`useNoBrowserMenus`, the key dispatcher) stay in
//! front. The two `unsafe` blocks this adds (the menu filter and the settings) are authorised by ruling D-72 item 4 on
//! D-59 item 3's conditions: a SAFETY comment on each block, nothing borrowed outliving the call (the event handler
//! owns no borrowed data; the webview holds it by COM reference count), and any failure degrading to the unfiltered
//! menu and default settings without a crash (every COM result is checked or ignored, nothing unwraps).
#![allow(
    unsafe_code,
    reason = "the webview snapshot, menu and settings APIs are foreign (COM, Objective-C) calls (rulings D-6, D-72)"
)]

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

/// D-70 item 2: the product's webview shows no browser chrome. On Windows the default context menu opens only on
/// editable content, keeping its text-editing items (undo, redo, cut, copy, paste, paste as plain text, select all,
/// emoji, writing direction) and nothing else (no Back, Reload, Save as, Print, Inspect, passwords or more tools), and
/// the browser's accelerator keys and autofill are off. Elsewhere the page's rules alone apply. Called once for each
/// webview window when it is created; a failure leaves the page's rules in place.
pub fn harden(webview: &tauri::WebviewWindow) {
    let _ = webview.with_webview(|pw| {
        platform::harden(&pw);
    });
}

/// The default context menu items D-70 item 2 keeps on editable content (WebView2's item names).
pub const TEXT_MENU_ITEMS: &[&str] =
    &["undo", "redo", "cut", "copy", "paste", "pasteAndMatchStyle", "selectAll", "emoji", "writingDirection"];

/// Which items of a default context menu stay, by name (`None` for a separator): the text-editing items, and the
/// separators between kept groups only (no leading, trailing or doubled separator).
#[must_use]
pub fn kept_items(names: &[Option<&str>]) -> Vec<bool> {
    let mut keep: Vec<bool> = names.iter().map(|n| n.is_some_and(|n| TEXT_MENU_ITEMS.contains(&n))).collect();
    let mut last_item: Option<usize> = None;
    for (i, n) in names.iter().enumerate() {
        match n {
            None => {
                // A separator stays when an item was kept before it since the previous kept separator and one is kept
                // after it.
                let before = last_item.is_some();
                let after = (i + 1..names.len()).any(|j| names[j].is_some() && keep[j]);
                keep[i] = before && after;
                if keep[i] {
                    last_item = None;
                }
            }
            Some(_) if keep[i] => last_item = Some(i),
            Some(_) => {}
        }
    }
    keep
}

#[cfg(windows)]
mod platform {
    use std::sync::mpsc::Sender;

    use webview2_com::ContextMenuRequestedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR, ICoreWebView2_11,
        ICoreWebView2ContextMenuRequestedEventArgs, ICoreWebView2Settings3, ICoreWebView2Settings4,
    };
    use windows::core::Interface;

    /// The menu of one request: the target editable or not; on editable content, the text-editing items only.
    fn filter(args: &ICoreWebView2ContextMenuRequestedEventArgs) -> windows::core::Result<()> {
        // SAFETY: COM calls on the webview's own thread, inside its ContextMenuRequested event; every out-parameter is
        // a local the call fills, and the names are CoTaskMem strings taken (and freed) by `take_pwstr`.
        unsafe {
            let target = args.ContextMenuTarget()?;
            let mut editable = windows::core::BOOL::default();
            target.IsEditable(&mut editable)?;
            if !editable.as_bool() {
                // Not text entry: no default menu at all (the page has shown its own menu, or none).
                return args.SetHandled(true);
            }
            let items = args.MenuItems()?;
            let mut count = 0u32;
            items.Count(&mut count)?;
            let mut names: Vec<Option<String>> = Vec::new();
            for i in 0..count {
                let item = items.GetValueAtIndex(i)?;
                let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
                item.Kind(&mut kind)?;
                if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                    names.push(None);
                } else {
                    let mut name = windows::core::PWSTR::null();
                    item.Name(&mut name)?;
                    names.push(Some(webview2_com::take_pwstr(name)));
                }
            }
            let borrowed: Vec<Option<&str>> = names.iter().map(|n| n.as_deref()).collect();
            let keep = super::kept_items(&borrowed);
            // From the end, so the indices of the items still to visit do not move.
            for (i, k) in keep.iter().enumerate().rev() {
                if !k {
                    items.RemoveValueAtIndex(u32::try_from(i).unwrap_or(u32::MAX))?;
                }
            }
            Ok(())
        }
    }

    pub fn harden(pw: &tauri::webview::PlatformWebview) {
        // SAFETY: COM calls on the webview's own thread (with_webview runs this on the main thread); the handler is a
        // reference-counted COM object the webview keeps for its life.
        unsafe {
            let Ok(core) = pw.controller().CoreWebView2() else { return };
            if let Ok(settings) = core.Settings() {
                if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                    let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
                }
                if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
                    let _ = s4.SetIsPasswordAutosaveEnabled(false);
                    let _ = s4.SetIsGeneralAutofillEnabled(false);
                }
            }
            if let Ok(core11) = core.cast::<ICoreWebView2_11>() {
                let handler = ContextMenuRequestedEventHandler::create(Box::new(|_sender, args| {
                    if let Some(args) = args {
                        let _ = filter(&args);
                    }
                    Ok(())
                }));
                let mut token = 0i64;
                let _ = core11.add_ContextMenuRequested(&handler, &mut token);
            }
        }
    }

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
    /// macOS: the page's rules (`useNoBrowserMenus`, the key dispatcher) govern the menu and the keys.
    pub fn harden(_pw: &tauri::webview::PlatformWebview) {}

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
    pub fn harden(_pw: &tauri::webview::PlatformWebview) {}

    use std::sync::mpsc::Sender;

    pub fn start(_pw: &tauri::webview::PlatformWebview, _tx: Sender<Result<Vec<u8>, String>>) -> Result<(), String> {
        Err("window captures are implemented on Windows and macOS".into())
    }
}

#[cfg(test)]
mod tests {
    use super::kept_items;

    /// D-70 item 2: on editable content the default menu keeps its text-editing items and the separators between
    /// them; every browser entry goes.
    #[test]
    fn the_text_menu_keeps_text_editing_only() {
        let menu = [
            Some("emoji"),
            None,
            Some("other"), // Import passwords
            None,
            Some("undo"),
            None,
            Some("cut"),
            Some("copy"),
            Some("paste"),
            Some("pasteAndMatchStyle"),
            Some("selectAll"),
            None,
            Some("writingDirection"),
            None,
            Some("other"), // More tools
        ];
        let keep = kept_items(&menu);
        let kept: Vec<Option<&str>> = menu.iter().zip(&keep).filter(|(_, k)| **k).map(|(n, _)| *n).collect();
        assert_eq!(
            kept,
            vec![
                Some("emoji"),
                None,
                Some("undo"),
                None,
                Some("cut"),
                Some("copy"),
                Some("paste"),
                Some("pasteAndMatchStyle"),
                Some("selectAll"),
                None,
                Some("writingDirection"),
            ]
        );
        let page = [Some("back"), Some("forward"), Some("reload"), None, Some("saveAs"), Some("print"), None];
        assert!(kept_items(&page).iter().all(|k| !k), "a page menu keeps nothing");
    }
}
