// @vitest-environment node
// The canvas pull loop (design note 1 §9, note 2 §1): F1 (bounded frame work, nothing dropped), F2 (no frame awaits
// IPC), T1 (never shown at another revision), T2 (full states, latest wins), T3 (requests in flight bounded: one scene,
// one per drawing), T6 (generations). Real TTRD payloads from the golden corpus, patched to the wanted id, generation
// and revision; a fake host whose answers the test releases; a fake clock.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import type { DecodedDrawing } from './format';
import { CanvasModel, type CanvasHost, type Clock, type ResidentSink } from './model';

const base = (() => {
  const b = readFileSync(new URL('./testdata/synthetic.ttrd', import.meta.url));
  return new Uint8Array(b.buffer, b.byteOffset, b.byteLength).slice();
})();
const gradientFree = (() => {
  // drawing.ttrd has gradient slots; synthetic.ttrd too. A solid-only payload: synthetic with every slot kind set to
  // solid (kind byte of each colour table entry) is enough for the palette test.
  const b = base.slice();
  const v = new DataView(b.buffer);
  let at = 80;
  const slots = v.getUint32(12, true);
  for (let i = 0; i < slots; i++) {
    const len = v.getUint16(at, true);
    b[at + 2] = 0;
    at += 8 + Math.ceil(len / 4) * 4;
  }
  return b;
})();

const ULIDS = [
  '01K7AAAAAAAAAAAAAAAAAAAAAA',
  '01K7BBBBBBBBBBBBBBBBBBBBBB',
  '01K7CCCCCCCCCCCCCCCCCCCCCC',
  '01K7DDDDDDDDDDDDDDDDDDDDDD',
  '01K7EEEEEEEEEEEEEEEEEEEEEE',
];
const id = (i: number) => `dr_${ULIDS[i] ?? ''}`;

function payload(ulid: string, generation: number | bigint, rev: number | bigint, from = base): ArrayBuffer {
  const b = from.slice();
  const v = new DataView(b.buffer);
  v.setBigUint64(16, BigInt(generation), true);
  v.setBigUint64(24, BigInt(rev), true);
  for (let i = 0; i < 26; i++) b[32 + i] = ulid.charCodeAt(i);
  return b.buffer;
}

function scene(
  generation: number | bigint,
  entries: [number, number | bigint][],
  stamp = 1,
  documentRev?: number,
) {
  return {
    generation,
    scene: {
      viewId: 'drawingView',
      ...(documentRev === undefined ? {} : { documentRev }),
      background: [240, 240, 240, 255],
      grid: { visible: false, type: 'normal', overlay: false },
      entries: entries.map(([i, rev]) => ({
        drawingId: id(i),
        rev,
        role: 'current',
        opacity: 1,
        tint: null,
        shade: false,
        outline: false,
      })),
      colours: {},
      paletteStamp: stamp,
    },
  };
}

interface Pending<T> {
  resolve: (v: T) => void;
  reject: (e: unknown) => void;
}

class FakeHost implements CanvasHost {
  scenes: Pending<unknown>[] = [];
  loads: (Pending<ArrayBuffer> & { drawingId: string; bucket: number; outline: boolean })[] = [];
  canvasScene(): Promise<unknown> {
    return new Promise((resolve, reject) => this.scenes.push({ resolve, reject }));
  }
  canvasLoad(drawingId: string, bucket: number, outline: boolean): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => this.loads.push({ resolve, reject, drawingId, bucket, outline }));
  }
}

class Sink implements ResidentSink {
  uploads: { id: string; d: DecodedDrawing }[] = [];
  drops: string[] = [];
  constructor(private readonly onUpload: () => void = () => undefined) {}
  upload(drawingId: string, d: DecodedDrawing) {
    this.uploads.push({ id: drawingId, d });
    this.onUpload();
  }
  drop(drawingId: string) {
    this.drops.push(drawingId);
  }
}

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

function setup(onUpload?: (clock: { t: number }) => void) {
  const clock = { t: 0 };
  const c: Clock = { now: () => clock.t };
  const host = new FakeHost();
  const sink = new Sink(() => onUpload?.(clock));
  const model = new CanvasModel('drawingView', host, sink, c);
  return { clock, host, sink, model };
}

