// Typed host API. UI code depends on this interface only, never on a transport or on @tauri-apps/*.
import type { HostTransport, Unlisten } from './transport';
import type { BenchPlan, EngineInfo, ErrorInfo, MenuCommandEvent, PingRequest, PingResponse } from './types';

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

function isErrorInfo(value: unknown): value is ErrorInfo {
  return isRecord(value) && typeof value['code'] === 'string' && typeof value['message'] === 'string';
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

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null;
}

function isPingResponse(v: unknown): v is PingResponse {
  return (
    isRecord(v) &&
    typeof v['seq'] === 'number' &&
    typeof v['handlerNs'] === 'number' &&
    typeof v['engineUptimeNs'] === 'number' &&
    (v['payload'] === null || typeof v['payload'] === 'string')
  );
}

function isEngineInfo(v: unknown): v is EngineInfo {
  return (
    isRecord(v) &&
    typeof v['name'] === 'string' &&
    typeof v['version'] === 'string' &&
    typeof v['formatVersion'] === 'string' &&
    typeof v['registryVersion'] === 'string'
  );
}

function isMenuCommandEvent(v: unknown): v is MenuCommandEvent {
  return (
    isRecord(v) &&
    typeof v['command'] === 'string' &&
    isRecord(v['params']) &&
    (v['handler'] === 'frontend' || v['handler'] === 'pending') &&
    (v['milestone'] === null || typeof v['milestone'] === 'string')
  );
}

function isBenchPlanOrNull(v: unknown): v is BenchPlan | null {
  return (
    v === null ||
    (isRecord(v) &&
      typeof v['count'] === 'number' &&
      Array.isArray(v['payloadBytes']) &&
      v['payloadBytes'].every((n) => typeof n === 'number'))
  );
}

function isNull(v: unknown): v is null {
  return v === null;
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
