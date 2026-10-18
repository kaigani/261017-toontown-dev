# ADR-0001 — Desktop shell: Tauri 2

Status: accepted (C0.1, 2026-10-17). Confirmation of the spikes' evidence follows in the C0.4 go/no-go report.

## Context

The application needs a desktop shell on Windows and macOS that hosts 21 dockable panels, native menus, dialogs,
single-instance behaviour and OS open-document events (spec 01), pressure/tilt tablet input (02 §6), and an
interactive canvas for 100 k-stroke drawings and 1 000-element scenes. The engine (document, commands, renderer,
MCP) must run without any UI framework in headless mode (11 §3), so the shell has to be a replaceable client of the
engine, not its owner (11 P2).

Options were compared in plan §2.2: Tauri 2, Qt 6, pure-Rust GUI toolkits and Flutter.

## Decision

- **Tauri 2** (WebView2 on Windows, WKWebView on macOS) is the shell. The Rust engine runs in-process inside the
  Tauri host; the TypeScript + React front end (dockview for docking) talks to the host only through the typed IPC
  module `desktop/src/ipc/` (transport interface + Tauri and mock implementations). ESLint forbids `@tauri-apps/*`
  imports anywhere else.
- **Electron is excluded outright by user decision (plan §6.7, 2026-10-17).** It is not a fallback and is not
  evaluated. `deny.toml` bans Electron bindings.
- Fallbacks if a Tauri spike fails (plan §2.3), in order: native `octotablet` tablet input in the Rust host streamed
  over a Tauri channel (C0.2); a native `wgpu` child surface for the canvas (C0.3). Only if those fail on **both**
  OSes does Qt 6 become a question for the user.
- Version: `tauri` 2.12.x / `@tauri-apps/cli` 2.12.x. Tauri 3 is in alpha and is not used.

## Consequences

- The UI is web technology: docking, forms, key-binding editor and preferences are straightforward; canvas
  throughput and tablet input are the risks C0.2 and C0.3 measure.
- Every host call crosses a process boundary (webview ↔ host). C0.1 measured the round trip on WebView2 at
  p50 0.70 ms / p95 0.80 ms for an empty ping and ~20 ms for a 1 MiB JSON echo, so bulk data must use binary IPC
  (`tauri::ipc::Response`, C0.3) rather than JSON.
- Menus are native (muda) and are built from a declarative manifest (`desktop/src/ui-bindings/menu.json`) whose
  items are command ids; the M1 parity test diffs it against the registry (11 §14.1).
- Platform webview differences (WKWebView vs Chromium) must be tested on both OSes; macOS checks are run by the user
  from scripts in `docs/manual-tests/`.
