//! C0.3 rasteriser determinism spike (**spike code**, not product code).
//!
//! Renders every fixture scene of `scene.rs` with the in-house rasteriser (`ttraster`) and with `vello_cpu` in three
//! configurations, writes PNGs, and records SHA-256 hashes of the raw premultiplied RGBA8 pixels and of the PNG files.
//!
//! ```text
//! cargo run --release --locked -p tt-render --example c03_raster -- [--out DIR] [--repeat N]
//!     [--report REPORT.json] [--write-hashes HASHES.json] [--check HASHES.json]
//! ```
//! * `--check` exits with status 1 if any hash differs from the committed file (CI runs this on Windows, macOS and
//!   Linux, so a cross-OS difference fails the build); every scene is still rendered and reported first.
//! * Each scene is rendered `--repeat` times (default 3); differing hashes between repeats also fail.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "command-line spike tool")]

mod scene;
mod ttraster;
mod vello;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const HASH_FORMAT: &str = "tt-c03-raster-hashes/1";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 over a canonical header (`TTPIX1`, width and height as big-endian u32) and the RGBA8 bytes.
fn pixel_hash(w: u32, h: u32, rgba: &[u8]) -> String {
    let mut d = Sha256::new();
    d.update(b"TTPIX1");
    d.update(w.to_be_bytes());
    d.update(h.to_be_bytes());
    d.update(rgba);
    hex(&d.finalize())
}

fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Balanced);
        let mut writer = enc.write_header()?;
        writer.write_image_data(rgba)?;
        writer.finish()?;
    }
    Ok(out)
}

/// Premultiplied to straight alpha for PNG (all fixture backgrounds are opaque, so this is the identity in practice).
fn unpremultiply(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let a = u32::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Renderer {
    Tt,
    Vello(vello::Config),
}

impl Renderer {
    fn name(self) -> &'static str {
        match self {
            Renderer::Tt => "ttraster",
            Renderer::Vello(c) => c.name(),
        }
    }
    fn render(self, s: &scene::Scene) -> Vec<u8> {
        match self {
            Renderer::Tt => ttraster::render(s).rgba,
            Renderer::Vello(c) => vello::render(s, c),
        }
    }
}

const RENDERERS: [Renderer; 4] = [
    Renderer::Tt,
    Renderer::Vello(vello::Config::Fallback),
    Renderer::Vello(vello::Config::Detected),
    Renderer::Vello(vello::Config::Threads3),
];

struct Args {
    out: PathBuf,
    repeat: usize,
    report: Option<PathBuf>,
    write_hashes: Option<PathBuf>,
    check: Option<PathBuf>,
}

