# ADR-0003 — Licensing policy for dependencies and shipped components

Status: accepted (C0.1, 2026-10-17; amended after review round 1: one shared allow-list, 0BSD added, SPDX
evaluation). The FFmpeg sidecar build/encoder/fetch plan gets its own ADR in C0.4.

## Context

The product ships closed binaries for Windows and macOS. The user decided (plan §6.3) that video encoding uses an
LGPL FFmpeg build as a separate sidecar with platform encoders and that no GPL component ships. Rust crates are
statically linked into our executables; JavaScript dependencies are bundled into the app's web assets.

## Decision

1. **One allow-list for both ecosystems.** The approved set is exactly:
   **MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MPL-2.0, Unicode-3.0, 0BSD.**
   - The first seven are the user's list. **Unicode-3.0** (OSI-approved, permissive) is required by the ICU4X crates
     (`icu_*`, `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke`, `zerofrom`, `zerovec`, `zerotrie`) that
     Tauri pulls in through `url` → `idna`. **0BSD** (public-domain-equivalent, permissive) appears today only as one
     alternative of multi-licensed crates (e.g. `adler2`: `0BSD OR MIT OR Apache-2.0`) and is common in npm packages. Both additions were accepted by the orchestrator on 2026-10-17 (C0.1 review round 1, C01-R1-01).
   - The list lives in one place, `deny.toml` `[licenses] allow`. `scripts/check-npm-licenses.mjs` reads that array,
     so the two gates cannot drift; `tests/adversarial/guards.mjs allowlist` checks list = this ADR's set.
   - Everything else is rejected, which rejects GPL, AGPL and LGPL. Licence expressions are evaluated as SPDX
     boolean expressions (AND binds tighter than OR, parentheses nest, `WITH` exceptions must be allowed as a whole,
     `X+` passes when `X` does); malformed or non-SPDX strings fail closed.
   - Dual-licensed packages pass when one alternative is allowed (e.g. `MIT OR Unlicense`).
2. **Rust dependency graph** (`cargo deny check` in CI on Windows and macOS). The graph covers the shipped targets
   only (Windows x86_64/aarch64, macOS x86_64/aarch64). Linux is not a product platform; it only runs the headless
   CI proof, whose crates are a subset of the Windows/macOS graph. Sources: crates.io only; git and other registries
   are rejected.
3. **LGPL is allowed only as a dynamically linked or separately executed sidecar** — concretely the FFmpeg build
   (plan §6.3), invoked as a separate process by `tt-io`, never linked into our binaries, shipped with its licence
   text and corresponding-source offer. Because a sidecar is not a Cargo dependency, LGPL never appears in the Rust
   allow-list.
4. **JavaScript shipped in the app** (`scripts/check-npm-licenses.mjs`, `pnpm licenses:check` in CI): production
   dependencies must satisfy the same allow-list (rule 1). Development dependencies (build tools, not distributed)
   are reported, not gated.
5. **Data files** (the lip-sync ONNX model, plan §6.4) are downloaded on first use under their own licence
   (Apache-2.0 for the recommended model) and are not part of the installer.
6. **Never**: GPL or AGPL in anything we ship; FFmpeg builds configured with `--enable-gpl` or `--enable-nonfree`
   (no x264/x265, no fdk-aac).

## Consequences

- A new dependency with any other licence fails CI and needs an ADR amendment before it can be added.
- Installers must carry a third-party notices file generated from `cargo deny list` / `pnpm licenses list` plus the
  FFmpeg notice (M11, C11.2).
