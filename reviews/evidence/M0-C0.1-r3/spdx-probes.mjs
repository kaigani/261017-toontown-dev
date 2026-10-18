import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { satisfies, parse } from '../../../scripts/check-npm-licenses.mjs';
const invalid = ['MIT+ +', 'MIT WITH', 'WITH Classpath-exception-2.0', '()', '(MIT', 'MIT)', 'MIT OR (AND)', 'LicenseRef-', 'DocumentRef-:LicenseRef-x', 'MΙT', 'MIT OR ＭＩＴ', 'MIT++', 'MIT OR +'];
for (const s of invalid) {
  assert.equal(satisfies(s), false, s);
  assert.equal(satisfies(`MIT OR (${s})`), false, `hidden: ${s}`);
  console.log('reject', s);
}
for (const [s, expected] of [['mit', true], ['GPL-3.0', false], ['MIT OR GPL-3.0', true], ['MIT OR LicenseRef-private', true], ['MIT OR DocumentRef-local:LicenseRef-x', true]]) {
  assert.doesNotThrow(() => parse(s));
  assert.equal(satisfies(s), expected);
  console.log('valid operand/policy result', s, expected);
}
for (const [label, s, expected] of [
  ['10001 flat tokens', Array(5001).fill('MIT').join(' OR '), true],
  ['10001 flat tokens, bad last operand', Array(5000).fill('MIT').join(' OR ')+' OR Unknown', false],
  ['10001 nested tokens', '('.repeat(5000)+'MIT'+')'.repeat(5000), false],
]) {
  const start=performance.now(); const result=satisfies(s); const ms=performance.now()-start;
  assert.equal(result, expected); assert.ok(ms<2000);
  console.log(label, {result, ms});
}
const require=createRequire(new URL('../../../package.json',import.meta.url));
for (const p of ['spdx-license-ids','spdx-exceptions']) console.log('pinned list',p,require(`${p}/package.json`).version,require.resolve(`${p}/index.json`));
