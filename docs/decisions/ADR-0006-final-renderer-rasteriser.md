# ADR-0006 — Final-quality rasteriser and its arithmetic policy

Status: accepted for M3 (C0.4, 2026-10-18), subject to the orchestrator's review of C0.4. It resolves the C0.4 ticket
C03-R1-04 ("SIMD policy for the final renderer").

## Context

Spec 06 §6.7 requires a final-quality renderer whose output is byte-identical "on the same machine and build", and
says it SHOULD be identical across machines. Spec 06 decision 14 asks the renderer to "avoid fused or reordered
floating-point operations where results could differ". Spec 11 §3 recommends that offscreen renders always use the
deterministic software rasteriser. Plan §2.1 says to evaluate `vello_cpu` first and to keep an own exact-area
rasteriser as the fallback.

C0.3 measured both candidates on 39 fixture scenes in six fixed configurations. The evidence and its limits are in
`reviews/M0-C0.3-handoff.md` and the reviewers' closure report `reviews/M0-C0.3-r4-closure.md`. Established there:
- **Each fixed configuration is byte-identical across OSes.** That holds for all 234 entries (39 scenes × 6
  configurations; `crates/render/examples/c03_raster/hashes.json`) on Windows x86-64, macOS arm64 and Linux x86-64,
  in two processes each. CI runs:
  - 39126106552 (golden source; Windows AVX2, macOS NEON, Linux AVX-512);
  - 39126796883 (AVX2 / NEON / AVX2);
  - 39129350790 (Windows AVX-512 / NEON / AVX2);
  - 39131376596 (closure: 234/234 entries and 1,404 PNG byte comparisons).
- **Scalar and SIMD are not pixel-equivalent.** `vello_cpu` with `Level::fallback()` differs from the detected SIMD
  level on 20 scenes (4 original + 16 stress), in both the u8 and the f32 pipelines. The detected levels (AVX2,
  AVX-512, NEON) agree with each other.
- **The u8 pipeline loses opacity.** Alpha drops to 250 over an opaque background; the f32 pipeline and `ttraster`
  keep 255 (`crates/render/tests/c03_fixed_configurations.rs`, `c03_r2_quality.rs`).
- **Compiler variation tested so far does not change the scalar hashes.** Per the reviewer's experiment:
  - the default release build and `RUSTFLAGS="-C target-cpu=native"` reproduce all 234 entries;
  - `opt-level` 1 versus 3 reproduces every `ttraster` hash (`reviews/evidence/M0-C0.3-r2/profiles.py`).
- **Cost.** On the 512 × 512 performance scene, this machine measures:
  - scalar f32 about **6.9 ms**, detected-SIMD f32 **4.8 ms**, `ttraster` **18.5 ms**;
  - f32 versus u8 costs **about −3 % to +15 %** on the CI runners, which is "an approximate trade-off, not a bound"
    (reviewer).

The reviewer's round-2 assessment, which both parties recorded: "provisionally choose `vello_cpu` f32 with
`Level::fallback()` behind the project's adapter, keeping `ttraster` as an independent reference. ... Neither adapter
is yet a spec-complete final renderer."

## Decision

1. **The final-quality renderer is `vello_cpu`, f32 pipeline, scalar arithmetic, single-threaded, behind our own
   adapter.** The adapter lives in `tt-render` (M3) and consumes the shared render list (spec 06 §6.1).
   Concretely:
   - `RenderMode::OptimizeQuality`;
   - `RenderSettings { level: Level::fallback(), num_threads: 0 }`;
   - `vello_cpu` with the feature set the goldens were made with (`std`, `u8_pipeline`, `f32_pipeline`,
     `multithreading`). Dropping the unused features is expected to change nothing, but it is a golden-gated change
     like any other;
   - `fearless_simd` with `force_support_fallback`;
   - this is the C0.3 configuration `vello_cpu_f32_fallback`.

   The u8 pipeline, the detected SIMD level and the multithreaded renderer are **not** used for final output.
   Multithreading is excluded because no multithreaded scalar configuration has been golden-tested; adding one
   requires its own golden set first.
2. **`ttraster` stays as an independent reference.** It is a dev/test-only crate target, never shipped. Its use is
   coverage and compositing cross-checks on the subset it implements. It is not a fallback renderer.
3. **What "deterministic" means for this project.** For one application version, identical inputs produce
   byte-identical output, on every supported OS and CPU:
   - the version is fixed by the same source, `Cargo.lock`, `rust-toolchain.toml` and release profile;
   - the inputs are the same render list, frame, size and options;
   - the output is the premultiplied RGBA8 buffer, and the exported image derived from it.

   This adopts spec 06 §6.7's SHOULD (across machines) as a project requirement, on top of its MUST (same machine and
   build). It is **not** a promise across versions: an upgrade may change pixels, but only through a reviewed golden
   update (item 5).
