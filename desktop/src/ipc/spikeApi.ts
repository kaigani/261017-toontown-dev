// M0 spike API (C0.2 tablet input, C0.3 canvas throughput). Same boundary as the product API: every value from the host, including each
// streamed channel message, is certified against its schema before UI code sees it. Not product API.
import { certifyPixelFrame, certifyRenderList } from './binaryEnvelope';
import { HostError, toHostError } from './hostApi';
import { certify } from './schema';
import {
  isNativeBatch,
  isNativeStartInfo,
  isPenSimReport,
  isProbeResult,
  isSavedLog,
  isSpikeEnvironment,
  isSpikeModeOrNull,
} from './spikeSchemas';
import type { HostTransport } from './transport';
import type {
  CanvasSceneRequest,
  NativeBatch,
  NativeStartInfo,
  PenSimReport,
  PenSimRequest,
  PixelFramePayload,
  ProbeResult,
  RenderListPayload,
  SavedLog,
  SpikeEnvironment,
  SpikeMode,
  BenchPlan,
} from './types';
import { isBenchPlanOrNull, isNull } from './wireSchemas';

/** M0 diagnostics: the IPC benchmark (`--ipc-bench`). Only the diagnostics build calls the host (C0.4). */
export interface BenchApi {
  /** The benchmark requested on the command line, or null. */
  benchPlan(): Promise<BenchPlan | null>;
  /** Hands the measurements to the host, which writes them and exits. */
  benchReport(report: unknown): Promise<void>;
}

export interface SpikeApi {
  /** The spike launch (`--spike tablet`), or null in a normal launch. */
  mode(): Promise<SpikeMode | null>;
  /** Host facts for the log: OS build, webview runtime version, CPU count, window geometry. */
  environment(): Promise<SpikeEnvironment>;
  /** Starts the native listener; `onBatch` receives validated batches, `onInvalid` counts rejected messages. */
  nativeStart(onBatch: (batch: NativeBatch) => void, onInvalid?: () => void): Promise<NativeStartInfo>;
  nativeStop(): Promise<void>;
  /** Windows only: injects a synthetic pen stroke into the app's own window. */
  simulatePen(request: PenSimRequest): Promise<PenSimReport>;
  /**
   * Windows only: the same stroke into a plain same-process native window with WM_POINTER and raw-input listeners,
   * plus a RealTimeStylus unless `rts` is false (C02-R1-04).
   */
  probeNativeWindow(request: PenSimRequest, rts?: boolean): Promise<ProbeResult>;
  /** Windows only: starts (`true`) or stops (`false`, returns its counts) the raw-input HID listener. */
  rawInputSink(start: boolean): Promise<ProbeResult>;
  /** Writes the log to the `--spike-log` path. */
  saveLog(log: unknown): Promise<SavedLog>;
  /** C0.3: one binary render list (`tauri::ipc::Response`), its envelope certified. */
  canvasScene(request: CanvasSceneRequest): Promise<RenderListPayload>;
  /** C0.3: a synthetic RGBA8 frame of `width × height` (host-raster fallback estimate), its envelope certified. */
  canvasPixels(width: number, height: number, frame: number): Promise<PixelFramePayload>;
}

async function binary(
  transport: HostTransport,
  command: string,
  args: Record<string, unknown>,
): Promise<ArrayBuffer> {
  if (!transport.invokeBinary) {
    throw new HostError({ code: 'ipc.unsupported', message: 'transport has no binary responses' });
  }
  try {
    return await transport.invokeBinary(command, args);
  } catch (error: unknown) {
    throw toHostError(error);
  }
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

export function createBenchApi(transport: HostTransport): BenchApi {
  return {
    benchPlan: () => checked(transport.invoke('bench_plan'), 'bench_plan', isBenchPlanOrNull),
    benchReport: async (report) => {
      await checked(transport.invoke('bench_report', { report }), 'bench_report', isNull);
    },
  };
}

export function createSpikeApi(transport: HostTransport): SpikeApi {
  return {
    mode: () => checked(transport.invoke('spike_mode'), 'spike_mode', isSpikeModeOrNull),
    environment: () =>
      checked(transport.invoke('spike_environment'), 'spike_environment', isSpikeEnvironment),
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
    probeNativeWindow: (request, rts = true) =>
      checked(
        transport.invoke('spike_probe_native_window', { request, rts }),
        'spike_probe_native_window',
        isProbeResult,
      ),
    rawInputSink: (start) =>
      checked(transport.invoke('spike_raw_input_sink', { start }), 'spike_raw_input_sink', isProbeResult),
    saveLog: (log) => checked(transport.invoke('spike_save_log', { log }), 'spike_save_log', isSavedLog),
    canvasScene: async (request) => {
      const bytes = await binary(transport, 'spike_canvas_scene', { request });
      return { header: certifyRenderList(bytes), bytes };
    },
    canvasPixels: async (width, height, frame) => {
      const bytes = await binary(transport, 'spike_canvas_pixels', { width, height, frame });
      return { header: certifyPixelFrame(bytes), bytes };
    },
  };
}
