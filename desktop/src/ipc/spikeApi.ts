// M0 spike API (C0.2 tablet input). Same boundary as the product API: every value from the host, including each
// streamed channel message, is certified against its schema before UI code sees it. Not product API.
import { HostError, toHostError } from './hostApi';
import { certify } from './schema';
import {
  isNativeBatch,
  isNativeStartInfo,
  isPenSimReport,
  isSavedLog,
  isSpikeModeOrNull,
} from './spikeSchemas';
import type { HostTransport } from './transport';
import type { NativeBatch, NativeStartInfo, PenSimReport, PenSimRequest, SavedLog, SpikeMode } from './types';
import { isNull } from './wireSchemas';

export interface SpikeApi {
  /** The spike launch (`--spike tablet`), or null in a normal launch. */
  mode(): Promise<SpikeMode | null>;
  /** Starts the native listener; `onBatch` receives validated batches, `onInvalid` counts rejected messages. */
  nativeStart(onBatch: (batch: NativeBatch) => void, onInvalid?: () => void): Promise<NativeStartInfo>;
  nativeStop(): Promise<void>;
  /** Windows only: injects a synthetic pen stroke into the app's own window. */
  simulatePen(request: PenSimRequest): Promise<PenSimReport>;
  /** Writes the log to the `--spike-log` path. */
  saveLog(log: unknown): Promise<SavedLog>;
}

async function checked<T>(
  promise: Promise<unknown>,
  command: string,
  check: (v: unknown) => v is T,
): Promise<T> {
  let value: unknown;
  try {
    value = await promise;
  } catch (error: unknown) {
    throw toHostError(error);
  }
  const data = certify(check, value);
  if (data === undefined) {
    throw new HostError({ code: 'ipc.invalidResponse', message: `unexpected response from ${command}` });
  }
  return data;
}

export function createSpikeApi(transport: HostTransport): SpikeApi {
  return {
    mode: () => checked(transport.invoke('spike_mode'), 'spike_mode', isSpikeModeOrNull),
    nativeStart: (onBatch, onInvalid) => {
      if (!transport.invokeWithChannel) {
        return Promise.reject(
          new HostError({ code: 'ipc.unsupported', message: 'transport has no channels' }),
        );
      }
      return checked(
        transport.invokeWithChannel('spike_tablet_native_start', {}, 'channel', (message) => {
          const batch = certify(isNativeBatch, message);
          if (batch) onBatch(batch);
          else onInvalid?.();
        }),
        'spike_tablet_native_start',
        isNativeStartInfo,
      );
    },
    nativeStop: async () => {
      await checked(transport.invoke('spike_tablet_native_stop'), 'spike_tablet_native_stop', isNull);
    },
    simulatePen: (request) =>
      checked(transport.invoke('spike_pen_simulate', { request }), 'spike_pen_simulate', isPenSimReport),
    saveLog: (log) => checked(transport.invoke('spike_save_log', { log }), 'spike_save_log', isSavedLog),
  };
}
