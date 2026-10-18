import assert from 'node:assert/strict';
import { test } from 'node:test';
import { writeFileSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { ESLint } from '../../desktop/node_modules/eslint/lib/api.js';
import { build } from '../../desktop/node_modules/vite/dist/node/index.js';

const desktop = fileURLToPath(new URL('../../desktop/', import.meta.url));
const eslint = new ESLint({ cwd: desktop });
const direct = "import { invoke } from '@tauri-apps/api/core';\nexport const bypass = () => invoke<unknown>('app_info');\n";
const boundaryRules = new Set(['no-restricted-imports', 'no-restricted-syntax', 'no-restricted-globals', 'no-eval', 'no-new-func']);

function assertBoundary(result) {
  assert.ok(result);
  assert.equal(result.messages.some((m) => m.fatal), false, JSON.stringify(result.messages));
  assert.ok(result.messages.some((m) => boundaryRules.has(m.ruleId)), JSON.stringify(result.messages));
}

test('controls: variable imports, template imports, globals, eval and barrel exports are rejected', async () => {
  for (const source of [
    direct,
    'export const f = (someVar: string) => import(someVar);',
    'export const f = (x: string) => import(`@tauri-apps/${x}`);',
    "export const f = () => Reflect.get(globalThis, '__TAURI' + '__') as unknown;",
    "export const f = () => Reflect.get(window, '__TAURI_INTERNALS__') as unknown;",
    "export const f = new Function('return 1');",
    "export { invoke } from '@tauri-apps/api/core';",
  ]) {
    const [r] = await eslint.lintText(source, { filePath: desktop + 'src/ipc/hostApi.ts' });
    assertBoundary(r);
  }
});

for (const ext of ['mts', 'cts']) {
  test(`C01-R2-01: the boundary also covers Vite-bundleable .${ext} modules`, async () => {
    const file = desktop + `src/test/c01-r2-probe-${process.pid}.${ext}`;
    writeFileSync(file, direct, { flag: 'wx' });
    try {
      const bundle = await build({
        root: desktop, configFile: false, logLevel: 'silent',
        build: { write: false, minify: false, lib: { entry: file, formats: ['es'], fileName: 'probe' } },
      });
      const outputs = (Array.isArray(bundle) ? bundle : [bundle]).flatMap((b) => b.output ?? []);
      assert.ok(outputs.some((o) => o.type === 'chunk' && o.code.includes('app_info')), 'Vite must actually bundle the bypass');
      const [r] = await eslint.lintFiles([file]);
      assertBoundary(r);
    } finally { unlinkSync(file); }
  });
}

test('C01-R2-01: source comments cannot disable the host boundary', async () => {
  const [r] = await eslint.lintText('/* eslint-disable */\n' + direct, { filePath: desktop + 'src/lib/latency.ts' });
  assert.ok(r.errorCount + r.warningCount > 0, JSON.stringify(r));
});

test('C01-R2-01: ordinary UI code cannot invoke the raw transport instead of HostApi', async () => {
  const [r] = await eslint.lintText(
    "import { createTauriTransport } from '../ipc/tauriTransport';\nexport const bypass = () => createTauriTransport().invoke('app_info');\n",
    { filePath: desktop + 'src/lib/latency.ts' },
  );
  assertBoundary(r);
});

test('C01-R2-01: lint coverage follows frontend imports outside desktop/src', async () => {
  const file = desktop + `src/test/c01-r2-outside-entry-${process.pid}.ts`;
  writeFileSync(file, "export { outsideBypass } from '../../../tests/adversarial/fixtures/c01-r2-outside.mjs';\n", { flag: 'wx' });
  try {
    const bundle = await build({
      root: desktop, configFile: false, logLevel: 'silent',
      build: { write: false, minify: false, lib: { entry: file, formats: ['es'], fileName: 'probe' } },
    });
    const outputs = (Array.isArray(bundle) ? bundle : [bundle]).flatMap((b) => b.output ?? []);
    assert.ok(outputs.some((o) => o.type === 'chunk' && o.code.includes('app_info')));
    // The exact CLI glob (eslint .) must reject the reachable forbidden import, wherever it lives.
    // Fresh process: lintText above intentionally replaces source in TypeScript's in-memory project service.
    const cli = spawnSync(process.execPath, ['node_modules/eslint/bin/eslint.js', '.', '--max-warnings', '0', '--format', 'json'], {
      cwd: desktop, encoding: 'utf8', timeout: 30_000, maxBuffer: 8 * 1024 * 1024,
    });
    assert.ifError(cli.error);
    const results = JSON.parse(cli.stdout);
    assert.ok(results.some((r) => r.messages.some((m) => boundaryRules.has(m.ruleId))), 'eslint . accepts the entire tree with the outside import');
  } finally { unlinkSync(file); }
});
