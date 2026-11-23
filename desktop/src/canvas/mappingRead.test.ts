// The canvas mapping's read gate (macOS byte gate, run 45082297045): the older read answering last must not win.
import { describe, expect, it } from 'vitest';

import { mappingReader } from './mappingRead';

type Mapping = number[];

/** A read whose answer the test releases when it chooses. */
function deferred() {
  let resolve: (v: Mapping | null) => void = () => undefined;
  const promise = new Promise<Mapping | null>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

/** The reads in the order the canvas starts them. */
function reads(...answers: ReturnType<typeof deferred>[]) {
  const queue = [...answers];
  return () => queue.shift()?.promise ?? Promise.resolve(null);
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe('the canvas mapping read', () => {
  it("keeps the newest read's mapping when the older read answers last", async () => {
    const mount = deferred();
    const sized = deferred();
    const applied: Mapping[] = [];
    const refresh = mappingReader(
      reads(mount, sized),
      (m) => applied.push(m),
      () => true,
    );
    refresh(); // the mount read (the old size)
    refresh(); // the read after the size report
    sized.resolve([1, 0, 0, 1, 242, 0]);
    await settle();
    mount.resolve([1, 0, 0, 1, 248.5, 0]);
    await settle();
    expect(applied).toEqual([[1, 0, 0, 1, 242, 0]]);
  });

  it('applies answers in order, and none after the canvas stops', async () => {
    const first = deferred();
    const second = deferred();
    let live = true;
    const applied: Mapping[] = [];
    const refresh = mappingReader(
      reads(first, second),
      (m) => applied.push(m),
      () => live,
    );
    refresh();
    first.resolve([2, 0, 0, 2, 0, 0]);
    await settle();
    refresh();
    live = false;
    second.resolve([3, 0, 0, 3, 0, 0]);
    await settle();
    expect(applied).toEqual([[2, 0, 0, 2, 0, 0]]);
  });
});
