// Scoped R3-01 closure regression: scan the final emitted code, including late constant folding.
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { test } from 'node:test';

const desktop = fileURLToPath(new URL('../../desktop/', import.meta.url));
const require = createRequire(path.join(desktop, 'package.json'));
const { ESLint } = require('eslint');
const { build } = await import(pathToFileURL(require.resolve('vite')).href);

test('C01-R4-01: late constant folding cannot emit an unattributed raw-host invoke', async () => {
  const parent = path.join(desktop, 'src/__probes__');
  mkdirSync(parent, { recursive: true });
  const dir = mkdtempSync(path.join(parent, 'r4-fold-'));
  try {
    // Only literal integer arguments; no runtime input or variables. The production compiler folds this call.
    const source = 'export const bypass = () => window[String.fromCharCode(95,95,84,65,85,82,73,95,73,78,84,69,82,78,65,76,83,95,95)].invoke("app_info");\n';
    const entry = path.join(dir, 'probe.js');
    writeFileSync(entry, source);
    const [lint] = await new ESLint({ cwd: desktop }).lintFiles([entry]);
    console.log('lint diagnostics:', lint.messages);
    let result;
    try {
      result = await build({
        root: desktop, configFile: path.join(desktop, 'vite.config.ts'), mode: 'production', logLevel: 'silent',
        build: { write: false, sourcemap: false, lib: { entry, formats: ['es'], fileName: 'probe' } },
      });
    } catch (error) {
      assert.match(String(error), /IPC boundary/);
      return;
    }
    const chunks = (Array.isArray(result) ? result : [result]).flatMap(r => r.output ?? []).filter(o => o.type === 'chunk');
    assert.equal(chunks.length, 1);
    const code = chunks[0].code;
    assert.match(code, /__TAURI_INTERNALS__/);
    assert.doesNotMatch(code, /String\.fromCharCode/);
    console.log('final emitted code:', code);
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const calls = [];
    Object.defineProperty(globalThis, 'window', {
      configurable: true, value: { __TAURI_INTERNALS__: { invoke(command) { calls.push(command); return 'raw-host'; } } },
    });
    try {
      const module = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
      assert.equal(module.bypass(), 'raw-host');
      assert.deepEqual(calls, ['app_info']);
    } finally {
      if (previous) Object.defineProperty(globalThis, 'window', previous);
      else delete globalThis.window;
    }
    assert.fail('production build emitted and executed an unattributed direct raw-host invoke after the chunk scan');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
