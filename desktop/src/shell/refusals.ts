// How a refused PERSON's action is reported (ruling D-87; finding C44B-R3-01): one place decides, whatever the input
// (a pointer gesture, a key, a native menu item, a context-menu item, a Tool options or Properties control). A refusal
// by the interactive-target rule (`drawing.locked`, `element.locked`, `element.hidden`) or a disabled command
// (`command.disabled`) puts the command's label and the refusal's reason on the status line; no message box (01 §8.4).
import { commandLabel } from './commandLabel';

/** The refusals the status line reports. */
export const REFUSALS: ReadonlySet<string> = new Set([
  'drawing.locked',
  'element.locked',
  'element.hidden',
  'command.disabled',
]);

/** A refusal as the window receives it: an envelope's `error`, or a host error of the key dispatch. */
export interface Refusal {
  readonly code?: string;
  readonly message?: string;
  readonly details?: unknown;
}

/** The status line's text for a refusal of `commandId` ("Delete Points: the element is locked: ..."), or null when the
 *  error is not one the status line reports. The command named in the refusal's details wins over `commandId` (a
 *  gesture names the command it would have sent, a key the command it is bound to). */
export function refusalLine(error: Refusal | undefined, commandId: string): string | null {
  if (!error?.code || !REFUSALS.has(error.code)) return null;
  const d = (typeof error.details === 'object' && error.details !== null ? error.details : {}) as Record<
    string,
    unknown
  >;
  const named = [d['refusedCommandId'], d['commandId']].find((v): v is string => typeof v === 'string');
  const label = commandLabel(named ?? commandId);
  // A disabled command's message names its id; the reason alone is the person's.
  const reason =
    error.code === 'command.disabled' && typeof d['reason'] === 'string' && d['reason'] !== ''
      ? d['reason']
      : (error.message ?? 'not possible here');
  return `${label}: ${reason}`;
}
