// C43B-R1S-06: a commit's answer acts only for its own gesture and generation; adopting a generation and unmounting
// drop the preview's resources. Deterministic: the commits are answered when the test says, the clock is the test's.
import { describe, expect, it } from 'vitest';

import { GestureLifecycle, type GestureToken } from './gestureLifecycle';

/** The canvas side as CanvasView keeps it: an overlay, a preview holding two GPU parts, a renderer's resident keys. */
function canvas(generation = 1) {
  const resident = new Set<string>();
  const state = {
    overlay: null as string | null,
    preview: null as string | null,
    parts: null as string[] | null,
  };
  const life = new GestureLifecycle(generation, () => {
    state.overlay = null;
    state.preview = null;
    for (const k of state.parts ?? []) resident.delete(k);
    state.parts = null;
  });
  const pending: { token: GestureToken; answer: (failed: boolean) => void }[] = [];
  return {
    life,
    state,
    resident,
    /** A press of the Select tool: its overlay, its preview, its two parts loaded. */
    press(name: string): GestureToken {
      // The press first drops the previous preview (CanvasView's onDown: endPreview), then begins.
      for (const k of state.parts ?? []) resident.delete(k);
      const token = life.begin();
      state.overlay = `${name} overlay`;
      state.preview = `${name} preview`;
      state.parts = [`${name}#rest`, `${name}#selection`];
      for (const k of state.parts) resident.add(k);
      return token;
    },
    /** The release: the commit goes out; its answer comes when `answerOldest` is called. */
    release(token: GestureToken, version: number) {
      pending.push({
        token,
        answer: (failed) => {
          if (life.answered(token, { version, deadline: failed ? 0 : 1000 }) && failed) state.overlay = null;
        },
      });
    },
    answerOldest(failed = false) {
      pending.shift()?.answer(failed);
    },
  };
}

describe('gesture lifecycle (C43B-R1S-06)', () => {
  it('a delayed answer arriving after a second gesture began leaves the second gesture alone', () => {
    const c = canvas();
    const first = c.press('first');
    c.release(first, 5);
    const second = c.press('second');
    // The first commit answers now, while the second drag is in progress.
    c.answerOldest();
    expect(c.life.isCurrent(first)).toBe(false);
    expect(c.life.isCurrent(second)).toBe(true);
    // Frames pass with the scene revision moved on and the clock past every deadline: nothing is cleared.
    expect(c.life.frame(99, 5000)).toBe(false);
    expect(c.state).toEqual({
      overlay: 'second overlay',
      preview: 'second preview',
      parts: ['second#rest', 'second#selection'],
    });
    expect([...c.resident]).toEqual(['second#rest', 'second#selection']);
    // The second gesture's own answer clears it once its committed state is drawn.
    c.release(second, 6);
    c.answerOldest();
    expect(c.life.frame(6, 10)).toBe(false);
    expect(c.life.frame(7, 10)).toBe(true);
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
    expect(c.resident.size).toBe(0);
  });

  it('a refused delayed answer does not drop the overlay of the gesture after it', () => {
    const c = canvas();
    const first = c.press('first');
    c.release(first, 5);
    c.press('second');
    c.answerOldest(true);
    expect(c.state.overlay).toBe('second overlay');
    expect(c.life.frame(0, 5000)).toBe(false);
    expect(c.state.preview).toBe('second preview');
  });

  it('a restart between the release and the answer discards the preview at once and ignores the answer', () => {
    const c = canvas(1);
    const first = c.press('first');
    c.release(first, 5);
    // The engine restarts before the commit answers: adopting the new generation drops the preview and its parts.
    expect(c.life.adopt(2)).toBe(true);
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
    expect(c.resident.size).toBe(0);
    expect(c.life.generation).toBe(2);
    // The old generation's answer arrives late: obsolete, nothing armed.
    c.answerOldest();
    expect(c.life.isCurrent(first)).toBe(false);
    // A gesture of the new generation is not cleared by it.
    const next = c.press('next');
    expect(next.generation).toBe(2);
    expect(c.life.frame(99, 5000)).toBe(false);
    expect(c.state.preview).toBe('next preview');
    // Adopting the generation already followed changes nothing.
    expect(c.life.adopt(2)).toBe(false);
    expect(c.state.preview).toBe('next preview');
  });

  it('an answer for the current gesture clears at its deadline when the revision does not move', () => {
    const c = canvas();
    const g = c.press('only');
    c.release(g, 5);
    c.answerOldest();
    expect(c.life.frame(5, 999)).toBe(false);
    expect(c.life.frame(5, 1001)).toBe(true);
    expect(c.resident.size).toBe(0);
  });

  it('unmounting drops the preview resources and later answers act on nothing', () => {
    const c = canvas();
    const g = c.press('only');
    c.release(g, 5);
    c.life.unmount();
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
    expect(c.resident.size).toBe(0);
    c.answerOldest();
    expect(c.life.isCurrent(g)).toBe(false);
    expect(c.life.frame(99, 5000)).toBe(false);
  });
});
