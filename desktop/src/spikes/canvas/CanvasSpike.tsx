// C0.3 canvas throughput spike page (spike code). Shown instead of the workspace only when the host was started
// with `--spike canvas`; never reachable from the menu.
//
// For each back end (WebGPU when available, and WebGL2, which is mandatory and measured on its own) and each scene
// (100 000 cubic paths; 1 000 transformed groups; 10 000 brush strokes) it measures:
// render-list transfer (10 sequential binary invokes), decode, upload, an idle phase (display refresh), and 10 s of
// scripted pan/zoom; then the per-frame cost of moving a finished bitmap across IPC (host-raster fallback). Raw
// samples go into a version-1 log; `scripts/analyse-canvas-log.py` recomputes every summary from them.
import { useCallback, useEffect, useRef, useState } from 'react';

import { toHostError, useHost, type CanvasSceneKind, type SpikeEnvironment, type SpikeMode } from '../../ipc';
import { stats, summarizeRun, type FrameSample, type RunSummary, type TransferSample } from './analysis';
import { animateGroups, cameraAt, PAN_ZOOM_MS } from './camera';
import { decodeRenderList, type DecodeStats } from './decode';
import type { Backend, Renderer, UploadTiming } from './renderer';
import { createWebGL2 } from './webgl2';
import { createWebGPU, webgpuApi } from './webgpu';

export const CANVAS_LOG_VERSION = 1;
const SCENES: CanvasSceneKind[] = ['paths100k', 'groups1k', 'brush10k'];
const SEED = 1;
const TRANSFERS = 10;
const IDLE_MS = 1000;
const PIXEL_TRANSFERS = 20;
const TOLERANCE = 0.5;

interface CanvasGeometry {
  cssWidth: number;
  cssHeight: number;
  width: number;
  height: number;
  dpr: number;
}

interface RunRecord {
  backend: Backend;
  scene: CanvasSceneKind;
  seed: number;
  startedAt: string;
  renderer: Record<string, unknown> | null;
  canvas: CanvasGeometry | null;
  transfers: TransferSample[];
  decode: ({ ms: number } & DecodeStats) | null;
  upload: UploadTiming | null;
  /** rAF timestamps of the idle phase (nothing drawn). */
  idle: number[];
  frames: FrameSample[];
  /** JPEG data URL of one frame at the camera's start position (visual check of what was drawn). */
  snapshot: string | null;
  error: string | null;
}

interface PixelRun {
  backend: Backend;
  width: number;
  height: number;
  samples: { i: number; bytes: number; invokeMs: number; fillNs: number; uploadMs: number }[];
  error: string | null;
}

const nextFrame = () =>
  new Promise<number>((resolve) => {
    requestAnimationFrame(resolve);
  });

const sleep = (ms: number) =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });

function errorText(e: unknown): string {
  if (e instanceof Error && !('code' in e)) return e.message;
  const err = toHostError(e);
  return `${err.code}: ${err.message}`;
}

function fmt(v: number | null | undefined, digits = 1): string {
  return v === null || v === undefined || !Number.isFinite(v) ? '–' : v.toFixed(digits);
}

