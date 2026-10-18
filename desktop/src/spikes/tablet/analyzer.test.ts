// @vitest-environment node
// The offline analyzer (scripts/analyse-tablet-log.py) and the page's analysis must agree on the same raw data, and
// the analyzer must refuse log versions it does not know (C02-R1-01, C02-R1-03, C02-R1-07).
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it } from 'vitest';

import type { PenSimReport } from '../../ipc';
import { matchSimulation, summarizeWeb, type WebSample } from './analysis';

const ROOT = fileURLToPath(new URL('../../../../', import.meta.url));
const PYTHON = process.platform === 'win32' ? 'python' : 'python3';
const dir = mkdtempSync(join(tmpdir(), 'toontown-analyzer-'));
afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

function analyse(log: unknown): { status: number | null; stdout: string; stderr: string; json: unknown } {
  const input = join(dir, `log-${String(Math.random()).slice(2)}.json`);
  const output = `${input}.analysis.json`;
  writeFileSync(input, JSON.stringify(log));
  const r = spawnSync(PYTHON, ['-I', 'scripts/analyse-tablet-log.py', input, '--json', output], {
    cwd: ROOT,
    encoding: 'utf8',
    env: { ...process.env, PYTHONIOENCODING: 'utf-8' },
    timeout: 60_000,
  });
  const json: unknown = existsSync(output) ? JSON.parse(readFileSync(output, 'utf8')) : null;
  return { status: r.status, stdout: r.stdout, stderr: r.stderr, json };
}

const sample = (t: number, kind: WebSample['kind'], pressure: number, dispatch: number): WebSample => ({
  t,
  handlerT: t + 3,
  kind,
  pointerType: 'pen',
  pointerId: 7,
  x: t,
  y: 0,
  pressure,
  tiltX: 5,
  tiltY: -5,
  twist: 0,
  dispatch,
  coalescedCount: 1,
  coalescedIndex: 0,
});

// Two strokes: a 64-frame synthetic stroke (frames 10 and 20 dropped) at 250 Hz, and a 100 Hz hand stroke.
const synthetic: WebSample[] = [];
for (let i = 0; i < 64; i++) {
  if (i === 10 || i === 20) continue;
  synthetic.push(sample(1000 + i * 4, i === 0 ? 'down' : 'move', (i + 1) / 1024, i));
}
synthetic.push(sample(1000 + 64 * 4, 'up', 0, 64));
const hand: WebSample[] = [0, 1, 2, 3, 4, 5].map((i) =>
  sample(9000 + i * 10, i === 0 ? 'down' : i === 5 ? 'up' : 'move', i === 5 ? 0 : 0.3 + i / 100, 100 + i),
);
const samples = [...synthetic, ...hand];
const timeOrigin = 1_700_000_000_000;
const report: PenSimReport = {
  method: 'InjectSyntheticPointerInput',
  frames: 64,
  hz: 250,
  achievedHz: 250,
  injectEpochMs: Array.from({ length: 64 }, (_, i) => timeOrigin + 1000 + i * 4 - 2),
  injectReturnEpochMs: Array.from({ length: 64 }, (_, i) => timeOrigin + 1000 + i * 4 - 1.5),
  screenX: Array.from({ length: 64 }, (_, i) => i),
  screenY: Array.from({ length: 64 }, () => 0),
  screenRect: [0, 0, 100, 100],
  pattern: 'sine',
  target: 'webview',
  pressure: Array.from({ length: 64 }, (_, i) => i + 1),
  tiltX: Array.from({ length: 64 }, () => 5),
  tiltY: Array.from({ length: 64 }, () => -5),
  rotation: Array.from({ length: 64 }, () => 0),
};

const v2 = {
  spike: 'tablet',
  version: 2,
  environment: { timeOrigin, userAgent: 'fixture', devicePixelRatio: 1, coalescedApi: true },
  nativeInfo: null,
  runs: [
    {
      label: 'fixture run',
      target: 'webview',
      request: { left: 0, top: 0, width: 100, height: 100, hz: 250, frames: 64, pattern: 'sine' },
      webRange: [0, synthetic.length],
      nativeRange: [0, 0],
      report,
      probe: null,
      rawInput: null,
      error: null,
    },
  ],
  web: { samples, frames: [], truncated: 0 },
  native: { packets: [], hostDropped: 0, sendFailures: 0, invalidBatches: 0, truncated: 0 },
  // Deliberately wrong stored summaries: the analyzer must not read them.
  summary: { web: { rateHz: 1, strokes: [] } },
  simulations: [{ web: { received: 1, missing: 63 } }],
};

