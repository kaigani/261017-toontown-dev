// C0.2 tablet spike page (spike code). Shown instead of the workspace only when the host was started with
// `--spike tablet`; never reachable from the menu. Compares the web path (pointer events in the webview) with the
// native path (host listener streaming over a channel) on the same strokes, live and in a JSON log.
//
// Log format version 3 (C02-R1-01, C02-R2-02/03): raw samples and packets, the raw host report of every synthetic
// run (with the injector's monotonic contact endpoints and foreground/focus snapshots), run boundaries (sample index
// ranges), the requested geometry and canvas rectangle, environment facts and truncation counters. Summaries in the
// log are for convenience only; `scripts/analyse-tablet-log.py` recomputes everything from the raw data.
import { useCallback, useEffect, useRef, useState } from 'react';

import {
  toHostError,
  useHost,
  type NativePacket,
  type NativeStartInfo,
  type PenPattern,
  type PenSimReport,
  type PenSimRequest,
  type ProbeResult,
  type SpikeEnvironment,
  type SpikeMode,
} from '../../ipc';
import {
  inferPressureScale,
  matchSimulation,
  summarizeNative,
  summarizeWeb,
  type LossEstimate,
  type NativeSummary,
  type SimulationMatch,
  type StreamSummary,
  type StrokeSummary,
  type WebSample,
  type WebSummary,
} from './analysis';
import { MAX_SAMPLES, WebCapture } from './capture';

export const LOG_VERSION = 3;

/**
 * The actions of the manual procedures (docs/manual-tests/M0-C0.2-tablet-macos.md, M0-C0.4-tablet-windows.md). The
 * person presses Space to start and to finish each action and K to skip it, so the analyzer can attribute contact
 * episodes to actions by time and ignore UI clicks (C04-R2-02). Keys produce no pointer events.
 */
const ACTIONS = [
  { id: 'A1', name: 'pressure ramp' },
  { id: 'A2', name: 'speed' },
  { id: 'A3', name: 'tilt' },
  { id: 'A4', name: 'barrel rotation (skip with K if the pen has none)' },
  { id: 'A5', name: 'hover (no contact)' },
  { id: 'A6', name: 'stationary pressure' },
] as const;

/** Minimum time between two marker transitions (C04-R3-02): a bouncing key or pen button cannot advance twice. */
const MIN_TRANSITION_MS = 500;

/** One raw Space/K keyboard event and what the page did with it (C04-R3-02). */
export interface KeyEventRecord {
  type: 'keydown' | 'keyup';
  key: string;
  repeat: boolean;
  t: number;
  outcome: string;
}

export interface ActionRecord {
  id: string;
  state: 'done' | 'skipped' | 'unfinished';
  /** performance.now() (the web samples' clock) and wall-clock epoch ms (the native packets' host clock). */
  startPerf: number;
  endPerf: number;
  startEpochMs: number;
  endEpochMs: number;
}
const LIVE_WINDOW = 5000;
const STROKES_SHOWN = 6;

interface Scenario {
  label: string;
  target: 'webview' | 'nativeWindow';
  hz: number;
  frames: number;
  pattern: PenPattern;
  /** Native-window runs: attach a RealTimeStylus to the probe window (default true). */
  rts?: boolean;
}

/** Target rectangle in CSS px, relative to the canvas origin. */
interface MatrixRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Every run of a matrix uses the same rectangle (C02-R2-03): the canvas origin and this size, clamped to the canvas.
 * 1012 × 275 is the round-1 runner canvas on which the sine path has four repeated pixels (frames 92/421/604/933).
 */
const MATRIX_SIZE = { width: 1012, height: 275 };

function matrixRectFor(canvas: DOMRect): MatrixRect {
  return {
    left: canvas.left,
    top: canvas.top,
    width: Math.min(MATRIX_SIZE.width, canvas.width),
    height: Math.min(MATRIX_SIZE.height, canvas.height),
  };
}

function inside(rect: MatrixRect, canvas: DOMRect): boolean {
  const eps = 0.5;
  return (
    rect.left >= canvas.left - eps &&
    rect.top >= canvas.top - eps &&
    rect.left + rect.width <= canvas.left + canvas.width + eps &&
    rect.top + rect.height <= canvas.top + canvas.height + eps
  );
}

