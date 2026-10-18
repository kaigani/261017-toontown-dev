// Runtime validation of values that cross the host boundary. The schemas are JSON Schema draft 2020-12, generated
// from the Rust types (src/ipc/generated/, ADR-0012); the hand-written subset of M0 is a subset of what this accepts.
// Supported keywords: type (one or several), enum, const, pattern, minLength, maxLength, minimum, maximum,
// exclusiveMinimum, exclusiveMaximum, multipleOf, items (schema or false), prefixItems, minItems, maxItems,
// properties, required, additionalProperties, patternProperties, propertyNames, anyOf, oneOf, allOf, not,
// if/then/else and $ref into a definitions table. Annotations (description, readOnly, default, x-…) are ignored.
//
// Two layers, both own-property based (C01-R2-02):
//   1. `plainDataViolations`: the value must be plain JSON data — null, booleans, strings, finite numbers, arrays and
//      objects whose prototype is Array.prototype / Object.prototype (or null), holding only own, enumerable,
//      string-keyed data properties (no accessors, functions, symbols, holes or cycles). Inherited fields therefore
//      never count, and a key such as `__proto__` or `constructor` is just another own key.
//   2. `violations`: the schema. Property membership and required fields use own-property checks and descriptors
//      only; nothing is looked up with `in` or bare indexing on a schema map.
// `certify` checks the original, rejects anything the structured-clone algorithm refuses (a transparent Proxy,
// C01-R3-04), takes its own deep copy, validates the copy again and returns it, so callers receive exactly the data
// that was validated.
//
// Integers: a JS number is accepted for an integer schema only when it is a safe integer; integers beyond
// ±(2^53 − 1) arrive as `bigint` from the lossless wire parser (wireJson.ts) and are accepted only beyond that
// range, so each value has one representation. `plainDataViolations` keeps rejecting `bigint`: only the parser,
// never an object handed over by a transport, can produce one.
//
// Platform floor (ADR-0005: Safari 15.0 / macOS 12.0): no `Object.hasOwn` and no `structuredClone` (both Safari 15.4);
// the clone probe uses `MessageChannel` (Safari 5.1); `compat/compat` in the lint gate enforces this.

type JsonType = 'string' | 'integer' | 'number' | 'null' | 'boolean' | 'array' | 'object';
type Bound = number | bigint;
type Literal = string | number | boolean | null;

export interface SchemaObject {
  readonly type?: JsonType | readonly JsonType[];
  readonly enum?: readonly Literal[];
  readonly const?: Literal;
  readonly pattern?: string;
  readonly minLength?: number;
  readonly maxLength?: number;
  readonly minimum?: Bound;
  readonly maximum?: Bound;
  readonly exclusiveMinimum?: Bound;
  readonly exclusiveMaximum?: Bound;
  readonly multipleOf?: number;
  readonly items?: Schema;
  readonly prefixItems?: readonly Schema[];
  readonly minItems?: number;
  readonly maxItems?: number;
  readonly properties?: Readonly<Record<string, Schema>>;
  readonly required?: readonly string[];
  readonly additionalProperties?: Schema;
  readonly patternProperties?: Readonly<Record<string, Schema>>;
  readonly propertyNames?: Schema;
  readonly anyOf?: readonly Schema[];
  readonly oneOf?: readonly Schema[];
  readonly allOf?: readonly Schema[];
  readonly not?: Schema;
  readonly if?: Schema;
  readonly then?: Schema;
  readonly else?: Schema;
  readonly $ref?: string;
  readonly $schema?: string;
  readonly $id?: string;
  readonly $defs?: Readonly<Record<string, Schema>>;
  readonly title?: string;
  readonly description?: string;
  readonly readOnly?: boolean;
  readonly default?: unknown;
  readonly format?: string;
  readonly [annotation: `x-${string}`]: unknown;
}

/** A JSON Schema: an object schema or a boolean schema (`true` accepts everything, `false` nothing). */
export type Schema = SchemaObject | boolean;

