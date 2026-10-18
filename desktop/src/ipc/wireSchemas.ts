// Schemas of every product value the host sends to the front end. Generated from the Rust types
// (src/ipc/generated/wire.ts, crates/engine/src/wire.rs; ADR-0012); only the M0 diagnostics plan below is
// hand-written, because diagnostics are not product API.
import { WIRE_DEFS } from './generated/wire';
import { guard, U64, type Defs, type Schema } from './schema';
import type { BenchPlan, EngineInfo, ErrorInfo, MenuCommandEvent, PingResponse } from './types';

export { COMMAND_ID_PATTERN, FORMAT_VERSION, REGISTRY_VERSION } from './generated/wire';

/** The definitions table the wire schemas' `$ref`s resolve against. */
export const WIRE: Defs = WIRE_DEFS;

function def(name: string): Schema {
  return { $ref: `#/$defs/${name}` };
}

function requiredOf(name: string): readonly string[] {
  const s = Object.getOwnPropertyDescriptor(WIRE_DEFS, name)?.value as Schema | undefined;
  if (typeof s !== 'object') return [];
  return s.required ?? [];
}

export const RUN_MODES = ['attached', 'headless', 'batchScript'] as const;

/** Every field of `EngineInfo` (all required by the generated schema). */
export const ENGINE_INFO_FIELDS: readonly string[] = requiredOf('EngineInfo');

export const errorInfoSchema: Schema = def('ErrorInfo');
export const pingResponseSchema: Schema = def('PingResponse');
export const engineInfoSchema: Schema = def('EngineInfo');
export const menuCommandEventSchema: Schema = def('MenuCommandEvent');

/** M0 diagnostics only (C0.4): the benchmark plan `toontown --ipc-bench` hands to the page. */
export const benchPlanSchema: Schema = {
  type: 'object',
  properties: {
    count: { type: 'integer', minimum: 1, maximum: 0xffff_ffff },
    payloadBytes: { type: 'array', items: U64 },
  },
  required: ['count', 'payloadBytes'],
  additionalProperties: false,
};

export const isErrorInfo = guard<ErrorInfo>(errorInfoSchema, WIRE);
export const isPingResponse = guard<PingResponse>(pingResponseSchema, WIRE);
export const isEngineInfo = guard<EngineInfo>(engineInfoSchema, WIRE);
export const isMenuCommandEvent = guard<MenuCommandEvent>(menuCommandEventSchema, WIRE);
export const isBenchPlanOrNull = guard<BenchPlan | null>({ anyOf: [benchPlanSchema, { type: 'null' }] });
export const isNull = guard<null>({ type: 'null' });
