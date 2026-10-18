// Executes the actual transport source with only the Tauri native invoke endpoint stubbed.
// Kept outside shipped modules: no IPC boundary exemption or product code modification.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require = createRequire(new URL('../../desktop/package.json', import.meta.url));
const ts = require('typescript');
const source = readFileSync(new URL('../../desktop/src/ipc/tauriTransport.ts', import.meta.url), 'utf8');
const js = ts.transpileModule(source, {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
const input = readFileSync(0);
const mode = process.argv[2] || 'arraybuffer';
const bytes = input.buffer.slice(input.byteOffset,input.byteOffset+input.byteLength);
const response = mode === 'array' ? [...input] : mode === 'view' ? new Uint8Array(bytes) : bytes;
const module = {exports:{}};
const stubRequire = (id) => {
  if(id === '@tauri-apps/api/core') return {invoke:async()=>response, isTauri:()=>true, Channel:class{}};
  if(id === '@tauri-apps/api/event') return {listen:async()=>()=>{}};
  throw new Error('unexpected import '+id);
};
new Function('require','module','exports',js)(stubRequire,module,module.exports);
const result = await module.exports.createTauriTransport().invokeBinary('spike_canvas_scene',{});
process.stdout.write(Buffer.from(result));
