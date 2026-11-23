// M0 diagnostics: automated IPC latency benchmark, driven by `toontown --ipc-bench <out.json>` (see
// desktop/src-tauri/src/diagnostics.rs). Not product code; removed or gated behind a dev build after M0.
import type { BenchPlan, HostApi } from '../ipc';
import { measureBurst, type LatencySummary } from '../lib/latency';

export interface IpcBenchReport {
  ok: boolean;
  transport: HostApi['transportKind'];
  userAgent: string;
  count: number;
  bursts: { payloadBytes: number; roundTripMs: LatencySummary | null }[];
  error?: string;
  /** The start-up watchdog's findings (its report only): how far the benchmark got, what was uncaught. */
  startup?: { progress: string; errors: StartupError[] };
}

export async function runIpcBench(host: HostApi, plan: BenchPlan): Promise<IpcBenchReport> {
  const report: IpcBenchReport = {
    ok: true,
    transport: host.transportKind,
    userAgent: navigator.userAgent,
    count: plan.count,
    bursts: [],
  };
  try {
    for (const [i, payloadBytes] of plan.payloadBytes.entries()) {
      progress = `measuring ${payloadBytes} bytes (burst ${i + 1} of ${plan.payloadBytes.length})`;
      const roundTripMs = await measureBurst(host, plan.count, { payloadBytes, warmup: 20 });
      report.bursts.push({ payloadBytes, roundTripMs });
    }
  } catch (e: unknown) {
    report.ok = false;
    report.error = e instanceof Error ? e.message : String(e);
  }
  return report;
}

/** Runs the benchmark if the host requested one; resolves to true when it did. */
export async function maybeRunIpcBench(host: HostApi): Promise<boolean> {
  progress = 'waiting for the plan';
  const plan = await host.benchPlan();
  if (!plan) return false;
  const report = await runIpcBench(host, plan);
  if (reported) return true;
  reported = true;
  await host.benchReport(report);
  return true;
}

// ----- start-up watchdog (M5 C5.1b) ------------------------------------------------------------------------------------
// The benchmark starts from an effect of the shell's root component, so a start-up failure that unmounts the tree (an
// uncaught error in a render or an effect) or a front end too busy to finish leaves the host waiting until it exits
// with status 2 and nothing to say why. The watchdog, installed before the first render, reports the uncaught errors
// and rejections seen since start-up and how far the benchmark got, in time for the host to write them.

/** What the benchmark is doing now (the watchdog's report names it). */
let progress = 'not started';
/** Whether a report was sent (the benchmark's or the watchdog's: the host takes the first and exits). */
let reported = false;
const startupErrors: StartupError[] = [];

/** The watchdog's wait: well inside the host's 120 s (`diagnostics::TIMEOUT`). */
export const WATCHDOG_MS = 90_000;

/** One uncaught error or rejection seen since start-up. */
export interface StartupError {
  kind: 'error' | 'rejection';
  name: string | null;
  message: string;
  /** The first stack frames. */
  frames: string[];
}

function startupError(kind: StartupError['kind'], e: unknown, fallback?: string): StartupError {
  if (!(e instanceof Error)) return { kind, name: null, message: fallback ?? String(e), frames: [] };
  const frames = e.stack ? e.stack.split(/\r?\n/).slice(1, 6) : [];
  return { kind, name: e.name, message: e.message, frames: frames.map((f) => f.trim()) };
}

/** Records uncaught errors and, if the host requested a benchmark, reports them when no report has gone after
 *  `waitMs`. Diagnostics builds only (the caller checks `__TT_M0_DIAGNOSTICS__`). */
export function watchStartup(
  host: HostApi,
  target: Pick<Window, 'addEventListener'>,
  waitMs = WATCHDOG_MS,
): void {
  target.addEventListener('error', (e: Event) => {
    const ev = e as ErrorEvent;
    startupErrors.push(startupError('error', ev.error, ev.message));
  });
  target.addEventListener('unhandledrejection', (e: Event) => {
    startupErrors.push(startupError('rejection', (e as PromiseRejectionEvent).reason));
  });
  void host.benchPlan().then(
    (plan) => {
      if (!plan) return;
      setTimeout(() => {
        if (reported) return;
        reported = true;
        const report: IpcBenchReport = {
          ok: false,
          transport: host.transportKind,
          userAgent: navigator.userAgent,
          count: plan.count,
          bursts: [],
          error: `no benchmark report after ${waitMs / 1000} s (${startupErrors.length} uncaught at start-up)`,
          startup: { progress, errors: startupErrors.slice(0, 10) },
        };
        void host.benchReport(report).catch(() => undefined);
      }, waitMs);
    },
    () => undefined,
  );
}

/** Test hook: forget the module's state. */
export function resetWatchdogForTests(): void {
  progress = 'not started';
  reported = false;
  startupErrors.length = 0;
}
