// Typed host API. UI code depends on this interface only, never on a transport or on @tauri-apps/*.
import type { HostTransport, Unlisten } from './transport';
import type { BenchPlan, EngineInfo, ErrorInfo, MenuCommandEvent, PingRequest, PingResponse } from './types';
import {
  isBenchPlanOrNull,
  isEngineInfo,
  isErrorInfo,
  isMenuCommandEvent,
  isNull,
  isPingResponse,
} from './wireSchemas';

export const MENU_COMMAND_EVENT = 'menu-command';

/** A failed host call, carrying the host's stable error code. */
export class HostError extends Error {
  readonly code: string;
  readonly details: unknown;

  constructor(info: ErrorInfo) {
    super(info.message);
    this.name = 'HostError';
    this.code = info.code;
    this.details = info.details;
  }
}

export interface HostApi {
  readonly transportKind: HostTransport['kind'];
  ping(request: PingRequest): Promise<PingResponse>;
  appInfo(): Promise<EngineInfo>;
  onMenuCommand(handler: (event: MenuCommandEvent) => void): Promise<Unlisten>;
  /** M0 diagnostics: the benchmark requested on the command line, or null. */
  benchPlan(): Promise<BenchPlan | null>;
  /** M0 diagnostics: hands the measurements to the host, which writes them and exits. */
  benchReport(report: unknown): Promise<void>;
}

/** Normalises anything a transport rejects with into a `HostError`. */
export function toHostError(error: unknown): HostError {
  if (error instanceof HostError) return error;
  if (isErrorInfo(error)) return new HostError(error);
  const message = error instanceof Error ? error.message : String(error);
  return new HostError({ code: 'internal', message });
}

async function call<T>(
  transport: HostTransport,
  command: string,
  args: Record<string, unknown> | undefined,
  check: (value: unknown) => value is T,
): Promise<T> {
  let value: unknown;
  try {
    value = await transport.invoke(command, args);
  } catch (error: unknown) {
    throw toHostError(error);
  }
  if (!check(value)) {
    throw new HostError({ code: 'ipc.invalidResponse', message: `unexpected response from ${command}` });
  }
  return value;
}

export function createHostApi(transport: HostTransport): HostApi {
  return {
    transportKind: transport.kind,
    ping: (request) => call(transport, 'ping', { request }, isPingResponse),
    appInfo: () => call(transport, 'app_info', undefined, isEngineInfo),
    onMenuCommand: (handler) =>
      transport.listen(MENU_COMMAND_EVENT, (payload) => {
        // Malformed events are dropped: they can only come from a host/front-end version mismatch.
        if (isMenuCommandEvent(payload)) handler(payload);
      }),
    benchPlan: () => call(transport, 'bench_plan', undefined, isBenchPlanOrNull),
    benchReport: async (report) => {
      await call(transport, 'bench_report', { report }, isNull);
    },
  };
}
