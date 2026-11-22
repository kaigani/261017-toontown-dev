// SI-157: a gesture's commit goes out after the reports before it, and the end of its modal state only once the commit
// has answered; C42B-R2-03: an end is never overtaken by the start before it.
import { describe, expect, it } from 'vitest';

import { gestureOrder, type GestureModal } from './gestureOrder';

/** An engine whose calls are logged and answered only when the test says so. */
function fakeEngine() {
  const log: string[] = [];
  const pending: (() => void)[] = [];
  const later = <T>(what: string, value: T, fail = false) =>
    new Promise<T>((resolve, reject) => {
      log.push(`send ${what}`);
      pending.push(() => {
        log.push(`answer ${what}`);
        if (fail) reject(new Error(what));
        else resolve(value);
      });
    });
  let failCommit = false;
  return {
    log,
    failNextCommit() {
      failCommit = true;
    },
    /** Answers the oldest unanswered call, then lets the promise chain run. */
    async answer() {
      pending.shift()?.();
      for (let i = 0; i < 10; i++) await Promise.resolve();
    },
    engine: {
      report(r: { modal: GestureModal }) {
        return later(r.modal ? `on ${r.modal.title}` : 'end', undefined);
      },
      gestureCommit(request: string) {
        const fail = failCommit;
        failCommit = false;
        return later(`commit ${request}`, `env ${request}`, fail);
      },
    },
  };
}

const settle = async () => {
  for (let i = 0; i < 10; i++) await Promise.resolve();
};

describe('the order of a gesture window messages (SI-157)', () => {
  it('sends the commit after the press report and reports the end only after the commit answered', async () => {
    const f = fakeEngine();
    const o = gestureOrder(f.engine);
    o.report({ kind: 'gesture', title: 'transform' });
    const answered = o.commitThenEnd('drag');
    await settle();
    // The commit waits for the press report's answer.
    expect(f.log).toEqual(['send on transform']);
    await f.answer();
    expect(f.log).toEqual(['send on transform', 'answer on transform', 'send commit drag']);
    // The end is not reported while the commit is unanswered.
    await settle();
    expect(f.log).not.toContain('send end');
    await f.answer();
    expect(f.log).toEqual([
      'send on transform',
      'answer on transform',
      'send commit drag',
      'answer commit drag',
      'send end',
    ]);
    await expect(answered).resolves.toBe('env drag');
  });

  it('reports the end after a commit that failed, and keeps later messages behind it', async () => {
    const f = fakeEngine();
    const o = gestureOrder(f.engine);
    f.failNextCommit();
    const answered = o.commitThenEnd('bad');
    o.report({ kind: 'gesture', title: 'select' });
    await settle();
    await f.answer();
    await expect(answered).rejects.toThrow('commit bad');
    // The press report waits for the end's answer.
    expect(f.log).toEqual(['send commit bad', 'answer commit bad', 'send end']);
    await f.answer();
    expect(f.log).toEqual([
      'send commit bad',
      'answer commit bad',
      'send end',
      'answer end',
      'send on select',
    ]);
  });
});
