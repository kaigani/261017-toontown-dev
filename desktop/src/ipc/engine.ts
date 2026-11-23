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
  /** The focused text field's kind (SI-193): the canvas's text editor gives undo and redo to the document. */
  textFieldKind?: 'canvasText' | 'ordinary';
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

/** One delivery of the agent token to this window (D-65 item 2). */
export interface TokenDelivery {
  token: string;
  delivery: number;
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
  /** `canvas_scene` (M4 C4.2b; design note 1 §9): a Drawing view's composition, still unvalidated
   *  (`canvas/scene.ts` validates it). Rejects with the host's error value. */
  canvasScene(viewId: string): Promise<unknown>;
  /** `canvas_load`: one drawing's TTRD payload (validated by `canvas/format.ts`). Rejects with the host's error value. */
  canvasLoad(
    drawingId: string,
    bucket: number,
    outline?: boolean,
    fastPencil?: boolean,
    /** Which objects (M4 C4.3b, preview note §3): every object, the selected ones, or the rest. */
    part?: 'all' | 'selection' | 'rest',
  ): Promise<ArrayBuffer>;
  /** `gesture_commit` (M4 C4.2b; design note 2 §6): a gesture made in a view window, as `pointer` request fields; the
   *  host runs the headless `pointer` driver with the user's origin and answers its envelope. The request names the
   *  generation the window follows; a gesture of a replaced engine is refused, nothing run (D-7b). */
  gestureCommit(request: {
    view: string;
    pointerType?: 'mouse' | 'pen' | 'penEraser';
    events: unknown[];
    agentRequest?: number;
  }): Promise<Envelope>;
  /** `gesture_abort` (P2; SI-135): a user press interrupted the agent gesture `agentRequest` the window was playing. */
  gestureAbort(agentRequest: number): Promise<void>;
  /** `text_input` (M4 C4.4b, design note 5 §4): one input of the Text tool's editor into the kernel's input path (typed
   *  text, an editing key, a paste or a cut), on the generation the window follows (D-7b). */
  textInput(input: {
    kind: 'text' | 'key' | 'paste' | 'cut';
    text?: string;
    keys?: string;
  }): Promise<Envelope>;
  /** `timing_gesture` (M5 C5.1b; reviews/M5-design-timeline-view.md section 3): a press, release or abort in the
   *  Timeline, through the kernel's timing-gesture interpreter on the generation the window follows (D-7b). The press
   *  answers `{grabId, target}`; the release `{plan, envelopes}` or `{cancelled: true, error}` (E-10, E-23, E-28); the
   *  abort `{aborted: true}`. A refusal (no project, a stale generation, an unknown grab) is `{ok: false, error}`. */
  timingGesture(request: TimingGestureRequest): Promise<TimingGestureAnswer>;
  /** `canvas_presence`: this window's canvas for `viewId` is at `rect` (view pixels), or (`null`) went away. */
  canvasPresence(
    viewId: string,
    rect: { x: number; y: number; w: number; h: number } | null,
    dpr: number,
  ): Promise<void>;
  /** Agent gestures the host routes to this window (`agent-input`; design note 2 §6). */
  onAgentInput(handler: (input: AgentInput) => void): Promise<Unlisten>;
  /** `agent_token` (D-62 item 2, D-65 item 2): a numbered delivery of the agent token, for Preferences › Agent access
   *  to show to the person or copy; never a registry command, so no agent reaches it. The host counts the delivery as
   *  possibly on screen until `tokenHidden(delivery)`. `null` when nothing was delivered (a window capture runs). */
  agentToken(): Promise<TokenDelivery | null>;
  /** `agent_acting` (D-86 item 2): the agents' requests running now, `seq` the number of the last `shell.agentActing`
   *  notice they reflect, for a window that mounts while an agent acts. */
  agentActing(): Promise<{ generation: number; seq: number; acting: unknown[] }>;
  /** `token_hidden` (D-65 item 2): this window acknowledges `delivery` hidden: the mask painted over it, or the token
   *  never painted (a Copy, or an answer that came after its reveal ended). */
  tokenHidden(delivery: number): Promise<void>;
  /** `token-hide`: a window capture is about to run; a shown token must be masked now. */
  onTokenHide(handler: () => void): Promise<Unlisten>;
  /** `page_ready` (D-69 item 1, D-72 item 1): this window's document has painted after its bootstrap. Whether the host
   *  accepted it: only for a navigation it saw start and finish, once; it then retires the token deliveries of the
   *  document this one replaced and lets captures of the window through again. */
  pageReady(): Promise<boolean>;
  /** `token_field` (D-69 item 1): the main window shows the token field (`true`, from its mount) or no longer does. */
  tokenField(present: boolean): Promise<void>;
  /** `token_rect` (D-69 item 1): the answer to a `token-rect` request: the field's rectangle in CSS pixels, or `null`. */
  tokenRect(request: number, rect: TokenRect | null): Promise<void>;
  /** `token-rect`: the host asks where the token field is, just before and just after a capture of the main window. */
  onTokenRect(handler: (request: number) => void): Promise<Unlisten>;
}

