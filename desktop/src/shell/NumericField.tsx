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

import { parseNumber, refusalOf, type CommitAnswer, type NumericRange } from './numeric';

export function NumericField({
  id,
  label,
  value,
  range,
  step,
  command,
  params,
  role,
  disabled,
  className,
  inputRef,
  inputId,
  ttLabel,
  guiOnly,
  mixed,
  disabledReason,
  onSubmit,
  onCommit,
}: {
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
  /** The value differs over what the field shows (the Text panel over a highlight, design note 5 §6): empty, with the
   *  placeholder "Mixed"; any typed value is a change. */
  mixed?: boolean;
  /** Why the field is disabled (the design note for panels §1: disabled rather than hidden): shown as its tooltip,
   *  and the field shows no value. */
  disabledReason?: string;
  /** Enter on an unchanged value: a form outside a `<form>` element submits here (Enter in any field is its
   *  default button, 01 §2). */
  onSubmit?: () => void;
  /** Called with a valid, changed value; its answer says whether the command refused it. */
  onCommit: ((v: number) => void) | ((v: number) => CommitAnswer | Promise<CommitAnswer>);
}) {
  const [typed, setTyped] = useState<{ from: number; text: string } | null>(null);
  const [problem, setProblem] = useState<{ from: number; text: string } | null>(null);
  const describedBy = useId();
  // The value a commit handed on and is waiting for: an Enter followed by focus loss sends it once.
  const sent = useRef<{ from: number; value: number } | null>(null);
  const blank = mixed === true || (disabled === true && disabledReason !== undefined);
  const draft = typed?.from === value ? typed.text : blank ? '' : String(value);
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
    if (parsed.value === value && !mixed) {
      restore(null);
      return false;
    }
    if (sent.current?.from === value && sent.current.value === parsed.value) return true;
    const from = value;
    sent.current = { from, value: parsed.value };
    setProblem(null);
    void Promise.resolve(onCommit(parsed.value)).then((answer) => {
      sent.current = null;
      // A staged form's commit answers nothing (`void`): accepted.
      const refused = refusalOf(answer ?? null);
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
      else onSubmit?.();
    } else if (e.key === 'Escape') {
      if (typed?.from === value || shownProblem !== null) {
        e.preventDefault();
        e.stopPropagation();
      }
      restore(null);
    } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const base = parseNumber(draft, {});
      const by = step ?? 1;
      let next = (base.ok ? base.value : value) + (e.key === 'ArrowUp' ? by : -by);
      if (range.min !== undefined) next = Math.max(range.min, next);
      if (range.max !== undefined) next = Math.min(range.max, next);
      setTyped({ from: value, text: String(Number(next.toFixed(6))) });
    }
  };
  return (
    <>
      <input
        ref={inputRef}
        id={inputId}
        type="text"
        inputMode="decimal"
        className={className}
        value={draft}
        placeholder={mixed ? 'Mixed' : undefined}
        disabled={disabled}
        aria-label={label}
        aria-invalid={shownProblem !== null ? true : undefined}
        aria-describedby={shownProblem !== null ? describedBy : undefined}
        title={shownProblem ?? (disabled ? disabledReason : undefined)}
        {...(disabled && disabledReason ? { 'data-tt-disabled-reason': disabledReason } : {})}
        data-tt-role={role ?? 'textField'}
        data-tt-id={id}
        {...(ttLabel ? { 'data-tt-label': ttLabel } : {})}
        {...(guiOnly ? { 'data-tt-gui-only': guiOnly } : {})}
        {...(command ? { 'data-tt-command': command } : {})}
        {...(params ? { 'data-tt-params': JSON.stringify(params) } : {})}
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
