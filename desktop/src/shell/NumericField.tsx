// The one numeric field of the window (ruling D-61 items 1-3; C44A-R1-20, -21): every numeric control in the panels,
// Preferences and the forms is this component, with one commit rule.
//
// - The display is the retained value (the kernel's, or a form's staged value) whenever nothing is being typed; a
//   draft belongs to the value it was typed from, so a change made elsewhere while editing shows at once.
// - Enter and focus loss commit; Escape restores. A commit parses strictly: an empty or non-numeric entry is never a
//   number (nothing is sent, the field returns to the retained value and says why); a value outside the command's
//   declared range (`declaredRange`, from the generated schema) is refused the same way, naming the range; a whole
//   number is required where the command declares an integer. A valid, changed value is handed to `onCommit`; when the
//   command refuses it, the field returns to the retained value and shows the refusal's message.
// - Arrow Up and Down step the draft by `step` within the range (committed by Enter or focus loss).
//
// The input is a text field (`inputMode="decimal"`), not `type="number"`: a number input reports letters as an empty
// value, which is how C44A-R1-21's letters became a committed zero.
import { useId, useRef, useState, type KeyboardEvent, type Ref } from 'react';

import type { Envelope } from '../ipc';
import { COMMAND_DEFS } from '../ipc/generated/commands';
import type { Schema, SchemaObject } from '../ipc/schema';

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
  const params = COMMAND_DEFS[`${command}.params`] as Schema | undefined;
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
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly problem: string };

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
export type CommitAnswer = void | null | string | Envelope;

const refusalOf = (a: CommitAnswer): string | null => {
  if (typeof a === 'string') return a;
  if (a && typeof a === 'object' && a.status === 'error') return a.error?.message ?? 'The value was refused.';
  return null;
};

export function NumericField(props: {
  /** `data-tt-id`. */
  id: string;
  /** The accessible name. */
  label: string;
  /** The retained value: the kernel's, or the form's staged value. */
  value: number;
  range: NumericRange;
  step?: number;
  /** The command a commit sends, declared with the parameters of the current value (`data-tt-command`). */
  command?: string;
  params?: Record<string, unknown>;
  /** `data-tt-role` (panels use `textField`, Preferences and forms `field`). */
  role?: string;
  disabled?: boolean;
  className?: string;
  inputRef?: Ref<HTMLInputElement>;
  /** The input's element id (for a `<label htmlFor>`). */
  inputId?: string;
  /** `data-tt-label`, where the window tree names the control by something other than its accessible name. */
  ttLabel?: string;
  /** `data-tt-gui-only`: the reason a field that sends no command of its own is GUI-only (a form's `typing`). */
  guiOnly?: string;
  /** Enter on an unchanged value: a form outside a `<form>` element submits here (Enter in any field is its
   *  default button, 01 §2). */
  onSubmit?: () => void;
  /** Called with a valid, changed value; its answer says whether the command refused it. */
  onCommit: (v: number) => CommitAnswer | Promise<CommitAnswer>;
}) {
  const { value, range } = props;
  const [typed, setTyped] = useState<{ from: number; text: string } | null>(null);
  const [problem, setProblem] = useState<{ from: number; text: string } | null>(null);
  const describedBy = useId();
  // The value a commit handed on and is waiting for: an Enter followed by focus loss sends it once.
  const sent = useRef<{ from: number; value: number } | null>(null);
  const draft = typed?.from === value ? typed.text : String(value);
  const shownProblem = problem?.from === value ? problem.text : null;

  const restore = (why: string | null) => {
    setTyped(null);
    setProblem(why === null ? null : { from: value, text: why });
  };
  /** Commits the draft; `true` when it handed a changed value on (a form's Enter then does not submit yet). */
  const commit = (): boolean => {
    if (typed?.from !== value) return false;
    const parsed = parseNumber(typed.text, range);
    if (!parsed.ok) {
      restore(parsed.problem);
      return true;
    }
    if (parsed.value === value) {
      restore(null);
      return false;
    }
    if (sent.current?.from === value && sent.current.value === parsed.value) return true;
    const from = value;
    sent.current = { from, value: parsed.value };
    setProblem(null);
    void Promise.resolve(props.onCommit(parsed.value)).then((answer) => {
      sent.current = null;
      const refused = refusalOf(answer);
      // The draft goes either way: the display is the retained value, which an accepted commit has changed.
      setTyped((t) => (t?.from === from ? null : t));
      if (refused !== null) setProblem({ from, text: refused });
    });
    return true;
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      // A form's Enter submits the values it holds: a field that has just changed its value takes this Enter.
      if (commit()) e.preventDefault();
      else props.onSubmit?.();
    } else if (e.key === 'Escape') {
      if (typed?.from === value || shownProblem !== null) {
        e.preventDefault();
        e.stopPropagation();
      }
      restore(null);
    } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const base = parseNumber(draft, {});
      const step = props.step ?? 1;
      let next = (base.ok ? base.value : value) + (e.key === 'ArrowUp' ? step : -step);
      if (range.min !== undefined) next = Math.max(range.min, next);
      if (range.max !== undefined) next = Math.min(range.max, next);
      setTyped({ from: value, text: String(Number(next.toFixed(6))) });
    }
  };
  return (
    <>
      <input
        ref={props.inputRef}
        id={props.inputId}
        type="text"
        inputMode="decimal"
        className={props.className}
        value={draft}
        disabled={props.disabled}
        aria-label={props.label}
        aria-invalid={shownProblem !== null ? true : undefined}
        aria-describedby={shownProblem !== null ? describedBy : undefined}
        title={shownProblem ?? undefined}
        data-tt-role={props.role ?? 'textField'}
        data-tt-id={props.id}
        {...(props.ttLabel ? { 'data-tt-label': props.ttLabel } : {})}
        {...(props.guiOnly ? { 'data-tt-gui-only': props.guiOnly } : {})}
        {...(props.command ? { 'data-tt-command': props.command } : {})}
        {...(props.params ? { 'data-tt-params': JSON.stringify(props.params) } : {})}
        {...(shownProblem !== null ? { 'data-tt-problem': shownProblem } : {})}
        onChange={(e) => {
          setTyped({ from: value, text: e.target.value });
          setProblem(null);
        }}
        onKeyDown={onKeyDown}
        onBlur={() => {
          commit();
        }}
      />
      {shownProblem !== null && (
        <small id={describedBy} className="field-problem" role="status">
          {shownProblem}
        </small>
      )}
    </>
  );
}
