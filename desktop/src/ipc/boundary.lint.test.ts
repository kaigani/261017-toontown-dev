// @vitest-environment node
/// <reference types="node" />
// Permanent positive and negative cases for the IPC boundary lint (eslint.ipc-boundary.js, C01-R1-02). Each case is
// linted with the project's real ESLint configuration as if it were a file inside src/.
import { fileURLToPath } from 'node:url';

import { ESLint } from 'eslint';
import { describe, expect, it } from 'vitest';

import { ipcBoundaryRules } from '../../eslint.ipc-boundary.js';

const desktop = fileURLToPath(new URL('../../', import.meta.url));
const eslint = new ESLint({ cwd: desktop });
const BOUNDARY_RULES = new Set(Object.keys(ipcBoundaryRules));

/** Lints `code` as the given src file and returns the boundary-rule violations. */
async function boundaryViolations(code: string, file = 'src/lib/latency.ts'): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath: `${desktop}${file}` });
  const fatal = result?.messages.filter((m) => m.fatal) ?? [];
  if (fatal.length > 0)
    throw new Error(`lint failed to parse the case: ${fatal.map((m) => m.message).join('; ')}`);
  return (result?.messages ?? [])
    .filter((m) => m.ruleId && BOUNDARY_RULES.has(m.ruleId))
    .map((m) => m.ruleId ?? '');
}

const TAURI = ['@tauri-apps', 'api', 'core'].join('/');
const GLOBAL = ['__TAURI', '__'].join('');
const INTERNALS = ['__TAURI', '_INTERNALS__'].join('');

const BYPASSES: Record<string, string> = {
  'static import': `import { invoke } from '${TAURI}';\nexport const f = () => invoke('app_info');\n`,
  'type-only import': `import type { InvokeArgs } from '${TAURI}';\nexport type A = InvokeArgs;\n`,
  're-export': `export { invoke } from '${TAURI}';\n`,
  'export star': `export * from '${TAURI}';\n`,
  'dynamic import': `export async function f() {\n  const m = await import('${TAURI}');\n  return m.invoke('app_info');\n}\n`,
  'dynamic import via template': `export async function f() {\n  return import(\`${TAURI}\`);\n}\n`,
  'dynamic import via variable': `const spec = '${TAURI}';\nexport const f = () => import(spec);\n`,
  'dynamic import via concatenation': `export const f = () => import('@tauri' + '-apps/api/core');\n`,
  require: `declare const require: (id: string) => unknown;\nexport const m = require('${TAURI}');\n`,
  'import.meta.glob': `export const m = import.meta.glob('/node_modules/@tauri*');
`,
  'import.meta.glob through a cast': `export const m = (import.meta as unknown as { glob(p: string): unknown }).glob('/node_modules/@tauri*');
`,
  'aliased import.meta': `const meta: unknown = import.meta;
export const m = meta;
`,
  'window global': `export const t = (window as unknown as Record<string, unknown>).${GLOBAL};\n`,
  'computed window global': `export const t = (window as unknown as Record<string, unknown>)['${INTERNALS}'];\n`,
  'bare global': `declare const ${INTERNALS}: unknown;\nexport const t = ${INTERNALS};\n`,
  'in operator': `export const has = '${INTERNALS}' in window;\n`,
  eval: `export const f = () => eval('1');\n`,
  'new Function': `export const f = new Function('return 1');\n`,
};

describe('IPC boundary lint', () => {
  for (const [name, code] of Object.entries(BYPASSES)) {
    it(`rejects ${name} outside src/ipc/tauriTransport.ts`, async () => {
      expect(await boundaryViolations(code)).not.toEqual([]);
    }, 60_000);
  }

  it('accepts the transport file itself', async () => {
    const code = `import { invoke } from '${TAURI}';\nexport const f = () => invoke('app_info');\n`;
    expect(await boundaryViolations(code, 'src/ipc/tauriTransport.ts')).toEqual([]);
  }, 60_000);

  it('accepts ordinary code that goes through the host API', async () => {
    const code = `import { useHost } from '../ipc';
export const f = () => useHost().appInfo();
export const lazy = () => import('../ipc');
export const dev = import.meta.env.DEV;
`;
    expect(await boundaryViolations(code)).toEqual([]);
  }, 60_000);

  it('accepts the real source tree', async () => {
    const results = await eslint.lintFiles(['src/**/*.{ts,tsx}']);
    const hits = results.flatMap((r) =>
      r.messages
        .filter((m) => m.ruleId && BOUNDARY_RULES.has(m.ruleId))
        .map((m) => `${r.filePath}: ${m.message}`),
    );
    expect(hits).toEqual([]);
  }, 120_000);
});