const run240 = (label: string, target: Scenario['target'], pattern: PenPattern, rts?: boolean): Scenario => ({
  label,
  target,
  hz: 240,
  frames: 1024,
  pattern,
  ...(rts === undefined ? {} : { rts }),
});

// `--spike-auto` (Windows runner), all in one rectangle. Webview: sine at 240 and 500 Hz, zigzag (never the same
// pixel twice in a row), stationary (position fixed; pressure, tilt and rotation vary) and pressure-only (position,
// tilt and rotation fixed). Then the same trajectories into a plain same-process native window, without and with a
// RealTimeStylus attached (C02-R1-04; with RTS attached, Ink takes over the stream after a few WM_POINTER frames).
const AUTO_SCENARIOS: Scenario[] = [
  run240('webview sine 240 Hz', 'webview', 'sine'),
  { label: 'webview sine 500 Hz', target: 'webview', hz: 500, frames: 1024, pattern: 'sine' },
  run240('webview zigzag 240 Hz', 'webview', 'zigzag'),
  run240('webview stationary 240 Hz', 'webview', 'stationary'),
  run240('webview pressure-only 240 Hz', 'webview', 'pressureOnly'),
  run240('native window sine 240 Hz, no RTS', 'nativeWindow', 'sine', false),
  run240('native window stationary 240 Hz, no RTS', 'nativeWindow', 'stationary', false),
  run240('native window pressure-only 240 Hz, no RTS', 'nativeWindow', 'pressureOnly', false),
  run240('native window sine 240 Hz', 'nativeWindow', 'sine', true),
  run240('native window stationary 240 Hz', 'nativeWindow', 'stationary', true),
];

/** One synthetic run: boundaries in the raw streams plus everything the host returned. */
interface RunRecord {
  label: string;
  target: 'webview' | 'nativeWindow';
  request: PenSimRequest;
  /** Canvas rectangle (CSS px, viewport) when the run started; the request must lie inside it. */
  canvasRect: MatrixRect;
  startedAt: string;
  /** performance.now() at the start and end of the run. */
  perfStart: number;
  perfEnd: number | null;
  /** Index ranges `[start, end)` into web.samples and native.packets. */
  webRange: [number, number];
  nativeRange: [number, number];
  /** Raw host report (webview runs) — verbatim. */
  report: PenSimReport | null;
  /** Raw probe result (native-window runs) — verbatim. */
  probe: ProbeResult | null;
  /** Raw-input listener result during a webview run (null if unavailable). */
  rawInput: ProbeResult | null;
  error: string | null;
}

interface SimulationRecord {
  label: string;
  request: PenSimRequest;
  report: PenSimReport;
  web: SimulationMatch;
  native: SimulationMatch;
}

/** Facts typed in by the person running the test (macOS manual run), saved with the log. */
interface RunMetadata {
  tester: string;
  computer: string;
  osVersion: string;
  webkit: string;
  tablet: string;
  driver: string;
  /** `yes` / `no` from the pen's specification (ADR-0009 Step 0: a capability the pen lacks is n/a). */
  penTilt: string;
  penRotation: string;
  notes: string;
}

const EMPTY_METADATA: RunMetadata = {
  tester: '',
  computer: '',
  osVersion: '',
  webkit: '',
  tablet: '',
  driver: '',
  penTilt: '',
  penRotation: '',
  notes: '',
};

const METADATA_FIELDS: { key: keyof RunMetadata; label: string; hint: string }[] = [
  { key: 'computer', label: 'Computer', hint: 'e.g. MacBook Pro 14" M3, 2023' },
  { key: 'osVersion', label: 'macOS / Windows version', hint: 'e.g. macOS 15.3 (24D60)' },
  { key: 'webkit', label: 'Safari / WebKit version', hint: 'Safari → About Safari' },
  { key: 'tablet', label: 'Tablet model', hint: 'e.g. Wacom Intuos Pro M (PTH-660)' },
  { key: 'driver', label: 'Tablet driver version', hint: 'Wacom Center → About' },
  { key: 'penTilt', label: 'Pen has tilt (yes / no)', hint: "from the pen's specification" },
  { key: 'penRotation', label: 'Pen has barrel rotation (yes / no)', hint: 'e.g. yes for the Wacom Art Pen' },
  { key: 'tester', label: 'Your name (optional)', hint: '' },
  { key: 'notes', label: 'Notes', hint: 'anything unusual' },
];

