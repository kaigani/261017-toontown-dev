// Typed host API. UI code depends on this interface only, never on a transport or on the Tauri packages.
//
// Product calls cross as JSON text in both directions (ADR-0012): arguments are sent as `{json: <text>}` written by
// `stringifyWireJson`, and the host answers with JSON text (a UTF-8 body or a string), rejects with the JSON text of
// an `ErrorInfo`, and emits events as JSON text. Text is parsed losslessly (64-bit integers survive as bigint) and
// validated against the generated schemas. A transport that hands over objects instead (in-process test doubles)
// goes through `certify`: plain data only, structured-clone-able (no Proxy), snapshotted, validated.
import { certify, type Schema } from './schema';
import { createBenchApi, createSpikeApi, type BenchApi, type SpikeApi } from './spikeApi';
import type { HostTransport, Unlisten } from './transport';
import type { EngineInfo, ErrorInfo, MenuCommandEvent, PingRequest, PingResponse } from './types';
import { certifyWireText, decodeWireBytes, stringifyWireJson } from './wireJson';
import {
  engineInfoSchema,
  errorInfoSchema,
  isEngineInfo,
  isErrorInfo,
  isMenuCommandEvent,
  isPingResponse,
  menuCommandEventSchema,
  pingResponseSchema,
  WIRE,
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

/**
 * Accepts a value delivered by a transport: JSON text (string or UTF-8 bytes) is parsed losslessly and validated;
 * anything else must pass `certify`. Returns undefined for anything invalid; never throws.
 */
export function acceptWire<T>(
  value: unknown,
  schema: Schema,
  check: (value: unknown) => value is T,
): T | undefined {
  try {
    if (typeof value === 'string') return certifyWireText<T>(value, schema, WIRE);
    if (value instanceof ArrayBuffer) {
      const text = decodeWireBytes(value);
      return text === undefined ? undefined : certifyWireText<T>(text, schema, WIRE);
    }
    return certify(check, value);
  } catch {
    return undefined;
  }
}

/** Normalises anything a transport rejects with into a `HostError`. */
export function toHostError(error: unknown): HostError {
  if (error instanceof HostError) return error;
  const info = acceptWire<ErrorInfo>(error, errorInfoSchema, isErrorInfo);
  if (info !== undefined) return new HostError(info);
  let message: string;
  try {
    message = error instanceof Error ? error.message : String(error);
  } catch {
    message = 'unreadable error value';
  }
  return new HostError({ code: 'internal', message });
}

async function call<T>(
  transport: HostTransport,
  command: string,
  args: unknown,
  schema: Schema,
  check: (value: unknown) => value is T,
): Promise<T> {
  let value: unknown;
  try {
    value = await transport.invoke(
      command,
      args === undefined ? undefined : { json: stringifyWireJson(args) },
    );
  } catch (error: unknown) {
    throw toHostError(error);
  }
  const data = acceptWire(value, schema, check);
  if (data === undefined) {
    throw new HostError({ code: 'ipc.invalidResponse', message: `unexpected response from ${command}` });
  }
  return data;
}

export function createHostApi(transport: HostTransport): HostApi {
  return {
    transportKind: transport.kind,
    ping: (request) => call(transport, 'ping', request, pingResponseSchema, isPingResponse),
    appInfo: () => call(transport, 'app_info', undefined, engineInfoSchema, isEngineInfo),
    onMenuCommand: (handler) =>
      transport.listen(MENU_COMMAND_EVENT, (payload) => {
        // Malformed events are dropped: they can only come from a host/front-end version mismatch.
        const event = acceptWire(payload, menuCommandEventSchema, isMenuCommandEvent);
        if (event !== undefined) handler(event);
      }),
    // M0 diagnostics exist only in the diagnostics build. Their command names live in spikeApi.ts, which the default
    // bundle drops entirely, so it names none of them (C0.4).
    ...(__TT_M0_DIAGNOSTICS__ ? createBenchApi(transport) : NO_BENCH),
    spike: __TT_M0_DIAGNOSTICS__ ? createSpikeApi(transport) : NO_SPIKES,
  };
}
