// Tests of the SPDX evaluator in check-npm-licenses.mjs. Run: node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ALLOWED, parse, readAllowList, satisfies } from './check-npm-licenses.mjs';

const allow = new Set(['MIT', 'Apache-2.0', 'BSD-3-Clause', 'Apache-2.0 WITH LLVM-exception']);

test('single identifiers', () => {
  assert.equal(satisfies('MIT', allow), true);
  assert.equal(satisfies('GPL-3.0-only', allow), false);
  assert.equal(satisfies('LGPL-2.1-or-later', allow), false);
});

test('OR needs one allowed alternative, AND needs all', () => {
  assert.equal(satisfies('MIT OR GPL-3.0-only', allow), true);
  assert.equal(satisfies('GPL-3.0-only OR MIT', allow), true);
  assert.equal(satisfies('MIT AND GPL-3.0-only', allow), false);
  assert.equal(satisfies('MIT AND Apache-2.0', allow), true);
});

test('precedence: AND binds tighter than OR', () => {
  // = MIT OR (GPL AND Apache): satisfiable through MIT.
  assert.equal(satisfies('MIT OR GPL-3.0-only AND Apache-2.0', allow), true);
  // = (GPL AND MIT) OR AGPL: every alternative needs a forbidden licence.
  assert.equal(satisfies('GPL-3.0-only AND MIT OR AGPL-3.0-only', allow), false);
});

test('nested groups (C01-R1-01)', () => {
  assert.equal(satisfies('(MIT OR Apache-2.0) AND (GPL-3.0-only OR AGPL-3.0-only)', allow), false);
  assert.equal(satisfies('(MIT OR Apache-2.0) AND (GPL-3.0-only OR BSD-3-Clause)', allow), true);
  assert.equal(satisfies('((MIT))', allow), true);
  assert.equal(satisfies('(GPL-2.0-only OR (AGPL-3.0-only AND MIT))', allow), false);
  assert.deepEqual(parse('(MIT OR isc) AND Zlib'), {
    op: 'AND',
    terms: [{ op: 'OR', terms: [{ license: 'MIT' }, { license: 'ISC' }] }, { license: 'Zlib' }],
  });
});

test('WITH exceptions must be allowed as a whole', () => {
  assert.equal(satisfies('Apache-2.0 WITH LLVM-exception', allow), true);
  assert.equal(satisfies('GPL-2.0-only WITH Classpath-exception-2.0', allow), false);
  assert.equal(satisfies('MIT WITH Some-exception', allow), false);
});

test('or-later suffix', () => {
  assert.equal(satisfies('Apache-2.0+', allow), true);
  assert.equal(satisfies('GPL-2.0+', allow), false);
});

test('malformed and non-SPDX strings fail closed', () => {
  for (const bad of ['', '(MIT', 'MIT)', 'MIT OR', 'MIT or Apache-2.0', 'AND MIT', 'MIT*', 'SEE LICENSE IN x', 'UNKNOWN']) {
    assert.equal(satisfies(bad, allow), false, bad);
  }
});

test('every operand is validated, even behind an allowed OR branch (C01-R2-03)', () => {
  for (const bad of [
    'MIT OR +', 'MIT OR :', 'MIT OR and', 'MIT OR Unknown-License', 'MIT OR GPL-3.0-only WITH +',
    'MIT OR LicenseRef-', 'MIT OR MIT++', 'MIT OR GPL-2.0-only WITH Not-An-Exception', 'MIT OR LicenseRef-x WITH LLVM-exception',
    'MIT OR DocumentRef-:LicenseRef-x', 'MIT OR DocumentRef-a:', 'MIT OR +MIT', 'MIT OR MIT+ +',
  ]) {
    assert.equal(satisfies(bad, allow), false, bad);
  }
  assert.equal(satisfies('MIT OR LicenseRef-private', allow), true);
  assert.equal(satisfies('MIT OR DocumentRef-spdx-tool-1.2:LicenseRef-MIT-Style-2', allow), true);
  assert.equal(satisfies('mit', allow), true, 'identifiers match case-insensitively (Annex D)');
  assert.equal(satisfies('Apache-2.0 WITH llvm-exception', allow), true);
  assert.equal(satisfies('GPL-2.0 OR MIT', allow), true, 'deprecated identifiers are valid syntax');
});

test('allow-list is read from deny.toml', () => {
  assert.deepEqual(
    [...readAllowList('[licenses]\nversion = 2\nallow = [\n  "MIT", # c\n  "Zlib",\n]\n\n[bans]\nallow = ["x"]\n')],
    ['MIT', 'Zlib'],
  );
  assert.throws(() => readAllowList('[bans]\nallow = ["MIT"]\n'));
  assert.ok(ALLOWED.has('MIT') && ALLOWED.has('Unicode-3.0') && ALLOWED.has('0BSD'));
  assert.ok(![...ALLOWED].some((l) => /GPL/.test(l)), 'no GPL-family licence may be allowed');
});
