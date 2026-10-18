// @vitest-environment node
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { it } from 'vitest';

it('C0.2 adversarial native conversion and offline analysis regressions', () => {
  execFileSync(process.platform === 'win32' ? 'python' : 'python3', ['tests/adversarial/c02-r1-tooling.py'], {
    cwd: fileURLToPath(new URL('../../../', import.meta.url)),
    encoding: 'utf8',
    stdio: 'pipe',
    timeout: 120_000,
  });
}, 120_000);
