//! C0.3 canvas throughput spike (**spike code**): render lists generated in Rust and sent to the webview as binary
//! `tauri::ipc::Response` bodies (not JSON). Reachable only with `--spike canvas`.
//!
//! Commands:
//! * [`spike_canvas_scene`]: one encoded render list (format in [`renderlist`]); generation is cached per
//!   (scene, seed), encoding runs on every call and its time travels in the header.
//! * [`spike_canvas_pixels`]: a `width × height` RGBA8 frame (plus a 16-byte header) for the host-raster fallback
//!   estimate: what moving one finished bitmap per frame across IPC costs.

pub mod renderlist;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::Deserialize;
use tauri::State;
use tauri::ipc::Response;
use tt_engine::ErrorInfo;

use super::tablet::{SpikeSettings, error};
use renderlist::{Kind, Scene};

/// Managed state of the canvas spike.
pub struct CanvasSpike {
    settings: Option<SpikeSettings>,
    scenes: Mutex<HashMap<(Kind, u32), (Scene, u64)>>,
}

impl CanvasSpike {
    /// State for the given launch settings (`None` or another spike = disabled).
    #[must_use]
    pub fn new(settings: Option<SpikeSettings>) -> Self {
        Self { settings: settings.filter(|s| s.name == "canvas"), scenes: Mutex::new(HashMap::new()) }
    }

    fn require(&self) -> Result<&SpikeSettings, ErrorInfo> {
        self.settings.as_ref().ok_or_else(|| error("spike.notEnabled", "start the app with --spike canvas".into()))
    }
}

/// Which render list to send.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasSceneRequest {
    /// `paths100k`, `groups1k` or `brush10k`.
    pub scene: String,
    /// Generator seed.
    pub seed: u32,
    /// Animation frame (moves the group transforms of `groups1k`; 0 = base layout).
    pub frame: u32,
}

/// Encodes the requested scene as a binary render list (see [`renderlist`]).
///
/// # Errors
/// `spike.notEnabled`, `arg.invalid` (unknown scene), `internal`.
#[tauri::command]
pub async fn spike_canvas_scene(
    spike: State<'_, CanvasSpike>,
    request: CanvasSceneRequest,
) -> Result<Response, ErrorInfo> {
    spike.require()?;
    let kind = Kind::parse(&request.scene)
        .ok_or_else(|| error("arg.invalid", format!("unknown scene {:?}", request.scene)))?;
    let mut cache = spike.scenes.lock().map_err(|_| error("internal", "scene cache poisoned".into()))?;
    let (scene, generate_ns) = match cache.get(&(kind, request.seed)) {
        Some((scene, _)) => (scene, 0),
        None => {
            let t0 = Instant::now();
            let scene = renderlist::generate(kind, request.seed);
            let ns = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let entry = cache.entry((kind, request.seed)).or_insert((scene, ns));
            (&entry.0, ns)
        }
    };
    Ok(Response::new(renderlist::encode(kind, scene, request.frame, generate_ns)))
}

/// Magic of a pixel frame (`TTPX`, then width u32, height u32, fill ns u32; then RGBA8).
pub const PIXELS_MAGIC: &[u8; 4] = b"TTPX";

/// A synthetic RGBA8 frame of the given size, for measuring per-frame bitmap transfer.
///
/// # Errors
/// `spike.notEnabled`, `arg.invalid` (size 0 or above 8192 × 8192).
#[tauri::command]
pub async fn spike_canvas_pixels(
    spike: State<'_, CanvasSpike>,
    width: u32,
    height: u32,
    frame: u32,
) -> Result<Response, ErrorInfo> {
    spike.require()?;
    if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
        return Err(error("arg.invalid", "width and height must be 1..=8192".into()));
    }
    let t0 = Instant::now();
    let n = width as usize * height as usize;
    let mut out = Vec::with_capacity(16 + 4 * n);
    out.extend_from_slice(PIXELS_MAGIC);
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    let shade = (frame & 0xFF) as u8;
    for i in 0..n {
        let x = (i % width as usize) as u8;
        out.extend_from_slice(&[x, shade, x ^ shade, 255]);
    }
    let ns = u32::try_from(t0.elapsed().as_nanos()).unwrap_or(u32::MAX);
    out[12..16].copy_from_slice(&ns.to_le_bytes());
    Ok(Response::new(out))
}
