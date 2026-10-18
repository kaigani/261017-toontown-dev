// Schemas of the C0.2 tablet-spike wire values (M0 spike; not product API).
import { guard, NUMBER, STRING, U64, type Schema } from './schema';
import type { NativeBatch, NativeStartInfo, PenSimReport, SavedLog, SpikeMode } from './types';

const nullable = (s: Schema): Schema => ({ anyOf: [s, { type: 'null' }] });
const obj = (properties: Record<string, Schema>): Schema => ({
  type: 'object',
  properties,
  required: Object.keys(properties),
  additionalProperties: false,
});

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
});

export const nativeBatchSchema = obj({ packets: { type: 'array', items: nativePacketSchema } });

export const nativeStartInfoSchema = obj({
  backend: { type: 'string', enum: ['octotablet-ink', 'appkit-nsevent'] },
  tablets: { type: 'array', items: STRING },
  timestampGranularityMs: nullable(NUMBER),
});

export const penSimReportSchema = obj({
  method: STRING,
  frames: U64,
  hz: U64,
  achievedHz: NUMBER,
  injectEpochMs: { type: 'array', items: NUMBER },
  pressure: { type: 'array', items: U64 },
  tiltX: { type: 'array', items: { type: 'integer', minimum: -90, maximum: 90 } },
  tiltY: { type: 'array', items: { type: 'integer', minimum: -90, maximum: 90 } },
  rotation: { type: 'array', items: U64 },
});

export const savedLogSchema = obj({ path: STRING, bytes: U64 });

export const isSpikeModeOrNull = guard<SpikeMode | null>({ anyOf: [spikeModeSchema, { type: 'null' }] });
export const isNativeBatch = guard<NativeBatch>(nativeBatchSchema);
export const isNativeStartInfo = guard<NativeStartInfo>(nativeStartInfoSchema);
export const isPenSimReport = guard<PenSimReport>(penSimReportSchema);
export const isSavedLog = guard<SavedLog>(savedLogSchema);
