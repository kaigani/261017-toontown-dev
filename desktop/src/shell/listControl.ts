// List controls that send a command when a member is chosen (C43B-R2-03). Each `<option>` declares the reference
// choosing it sends (`data-tt-command`, `data-tt-params`), so `inspect_ui` reports every option with its label, its
// selected state and that reference (`uiTree.ts`), and the parity collector checks every option declares one
// (`parity.ts`). The change handler sends the chosen option's declared reference, never parameters built elsewhere, so
// what an option declares and what choosing it sends cannot differ.
import type { ChangeEvent } from 'react';

import type { ActionsApi } from './actionsApi';

/** The attributes of one option of the list control `listId`: its id, label and the reference choosing it sends. */
export function optionRef(
  listId: string,
  value: string,
  label: string,
  commandId: string,
  params: Record<string, unknown>,
): Record<string, string> {
  return {
    'data-tt-id': `${listId}.${value}`,
    'data-tt-label': label,
    'data-tt-command': commandId,
    'data-tt-params': JSON.stringify(params),
  };
}

/** The reference the chosen option declares, or `null` when it declares none. */
export function chosenRef(
  select: HTMLSelectElement,
): { commandId: string; params: Record<string, unknown> } | null {
  const option = select.selectedOptions[0] as HTMLOptionElement | undefined;
  const commandId = option?.getAttribute('data-tt-command');
  const params = option?.getAttribute('data-tt-params');
  if (!commandId || !params) return null;
  return { commandId, params: JSON.parse(params) as Record<string, unknown> };
}

/** A list control's change handler: runs the chosen option's declared reference. */
export function runChosen(actions: ActionsApi) {
  return (e: ChangeEvent<HTMLSelectElement>) => {
    const ref = chosenRef(e.target);
    if (ref) void actions.run(ref.commandId, ref.params);
  };
}
