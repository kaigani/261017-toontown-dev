// M5 C5.1b, plan §6.2: the Timeline window's budgets on the workstation (`toontown --timing-check <out>
// --timing-check-budgets`, a diagnostics build; the kernel's budgets are `crates/executor/examples/c51b_budgets.rs`).
// The scene is the plan's, 1,000 rows over 9,999 frames and 200 pegs keyed every 4 frames, keyed where the measured
// views can show keys (frames 1-400), built through commands. The
// panel's draw probe (TimelinePanel.tsx, diagnostics builds only) records each draw's work and what it showed.
//
// - work per frame while scrolling, zooming and dragging: the draw's work p95 <= 8 ms, and at most 1 % of the frame
//   intervals over twice the control's median (the control: the same frames with nothing changing, D-33);
// - a click on a cell to the selection drawn, and a wheel step to the scrolled repaint: p95 <= two control frames;
// - playback at 24 fps (Force Frame Rate off): every frame of the range presented in order; the interval p95 <= 1.5
//   periods.
//
// Not product code: absent from the default build.
import type { HostApi } from '../ipc';
import { LAYOUT } from '../timeline/geometry';

type Json = Record<string, unknown>;
interface Draw {
  readonly at: number;
  readonly ms: number;
  readonly scrollTop: number;
  readonly firstFrame: number;
  readonly zoomLevel: number;
  readonly currentFrame: number;
  readonly cells: string;
}

const frame = () =>
  new Promise<number>((r) => {
    requestAnimationFrame(r);
  });
const sleep = (ms: number) =>
  new Promise<void>((r) => {
    setTimeout(r, ms);
  });

function quantile(values: readonly number[], q: number): number {
  if (values.length === 0) return Number.NaN;
  const s = [...values].sort((a, b) => a - b);
  return s[Math.round((s.length - 1) * q)] ?? Number.NaN;
}

function el(id: string): HTMLElement | null {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  return e instanceof HTMLElement ? e : null;
}

async function intervals(n: number, step: (i: number) => void | Promise<void>): Promise<number[]> {
  const out: number[] = [];
  let last = await frame();
  for (let i = 0; i < n; i++) {
    await step(i);
    const now = await frame();
    out.push(now - last);
    last = now;
  }
  return out;
}

