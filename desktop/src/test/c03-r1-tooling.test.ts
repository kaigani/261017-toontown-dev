// @vitest-environment node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';

it('C03-R1: incomplete canvas evidence and missing committed PNGs fail closed', () => {
  const result = spawnSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['-I', 'tests/adversarial/c03-r1-tooling.py'],
    {
      cwd: fileURLToPath(new URL('../../../', import.meta.url)),
      encoding: 'utf8',
      timeout: 60_000,
    },
  );
  expect(result.status, result.stdout + result.stderr).toBe(0);
});