4. **What is pinned.** Any change to the following is a golden-gated change (item 5):
   - the exact versions of `vello_cpu` and its rendering dependencies (`vello_common`, `fearless_simd`, `kurbo`,
     `peniko`, `color`) in `Cargo.lock`, with the `vello_cpu` requirement written as `=0.3.x` when it moves into
     `tt-render`'s normal dependencies;
   - the Rust toolchain (`rust-toolchain.toml`, currently 1.98.1);
   - the release profile (`codegen-units`, `opt-level`, LTO);
   - **no** `target-cpu` / `target-feature` RUSTFLAGS in shipped builds;
   - the arithmetic configuration of item 1.
5. **What CI re-verifies, on every push.** The committed hashes and PNG goldens are checked on Windows, macOS and
   Linux in two processes each (`ci.yml` job `raster determinism`). The scalar configurations are also checked as a
   test (`c03_fixed_configurations`) in the Windows and macOS check jobs. From M3 C3.3:
   - the same goldens are rendered through the **shipped** headless binary's render command, not only through an
     example, so the product profile and linkage are what is verified;
   - each new renderer feature adds fixtures before it is used.

   A change that alters any hash fails CI. It needs a golden diff that a reviewer reads, with the cause stated, and a
   note in this ADR's history.
6. **Cost is accepted.** Scalar f32 costs about 1.4× the SIMD time on the performance scene. The reviewer's
   pixel-area extrapolation is about **55 ms per 1080p frame** (scalar f32), against 38 ms SIMD and 146 ms
   `ttraster`. It is an estimate, not a measurement, and no export budget exists in the spec. Scene density matters
   more than pixel count: the C0.3 canvas scenes take 82–1152 ms at 1080p with detected-SIMD u8 `vello_cpu` on one
   thread (`--canvas-bench`). Export
   latency, memory and cancellation budgets are set from M3 measurements, not from this figure.
7. **The interactive renderer is not bound by this.** It is GPU-based, and spec 06 §6.10 says "Determinism: not
   required". It may use whatever the GPU does (ADR-0007).
8. **The dependency exposure is accepted explicitly.** The locked `vello_cpu` graph has 35 crates, MIT/Apache-2.0,
   339,030 source lines and 1,727 lines containing `unsafe`, all in transitive crates; `vello_cpu` and
   `vello_common` themselves have none (`reviews/evidence/M0-C0.3/vello-dependency-audit.txt`, regenerated by
   `scripts/audit-vello-deps.py`).

## Consequences

- **C03-R1-04 is resolved by policy.** Final pixels never come from a detected SIMD level, so "SIMD dispatch changes
  pixels" no longer affects shipped output.
  - The reviewer test `c03_r1_04_simd_dispatch_must_not_change_final_pixels` stays as written and ignored. Its
    ignore reason still names the C0.4 ticket.
  - Retargeting that reason to this ADR is an edit to a reviewer test, so it is left to the orchestrator's ruling.
- **M3 C3.3 needs an adapter feature matrix before the renderer is adopted.** It covers:
  - transformed, mirrored, image and alpha paints;
  - inverted and subtree clips;
  - flatten and effect layers;
  - blur and feather;
  - colour-transform order (spec 06 §6.5);
  - AA seams;
  - the premultiplied → straight-alpha conversion for RGBA export (spec 09 §4.6), whose rounding is ours to define.

  Each item gets correctness and determinism goldens. Matching hashes on the C0.3 corpus do not establish correctness
  of untested upstream features.
- The u8 opacity limitation stays pinned by its test, so an upstream change is noticed.

### Still hypothesis (named experiments)

- **SSE4.2-only x86-64, Windows on arm64, macOS on x86-64.** The scalar path uses no CPU-specific intrinsics, so
  identity is expected, but it has not been run.
  - *Experiment R-1:* add `windows-11-arm` and an Intel macOS runner (or a user machine) to the raster job.
  - *Experiment R-2:* run the raster check on an SSE4.2-only CPU (an emulator such as Intel SDE is acceptable for
    this).
- **Future toolchains and LTO.** These are covered by the golden gate when they change; they are not established in
  advance.
- **Multithreaded scalar rendering.** *Experiment R-3:* add a `vello_cpu_f32_fallback_mt` configuration to the golden
  set before any use.