export async function runTimelineBudgets(host: HostApi, workDir: string | null): Promise<Json> {
  const engine = host.engine;
  const run = async (command: string, params: Json): Promise<Json> => {
    const env = await engine.run(command, params).done;
    if (env.status !== 'ok')
      throw new Error(`${command}: ${env.error?.code ?? 'error'} ${env.error?.message ?? ''}`);
    return (env.result ?? {}) as Json;
  };
  const draws: Draw[] = [];
  (window as unknown as { __ttTimelineDraws?: Draw[] }).__ttTimelineDraws = draws;
  const built = performance.now();

  // The scene: 800 drawings (the first anchors 9,999 frames), 200 pegs keyed every 4 frames.
  await run('project.create', { name: 'Budgets', onUnsaved: 'discard' });
  // Hidden while the scene is built: a shown Timeline reads its rows again after each of the 2,500 key additions.
  await run('window.panel.set', { panelId: 'timeline', visible: false });
  // Saved and opened again now and then while it is built: each step is an undo step holding the curves as they were,
  // so 2,500 key additions in one history grow with their square. Opening the saved project starts a new history.
  if (workDir === null) throw new Error('no folder to save the scene in');
  const path = `${workDir}/tt-timeline-budgets.animpkg`;
  // Saved, the save's job waited for, then opened (an open that saves first reads the file before its own save).
  const waited = async (command: string, params: Json) => {
    const out = await run(command, params);
    if (typeof out['jobId'] === 'string') await run('job.wait', { jobId: out['jobId'], timeoutMs: 600_000 });
  };
  const reopen = async () => {
    await waited('project.save', {});
    // Closed first: an open of the project's own path finds its lock held and opens it read-only.
    await run('project.close', { onUnsaved: 'fail' });
    await waited('project.open', { path, onUnsaved: 'fail' });
  };
  await waited('project.saveAs', { path, onExists: 'overwrite' });
  const ids = (r: Json) => (Array.isArray(r['elementIds']) ? (r['elementIds'] as string[]) : []);
  const drawings: string[] = [];
  for (let b = 0; b < 9; b++)
    drawings.push(...ids(await run('element.create', { type: 'drawing', count: b === 8 ? 8 : 99 })));
  for (const [i, id] of drawings.entries())
    await run('element.drawing.create', {
      elementId: id,
      name: '1',
      exposeFrom: 1,
      exposeTo: i === 0 ? 9999 : 200 + (i % 40) * 5,
    });
  await reopen();
  // 200 pegs keyed every 4 frames where the measured views can show keys: frames 1-400 (the zoom levels stepped show
  // about 110 frames at most). Keys beyond them change no draw: building them through the command path costs an undo
  // step per frame that holds every curve (quadratic in the keys).
  const pegs: string[] = [];
  for (let b = 0; b < 3; b++)
    pegs.push(...ids(await run('element.create', { type: 'peg', count: b === 2 ? 2 : 99 })));
  for (let f = 1, k = 0; f <= 400; f += 4, k++) {
    await run('anim.key.add', { nodeIds: pegs, frame: f });
    if (k % 25 === 24) await reopen();
  }
  await reopen();
  const buildSeconds = (performance.now() - built) / 1000;

  await run('window.panel.set', { panelId: 'timeline', visible: true, focus: true });
  await run('timeline.view.set', { zoomLevel: 6, firstFrame: 1 });
  for (let i = 0; i < 200 && !el('timeline.row.' + (pegs[0] ?? '')); i++) await sleep(50);
  await sleep(500);
  const list = el('timeline.layers');
  const tracks = el('timeline.tracks');
  if (!list || !tracks) throw new Error('the Timeline is not shown');
  // New elements go to the top: the rows' order from the kernel.
  const order = (
    ((await run('timeline.rows.list', { from: 1, to: 1 }))['rows'] as Json[] | undefined) ?? []
  ).map((r) => r['elementId']);
  const pegRow = Math.max(0, order.indexOf(pegs[0]));
  const drawingRow = Math.max(0, order.indexOf(drawings[0]));

  // The control: frames with nothing changing.
  const control = await intervals(120, () => undefined);
  const controlMedian = quantile(control, 0.5);
  const work = (from: number) => draws.filter((d) => d.at >= from).map((d) => d.ms);
  const over = (xs: readonly number[]) =>
    xs.filter((x) => x > 2 * controlMedian).length / Math.max(1, xs.length);

  // Scrolling: one row a frame.
  let since = performance.now();
  const scrollIntervals = await intervals(120, (i) => {
    list.scrollTop = (i + 1) * LAYOUT.rowHeight;
    list.dispatchEvent(new Event('scroll'));
  });
  const scrollWork = work(since);
  // Zooming: the view's zoom stepped each frame (the kernel answers; the panel draws the new ladder step).
  since = performance.now();
  const zoomIntervals = await intervals(60, (i) => {
    void run('timeline.view.set', { zoomLevel: 4 + (i % 5) });
  });
  await sleep(300);
  const zoomWork = work(since);
  await run('timeline.view.set', { zoomLevel: 6, firstFrame: 1 });
  list.scrollTop = 0;
  list.dispatchEvent(new Event('scroll'));
  await sleep(300);
  // Dragging: a peg's bar body dragged along, a move a frame (the preview drawn each time), then Escape.
  const z = LAYOUT.zoomLevels[6] ?? { cellWidth: 16, framesPerCell: 1 };
  const box = tracks.getBoundingClientRect();
  list.scrollTop = pegRow * LAYOUT.rowHeight;
  list.dispatchEvent(new Event('scroll'));
  await sleep(300);
  const fire = (type: string, x: number, buttons: number) => {
    tracks.dispatchEvent(
      new PointerEvent(type, {
        bubbles: true,
        clientX: box.left + x,
        clientY: box.top + LAYOUT.rowHeight / 2,
        button: 0,
        buttons,
        pointerId: 43,
        pointerType: 'mouse',
        isPrimary: true,
      }),
    );
  };
  fire('pointerdown', 20.5 * z.cellWidth, 1);
  await sleep(200);
  since = performance.now();
  const dragIntervals = await intervals(120, (i) => {
    fire('pointermove', 20.5 * z.cellWidth + (i % 40) * 3, 1);
  });
  const dragWork = work(since);
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  fire('pointerup', 20.5 * z.cellWidth, 0);
  await sleep(300);

  // A click on a cell to the selection drawn (30 samples, alternating frames on the first drawing's row).
  list.scrollTop = drawingRow * LAYOUT.rowHeight;
  list.dispatchEvent(new Event('scroll'));
  await sleep(300);
  const clicks: number[] = [];
  for (let i = 0; i < 30; i++) {
    const f = 3 + (i % 2) * 4;
    const x = (f - 1) * z.cellWidth + z.cellWidth / 2;
    const want = `:${f}-${f}`;
    const t0 = performance.now();
    fire('pointerdown', x, 1);
    fire('pointerup', x, 0);
    for (let k = 0; k < 120; k++) {
      const hit = draws.find((d) => d.at >= t0 && d.cells.endsWith(want));
      if (hit) {
        clicks.push(hit.at + hit.ms - t0);
        break;
      }
      await frame();
    }
    await sleep(100);
  }
  // A wheel step to the scrolled repaint (30 samples).
  const wheels: number[] = [];
  const boxEl = tracks.parentElement?.parentElement ?? tracks;
  for (let i = 0; i < 30; i++) {
    const before = list.scrollTop;
    const t0 = performance.now();
    boxEl.dispatchEvent(new WheelEvent('wheel', { bubbles: true, deltaY: i % 2 === 0 ? 66 : -66 }));
    for (let k = 0; k < 120; k++) {
      const hit = draws.find((d) => d.at >= t0 && d.scrollTop !== before);
      if (hit) {
        wheels.push(hit.at + hit.ms - t0);
        break;
      }
      await frame();
    }
    await sleep(100);
  }

  // Playback at 24 fps over a 48-frame range, Loop off: every frame presented, in order.
  await run('playback.rate.set', { fps: 24 });
  await run('playback.forceRate.set', { enabled: false });
  await run('playback.loop.set', { enabled: false });
  await run('timeline.playbackRange.set', { mode: 'free', start: 1, end: 48 });
  await run('timeline.currentFrame.set', { frame: 1 });
  await sleep(300);
  since = performance.now();
  await run('playback.start', {});
  for (let k = 0; k < 400; k++) {
    const state = await run('playback.state.get', {});
    if (state['playing'] !== true && performance.now() - since > 500) break;
    await sleep(25);
  }
  await sleep(200);
  const presented: { frame: number; at: number }[] = [];
  for (const d of draws.filter((x) => x.at >= since)) {
    if (presented.at(-1)?.frame !== d.currentFrame) presented.push({ frame: d.currentFrame, at: d.at });
  }
  const frames = presented.map((p) => p.frame);
  const missing: number[] = [];
  for (let f = 1; f <= 48; f++) if (!frames.includes(f)) missing.push(f);
  const inOrder = frames.every((f, i) => i === 0 || f >= (frames[i - 1] ?? 0));
  const playIntervals = presented.slice(1).map((p, i) => p.at - (presented[i]?.at ?? p.at));
  const period = 1000 / 24;

  const p95 = (xs: readonly number[]) => quantile(xs, 0.95);
  const rows: Json[] = [
    {
      name: 'work per frame while scrolling',
      drawP95Ms: p95(scrollWork),
      draws: scrollWork.length,
      overTwiceControl: over(scrollIntervals),
      ok: p95(scrollWork) <= 8 && over(scrollIntervals) <= 0.01,
    },
    {
      name: 'work per frame while zooming',
      drawP95Ms: p95(zoomWork),
      draws: zoomWork.length,
      overTwiceControl: over(zoomIntervals),
      ok: p95(zoomWork) <= 8 && over(zoomIntervals) <= 0.01,
    },
    {
      name: 'work per frame while dragging',
      drawP95Ms: p95(dragWork),
      draws: dragWork.length,
      overTwiceControl: over(dragIntervals),
      ok: p95(dragWork) <= 8 && over(dragIntervals) <= 0.01,
    },
    {
      name: 'a click on a cell to the selection drawn',
      p95Ms: p95(clicks),
      samples: clicks.length,
      boundMs: 2 * controlMedian,
      ok: clicks.length === 30 && p95(clicks) <= 2 * controlMedian,
    },
    {
      name: 'a wheel step to the scrolled repaint',
      p95Ms: p95(wheels),
      samples: wheels.length,
      boundMs: 2 * controlMedian,
      ok: wheels.length === 30 && p95(wheels) <= 2 * controlMedian,
    },
    {
      name: 'playback at 24 fps: every frame presented, in order',
      presented: frames.length,
      missing,
      inOrder,
      intervalP95Ms: p95(playIntervals),
      boundMs: 1.5 * period,
      ok: missing.length === 0 && inOrder && p95(playIntervals) <= 1.5 * period,
    },
  ];
  return {
    ok: rows.every((r) => r['ok'] === true),
    scene: {
      drawings: drawings.length,
      pegs: pegs.length,
      keysPerPeg: 100,
      keyedFrames: '1-400 every 4',
      buildSeconds,
    },
    control: { frames: control.length, medianMs: controlMedian },
    rows,
  };
}
