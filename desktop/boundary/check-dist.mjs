// Post-build IPC boundary check on the files actually written to disk (C01-R4-01). Runs after `vite build` in the
// `build` script, independently of any Vite plugin, so it still holds if someone disables plugins or changes the
// Vite config. Fails when any emitted .js/.mjs/.cjs/.html file other than the dedicated `tauri-transport` chunk
// mentions the Tauri globals or package names in any form ./scan.mjs detects (after minification).
//
// Usage: node boundary/check-dist.mjs [dist]
import { existsSync } from 'node:fs';
import path from 'node:path';

import { scanDist } from './scan.mjs';

const dir = path.resolve(process.argv[2] ?? 'dist');
if (!existsSync(dir)) {
  console.error(`IPC boundary (dist check): ${dir} does not exist; run vite build first`);
  process.exit(2);
}
const { problems, transportFiles } = scanDist(dir);
for (const p of problems)
  console.error(`IPC boundary (dist check): ${p.file}: ${p.hits.slice(0, 5).join(', ')}`);
if (transportFiles !== 1) {
  console.error(
    `IPC boundary (dist check): expected exactly one tauri-transport chunk, found ${transportFiles}`,
  );
}
if (problems.length > 0 || transportFiles !== 1) process.exit(1);
console.log(
  `IPC boundary (dist check): ok (${path.relative(process.cwd(), dir) || '.'}; Tauri code only in tauri-transport)`,
);
