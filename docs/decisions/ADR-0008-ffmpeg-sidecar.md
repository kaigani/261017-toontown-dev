# ADR-0008 — FFmpeg sidecar: LGPL build, encoders, supply chain and licence gates

Status: accepted as the plan (C0.4, 2026-10-18; revised after C0.4 review round 1). **No code yet.**
- **M8 C8.3**, plan §4: "FFmpeg sidecar build, fetch/verify mechanism and decoder readiness (moved here from M10 on
  2026-10-18 because import needs the sidecar first; see ADR-0008), media elements, movie import as sequence". This
  was ruled in C04-R1-07.
- **M10 C10.1**, plan §4: "FFmpeg encoders (sidecar itself built in M8 C8.3): MP4/MOV/GIF export, audio mux, …,
  repeat-encode determinism checks (09 §11.1(6))".
- Printing is M10 C10.2.

It implements user decision 3 (plan §6) and ADR-0003 rule 3, which deferred this plan to C0.4. It is not legal
advice. Items that need the user's judgement are in the C0.4 go/no-go escalation list.

## Context

Spec 09 §4.3 makes MP4 (H.264 + AAC) the baseline export and requires MOV (H.264; an alpha-capable codec, PNG-in-MOV
or ProRes 4444, SHOULD). Spec 09 §4.4 asks for quality "lossless where supported". Spec 09 §11.1(6) requires
`deterministic: true` on movie export to strip creation timestamps and encoder-version metadata "so repeated exports
produce identical checksums". Spec 09 §8 imports MOV/MP4/M4V (H.264, H.265, MPEG-4 Part 2, ProRes, Motion JPEG) and
AVI.

User decision 3: "LGPL FFmpeg sidecar with platform encoders (`h264_mf`, `h264_videotoolbox`), AAC via the platform
encoder or FFmpeg's native `aac`. No GPL component ships." ADR-0003 allows LGPL only as a separately executed
sidecar, never linked into our binaries, and forbids `--enable-gpl` / `--enable-nonfree` builds.

## Decision

### 1. The build: LGPL-2.1-or-later, with an explicit component whitelist

- We build FFmpeg ourselves from a **pinned upstream release tarball**. The version is chosen at M8 C8.3. Its SHA-256
  and the upstream release signature are verified before building.
- **Licence switches.** Never `--enable-gpl`, `--enable-nonfree` or `--enable-version3`. The result is LGPL v2.1 or
  later, as printed by `ffmpeg -L`. No external codec libraries: no libx264/libx265, no libfdk-aac, nor any other
  `--enable-lib*` codec.
- **Component whitelist.** `--disable-autodetect` only stops discovery of external libraries; it does not limit
  FFmpeg's built-in codecs. The build therefore starts from `--disable-everything` and enables each component
  explicitly:
  - `--enable-encoder=…`, `--enable-decoder=…`, `--enable-muxer=…`, `--enable-demuxer=…`, `--enable-parser=…`,
    `--enable-bsf=…`, `--enable-protocol=file,pipe` and `--enable-filter=…`;
  - the platform frameworks: `--enable-mediafoundation` on Windows, `--enable-videotoolbox --enable-audiotoolbox` on
    macOS;
  - `--disable-network`, `--disable-ffplay`, `--disable-doc`.

  The lists live in the committed build script and in `tools/ffmpeg/manifest.json`.
- Only the `ffmpeg` program is built (`ffprobe` if import needs it). The libraries are linked statically into that one
  executable. The executable is the LGPL work; our application only runs it as a separate process.
- **Encoders per OS** (whitelist for M10 C10.1). Spec 09 §4.4 sets the defaults: H.264, stereo 48 kHz, 192 kbit/s.

  | Purpose | Windows | macOS | Both (native FFmpeg, LGPL) |
  |---|---|---|---|
  | H.264 video (MP4, MOV) | `h264_mf` | `h264_videotoolbox` | — |
  | AAC audio | `aac_mf` preferred | `aac_at` preferred | `aac` (fallback, and the deterministic-mode candidate, §4) |
  | Alpha-capable MOV (spec SHOULD) | — | (`prores_videotoolbox` if verified) | `png` (PNG-in-MOV); `prores_ks` (ProRes 4444) |
  | **Lossless** ("lossless where supported", spec 09 §4.4) | — | — | `ffv1` in MKV, and in MOV if the muxer passes the M10 check; PNG image sequences are written by our own code |
  | PCM in MOV | — | — | `pcm_s16le` / `pcm_s24le` |
  | Animated GIF (spec SHOULD) | — | — | `gif` |

  - **Lossless means mathematically lossless only.** It is offered for:
    - PNG image sequences;
    - FFV1 (MKV; MOV once verified);
    - PNG-in-MOV once it passes the same verification.

    Lossless is verified in M10 by decoding the exported file and comparing every pixel (and alpha) with the rendered
    frames.
  - **ProRes variants, including 4444, are "visually lossless", not lossless.** Their colour coding is lossy, and
    they are labelled that way in the UI and in `export.formats.list`.
  - JPEG sequences and H.264 are never offered as lossless.
