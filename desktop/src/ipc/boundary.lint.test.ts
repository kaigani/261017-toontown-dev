// @vitest-environment node
/// <reference types="node" />
// Permanent positive and negative cases for the IPC boundary lint (boundary/eslint.js; C01-R1-02, C01-R2-01). Each
// case is linted with the project's real ESLint configuration as a file inside the desktop package.
import { execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ESLint } from 'eslint';
import { afterAll, describe, expect, it } from 'vitest';

import { EXTENSIONS, ipcBoundaryRules, MAX_SRC_DEPTH } from '../../boundary/eslint.js';

const desktop = fileURLToPath(new URL('../../', import.meta.url));
const probes = `${desktop}src/__probes__/lint-${String(process.pid)}/`;
const eslint = new ESLint({ cwd: desktop });
const BOUNDARY_RULES = new Set(Object.keys(ipcBoundaryRules));
const COMMENTS = '@eslint-community/eslint-comments/';

afterAll(() => {
  rmSync(probes, { recursive: true, force: true });
});

function boundaryHits(result: ESLint.LintResult | undefined): string[] {
  const fatal = result?.messages.filter((m) => m.fatal) ?? [];
  if (fatal.length > 0)
    throw new Error(`lint failed to parse the case: ${fatal.map((m) => m.message).join('; ')}`);
  return (result?.messages ?? [])
    .map((m) => m.ruleId ?? '')
    .filter((id) => BOUNDARY_RULES.has(id) || id.startsWith(COMMENTS));
}

/** Lints `code` as if it were the given (existing) file and returns the boundary-rule violations. */
async function lintAs(code: string, file = 'src/lib/latency.ts'): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath: `${desktop}${file}` });
  return boundaryHits(result);
}

/** Writes `code` to a new probe file under src/__probes__/ and lints it from disk. */
async function lintFile(relative: string, code: string): Promise<string[]> {
  const file = `${probes}${relative}`;
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, code);
  const [result] = await eslint.lintFiles([file]);
  return boundaryHits(result);
}

const TAURI = ['@tauri-apps', 'api', 'core'].join('/');
const GLOBAL = ['__TAURI', '__'].join('');
const INTERNALS = ['__TAURI', '_INTERNALS__'].join('');
const DIRECT = `import { invoke } from '${TAURI}';\nexport const f = () => invoke('app_info');\n`;
const T = 120_000;

const BYPASSES: Record<string, string> = {
  'static import': DIRECT,
  'type-only import': `import type { InvokeArgs } from '${TAURI}';\nexport type A = InvokeArgs;\n`,
  're-export': `export { invoke } from '${TAURI}';\n`,
  'export star': `export * from '${TAURI}';\n`,
  'dynamic import': `export async function f() {\n  const m = await import('${TAURI}');\n  return m.invoke('app_info');\n}\n`,
  'dynamic import via template': `export async function f() {\n  return import(\`${TAURI}\`);\n}\n`,
  'dynamic import via variable': `const spec = '${TAURI}';\nexport const f = () => import(spec);\n`,
  'dynamic import via concatenation': `export const f = () => import('@tauri' + '-apps/api/core');\n`,
  require: `declare const require: (id: string) => unknown;\nexport const m = require('${TAURI}');\n`,
  'import.meta.glob': `export const m = import.meta.glob('/node_modules/@tauri*');\n`,
  'import.meta.glob through a cast': `export const m = (import.meta as unknown as { glob(p: string): unknown }).glob('/node_modules/@tauri*');\n`,
  'aliased import.meta': `const meta: unknown = import.meta;\nexport const m = meta;\n`,
  'window global': `export const t = (window as unknown as Record<string, unknown>).${GLOBAL};\n`,
  'computed window global': `export const t = (window as unknown as Record<string, unknown>)['${INTERNALS}'];\n`,
  'bare global': `declare const ${INTERNALS}: unknown;\nexport const t = ${INTERNALS};\n`,
  'in operator': `export const has = '${INTERNALS}' in window;\n`,
  eval: `export const f = () => eval('1');\n`,
  'new Function': `export const f = new Function('return 1');\n`,
  'raw transport (static)': `import { createTauriTransport } from '../ipc/tauriTransport';\nexport const f = () => createTauriTransport().invoke('app_info');\n`,
  'raw transport (dynamic)': `export const f = async () => (await import('../ipc/tauriTransport')).createTauriTransport();\n`,
  'raw transport (re-export)': `export * from '../ipc/tauriTransport';\n`,
  'raw transport (with extension)': `export { createTauriTransport } from '../ipc/tauriTransport.ts';\n`,
  'absolute specifier': `export * from '/src/ipc';\n`,
  'drive specifier': `export * from 'C:/src/ipc';\n`,
  'file: specifier': `export * from 'file:///src/ipc';\n`,
  'backslash specifier': `export * from '..\\\\ipc';\n`,
  'non-canonical relative': `export * from '../ipc/../ipc';\n`,
  'bare parent': `export * from '..';\n`,
  'outside src (desktop root)': `export * from '../../vite.config';\n`,
  'outside desktop (repo tests)': `export * from '../../../tests/adversarial/fixtures/c01-r2-outside.mts';\n`,
};

