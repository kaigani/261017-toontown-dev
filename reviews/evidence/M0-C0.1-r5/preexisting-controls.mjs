// Establish whether nonblocking edge cases were introduced by the R4 fix.
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { readFileSync, writeFileSync, mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const out = path.dirname(fileURLToPath(import.meta.url));
const desktop = path.resolve(out, '../../../desktop');
const require = createRequire(path.join(desktop, 'package.json'));
const ts = require('typescript');
const { ESLint } = require('eslint');
const viteUrl = pathToFileURL(require.resolve('vite')).href;
const { build } = await import(viteUrl);
const oldText = file => execFileSync('git', ['show', 'aae694d:' + file], { cwd: desktop, encoding: 'utf8' });
const dataUrl = code => 'data:text/javascript;base64,' + Buffer.from(code).toString('base64');
const oldRulesUrl = dataUrl(oldText('desktop/boundary/eslint.js'));
const oldPluginSource = oldText('desktop/boundary/vite-plugin.ts')
  .replace("'./eslint.js'", JSON.stringify(oldRulesUrl)).replace("'vite'", JSON.stringify(viteUrl));
const oldPluginUrl = dataUrl(ts.transpileModule(oldPluginSource, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText);
const oldPlugin = await import(oldPluginUrl);
const comments = require('@eslint-community/eslint-plugin-eslint-comments');
const oldRules = await import(oldRulesUrl);
const oldLint = new ESLint({ cwd: desktop, overrideConfig: oldRules.ipcBoundaryConfig(comments.default ?? comments) });
const parent = path.join(desktop, 'src/__probes__'); mkdirSync(parent, { recursive: true });
const dir = mkdtempSync(path.join(parent, 'r5-prior-'));
const rows = [];
try {
  for (const [name, code] of [
    ['paintWorklet', "export const start=()=>CSS.paintWorklet.addModule('/src/w.js');"],
    ['audioWorklet', "export const start=()=>new AudioContext().audioWorklet.addModule('/src/w.js');"],
  ]) {
    const [r] = await oldLint.lintText(code, { filePath: path.join(dir, 'prior.js') });
    rows.push({ name, priorRevision: 'aae694d', lintErrors: r.errorCount, messages: r.messages });
  }
  const entry = path.join(dir, 'reexport.js');
  writeFileSync(entry, "import {x} from '../../ipc'; export const bypass=()=>x().invoke('app_info');");
  const result = await build({ root: desktop, configFile: false, logLevel: 'silent', mode: 'production',
    plugins: [{ name: 'prior-index-reexport-fixture', enforce: 'pre', transform(code, id) {
      if (id.replaceAll('\\', '/') === path.join(desktop, 'src/ipc/index.ts').replaceAll('\\', '/'))
        return code + "\nexport {createTauriTransport as x} from './tauriTransport';";
    } }, ...oldPlugin.ipcBoundary({ allowTestSupport: false, desktopDir: desktop })],
    build: { write: false, sourcemap: false, target: ['es2022', 'chrome110', 'safari15'], lib: { entry, formats: ['es'], fileName: 'prior' } },
  });
  const chunks = (Array.isArray(result) ? result : [result]).flatMap(r => r.output).filter(f => f.type === 'chunk');
  const code = chunks.map(f => f.code).join('\n');
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls = [];
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { __TAURI_INTERNALS__: { invoke(command) { calls.push(command); return Promise.resolve('raw-host'); } } } });
  try {
    const mod = await import(dataUrl(code));
    rows.push({ name: 'renamed-index-reexport', priorRevision: 'aae694d', accepted: true, result: await mod.bypass(), calls });
    assert.deepEqual(calls, ['app_info']);
  } finally { if (previous) Object.defineProperty(globalThis, 'window', previous); else delete globalThis.window; }
} finally {
  writeFileSync(path.join(out, 'preexisting-controls.json'), JSON.stringify(rows, null, 2) + '\n');
  assert.ok(path.resolve(dir).startsWith(path.resolve(parent) + path.sep));
  rmSync(dir, { recursive: true, force: true });
}
console.log(JSON.stringify(rows, null, 2));
