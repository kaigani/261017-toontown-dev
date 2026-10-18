// Schemas of the C0.2 tablet-spike wire values (M0 spike; not product API).
import { ANY, guard, NUMBER, STRING, U64, type Schema } from './schema';
import type {
  NativeBatch,
  NativeStartInfo,
  PenSimReport,
  ProbeResult,
  SavedLog,
  SpikeEnvironment,
  SpikeMode,
} from './types';

const nullable = (s: Schema): Schema => ({ anyOf: [s, { type: 'null' }] });
const obj = (properties: Record<string, Schema>, optional: Record<string, Schema> = {}): Schema => ({
  type: 'object',
  properties: { ...properties, ...optional },
  required: Object.keys(properties),
  additionalProperties: false,
});
const I32: Schema = { type: 'integer', minimum: -2147483648, maximum: 2147483647 };
const SAFE_INT: Schema = {
  type: 'integer',
  minimum: -Number.MAX_SAFE_INTEGER,
  maximum: Number.MAX_SAFE_INTEGER,
};

export const spikeModeSchema = obj({
  name: { type: 'string', enum: ['tablet'] },
  logPath: nullable(STRING),
  nativeBackend: nullable({ type: 'string', enum: ['octotablet-ink', 'appkit-nsevent'] }),
  penSimulation: { type: 'boolean' },
  auto: { type: 'boolean' },
});

export const nativePacketSchema = obj({
  kind: { type: 'string', enum: ['pose', 'down', 'up', 'in', 'out', 'frame', 'proximity'] },
  hostEpochMs: NUMBER,
  deviceMs: nullable(NUMBER),
  x: NUMBER,
  y: NUMBER,
  pressure: nullable(NUMBER),
  tiltX: nullable(NUMBER),
  tiltY: nullable(NUMBER),
  twist: nullable(NUMBER),
  tool: STRING,
  seq: U64,
  rawTiltX: nullable(NUMBER),
  rawTiltY: nullable(NUMBER),
  deviceId: nullable(STRING),
  windowNumber: nullable(SAFE_INT),
});

export const nativeBatchSchema = obj({
  packets: { type: 'array', items: nativePacketSchema },
  hostDropped: U64,
  sendFailures: U64,
});

const windowGeometrySchema = obj({
  scaleFactor: NUMBER,
  innerX: I32,
  innerY: I32,
  innerWidth: U64,
  innerHeight: U64,
});

export const nativeStartInfoSchema = obj({
  backend: { type: 'string', enum: ['octotablet-ink', 'appkit-nsevent'] },
  tablets: { type: 'array', items: STRING },
  timestampGranularityMs: nullable(NUMBER),
  deviceClock: STRING,
  window: nullable(windowGeometrySchema),
});

export const spikeEnvironmentSchema = obj({
  os: STRING,
  arch: STRING,
  osVersion: nullable(STRING),
  webviewVersion: nullable(STRING),
  cpuCount: nullable(U64),
  window: nullable(windowGeometrySchema),
});

// The fields added in review round 1 are optional, so a report without them (an older host) is still accepted.
export const penSimReportSchema = obj(
  {
    method: STRING,
    frames: U64,
    hz: U64,
    achievedHz: NUMBER,
    injectEpochMs: { type: 'array', items: NUMBER },
    pressure: { type: 'array', items: U64 },
    tiltX: { type: 'array', items: { type: 'integer', minimum: -90, maximum: 90 } },
    tiltY: { type: 'array', items: { type: 'integer', minimum: -90, maximum: 90 } },
    rotation: { type: 'array', items: U64 },
  },
  {
    injectReturnEpochMs: { type: 'array', items: NUMBER },
    screenX: { type: 'array', items: I32 },
    screenY: { type: 'array', items: I32 },
    screenRect: { type: 'array', items: I32 },
    pattern: { type: 'string', enum: ['sine', 'zigzag', 'stationary'] },
    target: { type: 'string', enum: ['webview', 'nativeWindow'] },
  },
);

/** Probe results are logged verbatim (any plain JSON object); the analyzer interprets them. */
export const probeResultSchema: Schema = {
  type: 'object',
  properties: {},
  required: [],
  additionalProperties: ANY,
};

export const savedLogSchema = obj({ path: STRING, bytes: U64 });

export const isSpikeModeOrNull = guard<SpikeMode | null>({ anyOf: [spikeModeSchema, { type: 'null' }] });
export const isNativeBatch = guard<NativeBatch>(nativeBatchSchema);
export const isNativeStartInfo = guard<NativeStartInfo>(nativeStartInfoSchema);
export const isSpikeEnvironment = guard<SpikeEnvironment>(spikeEnvironmentSchema);
export const isPenSimReport = guard<PenSimReport>(penSimReportSchema);
export const isProbeResult = guard<ProbeResult>(probeResultSchema);
export const isSavedLog = guard<SavedLog>(savedLogSchema);
