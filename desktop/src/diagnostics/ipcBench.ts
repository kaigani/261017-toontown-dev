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
    for (const payloadBytes of plan.payloadBytes) {
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
  const plan = await host.benchPlan();
  if (!plan) return false;
  await host.benchReport(await runIpcBench(host, plan));
  return true;
}
