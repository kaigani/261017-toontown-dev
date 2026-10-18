// C0.2 tablet spike page (spike code). Shown instead of the workspace only when the host was started with
// `--spike tablet`; never reachable from the menu. Compares the web path (pointer events in the webview) with the
// native path (host listener streaming over a channel) on the same strokes, live and in a JSON log.
import { useCallback, useEffect, useRef, useState } from 'react';

import {
  toHostError,
  useHost,
  type NativePacket,
  type NativeStartInfo,
  type PenSimReport,
  type SpikeMode,
} from '../../ipc';
import {
  matchSimulation,
  summarizeNative,
  summarizeWeb,
  type SimulationMatch,
  type StreamSummary,
  type WebSample,
  type WebSummary,
} from './analysis';
import { MAX_SAMPLES, WebCapture } from './capture';

const LIVE_WINDOW = 5000;
const AUTO_SCENARIOS = [
  { hz: 240, frames: 1024 },
  { hz: 500, frames: 1024 },
] as const;

interface SimulationRecord {
  request: { hz: number; frames: number };
  report: PenSimReport;
  web: SimulationMatch;
  native: SimulationMatch;
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });

function fmt(v: number | null | undefined, digits = 1): string {
  return v === null || v === undefined ? '–' : v.toFixed(digits);
}

