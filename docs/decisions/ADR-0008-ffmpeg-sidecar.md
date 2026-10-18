# ADR-0008 — FFmpeg sidecar: LGPL build, encoders, supply chain and licence gates

Status: accepted as the plan (C0.4, 2026-10-18). **No code yet**: the sidecar is built in M9 (export). It implements
user decision 3 (plan §6) and ADR-0003 rule 3, which deferred this plan to C0.4. It is not legal advice. Items that
need the user's judgement are in the C0.4 go/no-go escalation list.

## Context

Spec 09 §4.3 makes MP4 (H.264 + AAC) the baseline export and requires MOV (H.264; an alpha-capable codec, PNG-in-MOV
or ProRes 4444, SHOULD). Spec 09 §8 imports MOV/MP4/M4V (H.264, H.265, MPEG-4 Part 2, ProRes, Motion JPEG) and AVI.

User decision 3: "LGPL FFmpeg sidecar with platform encoders (`h264_mf`, `h264_videotoolbox`), AAC via the platform
encoder or FFmpeg's native `aac`. No GPL component ships." ADR-0003 allows LGPL only as a separately executed
sidecar, never linked into our binaries, and forbids `--enable-gpl` / `--enable-nonfree` builds.

## Decision

### 1. The build: LGPL-2.1-or-later, configured explicitly

- We build FFmpeg ourselves from a **pinned upstream release tarball**. The version is chosen in M9. Its SHA-256 and
  the upstream release signature are verified before building.
- `configure` runs with `--disable-autodetect`, so nothing is enabled by accident, and then enables only what is
  listed here.
- **Never** `--enable-gpl`, `--enable-nonfree` or `--enable-version3`. The result is LGPL v2.1 or later, as printed by
  `ffmpeg -L`.
- No external codec libraries: no libx264/libx265, no libfdk-aac, nor any other `--enable-lib*` codec.
- Only the `ffmpeg` program is built (`ffprobe` if M9 needs it). The libraries are linked statically into that one
  executable. The executable is the LGPL work; our application only runs it as a separate process.
- **Encoders per OS.** Spec 09 §4.4 sets the defaults: H.264, stereo 48 kHz, 192 kbit/s.

  | Purpose | Windows (`--enable-mediafoundation`) | macOS (`--enable-videotoolbox --enable-audiotoolbox`) | Both (native FFmpeg, LGPL) |
  |---|---|---|---|
  | H.264 video (MP4, MOV) | `h264_mf` | `h264_videotoolbox` | — |
  | AAC audio | `aac_mf` preferred | `aac_at` preferred | `aac` as fallback |
  | Alpha MOV (SHOULD) | — | (`prores_videotoolbox` if verified) | `png` (PNG-in-MOV), `prores_ks` (ProRes 4444) |
  | PCM in MOV | — | — | `pcm_s16le` / `pcm_s24le` |
  | Animated GIF (SHOULD) | — | — | `gif` |

  Muxers: `mp4`, `mov`, `gif`; image sequences are written by our own code. Decoders and demuxers are enabled only
  for the spec 09 §8 import list. See also the open question on decoders under Consequences.

### 2. Supply chain: fetched, verified and shipped as a build input

- **Build (CI, M9).** A committed build script produces one sidecar per shipped target:
  - Windows x86-64, and aarch64 if Windows on Arm is shipped;
  - macOS arm64 and x86-64.

  Each output is published as a release asset of this repository, with a manifest
  `tools/ffmpeg/manifest.json` holding the upstream version, the source tarball's SHA-256, the configure line, and
  each asset's SHA-256.
- **Fetch (app build).** `scripts/fetch-ffmpeg.py` downloads the pinned assets, verifies each SHA-256 against the
  manifest, and fails the build on any mismatch. Nothing is downloaded at run time, unlike the lip-sync model
  (decision 4).
- **Ship.** The sidecar is a Tauri `bundle.externalBin` (`binaries/ffmpeg-<target-triple>[.exe]`), placed next to the
  application executable.
  - It is signed with the application: Authenticode on Windows; on macOS, codesign with the hardened runtime, then
    notarization of the whole bundle.
  - The headless distribution carries the same executable next to `toontown-headless`.
- **Run.** `tt-io` (the engine, so headless export works) starts it with `std::process::Command`, using an argument
  array and no shell. It resolves the path next to the running executable, streams frames over stdin pipes, and
  treats a missing, non-matching or failing sidecar as a typed `export.*` error. There are no Rust FFmpeg bindings
  (`ffmpeg-next`, `ffmpeg-sys*`): they would link libav* into our binaries, which ADR-0003 rule 3 forbids.

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

### 4. Licence gates

- **`cargo deny` and the JS licence gate stay as they are.** FFmpeg is not a Cargo or npm dependency, so LGPL is never
  added to the shared allow-list (ADR-0003 rule 1). Any Cargo crate that links FFmpeg fails the existing gate (LGPL /
  GPL operands are rejected) and is rejected by this ADR.
- **A sidecar gate is added with the build (M9).** `scripts/check-ffmpeg-sidecar.py <ffmpeg>`, in CI for every
  shipped target, before packaging. It fails unless:
  - `-buildconf` contains none of `--enable-gpl`, `--enable-nonfree`, `--enable-version3` or `--enable-lib`;
  - `-L` reports the LGPL version 2.1 or later;
  - `-encoders` lists the platform H.264 encoder and an AAC encoder for that target;
  - the executable's SHA-256 equals the manifest entry.

## Consequences

- **Windows "N" / "KN" editions ship without Media Foundation**, so `h264_mf` and `aac_mf` are unavailable there.
  Export then fails with a typed error that tells the user to install the Media Feature Pack. Whether that is
  acceptable is a user decision (escalation list).
- **Spec 09 §4.4's quality setting needs a mapping** ("high (≈ CRF 18 equivalent)", "lossless where supported"):
  - CRF is an x264 rate-control concept, and neither platform encoder implements it;
  - M9 maps the quality levels to `h264_mf`'s quality-based mode and VideoToolbox's constant-quality or bit-rate
    settings, and measures them;
  - lossless H.264 is not offered; lossless export uses PNG-in-MOV, ProRes 4444 or an image sequence. This is
    recorded in `plan/spec-issues.md`.
- **Decoders for import are not covered by decision 3, which speaks about encoders.** FFmpeg's native H.264/HEVC
  decoders are LGPL, but codec patents can concern decoding too. Platform-accelerated decoding (`d3d11va`,
  `videotoolbox` hwaccel) is an alternative. The import posture is the user's decision (escalation list). The build
  enables decoders only after it.
- Encoders differ per OS, so exported movie bytes differ per OS. Spec determinism (06 §6.7) covers rendered frames,
  not encoded files. The frames fed to the encoder are the deterministic ADR-0006 output.

### Experiment (M9 entry)

- **E-FF1.** Build the LGPL sidecar in CI for Windows x86-64 and macOS arm64, and run the sidecar gate.
- Encode a 10 s 1080p fixture at the spec defaults, MP4 (H.264 + AAC) and MOV (PNG-in-MOV with alpha), and verify:
  - the streams with `ffprobe`;
  - fast start;
  - playback in the OS player.
- Measure the quality mapping above, and record the behaviour without Media Foundation (a Windows N VM).
