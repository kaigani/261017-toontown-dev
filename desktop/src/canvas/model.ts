// The canvas's pull loop (M4 C4.2b; design note 1 §9, note 2 §1; invariants T1, T2, T3, T6, F1, F2).
//
// The frame step is synchronous: IPC answers arrive through promise callbacks into an inbox, and `frame()` only reads
// the inbox, so no frame waits on IPC (F2). Each frame applies inbox work oldest first until its budget is spent (4 ms
// of decode and upload by default), always at least one item so the stream progresses; the rest waits for the next
// frame (F1). Every scene is a full state, so a newer one replaces an older one (T2); at most one scene request and
// one load per drawing are in flight (T3: bound 1). A payload of another generation or of a revision other than the
// latest scene's is discarded and asked for again (T1, T6).

import { decodeDrawing, type DecodedDrawing, FormatError } from './format';
import { parseSceneAnswer, type Scene, type SceneAnswer } from './scene';

export interface CanvasHost {
  canvasScene(viewId: string): Promise<unknown>;
  canvasLoad(drawingId: string, bucket: number): Promise<ArrayBuffer>;
}

/** Where decoded drawings go (a back end, or a recording double in tests). */
export interface ResidentSink {
  upload(drawingId: string, drawing: DecodedDrawing): void;
  drop(drawingId: string): void;
}

export interface Clock {
  now(): number;
}

export interface Resident {
  rev: number;
  bucket: number;
  /** The palette stamp it was loaded at (gradient and texture slots resolve at load). */
  stamp: number;
  /** Has gradient or texture slots, so it reloads on a palette change. */
  paletteBound: boolean;
  bytes: number;
}

export interface FrameStats {
  /** Inbox items applied this frame. */
  applied: number;
  /** Inbox items left for the next frame. */
  carried: number;
  /** Milliseconds spent applying. */
  spentMs: number;
  /** Requests issued this frame. */
  requested: number;
}

type InboxItem =
  | { kind: 'scene'; answer: SceneAnswer }
  | { kind: 'load'; drawingId: string; bucket: number; bytes: ArrayBuffer }
  | { kind: 'failed'; what: string; error: unknown };

const DRAWING_PREFIX = 'dr_';

export class CanvasModel {
  readonly viewId: string;
  private readonly host: CanvasHost;
  private readonly sink: ResidentSink;
  private readonly clock: Clock;
  readonly budgetMs: number;

  private dirty = true;
  private sceneInFlight = false;
  private readonly loading = new Map<string, number>();
  private readonly inbox: InboxItem[] = [];
  private generation: number | null = null;
  private bucket = 0;
  scene: Scene | null = null;
  readonly resident = new Map<string, Resident>();
  /** Failures seen (decode refusals, host errors), newest last, at most 20. */
  readonly failures: string[] = [];
  /** Payloads discarded for a stale generation or revision. */
  discarded = 0;
  /** The largest number of requests in flight at once (T3). */
  peakInFlight = 0;
  /** Bumps whenever what the scene draws changed (a back end redraws its static layers then). */
  version = 0;

  constructor(viewId: string, host: CanvasHost, sink: ResidentSink, clock: Clock, budgetMs = 4) {
    this.viewId = viewId;
    this.host = host;
    this.sink = sink;
    this.clock = clock;
    this.budgetMs = budgetMs;
  }

  /** An engine event arrived: the composition may have changed. */
  markDirty(): void {
    this.dirty = true;
  }

  /** The engine generation the window follows (D-7b: a restart is a generation change). */
  setGeneration(generation: number): void {
    if (this.generation === generation) return;
    this.generation = generation;
    this.dropAll();
    this.dirty = true;
  }

  /** The zoom bucket the view needs (SI-137); a change reloads every resident drawing at the new bucket. */
  setBucket(bucket: number): void {
    if (bucket === this.bucket) return;
    this.bucket = bucket;
  }

  get currentBucket(): number {
    return this.bucket;
  }

  /** Requests in flight (T3). */
  get inFlight(): number {
    return (this.sceneInFlight ? 1 : 0) + this.loading.size;
  }

  private dropAll(): void {
    for (const id of this.resident.keys()) this.sink.drop(id);
    this.resident.clear();
    this.scene = null;
    this.version++;
  }

  private fail(what: string, error: unknown): void {
    const text =
      error instanceof Error ? error.message : typeof error === 'string' ? error : JSON.stringify(error);
    this.failures.push(`${what}: ${text}`);
    if (this.failures.length > 20) this.failures.shift();
  }

