import assert from 'node:assert/strict';
import { test } from 'node:test';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { ALLOWED, readAllowList, satisfies } from '../../scripts/check-npm-licenses.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));

test('C01-R2-03: a permitted OR branch must not hide malformed SPDX operands', () => {
  const malformed = [
    'MIT OR +', 'MIT OR :', 'MIT OR and', 'MIT OR Unknown-License',
    'MIT OR GPL-3.0-only WITH +', 'MIT OR LicenseRef-', 'MIT OR MIT++',
  ];
  const accepted = malformed.filter((expr) => satisfies(expr));
  assert.deepEqual(accepted, [], 'the entire expression must parse as valid SPDX before policy evaluation');
});

test('SPDX attack controls: precedence, nesting, exceptions, suffixes and malformed syntax', () => {
  for (const bad of [
    '', ' ', 'MIT or GPL-3.0-only', 'MIT and GPL-3.0-only', '(MIT', 'MIT)',
    'MIT AND', 'MIT OR', 'MIT\u200bOR Apache-2.0', 'UNKNOWN', 'LicenseRef-private',
    'MIT WITH LLVM-exception', 'GPL-2.0+', 'GPL-2.0-only WITH Classpath-exception-2.0',
    'MIT AND ((GPL-3.0-only AND Apache-2.0) OR (MIT AND AGPL-3.0-only)) AND BSD-3-Clause',
  ]) assert.equal(satisfies(bad), false, bad);
  for (const good of [
    '((MIT))', 'MIT OR GPL-3.0-only', 'MIT AND (GPL-3.0-only OR Apache-2.0)',
    '\tMIT\tAND\tApache-2.0\t', 'Apache-2.0+', 'MIT OR LicenseRef-private',
  ]) assert.equal(satisfies(good), true, good);
});

test('boolean evaluation agrees with independently constructed expressions at depth four', () => {
  let state = 0x12345678;
  const next = () => (state = (Math.imul(state, 1664525) + 1013904223) >>> 0);
  function expression(depth) {
    if (!depth) return next() & 1 ? ['MIT', true] : ['GPL-3.0-only', false];
    const [a, av] = expression(depth - 1);
    const [b, bv] = expression(depth - 1);
    const and = Boolean(next() & 8);
    return [`(${a} ${and ? 'AND' : 'OR'} ${b})`, and ? av && bv : av || bv];
  }
  for (let i = 0; i < 300; i++) {
    const [expr, expected] = expression(4);
    assert.equal(satisfies(expr), expected, expr);
  }
});

test('Rust config, JS runtime and the exact ADR list agree; imported gate follows an edited copy', async () => {
  const toml = readFileSync(resolve(root, 'deny.toml'), 'utf8');
  const adr = readFileSync(resolve(root, 'docs/decisions/ADR-0003-licensing-policy.md'), 'utf8');
  const list = /approved set is exactly:\s*\*\*([^*]+)\.\*\*/.exec(adr);
  assert.ok(list);
  assert.deepEqual([...ALLOWED].sort(), list[1].split(',').map((s) => s.trim()).sort());
  assert.deepEqual([...readAllowList(toml)].sort(), [...ALLOWED].sort());
  // All mutations remain under the authorized evidence folder. Never modify the actual deny.toml.
  const temp = resolve(root, 'reviews/evidence/M0-C0.1-r2/license-copy-' + process.pid);
  assert.ok(temp.startsWith(resolve(root, 'reviews/evidence/M0-C0.1-r2') + '/'.replace('/', process.platform === 'win32' ? '\\' : '/')));
  const copy = resolve(temp, 'scripts/check-npm-licenses.mjs');
  mkdirSync(dirname(copy), { recursive: true });
  try {
    copyFileSync(resolve(root, 'scripts/check-npm-licenses.mjs'), copy);
    writeFileSync(resolve(temp, 'deny.toml'), toml.replace('"MIT",', '"CC0-1.0",'));
    const gate = await import(pathToFileURL(copy).href);
    assert.equal(gate.satisfies('MIT'), false);
    assert.equal(gate.satisfies('CC0-1.0'), true);
    assert.equal(satisfies('MIT'), true);
    assert.equal(satisfies('CC0-1.0'), false);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});
