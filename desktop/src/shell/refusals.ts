// How a refused PERSON's action is reported (ruling D-87; finding C44B-R3-01): one place decides, whatever the input
// (a pointer gesture, a key, a native menu item, a context-menu item, a Tool options or Properties control). A refusal
// by the interactive-target rule (`drawing.locked`, `element.locked`, `element.hidden`) or a disabled command
// (`command.disabled`) puts the command's label and the refusal's reason on the status line; no message box (01 §8.4).
// A handler panic the engine contained (ruling E-29) is reported the same way, and so is a call that waited for the
// system fonts past its bound (ruling E-43, SI-215), with the line shown while a call waits for them.
import { commandLabel } from './commandLabel';

/** The status line while a call waits for the system font list (`shell.fontsLoading {waiting: true}`; ruling E-43). */
export const FONTS_READING_LINE = 'Reading the system fonts...';

/** The status line for the `shell.fontsLoading` notice, or null when nothing waits (the list was read). */
export function fontsNoticeLine(payload: unknown): string | null {
  const waiting =
    typeof payload === 'object' && payload !== null && (payload as Record<string, unknown>)['waiting'];
  return waiting === true ? FONTS_READING_LINE : null;
}

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
  const d = (typeof error?.details === 'object' && error.details !== null ? error.details : {}) as Record<
    string,
    unknown
  >;
  // A handler panic the engine contained (ruling E-29): one failed call, nothing changed, the engine keeps serving.
  // The person is told once, here; no message box.
  if (error?.code === 'internal' && d['reason'] === 'handlerPanicked') {
    const named = typeof d['commandId'] === 'string' ? d['commandId'] : commandId;
    return `${commandLabel(named)}: failed unexpectedly; nothing was changed`;
  }
  // The system fonts were still being read past the call's bound (ruling E-43): nothing was changed; a retry works.
  if (error?.code === 'command.timeout' && d['reason'] === 'fontsLoading') {
    const named = typeof d['commandId'] === 'string' ? d['commandId'] : commandId;
    return `${commandLabel(named)}: the system fonts are still being read; nothing was changed, try again in a moment`;
  }
  if (!error?.code || !REFUSALS.has(error.code)) return null;
  const named = [d['refusedCommandId'], d['commandId']].find((v): v is string => typeof v === 'string');
  const label = commandLabel(named ?? commandId);
  // A disabled command's message names its id; the reason alone is the person's.
  const reason =
    error.code === 'command.disabled' && typeof d['reason'] === 'string' && d['reason'] !== ''
      ? d['reason']
      : (error.message ?? 'not possible here');
  return `${label}: ${reason}`;
}
