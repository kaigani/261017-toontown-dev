/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

import { ipcBoundary } from './boundary/vite-plugin.ts';

// Tauri expects a fixed dev port and must not clear its terminal output.
export default defineConfig(({ mode }) => ({
  // The IPC boundary is enforced at bundling time for dev, build and Vitest (boundary/vite-plugin.ts).
  plugins: [ipcBoundary({ allowTestSupport: mode === 'test' }), react()],
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
    setupFiles: ['src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    // Boundary tests write probe files under src/__probes__ and lint/bundle the tree; run files one at a time so
    // no test ever observes another test's probes.
    fileParallelism: false,
  },
}));
