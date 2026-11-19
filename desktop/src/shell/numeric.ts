// The numeric field's rules (ruling D-61 items 1-3; C44A-R1-20, -21): strict parsing and the command's declared range.
// The component is `NumericField.tsx`.
import type { Envelope } from '../ipc';
import { COMMAND_DEFS } from '../ipc/generated/commands';
import type { SchemaObject } from '../ipc/schema';

/** A numeric control's declared range. */
export interface NumericRange {
  readonly min?: number;
  readonly max?: number;
  readonly integer?: boolean;
}

const bound = (b: unknown): number | undefined =>
  typeof b === 'number' ? b : typeof b === 'bigint' ? Number(b) : undefined;

/** The range `command` declares for its parameter `param` (minimum, maximum, integer), from the generated schema;
 *  `{}` when the schema declares none. */
export function declaredRange(command: string, param: string): NumericRange {
  const params = COMMAND_DEFS[`${command}.params`];
  if (!params || typeof params === 'boolean') return {};
  const p = params.properties?.[param];
  if (p === undefined || typeof p === 'boolean') return {};
  // A nullable number is written as `type: ["number", "null"]` or as `anyOf: [{number}, {null}]`.
  const s: SchemaObject =
    p.anyOf?.find((x): x is SchemaObject => typeof x !== 'boolean' && x.type !== 'null') ?? p;
  const types = Array.isArray(s.type) ? s.type : [s.type];
  const min = bound(s.minimum);
  const max = bound(s.maximum);
  return {
    ...(min !== undefined && Number.isFinite(min) ? { min } : {}),
    ...(max !== undefined && Number.isFinite(max) && max <= Number.MAX_SAFE_INTEGER ? { max } : {}),
    ...(types.includes('integer') ? { integer: true } : {}),
  };
}

/** A decimal number as a person types it: optional sign, digits with an optional fraction, optional exponent. */
const NUMBER = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;

export type Parsed =
  { readonly ok: true; readonly value: number } | { readonly ok: false; readonly problem: string };

function rangeText(r: NumericRange): string {
  if (r.min !== undefined && r.max !== undefined) return `from ${String(r.min)} to ${String(r.max)}`;
  if (r.min !== undefined) return `at least ${String(r.min)}`;
  return `at most ${String(r.max)}`;
}

/** Strict parsing of a typed entry against a range: the value, or why it is not one. */
export function parseNumber(text: string, range: NumericRange): Parsed {
  const t = text.trim();
  if (t === '') return { ok: false, problem: 'Enter a number.' };
  if (!NUMBER.test(t)) return { ok: false, problem: `"${t}" is not a number.` };
  const value = Number(t);
  if (!Number.isFinite(value)) return { ok: false, problem: `"${t}" is not a number.` };
  if (range.integer && !Number.isInteger(value)) return { ok: false, problem: 'Enter a whole number.' };
  if ((range.min !== undefined && value < range.min) || (range.max !== undefined && value > range.max))
    return { ok: false, problem: `The value is ${rangeText(range)}.` };
  return { ok: true, value };
}

/** What `onCommit` may answer: nothing (accepted), the command's envelope, or a refusal's message. */
export type CommitAnswer = undefined | null | string | Envelope;

/** The refusal's message in a commit's answer, or `null` when it was accepted. */
export function refusalOf(a: CommitAnswer): string | null {
  if (typeof a === 'string') return a;
  if (a && typeof a === 'object' && a.status === 'error') return a.error?.message ?? 'The value was refused.';
  return null;
}
