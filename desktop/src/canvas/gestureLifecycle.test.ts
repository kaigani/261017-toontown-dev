// C43B-R1S-06: a commit's answer acts only for its own gesture and generation; adopting a generation and unmounting
// drop the preview's resources. C43B-R2S-02: after an answer that made a step, only the frame drawing the committed
// state clears the preview; a deadline applies only to an answer without a step. Deterministic: the commits are answered
// when the test says, the clock is the test's (the answer arrives at time 0, as CanvasView's `clearAt` computes it).
import { describe, expect, it } from 'vitest';

import { answerMadeStep, answerRevision, GestureLifecycle, type GestureToken } from './gestureLifecycle';

/** How an answer lets the preview go, as CanvasView's `clearAt` does at the answer's arrival (time 0). */
type Outcome = 'step' | 'noStep' | 'refused';
const deadlineOf = (o: Outcome): number | null => (o === 'step' ? null : o === 'refused' ? 0 : 400);
/** What the window draws: a scene fully resident at local version `v` and document revision `rev` (by default the
 *  same number: each revision drawn once). */
const drawn = (v: number, rev: number = v) => ({ settledVersion: v, settledDocumentRev: BigInt(rev) });

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
  const pending: { token: GestureToken; answer: (failed: Outcome) => void }[] = [];
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
    /** The release at document revision `rev`: the commit goes out; its answer comes when `answerOldest` is called,
     *  with the revision `rev + 1` when it made a step (as CanvasView's `clearAt` reads the envelope). */
    release(token: GestureToken, rev: number) {
      pending.push({
        token,
        answer: (failed: Outcome) => {
          const at = {
            documentRev: failed === 'step' ? BigInt(rev + 1) : null,
            deadline: deadlineOf(failed),
            afterVersion: rev,
          };
          if (life.answered(token, at) && failed === 'refused') state.overlay = null;
        },
      });
    },
    answerOldest(failed: Outcome = 'step') {
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
    expect(c.life.frame(drawn(99), 5000)).toBe(false);
    expect(c.state).toEqual({
      overlay: 'second overlay',
      preview: 'second preview',
      parts: ['second#rest', 'second#selection'],
    });
    expect([...c.resident]).toEqual(['second#rest', 'second#selection']);
    // The second gesture's own answer clears it once its committed state is drawn.
    c.release(second, 6);
    c.answerOldest();
    expect(c.life.frame(drawn(6), 10)).toBe(false);
    expect(c.life.frame(drawn(7), 10)).toBe(true);
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
    expect(c.resident.size).toBe(0);
  });

  it('a refused delayed answer does not drop the overlay of the gesture after it', () => {
    const c = canvas();
    const first = c.press('first');
    c.release(first, 5);
    c.press('second');
    c.answerOldest('refused');
    expect(c.state.overlay).toBe('second overlay');
    expect(c.life.frame(drawn(0), 5000)).toBe(false);
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
    expect(c.life.frame(drawn(99), 5000)).toBe(false);
    expect(c.state.preview).toBe('next preview');
    // Adopting the generation already followed changes nothing.
    expect(c.life.adopt(2)).toBe(false);
    expect(c.state.preview).toBe('next preview');
  });

  it('an answer that made no step clears at its deadline when the revision does not move', () => {
    const c = canvas();
    const g = c.press('only');
    c.release(g, 5);
    c.answerOldest('noStep');
    expect(c.life.frame(drawn(5), 399)).toBe(false);
    expect(c.life.frame(drawn(5), 401)).toBe(true);
    expect(c.resident.size).toBe(0);
  });

  it('C43B-R2S-02: a successful step whose model settles after 400 ms keeps the preview until it is drawn', () => {
    const c = canvas();
    const g = c.press('only');
    c.release(g, 5);
    c.answerOldest('step');
    // Frames long past any deadline while the committed state has not been drawn: the preview stays.
    for (const now of [100, 400, 401, 1000, 5000]) expect(c.life.frame(drawn(5), now)).toBe(false);
    expect(c.state.preview).toBe('only preview');
    expect([...c.resident]).toEqual(['only#rest', 'only#selection']);
    // The frame that draws the committed state (the revision past the one before the commit) clears it.
    expect(c.life.frame(drawn(6), 5001)).toBe(true);
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
  });

  it('C43B-R3S-02: a scene answer in flight before the commit settles without clearing the preview', () => {
    const c = canvas();
    const g = c.press('only');
    // Released at revision 5 while a scene reload (a view change) is in flight; that reload's answer arrives first and
    // settles: the local version moves on, but its scene was built at revision 5.
    c.release(g, 5);
    expect(c.life.frame(drawn(6, 5), 10)).toBe(false);
    // The commit then answers with a step at revision 6, while its scene is still loading.
    c.answerOldest('step');
    // The next frames draw the pre-commit scene (revision 5) at later local versions: the preview stays.
    expect(c.life.frame(drawn(6, 5), 20)).toBe(false);
    expect(c.life.frame(drawn(7, 5), 5000)).toBe(false);
    expect(c.state.preview).toBe('only preview');
    // The scene at the commit's revision is resident: that frame clears it.
    expect(c.life.frame(drawn(8, 6), 5001)).toBe(true);
    expect(c.state).toEqual({ overlay: null, preview: null, parts: null });
  });

  it("a commit's revision is read exactly from its envelope", () => {
    expect(answerRevision({ status: 'ok', documentRev: 7 })).toBe(7n);
    expect(answerRevision({ status: 'ok', documentRev: 18446744073709551615n })).toBe(18446744073709551615n);
    expect(answerRevision({ status: 'ok', documentRev: null })).toBeNull();
    expect(answerRevision({ status: 'ok' })).toBeNull();
  });

  it('a refused answer clears at once', () => {
    const c = canvas();
    const g = c.press('only');
    c.release(g, 5);
    c.answerOldest('refused');
    expect(c.state.overlay).toBeNull();
    expect(c.life.frame(drawn(5), 1)).toBe(true);
    expect(c.state.preview).toBeNull();
  });

  it('a step is told by the envelope or by a produced command', () => {
    expect(answerMadeStep({ status: 'ok', undo: { stepId: 'u1' } })).toBe(true);
    expect(
      answerMadeStep({ status: 'ok', result: { commands: [{ envelope: { undo: { stepId: 'u2' } } }] } }),
    ).toBe(true);
    expect(answerMadeStep({ status: 'ok', result: { commands: [{ envelope: {} }] } })).toBe(false);
    expect(answerMadeStep({ status: 'ok' })).toBe(false);
    expect(answerMadeStep({ status: 'error', undo: { stepId: 'u3' } })).toBe(false);
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
    expect(c.life.frame(drawn(99), 5000)).toBe(false);
  });
});
