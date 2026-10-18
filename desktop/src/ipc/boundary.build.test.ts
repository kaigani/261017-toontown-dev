// @vitest-environment node
/// <reference types="node" />
// Build-level proof of the IPC boundary (boundary/vite-plugin.ts; C01-R2-01): each bypass class is bundled with the
// project's REAL Vite config in production mode and must fail with an "IPC boundary" error, whatever the linter
// says. Positive controls prove that legitimate code still builds.
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { ESLint } from 'eslint';
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

/** Bundles with the config's default minification (the folding the R4 regression relies on). */
async function bundleMinified(name: string, code: string): Promise<string> {
  mkdirSync(`${probes}${name.split('/').slice(0, -1).join('/')}`, { recursive: true });
  const entry = `${probes}${name}`;
  writeFileSync(entry, code);
  const result = await build({
    configFile: `${desktop}vite.config.ts`,
    root: desktop,
    mode: 'production',
    logLevel: 'silent',
    build: { write: false, sourcemap: false, lib: { entry, formats: ['es'], fileName: 'probe' } },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => ('output' in r ? r.output : []));
  return outputs.map((o) => (o.type === 'chunk' ? o.code : '')).join('\n');
}

// C01-R4-01: Tauri names assembled from constant fragments. Every form must fail the build, minified or not.
const g = (expr: string) => `export const b = () => window[${expr}].invoke('app_info');\n`;
const ASSEMBLED: Record<string, string> = {
  'String.fromCharCode': g('String.fromCharCode(95,95,84,65,85,82,73,95,73,78,84,69,82,78,65,76,83,95,95)'),
  concat: g("'__TAU'.concat('RI_INTERNALS__')"),
  'array join': g("['__TAU', 'RI_INTERNALS__'].join('')"),
  'template pieces': g("`${'_'}${'_'}TAURI_INTERNALS__`"),
  atob: g("atob('X19UQVVSSV9JTlRFUk5BTFNfXw==')"),
};

describe('string-assembled Tauri names (C01-R4-01)', () => {
  for (const [name, code] of Object.entries(ASSEMBLED)) {
    const slug = name.replace(/\W+/g, '-');
    it(
      `refuses ${name} (unminified build)`,
      async () => {
        await expect(bundle(`asm-${slug}.js`, code)).rejects.toThrow(/IPC boundary/);
      },
      T,
    );
    it(
      `refuses ${name} (minified build)`,
      async () => {
        await expect(bundleMinified(`asm-min-${slug}.js`, code)).rejects.toThrow(/IPC boundary/);
      },
      T,
    );
  }
});

describe('final-output checks on synthetic bundles (generateBundle)', () => {
  const [, scanner] = ipcBoundary({ allowTestSupport: false, desktopDir: desktop.slice(0, -1) });
  const hook = scanner?.generateBundle;
  const generateBundle = (typeof hook === 'function' ? hook : hook?.handler) as unknown as (
    this: unknown,
    options: unknown,
    bundle: Record<string, unknown>,
  ) => unknown;
  const ctx = {
    error(message: string): never {
      throw new Error(message);
    },
  };
  const transportId = `${desktop}src/ipc/tauriTransport.ts`;
  const indexId = `${desktop}src/ipc/index.ts`;
  const tauriId = `${desktop}node_modules/@tauri-apps/api/core.js`;
  interface FakeChunk {
    type: 'chunk';
    fileName: string;
    name: string;
    code: string;
    moduleIds: string[];
    exports: string[];
    imports: string[];
    dynamicImports: string[];
  }
  const chunk = (fileName: string, name: string, code: string, moduleIds: string[]): FakeChunk => ({
    type: 'chunk',
    fileName,
    name,
    code,
    moduleIds,
    exports: [],
    imports: [],
    dynamicImports: [],
  });
  const TRANSPORT_FILE_NAME = 'assets/tauri-transport-x.js';
  const ok = () => {
    const transport = chunk(
      TRANSPORT_FILE_NAME,
      'tauri-transport',
      'window.__TAURI_INTERNALS__.invoke("x")',
      [transportId, tauriId],
    );
    transport.exports = ['a', 'b'];
    const index = chunk('assets/index-x.js', 'index', 'import{a,b}from"./tauri-transport-x.js";', [indexId]);
    index.imports = [TRANSPORT_FILE_NAME];
    return {
      transport,
      index,
      bundle: { [TRANSPORT_FILE_NAME]: transport, 'assets/index-x.js': index } as Record<string, unknown>,
    };
  };
  const run = (bundle: Record<string, unknown>) => () => generateBundle.call(ctx, {}, bundle);

  it('accepts the expected layout', () => {
    expect(run(ok().bundle)).not.toThrow();
  });
  it('rejects a minified Tauri invoke in another chunk', () => {
    const b = ok();
    b.index.code += 'window.__TAURI_INTERNALS__.invoke("app_info")';
    expect(run(b.bundle)).toThrow(/final output/);
  });
  it('rejects disguised forms in another chunk or asset', () => {
    for (const code of [
      'window["\\x5f\\x5fTAURI_INTERNALS__"]',
      'String.fromCharCode(95,95,84,65,85,82,73)',
    ]) {
      const b = ok();
      b.index.code += code;
      expect(run(b.bundle), code).toThrow(/final output/);
    }
    const b = ok();
    b.bundle['index.html'] = {
      type: 'asset',
      fileName: 'index.html',
      source: '<script>window.__tauri__</script>',
    };
    expect(run(b.bundle)).toThrow(/final output/);
  });
  it('rejects Tauri modules outside the transport chunk and foreign modules inside it', () => {
    const outside = ok();
    outside.index.moduleIds.push(tauriId);
    expect(run(outside.bundle)).toThrow(/outside tauri-transport/);
    const inside = ok();
    inside.transport.moduleIds.push(`${desktop}src/App.tsx`);
    expect(run(inside.bundle)).toThrow(/also contains/);
  });
  it('rejects extra transport exports and importers other than the ipc/index chunk', () => {
    const exportsMore = ok();
    exportsMore.transport.exports = ['a', 'b', 'invoke'];
    expect(run(exportsMore.bundle)).toThrow(/exports 3 bindings/);
    const other = ok();
    const app = chunk('assets/other-x.js', 'other', '', [`${desktop}src/App.tsx`]);
    app.imports = [TRANSPORT_FILE_NAME];
    other.bundle['assets/other-x.js'] = app;
    expect(run(other.bundle)).toThrow(/does not contain src\/ipc\/index\.ts/);
  });
});

describe('standalone dist check (boundary/check-dist.mjs)', () => {
  const check = (files: Record<string, string>): number => {
    mkdirSync(probes, { recursive: true });
    const dir = mkdtempSync(`${probes}dist-`);
    for (const [name, content] of Object.entries(files)) {
      mkdirSync(`${dir}/${name.split('/').slice(0, -1).join('/')}`, { recursive: true });
      writeFileSync(`${dir}/${name}`, content);
    }
    try {
      execFileSync(process.execPath, [`${desktop}boundary/check-dist.mjs`, dir], { stdio: 'pipe' });
      return 0;
    } catch (e: unknown) {
      return (e as { status?: number }).status ?? -1;
    }
  };
  const transport = { 'assets/tauri-transport-a1.js': 'window.__TAURI_INTERNALS__.invoke("x")' };

  it('passes a clean dist', () => {
    expect(check({ ...transport, 'assets/index-a1.js': 'console.log(1)', 'index.html': '<p>ok</p>' })).toBe(
      0,
    );
  });
  it('fails on any mention outside the transport chunk, in any detected form', () => {
    for (const bad of [
      'window.__TAURI_INTERNALS__.invoke("app_info")',
      'window["\\u005f\\u005fTAURI_INTERNALS__"]',
      'String.fromCharCode(95,95,84,65,85,82,73,95,73,78,84,69,82,78,65,76,83,95,95)',
      'atob("X19UQVVSSV9JTlRFUk5BTFNfXw==")',
      'import"@TAURI-APPS/api"',
    ]) {
      expect(check({ ...transport, 'assets/index-a1.js': bad }), bad).toBe(1);
    }
    expect(check({ ...transport, 'index.html': '<script>window.__TAURI__</script>' })).toBe(1);
  });
  it('fails when the transport chunk is missing (plugins or manualChunks disabled)', () => {
    expect(check({ 'assets/index-a1.js': 'console.log(1)' })).toBe(1);
  });
});

describe('workers are refused (C01-R4-02)', () => {
  it(
    'a worker-importing entry fails lint, and its Tauri-importing worker fails the build',
    async () => {
      mkdirSync(`${probes}worker/`, { recursive: true });
      writeFileSync(`${probes}worker/w.js`, `import { invoke } from '${TAURI}';\ninvoke('app_info');\n`);
      const entryCode =
        "export const start = () => new Worker(new URL('./w.js', import.meta.url), { type: 'module' });\n";
      writeFileSync(`${probes}worker/entry.js`, entryCode);
      const [lint] = await new ESLint({ cwd: desktop }).lintFiles([`${probes}worker/entry.js`]);
      expect(lint?.errorCount ?? 0).toBeGreaterThan(0);
      await expect(bundleMinified('worker/entry.js', entryCode)).rejects.toThrow(/IPC boundary/);
    },
    T,
  );
});
