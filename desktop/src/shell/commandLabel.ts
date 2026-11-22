// What an agent's call is called in the window (ruling D-62 item 3, C44A-R1-42; 11 §6.4; design note 2 §5): the
// registry's label of the command, never its id; a batch by its own label, or "Batch (n steps)".
import { COMMAND_INDEX } from '../ipc/generated/commands';

const INDEX = COMMAND_INDEX as Record<string, { label?: string } | undefined>;

/** Calls that are not registry commands: what the activity log records for them (design note 2 §11). */
const OWN: Readonly<Record<string, string>> = {
  'answer.undelivered': 'Answer not delivered',
};

/** A batch's label: its own when it has one, else "Batch (n steps)" (or "Batch" when the count is unknown). */
export function batchLabel(label: unknown, steps: unknown): string {
  if (typeof label === 'string' && label.trim() !== '') return label;
  const n = typeof steps === 'number' ? steps : Array.isArray(steps) ? steps.length : null;
  if (n === null) return 'Batch';
  return `Batch (${String(n)} step${n === 1 ? '' : 's'})`;
}

/** The label of `commandId` as a person reads it; `params` gives a batch its label and step count. */
export function commandLabel(commandId: string, params?: unknown): string {
  if (commandId === 'batch') {
    const p = typeof params === 'object' && params !== null ? (params as Record<string, unknown>) : {};
    return batchLabel(p['label'], p['steps']);
  }
  return INDEX[commandId]?.label ?? OWN[commandId] ?? commandId;
}
