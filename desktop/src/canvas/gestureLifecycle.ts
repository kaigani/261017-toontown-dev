// The life of a canvas gesture's overlay and preview after its release (C43B-R1S-06; preview note §4, D-7b).
//
// Each gesture (a user's press, an agent gesture's commit) gets a token naming it and the engine generation it was made
// in. A commit's answer arms the clearing of the overlay and preview only while its token is still the current one:
// an answer that arrives after a later gesture began, or after the engine restarted, is obsolete and changes nothing.
// Adopting a new generation and unmounting discard the preview's resources at once, whatever was pending.
//
// C43B-R2S-02 and C43B-R3S-02 (preview note §4, ruling D-48): after a successful answer that made an undo step, the
// overlay and preview are cleared only by a frame that draws a scene resident at the document revision the commit
// answered with, however long that takes: a slow redraw must not show the old state in between, and a scene answer
// that was in flight before the commit (built at an older revision) settles without clearing it. A time fallback
// applies only to an answer without a step: a refusal, an error (at once) or a commit that changed nothing (briefly).

export interface GestureToken {
  readonly gesture: number;
  readonly generation: number;
}

/** When the overlay and preview give way. After a step: the frame that draws a scene resident at `documentRev` (the
 *  revision the commit answered with) or later. Without a step: the first frame after `deadline`. `afterVersion`
 *  serves only a step whose answer carries no revision (never a kernel answer): a scene settled after the answer. */
export interface ClearAt {
  readonly documentRev: bigint | null;
  readonly deadline: number | null;
  readonly afterVersion: number;
}

/** What the window draws now: the latest scene fully resident, its local version and its document revision. */
export interface Drawn {
  readonly settledVersion: number;
  readonly settledDocumentRev: bigint | null;
}

/** The part of a gesture commit's answer that says whether it made an undo step: the envelope's own `undo`, or that of
 *  a command the gesture produced (`result.commands[].envelope.undo`). */
export interface CommitAnswer {
  readonly status: string;
  readonly result?: unknown;
  readonly undo?: unknown;
  readonly documentRev?: number | bigint | null;
}

/** The document revision a commit answered with (exact), or null. */
export function answerRevision(env: CommitAnswer): bigint | null {
  const r = env.documentRev;
  if (typeof r === 'bigint') return r;
  if (typeof r === 'number' && Number.isSafeInteger(r) && r >= 0) return BigInt(r);
  return null;
}

/** Whether a gesture commit's answer made an undo step (a successful change of the document). */
export function answerMadeStep(env: CommitAnswer): boolean {
  if (env.status !== 'ok') return false;
  if (env.undo !== undefined && env.undo !== null) return true;
  const commands = (env.result as { commands?: unknown } | undefined)?.commands;
  return (
    Array.isArray(commands) &&
    commands.some((c) => {
      const undo = (c as { envelope?: { undo?: unknown } } | null)?.envelope?.undo;
      return undo !== undefined && undo !== null;
    })
  );
}

export class GestureLifecycle {
  private seq = 0;
  private latest: GestureToken | null = null;
  private clear: (ClearAt & { readonly token: GestureToken }) | null = null;
  private gone = false;

  /** `discard` drops the overlay and the preview with its resources (the GPU parts it loaded). */
  constructor(
    private followed: number,
    private readonly discard: () => void,
  ) {}

  /** The generation this window follows. */
  get generation(): number {
    return this.followed;
  }

  /** A new gesture: earlier gestures' answers become obsolete, and a clearing they armed is disarmed (the new
   *  gesture's overlay is not cleared by an older commit). */
  begin(): GestureToken {
    this.seq += 1;
    this.latest = { gesture: this.seq, generation: this.followed };
    this.clear = null;
    return this.latest;
  }

  /** Whether `token` is the latest gesture of the generation followed, in a mounted window. */
  isCurrent(token: GestureToken): boolean {
    return (
      !this.gone &&
      this.latest !== null &&
      token.gesture === this.latest.gesture &&
      token.generation === this.latest.generation &&
      token.generation === this.followed
    );
  }

  /** A commit answered: arms the clearing when its gesture is still current. Returns whether it was. */
  answered(token: GestureToken, at: ClearAt): boolean {
    if (!this.isCurrent(token)) return false;
    this.clear = { ...at, token };
    return true;
  }

  /** Once per frame: when the armed clearing is due, disarms it and discards. Returns whether it discarded. */
  frame(drawn: Drawn, now: number): boolean {
    const c = this.clear;
    if (!c || this.gone) return false;
    if (!this.isCurrent(c.token)) {
      this.clear = null;
      return false;
    }
    const due =
      c.documentRev !== null
        ? drawn.settledDocumentRev !== null && drawn.settledDocumentRev >= c.documentRev
        : c.deadline !== null
          ? now > c.deadline
          : drawn.settledVersion > c.afterVersion;
    if (due) {
      this.clear = null;
      this.discard();
      return true;
    }
    return false;
  }

  /** A new engine generation: every gesture of the old one is obsolete and the preview is discarded now (D-7b).
   *  Returns false when the generation is the one already followed. */
  adopt(generation: number): boolean {
    if (generation === this.followed) return false;
    this.followed = generation;
    this.latest = null;
    this.clear = null;
    this.discard();
    return true;
  }

  /** The window closes: the preview's resources are dropped and no later answer acts. */
  unmount(): void {
    if (this.gone) return;
    this.gone = true;
    this.latest = null;
    this.clear = null;
    this.discard();
  }
}