/** One pointer sample of a Timeline gesture, in the surface's own pixels, with the modifiers held (kernel names). */
export interface TimingSample {
  readonly x: number;
  readonly y: number;
  readonly modifiers: readonly string[];
}

/** A Timeline gesture's layout, captured at the press (the kernel's `GestureLayout`). */
export interface TimingLayout {
  readonly surface: 'layers' | 'tracks' | 'ruler';
  readonly zoomLevel: number;
  readonly firstFrame: number;
  readonly scrollTop: number;
  readonly platform: 'windows' | 'macos' | 'linux';
}

export type TimingGestureRequest =
  | { readonly phase: 'press'; readonly layout: TimingLayout; readonly sample: TimingSample }
  | { readonly phase: 'release'; readonly grabId: number; readonly samples: readonly TimingSample[] }
  | { readonly phase: 'abort'; readonly grabId: number };

export type TimingGestureAnswer =
  | { readonly ok: true; readonly value: Readonly<Record<string, unknown>> }
  | { readonly ok: false; readonly error: CommandFailure };

/** The token field's rectangle in the window's CSS pixels (D-69 item 1). */
export interface TokenRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** An agent's `pointer` request routed to this window (attached mode). */
export interface AgentInput {
  requestId: number;
  viewId: string;
  /** The engine generation it was routed on: played only while the window follows it (D-7b). */
  generation: number;
  request: Record<string, unknown>;
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

/**
 * The JSON text of a `gesture_commit` request (C42B-R1-04): the native writer when every value is what the wire writer
 * would write the same way (finite numbers below 2^53, ASCII strings, no `bigint`): the collector's events always
 * are, and a 10,000-sample gesture then costs about 1 ms instead of 18; anything else goes through the wire writer.
 */
export function gestureJson(request: Record<string, unknown>): string {
  const plain = (v: unknown, depth: number): boolean => {
    if (depth > 8) return false;
    if (v === null || typeof v === 'boolean') return true;
    if (typeof v === 'number') return Number.isFinite(v) && Math.abs(v) < 2 ** 53;
    if (typeof v === 'string') return /^[\x20-\x7e]*$/.test(v);
    if (Array.isArray(v)) return v.every((x) => plain(x, depth + 1));
    if (typeof v === 'object' && Object.getPrototypeOf(v) === Object.prototype) {
      return Object.values(v).every((x) => x === undefined || plain(x, depth + 1));
    }
    return false;
  };
  return plain(request, 1) ? JSON.stringify(request) : stringifyWireJson(request);
}

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
    async canvasScene(viewId) {
      return decode(await invoke('canvas_scene', { viewId }));
    },
    async agentActing() {
      const v = decode(await invoke('agent_acting', {}));
      const o = typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : {};
      return {
        generation: typeof o['generation'] === 'number' ? o['generation'] : 0,
        seq: typeof o['seq'] === 'number' ? o['seq'] : 0,
        acting: Array.isArray(o['acting']) ? (o['acting'] as unknown[]) : [],
      };
    },
    async agentToken() {
      const v = decode(await invoke('agent_token', {}));
      const o = typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : {};
      const token = o['token'];
      const delivery = o['delivery'];
      return typeof token === 'string' && typeof delivery === 'number' ? { token, delivery } : null;
    },
    async tokenHidden(delivery) {
      decode(await invoke('token_hidden', { delivery }));
    },
    onTokenHide(handler) {
      return transport.listen('token-hide', () => {
        handler();
      });
    },
    async pageReady() {
      const v = decode(await invoke('page_ready', {}));
      return typeof v === 'object' && v !== null && (v as Record<string, unknown>)['accepted'] === true;
    },
    async tokenField(present) {
      decode(await invoke('token_field', { present }));
    },
    async tokenRect(request, rect) {
      decode(await invoke('token_rect', { request, rect }));
    },
    onTokenRect(handler) {
      return transport.listen('token-rect', (payload) => {
        const request =
          typeof payload === 'object' && payload !== null
            ? (payload as Record<string, unknown>)['request']
            : null;
        if (typeof request === 'number') handler(request);
      });
    },
    async gestureCommit(request) {
      try {
        const v = await transport.invoke('gesture_commit', { json: gestureJson({ ...request, generation }) });
        return (
          asEnvelope(v) ??
          localFailure('ipc.invalidResponse', 'the answer to a gesture is not a command envelope')
        );
      } catch (error) {
        return failureOf(error);
      }
    },
    async gestureAbort(agentRequest) {
      await invoke('gesture_abort', { agentRequest });
    },
    async textInput(input) {
      try {
        const v = await transport.invoke('text_input', { json: JSON.stringify({ ...input, generation }) });
        return (
          asEnvelope(v) ??
          localFailure('ipc.invalidResponse', 'the answer to a text input is not a command envelope')
        );
      } catch (error) {
        return failureOf(error);
      }
    },
    async timingGesture(request) {
      try {
        const v = decode(
          await transport.invoke('timing_gesture', { json: gestureJson({ ...request, generation }) }),
        );
        if (typeof v === 'object' && v !== null && !Array.isArray(v)) {
          return { ok: true, value: v as Record<string, unknown> };
        }
        return {
          ok: false,
          error: {
            code: 'ipc.invalidResponse',
            message: 'the answer to a Timeline gesture is not an object',
          },
        };
      } catch (error) {
        const failure = failureOf(error).error;
        return { ok: false, error: failure ?? { code: 'ipc.failed', message: String(error) } };
      }
    },
    async canvasPresence(viewId, rect, dpr) {
      await invoke('canvas_presence', { viewId, rect, dpr });
    },
    onAgentInput(handler) {
      return transport.listen('agent-input', (payload) => {
        try {
          const v = decode(payload);
          if (typeof v !== 'object' || v === null) return;
          const o = v as Record<string, unknown>;
          const req = o['request'];
          if (typeof o['requestId'] !== 'number' || typeof o['viewId'] !== 'string') return;
          if (typeof o['generation'] !== 'number') return;
          if (typeof req !== 'object' || req === null) return;
          handler({
            requestId: o['requestId'],
            viewId: o['viewId'],
            generation: o['generation'],
            request: req as Record<string, unknown>,
          });
        } catch {
          // A malformed message can only come from a version mismatch; it is dropped (the agent's call times out).
        }
      });
    },
    async canvasLoad(drawingId, bucket, outline = false, fastPencil = false, part = 'all') {
      if (!transport.invokeBinary) throw new Error('ipc.unsupported: this transport has no binary commands');
      return transport.invokeBinary('canvas_load', {
        json: stringifyWireJson({ drawingId, bucket, outline, fastPencil, part }),
      });
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
