"""Temporary probe of the unchanged R3 harness; leaves no test or product edits."""
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
probe = ROOT / 'desktop/src/test/c03-r4-closure-probe.test.tsx'
source = (ROOT / 'desktop/src/test/c03-r3-page.test.tsx').read_text()
needle = '  const log = await pageLog(10_000, true);'
assert source.count(needle) == 1 and not probe.exists()
source = source.replace(needle, needle + '''
  const fps = screen.getAllByRole('cell', { name: 'insufficient samples (n=1)', exact: true });
  const targets = screen.getAllByRole('cell', { name: 'not estimated: insufficient samples (n=1)', exact: true });
  expect(fps).toHaveLength(3);
  expect(targets).toHaveLength(3);
  writeFileSync(join(process.env['C03_REVIEW_EVIDENCE']!, 'page-readout.json'), JSON.stringify({
    completion: 'original harness asserted All runs finished before Save log changed the status',
    fps: fps.map((cell) => cell.textContent),
    targets: targets.map((cell) => cell.textContent),
    rows: screen.getAllByRole('row').map((row) => row.textContent),
  }, null, 2));
''')
source += '''
it('nullable optional access still rejects absent or incorrect interval statistics', () => {
  const cases: ({ intervalMs: { p95?: number; max?: number } | null } | undefined)[] = [
    undefined, { intervalMs: null }, { intervalMs: {} }, { intervalMs: { p95: 0, max: 0 } },
  ];
  const page = { intervalMs: { p95: 16.7, max: 33.3 } };
  for (const a of cases) {
    expect(() => expect(a?.intervalMs?.p95).toBeCloseTo(page.intervalMs?.p95 ?? Number.NaN, 9)).toThrow();
    expect(() => expect(a?.intervalMs?.max).toBeCloseTo(page.intervalMs?.max ?? Number.NaN, 9)).toThrow();
  }
});
'''
(OUT / 'page-probe-source.txt').write_text(source, encoding='utf-8')
try:
    probe.write_text(source, encoding='utf-8')
    start = time.time()
    command = 'pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c03-r4-closure-probe.test.tsx'
    with (OUT / 'page-readout.log').open('wb') as f:
        p = subprocess.run(['cmd', '/d', '/c', command], cwd=ROOT, stdout=f, stderr=subprocess.STDOUT,
                           env={**os.environ, 'C03_REVIEW_EVIDENCE': str(OUT), 'PYTHONIOENCODING': 'utf-8'}, timeout=180)
    receipt = dict(command=command, exitCode=p.returncode, seconds=time.time()-start)
    (OUT / 'page-readout.receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(receipt)
    assert p.returncode == 0
finally:
    probe.unlink(missing_ok=True)
