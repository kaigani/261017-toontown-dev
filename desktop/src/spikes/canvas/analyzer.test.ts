// @vitest-environment node
// The page's run summary and the offline canvas analyzer (scripts/analyse-canvas-log.py) must agree on the same raw
// samples, and the analyzer must refuse incomplete or contradictory logs.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it } from 'vitest';

import { summarizeRun, type FrameSample } from './analysis';

const ROOT = fileURLToPath(new URL('../../../../', import.meta.url));
const PYTHON = process.platform === 'win32' ? 'python' : 'python3';
const dir = mkdtempSync(join(tmpdir(), 'toontown-canvas-analyzer-'));
afterAll(() => {
  rmSync(dir, { recursive: true, force: true });
});

function analyse(log: unknown): { status: number | null; stdout: string; stderr: string; json: unknown } {
  const input = join(dir, `log-${String(Math.random()).slice(2)}.json`);
  const output = `${input}.analysis.json`;
  writeFileSync(input, JSON.stringify(log));
  const r = spawnSync(PYTHON, ['-I', 'scripts/analyse-canvas-log.py', input, '--json', output], {
    cwd: ROOT,
    encoding: 'utf8',
    timeout: 60_000,
  });
  const json: unknown = existsSync(output) ? JSON.parse(readFileSync(output, 'utf8')) : null;
  return { status: r.status, stdout: r.stdout, stderr: r.stderr, json };
}

// 60 Hz idle; pan/zoom frames every 16.7 ms with a 50 ms stall every 100 frames.
const idle = Array.from({ length: 61 }, (_, i) => 100 + i * (1000 / 60));
const frames: FrameSample[] = [];
let t = 2000;
for (let i = 0; i < 610; i++) {
  t += i > 0 && i % 100 === 0 ? 50 : 1000 / 60;
  frames.push({ t, cpuMs: 0.2 + (i % 5) * 0.01, gpuMs: i % 2 === 0 ? 4 : null });
}
const transfers = Array.from({ length: 10 }, (_, i) => ({
  i,
  frame: i,
  bytes: 8_700_000,
  invokeMs: 50 + i,
  encodeNs: 2_000_000 + i,
  generateNs: i === 0 ? 30_000_000 : 0,
}));
const run = {
  backend: 'webgl2',
  scene: 'paths100k',
  seed: 1,
  startedAt: '2026-10-18T00:00:00Z',
  renderer: { renderer: 'test GPU' },
  canvas: { cssWidth: 800, cssHeight: 600, width: 800, height: 600, dpr: 1 },
  transfers,
  decode: { ms: 90, paths: 100000, segments: 1, tolerance: 0.5 },
  upload: { cpuMs: 10, completeMs: 20 },
  idle,
  frames,
  snapshot: null,
  error: null,
};
const log = {
  spike: 'canvas',
  version: 1,
  source: 'auto',
  environment: { timeOrigin: 1_700_000_000_000, userAgent: 'fixture', devicePixelRatio: 1 },
  parameters: { transfers: 10, idleMs: 1000, panZoomMs: 10000, pixelTransfers: 2, scenes: [], seed: 1 },
  runs: [run],
  pixelRuns: [
    {
      backend: 'webgl2',
      width: 2,
      height: 2,
      samples: [0, 1].map((i) => ({ i, bytes: 32, invokeMs: 1, fillNs: 5, uploadMs: 0.5 })),
      error: null,
    },
  ],
};

interface Analysis {
  runs: { fps: number; intervalMs: { p95: number; max: number }; longFrames: number; meetsTarget: boolean }[];
}

describe('canvas analyzer', () => {
  it('recomputes the run summary the page shows', () => {
    const r = analyse(log);
    expect(r.status, r.stderr).toBe(0);
    const a = (r.json as Analysis).runs[0];
    const page = summarizeRun(run);
    expect(a?.fps).toBeCloseTo(page.fps ?? Number.NaN, 9);
    expect(a?.intervalMs.p95).toBeCloseTo(page.intervalMs?.p95 ?? Number.NaN, 9);
    expect(a?.intervalMs.max).toBeCloseTo(page.intervalMs?.max ?? Number.NaN, 9);
    expect(a?.longFrames).toBe(page.longFrames);
    expect(a?.longFrames).toBe(6);
    expect(a?.meetsTarget).toBe(page.meetsTarget);
    expect(r.stdout).toContain('| webgl2 | paths100k | test GPU |');
  });

  it('labels an explicitly failed run and rejects malformed or contradictory logs', () => {
    const failed = analyse({ ...log, runs: [{ ...run, frames: [], error: 'webgpu is not available' }] });
    expect(failed.status, failed.stderr).toBe(0);
    expect(failed.stdout).toContain('**FAILED**: webgpu is not available');
    const cases: [string, unknown, string][] = [
      ['version', { ...log, version: 2 }, 'unsupported canvas log version'],
      ['source', { ...log, source: undefined }, 'source'],
      ['manual with runs', { ...log, source: 'manual' }, "source 'manual'"],
      ['auto without runs', { ...log, runs: [], pixelRuns: [] }, 'has no runs'],
      [
        'missing transfers',
        { ...log, runs: [{ ...run, transfers: transfers.slice(1) }] },
        'transfer samples',
      ],
      ['short pan/zoom', { ...log, runs: [{ ...run, frames: frames.slice(0, 100) }] }, 'shorter than'],
      ['no time origin', { ...log, environment: {} }, 'timeOrigin'],
    ];
    for (const [name, bad, message] of cases) {
      const r = analyse(bad);
      expect(r.status, name).toBe(2);
      expect(r.stdout, name).toBe('');
      expect(r.stderr, name).toContain(message);
    }
  });
});
