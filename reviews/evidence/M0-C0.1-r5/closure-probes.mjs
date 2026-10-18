// Real-production-config probes. Product files are never edited; fixture transforms are explicit below.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import path from 'node:path';

const out = path.dirname(fileURLToPath(import.meta.url));
const desktop = path.resolve(out, '../../../desktop');
const require = createRequire(path.join(desktop, 'package.json'));
const { ESLint } = require('eslint');
const { build, resolveConfig } = await import(pathToFileURL(require.resolve('vite')));
const eslint = new ESLint({ cwd: desktop });
const configFile = path.join(desktop, 'vite.config.ts');
const parent = path.join(desktop, 'src/__probes__');
mkdirSync(parent, { recursive: true });
const dir = mkdtempSync(path.join(parent, 'r5-'));
const rows = [];
const slash = p => p.replaceAll('\\', '/');
const summarize = f => {
  if (!f.code) return f;
  const { code, ...meta } = f;
  return { ...meta, bytes: Buffer.byteLength(code), sha256: createHash('sha256').update(code).digest('hex'),
    literalTauri: /__TAURI/.test(code), ...(code.length < 3000 ? { code } : {
      relevantExcerpts: [...code.matchAll(/.{0,100}(?:reviewBypass|\.reduce\(|TextDecoder|\.invoke\("app_info"\)|export \{|import\{).{0,160}/g)].slice(-12).map(m => m[0]),
    }) };
};
const record = (name, data) => {
  if (data.generated) data.generated = data.generated.map(summarize);
  if (data.written) data.written = data.written.map(summarize);
  rows.push({ name, ...data }); console.log(JSON.stringify(rows.at(-1)));
};
const index = path.join(desktop, 'src/ipc/index.ts');
const indexSource = readFileSync(index, 'utf8');
async function lint(code, filePath = path.join(dir, 'lint.js')) {
  const [r] = await eslint.lintText(code, { filePath });
  return { errors: r.errorCount, messages: r.messages.map(m => ({ rule: m.ruleId, message: m.message })) };
}
async function bundle(name, code, { input, indexAppend, app = false } = {}) {
  const entry = path.join(dir, name + '.js');
  writeFileSync(entry, code);
  const dist = path.join(out, 'outputs', name, 'dist');
  const generated = [];
  const plugins = [{
    name: 'review-observe-final-output', enforce: 'pre',
    transform(source, id) {
      if (indexAppend && slash(id) === slash(index)) return source + '\n' + indexAppend;
      if (app && slash(id) === slash(path.join(desktop, 'src/main.tsx'))) {
        return source + `\nimport { bypass } from ${JSON.stringify('./' + slash(path.relative(path.join(desktop, 'src'), entry)))}; window.reviewBypass = bypass;`;
      }
    },
    generateBundle: { order: 'pre', handler(_options, files) {
      for (const f of Object.values(files)) generated.push(f.type === 'chunk'
        ? { fileName: f.fileName, name: f.name, code: f.code, moduleIds: f.moduleIds, exports: f.exports, imports: f.imports, dynamicImports: f.dynamicImports }
        : { fileName: f.fileName, asset: true });
    } },
  }];
  let accepted = false, error;
  try {
    await build({ root: desktop, configFile, logLevel: 'silent', mode: 'production', plugins,
      build: { write: true, outDir: dist, emptyOutDir: true,
        ...(app ? {} : { lib: { entry, formats: ['es'], fileName: 'probe' } }),
        ...(input ? { rolldownOptions: { input } } : {}),
      } });
    accepted = true;
  } catch (e) { error = String(e.message); }
  const written = generated.filter(f => !f.asset).map(f => ({ fileName: f.fileName,
    exists: existsSync(path.join(dist, f.fileName)),
    ...(existsSync(path.join(dist, f.fileName)) ? { code: readFileSync(path.join(dist, f.fileName), 'utf8') } : {}),
  }));
  let runtime;
  const candidate = written.find(f => f.exists && f.code.includes('bypass'));
  if (accepted && candidate && !app) {
    const old = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const calls = [];
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { __TAURI_INTERNALS__: {
      invoke(command, args) { calls.push({ command, args }); return Promise.resolve('raw-host'); },
      metadata: { currentWindow: { label: 'review' } },
    } } });
    try {
      const mod = await import(pathToFileURL(path.join(dist, candidate.fileName)).href + '?review=' + Date.now());
      runtime = { result: await mod.bypass(), calls };
    } catch (e) { runtime = { error: String(e), calls }; }
    finally { if (old) Object.defineProperty(globalThis, 'window', old); else delete globalThis.window; }
  }
  return { lint: await lint(code, entry), accepted, error, generated, written, runtime };
}
try {
  const chars = [...'__TAURI_INTERNALS__'];
  const codes = chars.map(c => c.charCodeAt(0)).join(',');
  const variants = {
    slice: "'__TAURI_INTERNALS__'.slice(0)",
    substring: "'X__TAURI_INTERNALS__'.substring(1)",
    uppercase: "'__tauri_internals__'.toUpperCase()",
    reduce: `${JSON.stringify(chars)}.reduce((a,b)=>a+b,'')`,
    uri: "decodeURIComponent('%5F%5FTAURI_INTERNALS__')",
    json: String.raw`JSON.parse('"\\u005f\\u005fTAURI_INTERNALS__"')`,
    jsonSingleEscape: String.raw`JSON.parse('"\u005f\u005fTAURI_INTERNALS__"')`,
    decoder: `new TextDecoder().decode(new Uint8Array([${codes}]))`,
    numeric: `('__'+(${parseInt('tauri', 36)}).toString(36).toUpperCase()+'_INTERNALS__')`,
    object: "({a:'__TAU',b:'RI_INTERNALS__'}).a + ({a:'__TAU',b:'RI_INTERNALS__'}).b",
    charcodes: `String.fromCharCode(${codes})`,
  };
  for (const [name, expr] of Object.entries(variants)) record('constant-' + name,
    await bundle('constant-' + name, `export const bypass=()=>window[${expr}].invoke('app_info');`));
  record('const-object', await bundle('const-object', "const obj={a:'__TAU',b:'RI_INTERNALS__'}; export const bypass=()=>window[obj.a+obj.b].invoke('app_info');"));
  for (const name of ['reduce', 'decoder', 'numeric', 'json', 'object']) record('app-' + name,
    await bundle('app-' + name, `export const bypass=()=>window[${variants[name]}].invoke('app_info');`, { app: true }));

  record('dynamic-raw', await bundle('dynamic-raw', "export const bypass=()=>import('../../ipc/tauriTransport');"));
  record('renamed-ui-reexport', await bundle('renamed-ui-reexport', "export {createTauriTransport as x} from '../../ipc/tauriTransport';"));
  record('chunk-name-comment', await bundle('chunk-name-comment', "export const bypass=()=>import(/* webpackChunkName: 'tauri-transport' */ '../../ipc/tauriTransport');"));
  const first = path.join(dir, 'first.js'), second = path.join(dir, 'second.js');
  writeFileSync(first, "import {createDefaultHostApi} from '../../ipc'; export const first=()=>createDefaultHostApi().appInfo();");
  writeFileSync(second, "import {createDefaultHostApi} from '../../ipc'; export const second=()=>createDefaultHostApi().appInfo();");
  record('two-index-importers', await bundle('two-index-importers', 'export const noop=1;', { input: { first, second } }));
  const renamed = "export {createTauriTransport as x} from './tauriTransport';";
  record('renamed-index-reexport', { indexLint: await lint(indexSource + '\n' + renamed, index),
    ...await bundle('renamed-index-reexport', "import {x} from '../../ipc'; export const bypass=()=>x().invoke('app_info');", { indexAppend: renamed }) });

  const workers = {
    worker: "export const start=()=>new Worker('/src/w.js');",
    importWorker: "import W from './w?worker'; export const start=()=>new W();",
    inlineWorker: "import W from './w?worker&inline'; export const start=()=>new W();",
    serviceWorker: "export const start=()=>navigator.serviceWorker.register('/src/w.js');",
    sharedWorker: "export const start=()=>new SharedWorker('/src/w.js');",
    worklet: "export const start=()=>new Worklet();",
    paintWorklet: "export const start=()=>CSS.paintWorklet.addModule('/src/w.js');",
    audioElementWorklet: "export const start=()=>new Audio().audioWorklet;",
    audioContextWorklet: "export const start=()=>new AudioContext().audioWorklet.addModule('/src/w.js');",
  };
  for (const [name, code] of Object.entries(workers)) record('worker-' + name, await lint(code));
  const assembly = {
    fromCharCode: 'String.fromCharCode(95)', fromCodePoint: 'String.fromCodePoint(95)', raw: 'String.raw`a`',
    atob: "atob('YQ==')", windowAtob: "window.atob('YQ==')", concat: "'a'.concat('b')", join: "['a','b'].join('')", template: "`${'a'}${'b'}`",
  };
  for (const [name, expr] of Object.entries(assembly)) for (const [location, file] of [
    ['shipped', path.join(dir, 'lint.js')], ['test', path.join(desktop, 'src/ipc/hostApi.test.ts')], ['transport', path.join(desktop, 'src/ipc/tauriTransport.ts')],
  ]) record('assembly-' + name + '-' + location, await lint('export const value=' + expr + ';', file));
  for (const ext of ['ts', 'tsx', 'mts', 'cts', 'js', 'jsx', 'mjs', 'cjs']) {
    const c = await eslint.calculateConfigForFile(path.join(dir, 'lint.' + ext));
    record('noInlineConfig-' + ext, { noInlineConfig: c.linterOptions.noInlineConfig });
    assert.equal(c.linterOptions.noInlineConfig, true);
  }
  record('inline-disable', await lint("/* eslint no-restricted-properties: off */\nexport const n=String.fromCharCode(95);"));
  const c = await resolveConfig({ root: desktop, configFile }, 'build');
  const scanner = c.plugins.find(p => p.name === 'toontown-ipc-boundary-scan');
  const getHook = name => typeof scanner[name] === 'function' ? scanner[name] : scanner[name].handler;
  const bad = 'window.__TAURI_INTERNALS__.invoke("app_info");';
  const asset = { type: 'asset', fileName: 'bypass.mjs', source: bad };
  for (const name of ['generateBundle', 'writeBundle']) {
    const d = path.join(out, 'synthetic', name, 'dist'); mkdirSync(d, { recursive: true }); writeFileSync(path.join(d, asset.fileName), bad);
    try { getHook(name).call({ error(m) { throw Error(m); } }, { dir: d }, { [asset.fileName]: asset }); record('hook-' + name, { rejected: false }); }
    catch (e) { record('hook-' + name, { rejected: true, error: e.message }); }
  }
  for (const ext of ['js', 'mjs', 'cjs', 'html', 'js.map']) {
    const d = path.join(out, 'synthetic', ext, 'dist'); mkdirSync(d, { recursive: true });
    writeFileSync(path.join(d, 'tauri-transport-control.js'), bad);
    writeFileSync(path.join(d, 'bypass.' + ext), ext === 'html' ? `<script>${bad}</script>` : bad);
    const result = spawnSync(process.execPath, [path.join(desktop, 'boundary/check-dist.mjs'), d], { encoding: 'utf8' });
    record('dist-' + ext, { exit: result.status, stdout: result.stdout, stderr: result.stderr });
    assert.equal(result.status, ext === 'js.map' ? 0 : 1);
  }
} finally {
  writeFileSync(path.join(out, 'closure-probes.json'), JSON.stringify(rows, null, 2) + '\n');
  // Only this mkdtemp-created child beneath this worktree is removed.
  assert.ok(path.resolve(dir).startsWith(path.resolve(parent) + path.sep));
  rmSync(dir, { recursive: true, force: true });
}
