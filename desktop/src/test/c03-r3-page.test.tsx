// R3 extends R2's unmodified real-page harness at the one-sample boundary.
// Real CanvasSpike builds/saves these logs. Only host, GPU and clock are simulated.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { gunzipSync } from 'node:zlib';
import { afterEach, expect, it, vi } from 'vitest';
import { createHostApi, createMockTransport, HostProvider, type RenderListHeader } from '../ipc';
import { CanvasSpike } from '../spikes/canvas/CanvasSpike';
import type { DecodeStats } from '../spikes/canvas/decode';

const ROOT = resolve(process.cwd(), '..');
const records = JSON.parse(
  gunzipSync(
    readFileSync(join(ROOT, 'reviews/evidence/M0-C0.3/canvas-r1-local-windows-rtx4090-auto.json.gz')),
  ).toString(),
) as {
  runs: { scene: string; decode: DecodeStats; transfers: { header: RenderListHeader; bytes: number }[] }[];
};
vi.mock('../spikes/canvas/webgpu', () => ({ webgpuApi: () => null }));
vi.mock('../spikes/canvas/webgl2', () => ({
  createWebGL2: () =>
    Promise.resolve({
      backend: 'webgl2',
      info: { renderer: 'review clock stub' },
      upload: () => Promise.resolve({ cpuMs: 0, completeMs: 0 }),
      frame: () => ({ cpuMs: 0, gpuDone: Promise.resolve(null) }),
      uploadPixels: () => Promise.resolve(0),
      destroy: () => undefined,
    }),
}));
vi.mock('../spikes/canvas/decode', () => ({
  decodeRenderList: (header: RenderListHeader) => ({
    groups: new Float32Array(6),
    stats: records.runs.find((r) => r.scene === header.kind)?.decode,
  }),
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

async function pageLog(interval: number, run: boolean) {
  vi.useFakeTimers();
  let timestamp = 100;
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) =>
    setTimeout(() => {
      timestamp += interval;
      callback(timestamp);
    }, 1),
  );
  Object.defineProperty(window.screen, 'width', { configurable: true, value: 1920 });
  Object.defineProperty(window.screen, 'height', { configurable: true, value: 1080 });
  vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue('data:image/jpeg;base64,');
  const api = createHostApi(createMockTransport());
  api.spike.environment = () =>
    Promise.resolve({
      os: 'windows',
      arch: 'x86_64',
      osVersion: 'review',
      webviewVersion: 'jsdom',
      cpuCount: 1,
      window: null,
    });
  api.spike.canvasScene = (request) => {
    const r = records.runs.find((r) => r.scene === request.scene);
    const t = r?.transfers[0];
    if (!t) throw new Error('missing fixture transfer');
    return Promise.resolve({
      header: { ...t.header, frame: request.frame, byteLength: t.bytes, encodeNs: 0, generateNs: 0 },
      bytes: new ArrayBuffer(0),
    });
  };
  api.spike.canvasPixels = (width, height) =>
    Promise.resolve({
      header: { width, height, fillNs: 0, byteLength: 16 + 4 * width * height },
      bytes: new ArrayBuffer(16 + 4 * width * height),
    });
  let saved: unknown;
  api.spike.saveLog = (log) => {
    saved = log;
    return Promise.resolve({ path: 'review.json', bytes: 1 });
  };
  render(
    <HostProvider api={api}>
      <CanvasSpike
        mode={{
          name: 'canvas',
          auto: false,
          renderer: 'webgl2',
          logPath: 'review.json',
          penSimulation: false,
          nativeBackend: null,
        }}
      />
    </HostProvider>,
  );
  await act(async () => {
    await Promise.resolve();
  });
  if (run) {
    fireEvent.click(screen.getByText('Run all scenes'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(screen.getByText('All runs finished.')).toBeTruthy();
  }
  await act(async () => {
    fireEvent.click(screen.getByText('Save log'));
    await Promise.resolve();
  });
  expect(saved).toBeDefined();
  return saved;
}

function analyze(log: unknown) {
  const dir = mkdtempSync(join(tmpdir(), 'c03-r3-page-'));
  try {
    writeFileSync(join(dir, 'page.json'), JSON.stringify(log));
    if (process.env['C03_REVIEW_EVIDENCE'])
      writeFileSync(
        join(
          process.env['C03_REVIEW_EVIDENCE'],
          `page-r3-${expect.getState().currentTestName?.includes('control') ? 'two-frames' : 'one-frame'}.json`,
        ),
        JSON.stringify(log),
      );
    return spawnSync(
      process.platform === 'win32' ? 'python' : 'python3',
      ['-I', 'scripts/analyse-canvas-log.py', join(dir, 'page.json')],
      { cwd: ROOT, encoding: 'utf8' },
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

it('control: completed 0.2 Hz phase with two recorded pan frames is accepted', async () => {
  const r = analyze(await pageLog(5000, true));
  expect(r.status, r.stderr).toBe(0);
});

it('C03-R3-01: completed 0.1 Hz phase is accepted even with one recorded pan frame', async () => {
  const log = await pageLog(10_000, true);
  const saved = log as {
    version: number;
    runs: {
      error: string | null;
      frames: { t: number }[];
      phases: { panZoom: { start: number; end: number } };
    }[];
  };
  expect(saved.version).toBe(2);
  expect(saved.runs).toHaveLength(3);
  for (const run of saved.runs) {
    expect(run.error).toBeNull();
    expect(run.frames).toHaveLength(1);
    expect(run.phases.panZoom.end - run.phases.panZoom.start).toBe(10_000);
    expect(run.frames[0]?.t).toBe(run.phases.panZoom.end);
  }
  const r = analyze(log);
  expect(r.status, r.stderr).toBe(0);
});
