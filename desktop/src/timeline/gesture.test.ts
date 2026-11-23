// The Timeline gesture controller against a host double (reviews/M5-design-timeline-view.md sections 3 and 10): press,
// preview, release, refusal, cancellation (E-10), abort, and the newest-only scrub (E-11).
import { describe, expect, it } from 'vitest';

import type { CommandFailure, TimingGestureAnswer, TimingGestureRequest, TimingLayout } from '../ipc';
import { TimelineGesture, type Preview } from './gesture';

/** A promise the test resolves when it chooses. */
function later<T>() {
  let resolve: (v: T) => void = () => undefined;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

/** A host double: records requests; answers the press with `target`, the release with `release`. */
function host(target: Record<string, unknown>, release: TimingGestureAnswer) {
  const requests: TimingGestureRequest[] = [];
  const scrubs: { frame: number; done: ReturnType<typeof later<undefined>> }[] = [];
  return {
    requests,
    scrubs,
    timingGesture(request: TimingGestureRequest): Promise<TimingGestureAnswer> {
      requests.push(request);
      if (request.phase === 'press') return Promise.resolve({ ok: true, value: { grabId: 7, target } });
      if (request.phase === 'abort') return Promise.resolve({ ok: true, value: { aborted: true } });
      return Promise.resolve(release);
    },
    scrub(frame: number): Promise<void> {
      const done = later<undefined>();
      scrubs.push({ frame, done });
      return done.promise.then(() => undefined);
    },
  };
}

function panel() {
  const previews: (Preview | null)[] = [];
  const refusals: CommandFailure[] = [];
  const commits: (number | null)[] = [];
  return {
    previews,
    refusals,
    commits,
    events: {
      preview: (p: Preview | null) => previews.push(p),
      refusal: (e: CommandFailure) => refusals.push(e),
      committed: (rev: number | null) => commits.push(rev),
    },
  };
}

const tracks: TimingLayout = {
  surface: 'tracks',
  zoomLevel: 6,
  firstFrame: 1,
  scrollTop: 0,
  platform: 'windows',
};
const ruler: TimingLayout = { ...tracks, surface: 'ruler' };
const at = (x: number, y = 16) => ({ x, y, modifiers: [] });
const committed: TimingGestureAnswer = {
  ok: true,
  value: { plan: { kind: 'commands' }, envelopes: [{ status: 'ok', documentRev: 12 }] },
};

describe('the Timeline gesture controller', () => {
  it('previews from the kernel target and the travel, then commits through the kernel', async () => {
    const h = host({ kind: 'barBody', row: 0, element: 'e1', frame: 10 }, committed);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(tracks, at(160));
    await settle();
    g.move(at(200));
    const last = p.previews[p.previews.length - 1];
    expect(last?.target.kind).toBe('barBody');
    expect(last?.frames).toBe(2);
    expect(last?.dragged).toBe(true);
    await g.release(at(208));
    expect(h.requests.map((r) => r.phase)).toEqual(['press', 'release']);
    const release = h.requests[1];
    expect(release?.phase === 'release' && release.samples.map((s) => s.x)).toEqual([160, 200, 208]);
    expect(p.commits).toEqual([12]);
    expect(p.refusals).toEqual([]);
  });

  it('shows a cancellation on the status line and clears the preview at once (E-10)', async () => {
    const cancelled: TimingGestureAnswer = {
      ok: true,
      value: { cancelled: true, error: { code: 'timeline.targetChanged', message: 'changed' } },
    };
    const h = host({ kind: 'barBody', row: 0, element: 'e1', frame: 10 }, cancelled);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(tracks, at(160));
    await settle();
    await g.release(at(240));
    expect(p.refusals.map((e) => e.code)).toEqual(['timeline.targetChanged']);
    expect(p.previews[p.previews.length - 1]).toBeNull();
    expect(p.commits).toEqual([]);
  });

  it('shows a refused command and clears the preview', async () => {
    const refused: TimingGestureAnswer = {
      ok: true,
      value: {
        plan: {},
        envelopes: [{ status: 'error', error: { code: 'element.locked', message: 'locked' } }],
      },
    };
    const h = host({ kind: 'barBody', row: 0, element: 'e1', frame: 10 }, refused);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(tracks, at(160));
    await settle();
    await g.release(at(240));
    expect(p.refusals.map((e) => e.code)).toEqual(['element.locked']);
    expect(p.previews[p.previews.length - 1]).toBeNull();
  });

  it('aborts on Escape: the grab is dropped, nothing is released', async () => {
    const h = host({ kind: 'barBody', row: 0, element: 'e1', frame: 10 }, committed);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(tracks, at(160));
    g.move(at(240));
    g.abort();
    await settle();
    await g.release(at(260));
    expect(h.requests.map((r) => r.phase)).toEqual(['press', 'abort']);
    expect(p.previews[p.previews.length - 1]).toBeNull();
    expect(p.commits).toEqual([]);
    expect(g.active).toBe(false);
  });

  it('scrubs with the newest frame only while a call is outstanding (E-11)', async () => {
    const h = host({ kind: 'ruler', frame: 3 }, committed);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(ruler, at(40));
    await settle();
    expect(h.scrubs.map((s) => s.frame)).toEqual([3]);
    // While frame 3 is outstanding the pointer crosses frames 4 to 9: only the newest is sent next.
    for (const x of [56, 72, 88, 104, 120, 136]) g.move(at(x));
    expect(h.scrubs.map((s) => s.frame)).toEqual([3]);
    h.scrubs[0]?.done.resolve(undefined);
    await settle();
    expect(h.scrubs.map((s) => s.frame)).toEqual([3, 9]);
    h.scrubs[1]?.done.resolve(undefined);
    await settle();
    expect(h.scrubs).toHaveLength(2);
  });

  it('does not scrub from a press on a free range marker', async () => {
    const h = host({ kind: 'rangeStart' }, committed);
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(ruler, at(40));
    await settle();
    g.move(at(90));
    expect(h.scrubs).toEqual([]);
  });

  it('reports a refused press (no project) and previews nothing', async () => {
    const h = host({}, committed);
    h.timingGesture = (request: TimingGestureRequest) => {
      h.requests.push(request);
      return Promise.resolve({ ok: false, error: { code: 'project.none', message: 'no project' } });
    };
    const p = panel();
    const g = new TimelineGesture(h, p.events);
    g.press(tracks, at(10));
    await settle();
    await g.release(at(10));
    expect(p.refusals.map((e) => e.code)).toEqual(['project.none']);
    expect(h.requests.map((r) => r.phase)).toEqual(['press']);
  });
});
