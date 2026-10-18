# C0.1 adversarial regressions

These tests assert the required behavior and intentionally fail at the reviewed commit `24a2562`.
They must remain after the builder fixes the findings. No product implementation is changed by this review.

From the checkout root, after `pnpm install --frozen-lockfile` and `pnpm --dir desktop build`:

```text
cargo test --workspace --locked
pnpm test
```

- `bins/headless/Cargo.toml` registers `c01.rs` as a Rust integration test. Its Node/Python checks use the
  documented development prerequisites and the existing desktop dependencies; no global installs are needed.
- `desktop/src/test/adversarial.test.ts` registers `frontend-cases.ts` with the existing Vitest suite.
- The ESLint regression creates one temporary `desktop/src/review-boundary-probe.ts` and removes it in `finally`.
  Do not run a separate lint/typecheck concurrently with that regression.
- `wire_engine_info_field_names_match_typescript` and the Rust UTF-8 limit test are passing controls.
- Expected review result: Rust adversarial target **5 failing / 2 passing**; Vitest **8 failing / 22 baseline passing**.

Optional Windows runtime reproduction after the release builds:

```text
python tests/adversarial/collect_runtime_evidence.py
rustc --edition 2024 tests/adversarial/fixtures/graphics_import.rs -o target/review/graphics-import.exe
python scripts/check-headless-deps.py target/review/graphics-import.exe
```

Create `target/review/` first for the last two commands. The graphics fixture retains a DirectComposition import;
it never calls that API, and the reproduction only inspects the executable. The current proof incorrectly passes it.
The runtime collector writes the measurements under `reviews/evidence/M0-C0.1-r1/` and launches the real app for its
automatic benchmark. It is a Windows measurement helper, not a cross-platform CI test.