interface GpuInfo {
  available: boolean;
  vendor?: string | undefined;
  architecture?: string | undefined;
  device?: string | undefined;
  description?: string | undefined;
  error?: string;
}

// WebGPU (and, per the compat data, hardwareConcurrency) are not in the Safari 15 floor: feature-detect them
// through a local interface.
interface MaybeGpuNavigator {
  hardwareConcurrency?: number;
  gpu?: {
    requestAdapter: () => Promise<{
      info?: { vendor?: string; architecture?: string; device?: string; description?: string };
    } | null>;
  };
}

async function gpuInfo(): Promise<GpuInfo> {
  const gpu = (navigator as unknown as MaybeGpuNavigator).gpu;
  if (!gpu) return { available: false };
  try {
    const adapter = await gpu.requestAdapter();
    const info = adapter?.info;
    return {
      available: adapter !== null,
      vendor: info?.vendor,
      architecture: info?.architecture,
      device: info?.device,
      description: info?.description,
    };
  } catch (e: unknown) {
    return { available: false, error: String(e) };
  }
}

const sleep = (ms: number) =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });

function fmt(v: number | null | undefined, digits = 1): string {
  return v === null || v === undefined ? '–' : v.toFixed(digits);
}

function lossText(loss: LossEstimate): string {
  return loss.basis === 'reference'
    ? `${String(loss.missing)} of ${String(loss.expected)} (${loss.reference})`
    : loss.note;
}

function errorText(e: unknown): string {
  const err = toHostError(e);
  return `${err.code}: ${err.message}`;
}