export function CanvasSpike({ mode }: { mode: SpikeMode }) {
  const host = useHost();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const runs = useRef<RunRecord[]>([]);
  const pixelRuns = useRef<PixelRun[]>([]);
  const statusLog = useRef<{ at: string; message: string }[]>([]);
  const hostEnv = useRef<{ value: SpikeEnvironment | null; error: string | null }>({
    value: null,
    error: null,
  });
  const [status, setStatusText] = useState('Ready.');
  const [rows, setRows] = useState<{ run: RunRecord; summary: RunSummary }[]>([]);
  const [pixelRows, setPixelRows] = useState<PixelRun[]>([]);
  const [busy, setBusy] = useState(false);
  const autoStarted = useRef(false);
  const ranAnything = useRef(false);

  const setStatus = useCallback((message: string) => {
    statusLog.current.push({ at: new Date().toISOString(), message });
    setStatusText(message);
  }, []);

  useEffect(() => {
    host.spike.environment().then(
      (value) => {
        hostEnv.current = { value, error: null };
      },
      (e: unknown) => {
        hostEnv.current = { value: null, error: errorText(e) };
      },
    );
  }, [host]);

  const backends = useCallback((): Backend[] => {
    if (mode.renderer === 'webgl2') return ['webgl2'];
    if (mode.renderer === 'webgpu') return ['webgpu'];
    return webgpuApi() ? ['webgpu', 'webgl2'] : ['webgl2'];
  }, [mode.renderer]);

  /** A fresh canvas filling the host area (a canvas can hold only one context type). */
  const freshCanvas = useCallback((): { canvas: HTMLCanvasElement; geometry: CanvasGeometry } | null => {
    const hostEl = hostRef.current;
    if (!hostEl) return null;
    hostEl.replaceChildren();
    const canvas = document.createElement('canvas');
    canvas.className = 'canvas-spike-surface';
    hostEl.appendChild(canvas);
    const rect = hostEl.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const geometry = {
      cssWidth: rect.width,
      cssHeight: rect.height,
      width: Math.max(1, Math.round(rect.width * dpr)),
      height: Math.max(1, Math.round(rect.height * dpr)),
      dpr,
    };
    canvas.width = geometry.width;
    canvas.height = geometry.height;
    return { canvas, geometry };
  }, []);

  const makeRenderer = useCallback(async (backend: Backend, canvas: HTMLCanvasElement): Promise<Renderer> => {
    return backend === 'webgpu' ? createWebGPU(canvas) : createWebGL2(canvas);
  }, []);

  const runScene = useCallback(
    async (backend: Backend, scene: CanvasSceneKind): Promise<void> => {
      ranAnything.current = true;
      const run: RunRecord = {
        backend,
        scene,
        seed: SEED,
        startedAt: new Date().toISOString(),
        renderer: null,
        canvas: null,
        transfers: [],
        decode: null,
        upload: null,
        idle: [],
        frames: [],
        snapshot: null,
        error: null,
      };
      runs.current.push(run);
      let renderer: Renderer | null = null;
      try {
        const surface = freshCanvas();
        if (!surface) throw new Error('no canvas host');
        run.canvas = surface.geometry;
        renderer = await makeRenderer(backend, surface.canvas);
        run.renderer = renderer.info;

        setStatus(`${backend} ${scene}: transferring the render list ${String(TRANSFERS)}×…`);
        let first: Awaited<ReturnType<typeof host.spike.canvasScene>> | null = null;
        for (let i = 0; i < TRANSFERS; i++) {
          const t0 = performance.now();
          const payload = await host.spike.canvasScene({ scene, seed: SEED, frame: i });
          const invokeMs = performance.now() - t0;
          run.transfers.push({
            i,
            frame: i,
            bytes: payload.header.byteLength,
            invokeMs,
            encodeNs: payload.header.encodeNs,
            generateNs: payload.header.generateNs,
            header: {
              kind: payload.header.kind,
              seed: payload.header.seed,
              frame: payload.header.frame,
              groups: payload.header.groups,
              paths: payload.header.paths,
              points: payload.header.points,
              verbs: payload.header.verbs,
            },
          });
          first ??= payload;
        }
        if (!first) throw new Error('no render list received');

        setStatus(`${backend} ${scene}: decoding…`);
        await nextFrame();
        const d0 = performance.now();
        const geometry = decodeRenderList(first.header, first.bytes, TOLERANCE);
        run.decode = { ms: performance.now() - d0, ...geometry.stats };

        setStatus(`${backend} ${scene}: uploading…`);
        run.upload = await renderer.upload(geometry);

        // One frame at the start position, read back in the same task (before presentation) for the log.
        {
          const groups0 = new Float32Array((geometry.groups.length / 6) * 8);
          animateGroups(geometry.groups, groups0, 0, false);
          renderer.frame(cameraAt(0, surface.geometry.width, surface.geometry.height), groups0);
          run.snapshot = surface.canvas.toDataURL('image/jpeg', 0.85);
        }

        setStatus(`${backend} ${scene}: idle phase (display refresh)…`);
        const idleStart = await nextFrame();
        run.idle.push(idleStart);
        for (;;) {
          const t = await nextFrame();
          run.idle.push(t);
          if (t - idleStart >= IDLE_MS) break;
        }

        setStatus(`${backend} ${scene}: pan/zoom for ${String(PAN_ZOOM_MS / 1000)} s…`);
        const groups = new Float32Array((geometry.groups.length / 6) * 8);
        const animate = scene === 'groups1k';
        const pending: Promise<unknown>[] = [];
        const start = await nextFrame();
        for (;;) {
          const t = await nextFrame();
          const elapsed = t - start;
          const camera = cameraAt(elapsed, surface.geometry.width, surface.geometry.height);
          animateGroups(geometry.groups, groups, elapsed, animate);
          const timing = renderer.frame(camera, groups);
          const sample: FrameSample = { t, cpuMs: timing.cpuMs, gpuMs: null };
          run.frames.push(sample);
          pending.push(
            timing.gpuDone.then((ms) => {
              sample.gpuMs = ms;
            }),
          );
          if (elapsed >= PAN_ZOOM_MS) break;
        }
        await Promise.all(pending);
        const summary = summarizeRun(run);
        setRows((r) => [...r, { run, summary }]);
        setStatus(
          `${backend} ${scene}: ${fmt(summary.fps)} fps (interval p95 ${fmt(summary.intervalMs?.p95, 2)} ms).`,
        );
      } catch (e: unknown) {
        run.error = errorText(e);
        setStatus(`${backend} ${scene} failed: ${run.error}`);
      } finally {
        renderer?.destroy();
      }
    },
    [host, freshCanvas, makeRenderer, setStatus],
  );

  const runPixels = useCallback(
    async (backend: Backend): Promise<void> => {
      const surface = freshCanvas();
      const run: PixelRun = {
        backend,
        width: surface?.geometry.width ?? 0,
        height: surface?.geometry.height ?? 0,
        samples: [],
        error: null,
      };
      pixelRuns.current.push(run);
      let renderer: Renderer | null = null;
      try {
        if (!surface) throw new Error('no canvas host');
        renderer = await makeRenderer(backend, surface.canvas);
        setStatus(`${backend}: pixel-frame transfer ${String(run.width)}×${String(run.height)}…`);
        for (let i = 0; i < PIXEL_TRANSFERS; i++) {
          const t0 = performance.now();
          const frame = await host.spike.canvasPixels(run.width, run.height, i);
          const invokeMs = performance.now() - t0;
          const uploadMs = await renderer.uploadPixels(
            new Uint8Array(frame.bytes, 16),
            run.width,
            run.height,
          );
          run.samples.push({
            i,
            bytes: frame.header.byteLength,
            invokeMs,
            fillNs: frame.header.fillNs,
            uploadMs,
          });
        }
      } catch (e: unknown) {
        run.error = errorText(e);
      } finally {
        renderer?.destroy();
        setPixelRows([...pixelRuns.current]);
      }
    },
    [host, freshCanvas, makeRenderer, setStatus],
  );

  const buildLog = useCallback(
    () => ({
      spike: 'canvas',
      version: CANVAS_LOG_VERSION,
      source: mode.auto ? 'auto' : ranAnything.current ? 'interactive' : 'manual',
      savedAt: new Date().toISOString(),
      environment: {
        timeOrigin: performance.timeOrigin,
        userAgent: navigator.userAgent,
        hardwareConcurrency:
          (navigator as unknown as { hardwareConcurrency?: number }).hardwareConcurrency ?? null,
        devicePixelRatio: window.devicePixelRatio,
        viewport: { width: window.innerWidth, height: window.innerHeight },
        screen: { width: screen.width, height: screen.height },
        webgpuApi: webgpuApi() !== null,
        forcedRenderer: mode.renderer ?? null,
        host: hostEnv.current.value,
        hostError: hostEnv.current.error,
        mode,
      },
      parameters: {
        scenes: SCENES,
        seed: SEED,
        transfers: TRANSFERS,
        idleMs: IDLE_MS,
        panZoomMs: PAN_ZOOM_MS,
        pixelTransfers: PIXEL_TRANSFERS,
        flattenTolerance: TOLERANCE,
      },
      runs: runs.current,
      pixelRuns: pixelRuns.current,
      statusLog: statusLog.current,
    }),
    [mode],
  );

  const save = useCallback(async () => {
    try {
      const saved = await host.spike.saveLog(buildLog());
      setStatus(`Log saved: ${saved.path} (${String(saved.bytes)} bytes).`);
    } catch (e: unknown) {
      setStatus(`Save failed: ${errorText(e)}`);
    }
  }, [host, buildLog, setStatus]);

  const runAll = useCallback(async () => {
    setBusy(true);
    try {
      for (const backend of backends()) {
        for (const scene of SCENES) {
          await runScene(backend, scene);
          await sleep(300);
        }
        await runPixels(backend);
      }
      setStatus('All runs finished.');
    } finally {
      setBusy(false);
    }
  }, [backends, runScene, runPixels, setStatus]);

  useEffect(() => {
    if (!mode.auto || autoStarted.current) return;
    autoStarted.current = true;
    void (async () => {
      await sleep(1000);
      await runAll();
      await save();
    })();
  }, [mode.auto, runAll, save]);

  const pixelSummary = pixelRows.map((p) => ({
    backend: p.backend,
    size: `${String(p.width)}×${String(p.height)}`,
    invoke: stats(p.samples.map((s) => s.invokeMs)),
    upload: stats(p.samples.map((s) => s.uploadMs)),
    error: p.error,
  }));

  return (
    <div className="canvas-spike">
      <header className="spike-header">
        <strong>C0.3 canvas throughput spike</strong> — not product UI. Back ends: {backends().join(', ')}
        {mode.renderer ? ` (forced with --spike-renderer ${mode.renderer})` : ''}. Log:{' '}
        {mode.logPath ?? 'not set (start with --spike-log <file>)'}.
      </header>
      <div className="spike-toolbar">
        <button type="button" disabled={busy} onClick={() => void runAll()}>
          Run all scenes
        </button>
        <button type="button" disabled={busy || !mode.logPath} onClick={() => void save()}>
          Save log
        </button>
        <span className="spike-status">{status}</span>
      </div>
      <div ref={hostRef} className="canvas-spike-host" data-testid="canvas-host" />
      <div className="canvas-spike-readout">
        <table className="spike-runs">
          <thead>
            <tr>
              <th>back end</th>
              <th>scene</th>
              <th>MB / transfer</th>
              <th>transfer ms p50 / p95</th>
              <th>encode ms p50</th>
              <th>decode ms</th>
              <th>upload ms</th>
              <th>fps</th>
              <th>interval ms p50 / p95 / max</th>
              <th>long frames</th>
              <th>CPU ms p95</th>
              <th>GPU ms p95</th>
              <th>60 fps target</th>
            </tr>
          </thead>
          <tbody>
            {rows.map(({ run, summary: s }) => (
              <tr key={`${run.backend}-${run.scene}-${run.startedAt}`}>
                <td>{run.backend}</td>
                <td>{run.scene}</td>
                <td>{fmt((s.bytesPerTransfer ?? 0) / 1e6, 2)}</td>
                <td>
                  {fmt(s.transferMs?.p50)} / {fmt(s.transferMs?.p95)}
                </td>
                <td>{fmt(s.encodeMs?.p50, 2)}</td>
                <td>{fmt(run.decode?.ms)}</td>
                <td>{fmt(run.upload?.completeMs)}</td>
                <td>{fmt(s.fps)}</td>
                <td>
                  {fmt(s.intervalMs?.p50, 2)} / {fmt(s.intervalMs?.p95, 2)} / {fmt(s.intervalMs?.max, 1)}
                </td>
                <td>{String(s.longFrames)}</td>
                <td>{fmt(s.cpuMs?.p95, 2)}</td>
                <td>{fmt(s.gpuMs?.p95, 2)}</td>
                <td>{s.meetsTarget === null ? '–' : s.meetsTarget ? 'met' : 'missed'}</td>
              </tr>
            ))}
            {pixelSummary.map((p) => (
              <tr key={`px-${p.backend}`}>
                <td>{p.backend}</td>
                <td>pixel frame {p.size}</td>
                <td colSpan={11}>
                  {p.error ??
                    `transfer p50 ${fmt(p.invoke?.p50)} / p95 ${fmt(p.invoke?.p95)} ms; upload p50 ${fmt(
                      p.upload?.p50,
                    )} ms`}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
