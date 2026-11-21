// The canvas's measurements (M4 C4.2b; plan §6 and ruling D-5: GPU and webview quantities, measured in the running
// application with `--canvas-metrics`). Pure: `CanvasView` feeds it timestamps on `performance.now()`'s clock (the
// clock of `Event.timeStamp`), the tests feed it numbers.
//
// - frame time: the interval between consecutive frames (≥ 57 fps means ≤ 17.5 ms p95), and the work of each frame;
// - input-to-ink: from the oldest pointer sample not yet drawn to the frame that draws the overlay with it;
// - release-to-committed: from the pointer-up to the frame that draws the committed drawing (a newer settled model
//   version: every drawing resident at its scene revision, the committed stroke included);
// - loads: each drawing's request-to-upload time (transfer, decode and upload) with its bytes.

const ring = <T>(a: T[], v: T, n: number) => {
  a.push(v);
  if (a.length > n) a.shift();
};

export interface Stat {
  n: number;
  p50: number | null;
  p95: number | null;
  max: number | null;
}

export function stat(values: readonly number[]): Stat {
  if (values.length === 0) return { n: 0, p50: null, p95: null, max: null };
  const s = [...values].sort((a, b) => a - b);
  const at = (q: number) => s[Math.min(s.length - 1, Math.max(0, Math.ceil(q * s.length) - 1))] ?? null;
  const round = (v: number | null) => (v === null ? null : Math.round(v * 100) / 100);
  return { n: s.length, p50: round(at(0.5)), p95: round(at(0.95)), max: round(s[s.length - 1] ?? null) };
}

export class CanvasMetrics {
  readonly frames: number[] = [];
  /** Frame intervals while a gesture is in progress (the plan's "during a gesture"). */
  readonly gestureFrames: number[] = [];
  readonly work: number[] = [];
  /** The work of each frame drawn while a gesture is in progress (C43B-R1S-08: the plan's per-frame budget during a
   *  drag is measured on these frames only, not on idle frames). */
  readonly gestureWork: number[] = [];
  /** The work of each frame drawn with no gesture in progress (the idle measurement's own frames). */
  readonly idleWork: number[] = [];
  readonly ink: number[] = [];
  readonly commit: number[] = [];
  /** Each commit's parts: release to the commit's answer, the answer to the frame drawing it. */
  readonly commitParts: { answerMs: number; drawMs: number }[] = [];
  /** Scene pulls, request to apply (E-C1). */
  readonly scenes: number[] = [];
  readonly loads: { key: string; bytes: number; ms: number }[] = [];
  private last: number | null = null;
  private pendingInk: number | null = null;
  private release: { t: number; version: number; answered: number | null } | null = null;
  /** Keystroke to glyph (design note 5 §8): a key's timeStamp to the frame drawing a newer model. */
  readonly keys: number[] = [];
  private key: { t: number; version: number } | null = null;

  /** A key was typed into the Text tool's editor at `t` (its event's timeStamp), with the model at `version`. */
  keyed(t: number, version: number): void {
    this.key ??= { t, version };
  }

  /** A frame began at `now` and its work took `workMs`; `inGesture` while a gesture is in progress. */
  frame(now: number, workMs: number, inGesture = false): void {
    if (this.last !== null) ring(inGesture ? this.gestureFrames : this.frames, now - this.last, 300);
    this.last = now;
    ring(this.work, workMs, 300);
    ring(inGesture ? this.gestureWork : this.idleWork, workMs, 300);
  }

  /** A pointer sample arrived at `t` (its event's timeStamp). */
  sample(t: number): void {
    this.pendingInk ??= t;
  }

  /** The overlay with every sample so far was drawn at `now`. */
  inkDrawn(now: number): void {
    if (this.pendingInk !== null) {
      ring(this.ink, Math.max(0, now - this.pendingInk), 300);
      this.pendingInk = null;
    }
  }

  /** The pointer was released at `t`, with the model at `version`. */
  released(t: number, version: number): void {
    this.release = { t, version, answered: null };
  }

  /** The commit's answer arrived at `now` (the kernel ran the command). */
  answered(now: number): void {
    if (this.release?.answered === null) this.release.answered = now;
  }

  /** A frame drew the model at `version` at `now`: the committed drawing, when newer than at the release. */
  drawn(version: number, now: number): void {
    if (this.key && version > this.key.version) {
      ring(this.keys, Math.max(0, now - this.key.t), 50);
      this.key = null;
    }
    if (this.release && version > this.release.version) {
      ring(this.commit, Math.max(0, now - this.release.t), 50);
      const a = this.release.answered ?? now;
      ring(
        this.commitParts,
        { answerMs: Math.round((a - this.release.t) * 100) / 100, drawMs: Math.round((now - a) * 100) / 100 },
        50,
      );
      this.release = null;
    }
  }

  scene(ms: number): void {
    ring(this.scenes, ms, 300);
  }

  load(key: string, bytes: number, ms: number): void {
    ring(this.loads, { key, bytes, ms: Math.round(ms * 100) / 100 }, 20);
  }

  snapshot(): {
    frameMs: Stat;
    gestureFrameMs: Stat;
    workMs: Stat;
    gestureWorkMs: Stat;
    idleWorkMs: Stat;
    inkMs: Stat;
    commitMs: Stat;
    keyToGlyphMs: Stat;
    sceneMs: Stat;
    commits: number[];
    /** The raw intervals of the last frames, idle and in a gesture (ruling D-33: the share of long intervals). */
    frameIntervals: number[];
    gestureIntervals: number[];
    /** The raw work of the last frames drawn during a gesture (C43B-R1S-08). */
    gestureWork: number[];
    /** The raw work of the last frames drawn with no gesture, and the raw drag feedback samples: the populations of
     *  `idleWorkMs` and `inkMs` (ruling D-52: a budget is computed from its population). */
    idleWork: number[];
    ink: number[];
    commitParts: CanvasMetrics['commitParts'];
    loads: CanvasMetrics['loads'];
  } {
    return {
      frameMs: stat(this.frames),
      gestureFrameMs: stat(this.gestureFrames),
      workMs: stat(this.work),
      gestureWorkMs: stat(this.gestureWork),
      idleWorkMs: stat(this.idleWork),
      inkMs: stat(this.ink),
      commitMs: stat(this.commit),
      keyToGlyphMs: stat(this.keys),
      sceneMs: stat(this.scenes),
      commits: this.commit.map((v) => Math.round(v * 100) / 100),
      frameIntervals: this.frames.map((v) => Math.round(v * 100) / 100),
      gestureIntervals: this.gestureFrames.map((v) => Math.round(v * 100) / 100),
      gestureWork: this.gestureWork.map((v) => Math.round(v * 100) / 100),
      idleWork: this.idleWork.map((v) => Math.round(v * 100) / 100),
      ink: this.ink.map((v) => Math.round(v * 100) / 100),
      commitParts: [...this.commitParts],
      loads: [...this.loads],
    };
  }
}
