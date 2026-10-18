// Typed host API. UI code depends on this interface only, never on a transport or on the Tauri packages.
import { certify } from './schema';
import { createBenchApi, createSpikeApi, type BenchApi, type SpikeApi } from './spikeApi';
import type { HostTransport, Unlisten } from './transport';
import type { EngineInfo, ErrorInfo, MenuCommandEvent, PingRequest, PingResponse } from './types';
import { isEngineInfo, isErrorInfo, isMenuCommandEvent, isPingResponse } from './wireSchemas';

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

export interface HostApi extends BenchApi {
  readonly transportKind: HostTransport['kind'];
  ping(request: PingRequest): Promise<PingResponse>;
  appInfo(): Promise<EngineInfo>;
  onMenuCommand(handler: (event: MenuCommandEvent) => void): Promise<Unlisten>;
  /** M0 spikes (C0.2): reachable only when the host was started with --spike. */
  readonly spike: SpikeApi;
}

/** Refusal of every M0 diagnostic call in a build without them (C0.4; src/m0Diagnostics.d.ts). */
function notBuilt(): Promise<never> {
  return Promise.reject(
    new HostError({ code: 'diagnostics.notBuilt', message: 'this build contains no M0 diagnostics' }),
  );
}

/** The benchmark API of a build without M0 diagnostics: never a benchmark launch. */
const NO_BENCH: BenchApi = { benchPlan: () => Promise.resolve(null), benchReport: notBuilt };

/** The spike API of a build without M0 diagnostics: never a spike launch, every other call refused. */
const NO_SPIKES: SpikeApi = {
  mode: () => Promise.resolve(null),
  environment: notBuilt,
  nativeStart: notBuilt,
  nativeStop: notBuilt,
  simulatePen: notBuilt,
  probeNativeWindow: notBuilt,
  rawInputSink: notBuilt,
  saveLog: notBuilt,
  canvasScene: notBuilt,
  canvasPixels: notBuilt,
};

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
  const data = certify(check, value);
  if (data === undefined) {
    throw new HostError({ code: 'ipc.invalidResponse', message: `unexpected response from ${command}` });
  }
  return data;
}

export function createHostApi(transport: HostTransport): HostApi {
  return {
    transportKind: transport.kind,
    ping: (request) => call(transport, 'ping', { request }, isPingResponse),
    appInfo: () => call(transport, 'app_info', undefined, isEngineInfo),
    onMenuCommand: (handler) =>
      transport.listen(MENU_COMMAND_EVENT, (payload) => {
        // Malformed events are dropped: they can only come from a host/front-end version mismatch.
        const event = certify(isMenuCommandEvent, payload);
        if (event !== undefined) handler(event);
      }),
    // M0 diagnostics exist only in the diagnostics build. Their command names live in spikeApi.ts, which the default
    // bundle drops entirely, so it names none of them (C0.4).
    ...(__TT_M0_DIAGNOSTICS__ ? createBenchApi(transport) : NO_BENCH),
    spike: __TT_M0_DIAGNOSTICS__ ? createSpikeApi(transport) : NO_SPIKES,
  };
}
