# `src/ipc` — the only path from the front end to the Rust host

- `transport.ts` — `HostTransport`: `invoke(command, args) → Promise<unknown>` and `listen(event, handler)`.
  Transports deal in `unknown`; nothing is trusted until `hostApi.ts` validates it.
- `tauriTransport.ts` — the Tauri implementation. **The only file allowed to import `@tauri-apps/*`** (ESLint
  `no-restricted-imports`).
- `mockTransport.ts` — in-memory host for tests and for `pnpm dev` in a plain browser.
- `hostApi.ts` — the typed API UI code uses (`ping`, `appInfo`, `onMenuCommand`, M0 diagnostics), with response and
  event validation and error mapping to `HostError` (stable dotted `code`, 11 §10).
- `types.ts` — wire types mirroring the Rust side. Hand-written in M0; generated from the registry's JSON Schemas
  from M1.
- `context.ts` / `react.tsx` — `HostProvider` and `useHost()`.

Swapping the shell means writing another `HostTransport`; no UI code changes.
