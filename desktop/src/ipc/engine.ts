// The engine API of the desktop shell (M4 C4.1; ADR-0021): one entry point for every command, request cancellation,
// the key dispatch, sensor reports, the resync snapshot, native file pickers, and the numbered engine event stream.
//
// Every value crosses as JSON text (ADR-0012). Command parameters are validated against the generated schemas
// before they are sent and every answer is validated as a command envelope, so UI code only ever sees checked data.
import { isAvailability, type Availability, type CommandRef } from '../shell/availability';
import { COMMAND_DEFS, COMMAND_INDEX } from './generated/commands';
import type { Defs, Schema } from './schema';
import { violations } from './schema';
import type { HostTransport, Unlisten } from './transport';
import type { ErrorInfo } from './types';
import { certifyWireText, decodeWireBytes, parseWireJson, stringifyWireJson } from './wireJson';

const DEFS: Defs = COMMAND_DEFS;

/** An executor error (11 §10). */
export interface CommandFailure extends ErrorInfo {
  readonly hint?: string;
}

/** The command result envelope (11 §7.2), the fields the shell reads. */
export interface Envelope {
  readonly status: 'ok' | 'error' | 'needs_confirmation';
  readonly result?: unknown;
  readonly error?: CommandFailure;
  readonly warnings: readonly { readonly code: string; readonly message: string }[];
  readonly documentRev?: number | bigint | null;
  readonly undo?: unknown;
  readonly changes?: readonly unknown[];
}

/** One engine event (11 §9.2), numbered by the host in delivery order. */
export interface EngineEvent {
  readonly seq: number;
  /** The engine generation that emitted it (0 at start, +1 per restart; ruling O-14). */
  readonly generation?: number;
  readonly name: string;
  readonly payload: unknown;
}

/** Window event fired when the window adopts a new engine generation (reports must be made again). */
export const GENERATION_EVENT = 'tt-engine-generation';

/** What a key press resolved to and, when it ran a command, its envelope. */
export interface KeyOutcome {
  readonly outcome: 'unbound' | 'suspended' | 'command';
  readonly keys: string;
  readonly reason?: string;
  readonly commandId?: string;
  readonly params?: Record<string, unknown>;
  readonly enabled?: boolean;
  readonly dialog?: boolean;
  readonly envelope?: Envelope;
}

/** A sensor report (ADR-0021): facts only the shell knows. */
export interface ShellReport {
  modal?: { kind: 'dialog' | 'gesture' | 'progress'; title: string } | null;
  textFocus?: boolean;
  description?: string;
  pointer?: string;
  viewSizes?: Record<string, { width: number; height: number }>;
  screens?: { x: number; y: number; width: number; height: number; scale: number }[];
  attached?: boolean;
  uiTree?: unknown;
  renderedSeq?: number;
  messageShown?: {
    messageId: string;
    severity: string;
    text: string;
    details?: unknown;
    suppressed?: boolean;
  };
}

/** A native file or folder picker request. */
export interface PickRequest {
  kind: 'open' | 'save' | 'folder';
  title: string;
  filters?: { name: string; extensions: string[] }[];
  defaultPath?: string;
}

/** Options of a command run. */
export interface RunOptions {
  /** The caller offers Cancel: the host links a cancellation flag to the request (ADR-0020 decision 8). */
  cancellable?: boolean;
  dryRun?: boolean;
}

/** A command in flight. */
export interface Running {
  /** Request token when cancellable. */
  readonly token: string | null;
  readonly done: Promise<Envelope>;
}

export interface EngineApi {
  run(command: string, params?: Record<string, unknown>, options?: RunOptions): Running;
  cancel(token: string): Promise<void>;
  dispatchKey(keys: string): Promise<KeyOutcome>;
  report(report: ShellReport): Promise<void>;
  /** The engine generation the window follows; sensor reports name it (ruling O-14). */
  readonly generation: () => number;
  /** Adopts a new engine generation (after a restart): later reports name it and the window re-reports its facts. */
  setGeneration(generation: number): void;
  /** The availability of command references (ruling O-10): the kernel's one answer for every GUI surface. */
  availability(refs: readonly CommandRef[]): Promise<Availability[]>;
  pickPath(request: PickRequest): Promise<string | null>;
  onEvent(handler: (event: EngineEvent) => void): Promise<Unlisten>;
}

export const ENGINE_EVENT = 'engine-event';

interface IndexEntry {
  readonly params: string;
  readonly result: string;
}

const INDEX = COMMAND_INDEX as unknown as Readonly<Record<string, IndexEntry>>;

function ref(name: string): Schema {
  return { $ref: `#/$defs/${name}` };
}

/** A local refusal shaped like an executor error, so callers handle one kind of failure. */
export function localFailure(code: string, message: string, details?: unknown): Envelope {
  return { status: 'error', error: { code, message, details }, warnings: [] };
}