function SummaryTable({
  title,
  summary,
  web,
}: {
  title: string;
  summary: StreamSummary | null;
  web?: WebSummary | null;
}) {
  const s = summary;
  const rows: [string, string][] = s
    ? [
        ['samples', String(s.samples)],
        [
          'pointer types',
          Object.entries(s.pointerTypes)
            .map(([k, n]) => `${k} ${String(n)}`)
            .join(', ') || '–',
        ],
        ['rate (Hz, contact)', `${fmt(s.rateHz)} (median ${fmt(s.medianRateHz)})`],
        ['interval median / max (ms)', `${fmt(s.medianIntervalMs, 2)} / ${fmt(s.maxIntervalMs, 2)}`],
        ['gaps > 2× median', String(s.gaps)],
        [
          'pressure distinct / implied levels',
          `${String(s.pressure.distinct)} / ${s.pressure.levelsImplied ?? '–'}`,
        ],
        [
          'tilt present (x range, y range)',
          `${s.tiltPresent ? 'yes' : 'no'} (${s.tiltRangeX?.join('..') ?? '–'}, ${s.tiltRangeY?.join('..') ?? '–'})`,
        ],
        ['twist present', s.twistPresent ? 'yes' : 'no'],
      ]
    : [];
  if (web) {
    rows.push(
      ['coalesced events supported', web.coalescedSupported ? 'yes' : 'no'],
      [
        'coalesced per dispatch (p50 / max)',
        `${fmt(web.coalescedPerDispatch?.p50, 0)} / ${fmt(web.coalescedPerDispatch?.max, 0)}`,
      ],
      [
        'samples per frame (p50 / max)',
        `${fmt(web.samplesPerFrame?.p50, 0)} / ${fmt(web.samplesPerFrame?.max, 0)}`,
      ],
      [
        'event → handler latency p50 / p95 (ms)',
        `${fmt(web.handlerLatencyMs?.p50, 2)} / ${fmt(web.handlerLatencyMs?.p95, 2)}`,
      ],
    );
  }
  return (
    <section className="spike-col">
      <h3>{title}</h3>
      <table>
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k}>
              <th>{k}</th>
              <td>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

export function TabletSpike({ mode }: { mode: SpikeMode }) {
  const host = useHost();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const capture = useRef<WebCapture | null>(null);
  const native = useRef<NativePacket[]>([]);
  const invalidBatches = useRef(0);
  const simulations = useRef<SimulationRecord[]>([]);
  const [nativeInfo, setNativeInfo] = useState<NativeStartInfo | null>(null);
  const nativeInfoRef = useRef<NativeStartInfo | null>(null);
  const [status, setStatusText] = useState('Draw on the canvas with the pen.');
  const statusLog = useRef<{ at: string; message: string }[]>([]);
  const setStatus = useCallback((message: string) => {
    statusLog.current.push({ at: new Date().toISOString(), message });
    setStatusText(message);
  }, []);
  const [webSummary, setWebSummary] = useState<WebSummary | null>(null);
  const [nativeSummary, setNativeSummary] = useState<StreamSummary | null>(null);
  const autoStarted = useRef(false);

  // Drawing: web samples as a pressure-width line, native packets as red dots.
  const draw = useCallback((s: WebSample) => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx || s.kind !== 'move') return;
    const rect = canvas.getBoundingClientRect();
    ctx.fillStyle = s.pointerType === 'pen' ? '#6aa2ff' : '#999';
    const r = 0.5 + s.pressure * 6;
    ctx.beginPath();
    ctx.arc(s.x - rect.left, s.y - rect.top, r, 0, Math.PI * 2);
    ctx.fill();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    canvas.width = Math.round(rect.width);
    canvas.height = Math.round(rect.height);
    capture.current = new WebCapture(canvas, draw);
    const timer = setInterval(() => {
      const c = capture.current;
      if (!c) return;
      setWebSummary(summarizeWeb(c.samples.slice(-LIVE_WINDOW), c.frames.slice(-LIVE_WINDOW)));
      setNativeSummary(summarizeNative(native.current.slice(-LIVE_WINDOW)));
    }, 250);
    return () => {
      clearInterval(timer);
      capture.current?.stop();
    };
  }, [draw]);

  const startNative = useCallback(async () => {
    try {
      const info = await host.spike.nativeStart(
        (batch) => {
          const canvas = canvasRef.current;
          const rect = canvas?.getBoundingClientRect();
          const ctx = canvas?.getContext('2d');
          for (const p of batch.packets) {
            if (native.current.length < MAX_SAMPLES) native.current.push(p);
            if (ctx && rect && p.kind === 'pose') {
              // macOS reports window points from the bottom-left; Windows logical pixels from the top-left.
              const y = mode.nativeBackend === 'appkit-nsevent' ? window.innerHeight - p.y : p.y;
              ctx.fillStyle = '#ff6b6b';
              ctx.fillRect(p.x - rect.left - 1, y - rect.top - 1, 2, 2);
            }
          }
        },
        () => {
          invalidBatches.current++;
        },
      );
      nativeInfoRef.current = info;
      setNativeInfo(info);
      setStatus(`Native listener running (${info.backend}).`);
    } catch (e: unknown) {
      const err = toHostError(e);
      setStatus(`Native listener failed: ${err.code}: ${err.message}`);
    }
  }, [host, mode.nativeBackend, setStatus]);

  const simulate = useCallback(
    async (hz: number, frames: number) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      const c = capture.current;
      if (!rect || !c) return;
      const webStart = c.samples.length;
      const nativeStart = native.current.length;
      setStatus(`Simulating ${String(frames)} synthetic pen frames at ${String(hz)} Hz…`);
      try {
        const report = await host.spike.simulatePen({
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          hz,
          frames,
        });
        await sleep(500); // trailing events and native batches
        const web = matchSimulation(report, c.samples.slice(webStart), (s) => ({
          pressure: s.kind === 'move' ? s.pressure : null,
          tiltX: s.tiltX,
          tiltY: s.tiltY,
          epochMs: performance.timeOrigin + s.t,
        }));
        const nat = matchSimulation(report, native.current.slice(nativeStart), (p) => ({
          pressure: p.kind === 'pose' ? p.pressure : null,
          tiltX: p.tiltX,
          tiltY: p.tiltY,
          epochMs: p.hostEpochMs,
        }));
        simulations.current.push({ request: { hz, frames }, report, web, native: nat });
        setStatus(
          `Simulated ${String(report.frames)} frames at ${report.achievedHz.toFixed(0)} Hz: web received ` +
            `${String(web.received)}/${String(web.injected)}, native ${String(nat.received)}/${String(nat.injected)}.`,
        );
      } catch (e: unknown) {
        const err = toHostError(e);
        setStatus(`Simulation failed: ${err.code}: ${err.message}`);
      }
    },
    [host, setStatus],
  );

  const buildLog = useCallback(() => {
    const c = capture.current;
    const samples = c?.samples ?? [];
    return {
      spike: 'tablet',
      version: 1,
      savedAt: new Date().toISOString(),
      environment: {
        userAgent: navigator.userAgent,
        devicePixelRatio: window.devicePixelRatio,
        screen: { width: screen.width, height: screen.height },
        coalescedApi: c?.coalescedApi ?? false,
        mode,
      },
      summary: {
        web: summarizeWeb(samples, c?.frames ?? []),
        native: summarizeNative(native.current),
        nativeInfo: nativeInfoRef.current,
        invalidNativeBatches: invalidBatches.current,
      },
      statusLog: statusLog.current,
      simulations: simulations.current.map((s) => ({
        request: s.request,
        achievedHz: s.report.achievedHz,
        method: s.report.method,
        web: s.web,
        native: s.native,
      })),
      web: { samples, frames: c?.frames ?? [] },
      native: { packets: native.current },
    };
  }, [mode]);

  const save = useCallback(async () => {
    try {
      const saved = await host.spike.saveLog(buildLog());
      setStatus(`Log saved: ${saved.path} (${String(saved.bytes)} bytes).`);
    } catch (e: unknown) {
      const err = toHostError(e);
      setStatus(`Save failed: ${err.code}: ${err.message}`);
    }
  }, [host, buildLog, setStatus]);

  const clear = useCallback(() => {
    capture.current?.clear();
    native.current.length = 0;
    simulations.current.length = 0;
    const canvas = canvasRef.current;
    canvas?.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height);
  }, []);

  // Automated run (--spike-auto): native listener, synthetic-pen scenarios, save (the host then exits).
  useEffect(() => {
    if (!mode.auto || autoStarted.current) return;
    autoStarted.current = true;
    void (async () => {
      await sleep(1000);
      await startNative();
      for (const s of AUTO_SCENARIOS) {
        if (mode.penSimulation) await simulate(s.hz, s.frames);
        await sleep(300);
      }
      await save();
    })();
  }, [mode, startNative, simulate, save]);

  return (
    <div className="spike">
      <header className="spike-header">
        <strong>C0.2 tablet input spike</strong> — not product UI. Native backend:{' '}
        {mode.nativeBackend ?? 'none'}
        {nativeInfo
          ? ` (${nativeInfo.backend}; tablets: ${nativeInfo.tablets.join(', ') || 'none reported'})`
          : ''}
        . Log: {mode.logPath ?? 'not set (start with --spike-log <file>)'}.
      </header>
      <div className="spike-toolbar">
        <button type="button" onClick={() => void startNative()} disabled={nativeInfo !== null}>
          Start native listener
        </button>
        {mode.penSimulation && (
          <>
            <button type="button" onClick={() => void simulate(240, 1024)}>
              Simulate pen 240 Hz (synthetic)
            </button>
            <button type="button" onClick={() => void simulate(500, 1024)}>
              Simulate pen 500 Hz (synthetic)
            </button>
          </>
        )}
        <button type="button" onClick={clear}>
          Clear
        </button>
        <button type="button" onClick={() => void save()} disabled={!mode.logPath}>
          Save log
        </button>
        <span className="spike-status">{status}</span>
      </div>
      <canvas ref={canvasRef} className="spike-canvas" data-testid="spike-canvas" />
      <div className="spike-readout">
        <SummaryTable title="Web path (pointer events)" summary={webSummary} web={webSummary} />
        <SummaryTable title="Native path (host listener)" summary={nativeSummary} />
      </div>
      <footer className="spike-footer">
        Live view over the last {String(LIVE_WINDOW)} samples. Web handler latency is measured on the
        performance clock; simulation latency compares host and webview wall clocks.
      </footer>
    </div>
  );
}
