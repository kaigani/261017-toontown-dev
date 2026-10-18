# ADR-0004 — Line endings and cross-platform determinism

Status: accepted (C0.1, 2026-10-17). The rasteriser-specific part follows in C0.3/C0.4.

## Context

The spec requires byte-identical saves (10 §6.5 canonical JSON), byte-identical renders across runs, restarts and
between attached and headless modes (11 P7, 11 §14.5), and seeded randomness (06 §4.6). The product is built and
tested on two OSes and two CPU architectures (Windows x86_64, macOS aarch64). Text checked out with CRLF on Windows
would change hashes of fixtures and goldens; floating-point results can differ between platforms when the code
relies on platform maths libraries or compiler contraction.

## Decision

1. **Line endings.** `.gitattributes`: `* text=auto eol=lf`, binaries (`*.png`, `*.jpg`, `*.wav`, `*.mp3`,
   `*.animpkg`, `*.onnx`) marked binary. `.editorconfig`, `rustfmt.toml` (`newline_style = "Unix"`) and Prettier
   (`endOfLine: "lf"`) agree. Files the application writes (JSON, manifests) always use `\n`, never the platform
   line separator.
2. **Toolchain pinning.** `rust-toolchain.toml` pins the exact compiler (1.98.1) for every machine and CI runner;
   `Cargo.lock` and `pnpm-lock.yaml` are committed and CI uses `--locked` / `--frozen-lockfile`.
3. **Floating point in persisted geometry and final renders** uses only IEEE-754 correctly rounded operations
   (`+ − × ÷`, `sqrt`, conversions) in a fixed evaluation order, `f64` throughout. No `fast-math`, no reliance on
   FMA contraction (Rust does not contract by default; explicit `mul_add` only where the same instruction exists on
   every target). Transcendental functions (`sin`, `cos`, `atan2`, `exp`, `powf`, …) come from the platform libm and
   are **not** assumed identical across OSes: where they influence persisted or rendered data, M2/M3 use an in-house
   (or pure-Rust, e.g. `libm`) implementation and pin it with golden tests.
4. **No iteration-order nondeterminism**: hash maps that affect output use ordered maps or sorted iteration;
   randomness only from an in-house seeded PRNG with the seed stored in the document (`tt-fx`, M7).
5. **Evidence from M0.** `tt-engine`'s self-test includes `determinism.ieee754`: a Bézier/arc-length workload using
   only the operations of rule 3, hashed with SHA-256 and compared with a golden value recorded on Windows x86_64
   and reproduced bit-for-bit by an independent Python implementation. CI runs it on Windows x86_64, macOS aarch64
   and Linux x86_64, twice per job. C0.3 extends this to rasteriser output hashes compared across OSes.

## Consequences

- Any golden/fixture mismatch between OSes is a CI failure, not a tolerance to tune.
- Contributors on Windows must keep `core.autocrlf` from rewriting files; `.gitattributes` overrides it in this
  repository.