/** Whether a failure is the caller's own cancellation (SI-111: `command.timeout` with `reason: "cancelled"`). */
export function isCancelled(env: Envelope): boolean {
  if (env.error?.code !== 'command.timeout') return false;
  const d = env.error.details;
  return typeof d === 'object' && d !== null && (d as Record<string, unknown>)['reason'] === 'cancelled';
}

/** Whether a failure is a request the engine restart cut off (ruling O-14: `reason: "engineRestarted"`). */
export function isRestarted(env: Envelope): boolean {
  if (env.error?.code !== 'command.timeout') return false;
  const d = env.error.details;
  return (
    typeof d === 'object' && d !== null && (d as Record<string, unknown>)['reason'] === 'engineRestarted'
  );
}

/** Checks parameters against a command's generated schema; returns the problems (empty when valid). */
export function paramProblems(command: string, params: unknown): string[] {
  const entry = Object.prototype.hasOwnProperty.call(INDEX, command) ? INDEX[command] : undefined;
  if (!entry) return [`unknown command ${command}`];
  return violations(ref(entry.params), params, '', DEFS);
}

function decode(value: unknown): unknown {
  if (typeof value === 'string') return parseWireJson(value);
  if (value instanceof ArrayBuffer) {
    const text = decodeWireBytes(value);
    return text === undefined ? undefined : parseWireJson(text);
  }
  return value;
}

function asEnvelope(value: unknown): Envelope | undefined {
  try {
    let text: string | undefined;
    if (typeof value === 'string') text = value;
    else if (value instanceof ArrayBuffer) text = decodeWireBytes(value);
    if (text !== undefined) return certifyWireText<Envelope>(text, ref('envelope.root'), DEFS);
    return violations(ref('envelope.root'), value, '', DEFS).length === 0 ? (value as Envelope) : undefined;
  } catch {
    return undefined;
  }
}

function failureOf(error: unknown): Envelope {
  try {
    const v = decode(error);
    if (typeof v === 'object' && v !== null && typeof (v as Record<string, unknown>)['code'] === 'string') {
      const e = v as CommandFailure;
      return { status: 'error', error: e, warnings: [] };
    }
  } catch {
    // fall through
  }
  return localFailure('ipc.failed', error instanceof Error ? error.message : String(error));
}

let nextToken = 1;

export function createEngineApi(transport: HostTransport): EngineApi {
  const invoke = async (command: string, args: unknown): Promise<unknown> =>
    transport.invoke(command, { json: stringifyWireJson(args) });
  let generation = 0;

  return {
    generation: () => generation,
    setGeneration(next) {
      if (next === generation) return;
      generation = next;
      if (typeof window !== 'undefined')
        window.dispatchEvent(new CustomEvent(GENERATION_EVENT, { detail: next }));
    },
    run(command, params = {}, options = {}) {
      const problems = paramProblems(command, params);
      if (problems.length > 0) {
        return {
          token: null,
          done: Promise.resolve(
            localFailure('arg.invalid', `invalid parameters for ${command}: ${problems.join('; ')}`, {
              problems,
            }),
          ),
        };
      }
      const token = options.cancellable ? `r${String(nextToken++)}` : null;
      const request: Record<string, unknown> = { command, params };
      if (token) request['requestToken'] = token;
      if (options.dryRun) request['options'] = { dryRun: true };
      const done = invoke('run_command', request).then(
        (value) =>
          asEnvelope(value) ??
          localFailure('ipc.invalidResponse', `the answer to ${command} is not a command envelope`),
        (error: unknown) => failureOf(error),
      );
      return { token, done };
    },
    async cancel(token) {
      await invoke('cancel_request', { token });
    },
    async dispatchKey(keys) {
      const v = decode(await invoke('dispatch_key', { keys }));
      return v as KeyOutcome;
    },
    async report(report) {
      await invoke('report_shell', { ...report, generation });
    },
    async availability(refs) {
      const v = decode(await invoke('availability', { refs }));
      return Array.isArray(v) ? v.filter(isAvailability) : [];
    },
    async pickPath(request) {
      const v = decode(await invoke('pick_path', request));
      if (typeof v === 'object' && v !== null) {
        const path = (v as Record<string, unknown>)['path'];
        return typeof path === 'string' ? path : null;
      }
      return null;
    },
    onEvent(handler) {
      return transport.listen(ENGINE_EVENT, (payload) => {
        try {
          const v = decode(payload);
          if (typeof v !== 'object' || v === null) return;
          const o = v as Record<string, unknown>;
          if (typeof o['seq'] !== 'number' || typeof o['name'] !== 'string') return;
          const g = o['generation'];
          handler({
            seq: o['seq'],
            name: o['name'],
            payload: o['payload'],
            ...(typeof g === 'number' ? { generation: g } : {}),
          });
        } catch {
          // A malformed event can only come from a version mismatch; it is dropped.
        }
      });
    },
  };
}
