// @vitest-environment node
/// <reference types="node" />
// Build-level proof of the IPC boundary (boundary/vite-plugin.ts; C01-R2-01): each bypass class is bundled with the
// project's REAL Vite config in production mode and must fail with an "IPC boundary" error, whatever the linter
// says. Positive controls prove that legitimate code still builds.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { build, createServer, type Plugin } from 'vite';
import { afterAll, describe, expect, it } from 'vitest';

import { ipcBoundary } from '../../boundary/vite-plugin';

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

const htmlFixture = (html: string): Plugin => ({
  name: 'test-html-fixture',
  transformIndexHtml: { order: 'pre', handler: () => html },
});

async function buildApp(plugins: Plugin[]): Promise<void> {
  await build({
    configFile: `${desktop}vite.config.ts`,
    root: desktop,
    mode: 'production',
    logLevel: 'silent',
    plugins,
    build: { write: false, minify: false, sourcemap: false },
  });
}

describe('index.html imports only src/main.tsx (C01-R3-02)', () => {
  it(
    'refuses an inline module importing the raw transport',
    async () => {
      const html =
        '<script type="module">import { createTauriTransport } from "/src/ipc/tauriTransport.ts"; window.b = createTauriTransport;</script>';
      await expect(buildApp([htmlFixture(html)])).rejects.toThrow(/IPC boundary/);
    },
    T,
  );

  it(
    'refuses an inline module importing anything but main.tsx, even the host API',
    async () => {
      const html =
        '<script type="module">import { createDefaultHostApi } from "/src/ipc/index.ts"; window.b = createDefaultHostApi;</script>';
      await expect(buildApp([htmlFixture(html)])).rejects.toThrow(/IPC boundary/);
    },
    T,
  );

  it(
    'builds the real index.html (positive control)',
    async () => {
      await expect(buildApp([])).resolves.toBeUndefined();
    },
    T,
  );
});

describe('emitted-chunk attribution (C01-R3-01)', () => {
  const [, scanner] = ipcBoundary({ allowTestSupport: false, desktopDir: desktop.slice(0, -1) });
  const hook = scanner?.renderChunk;
  const renderChunk = (typeof hook === 'function' ? hook : hook?.handler) as unknown as (
    this: unknown,
    code: string,
    chunk: unknown,
  ) => unknown;
  const ctx = {
    error(message: string): never {
      throw new Error(message);
    },
  };
  const app = `${desktop}src/App.tsx`;
  const transport = `${desktop}src/ipc/tauriTransport.ts`;
  const mention = "window['__TAURI_INTERNALS__'].invoke('x');";
  const chunk = (modules: Record<string, string>) => ({
    fileName: 'probe.js',
    modules: Object.fromEntries(Object.entries(modules).map(([id, code]) => [id, { code }])),
  });

  it('accepts mentions attributable to the transport', () => {
    expect(() => renderChunk.call(ctx, mention, chunk({ [transport]: mention }))).not.toThrow();
  });
  it('rejects a mention in any other module', () => {
    expect(() => renderChunk.call(ctx, mention, chunk({ [app]: mention }))).toThrow(/IPC boundary/);
  });
  it('rejects mentions in the chunk that no module accounts for', () => {
    expect(() => renderChunk.call(ctx, mention + mention, chunk({ [transport]: mention }))).toThrow(
      /IPC boundary/,
    );
  });
});

describe('dev server: placeholder re-resolution only for approved modules (C01-R3-02)', () => {
  it(
    'serves the real graph, and refuses a module never reached through an import edge',
    async () => {
      const server = await createServer({
        configFile: `${desktop}vite.config.ts`,
        root: desktop,
        mode: 'development',
        logLevel: 'silent',
        server: { port: 0, strictPort: false, ws: false },
      });
      try {
        await expect(server.transformRequest('/src/ipc/tauriTransport.ts')).rejects.toThrow(/IPC boundary/);
        expect((await server.transformRequest('/src/main.tsx'))?.code).toBeTruthy();
        expect((await server.transformRequest('/src/ipc/index.ts'))?.code).toBeTruthy();
        // Now reached through index.ts, the transport is an approved module.
        expect((await server.transformRequest('/src/ipc/tauriTransport.ts'))?.code).toBeTruthy();
      } finally {
        await server.close();
      }
    },
    T,
  );
});

describe('config selection is pinned (C01-R3-02)', () => {
  it('every vite/vitest script passes --config vite.config.ts and Tauri runs those scripts', () => {
    const pkg = JSON.parse(readFileSync(`${desktop}package.json`, 'utf8')) as {
      scripts: Record<string, string>;
    };
    const viteScripts = Object.entries(pkg.scripts).filter(([, cmd]) => /\b(vite|vitest)\b/.test(cmd));
    expect(viteScripts.map(([name]) => name).sort()).toEqual(['build', 'dev', 'preview', 'test']);
    for (const [name, cmd] of viteScripts) expect(cmd, name).toMatch(/--config vite\.config\.ts(\s|$)/);
    const tauri = JSON.parse(readFileSync(`${desktop}src-tauri/tauri.conf.json`, 'utf8')) as {
      build: { beforeDevCommand: string; beforeBuildCommand: string };
    };
    expect(tauri.build.beforeDevCommand).toBe('pnpm dev');
    expect(tauri.build.beforeBuildCommand).toBe('pnpm build');
  });
});
