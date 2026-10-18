/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import { ipcBoundary } from './boundary/vite-plugin.ts';

// Tauri expects a fixed dev port and must not clear its terminal output.
const TEST_SETUP = 'src/test/setup.ts';
const isTestEntry = (file: string) => file === TEST_SETUP || /^src\/.+\.test\.[cm]?[jt]sx?$/.test(file);

export default defineConfig(({ mode }) => ({
  // The IPC boundary is enforced at bundling time for dev, build and Vitest (boundary/vite-plugin.ts).
  plugins: [
    ...ipcBoundary({
      allowTestSupport: mode === 'test',
      ...(mode === 'test' ? { testEntries: isTestEntry } : {}),
    }),
    react(),
  ],
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
  },
}));