const DISABLES: Record<string, string> = {
  'blanket block disable': `/* eslint-disable */\n${DIRECT}`,
  'blanket line disable': `import { invoke } from '${TAURI}'; // eslint-disable-line\nexport const f = () => invoke('x');\n`,
  'next-line disable of the rule': `// eslint-disable-next-line no-restricted-imports\n${DIRECT}`,
  'block disable of several boundary rules': `/* eslint-disable no-restricted-imports, no-restricted-syntax */\n${DIRECT}`,
  'disable of the comments rule itself': `/* eslint-disable ${COMMENTS}no-restricted-disable */\n${DIRECT}`,
  'disable of no-eval': `// eslint-disable-next-line no-eval\nexport const f = () => eval('1');\n`,
};

describe('IPC boundary lint: rejected forms', () => {
  for (const [name, code] of Object.entries(BYPASSES)) {
    it(
      `rejects ${name} in ordinary src code`,
      async () => {
        expect(await lintAs(code)).not.toEqual([]);
      },
      T,
    );
  }

  for (const [name, code] of Object.entries(DISABLES)) {
    it(
      `rejects ${name}`,
      async () => {
        // The disable either is reported itself (eslint-comments) or fails to silence the boundary; lint fails.
        const hits = await lintAs(code);
        expect(hits).not.toEqual([]);
        if (!name.includes('comments rule'))
          expect(
            hits.some((id) => id.startsWith(COMMENTS)),
            hits.join(','),
          ).toBe(true);
      },
      T,
    );
  }

  for (const ext of EXTENSIONS) {
    it(
      `covers .${ext} files`,
      async () => {
        const code =
          ext === 'cjs'
            ? `const { invoke } = require('${TAURI}');\nmodule.exports = () => invoke('app_info');\n`
            : DIRECT;
        // Distinct base names: TypeScript keeps only one of probe.ts / probe.tsx in a folder.
        expect(await lintFile(`ext/probe-${ext}.${ext}`, code)).not.toEqual([]);
      },
      T,
    );
  }

  it(
    'rejects the raw transport inside src/ipc too (only index.ts may import it)',
    async () => {
      expect(await lintAs(`export * from './tauriTransport';\n`, 'src/ipc/hostApi.ts')).not.toEqual([]);
    },
    T,
  );

  it(
    'rejects climbing out of src/ at any depth',
    async () => {
      expect(await lintAs(`export * from '../vite.config';\n`, 'src/App.tsx')).not.toEqual([]);
      expect(await lintFile('a/b/deep.ts', `export * from '../../../../../vite.config';\n`)).not.toEqual([]);
    },
    T,
  );

  it(
    'test files may reach only tests/ and boundary/ outside src/',
    async () => {
      expect(await lintAs(`export * from '../../vite.config';\n`, 'src/ipc/hostApi.test.ts')).not.toEqual([]);
      expect(await lintAs(`export * from '../../../crates/x';\n`, 'src/ipc/hostApi.test.ts')).not.toEqual([]);
      expect(await lintAs(`export * from '../../../../x';\n`, 'src/ipc/hostApi.test.ts')).not.toEqual([]);
    },
    T,
  );
});

