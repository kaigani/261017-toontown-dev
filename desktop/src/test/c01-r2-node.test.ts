// @vitest-environment node
/// <reference types="node" />
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { it } from 'vitest';

const root = fileURLToPath(new URL('../../../', import.meta.url));
for (const file of ['c01-r2-boundary.test.mjs', 'c01-r2-licenses.test.mjs']) {
  it(`round-2 permanent node regressions: ${file}`, () => {
    execFileSync(process.execPath, ['--test', `tests/adversarial/${file}`], {
      cwd: root,
      encoding: 'utf8',
      timeout: 90_000,
      stdio: 'pipe',
    });
  }, 100_000);
}
