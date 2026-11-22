// Reads answered out of order (CI run 45035198984): a surface keeps only its newest read's answers.
import { describe, expect, it } from 'vitest';

import { readGate } from './readGate';

describe('the read gate', () => {
  it("applies the newest read's answers and drops an older read's", () => {
    const gate = readGate();
    const first = gate();
    expect(first()).toBe(true);
    const second = gate();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
  });

  it('keeps each surface apart', () => {
    const a = readGate();
    const b = readGate();
    const readA = a();
    b();
    expect(readA()).toBe(true);
  });
});
