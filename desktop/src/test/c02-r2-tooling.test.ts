// @vitest-environment node
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';

import { percentile } from '../lib/latency';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const python = process.platform === 'win32' ? 'python' : 'python3';

it('C02-R2: permanent analyzer evidence-integrity attacks', () => {
  const r = spawnSync(python, ['-I', 'tests/adversarial/c02-r2-tooling.py'], {
    cwd: root,
    encoding: 'utf8',
    timeout: 60_000,
  });
  expect(r.status, r.stdout + r.stderr).toBe(0);
});

it('C02-R1-07 closure: Python and frontend percentile results have identical binary64 bits on ties', () => {
  const cases = [
    [1, 2, 3, 4, 5, 6],
    [0.1, 0.1, 0.3, 0.3, 0.7, 0.7],
    Array.from({ length: 100 }, (_, i) => Math.floor(i / 3) / 10),
  ];
  const percentiles = [1, 25, 50, 75, 95, 99, 100];
  const script = `import importlib.util,json,struct,sys
s=importlib.util.spec_from_file_location('a','scripts/analyse-tablet-log.py')
a=importlib.util.module_from_spec(s);s.loader.exec_module(a)
cases,ps=json.load(sys.stdin)
print(json.dumps([struct.pack('>d',a.pct(c,p)).hex() for c in cases for p in ps]))`;
  const r = spawnSync(python, ['-I', '-c', script], {
    cwd: root,
    input: JSON.stringify([cases, percentiles]),
    encoding: 'utf8',
    timeout: 30_000,
  });
  expect(r.status, r.stderr).toBe(0);
  const expected = cases.flatMap((c) =>
    percentiles.map((p) => {
      const b = Buffer.alloc(8);
      b.writeDoubleBE(percentile(c, p));
      return b.toString('hex');
    }),
  );
  expect(JSON.parse(r.stdout)).toEqual(expected);
});
