// Licence gate for the JavaScript that ships inside the app bundle (production dependencies of every workspace
// package). Same policy as deny.toml (ADR-0003): permissive licences only; no GPL, AGPL or LGPL. Development
// dependencies are build tools that are not distributed; they are listed for information only.
//
// Usage: node scripts/check-npm-licenses.mjs        (exit 1 on a violation)
import { execFileSync } from 'node:child_process';

const ALLOWED = new Set(['MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', 'Zlib', 'MPL-2.0', '0BSD']);

/** True when an SPDX expression is satisfied by the allow-list (OR: any side; AND: every side). */
export function satisfies(expression) {
  const expr = expression.trim().replace(/^\((.*)\)$/, '$1');
  if (/\sOR\s/.test(expr)) return expr.split(/\s+OR\s+/).some(satisfies);
  if (/\sAND\s/.test(expr)) return expr.split(/\s+AND\s+/).every(satisfies);
  return ALLOWED.has(expr);
}

function licenses(prodOnly) {
  const args = ['--dir', 'desktop', 'licenses', 'list', '--json', ...(prodOnly ? ['--prod'] : ['--dev'])];
  const out = execFileSync(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', args, {
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  return JSON.parse(out);
}

const violations = [];
let count = 0;
for (const [license, packages] of Object.entries(licenses(true))) {
  for (const pkg of packages) {
    count++;
    if (!satisfies(license)) violations.push(`${pkg.name}@${pkg.versions.join(',')}: ${license}`);
  }
}

const devSummary = Object.entries(licenses(false))
  .map(([license, packages]) => `${license} (${packages.length})`)
  .join(', ');

console.log(`production packages checked: ${count}`);
console.log(`development packages (not shipped, informational): ${devSummary}`);
if (violations.length > 0) {
  console.error(`licence violations in shipped JavaScript:\n  ${violations.join('\n  ')}`);
  process.exit(1);
}
console.log('licences ok');