describe('the canvas pull loop', () => {
  it('F2: frames never wait on IPC; a stalled host leaves one scene request in flight', () => {
    const { host, model } = setup();
    for (let i = 0; i < 100; i++) {
      const s = model.frame();
      expect(s.applied).toBe(0);
    }
    expect(host.scenes.length).toBe(1);
    expect(model.inFlight).toBe(1);
  });

  it('T1, T3: a drawing loads at the scene revision; an older payload is discarded and asked for again', async () => {
    const { host, sink, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(5, [[0, 10]]));
    await flush();
    model.frame(); // applies the scene, requests the load
    expect(host.loads.map((l) => l.drawingId)).toEqual([id(0)]);
    model.frame();
    expect(host.loads.length).toBe(1); // one load per drawing in flight
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 5, 10));
    await flush();
    model.frame();
    expect(sink.uploads.map((u) => Number(u.d.rev))).toEqual([10]);
    // A new revision: the old payload answering the new request is never shown.
    model.markDirty();
    model.frame();
    host.scenes[1]?.resolve(scene(5, [[0, 11]]));
    await flush();
    model.frame();
    expect(host.loads.length).toBe(2);
    host.loads[1]?.resolve(payload(ULIDS[0] ?? '', 5, 10));
    await flush();
    model.frame();
    expect(sink.uploads.length).toBe(1);
    expect(model.discarded).toBe(1);
    expect(host.loads.length).toBe(3); // asked for again
    host.loads[2]?.resolve(payload(ULIDS[0] ?? '', 5, 11));
    await flush();
    model.frame();
    expect(sink.uploads.map((u) => Number(u.d.rev))).toEqual([10, 11]);
    expect(model.peakInFlight).toBeLessThanOrEqual(2);
  });

  it('settledVersion follows only when every drawing is resident at its scene revision (C42B-R1-04)', async () => {
    const { host, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(5, [[0, 10]], 1, 40));
    await flush();
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 5, 10));
    await flush();
    model.frame();
    const settled = model.settledVersion;
    expect(settled).toBe(model.version);
    // C43B-R3S-02: the settled scene's document revision goes with it.
    expect(model.settledDocumentRev).toBe(40n);
    // A commit: the new revision's scene arrives first; the old geometry is still the one drawn.
    model.markDirty();
    host.scenes[1]?.resolve(scene(5, [[0, 11]], 1, 41));
    await flush();
    model.frame();
    expect(model.version).toBeGreaterThan(settled);
    expect(model.settledVersion).toBe(settled); // not the committed state yet
    expect(model.settledDocumentRev).toBe(40n); // nor its revision
    host.loads[1]?.resolve(payload(ULIDS[0] ?? '', 5, 11));
    await flush();
    model.frame();
    expect(model.settledVersion).toBe(model.version); // the committed drawing is resident
    expect(model.settledVersion).toBeGreaterThan(settled);
    expect(model.settledDocumentRev).toBe(41n);
  });

  it('a scene differing only in its selection rebuilds the overlay, not the static layers (frame-loop note §7.1)', async () => {
    const { host, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(5, [[0, 10]]));
    await flush();
    model.frame();
    const version = model.version;
    const selected = model.selectionVersion;
    const colours = model.scene?.colours;
    const withSelection = scene(5, [[0, 10]]);
    Object.assign(withSelection.scene, {
      drawingPivot: [1, 2],
      selection: { objects: [[0, 0, 10, 10]], bounds: [0, 0, 10, 10], pivot: [5, 5], points: [] },
    });
    model.markDirty();
    host.scenes[1]?.resolve(withSelection);
    await flush();
    model.frame();
    expect(model.version).toBe(version); // no static redraw
    expect(model.selectionVersion).toBe(selected + 1);
    expect(model.scene?.selection?.bounds).toEqual([0, 0, 10, 10]);
    expect(model.scene?.drawingPivot).toEqual([1, 2]);
    expect(model.scene?.colours).toBe(colours); // the same colours object: no colour re-upload
    // Deselecting: the overlay changes again, the static layers still not.
    model.markDirty();
    host.scenes[2]?.resolve(scene(5, [[0, 10]]));
    await flush();
    model.frame();
    expect(model.version).toBe(version);
    expect(model.selectionVersion).toBe(selected + 2);
    expect(model.scene?.selection).toBeNull();
  });

  it('T1 above 2^53: revisions and generations compare exactly, never rounded (C42B-R1-20)', async () => {
    const { host, sink, model } = setup();
    const big = 9_007_199_254_740_993n; // 2^53 + 1: as a double it would equal 2^53
    const gen = 9_007_199_254_740_995n;
    model.setGeneration(gen);
    model.frame();
    host.scenes[0]?.resolve(scene(gen, [[0, big]]));
    await flush();
    model.frame();
    // A payload one revision older, equal to the scene's revision once rounded to a double: discarded.
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', gen, big - 1n));
    await flush();
    model.frame();
    expect(sink.uploads.length).toBe(0);
    expect(model.discarded).toBe(1);
    // Of a generation that rounds to the same double: discarded too.
    host.loads[1]?.resolve(payload(ULIDS[0] ?? '', gen - 1n, big));
    await flush();
    model.frame();
    expect(sink.uploads.length).toBe(0);
    expect(model.discarded).toBe(2);
    host.loads[2]?.resolve(payload(ULIDS[0] ?? '', gen, big));
    await flush();
    model.frame();
    expect(sink.uploads.map((u) => u.d.rev)).toEqual([big]);
    expect(model.resident.get(id(0))?.rev).toBe(big);
  });

  it('T6: a new generation drops every resident drawing and reloads; an old-generation payload is discarded', async () => {
    const { host, sink, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(
      scene(5, [
        [0, 3],
        [1, 4],
      ]),
    );
    await flush();
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 5, 3));
    host.loads[1]?.resolve(payload(ULIDS[1] ?? '', 5, 4));
    await flush();
    model.frame();
    expect(model.resident.size).toBe(2);
    model.setGeneration(6);
    expect(sink.drops.sort()).toEqual([id(0), id(1)].sort());
    expect(model.resident.size).toBe(0);
    model.frame();
    host.scenes[1]?.resolve(scene(6, [[0, 3]]));
    await flush();
    model.frame();
    expect(host.loads.length).toBe(3);
    host.loads[2]?.resolve(payload(ULIDS[0] ?? '', 5, 3)); // the old generation's bytes
    await flush();
    model.frame();
    expect(model.resident.size).toBe(0);
    expect(model.discarded).toBe(1);
  });

  it('T2: a drawing that leaves the scene is dropped; the latest scene wins', async () => {
    const { host, sink, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(
      scene(1, [
        [0, 1],
        [1, 1],
      ]),
    );
    await flush();
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 1, 1));
    host.loads[1]?.resolve(payload(ULIDS[1] ?? '', 1, 1));
    await flush();
    model.frame();
    model.markDirty();
    model.frame();
    host.scenes[1]?.resolve(scene(1, [[1, 1]]));
    await flush();
    model.frame();
    expect(sink.drops).toEqual([id(0)]);
    expect([...model.resident.keys()]).toEqual([id(1)]);
  });

  it('an event asks for the scene at once, and its answer asks for the drawings at once (frames only apply)', async () => {
    const { host, sink, model } = setup();
    model.setGeneration(1); // the window's generation (the early loads are for the generation the canvas follows)
    model.frame();
    host.scenes[0]?.resolve(scene(1, [[0, 1]]));
    await flush();
    expect(host.loads.length).toBe(1); // asked on the answer's arrival, before any frame
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 1, 1));
    await flush();
    model.frame();
    model.markDirty();
    expect(host.scenes.length).toBe(2); // asked by the event itself
    model.markDirty();
    expect(host.scenes.length).toBe(2); // one scene request in flight at most (T3)
    host.scenes[1]?.resolve(scene(1, [[0, 2]]));
    await flush();
    expect(host.loads.map((l) => l.drawingId)).toEqual([id(0), id(0)]);
    expect(sink.uploads.length).toBe(1); // nothing is applied outside a frame (F2)
  });

  it('an identical scene answer changes nothing (no version bump, so no redraw)', async () => {
    const { host, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(1, [[0, 1]]));
    await flush();
    model.frame();
    const v = model.version;
    model.markDirty();
    model.frame();
    host.scenes[1]?.resolve(scene(1, [[0, 1]]));
    await flush();
    model.frame();
    expect(model.version).toBe(v);
    model.markDirty();
    model.frame();
    host.scenes[2]?.resolve(scene(1, [[0, 2]]));
    await flush();
    model.frame();
    expect(model.version).toBe(v + 1);
  });

  it('F1: a frame applies work within its budget and carries the rest; nothing is dropped', async () => {
    // Each upload costs 3 ms of the fake clock; the budget is 4 ms.
    const { host, sink, model } = setup((clock) => {
      clock.t += 3;
    });
    model.frame();
    host.scenes[0]?.resolve(
      scene(
        1,
        [0, 1, 2, 3, 4].map((i) => [i, 1]),
      ),
    );
    await flush();
    model.frame();
    expect(host.loads.length).toBe(5);
    host.loads.forEach((l, i) => {
      l.resolve(payload(ULIDS[i] ?? '', 1, 1));
    });
    await flush();
    const applied: number[] = [];
    for (let f = 0; f < 10 && sink.uploads.length < 5; f++) {
      const s = model.frame();
      applied.push(s.applied);
      expect(s.spentMs).toBeLessThanOrEqual(model.budgetMs + 3); // at most one item past the budget
    }
    expect(applied).toEqual([2, 2, 1]);
    expect(sink.uploads.map((u) => u.id)).toEqual([0, 1, 2, 3, 4].map(id));
  });

  it('a palette change reloads drawings with gradient or texture slots, not solid-only ones', async () => {
    const { host, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(
      scene(
        1,
        [
          [0, 1],
          [1, 1],
        ],
        7,
      ),
    );
    await flush();
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 1, 1)); // gradient slots
    host.loads[1]?.resolve(payload(ULIDS[1] ?? '', 1, 1, gradientFree));
    await flush();
    model.frame();
    model.markDirty();
    model.frame();
    host.scenes[1]?.resolve(
      scene(
        1,
        [
          [0, 1],
          [1, 1],
        ],
        8,
      ),
    );
    await flush();
    model.frame();
    expect(host.loads.slice(2).map((l) => l.drawingId)).toEqual([id(0)]);
  });

  it('a zoom bucket change reloads at the new bucket at the same revision', async () => {
    const { host, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(1, [[0, 1]]));
    await flush();
    model.frame();
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 1, 1));
    await flush();
    model.frame();
    model.setBucket(2);
    model.frame();
    expect(host.loads[1]?.bucket).toBe(2);
  });

  it('an outline entry is its own resident form (02 §10.2): loaded with outline, a fill payload is not shown for it', async () => {
    const { host, sink, model } = setup();
    model.frame();
    const both = scene(1, [[0, 1]]);
    const first = both.scene.entries[0];
    if (first) both.scene.entries.push({ ...first, outline: true, role: 'onion' });
    host.scenes[0]?.resolve(both);
    await flush();
    model.frame();
    expect(host.loads.map((l) => [l.drawingId, l.outline])).toEqual([
      [id(0), false],
      [id(0), true],
    ]);
    host.loads[0]?.resolve(payload(ULIDS[0] ?? '', 1, 1));
    host.loads[1]?.resolve(payload(ULIDS[0] ?? '', 1, 1)); // no outline flag: not the outline form
    await flush();
    model.frame();
    expect(sink.uploads.map((u) => u.id)).toEqual([id(0)]);
    expect(model.discarded).toBe(1);
    const outline = new Uint8Array(payload(ULIDS[0] ?? '', 1, 1));
    new DataView(outline.buffer).setUint16(58, 1, true);
    host.loads[2]?.resolve(outline.buffer);
    await flush();
    model.frame();
    expect(sink.uploads.map((u) => u.id)).toEqual([id(0), `${id(0)}#outline`]);
  });

  it('a corrupt payload is refused and recorded, never uploaded', async () => {
    const { host, sink, model } = setup();
    model.frame();
    host.scenes[0]?.resolve(scene(1, [[0, 1]]));
    await flush();
    model.frame();
    host.loads[0]?.resolve(new Uint8Array([1, 2, 3]).buffer);
    await flush();
    model.frame();
    expect(sink.uploads.length).toBe(0);
    expect(model.failures[0]).toMatch(/V1/);
  });
});
