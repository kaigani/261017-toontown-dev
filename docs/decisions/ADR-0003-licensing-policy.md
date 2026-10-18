# ADR-0003 — Licensing policy for dependencies and shipped components

Status: accepted (C0.1, 2026-10-17). The FFmpeg sidecar build/encoder/fetch plan gets its own ADR in C0.4.

## Context

The product ships closed binaries for Windows and macOS. The user decided (plan §6.3) that video encoding uses an
LGPL FFmpeg build as a separate sidecar with platform encoders and that no GPL component ships. Rust crates are
statically linked into our executables; JavaScript dependencies are bundled into the app's web assets.

## Decision

1. **Rust dependency graph** (`deny.toml`, `cargo deny check` in CI on Windows and macOS). Allowed licences:
   MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MPL-2.0 (the user's list) **plus Unicode-3.0**. Everything
   else is rejected, which rejects GPL, AGPL and LGPL. Notes:
   - **Unicode-3.0** (OSI-approved, permissive) is required by the ICU4X crates (`icu_*`, `litemap`, `potential_utf`,
     `tinystr`, `writeable`, `yoke`, `zerofrom`, `zerovec`, `zerotrie`) that Tauri pulls in through `url` → `idna`.
     It is the only addition to the user's list; flagged in the C0.1 handoff for confirmation.
   - Dual-licensed crates pass when one alternative is allowed (e.g. `MIT OR Unlicense`, `CC0-1.0 OR MIT-0 OR
     Apache-2.0`).
   - The deny graph covers the shipped targets only (Windows x86_64/aarch64, macOS x86_64/aarch64). Linux is not a
     product platform; it only runs the headless CI proof, whose crates are a subset of the Windows/macOS graph.
   - Sources: crates.io only; git and other registries are rejected.
2. **LGPL is allowed only as a dynamically linked or separately executed sidecar** — concretely the FFmpeg build
   (plan §6.3), invoked as a separate process by `tt-io`, never linked into our binaries, shipped with its licence
   text and corresponding-source offer. Because a sidecar is not a Cargo dependency, LGPL never appears in the Rust
   allow-list.
3. **JavaScript shipped in the app** (`scripts/check-npm-licenses.mjs`, `pnpm licenses:check` in CI): production
   dependencies must satisfy the same allow-list (plus 0BSD). Development dependencies (build tools, not
   distributed) are reported, not gated.
4. **Data files** (the lip-sync ONNX model, plan §6.4) are downloaded on first use under their own licence
   (Apache-2.0 for the recommended model) and are not part of the installer.
5. **Never**: GPL or AGPL in anything we ship; FFmpeg builds configured with `--enable-gpl` or `--enable-nonfree`
   (no x264/x265, no fdk-aac).

## Consequences

- A new dependency with any other licence fails CI and needs an ADR amendment before it can be added.
- Installers must carry a third-party notices file generated from `cargo deny list` / `pnpm licenses list` plus the
  FFmpeg notice (M11, C11.2).
