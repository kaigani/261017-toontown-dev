# Developer setup

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| rustup + Rust | 1.98.1 (pinned by `rust-toolchain.toml`) | `rustup toolchain install 1.98.1 --component rustfmt,clippy` |
| Windows: Visual Studio Build Tools 2022 | MSVC + Windows SDK | needed by Rust MSVC and Tauri |
| Windows: WebView2 runtime | evergreen | preinstalled on Windows 11 |
| macOS: Xcode command-line tools | current | `xcode-select --install` |
| Node.js | 22 LTS | |
| pnpm | 10.12.1 (pinned by `packageManager` in `package.json`) | `corepack enable` or install pnpm 10 |
| Python | 3.10+ | only for `scripts/*.py` (standard library only) |

## Global tools installed on the build machine

Recorded per plan §3.2 (no unrecorded global installs).

| Tool | How | Why |
|---|---|---|
| `cargo-deny` 0.20.2 | `cargo install --locked cargo-deny` (into `~/.cargo/bin`) | licence/advisory/source gate (`deny.toml`, ADR-0003) |
| rustup target `aarch64-apple-darwin` for toolchain 1.98.1 | `rustup target add aarch64-apple-darwin --toolchain 1.98.1` | attempted cross type-check of the macOS code (C0.2); it stops at C build scripts that need the macOS SDK, so the macOS CI runner is the real compile check |

The Tauri CLI is **not** global: it is the `@tauri-apps/cli` dev dependency of `desktop/` (`pnpm --dir desktop tauri …`).
Tauri downloads NSIS into its own cache on the first Windows bundle build.

## Everyday commands (repository root)

```sh
pnpm install --frozen-lockfile            # front-end dependencies
pnpm --dir desktop build                  # front end -> desktop/dist (needed before building the Rust host)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
pnpm lint && pnpm typecheck && pnpm test && pnpm format:check && pnpm licenses:check

pnpm --dir desktop tauri dev              # run the desktop app with hot reload
pnpm --dir desktop dev                    # front end only, in a browser, with the in-memory mock host
pnpm --dir desktop tauri build --bundles nsis   # Windows installer (macOS: --bundles app)

cargo run -p tt-headless -- --selftest    # headless engine self-test
python scripts/check-headless-deps.py target/release/toontown-headless.exe   # after cargo build --release -p tt-headless
target/release/toontown.exe --ipc-bench out.json   # automated IPC latency benchmark (M0 diagnostics)
target/release/toontown.exe --spike tablet --spike-log t.json   # C0.2 tablet spike page (add --spike-auto on Windows
                                                                 # for the synthetic-pen run; needs an uncovered desktop)
```

## Repository rules

- `spec/` is read-only. Spec problems go to `plan/spec-issues.md`.
- Line endings are LF everywhere (ADR-0004); do not let `core.autocrlf` rewrite files.
- Spikes only in `desktop/src/spikes/` and `crates/*/examples/`.
