// @vitest-environment node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const python = process.platform === 'win32' ? 'python' : 'python3';

it('C02-R3-01: missing synthetic run records cannot be classified as manual hardware', () => {
  const result = spawnSync(python, ['-I', 'tests/adversarial/c02-r3-tooling.py'], {
    cwd: root,
    encoding: 'utf8',
    timeout: 60_000,
  });
  expect(result.status, result.stdout + result.stderr).toBe(0);
});
