// @vitest-environment node
/// <reference types="node" />
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { it } from 'vitest';

it('round-4 closure: final emitted code cannot bypass the IPC boundary', () => {
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  execFileSync(process.execPath, ['--test', 'tests/adversarial/c01-r4-boundary.test.mjs'], {
    cwd: root,
    encoding: 'utf8',
    stdio: 'pipe',
    timeout: 120_000,
  });
}, 120_000);
