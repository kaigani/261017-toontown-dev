// Lossless JSON text transport (ADR-0012, C1.1 readiness bullet 1).
//
// The hosts send every product value as JSON text (Rust `tt_engine::wire::to_wire_json`), and the front end sends
// its requests the same way. `JSON.parse` would round integers beyond 2^53 − 1 (a u64 revision, size or seed) to the
// nearest double; this strict RFC 8259 parser keeps them as `bigint` instead, and `coerceNumbers` then turns any
// such literal back into a `number` where the schema says the value is a double (the canonical writer prints doubles
// below 1e21 without an exponent, so `1e20` arrives as plain digits).
//
// The parser builds every object and array itself, so its output is plain data by construction: own enumerable
// data properties defined (never assigned: an own `__proto__` key is just a key), dense arrays, no proxies, at most
// MAX_DEPTH levels. Duplicate keys, lone control characters, trailing garbage and non-JSON tokens are errors.
//
// The engine reads text with the same rules (`tt_model::json_text`), and both run the shared corpus
// `tests/fixtures/wire/hostile-json.json`, so they reject exactly the same texts (C11-R1-17): unpaired surrogates
// (the engine's UTF-8 strings cannot hold them) and integer literals that neither fit the 64-bit ranges nor are
// exactly a double (the engine would have to round them; C11-R1-07) are errors here too. Both writers write an
// integral double with 2^53 <= |x| < 1e21 as its exact integer digits, which a reader takes as that exact integer.

import { hasOwn, MAX_DEPTH, type Defs, type Schema, type SchemaObject, violations } from './schema';

/** A code point in the surrogate range: with the `u` flag only an unpaired surrogate matches. */
const LONE_SURROGATE = /[\uD800-\uDFFF]/u;
const I64_MIN = -(2n ** 63n);
const U64_MAX = 2n ** 64n - 1n;
const TWO_53 = 2 ** 53;

/** A malformed JSON text. */
export class WireJsonError extends Error {
  readonly offset: number;

  constructor(message: string, offset: number) {
    super(`${message} at offset ${offset}`);
    this.name = 'WireJsonError';
    this.offset = offset;
  }
}

function defineKey(target: object, key: string, value: unknown): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

