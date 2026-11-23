// The rows the Timeline shows (ruling E-37): every case of the table the kernel checks too, and the copy byte-equal to
// the kernel's original (crates/executor/src/timeline-rows-cases.json, written by scripts/gen-timeline-rows-cases.py).
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { shownIndices, shownRows } from './rows';
import table from './timeline-rows-cases.json';

const here = path.dirname(fileURLToPath(import.meta.url));

describe('the rows the Timeline shows', () => {
  it('is a byte copy of the kernel table', () => {
    const original = readFileSync(
      path.join(here, '..', '..', '..', 'crates', 'executor', 'src', 'timeline-rows-cases.json'),
      'utf8',
    );
    expect(readFileSync(path.join(here, 'timeline-rows-cases.json'), 'utf8')).toBe(original);
  });

  it('follows every case of the shared table', () => {
    const cases = table.cases as { rows: [number, boolean][]; shown: number[] }[];
    expect(cases.length).toBeGreaterThan(1000);
    for (const c of cases) {
      const rows = c.rows.map(([depth, collapsed]) => ({ depth, collapsed }));
      expect(shownIndices(rows), JSON.stringify(c)).toEqual(c.shown);
    }
  });

  it('keeps the rows themselves, in order', () => {
    const rows = [
      { id: 'a', depth: 0, collapsed: true },
      { id: 'b', depth: 1, collapsed: false },
      { id: 'c', depth: 0, collapsed: false },
    ];
    expect(shownRows(rows).map((r) => r.id)).toEqual(['a', 'c']);
  });
});
