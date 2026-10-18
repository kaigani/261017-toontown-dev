// @vitest-environment node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';

it('C03-R2-01: analyzer validates point/verb counts and workload for every seed', () => {
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const result = spawnSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['-I', 'tests/adversarial/c03-r2-tooling.py'],
    { cwd: root, encoding: 'utf8' },
  );
  expect(result.status, result.stdout + result.stderr).toBe(0);
});