/** Parses JSON text losslessly: integers beyond ±(2^53 − 1) become `bigint`. Throws `WireJsonError`. */
export function parseWireJson(text: string): unknown {
  let pos = 0;
  const fail = (message: string): never => {
    throw new WireJsonError(message, pos);
  };
  const ws = (): void => {
    for (;;) {
      const c = text.charCodeAt(pos);
      if (c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d) pos++;
      else return;
    }
  };
  const literal = (word: string, value: unknown): unknown => {
    if (text.startsWith(word, pos)) {
      pos += word.length;
      return value;
    }
    return fail('invalid literal');
  };
  const string = (): string => {
    // Find the closing quote (skipping escaped characters), then let JSON.parse decode exactly that token: it
    // rejects control characters and invalid escapes, and decodes \u escapes without string assembly (which the
    // IPC boundary lint forbids, ADR-0001).
    const start = pos;
    pos++;
    for (;;) {
      if (pos >= text.length) fail('unterminated string');
      const c = text.charCodeAt(pos);
      if (c === 0x22) break;
      pos += c === 0x5c ? 2 : 1;
    }
    pos++;
    let s: string;
    try {
      s = JSON.parse(text.slice(start, pos)) as string;
    } catch {
      pos = start;
      return fail('invalid string');
    }
    if (LONE_SURROGATE.test(s)) {
      pos = start;
      return fail('unpaired surrogate');
    }
    return s;
  };
  const number = (): number | bigint => {
    const m = /-?(?:0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?/uy;
    m.lastIndex = pos;
    const match = m.exec(text);
    if (match === null) return fail('invalid number');
    const token = match[0];
    pos += token.length;
    if (match[1] === undefined && match[2] === undefined) {
      const n = Number(token);
      // An integer token whose value is not a safe integer cannot be held exactly by a number.
      if (Number.isSafeInteger(n)) return n;
      const exact = BigInt(token);
      // Accepted when the engine can hold it exactly too: a 64-bit integer, or exactly the value of a double.
      if ((exact >= I64_MIN && exact <= U64_MAX) || (Number.isFinite(n) && BigInt(n) === exact)) return exact;
      pos -= token.length;
      return fail('integer literal is outside the 64-bit ranges and not exactly a double');
    }
    const n = Number(token);
    if (!Number.isFinite(n)) fail('number out of range');
    return n;
  };
  const value = (depth: number): unknown => {
    if (depth > MAX_DEPTH) fail(`nested deeper than ${MAX_DEPTH}`);
    ws();
    const c = text[pos];
    switch (c) {
      case '{': {
        pos++;
        const obj: object = {};
        ws();
        if (text[pos] === '}') {
          pos++;
          return obj;
        }
        for (;;) {
          ws();
          if (text[pos] !== '"') fail('expected a key');
          const key = string();
          if (hasOwn(obj, key)) fail(`duplicate key "${key}"`);
          ws();
          if (text[pos] !== ':') fail('expected ":"');
          pos++;
          defineKey(obj, key, value(depth + 1));
          ws();
          if (text[pos] === ',') {
            pos++;
            continue;
          }
          if (text[pos] === '}') {
            pos++;
            return obj;
          }
          fail('expected "," or "}"');
        }
      }
      case '[': {
        pos++;
        const arr: unknown[] = [];
        ws();
        if (text[pos] === ']') {
          pos++;
          return arr;
        }
        for (;;) {
          arr.push(value(depth + 1));
          ws();
          if (text[pos] === ',') {
            pos++;
            continue;
          }
          if (text[pos] === ']') {
            pos++;
            return arr;
          }
          fail('expected "," or "]"');
        }
      }
      case '"':
        return string();
      case 't':
        return literal('true', true);
      case 'f':
        return literal('false', false);
      case 'n':
        return literal('null', null);
      default:
        return number();
    }
  };
  const result = value(1);
  ws();
  if (pos !== text.length) fail('trailing characters');
  return result;
}

/** Writes JSON text; `bigint` values are written as integers. Throws `WireJsonError` for non-JSON values. */
export function stringifyWireJson(value: unknown): string {
  const seen = new Set<object>();
  const write = (v: unknown, depth: number): string => {
    if (depth > MAX_DEPTH) throw new WireJsonError(`nested deeper than ${MAX_DEPTH}`, 0);
    if (v === null) return 'null';
    switch (typeof v) {
      case 'string':
        if (LONE_SURROGATE.test(v)) throw new WireJsonError('unpaired surrogate', 0);
        return JSON.stringify(v);
      case 'boolean':
        return v ? 'true' : 'false';
      case 'number':
        if (!Number.isFinite(v)) throw new WireJsonError('non-finite number', 0);
        // Exact integer digits from 2^53 to 1e21 (the engine's canonical writer does the same; see the header).
        if (Number.isInteger(v) && Math.abs(v) >= TWO_53 && Math.abs(v) < 1e21) return BigInt(v).toString();
        return JSON.stringify(v);
      case 'bigint':
        return v.toString();
      case 'object': {
        if (seen.has(v)) throw new WireJsonError('cycle', 0);
        seen.add(v);
        let out: string;
        if (Array.isArray(v)) {
          out = `[${v.map((x) => write(x, depth + 1)).join(',')}]`;
        } else {
          const parts: string[] = [];
          for (const k of Object.keys(v)) {
            const x: unknown = Object.getOwnPropertyDescriptor(v, k)?.value;
            if (x === undefined) continue;
            if (LONE_SURROGATE.test(k)) throw new WireJsonError('unpaired surrogate', 0);
            parts.push(`${JSON.stringify(k)}:${write(x, depth + 1)}`);
          }
          out = `{${parts.join(',')}}`;
        }
        seen.delete(v);
        return out;
      }
      default:
        throw new WireJsonError(`${typeof v} is not JSON`, 0);
    }
  };
  return write(value, 1);
}

function ownSchema(obj: object | undefined, key: string): Schema | undefined {
  if (obj === undefined || !hasOwn(obj, key)) return undefined;
  return Object.getOwnPropertyDescriptor(obj, key)?.value as Schema | undefined;
}

function deref(schema: Schema, defs: Defs, guard = 0): Schema {
  if (typeof schema === 'boolean' || guard > 32) return schema;
  const ref = ownSchema(schema, '$ref') as unknown;
  if (typeof ref === 'string' && ref.startsWith('#/$defs/')) {
    const target = ownSchema(defs, ref.slice('#/$defs/'.length));
    if (target !== undefined) return deref(target, defs, guard + 1);
  }
  return schema;
}

/** Whether a schema (or any alternative of it) admits doubles but not integers-as-bigint. */
function wantsDouble(schema: Schema, value: unknown, defs: Defs, depth: number): boolean {
  if (typeof schema === 'boolean' || depth > 16) return false;
  const s: SchemaObject = deref(schema, defs) as SchemaObject;
  const type = ownSchema(s, 'type') as unknown;
  const types: unknown[] = typeof type === 'string' ? [type] : Array.isArray(type) ? type : [];
  if (types.includes('number') && !types.includes('integer')) return true;
  // Every `allOf` member applies (C11-R2-05): a double if one asks for it and none insists on an integer.
  const all = ownSchema(s, 'allOf') as unknown;
  if (Array.isArray(all)) {
    const members = all as Schema[];
    const integer = members.some((m) => {
      const t = ownSchema(deref(m, defs) as SchemaObject, 'type') as unknown;
      return t === 'integer' || (Array.isArray(t) && t.includes('integer') && !t.includes('number'));
    });
    if (!integer && members.some((m) => wantsDouble(m, value, defs, depth + 1))) return true;
  }
  for (const key of ['anyOf', 'oneOf'] as const) {
    const alts = ownSchema(s, key) as unknown;
    if (Array.isArray(alts)) {
      const branches = alts as Schema[];
      const integer = branches.some((b) => violations(b, value, '', defs).length === 0);
      if (!integer && branches.some((b) => wantsDouble(b, value, defs, depth + 1))) return true;
    }
  }
  return false;
}

/**
 * The schema objects that apply to `container` under `schema`: the schema itself, every `allOf`/`anyOf`/`oneOf`
 * member and the `if` branch the container selects (`then` when it satisfies `if`, otherwise `else`), with `$ref`s
 * resolved. The generated model schemas put the full-object requirements and the kind rules (e.g. effect parameter
 * types) under such branches (C11-R1-08). Evaluated once per container, not per member.
 */
function applicable(
  schema: Schema,
  defs: Defs,
  container: unknown,
  out: SchemaObject[],
  depth: number,
): void {
  if (typeof schema === 'boolean' || depth > 16) return;
  const s = deref(schema, defs) as SchemaObject;
  if (typeof s === 'boolean') return;
  out.push(s);
  const all = ownSchema(s, 'allOf') as unknown;
  if (Array.isArray(all)) for (const a of all as Schema[]) applicable(a, defs, container, out, depth + 1);
  // Only the `anyOf`/`oneOf` alternatives the container matches apply (C11-R2-05): as parsed, or with its digit-only
  // bigints read as doubles (a double branch cannot match before coercion). An unrelated double branch therefore
  // never converts an integer that another branch accepts.
  for (const k of ['anyOf', 'oneOf'] as const) {
    const alts = ownSchema(s, k) as unknown;
    if (!Array.isArray(alts)) continue;
    let asDoubles: unknown;
    for (const a of alts as Schema[]) {
      if (violations(a, container, '', defs).length === 0) {
        applicable(a, defs, container, out, depth + 1);
        continue;
      }
      asDoubles ??= bigintsAsNumbers(container, 0);
      if (violations(a, asDoubles, '', defs).length === 0) applicable(a, defs, container, out, depth + 1);
    }
  }
  const cond = ownSchema(s, 'if');
  if (cond !== undefined) {
    const branch = ownSchema(s, violations(cond, container, '', defs).length === 0 ? 'then' : 'else');
    if (branch !== undefined) applicable(branch, defs, container, out, depth + 1);
  }
}

/** The schemas of member `key` (`properties`/`additionalProperties`, or `prefixItems`/`items`) of applicable schemas. */
function memberSchemas(schemas: SchemaObject[], key: string | number): Schema[] {
  const out: Schema[] = [];
  for (const s of schemas) {
    if (typeof key === 'number') {
      const prefix = ownSchema(s, 'prefixItems') as unknown;
      const p = Array.isArray(prefix) ? (prefix as Schema[])[key] : undefined;
      const sub = p ?? ownSchema(s, 'items');
      if (sub !== undefined) out.push(sub);
    } else {
      const prop = ownSchema(ownSchema(s, 'properties') as object | undefined, key);
      const ap = ownSchema(s, 'additionalProperties');
      const sub = prop ?? (typeof ap === 'boolean' ? undefined : ap);
      if (sub !== undefined) out.push(sub);
    }
  }
  return out;
}

/** A copy of a parsed value with every `bigint` read as a double (only to test which alternative applies). */
function bigintsAsNumbers(value: unknown, depth: number): unknown {
  if (typeof value === 'bigint') return Number(value);
  if (depth > MAX_DEPTH || typeof value !== 'object' || value === null) return value;
  if (Array.isArray(value)) return value.map((v: unknown) => bigintsAsNumbers(v, depth + 1));
  const out: object = {};
  for (const k of Object.keys(value)) {
    defineKey(out, k, bigintsAsNumbers(Object.getOwnPropertyDescriptor(value, k)?.value, depth + 1));
  }
  return out;
}

/** Whether a parsed value holds a `bigint` anywhere (the parser produces plain data at most MAX_DEPTH deep). */
function holdsBigint(value: unknown, depth: number): boolean {
  if (typeof value === 'bigint') return true;
  if (depth > MAX_DEPTH || typeof value !== 'object' || value === null) return false;
  if (Array.isArray(value)) return value.some((v: unknown) => holdsBigint(v, depth + 1));
  return Object.keys(value).some((k) =>
    holdsBigint(Object.getOwnPropertyDescriptor(value, k)?.value, depth + 1),
  );
}

function coerceWith(value: unknown, schemas: Schema[], defs: Defs, depth: number): unknown {
  if (typeof value === 'bigint') {
    return schemas.some((s) => wantsDouble(s, value, defs, 0)) ? Number(value) : value;
  }
  if (depth > MAX_DEPTH || typeof value !== 'object' || value === null || schemas.length === 0) return value;
  const app: SchemaObject[] = [];
  for (const s of schemas) applicable(s, defs, value, app, 0);
  // Only members that can hold a bigint need their schemas.
  const sub = (item: unknown, key: string | number): unknown =>
    typeof item === 'object' || typeof item === 'bigint'
      ? coerceWith(item, memberSchemas(app, key), defs, depth + 1)
      : item;
  if (Array.isArray(value)) return value.map((item: unknown, i): unknown => sub(item, i));
  const out: object = {};
  for (const k of Object.keys(value)) {
    defineKey(out, k, sub(Object.getOwnPropertyDescriptor(value, k)?.value, k));
  }
  return out;
}

/**
 * Converts `bigint` literals back to `number` where the schema says the value is a double (see the header): where
 * any schema that applies at the position asks for a double. Returns a new value with objects and arrays rebuilt
 * with defined (never assigned) keys, or the value itself when it holds no `bigint` (nothing to convert).
 */
export function coerceNumbers(value: unknown, schema: Schema, defs: Defs, depth = 0): unknown {
  if (!holdsBigint(value, depth)) return value;
  return coerceWith(value, [schema], defs, depth);
}

/**
 * Parses wire text, coerces doubles and validates against `schema`; returns the value or undefined. Never throws.
 * The value is plain data built by the parser (bigint only for integers beyond the safe range).
 */
export function certifyWireText<T>(text: string, schema: Schema, defs: Defs): T | undefined {
  try {
    const parsed = coerceNumbers(parseWireJson(text), schema, defs);
    return violations(schema, parsed, '', defs).length === 0 ? (parsed as T) : undefined;
  } catch {
    return undefined;
  }
}

/** UTF-8 decoding of a binary IPC body (fatal on malformed input). */
export function decodeWireBytes(bytes: ArrayBuffer): string | undefined {
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return undefined;
  }
}
