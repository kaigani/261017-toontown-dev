// @vitest-environment node
/// <reference types="node" />
// Build-level proof of the IPC boundary (boundary/vite-plugin.ts; C01-R2-01): each bypass class is bundled with the
// project's REAL Vite config in production mode and must fail with an "IPC boundary" error, whatever the linter
// says. Positive controls prove that legitimate code still builds.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { build } from 'vite';
import { afterAll, describe, expect, it } from 'vitest';

const desktop = fileURLToPath(new URL('../../', import.meta.url));
const probes = `${desktop}src/__probes__/build-${String(process.pid)}/`;
const T = 120_000;

afterAll(() => {
  rmSync(probes, { recursive: true, force: true });
});

/** Bundles `code` as a library entry with the real config; resolves to the emitted code or rejects. */
async function bundle(name: string, code: string): Promise<string> {
  mkdirSync(probes, { recursive: true });
  const entry = `${probes}${name}`;
  writeFileSync(entry, code);
  const result = await build({
    configFile: `${desktop}vite.config.ts`,
    root: desktop,
    mode: 'production',
    logLevel: 'silent',
    build: {
      write: false,
      minify: false,
      sourcemap: false,
      lib: { entry, formats: ['es'], fileName: 'probe' },
    },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => ('output' in r ? r.output : []));
  return outputs.map((o) => (o.type === 'chunk' ? o.code : '')).join('\n');
}

const TAURI = ['@tauri-apps', 'api', 'core'].join('/');
const DIRECT = `import { invoke } from '${TAURI}';\nexport const b = () => invoke('app_info');\n`;
const INTERNALS = ['__TAURI', '_INTERNALS__'].join('');

// Probe entries live in src/__probes__/build-<pid>/, so `../../` is src/ and `../../../` is desktop/.
const BYPASSES: Record<string, [file: string, code: string]> = {
  'static import (.ts)': ['static.ts', DIRECT],
  'static import (.tsx)': ['static-tsx.tsx', DIRECT],
  'static import (.mts)': ['static-mts.mts', DIRECT],
  'static import (.cts)': ['static-cts.cts', DIRECT],
  'static import (.js)': ['static-js.js', DIRECT],
  'static import (.jsx)': ['static-jsx.jsx', DIRECT],
  'static import (.mjs)': ['static-mjs.mjs', DIRECT],
  'require (.cjs)': [
    'require.cjs',
    `const { invoke } = require('${TAURI}');\nmodule.exports = () => invoke('app_info');\n`,
  ],
  'lint suppressed with eslint-disable': ['suppressed.ts', `/* eslint-disable */\n${DIRECT}`],
  'dynamic import': [
    'dynamic.ts',
    `export const b = async () => (await import('${TAURI}')).invoke('app_info');\n`,
  ],
  're-export': ['reexport.ts', `export { invoke } from '${TAURI}';\n`],
  'import.meta.glob of the package': [
    'glob.ts',
    `export const m = import.meta.glob('/node_modules/@tauri-apps/api/*.js', { eager: true });\n`,
  ],
  'Tauri global': [
    'global.ts',
    `export const t = () => (window as unknown as Record<string, unknown>)['${INTERNALS}'];\n`,
  ],
  'raw transport import': [
    'raw.ts',
    `import { createTauriTransport } from '../../ipc/tauriTransport';\nexport const b = () => createTauriTransport().invoke('app_info');\n`,
  ],
  'raw transport dynamic import': [
    'raw-dynamic.ts',
    `export const b = async () => (await import('../../ipc/tauriTransport')).createTauriTransport();\n`,
  ],
  'module outside desktop (repo tests fixture)': [
    'outside-repo.ts',
    `export { outsideBypass } from '../../../../tests/adversarial/fixtures/c01-r2-outside.mts';\n`,
  ],
  'module outside src (desktop tooling)': [
    'outside-desktop.ts',
    `export * from '../../../boundary/eslint.js';\n`,
  ],
};

describe('IPC boundary at bundle time', () => {
  for (const [name, [file, code]] of Object.entries(BYPASSES)) {
    it(
      `refuses to bundle: ${name}`,
      async () => {
        await expect(bundle(file, code)).rejects.toThrow(/IPC boundary/);
      },
      T,
    );
  }

  it(
    'builds code that uses the validated host API (positive control)',
    async () => {
      const out = await bundle(
        'host-api.ts',
        `import { createDefaultHostApi } from '../../ipc';\nexport const b = () => createDefaultHostApi().appInfo();\n`,
      );
      // The legitimate path still reaches the transport through src/ipc/index.ts.
      expect(out).toContain('app_info');
    },
    T,
  );

  it(
    'builds plain code inside src (positive control)',
    async () => {
      const out = await bundle(
        'plain.ts',
        `import { summarize } from '../../lib/latency';\nexport const s = summarize([1]);\n`,
      );
      expect(out).toContain('summarize');
    },
    T,
  );
});