- **Muxers:** `mp4`, `mov`, `matroska`, `gif`.
- **Decoders and demuxers** (whitelist for M8 C8.3): only those for the spec 09 §8 import list, after the user's
  import posture decision (Consequences).

### 2. Supply chain: fetched, verified and shipped as a build input (from M8 C8.3)

- **Build (CI).** A committed build script produces one sidecar per shipped target:
  - Windows x86-64, and aarch64 if Windows on Arm is shipped;
  - macOS arm64 and x86-64.

  Each output is published as a release asset of this repository, with a manifest `tools/ffmpeg/manifest.json`
  holding the upstream version, the source tarball's SHA-256, the configure line, the component whitelist and each
  asset's SHA-256.
- **Fetch (app build).** `scripts/fetch-ffmpeg.py` downloads the pinned assets, verifies each SHA-256 against the
  manifest, and fails the build on any mismatch. Nothing is downloaded at run time, unlike the lip-sync model
  (decision 4).
- **Ship.** The sidecar is a Tauri `bundle.externalBin` (`binaries/ffmpeg-<target-triple>[.exe]`), placed next to the
  application executable.
  - It is signed with the application: Authenticode on Windows; on macOS, codesign with the hardened runtime, then
    notarization of the whole bundle.
  - The headless distribution carries the same executable next to `toontown-headless`.
- **Run.** `tt-io` (the engine, so headless import and export work) starts it with `std::process::Command`, using an
  argument array and no shell. It resolves the path next to the running executable and streams frames over stdin
  pipes. A missing, non-matching or failing sidecar is a typed `import.*` / `export.*` error. There are no Rust FFmpeg
  bindings (`ffmpeg-next`, `ffmpeg-sys*`): they would link libav* into our binaries, which ADR-0003 rule 3 forbids.

### 3. What the installer contains

- The application executable(s) and the `ffmpeg` sidecar.
- `THIRD-PARTY-NOTICES` (M11 C11.2), generated from `cargo deny list` and `pnpm licenses list`, plus an FFmpeg
  section:
  - the LGPL-2.1 text (`COPYING.LGPLv2.1`);
  - the FFmpeg version and exact configure line (also printed by `ffmpeg -buildconf`);
  - a statement that FFmpeg is a separate program the user may replace;
  - a written offer of, or URL to, the **exact** source tarball, our build script and the patches (none planned), kept
    for as long as the binaries are distributed.
- No FFmpeg headers or static libraries; there is nothing to relink, because our code does not link FFmpeg.

### 4. Movie determinism: three different properties

1. **Raster determinism** (spec 06 §6.7). Settled by ADR-0006: the frames fed to the encoder are byte-identical for
   the same version, across OSes and CPUs.
2. **Repeat-encode determinism** (spec 09 §11.1(6)) — **a requirement we must meet or escalate.** Two exports with
   `deterministic: true` must produce identical checksums on the same build, platform and configuration (same
   document revision and parameters). In that mode the export:
   - strips variable metadata: creation and modification times (`-fflags +bitexact`, `-flags:v +bitexact`,
     `-flags:a +bitexact`, no `creation_time`), the encoder-version tag, and random or UUID atoms;
   - fixes muxer choices: fast start, interleaving and timescale;
   - runs encoders single-threaded or with their deterministic options where they have them.

   The manifest records `deterministic: true` and the encoder actually used.
3. **Cross-platform movie identity** — **not required.** The encoders differ per OS (Media Foundation vs
   VideoToolbox), so movie bytes differ between platforms. Spec 09 §11.1(2) asks for byte-identical PNG renders "on
   the same build and platform", and nothing requires identical movies across platforms.

**If a platform encoder cannot produce identical output across runs** (hardware encoders may vary), deterministic
mode uses a software path on that platform instead:
- video: an LGPL-compatible software encoder for the same container — FFV1, or PNG-in-MOV where the format allows it;
  a software H.264 encoder only under a user licensing decision, since x264 is excluded by decision 3;
- audio: native `aac` or PCM.

