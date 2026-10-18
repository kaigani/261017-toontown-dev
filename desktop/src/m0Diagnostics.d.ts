/**
 * Compile-time switch for the M0 diagnostics (IPC benchmark, spike pages and their host commands; C0.4). `true` in
 * the `diagnostics` build mode (`pnpm build:diagnostics`, used by `src-tauri/tauri.diagnostics.conf.json`) and in
 * tests; `false` in the default build, whose bundle therefore contains no spike page and no diagnostic command name.
 * Defined in vite.config.ts.
 */
declare const __TT_M0_DIAGNOSTICS__: boolean;
