// The rows the Timeline shows (ruling E-37): from the members `timeline.rows.list` declares, by the rule the kernel's
// interpreter hit-tests with (`timing_gesture::shown_rows`). Rows come in Timeline order; a row is shown unless one of
// its ancestors is collapsed. Both sides check the shared case table `timeline-rows-cases.json`.

/** The members the rule reads. */
export interface RowShape {
  readonly depth: number;
  readonly collapsed: boolean;
}

/** The indices of the rows shown. */
export function shownIndices(rows: readonly RowShape[]): number[] {
  const out: number[] = [];
  let hiddenUnder: number | null = null;
  rows.forEach((r, i) => {
    if (hiddenUnder !== null) {
      if (r.depth > hiddenUnder) return;
      hiddenUnder = null;
    }
    if (r.collapsed) hiddenUnder = r.depth;
    out.push(i);
  });
  return out;
}

/** The rows shown, in order. */
export function shownRows<T extends RowShape>(rows: readonly T[]): T[] {
  return shownIndices(rows).flatMap((i) => {
    const r = rows[i];
    return r === undefined ? [] : [r];
  });
}
