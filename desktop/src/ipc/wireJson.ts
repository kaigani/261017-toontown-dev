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

import { hasOwn, MAX_DEPTH, type Defs, type Schema, type SchemaObject, violations } from './schema';

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
    try {
      return JSON.parse(text.slice(start, pos)) as string;
    } catch {
      pos = start;
      return fail('invalid string');
    }
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
      return Number.isSafeInteger(n) ? n : BigInt(token);
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
        return JSON.stringify(v);
      case 'boolean':
        return v ? 'true' : 'false';
      case 'number':
        if (!Number.isFinite(v)) throw new WireJsonError('non-finite number', 0);
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

function childSchemas(schema: Schema, defs: Defs, key: string | number, depth: number): Schema[] {
  if (typeof schema === 'boolean' || depth > 16) return [];
  const s = deref(schema, defs) as SchemaObject;
  const out: Schema[] = [];
  if (typeof key === 'number') {
    const prefix = ownSchema(s, 'prefixItems') as unknown;
    const p = Array.isArray(prefix) ? (prefix as Schema[])[key] : undefined;
    if (p !== undefined) out.push(p);
    else {
      const items = ownSchema(s, 'items');
      if (items !== undefined) out.push(items);
    }
  } else {
    const prop = ownSchema(ownSchema(s, 'properties') as object | undefined, key);
    if (prop !== undefined) out.push(prop);
    else {
      const ap = ownSchema(s, 'additionalProperties');
      if (ap !== undefined && typeof ap !== 'boolean') out.push(ap);
    }
  }
  for (const k of ['anyOf', 'oneOf', 'allOf'] as const) {
    const alts = ownSchema(s, k) as unknown;
    if (Array.isArray(alts))
      for (const a of alts as Schema[]) out.push(...childSchemas(a, defs, key, depth + 1));
  }
  return out;
}

/**
 * Converts `bigint` literals back to `number` where the schema says the value is a double (see the header). Returns
 * a new value; objects and arrays are rebuilt with defined (never assigned) keys.
 */
export function coerceNumbers(value: unknown, schema: Schema, defs: Defs, depth = 0): unknown {
  if (typeof value === 'bigint') {
    return wantsDouble(schema, value, defs, 0) ? Number(value) : value;
  }
  if (depth > MAX_DEPTH || typeof value !== 'object' || value === null) return value;
  if (Array.isArray(value)) {
    return value.map((item: unknown, i): unknown => {
      const schemas = childSchemas(schema, defs, i, 0);
      return schemas.length === 0 ? item : coerceNumbers(item, schemas[0] ?? true, defs, depth + 1);
    });
  }
  const out: object = {};
  for (const k of Object.keys(value)) {
    const item: unknown = Object.getOwnPropertyDescriptor(value, k)?.value;
    const schemas = childSchemas(schema, defs, k, 0);
    // Several candidate schemas (alternatives): coerce only if one of them asks for a double.
    const target =
      schemas.find((c) => typeof item === 'bigint' && wantsDouble(c, item, defs, 0)) ?? schemas[0];
    defineKey(out, k, target === undefined ? item : coerceNumbers(item, target, defs, depth + 1));
  }
  return out;
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