  private requestScene(): void {
    this.sceneInFlight = true;
    this.dirty = false;
    this.host.canvasScene(this.viewId).then(
      (v) => {
        try {
          this.inbox.push({ kind: 'scene', answer: parseSceneAnswer(v) });
        } catch (e) {
          this.inbox.push({ kind: 'failed', what: 'scene', error: e });
        }
      },
      (e: unknown) => this.inbox.push({ kind: 'failed', what: 'scene', error: e }),
    );
  }

  private requestLoad(drawingId: string): void {
    const bucket = this.bucket;
    this.loading.set(drawingId, bucket);
    this.host.canvasLoad(drawingId, bucket).then(
      (bytes) => this.inbox.push({ kind: 'load', drawingId, bucket, bytes }),
      (e: unknown) => this.inbox.push({ kind: 'failed', what: `load ${drawingId}`, error: e }),
    );
  }

  private wanted(drawingId: string): boolean {
    const s = this.scene;
    const e = s?.entries.find((x) => x.drawingId === drawingId);
    if (!s || !e) return false;
    const r = this.resident.get(drawingId);
    if (!r) return true;
    return r.rev !== e.rev || r.bucket !== this.bucket || (r.paletteBound && r.stamp !== s.paletteStamp);
  }

  private apply(item: InboxItem): void {
    switch (item.kind) {
      case 'scene': {
        this.sceneInFlight = false;
        const { generation, scene } = item.answer;
        if (this.generation !== null && generation < this.generation) {
          this.discarded++;
          this.dirty = true;
          return;
        }
        if (this.generation !== generation) {
          this.generation = generation;
          this.dropAll();
        }
        this.scene = scene;
        const shown = new Set(scene.entries.map((e) => e.drawingId));
        for (const id of [...this.resident.keys()]) {
          if (!shown.has(id)) {
            this.resident.delete(id);
            this.sink.drop(id);
          }
        }
        this.version++;
        return;
      }
      case 'load': {
        this.loading.delete(item.drawingId);
        let d: DecodedDrawing;
        try {
          d = decodeDrawing(item.bytes);
        } catch (e) {
          this.fail(`load ${item.drawingId}`, e instanceof FormatError ? e : new Error(String(e)));
          return;
        }
        const entry = this.scene?.entries.find((x) => x.drawingId === item.drawingId);
        const stale =
          !entry ||
          Number(d.generation) !== this.generation ||
          Number(d.rev) !== entry.rev ||
          item.bucket !== this.bucket ||
          `${DRAWING_PREFIX}${d.drawingUlid}` !== item.drawingId;
        if (stale) {
          // Never shown at another revision (T1) or generation (T6); asked for again when still wanted.
          this.discarded++;
          return;
        }
        const paletteBound = d.slots.some((s) => s.kind !== 'solid');
        this.sink.upload(item.drawingId, d);
        this.resident.set(item.drawingId, {
          rev: entry.rev,
          bucket: item.bucket,
          stamp: this.scene?.paletteStamp ?? 0,
          paletteBound,
          bytes: d.byteLength,
        });
        this.version++;
        return;
      }
      case 'failed':
        if (item.what === 'scene') this.sceneInFlight = false;
        else this.loading.delete(item.what.slice('load '.length));
        this.fail(item.what, item.error);
        return;
    }
  }

  /** One frame's transport work: apply inbox items within the budget, then issue the requests now needed. */
  frame(): FrameStats {
    const start = this.clock.now();
    let applied = 0;
    while (this.inbox.length > 0) {
      if (applied > 0 && this.clock.now() - start >= this.budgetMs) break;
      const item = this.inbox.shift();
      if (item) this.apply(item);
      applied++;
    }
    const spentMs = this.clock.now() - start;
    let requested = 0;
    if (this.dirty && !this.sceneInFlight) {
      this.requestScene();
      requested++;
    }
    if (this.scene) {
      for (const e of this.scene.entries) {
        if (!this.loading.has(e.drawingId) && this.wanted(e.drawingId)) {
          this.requestLoad(e.drawingId);
          requested++;
        }
      }
    }
    this.peakInFlight = Math.max(this.peakInFlight, this.inFlight);
    return { applied, carried: this.inbox.length, spentMs, requested };
  }

  /** Resident bytes (design note 1 §5's account). */
  residentBytes(): number {
    let n = 0;
    for (const r of this.resident.values()) n += r.bytes;
    return n;
  }
}
