# `src/ipc` — the only path from the front end to the Rust host

- `transport.ts` — `HostTransport`: `invoke(command, args) → Promise<unknown>` and `listen(event, handler)`.
  Transports deal in `unknown`; nothing is trusted until `hostApi.ts` validates it.
- `tauriTransport.ts` — the Tauri implementation. **The only file allowed to load the Tauri packages, and only
  `index.ts` may import it.** Enforced by the bundler (`../../boundary/vite-plugin.ts`, dev/build/Vitest) and by
  the linter (`../../boundary/eslint.js`, every JS/TS extension, inline disables forbidden).
- `mockTransport.ts` — in-memory host for tests and for `pnpm dev` in a plain browser.
- `hostApi.ts` — the typed API UI code uses (`ping`, `appInfo`, `onMenuCommand`, M0 diagnostics). Every response
  and event is checked to be plain data, snapshotted, validated against its schema and only then returned; errors
  map to `HostError` (stable dotted `code`, 11 §10).
- `schema.ts`, `wireSchemas.ts` — the JSON-Schema-subset validator (own-property semantics only) and the wire
  schemas; replaced by registry-generated schemas in M1.
- `types.ts` — wire types mirroring the Rust side. Hand-written in M0; generated from the registry's JSON Schemas
  from M1.
- `context.ts` / `react.tsx` — `HostProvider` and `useHost()`.

Swapping the shell means writing another `HostTransport`; no UI code changes.
