// @vitest-environment node
/// <reference types="node" />
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { it } from 'vitest';

it('round-3 IPC boundary regressions through the real lint/build configs', () => {
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  execFileSync(process.execPath, ['--test', 'tests/adversarial/c01-r3-boundary.test.mjs'], {
    cwd: root,
    encoding: 'utf8',
    stdio: 'pipe',
    timeout: 120_000,
  });
}, 120_000);