/** Definitions table that `$ref: "#/$defs/<Name>"` resolves against. */
export type Defs = Readonly<Record<string, Schema>>;

/** Largest integer a JS number holds exactly; Rust u64 values above it arrive as bigint (wireJson.ts). */
export const MAX_SAFE = Number.MAX_SAFE_INTEGER;
export const U64: Schema = { type: 'integer', minimum: 0, maximum: MAX_SAFE };
export const STRING: Schema = { type: 'string' };
export const NUMBER: Schema = { type: 'number' };
export const ANY: Schema = {};

/**
 * Nesting deeper than this is rejected rather than risking stack exhaustion. Equal to the engine's
 * `tt_model::json_text::MAX_TEXT_DEPTH` (the outermost value is at depth 1; scalars count), so both sides accept
 * exactly the same texts (C11-R1-17); model objects are limited to 100 levels inside it (C11-R1-09).
 */
export const MAX_DEPTH = 128;

const NO_DEFS: Defs = {};
const REF_PREFIX = '#/$defs/';

/** `Object.hasOwn` for the Safari 15.0 floor. */
export function hasOwn(obj: object, key: PropertyKey): boolean {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

/** The own data value of `key` on `obj`, or undefined; never consults the prototype chain or calls a getter. */
function ownValue(obj: object, key: string): unknown {
  const d = Object.getOwnPropertyDescriptor(obj, key);
  return d && hasOwn(d, 'value') ? d.value : undefined;
}

/** An own keyword of a schema object (schemas are plain data too: never read through the prototype). */
function kw<K extends keyof SchemaObject>(schema: SchemaObject, key: K): SchemaObject[K] | undefined {
  return ownValue(schema, key) as SchemaObject[K] | undefined;
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
  // Arrays: the own names must be exactly "0".."length-1" (integer keys come first, ascending), so a hole cannot
  // be compensated by a named property (C01-R3-03).
  if (isArray && (names.length !== value.length || names.some((k, i) => k !== String(i)))) {
    return [`${here}: sparse array or extra array property`];
  }
  seen.add(value);
  const out: string[] = [];
  for (const k of names) {
    const d = Object.getOwnPropertyDescriptor(value, k);
    if (!d || !hasOwn(d, 'value') || !d.enumerable) {
      out.push(`${at}/${k}: accessor or non-enumerable property`);
    } else {
      out.push(...plainDataViolations(d.value, `${at}/${k}`, seen));
    }
  }
  seen.delete(value);
  return out;
}

function typeOf(value: unknown): JsonType | 'bigint' | 'other' {
  if (value === null) return 'null';
  switch (typeof value) {
    case 'string':
      return 'string';
    case 'boolean':
      return 'boolean';
    case 'number':
      return 'number';
    case 'bigint':
      return 'bigint';
    case 'object':
      return Array.isArray(value) ? 'array' : 'object';
    default:
      return 'other';
  }
}

/** Whether a value matches one JSON type (the integer rules of the header comment). */
function hasType(t: JsonType, value: unknown): boolean {
  const actual = typeOf(value);
  switch (t) {
    case 'integer':
      return (
        (actual === 'number' && Number.isSafeInteger(value)) ||
        (actual === 'bigint' &&
          ((value as bigint) > BigInt(MAX_SAFE) || (value as bigint) < -BigInt(MAX_SAFE)))
      );
    case 'number':
      return actual === 'number' && Number.isFinite(value);
    case 'object':
      if (actual !== 'object') return false;
      {
        const proto: unknown = Object.getPrototypeOf(value);
        return proto === Object.prototype || proto === null;
      }
    case 'array':
      return actual === 'array' && Object.getPrototypeOf(value) === Array.prototype;
    default:
      return actual === t;
  }
}

function typeMessage(types: readonly JsonType[]): string {
  if (types.length === 1) {
    const [t] = types;
    if (t === 'number') return 'expected finite number';
    if (t === 'object') return 'expected object';
    return `expected ${t ?? 'nothing'}`;
  }
  return `expected ${types.join(' or ')}`;
}

function sameLiteral(a: unknown, b: Literal): boolean {
  return a === b;
}

function codePoints(s: string): number {
  return Array.from(s).length;
}

function compareBound(value: number | bigint, bound: Bound): number {
  // Mixed number/bigint relational comparisons are exact in JavaScript.
  return value < bound ? -1 : value > bound ? 1 : 0;
}

function resolveRef(ref: string, defs: Defs): Schema | undefined {
  if (!ref.startsWith(REF_PREFIX)) return undefined;
  const name = ref.slice(REF_PREFIX.length);
  return hasOwn(defs, name) ? (ownValue(defs, name) as Schema | undefined) : undefined;
}

/** Returns the JSON-pointer locations at which `value` violates `schema` (empty = valid). */
export function violations(schema: Schema, value: unknown, at = '', defs: Defs = NO_DEFS): string[] {
  return check(schema, value, at, defs, 0);
}

function check(schema: Schema, value: unknown, at: string, defs: Defs, depth: number): string[] {
  const here = at || '/';
  if (schema === true) return [];
  if (schema === false) return [`${here}: not allowed`];
  if (depth > 4 * MAX_DEPTH) return [`${here}: schema nested too deeply`];
  const out: string[] = [];
  const ref = kw(schema, '$ref');
  if (ref !== undefined) {
    const target = resolveRef(ref, defs);
    if (target === undefined) return [`${here}: unresolved $ref ${ref}`];
    out.push(...check(target, value, at, defs, depth + 1));
    if (out.length > 0) return out;
  }
  const anyOf = kw(schema, 'anyOf');
  if (anyOf !== undefined && !anyOf.some((s) => check(s, value, at, defs, depth + 1).length === 0)) {
    return [`${here}: no anyOf branch`];
  }
  const oneOf = kw(schema, 'oneOf');
  if (
    oneOf !== undefined &&
    oneOf.filter((s) => check(s, value, at, defs, depth + 1).length === 0).length !== 1
  ) {
    return [`${here}: not exactly one oneOf branch`];
  }
  for (const s of kw(schema, 'allOf') ?? []) out.push(...check(s, value, at, defs, depth + 1));
  const not = kw(schema, 'not');
  if (not !== undefined && check(not, value, at, defs, depth + 1).length === 0)
    out.push(`${here}: matches not`);
  const cond = kw(schema, 'if');
  if (cond !== undefined) {
    const branch =
      check(cond, value, at, defs, depth + 1).length === 0 ? kw(schema, 'then') : kw(schema, 'else');
    if (branch !== undefined) out.push(...check(branch, value, at, defs, depth + 1));
  }
  const declared = kw(schema, 'type');
  if (declared !== undefined) {
    const types: readonly JsonType[] = typeof declared === 'string' ? [declared] : declared;
    if (!types.some((t) => hasType(t, value))) return [...out, `${here}: ${typeMessage(types)}`];
  }
  const en = kw(schema, 'enum');
  if (en !== undefined && !en.some((e) => sameLiteral(value, e))) {
    return [...out, `${here}: not one of ${en.map(String).join('|')}`];
  }
  if (hasOwn(schema, 'const') && !sameLiteral(value, kw(schema, 'const') ?? null)) {
    return [...out, `${here}: not ${String(kw(schema, 'const'))}`];
  }
  if (typeof value === 'string') out.push(...stringViolations(schema, value, here));
  if (typeof value === 'number' || typeof value === 'bigint')
    out.push(...numberViolations(schema, value, here));
  if (hasType('array', value)) out.push(...arrayViolations(schema, value as unknown[], at, defs, depth));
  else if (hasType('object', value)) out.push(...objectViolations(schema, value as object, at, defs, depth));
  return out;
}

const regexCache = new Map<string, RegExp>();

/** Compiled `u`-mode regular expression for a schema pattern (cached; schemas are a fixed, generated set). */
function regex(pattern: string): RegExp {
  let r = regexCache.get(pattern);
  if (r === undefined) {
    r = new RegExp(pattern, 'u');
    if (regexCache.size < 1024) regexCache.set(pattern, r);
  }
  return r;
}

function stringViolations(schema: SchemaObject, value: string, here: string): string[] {
  const pattern = kw(schema, 'pattern');
  if (pattern !== undefined && !regex(pattern).test(value)) return [`${here}: does not match ${pattern}`];
  const min = kw(schema, 'minLength');
  const max = kw(schema, 'maxLength');
  if (min !== undefined || max !== undefined) {
    const n = codePoints(value);
    if (min !== undefined && n < min) return [`${here}: shorter than ${min}`];
    if (max !== undefined && n > max) return [`${here}: longer than ${max}`];
  }
  return [];
}

function numberViolations(schema: SchemaObject, value: number | bigint, here: string): string[] {
  const min = kw(schema, 'minimum');
  const max = kw(schema, 'maximum');
  const xmin = kw(schema, 'exclusiveMinimum');
  const xmax = kw(schema, 'exclusiveMaximum');
  if (min !== undefined && compareBound(value, min) < 0) return [`${here}: below ${String(min)}`];
  if (max !== undefined && compareBound(value, max) > 0) return [`${here}: above ${String(max)}`];
  if (xmin !== undefined && compareBound(value, xmin) <= 0) return [`${here}: not above ${String(xmin)}`];
  if (xmax !== undefined && compareBound(value, xmax) >= 0) return [`${here}: not below ${String(xmax)}`];
  const step = kw(schema, 'multipleOf');
  if (
    step !== undefined &&
    typeof value === 'number' &&
    Math.abs(value / step - Math.round(value / step)) > 1e-9
  ) {
    return [`${here}: not a multiple of ${step}`];
  }
  return [];
}

function arrayViolations(
  schema: SchemaObject,
  value: unknown[],
  at: string,
  defs: Defs,
  depth: number,
): string[] {
  const here = at || '/';
  const out: string[] = [];
  const minItems = kw(schema, 'minItems');
  const maxItems = kw(schema, 'maxItems');
  if (minItems !== undefined && value.length < minItems) out.push(`${here}: fewer than ${minItems} items`);
  if (maxItems !== undefined && value.length > maxItems) out.push(`${here}: more than ${maxItems} items`);
  const prefix = kw(schema, 'prefixItems') ?? [];
  const items = kw(schema, 'items');
  for (let i = 0; i < value.length; i++) {
    if (!hasOwn(value, i)) {
      out.push(`${at}/${i}: missing array element`);
      continue;
    }
    const element = ownValue(value, String(i));
    const s = i < prefix.length ? prefix[i] : items;
    if (s !== undefined) out.push(...check(s, element, `${at}/${i}`, defs, depth + 1));
  }
  return out;
}

function objectViolations(
  schema: SchemaObject,
  value: object,
  at: string,
  defs: Defs,
  depth: number,
): string[] {
  const properties = kw(schema, 'properties');
  const patterns = kw(schema, 'patternProperties');
  const additional = kw(schema, 'additionalProperties');
  const names = kw(schema, 'propertyNames');
  const out = (kw(schema, 'required') ?? [])
    .filter((k) => !hasOwn(value, k))
    .map((k) => `${at}/${k}: required`);
  for (const k of Object.keys(value)) {
    const v = ownValue(value, k);
    if (names !== undefined)
      out.push(...check(names, k, `${at}/${k}`, defs, depth + 1).map(() => `${at}/${k}: invalid name`));
    let matched = false;
    if (properties !== undefined && hasOwn(properties, k)) {
      matched = true;
      const prop = ownValue(properties, k) as Schema | undefined;
      if (prop !== undefined) out.push(...check(prop, v, `${at}/${k}`, defs, depth + 1));
    }
    if (patterns !== undefined) {
      for (const p of Object.keys(patterns)) {
        if (regex(p).test(k)) {
          matched = true;
          out.push(...check(ownValue(patterns, p) as Schema, v, `${at}/${k}`, defs, depth + 1));
        }
      }
    }
    if (matched || additional === undefined || additional === true) continue;
    if (additional === false) out.push(`${at}/${k}: not allowed`);
    else out.push(...check(additional, v, `${at}/${k}`, defs, depth + 1));
  }
  return out;
}

/** Plain data that satisfies `schema` (both layers). */
export function conforms(schema: Schema, value: unknown, defs: Defs = NO_DEFS): boolean {
  return plainDataViolations(value).length === 0 && violations(schema, value, '', defs).length === 0;
}

/** A type guard for `T` backed by `schema`. The schema must describe `T`; the wire tests check both agree. */
// T is the type the schema certifies; it appears only in the returned predicate by design
// (no-unnecessary-type-parameters is switched off for this file in eslint.config.js).
export function guard<T>(schema: Schema, defs: Defs = NO_DEFS): (value: unknown) => value is T {
  return (value: unknown): value is T => conforms(schema, value, defs);
}

/**
 * Deep copy of a value that has passed `plainDataViolations`: own enumerable data properties only, same prototype
 * (Object.prototype or null; arrays stay arrays). Keys are defined, never assigned, so an own `__proto__` key stays
 * an ordinary own key. Never calls getters (it reads data descriptors) and never uses `structuredClone`.
 */
export function copyPlainData(value: unknown): unknown {
  if (typeof value !== 'object' || value === null) return value;
  if (Array.isArray(value)) {
    const out: unknown[] = [];
    for (let i = 0; i < value.length; i++) out.push(copyPlainData(ownValue(value, String(i))));
    return out;
  }
  const out: object = Object.getPrototypeOf(value) === null ? (Object.create(null) as object) : {};
  for (const k of Object.keys(value)) {
    Object.defineProperty(out, k, {
      value: copyPlainData(ownValue(value, k)),
      enumerable: true,
      writable: true,
      configurable: true,
    });
  }
  return out;
}

/**
 * Whether the structured-clone algorithm accepts a value. It refuses every Proxy (an exotic object, HTML spec
 * StructuredSerializeInternal), so a transparent Proxy — which no own-property inspection can tell apart from plain
 * data — is rejected (C01-R3-04). Runs the algorithm through `MessageChannel.postMessage`, which serialises
 * synchronously and exists on the whole platform floor; both ports are closed, so nothing is delivered. Returns
 * false when no `MessageChannel` exists (fail closed).
 */
export function structuredCloneable(value: unknown): boolean {
  if (typeof MessageChannel !== 'function') return false;
  const channel = new MessageChannel();
  try {
    channel.port1.postMessage(value);
    return true;
  } catch {
    return false;
  } finally {
    channel.port1.close();
    channel.port2.close();
  }
}

/**
 * Validates a value received from a transport and returns a snapshot of it, or undefined; never throws for any
 * input. The original must be plain data and accepted by the structured-clone algorithm; the snapshot
 * (`copyPlainData`) is validated again, both layers, and is what the caller gets, so later changes to the
 * transport's object cannot affect validated data.
 */
export function certify<T>(check: (value: unknown) => value is T, value: unknown): T | undefined {
  try {
    if (plainDataViolations(value).length > 0) return undefined;
    if (!structuredCloneable(value)) return undefined;
    const snapshot = copyPlainData(value);
    return check(snapshot) ? snapshot : undefined;
  } catch {
    // A hostile Proxy can throw from any trap; that is an invalid value, not a crash of the caller.
    return undefined;
  }
}

/** `namespace.object[.verb]`, matching `is_command_id` in desktop/src-tauri/src/bindings.rs. */
export const COMMAND_ID_PATTERN = '^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$';
