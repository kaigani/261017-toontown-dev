// In-memory host used by tests and by `pnpm dev` outside Tauri. Mirrors the Rust host's behaviour for the
// commands that exist; unknown commands reject with `command.unknown` like the real registry will (11 §10).
import type { HostTransport, Unlisten } from './transport';
import type { EngineInfo, ErrorInfo, PingRequest, PingResponse } from './types';

export const MOCK_MAX_PING_PAYLOAD_BYTES = 1 << 20;

export interface MockTransport extends HostTransport {
  /** Delivers an event to subscribers, as the host would. */
  emit(event: string, payload: unknown): void;
  /** Number of active subscriptions for an event. */
  listenerCount(event: string): number;
}

export const MOCK_ENGINE_INFO: EngineInfo = {
  name: 'Toontown',
  version: '0.0.1',
  platform: 'browser',
  arch: 'mock',
  formatVersion: '0.0',
  registryVersion: '0.0.0',
  runMode: 'attached',
};

type Outcome = { ok: true; value: unknown } | { ok: false; error: ErrorInfo };

export function createMockTransport(): MockTransport {
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const started = performance.now();

  const handlers: Record<string, (args: Record<string, unknown>) => Outcome> = {
    ping: (args) => {
      const request = args['request'] as PingRequest;
      const length = request.payload?.length ?? 0;
      if (length > MOCK_MAX_PING_PAYLOAD_BYTES) {
        return {
          ok: false,
          error: {
            code: 'limit.exceeded',
            message: `ping payload is ${length}, above the limit of ${MOCK_MAX_PING_PAYLOAD_BYTES}`,
            details: { limit: MOCK_MAX_PING_PAYLOAD_BYTES, value: length },
          },
        };
      }
      const value: PingResponse = {
        seq: request.seq,
        payload: request.payload ?? null,
        engineUptimeNs: Math.round((performance.now() - started) * 1e6),
        handlerNs: 0,
      };
      return { ok: true, value };
    },
    app_info: () => ({ ok: true, value: MOCK_ENGINE_INFO }),
    bench_plan: () => ({ ok: true, value: null }),
    bench_report: () => ({
      ok: false,
      error: { code: 'diagnostics.notRequested', message: 'the app was not started with --ipc-bench' },
    }),
  };

  return {
    kind: 'mock',
    invoke(command, args = {}) {
      const handler = handlers[command];
      const outcome: Outcome = handler
        ? handler(args)
        : { ok: false, error: { code: 'command.unknown', message: `no such command: ${command}` } };
      // Tauri rejects with the plain serialized error value, not an Error instance; the mock does the same.
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
      return outcome.ok ? Promise.resolve(outcome.value) : Promise.reject(outcome.error);
    },
    listen(event, handler): Promise<Unlisten> {
      const set = listeners.get(event) ?? new Set();
      set.add(handler);
      listeners.set(event, set);
      return Promise.resolve(() => {
        set.delete(handler);
      });
    },
    emit(event, payload) {
      for (const handler of listeners.get(event) ?? []) handler(payload);
    },
    listenerCount(event) {
      return listeners.get(event)?.size ?? 0;
    },
  };
}