function SummaryTable({
  title,
  summary,
  web,
  native,
}: {
  title: string;
  summary: StreamSummary | null;
  web?: WebSummary | null;
  native?: NativeSummary | null;
}) {
  const s = summary;
  const rows: [string, string][] = s
    ? [
        ['samples (contact)', `${String(s.samples)} (${String(s.contactSamples)})`],
        [
          'pointer types',
          Object.entries(s.pointerTypes)
            .map(([k, n]) => `${k} ${String(n)}`)
            .join(', ') || '–',
        ],
        ['strokes', String(s.strokes.length)],
        ['rate in contact (Hz)', `${fmt(s.rateHz)} (median ${fmt(s.medianRateHz)})`],
        ['interval median / max (ms)', `${fmt(s.medianIntervalMs, 2)} / ${fmt(s.maxIntervalMs, 2)}`],
        ['intervals > 2× stroke median', `${String(s.gaps)} (heuristic)`],
        ['loss', lossText(s.loss)],
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
  if (native) rows.push(['clock', native.clock === 'device' ? 'device (deviceMs)' : 'host wall clock']);
  if (web) {
    const l = web.handlerLatencyMs;
    rows.push(
      ['getCoalescedEvents API', web.coalescedSupported ? 'available' : 'not available'],
      ['batching observed', web.coalescedObserved ? 'yes' : 'no'],
      [
        'coalesced per dispatch (p50 / max)',
        `${fmt(web.coalescedPerDispatch?.p50, 0)} / ${fmt(web.coalescedPerDispatch?.max, 0)}`,
      ],
      [
        'samples per frame (p50 / max)',
        `${fmt(web.samplesPerFrame?.p50, 0)} / ${fmt(web.samplesPerFrame?.max, 0)}`,
      ],
      [
        'handler − event p50/p95 (ms): oldest / newest / all',
        `${fmt(l.oldestCoalesced?.p50, 2)}/${fmt(l.oldestCoalesced?.p95, 2)} · ` +
          `${fmt(l.newestCoalesced?.p50, 2)}/${fmt(l.newestCoalesced?.p95, 2)} · ` +
          `${fmt(l.allSamples?.p50, 2)}/${fmt(l.allSamples?.p95, 2)}`,
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
      <StrokeTable strokes={s?.strokes ?? []} />
    </section>
  );
}

function StrokeTable({ strokes }: { strokes: StrokeSummary[] }) {
  const shown = strokes.slice(-STROKES_SHOWN);
  if (shown.length === 0) return null;
  return (
    <table className="spike-strokes">
      <thead>
        <tr>
          <th>stroke</th>
          <th>samples</th>
          <th>ms</th>
          <th>Hz</th>
          <th>median / max ms</th>
          <th>&gt;2× median</th>
          <th>pressure levels</th>
          <th>tilt</th>
        </tr>
      </thead>
      <tbody>
        {shown.map((s) => (
          <tr key={s.index}>
            <td>
              {String(s.index + 1)}
              {s.truncated ? '*' : ''}
            </td>
            <td>{String(s.samples)}</td>
            <td>{fmt(s.durationMs, 0)}</td>
            <td>{fmt(s.rateHz)}</td>
            <td>
              {fmt(s.medianIntervalMs, 2)} / {fmt(s.maxIntervalMs, 1)}
            </td>
            <td>{String(s.gapsOver2xMedian)}</td>
            <td>{String(s.pressure.distinct)}</td>
            <td>{s.tiltPresent ? 'yes' : 'no'}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function hidReports(result: ProbeResult | null): string {
  const listeners = result?.['listeners'] as { rawInput?: { hidReports?: number } } | null | undefined;
  const n = listeners?.rawInput?.hidReports;
  return n === undefined ? '–' : String(n);
}

export function TabletSpike({ mode }: { mode: SpikeMode }) {
  const host = useHost();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const capture = useRef<WebCapture | null>(null);
  const native = useRef<NativePacket[]>([]);
  const nativeCounters = useRef({ invalidBatches: 0, hostDropped: 0, sendFailures: 0, truncated: 0 });
  const runs = useRef<RunRecord[]>([]);
  const simulations = useRef<SimulationRecord[]>([]);
  const hostEnvironment = useRef<{ value: SpikeEnvironment | null; error: string | null }>({
    value: null,
    error: null,
  });
  const gpu = useRef<GpuInfo | null>(null);
  const [metadata, setMetadata] = useState<RunMetadata>(EMPTY_METADATA);
  const metadataRef = useRef<RunMetadata>(EMPTY_METADATA);
  const [nativeInfo, setNativeInfo] = useState<NativeStartInfo | null>(null);
  const nativeInfoRef = useRef<NativeStartInfo | null>(null);
  const [status, setStatusText] = useState('Draw on the canvas with the pen.');
  const statusLog = useRef<{ at: string; message: string }[]>([]);
  const setStatus = useCallback((message: string) => {
    statusLog.current.push({ at: new Date().toISOString(), message });
    setStatusText(message);
  }, []);
  const [webSummary, setWebSummary] = useState<WebSummary | null>(null);
  const [nativeSummary, setNativeSummary] = useState<NativeSummary | null>(null);
  const [runRows, setRunRows] = useState<RunRecord[]>([]);
  const autoStarted = useRef(false);
  const actions = useRef<ActionRecord[]>([]);
  const [actionStep, setActionStep] = useState<{ index: number; startPerf: number | null }>({
    index: 0,
    startPerf: null,
  });
  const actionStepRef = useRef(actionStep);
  const keyEvents = useRef<KeyEventRecord[]>([]);
  const lastTransition = useRef(Number.NEGATIVE_INFINITY);

  // Drawing: web samples as a pressure-width line, native packets as red dots.
  const draw = useCallback((s: WebSample) => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx || (s.kind !== 'move' && s.kind !== 'down')) return;
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

  // Environment facts, collected once (not on Save, so saving never waits for them).
  useEffect(() => {
    host.spike.environment().then(
      (value) => {
        hostEnvironment.current = { value, error: null };
      },
      (e: unknown) => {
        hostEnvironment.current = { value: null, error: errorText(e) };
      },
    );
    void gpuInfo().then((info) => {
      gpu.current = info;
    });
  }, [host]);

  const startNative = useCallback(async () => {
    try {
      const info = await host.spike.nativeStart(
        (batch) => {
          const canvas = canvasRef.current;
          const rect = canvas?.getBoundingClientRect();
          const ctx = canvas?.getContext('2d');
          nativeCounters.current.hostDropped = batch.hostDropped;
          nativeCounters.current.sendFailures = batch.sendFailures;
          for (const p of batch.packets) {
            if (native.current.length < MAX_SAMPLES) native.current.push(p);
            else nativeCounters.current.truncated++;
            if (ctx && rect && p.kind === 'pose') {
              // macOS reports window points from the bottom-left; Windows logical pixels from the top-left.
              const y = mode.nativeBackend === 'appkit-nsevent' ? window.innerHeight - p.y : p.y;
              ctx.fillStyle = '#ff6b6b';
              ctx.fillRect(p.x - rect.left - 1, y - rect.top - 1, 2, 2);
            }
          }
        },
        () => {
          nativeCounters.current.invalidBatches++;
        },
      );
      nativeInfoRef.current = info;
      setNativeInfo(info);
      setStatus(`Native listener running (${info.backend}).`);
    } catch (e: unknown) {
      setStatus(`Native listener failed: ${errorText(e)}`);
    }
  }, [host, mode.nativeBackend, setStatus]);

  const runScenario = useCallback(
    async (scenario: Scenario, target?: MatrixRect): Promise<RunRecord | null> => {
      const canvas = canvasRef.current?.getBoundingClientRect();
      const c = capture.current;
      if (!canvas || !c) return null;
      const rect = target ?? matrixRectFor(canvas);
      const request: PenSimRequest = {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        hz: scenario.hz,
        frames: scenario.frames,
        pattern: scenario.pattern,
      };
      const run: RunRecord = {
        label: scenario.label,
        target: scenario.target,
        request,
        canvasRect: { left: canvas.left, top: canvas.top, width: canvas.width, height: canvas.height },
        startedAt: new Date().toISOString(),
        perfStart: performance.now(),
        perfEnd: null,
        webRange: [c.samples.length, c.samples.length],
        nativeRange: [native.current.length, native.current.length],
        report: null,
        probe: null,
        rawInput: null,
        error: null,
      };
      runs.current.push(run);
      setStatus(`${scenario.label}: injecting ${String(scenario.frames)} synthetic pen frames…`);
      const finish = () => {
        run.perfEnd = performance.now();
        run.webRange[1] = c.samples.length;
        run.nativeRange[1] = native.current.length;
        setRunRows([...runs.current]);
      };
      if (!inside(rect, canvas)) {
        // The layout moved since the matrix rectangle was chosen: refuse rather than inject outside the canvas.
        run.error = 'spike.geometryChanged: the matrix rectangle is no longer inside the canvas';
        finish();
        setStatus(`${scenario.label} failed: ${run.error}`);
        return run;
      }
      if (scenario.target === 'nativeWindow') {
        try {
          run.probe = await host.spike.probeNativeWindow(request, scenario.rts ?? true);
          setStatus(`${scenario.label}: probe finished.`);
        } catch (e: unknown) {
          run.error = errorText(e);
          setStatus(`${scenario.label} failed: ${run.error}`);
        }
        finish();
        return run;
      }
      // Raw-input listener alongside the webview run (Windows); its absence is recorded, not fatal.
      const sink = host.spike.rawInputSink(true).then(
        () => true,
        (e: unknown) => {
          run.rawInput = { error: errorText(e) };
          return false;
        },
      );
      try {
        const sinkRunning = await sink;
        const report = await host.spike.simulatePen(request);
        run.report = report;
        await sleep(500); // trailing events and native batches
        finish();
        const web = matchSimulation(report, c.samples.slice(run.webRange[0], run.webRange[1]), (s) => ({
          pressure: s.kind === 'up' ? null : s.pressure,
          tiltX: s.tiltX,
          tiltY: s.tiltY,
          epochMs: performance.timeOrigin + s.t,
          handlerEpochMs: performance.timeOrigin + s.handlerT,
        }));
        const nativeRun = native.current.slice(run.nativeRange[0], run.nativeRange[1]);
        const nat = matchSimulation(
          report,
          nativeRun,
          (p) => ({
            pressure: p.kind === 'pose' ? p.pressure : null,
            tiltX: p.tiltX,
            tiltY: p.tiltY,
            epochMs: p.hostEpochMs,
          }),
          inferPressureScale(nativeRun.filter((p) => p.kind === 'pose').map((p) => p.pressure)),
        );
        simulations.current.push({ label: scenario.label, request, report, web, native: nat });
        setStatus(
          `${scenario.label}: ${String(report.frames)} frames at ${report.achievedHz.toFixed(0)} Hz; web received ` +
            `${String(web.received)}/${String(web.injected)}, native ${String(nat.received)}/${String(nat.injected)}.`,
        );
        if (sinkRunning) {
          host.spike.rawInputSink(false).then(
            (result) => {
              run.rawInput = result;
              setRunRows([...runs.current]);
            },
            (e: unknown) => {
              run.rawInput = { error: errorText(e) };
            },
          );
        }
      } catch (e: unknown) {
        run.error = errorText(e);
        finish();
        setStatus(`${scenario.label} failed: ${run.error}`);
        if (await sink) void host.spike.rawInputSink(false).catch(() => undefined);
      }
      return run;
    },
    [host, setStatus],
  );

  const simulate = useCallback(
    (hz: number) =>
      runScenario({
        label: `webview sine ${String(hz)} Hz`,
        target: 'webview',
        hz,
        frames: 1024,
        pattern: 'sine',
      }),
    [runScenario],
  );

  // Action markers (C04-R2-02, C04-R3-02): Space starts / finishes the current action, K skips it. A transition
  // happens on **keyup** of a deliberate press only: auto-repeat keydowns are ignored, a keyup needs its own
  // non-repeat keydown, and transitions are at least MIN_TRANSITION_MS apart. Every Space/K event is recorded in
  // `keyEvents` with what it did, so a corrupted sequence can be diagnosed. Ignored while typing in the metadata
  // form and in --spike-auto runs.
  useEffect(() => {
    if (mode.auto) return;
    const pressed = new Set<string>();
    // A transition is logged with the same time as its action record, so the analyzer can match them (C04-R4-02).
    const log = (e: KeyboardEvent, key: string, outcome: string, t = performance.now()) => {
      keyEvents.current.push({
        type: e.type === 'keyup' ? 'keyup' : 'keydown',
        key,
        repeat: e.repeat,
        t,
        outcome,
      });
    };
    const onKey = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      if (key !== ' ' && key !== 'k') return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
        log(e, key, 'ignored: typing in the form');
        return;
      }
      // Also stops Space from activating a focused toolbar button (browsers click on keyup).
      e.preventDefault();
      if (e.type === 'keydown') {
        if (e.repeat) {
          log(e, key, 'ignored: auto-repeat');
        } else {
          pressed.add(key);
          log(e, key, 'pressed');
        }
        return;
      }
      if (!pressed.delete(key)) {
        log(e, key, 'ignored: keyup without its keydown');
        return;
      }
      const now = performance.now();
      if (now - lastTransition.current < MIN_TRANSITION_MS) {
        log(e, key, `ignored: within ${String(MIN_TRANSITION_MS)} ms of the last transition`);
        return;
      }
      const step = actionStepRef.current;
      const action = ACTIONS[step.index];
      if (!action) {
        log(e, key, 'ignored: all actions recorded');
        return;
      }
      const record = (state: ActionRecord['state'], begin: number) => ({
        id: action.id,
        state,
        startPerf: begin,
        endPerf: now,
        startEpochMs: performance.timeOrigin + begin,
        endEpochMs: performance.timeOrigin + now,
      });
      let next: typeof step;
      if (key === ' ' && step.startPerf === null) {
        next = { index: step.index, startPerf: now };
        log(e, key, `transition: ${action.id} started`, now);
        setStatus(`${action.id} ${action.name}: started. Press Space when it is done.`);
      } else if (key === ' ' && step.startPerf !== null) {
        actions.current.push(record('done', step.startPerf));
        next = { index: step.index + 1, startPerf: null };
        log(e, key, `transition: ${action.id} done`, now);
        setStatus(`${action.id} ${action.name}: done.`);
      } else if (key === 'k' && step.startPerf === null) {
        actions.current.push(record('skipped', now));
        next = { index: step.index + 1, startPerf: null };
        log(e, key, `transition: ${action.id} skipped`, now);
        setStatus(`${action.id} ${action.name}: skipped.`);
      } else {
        log(e, key, 'ignored: K while an action is running');
        return;
      }
      lastTransition.current = now;
      actionStepRef.current = next;
      setActionStep(next);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('keyup', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('keyup', onKey);
    };
  }, [mode.auto, setStatus]);

  const buildLog = useCallback(() => {
    const c = capture.current;
    const samples = c?.samples ?? [];
    const frames = c?.frames ?? [];
    return {
      spike: 'tablet',
      version: LOG_VERSION,
      // How the log came about (C02-R3-01): `auto` = --spike-auto; `interactive` = synthetic strokes injected from
      // the toolbar; `manual` = no synthetic stroke at all (a hardware capture). The analyzer treats a log as a
      // manual capture only when it says so here.
      source: mode.auto ? 'auto' : runs.current.length > 0 ? 'interactive' : 'manual',
      matrixSize: MATRIX_SIZE,
      savedAt: new Date().toISOString(),
      metadata: metadataRef.current,
      environment: {
        timeOrigin: performance.timeOrigin,
        perfNowAtSave: performance.now(),
        userAgent: navigator.userAgent,
        hardwareConcurrency: (navigator as unknown as MaybeGpuNavigator).hardwareConcurrency ?? null,
        devicePixelRatio: window.devicePixelRatio,
        screen: { width: screen.width, height: screen.height },
        viewport: { width: window.innerWidth, height: window.innerHeight },
        coalescedApi: c?.coalescedApi ?? false,
        gpu: gpu.current,
        host: hostEnvironment.current.value,
        hostError: hostEnvironment.current.error,
        mode,
      },
      nativeInfo: nativeInfoRef.current,
      // Action markers; an action still running at Save is recorded as `unfinished`.
      actions: [
        ...actions.current,
        ...(() => {
          const step = actionStepRef.current;
          const action = ACTIONS[step.index];
          if (step.startPerf === null || !action) return [];
          const now = performance.now();
          return [
            {
              id: action.id,
              state: 'unfinished' as const,
              startPerf: step.startPerf,
              endPerf: now,
              startEpochMs: performance.timeOrigin + step.startPerf,
              endEpochMs: performance.timeOrigin + now,
            },
          ];
        })(),
      ],
      // Every Space/K event with what it did (C04-R3-02).
      keyEvents: keyEvents.current,
      runs: runs.current,
      simulations: simulations.current,
      web: {
        samples,
        frames,
        contextMenus: c?.contextMenus ?? [],
        truncated: c?.truncated ?? 0,
        maxSamples: MAX_SAMPLES,
      },
      native: { packets: native.current, ...nativeCounters.current, maxSamples: MAX_SAMPLES },
      statusLog: statusLog.current,
      // Convenience only: the analyzer recomputes all of this from the raw data above.
      summary: { web: summarizeWeb(samples, frames), native: summarizeNative(native.current) },
    };
  }, [mode]);

  const save = useCallback(async () => {
    const missing = METADATA_FIELDS.filter(
      (f) => f.key !== 'tester' && f.key !== 'notes' && metadataRef.current[f.key].trim() === '',
    ).map((f) => f.label);
    try {
      const saved = await host.spike.saveLog(buildLog());
      setStatus(
        `Log saved: ${saved.path} (${String(saved.bytes)} bytes).` +
          (missing.length > 0 && !mode.auto ? ` Not filled in: ${missing.join(', ')}.` : ''),
      );
    } catch (e: unknown) {
      setStatus(`Save failed: ${errorText(e)}`);
    }
  }, [host, buildLog, setStatus, mode.auto]);

  const clear = useCallback(() => {
    capture.current?.clear();
    native.current.length = 0;
    runs.current.length = 0;
    simulations.current.length = 0;
    nativeCounters.current.truncated = 0;
    actions.current.length = 0;
    keyEvents.current.length = 0;
    lastTransition.current = Number.NEGATIVE_INFINITY;
    actionStepRef.current = { index: 0, startPerf: null };
    setActionStep(actionStepRef.current);
    setRunRows([]);
    const canvas = canvasRef.current;
    canvas?.getContext('2d')?.clearRect(0, 0, canvas.width, canvas.height);
  }, []);

  const editMetadata = useCallback((key: keyof RunMetadata, value: string) => {
    metadataRef.current = { ...metadataRef.current, [key]: value };
    setMetadata(metadataRef.current);
  }, []);

  // Automated run (--spike-auto): native listener, synthetic-pen scenarios, save (the host then exits).
  useEffect(() => {
    if (!mode.auto || autoStarted.current) return;
    autoStarted.current = true;
    void (async () => {
      await sleep(1000);
      await startNative();
      if (mode.penSimulation) {
        // Let the page render the listener state first, then fix the rectangle for the whole matrix.
        await sleep(500);
        const canvas = canvasRef.current?.getBoundingClientRect();
        const rect = canvas ? matrixRectFor(canvas) : undefined;
        for (const s of AUTO_SCENARIOS) {
          const run = await runScenario(s, rect);
          // A refused stroke (something covered the target, e.g. a transient system window) is kept as a failed
          // run and retried once, as a separate run, after a pause.
          if (run?.error?.startsWith('spike.notForeground')) {
            await sleep(1500);
            await runScenario({ ...s, label: `${s.label} (retry)` }, rect);
          }
          await sleep(800);
        }
      }
      await save();
    })();
  }, [mode, startNative, runScenario, save]);

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
            <button type="button" onClick={() => void simulate(240)}>
              Simulate pen 240 Hz (synthetic)
            </button>
            <button type="button" onClick={() => void simulate(500)}>
              Simulate pen 500 Hz (synthetic)
            </button>
            <button
              type="button"
              onClick={() =>
                void runScenario(run240('native window sine 240 Hz, no RTS', 'nativeWindow', 'sine', false))
              }
            >
              Probe native window (synthetic)
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
      {!mode.auto && (
        <div className="spike-actions" data-testid="spike-actions">
          {(() => {
            const action = ACTIONS[actionStep.index];
            if (!action) return 'All actions recorded. Click Save log.';
            return actionStep.startPerf === null
              ? `Next: ${action.id} ${action.name}. Press Space to start (K = skip).`
              : `${action.id} ${action.name} running. Draw it, lift the pen, then press Space.`;
          })()}
        </div>
      )}
      <canvas ref={canvasRef} className="spike-canvas" data-testid="spike-canvas" />
      <div className="spike-readout">
        <SummaryTable title="Web path (pointer events)" summary={webSummary} web={webSummary} />
        <SummaryTable title="Native path (host listener)" summary={nativeSummary} native={nativeSummary} />
        <fieldset className="spike-meta">
          <legend>About this run (saved with the log)</legend>
          {METADATA_FIELDS.map((f) => (
            <label key={f.key}>
              <span>{f.label}</span>
              <input
                type="text"
                value={metadata[f.key]}
                placeholder={f.hint}
                onChange={(e) => {
                  editMetadata(f.key, e.target.value);
                }}
              />
            </label>
          ))}
        </fieldset>
      </div>
      <div className="spike-runs-box">
        <table className="spike-runs">
          <thead>
            <tr>
              <th>synthetic run</th>
              <th>web samples</th>
              <th>native packets</th>
              <th>raw HID reports</th>
              <th>result</th>
            </tr>
          </thead>
          <tbody>
            {runRows.map((r, i) => (
              <tr key={`${r.label}-${String(i)}`}>
                <td>{r.label}</td>
                <td>{String(r.webRange[1] - r.webRange[0])}</td>
                <td>{String(r.nativeRange[1] - r.nativeRange[0])}</td>
                <td>{hidReports(r.target === 'nativeWindow' ? r.probe : r.rawInput)}</td>
                <td>{r.error ?? ((r.report ?? r.probe) ? 'done (see log)' : '…')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <footer className="spike-footer">
        Live view over the last {String(LIVE_WINDOW)} samples; per-stroke rows show the last{' '}
        {String(STROKES_SHOWN)} strokes (* = cut off by the window). Loss is only stated against a reference
        (injected frames, host sequence numbers); long intervals alone are a heuristic. Handler latency is on
        the performance clock; simulation latency compares host and webview wall clocks.
      </footer>
    </div>
  );
}
