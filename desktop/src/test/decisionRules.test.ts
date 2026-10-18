// @vitest-environment node
/// <reference types="node" />
// C0.4 round 2: runs the executable decision-rule regressions (scripts/test_decision_rules.py) in the frontend gate:
// analyzer constants equal the ADR text, rate boundaries, the worked examples, action-to-episode mapping and the
// canvas completeness/null-score cases.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { expect, it } from 'vitest';

const repo = fileURLToPath(new URL('../../../', import.meta.url));

it('the C0.4 decision rules hold (scripts/test_decision_rules.py)', () => {
  const r = spawnSync(
    process.platform === 'win32' ? 'python' : 'python3',
    ['-I', 'scripts/test_decision_rules.py'],
    {
      cwd: repo,
      encoding: 'utf8',
    },
  );
  expect(r.status, `${r.stdout}\n${r.stderr}`).toBe(0);
  expect(r.stderr).toMatch(/Ran \d+ tests/);
});
