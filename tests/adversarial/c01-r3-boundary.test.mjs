// Round-3 regressions. Real ESLint/Vite config; no product source/config edits.
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync, rmSync, realpathSync, symlinkSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { after, test } from 'node:test';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const desktop = path.join(repo, 'desktop');
const require = createRequire(path.join(desktop, 'package.json'));
const { ESLint } = require('eslint');
const { build, resolveConfig, createServer } = await import(pathToFileURL(require.resolve('vite')).href);
const eslint = new ESLint({ cwd: desktop });
const parent = path.join(desktop, 'src/__probes__');
mkdirSync(parent, { recursive: true });
const dir = mkdtempSync(path.join(parent, 'r3-boundary-'));
after(() => rmSync(dir, { recursive: true, force: true }));
const configFile = path.join(desktop, 'vite.config.ts');
const slash = (p) => p.replaceAll('\\', '/');
const outputs = (result) => (Array.isArray(result) ? result : [result]).flatMap((r) => r.output ?? []);
const chunks = (result) => outputs(result).filter((o) => o.type === 'chunk').map((o) => o.code).join('\n');
let sequence = 0;
async function bundle(code, extra = {}) {
  const entry = path.join(dir, `entry-${sequence++}.js`);
  writeFileSync(entry, code);
  return build({ root: desktop, configFile, mode: 'production', logLevel: 'silent', ...extra,
    build: { write: false, minify: false, sourcemap: false, lib: { entry, formats: ['es'] } } });
}
const escapedGlobal = String.raw`/* eslint no-restricted-syntax: off */
export const bypass = () => window['\u005f\u005fTAURI_INTERNALS__'].invoke('app_info');`;

test('C01-R3-01: inline eslint configuration cannot disable boundary rules', async () => {
  const source = "/* eslint no-restricted-imports: off, no-restricted-syntax: off */\n" +
    "import { invoke } from '@tauri-apps/api/core';\nexport const f = () => invoke('app_info');\n";
  const [result] = await eslint.lintText(source, { filePath: path.join(desktop, 'src/lib/latency.ts') });
  assert.ok(result.errorCount > 0, 'inline rule reconfiguration suppresses every diagnostic');
});

test('C01-R3-01: escaped static Tauri global cannot pass both lint and production build', async () => {
  const file = path.join(dir, 'escaped.js');
  writeFileSync(file, escapedGlobal);
  const [lint] = await eslint.lintFiles([file]);
  if (lint.errorCount > 0) return;
  let result;
  try { result = await bundle(escapedGlobal); }
  catch (error) { assert.match(String(error), /IPC boundary/); return; }
  const code = chunks(result);
  assert.match(code, /TAURI_INTERNALS__/); // Ensure the forbidden operation survived tree-shaking.
  console.log('escaped emitted code:', code);
  assert.fail('zero lint diagnostics and emitted a direct raw-host invoke');
});

test('C01-R3-02: index.html is not a privileged importer of the raw transport in production', async () => {
  // A test-only HTML transform supplies the fixture at the real entry path without editing index.html.
  await assert.rejects(build({ root: desktop, configFile, mode: 'production', logLevel: 'silent',
    plugins: [{ name: 'review-html-fixture', transformIndexHtml: { order: 'pre', handler() {
      return '<script type="module">import {createTauriTransport} from "/src/ipc/tauriTransport.ts";window.bypass=createTauriTransport;</script>';
    } } }], build: { write: false, minify: false, sourcemap: false } }).then((result) => {
      assert.match(chunks(result), /window\.bypass\s*=\s*createTauriTransport/);
    }), /IPC boundary/);
});

test('controls: eslint-disable next-line, block-line, and eslint-env cannot hide the import', async () => {
  const importLine = "import { invoke } from '@tauri-apps/api/core';";
  for (const source of [
    '// eslint-disable-next-line no-restricted-imports, no-restricted-syntax\n' + importLine,
    importLine + ' /* eslint-disable-line no-restricted-imports, no-restricted-syntax */',
    '/* eslint-env node */\n' + importLine,
  ]) {
    const [result] = await eslint.lintText(source + "\nexport const f = () => invoke('x');\n", { filePath: path.join(desktop, 'src/lib/latency.ts') });
    assert.ok(result.errorCount > 0, source);
  }
});

