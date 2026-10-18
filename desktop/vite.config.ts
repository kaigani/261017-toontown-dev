/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ipcBoundary, transportChunk } from './boundary/vite-plugin.ts';

// Tauri expects a fixed dev port and must not clear its terminal output.
const TEST_SETUP = 'src/test/setup.ts';
const isTestEntry = (file: string) => file === TEST_SETUP || /^src\/.+\.test\.[cm]?[jt]sx?$/.test(file);
/** Build modes that include the M0 diagnostics (src/m0Diagnostics.d.ts); the default production build does not. */
const DIAGNOSTICS_MODES = ['diagnostics', 'test'];

export default defineConfig(({ mode }) => ({
  // The IPC boundary is enforced at bundling time for dev, build and Vitest (boundary/vite-plugin.ts).
  plugins: [
    ...ipcBoundary({
      allowTestSupport: mode === 'test',
      ...(mode === 'test' ? { testEntries: isTestEntry } : {}),
    }),
    react(),
  ],
  define: { __TT_M0_DIAGNOSTICS__: JSON.stringify(DIAGNOSTICS_MODES.includes(mode)) },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    // WebView2 (Chromium) and WKWebView on macOS 12+ (Safari 15+).
    target: ['es2022', 'chrome110', 'safari15'],
    sourcemap: true,
    rolldownOptions: {
      output: {
        // The transport and every @tauri-apps module form one dedicated chunk, so the boundary's final-output checks
        // can attribute Tauri code after minification (boundary/vite-plugin.ts, boundary/check-dist.mjs).
        manualChunks: transportChunk(path.dirname(fileURLToPath(import.meta.url))),
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: [TEST_SETUP],
    include: ['src/**/*.test.{ts,tsx}'],
    // Boundary tests write probe files under src/__probes__ and lint/bundle the tree; run files one at a time so
    // no test ever observes another test's probes.
    fileParallelism: false,
    // Vitest's persistent transform cache would let a cached module skip the IPC boundary's import-edge and
    // transform checks (boundary/vite-plugin.ts), so every run transforms and checks every module afresh.
    fsModuleCache: false,
    // Some reviewer regressions compile Rust and run Python (tests/adversarial); on CI runners that exceeds Vitest's
    // 5 s default. A longer per-test limit changes no assertion.
    testTimeout: 60_000,
  },
}));
