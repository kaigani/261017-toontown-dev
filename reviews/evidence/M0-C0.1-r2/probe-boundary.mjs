import { ESLint } from '../../../desktop/node_modules/eslint/lib/api.js';
import { writeFileSync, unlinkSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const desktop = fileURLToPath(new URL('../../../desktop/', import.meta.url));
const eslint = new ESLint({ cwd: desktop });
const direct = "import { invoke } from '@tauri-apps/api/core';\nexport const bypass = () => invoke<unknown>('app_info');\n";
const cases = {
  direct,
  variable: "export const bypass = (someVar: string) => import(someVar);",
  template: "export const bypass = (x: string) => import(`@tauri-apps/${x}`);",
  concatGlobal: "export const bypass = () => Reflect.get(globalThis, '__TAURI' + '__') as unknown;",
  reflect: "export const bypass = () => Reflect.get(window, '__TAURI_INTERNALS__') as unknown;",
  variableGlobal: "export const bypass = (key: string) => Reflect.get(window, key) as unknown;",
  newFunction: "export const bypass = new Function('return 1');",
  disable: '/* eslint-disable */\n' + direct,
  transport: "import { createTauriTransport } from '../ipc/tauriTransport';\nexport const bypass = () => createTauriTransport().invoke('app_info');\n",
  barrel: "export { invoke } from '@tauri-apps/api/core';",
};
for (const [name, code] of Object.entries(cases)) {
  const [r] = await eslint.lintText(code, { filePath: desktop + 'src/lib/latency.ts' });
  console.log(JSON.stringify({ name, errors: r.errorCount, warnings: r.warningCount, messages: r.messages }));
}
for (const ext of ['ts', 'tsx', 'mts', 'cts']) {
  const file = desktop + 'src/test/c01-r2-extension-probe.' + ext;
  try {
    writeFileSync(file, direct, { flag: 'wx' });
    const [r] = await eslint.lintFiles([file]);
    console.log(JSON.stringify({ name: ext, errors: r.errorCount, warnings: r.warningCount, messages: r.messages }));
  } finally { unlinkSync(file); }
}
for (const file of ['src/ipc/hostApi.ts', 'vite.config.ts', 'eslint.ipc-boundary.js']) {
  const [r] = await eslint.lintText(direct.replace('<unknown>', ''), { filePath: desktop + file });
  console.log(JSON.stringify({ name: file, errors: r.errorCount, warnings: r.warningCount, messages: r.messages }));
}