interface Analysis {
  version: number;
  web: { strokes: { samples: number; rateHz: number | null; gapsOver2xMedian: number }[] };
  runs: { web: { received: number; missing: number; missingFrames: number[] } }[];
}

describe('offline analyzer', () => {
  it('rejects unknown and obsolete log versions', () => {
    const r = analyse({ ...v2, version: 4 });
    expect(r.status).toBe(2);
    expect(r.stderr).toContain('unsupported log version 4');
    const v1 = analyse({ ...v2, version: 1 });
    expect(v1.status).toBe(2);
    expect(v1.stderr).toContain('version 1 is obsolete');
  });

  it('version 3: recomputes the host rate from the monotonic endpoints and rejects a disagreeing aggregate', () => {
    const start = 5_000_000_000;
    const end = start + 64 * 4_000_000; // 64 frames, 4 ms each
    const v3Report = {
      ...report,
      contactStartNs: start,
      contactEndNs: end,
      contactStartEpochMs: timeOrigin + 998,
      contactEndEpochMs: timeOrigin + 1254,
      focus: [],
    };
    const log = (achievedHz: number) => ({
      ...v2,
      version: 3,
      runs: [{ ...v2.runs[0], report: { ...v3Report, achievedHz } }],
    });
    const ok = analyse(log((64 * 1e9) / (end - start)));
    expect(ok.status, ok.stderr).toBe(0);
    const run = (ok.json as { runs: { hostEstimatorHz: number; firstToLastHz: number }[] }).runs[0];
    expect(run?.hostEstimatorHz).toBe(250);
    expect(run?.firstToLastHz).toBeCloseTo(250, 9);
    const bad = analyse(log(251));
    expect(bad.status).toBe(2);
    expect(bad.stderr).toContain('disagrees with the host estimator');
  });

  it('labels an explicitly failed run and refuses a successful run without its report', () => {
    const failed = analyse({
      ...v2,
      runs: [{ ...v2.runs[0], report: null, error: 'spike.notForeground: covered' }],
    });
    expect(failed.status, failed.stderr).toBe(0);
    expect(failed.stdout).toContain('**FAILED**: spike.notForeground: covered');
    const missing = analyse({ ...v2, runs: [{ ...v2.runs[0], report: null }] });
    expect(missing.status).toBe(2);
    expect(missing.stderr).toContain('no raw report');
  });

  it('reports a manual log without synthetic runs as having no injection reference', () => {
    const manual = analyse({ ...v2, runs: [], simulations: [] });
    expect(manual.status, manual.stderr).toBe(0);
    expect(manual.stdout).toContain('No synthetic runs: outcome (b)');
  });

  it('recomputes per-stroke results and run matches from raw data, agreeing with the page', () => {
    const r = analyse(v2);
    expect(r.status, r.stderr).toBe(0);
    const a = r.json as Analysis;
    expect(a.version).toBe(2);
    const page = summarizeWeb(samples, []);
    expect(a.web.strokes.map((s) => s.samples)).toEqual(page.strokes.map((s) => s.samples));
    a.web.strokes.forEach((s, i) => {
      expect(s.rateHz).toBeCloseTo(page.strokes[i]?.rateHz ?? Number.NaN, 9);
      expect(s.gapsOver2xMedian).toBe(page.strokes[i]?.gapsOver2xMedian);
    });
    const m = matchSimulation(report, synthetic, (s) => ({
      pressure: s.kind === 'up' ? null : s.pressure,
      tiltX: s.tiltX,
      tiltY: s.tiltY,
      epochMs: timeOrigin + s.t,
    }));
    expect(a.runs[0]?.web).toMatchObject({
      received: m.received,
      missing: m.missing,
      missingFrames: [10, 20],
    });
    expect(m.missingFrames).toEqual([10, 20]);
    expect(r.stdout).toContain('web path: 2 pen-down strokes');
    expect(r.stdout).toContain('below target'); // the 100 Hz hand stroke: below target, not a failure
  });
});
