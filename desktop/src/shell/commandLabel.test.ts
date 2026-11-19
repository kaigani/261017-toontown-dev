// D-62 item 3 (C44A-R1-42): the agent indicator and the Activity panel name a call by the registry's label.
import { describe, expect, it } from 'vitest';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { batchLabel, commandLabel } from './commandLabel';

const INDEX = COMMAND_INDEX as Record<string, { label?: string } | undefined>;

describe('commandLabel', () => {
  it("is the registry's label, never the id", () => {
    for (const id of ['agent.server.status', 'draw.rectangle.add', 'edit.selectAll', 'tool.pen.set']) {
      const label = INDEX[id]?.label;
      expect(label, id).toBeTruthy();
      expect(commandLabel(id)).toBe(label);
      expect(commandLabel(id)).not.toBe(id);
    }
  });

  it('names a batch by its own label, else by its step count', () => {
    expect(commandLabel('batch', { label: 'Tidy layers', steps: [{}, {}] })).toBe('Tidy layers');
    expect(commandLabel('batch', { steps: [{}, {}, {}] })).toBe('Batch (3 steps)');
    expect(commandLabel('batch', { steps: [{}] })).toBe('Batch (1 step)');
    expect(commandLabel('batch', {})).toBe('Batch');
    // The acting notice gives the count as a number.
    expect(batchLabel('', 4)).toBe('Batch (4 steps)');
  });

  it('names the transport record of an undelivered answer (D-59 item 4)', () => {
    expect(commandLabel('answer.undelivered')).toBe('Answer not delivered');
  });

  it('falls back to the id only for something the registry does not know', () => {
    expect(commandLabel('no.such.command')).toBe('no.such.command');
  });
});
