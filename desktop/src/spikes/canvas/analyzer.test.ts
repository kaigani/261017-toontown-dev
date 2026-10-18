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
  frames.push({ t, cpuMs: 0.2 + (i % 5) * 0.01, gpuMs: null });
}
const transfers = Array.from({ length: 10 }, (_, i) => ({
  i,
  frame: i,
  bytes: 8_700_096,
  header: {
    kind: 'paths100k',
    seed: 1,
    frame: i,
    groups: 1,
    paths: 100_000,
    points: 700_000,
    verbs: 300_000,
  },
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
  decode: {
    ms: 90,
    paths: 100_000,
    cubics: 200_000,
    strokePaths: 100_000,
    fillPaths: 0,
    segments: 2_832_713,
    fillVertices: 0,
    triangles: 0,
    tolerance: 0.5,
  },
  upload: { cpuMs: 10, completeMs: 20 },
  idle,
  frames,
  phases: { idle: { start: idle[0], end: idle[idle.length - 1] }, panZoom: { start: 2000, end: t } },
  snapshot: null,
  error: null,
};
const log = {
  spike: 'canvas',
  version: 2,
  source: 'auto',
  environment: {
    timeOrigin: 1_700_000_000_000,
    userAgent: 'fixture',
    devicePixelRatio: 1,
    viewport: { width: 800, height: 640 },
    screen: { width: 1920, height: 1080 },
    webgpuApi: false,
    forcedRenderer: 'webgl2',
    mode: { name: 'canvas', auto: true, renderer: 'webgl2' },
    host: { os: 'windows', arch: 'x86_64' },
    hostError: null,
  },
  parameters: {
    transfers: 10,
    idleMs: 1000,
    panZoomMs: 10000,
    pixelTransfers: 2,
    scenes: ['paths100k'],
    seed: 1,
    flattenTolerance: 0.5,
  },
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
  runs: {
    fps: number | null;
    intervalMs: { p95: number; max: number } | null;
    longFrames: number;
    meetsTarget: boolean | null;
    complete: boolean;
    cadence: string;
  }[];
}

// A complete pan/zoom phase observed by a single sample: a 0.1 Hz display reaches the 10 s span on its first callback.
const oneFrame = {
  ...run,
  idle: [100, 10_100],
  frames: [{ t: 30_100, cpuMs: 0.2, gpuMs: null }],
  phases: { idle: { start: 100, end: 10_100 }, panZoom: { start: 20_100, end: 30_100 } },
};

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

  it('accepts a complete 1 Hz run as a missed target (C03-R2-02)', () => {
    const slow = Array.from({ length: 10 }, (_, k) => ({ t: 3000 + 1000 * k, cpuMs: 0.2, gpuMs: null }));
    const r = analyse({
      ...log,
      runs: [
        {
          ...run,
          idle: [100, 1100],
          frames: slow,
          phases: { idle: { start: 100, end: 1100 }, panZoom: { start: 2000, end: 12_000 } },
        },
      ],
    });
    expect(r.status, r.stderr).toBe(0);
    const a = (r.json as Analysis).runs[0];
    expect(a?.fps).toBeCloseTo(1, 9);
    expect(a?.meetsTarget).toBe(false);
    expect(r.stdout).toContain('missed');
  });

  it('accepts a complete one-frame run as complete with insufficient cadence samples (C03-R3-01)', () => {
    const r = analyse({ ...log, runs: [oneFrame] });
    expect(r.status, r.stderr).toBe(0);
    const a = (r.json as Analysis).runs[0];
    expect(a?.complete).toBe(true);
    expect(a?.cadence).toBe('insufficient samples (n=1)');
    expect(a?.fps).toBeNull();
    expect(a?.intervalMs).toBeNull();
    expect(a?.meetsTarget).toBeNull();
    expect(r.stdout).toContain('not estimated: insufficient samples (n=1) (run complete)');
    expect(r.stdout).not.toContain('| met |');
    // The page's own summary declines to estimate in the same way.
    const page = summarizeRun(oneFrame);
    expect(page.fps).toBeNull();
    expect(page.meetsTarget).toBeNull();
  });

  it('rejects zero frames under a claimed complete span, and one frame in a version-1 log (C03-R3-01)', () => {
    const cases: [string, unknown][] = [
      ['zero frames, complete span', { ...log, runs: [{ ...oneFrame, frames: [] }] }],
      ['one frame, version 1', { ...log, version: 1, runs: [oneFrame] }],
      [
        'one frame, span ends early',
        {
          ...log,
          runs: [{ ...oneFrame, phases: { ...oneFrame.phases, panZoom: { start: 20_101, end: 30_100 } } }],
        },
      ],
      [
        'one frame before the span end',
        { ...log, runs: [{ ...oneFrame, frames: [{ t: 30_000, cpuMs: 0.2, gpuMs: null }] }] },
      ],
    ];
    for (const [name, bad] of cases) {
      const r = analyse(bad);
      expect(r.status, name).toBe(2);
      expect(r.stdout, name).toBe('');
    }
    expect(analyse(cases[0]?.[1]).stderr).toContain('pan/zoom frames missing');
  });

  it('labels an explicitly failed run and rejects malformed or contradictory logs', () => {
    const failed = analyse({ ...log, runs: [{ ...run, frames: [], error: 'webgpu is not available' }] });
    expect(failed.status, failed.stderr).toBe(0);
    expect(failed.stdout).toContain('**FAILED**: webgpu is not available');
    const cases: [string, unknown, string][] = [
      ['version', { ...log, version: 3 }, 'unsupported canvas log version'],
      ['source', { ...log, source: undefined }, 'source'],
      ['manual with runs', { ...log, source: 'manual' }, "source 'manual'"],
      ['auto without runs', { ...log, runs: [], pixelRuns: [] }, 'has no runs'],
      [
        'missing transfers',
        { ...log, runs: [{ ...run, transfers: transfers.slice(1) }] },
        'transfer samples',
      ],
      [
        'truncated pan/zoom samples',
        { ...log, runs: [{ ...run, frames: frames.slice(0, 100) }] },
        'truncated',
      ],
      [
        'pan/zoom phase ended early',
        {
          ...log,
          runs: [
            {
              ...run,
              frames: frames.slice(0, 100),
              phases: { ...run.phases, panZoom: { start: 2000, end: frames[99]?.t } },
            },
          ],
        },
        'incomplete run',
      ],
      [
        'idle phase ended early',
        {
          ...log,
          runs: [
            {
              ...run,
              idle: idle.slice(0, 30),
              phases: { ...run.phases, idle: { start: 100, end: idle[29] } },
            },
          ],
        },
        'incomplete run',
      ],
      ['phase spans missing', { ...log, runs: [{ ...run, phases: undefined }] }, 'phase spans missing'],
      [
        'version 1 log with short pan/zoom samples',
        { ...log, version: 1, runs: [{ ...run, frames: frames.slice(0, 100) }] },
        'shorter than',
      ],
      [
        'seed outside the workload table',
        { ...log, parameters: { ...log.parameters, seed: 2 } },
        'seed 2 not in the supported workload table',
      ],
      [
        'points moved into verbs',
        {
          ...log,
          runs: [
            {
              ...run,
              transfers: transfers.map((x) => ({
                ...x,
                header: { ...x.header, points: 0, verbs: x.header.verbs + 8 * x.header.points },
              })),
            },
          ],
        },
        'header points = 0',
      ],
      [
        'decode statistics impossible for the header',
        { ...log, runs: [{ ...run, decode: { ...run.decode, cubics: 300_001 } }] },
        "exceed the header's verbs",
      ],
      ['no time origin', { ...log, environment: {} }, 'timeOrigin'],
      [
        'header counts of another scene',
        {
          ...log,
          runs: [
            {
              ...run,
              transfers: transfers.map((x) => ({ ...x, header: { ...x.header, kind: 'brush10k' } })),
            },
          ],
        },
        'another scene',
      ],
      [
        'GPU time claimed for WebGL2',
        { ...log, runs: [{ ...run, frames: frames.map((f) => ({ ...f, gpuMs: 3 })) }] },
        'cannot measure',
      ],
    ];
    for (const [name, bad, message] of cases) {
      const r = analyse(bad);
      expect(r.status, name).toBe(2);
      expect(r.stdout, name).toBe('');
      expect(r.stderr, name).toContain(message);
    }
  });
});
