// The life of a canvas gesture's overlay and preview after its release (C43B-R1S-06; preview note §4, D-7b).
//
// Each gesture (a user's press, an agent gesture's commit) gets a token naming it and the engine generation it was made
// in. A commit's answer arms the clearing of the overlay and preview only while its token is still the current one:
// an answer that arrives after a later gesture began, or after the engine restarted, is obsolete and changes nothing.
// Adopting a new generation and unmounting discard the preview's resources at once, whatever was pending.

export interface GestureToken {
  readonly gesture: number;
  readonly generation: number;
}

/** When the committed state counts as drawn: the scene revision passed `version`, or the clock passed `deadline`. */
export interface ClearAt {
  readonly version: number;
  readonly deadline: number;
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
  frame(settledVersion: number, now: number): boolean {
    const c = this.clear;
    if (!c || this.gone) return false;
    if (!this.isCurrent(c.token)) {
      this.clear = null;
      return false;
    }
    if (settledVersion > c.version || now > c.deadline) {
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
