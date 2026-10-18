// Runtime validation of values that cross the host boundary. Wire shapes are written as a small subset of JSON
// Schema (draft 2020-12 keywords: type, properties, required, additionalProperties, enum, items, minimum, maximum,
// pattern, anyOf) so that M1 can replace these hand-written schemas with the ones generated from the registry
// (11 §7.2, 10 §2.8) without changing the validator or its callers.

export type Schema =
  | { type: 'string'; enum?: readonly string[]; pattern?: string }
  | { type: 'integer'; minimum?: number; maximum?: number }
  | { type: 'null' }
  | { type: 'array'; items: Schema }
  | {
      type: 'object';
      properties: Readonly<Record<string, Schema>>;
      required: readonly string[];
      additionalProperties: boolean | Schema;
    }
  | { anyOf: readonly Schema[] }
  | Record<string, never>; // `{}`: any JSON value

/** Largest integer a JS number holds exactly; Rust u64 values above it cannot cross this boundary losslessly. */
export const MAX_SAFE = Number.MAX_SAFE_INTEGER;
export const U64: Schema = { type: 'integer', minimum: 0, maximum: MAX_SAFE };
export const STRING: Schema = { type: 'string' };
export const ANY: Schema = {};

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Returns the JSON-pointer locations at which `value` violates `schema` (empty = valid). */
export function violations(schema: Schema, value: unknown, at = ''): string[] {
  if ('anyOf' in schema) {
    return schema.anyOf.some((s) => violations(s, value, at).length === 0)
      ? []
      : [`${at || '/'}: no anyOf branch`];
  }
  if (!('type' in schema)) return [];
  const here = at || '/';
  switch (schema.type) {
    case 'null':
      return value === null ? [] : [`${here}: expected null`];
    case 'string': {
      if (typeof value !== 'string') return [`${here}: expected string`];
      if (schema.enum && !schema.enum.includes(value))
        return [`${here}: not one of ${schema.enum.join('|')}`];
      if (schema.pattern !== undefined && !new RegExp(schema.pattern, 'u').test(value)) {
        return [`${here}: does not match ${schema.pattern}`];
      }
      return [];
    }
    case 'integer': {
      if (typeof value !== 'number' || !Number.isInteger(value)) return [`${here}: expected integer`];
      if (schema.minimum !== undefined && value < schema.minimum) return [`${here}: below ${schema.minimum}`];
      if (schema.maximum !== undefined && value > schema.maximum) return [`${here}: above ${schema.maximum}`];
      return [];
    }
    case 'array':
      if (!Array.isArray(value)) return [`${here}: expected array`];
      return value.flatMap((item, i) => violations(schema.items, item, `${at}/${i}`));
    case 'object': {
      if (!isPlainObject(value)) return [`${here}: expected object`];
      const out = schema.required.filter((k) => !(k in value)).map((k) => `${at}/${k}: required`);
      for (const [k, v] of Object.entries(value)) {
        const prop = schema.properties[k];
        if (prop) out.push(...violations(prop, v, `${at}/${k}`));
        else if (schema.additionalProperties === false) out.push(`${at}/${k}: not allowed`);
        else if (schema.additionalProperties !== true)
          out.push(...violations(schema.additionalProperties, v, `${at}/${k}`));
      }
      return out;
    }
  }
}

/** A type guard for `T` backed by `schema`. The schema must describe `T`; the wire tests check both agree. */
// T is the type the schema certifies; it appears only in the returned predicate by design.
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters
export function guard<T>(schema: Schema): (value: unknown) => value is T {
  return (value: unknown): value is T => violations(schema, value).length === 0;
}

/** `namespace.object[.verb]`, matching `is_command_id` in desktop/src-tauri/src/bindings.rs. */
export const COMMAND_ID_PATTERN = '^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$';