fn args() -> Result<Args, Box<dyn Error>> {
    let mut a =
        Args { out: PathBuf::from("target/c03-raster"), repeat: 3, report: None, write_hashes: None, check: None };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--out" => a.out = PathBuf::from(value()?),
            "--repeat" => a.repeat = value()?.parse()?,
            "--report" => a.report = Some(PathBuf::from(value()?)),
            "--write-hashes" => a.write_hashes = Some(PathBuf::from(value()?)),
            "--check" => a.check = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    if a.repeat == 0 {
        return Err("--repeat must be at least 1".into());
    }
    Ok(a)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = args()?;
    let scenes = scene::scenes();
    let mut hashes: BTreeMap<&str, BTreeMap<&str, Value>> = BTreeMap::new();
    let mut timings: BTreeMap<&str, BTreeMap<&str, Value>> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut images: BTreeMap<(&str, &str), Vec<u8>> = BTreeMap::new();

    for r in RENDERERS {
        let dir = args.out.join(r.name());
        fs::create_dir_all(&dir)?;
        for s in &scenes {
            let (w, h) = (u32::from(s.width), u32::from(s.height));
            let mut times = Vec::with_capacity(args.repeat);
            let mut first: Option<(String, Vec<u8>)> = None;
            for _ in 0..args.repeat {
                let t0 = Instant::now();
                let rgba = r.render(s);
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
                let hash = pixel_hash(w, h, &rgba);
                match &first {
                    None => first = Some((hash, rgba)),
                    Some((h0, _)) if *h0 != hash => {
                        failures.push(format!("{}/{}: repeats differ ({h0} vs {hash})", r.name(), s.name));
                    }
                    Some(_) => {}
                }
            }
            let Some((pixels, rgba)) = first else { continue };
            let png = png_bytes(w, h, &unpremultiply(&rgba))?;
            fs::write(dir.join(format!("{}.png", s.name)), &png)?;
            let png_sha = hex(&Sha256::digest(&png));
            hashes
                .entry(r.name())
                .or_default()
                .insert(s.name, json!({"pixels": pixels, "png": png_sha, "width": w, "height": h}));
            timings
                .entry(r.name())
                .or_default()
                .insert(s.name, json!({"msMedian": median(times.clone()), "msAll": times}));
            images.insert((r.name(), s.name), rgba);
        }
    }

    // Informational: how far the in-house rasteriser is from vello_cpu (fallback level) per scene.
    let mut diffs = BTreeMap::new();
    for s in &scenes {
        if let (Some(a), Some(b)) = (images.get(&("ttraster", s.name)), images.get(&("vello_cpu_fallback", s.name))) {
            let mut max = 0_u8;
            let mut sum = 0_u64;
            let mut differing = 0_usize;
            for (pa, pb) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
                let d = pa.iter().zip(pb).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
                max = max.max(d);
                sum += u64::from(d);
                differing += usize::from(d > 0);
            }
            let n = (a.len() / 4).max(1);
            diffs.insert(
                s.name,
                json!({"maxChannelDiff": max, "meanMaxChannelDiff": sum as f64 / n as f64,
                "pixelsDiffering": differing, "pixels": n}),
            );
        }
    }

    let hash_doc = json!({"format": HASH_FORMAT, "renderers": hashes});
    if let Some(path) = &args.write_hashes {
        fs::write(path, serde_json::to_string_pretty(&hash_doc)? + "\n")?;
        println!("wrote {}", path.display());
    }
    let mut mismatches = Vec::new();
    if let Some(path) = &args.check {
        let expected: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        if expected["format"] != HASH_FORMAT {
            return Err(format!("{}: unexpected hash format {}", path.display(), expected["format"]).into());
        }
        for r in RENDERERS {
            for s in &scenes {
                let want = &expected["renderers"][r.name()][s.name];
                let got = &hash_doc["renderers"][r.name()][s.name];
                for key in ["pixels", "png"] {
                    if want[key] != got[key] {
                        mismatches.push(format!(
                            "{}/{} {key}: expected {} got {}",
                            r.name(),
                            s.name,
                            want[key],
                            got[key]
                        ));
                    }
                }
            }
        }
    }

    let report = json!({
        "format": "tt-c03-raster-report/1",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "detectedSimdLevel": format!("{:?}", vello_cpu::Level::new()),
        "repeat": args.repeat,
        "scenes": scenes.iter().map(|s| json!({"name": s.name, "width": s.width, "height": s.height,
            "items": s.items.len(), "covers": s.covers})).collect::<Vec<_>>(),
        "hashes": hash_doc,
        "timingsMs": timings,
        "ttrasterVsVelloFallback": diffs,
        "repeatFailures": failures,
        "checkMismatches": mismatches,
    });
    if let Some(path) = &args.report {
        fs::write(path, serde_json::to_string_pretty(&report)? + "\n")?;
    }
    print_summary(&report, &scenes);
    if !failures.is_empty() || !mismatches.is_empty() {
        for f in failures.iter().chain(&mismatches) {
            eprintln!("FAIL {f}");
        }
        std::process::exit(1);
    }
    Ok(())
}

fn print_summary(report: &Value, scenes: &[scene::Scene]) {
    println!("{} {} (vello_cpu detected SIMD level {})", report["os"], report["arch"], report["detectedSimdLevel"]);
    let names: Vec<&str> = RENDERERS.iter().map(|r| r.name()).collect();
    println!("| scene | {} | ttraster vs vello max diff |", names.join(" ms | ") + " ms");
    println!("|---|{}---|", "---|".repeat(names.len()));
    for s in scenes {
        let cells: Vec<String> = names
            .iter()
            .map(|r| format!("{:.3}", report["timingsMs"][r][s.name]["msMedian"].as_f64().unwrap_or(f64::NAN)))
            .collect();
        println!(
            "| {} | {} | {} |",
            s.name,
            cells.join(" | "),
            report["ttrasterVsVelloFallback"][s.name]["maxChannelDiff"]
        );
    }
}