test('controls: pnpm nested paths, package symlinks, relative core.js and query suffixes are rejected', async () => {
  const core = realpathSync(require.resolve('@tauri-apps/api/core'));
  const logical = path.join(desktop, 'node_modules/@tauri-apps/api/core.js');
  for (const target of [core, logical, path.join(desktop, 'src/ipc/tauriTransport.ts')]) {
    for (const suffix of ['', '?review=1']) {
      const specifier = slash(path.relative(dir, target)) + suffix;
      const name = target.endsWith('tauriTransport.ts') ? 'createTauriTransport' : 'invoke';
      await assert.rejects(bundle(`export { ${name} } from ${JSON.stringify(specifier)};`), /IPC boundary/);
    }
  }
});

test('controls: aliases to resolved core and raw transport are rejected', async () => {
  for (const [target, name] of [[require.resolve('@tauri-apps/api/core'), 'invoke'], [path.join(desktop, 'src/ipc/tauriTransport.ts'), 'createTauriTransport']]) {
    await assert.rejects(bundle(`export { ${name} } from 'review-alias';`, { resolve: { alias: { 'review-alias': target } } }), /IPC boundary/);
  }
});

test('control: a directory symlink to ipc cannot bypass resolved transport identity', async () => {
  const link = path.join(dir, 'linked-ipc');
  symlinkSync(path.join(desktop, 'src/ipc'), link, process.platform === 'win32' ? 'junction' : 'dir');
  try { await assert.rejects(bundle("export {createTauriTransport} from './linked-ipc/tauriTransport.ts';"), /IPC boundary/); }
  finally { rmSync(link); }
});

test('controls: boundary is the first user plugin in serve/build/test modes; internal virtual ids stay usable', async () => {
  for (const [command, mode] of [['serve', 'development'], ['build', 'production'], ['serve', 'test']]) {
    const config = await resolveConfig({ root: desktop, configFile, mode }, command);
    const names = config.plugins.map((p) => p.name);
    const boundary = names.indexOf('toontown-ipc-boundary');
    assert.ok(boundary >= 0 && boundary < names.findIndex((n) => n.startsWith('vite:react')));
    console.log('plugin order', command, mode, names.slice(0, boundary + 2));
    const hook = config.plugins[boundary].resolveId;
    const resolveId = typeof hook === 'function' ? hook : hook.handler;
    assert.equal(await resolveId.call({}, '\0vite-internal', undefined, {}), null);
    // Synthetic resolved IDs exercise Windows backslashes independent of Vite's normal slash normalization.
    const target = path.join(desktop, 'src/ipc/tauriTransport.ts');
    await assert.rejects(resolveId.call({ resolve: async () => ({id:target+'?q'}), error: (message) => {throw new Error(message);} }, './raw', path.join(desktop,'src/App.tsx'), {}), /IPC boundary/);
  }
});

test('control: product imports of tests support are refused by the production build', async () => {
  await assert.rejects(bundle("export {outsideBypass} from '../../../../tests/adversarial/fixtures/c01-r2-outside.mts';"), /IPC boundary/);
});

test('positive control: validated host API still builds', async () => {
  const code = chunks(await bundle("import {createDefaultHostApi} from '../../ipc';export const call=()=>createDefaultHostApi().appInfo();"));
  assert.match(code, /app_info/);
});

test('test-support allowance is mode-wide, but product imports fail lint and production build', async () => {
  const source = "export {value} from '../../../../tests/adversarial/fixtures/c01-r3-plain.mjs';";
  assert.match(chunks(await bundle(source, {mode:'test'})), /value/);
  await assert.rejects(bundle(source), /IPC boundary/);
  const [lint] = await eslint.lintText(source, {filePath:path.join(dir,'support.js')});
  assert.ok(lint.errorCount > 0);
});

test('dev server serves the legitimate graph and rejects raw transport edges', async () => {
  const file = path.join(dir,'dev-raw.js');
  writeFileSync(file,"export {createTauriTransport} from '../../ipc/tauriTransport.ts';");
  const server=await createServer({root:desktop,configFile,logLevel:'silent',server:{port:0,strictPort:false}});
  try {
    await server.listen();
    assert.ok((await server.transformRequest('/src/main.tsx'))?.code);
    await assert.rejects(server.transformRequest('/'+slash(path.relative(desktop,file))), /IPC boundary/);
  } finally {await server.close();}
});

test('audit: alternate Vite config omits the boundary; CI needs fixed config selection', async () => {
  const alternate=path.join(dir,'other.config.mjs');
  writeFileSync(alternate,'export default {};\n');
  const config=await resolveConfig({root:desktop,configFile:alternate},'build');
  assert.equal(config.plugins.some((p)=>p.name==='toontown-ipc-boundary'),false);
  console.log('alternate config has no boundary; checked-in build/test scripts use auto-discovery, not --config');
});
