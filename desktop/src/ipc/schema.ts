// Runtime validation of values that cross the host boundary. Wire shapes are written as a small subset of JSON
// Schema (draft 2020-12 keywords: type, properties, required, additionalProperties, enum, items, minimum, maximum,
// pattern, anyOf) so that M1 can replace these hand-written schemas with the ones generated from the registry
// (11 §7.2, 10 §2.8) without changing the validator or its callers.
//
// Two layers, both own-property based (C01-R2-02):
//   1. `plainDataViolations`: the value must be plain JSON data — null, booleans, strings, finite numbers, arrays and
//      objects whose prototype is Array.prototype / Object.prototype (or null), holding only own, enumerable,
//      string-keyed data properties (no accessors, functions, symbols, holes or cycles). Inherited fields therefore
//      never count, and a key such as `__proto__` or `constructor` is just another own key.
//   2. `violations`: the schema. Property membership and required fields use `Object.hasOwn` and own-property
//      descriptors only; nothing is looked up with `in` or bare indexing on a schema map.
// `certify` checks the original, takes a structured-clone snapshot, validates the snapshot again and returns it, so
// callers receive exactly the data that was validated.

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

type AnyOfSchema = Extract<Schema, { anyOf: readonly Schema[] }>;
type TypedSchema = Extract<Schema, { type: string }>;

/** Largest integer a JS number holds exactly; Rust u64 values above it cannot cross this boundary losslessly. */
export const MAX_SAFE = Number.MAX_SAFE_INTEGER;
export const U64: Schema = { type: 'integer', minimum: 0, maximum: MAX_SAFE };
export const STRING: Schema = { type: 'string' };
export const ANY: Schema = {};

/** Nesting deeper than this is rejected rather than risking stack exhaustion. */
export const MAX_DEPTH = 64;

/** The own data value of `key` on `obj`, or undefined; never consults the prototype chain or calls a getter. */
function ownValue(obj: object, key: string): unknown {
  const d = Object.getOwnPropertyDescriptor(obj, key);
  return d && Object.hasOwn(d, 'value') ? d.value : undefined;
}

function isAnyOf(schema: Schema): schema is AnyOfSchema {
  return Object.hasOwn(schema, 'anyOf');
}

function isTyped(schema: Schema): schema is TypedSchema {
  return Object.hasOwn(schema, 'type');
}

/** Returns the locations at which `value` is not plain JSON data (empty = plain). */
export function plainDataViolations(value: unknown, at = '', seen = new Set<object>()): string[] {
  const here = at || '/';
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return [];
  if (typeof value === 'number') return Number.isFinite(value) ? [] : [`${here}: non-finite number`];
  if (typeof value !== 'object') return [`${here}: ${typeof value} is not JSON data`];
  if (seen.has(value)) return [`${here}: cycle`];
  if (seen.size >= MAX_DEPTH) return [`${here}: nested deeper than ${MAX_DEPTH}`];
  if (Object.getOwnPropertySymbols(value).length > 0) return [`${here}: symbol-keyed property`];
  const isArray = Array.isArray(value);
  const proto: unknown = Object.getPrototypeOf(value);
  if (isArray ? proto !== Array.prototype : proto !== Object.prototype && proto !== null) {
    return [`${here}: not a plain ${isArray ? 'array' : 'object'}`];
  }
  const names = Object.getOwnPropertyNames(value).filter((k) => !(isArray && k === 'length'));
  if (isArray && names.length !== value.length) return [`${here}: sparse array or extra array property`];
  seen.add(value);
  const out: string[] = [];
  for (const k of names) {
    const d = Object.getOwnPropertyDescriptor(value, k);
    if (!d || !Object.hasOwn(d, 'value') || !d.enumerable) {
      out.push(`${at}/${k}: accessor or non-enumerable property`);
    } else {
      out.push(...plainDataViolations(d.value, `${at}/${k}`, seen));
    }
  }
  seen.delete(value);
  return out;
}

/** Returns the JSON-pointer locations at which `value` violates `schema` (empty = valid). */
export function violations(schema: Schema, value: unknown, at = ''): string[] {
  if (isAnyOf(schema)) {
    return schema.anyOf.some((s) => violations(s, value, at).length === 0)
      ? []
      : [`${at || '/'}: no anyOf branch`];
  }
  if (!isTyped(schema)) return [];
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
    case 'array': {
      if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
        return [`${here}: expected array`];
      }
      const out: string[] = [];
      for (let i = 0; i < value.length; i++) {
        if (!Object.hasOwn(value, i)) out.push(`${at}/${i}: missing array element`);
        else out.push(...violations(schema.items, ownValue(value, String(i)), `${at}/${i}`));
      }
      return out;
    }
    case 'object': {
      if (typeof value !== 'object' || value === null || Array.isArray(value))
        return [`${here}: expected object`];
      const proto: unknown = Object.getPrototypeOf(value);
      if (proto !== Object.prototype && proto !== null) return [`${here}: expected plain object`];
      const out = schema.required.filter((k) => !Object.hasOwn(value, k)).map((k) => `${at}/${k}: required`);
      for (const k of Object.keys(value)) {
        const v = ownValue(value, k);
        const prop = Object.hasOwn(schema.properties, k)
          ? (ownValue(schema.properties, k) as Schema | undefined)
          : undefined;
        if (prop !== undefined) out.push(...violations(prop, v, `${at}/${k}`));
        else if (schema.additionalProperties === false) out.push(`${at}/${k}: not allowed`);
        else if (schema.additionalProperties !== true)
          out.push(...violations(schema.additionalProperties, v, `${at}/${k}`));
      }
      return out;
    }
  }
}

/** Plain data that satisfies `schema` (both layers). */
export function conforms(schema: Schema, value: unknown): boolean {
  return plainDataViolations(value).length === 0 && violations(schema, value).length === 0;
}

/** A type guard for `T` backed by `schema`. The schema must describe `T`; the wire tests check both agree. */
// T is the type the schema certifies; it appears only in the returned predicate by design.
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters
export function guard<T>(schema: Schema): (value: unknown) => value is T {
  return (value: unknown): value is T => conforms(schema, value);
}

/**
 * Validates a value received from a transport and returns a snapshot of it, or undefined. The original must be
 * plain data; the snapshot (a structured clone) is validated again and is what the caller gets, so later changes to
 * the transport's object cannot affect validated data.
 */
export function certify<T>(check: (value: unknown) => value is T, value: unknown): T | undefined {
  if (plainDataViolations(value).length > 0) return undefined;
  const snapshot: unknown = structuredClone(value);
  return check(snapshot) ? snapshot : undefined;
}

/** `namespace.object[.verb]`, matching `is_command_id` in desktop/src-tauri/src/bindings.rs. */
export const COMMAND_ID_PATTERN = '^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$';