const ASSEMBLY_AND_WORKERS: Record<string, string> = {
  'String.fromCharCode': 'export const n = String.fromCharCode(95, 95);\n',
  'String.fromCodePoint': 'export const n = String.fromCodePoint(95);\n',
  'String.raw': 'export const n = String.raw`a`;\n',
  'concat on a literal': "export const n = 'a'.concat('b');\n",
  'concat on a template': 'export const n = `a`.concat(`b`);\n',
  'join on an array literal': "export const n = ['a', 'b'].join('');\n",
  'template with literal pieces': "export const n = `${'_'}${'_'}x`;\n",
  atob: "export const n = atob('eA==');\n",
  'window.atob': "export const n = window.atob('eA==');\n",
  Worker: "export const w = () => new Worker('w.js');\n",
  SharedWorker: "export const w = () => new SharedWorker('w.js');\n",
  'window.Worker': "export const w = () => new window.Worker('w.js');\n",
  'navigator.serviceWorker': 'export const w = () => navigator.serviceWorker;\n',
  'new URL(…, import.meta.url)': "export const u = new URL('./w.js', import.meta.url);\n",
  '?worker import': "export { default } from './w?worker';\n",
};

describe('IPC boundary lint: string assembly and workers (C01-R4-01/02)', () => {
  for (const [name, code] of Object.entries(ASSEMBLY_AND_WORKERS)) {
    it(
      `rejects ${name} in shipped code`,
      async () => {
        expect(await lintAs(code)).not.toEqual([]);
      },
      T,
    );
  }

  it(
    'allows ordinary join, templates with variables and these APIs in tests',
    async () => {
      const ordinary = 'export const f = (xs: string[], v: string) => [xs.join(","), `a${v}`];\n';
      expect(await lintAs(ordinary)).toEqual([]);
      const inTest = "export const n = ['a', 'b'].join('') + String.fromCharCode(95) + atob('eA==');\n";
      expect(await lintAs(inTest, 'src/ipc/hostApi.test.ts')).toEqual([]);
    },
    T,
  );
});

describe('IPC boundary lint: accepted forms', () => {
  it(
    'the transport file may import Tauri',
    async () => {
      expect(await lintAs(DIRECT, 'src/ipc/tauriTransport.ts')).toEqual([]);
    },
    T,
  );

  it(
    'src/ipc/index.ts may import the transport',
    async () => {
      const code = `import { createTauriTransport } from './tauriTransport';\nexport const t = createTauriTransport;\n`;
      expect(await lintAs(code, 'src/ipc/index.ts')).toEqual([]);
    },
    T,
  );

  it(
    'ordinary code using the host API, relative imports inside src/, and import.meta.env',
    async () => {
      const code = `import { useHost } from '../ipc';\nexport { HostError } from '../ipc/hostApi';\nexport const f = () => useHost().appInfo();\nexport const lazy = () => import('../ipc');\nexport const dev = import.meta.env.DEV;\n`;
      expect(await lintAs(code)).toEqual([]);
      expect(await lintAs(`export * from './ipc';\nexport * from '.';\n`, 'src/App.tsx')).toEqual([]);
      expect(await lintFile('a/b/deep-ok.ts', `export * from '../../../../ipc';\n`)).toEqual([]);
    },
    T,
  );

  it(
    'test files may import repository test helpers and the boundary definitions',
    async () => {
      const code = `export * from '../../../tests/adversarial/frontend-cases';\nexport * from '../../boundary/eslint.js';\n`;
      expect(await lintAs(code, 'src/ipc/hostApi.test.ts')).toEqual([]);
    },
    T,
  );

  it(
    'the committed source tree is clean and no deeper than the per-depth rules cover',
    async () => {
      const tracked = execFileSync('git', ['ls-files', '--', 'src', 'boundary', '*.ts', '*.js'], {
        cwd: desktop,
        encoding: 'utf8',
      })
        .split('\n')
        .filter((f) => new RegExp(`\\.(${EXTENSIONS.join('|')})$`).test(f));
      for (const f of tracked.filter((x) => x.startsWith('src/'))) {
        expect(f.split('/').length - 2, f).toBeLessThanOrEqual(MAX_SRC_DEPTH);
      }
      const results = await eslint.lintFiles(tracked.map((f) => `${desktop}${f}`));
      const hits = results.flatMap((r) => boundaryHits(r).map((id) => `${r.filePath}: ${id}`));
      expect(hits).toEqual([]);
    },
    T,
  );
});

describe('public surface of src/ipc', () => {
  it('does not export the raw transport', async () => {
    const names = Object.keys(await import('./index'));
    expect(names).not.toContain('createTauriTransport');
    expect(names).not.toContain('isTauriRuntime');
    for (const n of names) expect(n.toLowerCase()).not.toContain('tauri');
  });
});
