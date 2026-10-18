// Schemas of every value the host sends to the front end (see types.ts for the matching TypeScript types).
// Hand-written in M0; generated from the registry's JSON Schemas from M1.
import { ANY, COMMAND_ID_PATTERN, guard, STRING, U64, type Schema } from './schema';
import type { BenchPlan, EngineInfo, ErrorInfo, MenuCommandEvent, PingResponse } from './types';

export const RUN_MODES = ['attached', 'headless', 'batchScript'] as const;

export const ENGINE_INFO_FIELDS = [
  'name',
  'version',
  'platform',
  'arch',
  'formatVersion',
  'registryVersion',
  'runMode',
] as const;

export const errorInfoSchema: Schema = {
  type: 'object',
  properties: { code: STRING, message: STRING, details: ANY },
  required: ['code', 'message'],
  additionalProperties: false,
};

export const pingResponseSchema: Schema = {
  type: 'object',
  properties: {
    seq: U64,
    payload: { anyOf: [STRING, { type: 'null' }] },
    engineUptimeNs: U64,
    handlerNs: U64,
  },
  required: ['seq', 'payload', 'engineUptimeNs', 'handlerNs'],
  additionalProperties: false,
};

export const engineInfoSchema: Schema = {
  type: 'object',
  properties: {
    name: STRING,
    version: STRING,
    platform: STRING,
    arch: STRING,
    formatVersion: { type: 'string', pattern: '^\\d+\\.\\d+$' },
    registryVersion: { type: 'string', pattern: '^\\d+\\.\\d+\\.\\d+$' },
    runMode: { type: 'string', enum: RUN_MODES },
  },
  required: ENGINE_INFO_FIELDS,
  additionalProperties: false,
};

export const menuCommandEventSchema: Schema = {
  type: 'object',
  properties: {
    command: { type: 'string', pattern: COMMAND_ID_PATTERN },
    params: { type: 'object', properties: {}, required: [], additionalProperties: true },
    handler: { type: 'string', enum: ['frontend', 'pending'] },
    milestone: { anyOf: [STRING, { type: 'null' }] },
  },
  required: ['command', 'params', 'handler', 'milestone'],
  additionalProperties: false,
};

export const benchPlanSchema: Schema = {
  type: 'object',
  properties: {
    count: { type: 'integer', minimum: 1, maximum: 0xffff_ffff },
    payloadBytes: { type: 'array', items: U64 },
  },
  required: ['count', 'payloadBytes'],
  additionalProperties: false,
};

export const isErrorInfo = guard<ErrorInfo>(errorInfoSchema);
export const isPingResponse = guard<PingResponse>(pingResponseSchema);
export const isEngineInfo = guard<EngineInfo>(engineInfoSchema);
export const isMenuCommandEvent = guard<MenuCommandEvent>(menuCommandEventSchema);
export const isBenchPlanOrNull = guard<BenchPlan | null>({ anyOf: [benchPlanSchema, { type: 'null' }] });
export const isNull = guard<null>({ type: 'null' });
