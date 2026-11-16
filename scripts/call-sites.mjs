// The parity gate's call-site extraction from the command line (C43B-R2S-01): the command ids a source calls, found by
// `desktop/src/test-support/callSites.ts` (the TypeScript compiler's syntax tree for TS and TSX, a lexer for Rust), the
// same module the parity test uses. Prints a JSON array.
//
// Usage: node scripts/call-sites.mjs [--as <repository path>] <file | ->
//   `--as` gives the path the source is judged as (product code or not, TS or Rust); `-` reads the source from stdin.

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

// `typescript` is the front end's dependency: resolved from desktop/.
const require = createRequire(new URL('../desktop/package.json', import.meta.url));
const ts = require('typescript');

const source = readFileSync(new URL('../desktop/src/test-support/callSites.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true },
}).outputText;
const mod = { exports: {} };
new Function('require', 'module', 'exports', compiled)(require, mod, mod.exports);
const { callSitesOf } = mod.exports;

const args = process.argv.slice(2);
let as = null;
if (args[0] === '--as') {
  as = args[1];
  args.splice(0, 2);
}
const input = args[0];
if (!input) {
  process.stderr.write('usage: node scripts/call-sites.mjs [--as <repository path>] <file | ->\n');
  process.exit(2);
}
const text = input === '-' ? readFileSync(0, 'utf8') : readFileSync(input, 'utf8');
const path = (as ?? input).replaceAll('\\', '/');
process.stdout.write(`${JSON.stringify(callSitesOf(path, text))}\n`);