If no allowed software path can meet a format's `deterministic: true` guarantee, the incompatibility is raised to the
spec owner as **SI-002** (`plan/spec-issues.md`), with the measured evidence. The parameter's guarantee is not
dropped silently. Until a ruling, that format rejects `deterministic: true` with a typed error rather than producing
varying files.

### 5. Licence gates

- **The existing gates do not cover native linkage.** `cargo deny` and the JS licence gate stay as they are, and LGPL
  is never added to the shared allow-list (ADR-0003 rule 1). But those gates evaluate each package's declared SPDX
  licence only. A crate declaring MIT that has `links = "avcodec"` and a build script emitting
  `cargo:rustc-link-lib=avcodec` passes them; the reviewer's probe shows this
  (`reviews/evidence/M0-C0.4-r1/license-link-probe.json`).
- **Native-linkage check.** This is an **M10 C10.1 obligation** (ruling C04-R1-06); I also run it from the first
  sidecar build at M8 C8.3. A committed check, run in CI on every shipped target, fails when:
  - any package in `cargo metadata` has a `links` key outside an allow-list of reviewed native libraries;
  - any build script emits `cargo:rustc-link-lib` for a library outside that allow-list. `cargo build -vv` prints
    each build script's directives, so the check reads them from that output.

  The allow-list starts with the native libraries the current graph links (OS import libraries and system
  frameworks), reviewed once. It never admits `avcodec`, `avformat`, `avutil`, `swscale`, `swresample` or `x264`.
- **Sidecar gate** (from M8 C8.3). `scripts/check-ffmpeg-sidecar.py <ffmpeg>`, in CI for every shipped target, before
  packaging. It fails unless:
  - `-buildconf` contains `--disable-everything` and none of `--enable-gpl`, `--enable-nonfree`, `--enable-version3`
    or `--enable-lib`;
  - `-L` reports the LGPL version 2.1 or later;
  - the `-encoders`, `-decoders`, `-muxers` and `-demuxers` lists **equal** the manifest's whitelist for that target.
    The lists are compared, not just checked for the presence of a few names;
  - the executable's SHA-256 equals the manifest entry.

## Consequences

- **Windows "N" / "KN" editions ship without Media Foundation**, so `h264_mf` and `aac_mf` are unavailable there.
  Export of those formats then fails with a typed error that tells the user to install the Media Feature Pack.
  PNG/FFV1 and native `aac` do not depend on Media Foundation. Whether that is acceptable is a user decision
  (escalation list).
- **Spec 09 §4.4's "high (≈ CRF 18 equivalent)" needs a mapping** onto `h264_mf`'s quality-based mode and
  VideoToolbox's constant-quality or bit-rate settings. That is measured in E-FF1. This is SI-001, open for that
  question only; the lossless half is settled by §1.
- **Decoders for import are not covered by decision 3, which speaks about encoders.** FFmpeg's native H.264/HEVC
  decoders are LGPL, but codec patents can concern decoding too. Platform-accelerated decoding (`d3d11va`,
  `videotoolbox` hwaccel) is an alternative. The import posture is the user's decision (escalation list), needed
  before M8 C8.3. The decoder whitelist follows it.

### Experiments

- **E-FF0 (M8 C8.3 entry)** proves the sidecar for import:
  - build the LGPL sidecar in CI for Windows x86-64 and macOS arm64;
  - pass the sidecar gate and the native-linkage check;
  - fetch and verify it through the manifest;
  - decode the spec 09 §8 import fixtures allowed by the user's posture, comparing decoded frames with reference PNGs.
- **E-FF1 (M10 C10.1 entry)** proves the encoders:
  - encode a 10 s 1080p fixture at the spec defaults: MP4 (H.264 + AAC), MOV (H.264; PNG-in-MOV with alpha; ProRes
    4444), MKV (FFV1) and GIF;
  - verify the streams with `ffprobe`, fast start, and playback in the OS player;
  - **lossless:** decode the PNG sequence, FFV1 and PNG-in-MOV outputs and compare every pixel and alpha value with
    the rendered frames. ProRes is checked only for being labelled "visually lossless";
  - **repeat-encode determinism:** export each format twice with `deterministic: true`, in two separate processes,
    on each platform, and compare SHA-256. Do the same for each muxer path (MP4, MOV, MKV) and for AAC (platform and
    native) separately from video. Inspect the metadata of both files for remaining timestamps or version strings.
    A platform encoder that varies triggers the §4 software path, or SI-002;
  - measure the quality mapping (SI-001), and record the behaviour without Media Foundation (a Windows N VM).
