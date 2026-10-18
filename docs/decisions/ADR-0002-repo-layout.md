# ADR-0002 — Repository layout and crate boundaries

Status: accepted (C0.1, 2026-10-17).

## Context

Plan §3.1 fixes a Cargo workspace of engine crates, two binaries and a Tauri desktop app with a TypeScript front end.
Two requirements constrain the layout: the engine must build and run with no UI dependency (11 §3), and nothing may
mutate the document outside the serialized executor (11 P2, plan §3.2).

## Decision

- **Cargo workspace** (`resolver = "3"`, edition 2024, toolchain pinned in `rust-toolchain.toml`):
  - `crates/{model, registry, executor, vector, colour, anim, fx, render, audio, lipsync, persist, io, mcp, engine}`,
    package names `tt-<dir>`. Each crate's `lib.rs` documents its responsibility, spec sections and milestone and
    exports `NAME`, `SPEC_SECTIONS`, `MILESTONE`.
  - Dependency direction: `model` ← `registry` ← `executor`; `model` ← `vector`, `colour`, `anim`, `audio`,
    `persist`; `anim` ← `fx`; those ← `render`; `render` + `persist` ← `io`; `audio` ← `lipsync`; everything ←
    `engine` (facade) ← `mcp`. Shells depend on `engine` (and `mcp`), never on document crates directly.
  - `bins/headless` → binary `toontown-headless` (`--version`, `--selftest`; `--mcp-stdio` and `--run` from M1).
  - `desktop/src-tauri` → package `tt-desktop`, binary `toontown`, bundle id `dev.toontown.app`.
- **pnpm workspace** with one package today, `desktop` (`@toontown/desktop`: React 19, dockview-react 8, Vite 8,
  TypeScript 6 strict, ESLint 10 strict-type-checked, Prettier, Vitest).
- Front-end structure: `src/ipc/` (the only path to the host), `src/ui-bindings/` (menu manifest), `src/layout/`,
  `src/panels/`, `src/status/`, `src/diagnostics/` (M0 measurement hooks), `src/spikes/` (C0.2/C0.3 spikes only).
- **Spikes** live only in `desktop/src/spikes/` and `crates/*/examples/`; code moves into product crates only after
  being rewritten with tests.
- Workspace lints: `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro` denied in non-test
  code (`clippy.toml` allows them in tests); `unsafe_code` denied (overridable per module with a justification);
  `missing_docs` warned and CI uses `-D warnings`.
- Model mutation boundary (enforced from M1): `tt-model` types expose no public mutators; mutation is possible only
  with a transaction capability that only `tt-executor` can construct, plus a CI lint against direct mutation from
  `engine` or `desktop`. M0 creates no document state, so no shortcut exists to remove later.

## Consequences

- The headless binary links 43 crates and no GUI/GPU library (checked in CI by `scripts/check-headless-deps.py`).
- The spec's literal `app --headless --mcp-stdio` form (11 §3): `toontown-headless` accepts `--headless` as a no-op
  for compatibility. Whether the desktop binary `toontown` should also accept `--headless` and delegate to the same
  code path is decided in M1 (C1.4), when the MCP server exists; recorded in the C0.1 handoff as an open question.
- Adding a crate means adding it to `tt-engine`'s component list (a unit test pins the list to plan §3.1).
