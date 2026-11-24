// M5 C5.1b: the Timeline's three-way check through the window's event path (plan §9; `toontown --timing-check`, a
// diagnostics build only; desktop/src-tauri/src/timing_check.rs). The host has opened the kernel's sample document.
// For each gesture of the plan's §6.3 the window:
//
// 1. starts from the sample document (reopened, its changes discarded), the view at zoom 6 from frame 1, the layer list
//    scrolled to the case's first row;
// 2. makes the gesture through its own event path: pointer events dispatched on the Timeline's canvas or layer list
//    (the handlers a person's pointer reaches), or, with `external`, waits while a script makes it with real OS input
//    at the points the window publishes (`window.__ttTimingCheck`);
// 3. records the press's layout and the release's samples, and before the release is sent replays them through the
//    kernel interpreter (the host's replay, nothing committed: the state the release then commits from);
// 4. compares the replay's plan with the plan its release committed, exactly; then the state the commit made with the
//    state the plan's commands make run directly from the same start (an undone commit for a document change, the
//    selection, current element and frame and playback range restored for a UI change), exactly.
//
// The report goes to the host, which writes it and exits. Not product code: absent from the default build.
import type { HostApi, TimingGestureAnswer, TimingGestureRequest } from '../ipc';
import { LAYOUT, platformOf, type Platform } from '../timeline/geometry';
import { runTimelineBudgets } from './timelineBudgets';

type Json = Record<string, unknown>;
interface Point {
  readonly x: number;
  readonly y: number;
}
type Surface = 'ruler' | 'tracks' | 'layers';
type Role = 'extend' | 'toggle' | 'singleCell' | 'copy' | 'link';

interface Case {
  readonly name: string;
  readonly spec: string;
  readonly surface: Surface;
  /** The command the plan must hold (the case reaches the gesture it names). */
  readonly command: string | null;
  /** Commands run before the gesture (each from the reopened sample document). */
  readonly setup?: (ids: Ids) => [string, Json][];
  /** The layer list's first shown row (so the case's rows are in view). */
  readonly scrollRow?: number;
  /** The points in the surface's own pixels, the layer list scrolled as `scrollRow` says. */
  readonly points: (g: Geometry) => Point[];
  readonly roles?: readonly Role[];
  /** Escape before the release: nothing may be sent or change. */
  readonly escape?: boolean;
  /** The refusal the plan's command must answer, from the window and run directly; nothing may change (E-50 item 2). */
  readonly refused?: string;
  /** Commands run after the case (a preference the case set: the sample document's reopening keeps preferences). */
  readonly teardown?: (ids: Ids) => [string, Json][];
}

type Ids = Readonly<Record<string, string>>;

interface Geometry {
  /** A row's index in the shown rows, by name. */
  row(name: string): number;
  /** The x of a frame's cell centre. */
  frame(f: number): number;
  /** The x of a frame's left edge. */
  edge(f: number): number;
  /** A y inside a row: `at` 'top' (upper quarter), 'upper' (upper half), 'middle', 'lower' (lower half). */
  y(name: string, at: 'top' | 'upper' | 'middle' | 'lower'): number;
}

const ZOOM = 6;
/** A press on a row's name, past its disclosure and visibility controls (they keep the press to themselves). */
const NAME_X = 120;
/** The zoom level's cell (zoom 6: 16 pixels, one frame per cell). */
const CELL = LAYOUT.zoomLevels[ZOOM] ?? { cellWidth: 16, framesPerCell: 1 };
const RULER_Y = 12;
const CASES: readonly Case[] = [
  {
    name: 'ruler: a click sets the current frame',
    spec: '04 line 612; plan §6.3 row 1',
    surface: 'ruler',
    command: 'timeline.currentFrame.set',
    points: (g) => [
      { x: g.frame(7), y: RULER_Y },
      { x: g.frame(7), y: RULER_Y },
    ],
  },
  {
    name: 'ruler: a drag scrubs',
    spec: '04 line 612; plan §6.3 rows 1-2',
    surface: 'ruler',
    command: 'timeline.currentFrame.set',
    points: (g) => [
      { x: g.frame(3), y: RULER_Y },
      { x: g.frame(6), y: RULER_Y },
      { x: g.frame(9), y: RULER_Y },
    ],
  },
  {
    name: 'ruler: a free range’s start marker dragged',
    spec: '04 line 669; plan §6.3 row 3',
    surface: 'ruler',
    command: 'timeline.playbackRange.set',
    setup: () => [['timeline.playbackRange.set', { mode: 'free', start: 5, end: 30 }]],
    points: (g) => [
      { x: g.frame(5), y: RULER_Y },
      { x: g.frame(10), y: RULER_Y },
    ],
  },
  {
    name: 'layer list: a click on a row name selects it',
    scrollRow: 2,
    spec: '04 §9.3; plan §6.3 row 4',
    surface: 'layers',
    command: 'element.select',
    points: (g) => [
      { x: NAME_X, y: g.y('Background', 'middle') },
      { x: NAME_X, y: g.y('Background', 'middle') },
    ],
  },
  {
    name: 'layer list: extend + click',
    scrollRow: 4,
    spec: '04 §16; plan §6.3 row 4',
    surface: 'layers',
    command: 'element.select',
    setup: (ids) => [['element.select', { elementIds: [ids['Background']] }]],
    roles: ['extend'],
    points: (g) => [
      { x: NAME_X, y: g.y('Dialogue', 'middle') },
      { x: NAME_X, y: g.y('Dialogue', 'middle') },
    ],
  },
  {
    name: 'layer list: toggle + click',
    scrollRow: 3,
    spec: '04 §16; plan §6.3 row 4',
    surface: 'layers',
    command: 'element.select',
    setup: (ids) => [['element.select', { elementIds: [ids['Background']] }]],
    roles: ['toggle'],
    points: (g) => [
      { x: NAME_X, y: g.y('Notes', 'middle') },
      { x: NAME_X, y: g.y('Notes', 'middle') },
    ],
  },
  {
    name: 'tracks: a click on a cell selects it',
    scrollRow: 1,
    spec: '04 §11.1; plan §6.3 row 5',
    surface: 'tracks',
    command: 'cell.select',
    points: (g) => [
      { x: g.frame(4), y: g.y('Bob', 'lower') },
      { x: g.frame(4), y: g.y('Bob', 'lower') },
    ],
  },
  {
    name: 'tracks: a drag across cells selects them',
    scrollRow: 1,
    spec: '04 §11.1; plan §6.3 row 5',
    surface: 'tracks',
    command: 'cell.select',
    points: (g) => [
      { x: g.frame(3), y: g.y('Bob', 'lower') },
      { x: g.frame(5), y: g.y('Bob', 'lower') },
      { x: g.frame(7), y: g.y('Bob', 'lower') },
    ],
  },
  {
    name: 'layer list: a row dropped between rows of its level reorders',
    scrollRow: 1,
    spec: '04 line 478; plan §6.3 row 6',
    surface: 'layers',
    command: 'element.order.set',
    points: (g) => [
      { x: NAME_X, y: g.y('Background', 'middle') },
      { x: NAME_X, y: g.y('Background', 'middle') - 12 },
      { x: NAME_X, y: g.y('Bob-P', 'top') },
    ],
  },
  {
    name: 'layer list: a row dropped onto a peg row is attached',
    scrollRow: 1,
    spec: '04 §9.2 line 487; plan §6.3 row 7',
    surface: 'layers',
    command: 'element.parent.set',
    points: (g) => [
      { x: NAME_X, y: g.y('Background', 'middle') },
      { x: NAME_X, y: g.y('Background', 'middle') - 12 },
      { x: NAME_X, y: g.y('Bob-P', 'middle') },
    ],
  },
  {
    name: 'layer list: a row dropped between rows of another level',
    scrollRow: 1,
    spec: '04 §9.2; plan §6.3 row 7',
    surface: 'layers',
    command: 'element.parent.set',
    points: (g) => [
      { x: NAME_X, y: g.y('Background', 'middle') },
      { x: NAME_X, y: g.y('Background', 'middle') - 12 },
      { x: NAME_X, y: g.y('Bob', 'top') },
    ],
  },
  {
    name: 'tracks: a bar body dragged',
    spec: '04 line 717; plan §6.3 row 8',
    surface: 'tracks',
    command: 'element.timing.set',
    scrollRow: 13,
    points: (g) => [
      { x: g.frame(20), y: g.y('Fade', 'middle') },
      { x: g.frame(21), y: g.y('Fade', 'middle') },
      { x: g.frame(23), y: g.y('Fade', 'middle') },
    ],
  },
  {
    name: 'tracks: a bar’s end dragged',
    spec: '04 line 718; plan §6.3 row 9',
    surface: 'tracks',
    command: 'element.timing.set',
    scrollRow: 13,
    setup: (ids) => [['element.timing.set', { elementIds: [ids['Fade']], duration: 20 }]],
    points: (g) => [
      { x: g.edge(21) - 2, y: g.y('Fade', 'lower') },
      { x: g.edge(21) - 2 + 2 * CELL.cellWidth, y: g.y('Fade', 'lower') },
      { x: g.edge(21) - 2 + 6 * CELL.cellWidth, y: g.y('Fade', 'lower') },
    ],
  },
  {
    name: 'tracks: a bar’s start dragged',
    spec: '04 line 719; plan §6.3 row 10',
    surface: 'tracks',
    command: 'element.timing.set',
    scrollRow: 13,
    points: (g) => [
      { x: g.edge(1) + 2, y: g.y('Fade', 'lower') },
      { x: g.edge(1) + 2 + 2 * CELL.cellWidth, y: g.y('Fade', 'lower') },
      { x: g.edge(1) + 2 + 4 * CELL.cellWidth, y: g.y('Fade', 'lower') },
    ],
  },
  {
    name: 'tracks: the loop handle dragged',
    spec: '04 line 720; plan §6.3 row 11',
    surface: 'tracks',
    command: 'element.timing.set',
    scrollRow: 13,
    setup: (ids) => [['element.timing.set', { elementIds: [ids['Fade']], duration: 12 }]],
    points: (g) => [
      { x: g.edge(13), y: g.y('Fade', 'upper') },
      { x: g.edge(19), y: g.y('Fade', 'upper') },
      { x: g.edge(25), y: g.y('Fade', 'upper') },
    ],
  },
  {
    name: 'tracks: a keyframe symbol dragged',
    scrollRow: 1,
    spec: '04 line 621; plan §6.3 row 12',
    surface: 'tracks',
    command: 'anim.key.move',
    points: (g) => [
      { x: g.frame(1), y: g.y('Bob-P', 'middle') },
      { x: g.frame(3), y: g.y('Bob-P', 'middle') },
      { x: g.frame(5), y: g.y('Bob-P', 'middle') },
    ],
  },
  ...(['move', 'copy', 'link'] as const).map((mode): Case => ({
    name: `tracks: selected cells dragged (${mode})`,
    spec: '04 §11.13; plan §6.3 row 13',
    surface: 'tracks',
    command: 'cell.range.move',
    scrollRow: 1,
    setup: (ids) => [['cell.select', { elementIds: [ids['Bob']], from: 2, to: 3 }]],
    ...(mode === 'move' ? {} : { roles: [mode] }),
    points: (g) => [
      { x: g.frame(2), y: g.y('Bob', 'lower') },
      { x: g.frame(5), y: g.y('Bob', 'lower') },
      { x: g.frame(14), y: g.y('Bob', 'lower') },
    ],
  })),
  // Ruling E-50 item 2 (04 lines 571-572): a locked element's timing and exposure still change in the Timeline; the
  // drawing lock refuses a move that overwrites the cells showing that drawing (04 §13.12, §13.16).
  {
    name: 'tracks: a locked peg’s bar body dragged',
    spec: '04 lines 571-572, 717; ruling E-50 item 2',
    surface: 'tracks',
    command: 'element.timing.set',
    scrollRow: 1,
    setup: (ids) => [['element.lock.set', { elementIds: [ids['Bob-P']], locked: true }]],
    points: (g) => [
      { x: g.frame(20), y: g.y('Bob-P', 'middle') },
      { x: g.frame(21), y: g.y('Bob-P', 'middle') },
      { x: g.frame(23), y: g.y('Bob-P', 'middle') },
    ],
  },
  {
    name: 'tracks: a locked drawing element’s selected cells dragged',
    spec: '04 lines 571-572, §11.13; ruling E-50 item 2',
    surface: 'tracks',
    command: 'cell.range.move',
    scrollRow: 1,
    setup: (ids) => [
      ['element.lock.set', { elementIds: [ids['Bob']], locked: true }],
      ['cell.select', { elementIds: [ids['Bob']], from: 2, to: 3 }],
    ],
    points: (g) => [
      { x: g.frame(2), y: g.y('Bob', 'lower') },
      { x: g.frame(5), y: g.y('Bob', 'lower') },
      { x: g.frame(14), y: g.y('Bob', 'lower') },
    ],
  },
  {
    name: 'tracks: cells dragged over a locked drawing are refused',
    spec: '04 §13.12, §13.16 (drawing.locked); ruling E-50 item 2',
    surface: 'tracks',
    command: 'cell.range.move',
    refused: 'drawing.locked',
    scrollRow: 1,
    // Bob's drawing at frame 10 locked; Drag and Drop with Exposure off overwrites the cells it lands on.
    setup: (ids) => [
      ['app.context.set', { elementId: ids['Bob'], frame: 10 }],
      ['draw.lock.set', { locked: true }],
      ['timeline.transferOptions.set', { operation: 'dragDrop', options: { exposure: false } }],
      ['cell.select', { elementIds: [ids['Bob']], from: 2, to: 3 }],
    ],
    teardown: () => [
      ['timeline.transferOptions.set', { operation: 'dragDrop', options: { exposure: true } }],
    ],
    points: (g) => [
      { x: g.frame(2), y: g.y('Bob', 'lower') },
      { x: g.frame(6), y: g.y('Bob', 'lower') },
      { x: g.frame(10), y: g.y('Bob', 'lower') },
    ],
  },
  {
    name: 'tracks: Escape during a drag sends and changes nothing',
    spec: '01 §6.1 line 495; plan §6.3 last row',
    surface: 'tracks',
    command: null,
    scrollRow: 13,
    escape: true,
    points: (g) => [
      { x: g.frame(20), y: g.y('Fade', 'middle') },
      { x: g.frame(24), y: g.y('Fade', 'middle') },
      { x: g.frame(28), y: g.y('Fade', 'middle') },
    ],
  },
];

interface Recording {
  layout?: unknown;
  samples?: unknown;
  replay?: unknown;
  answer?: TimingGestureAnswer;
  /** The press's answer and an abort, when one came (a probe's evidence). */
  pressAnswer?: unknown;
  aborted?: boolean;
  /** The UI state the release started from (restored before a UI plan's commands run directly). */
  before?: UiState;
  released: Promise<void>;
  resolve: () => void;
}

interface UiState {
  readonly timelineIds: readonly string[];
  readonly element: string | null;
  readonly frame: number;
  readonly range: Json;
}

const sleep = (ms: number) =>
  new Promise<void>((r) => {
    setTimeout(r, ms);
  });

const SURFACE_ID: Record<Surface, string> = {
  ruler: 'timeline.ruler',
  tracks: 'timeline.tracks',
  layers: 'timeline.layers',
};

function el(id: string): HTMLElement | null {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  return e instanceof HTMLElement ? e : null;
}

/** Deep copy without the revision numbers (a commit and its direct run reach the same state at other revisions). */
function strip(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(strip);
  if (v && typeof v === 'object') {
    const out: Json = {};
    for (const [k, x] of Object.entries(v as Json)) {
      if (k === 'rev' || k === 'documentRev' || k === 'revision') continue;
      out[k] = strip(x);
    }
    return out;
  }
  return v;
}

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(strip(a)) === JSON.stringify(strip(b));
}

export async function maybeRunTimingCheck(host: HostApi): Promise<boolean> {
  const plan = await host.timingCheckPlan();
  if (!plan) return false;
  if (plan.budgets) {
    // Plan §6.2's window budgets instead of the three-way rows.
    const budgets = await runTimelineBudgets(host, plan.workDir).catch((e: unknown) => ({
      ok: false,
      error: e instanceof Error ? e.message : String(e),
    }));
    await host.timingCheckReport({ ok: budgets.ok === true, budgets });
    return true;
  }
  const engine = host.engine;
  const run = async (command: string, params: Json): Promise<Json> => {
    const env = await engine.run(command, params).done;
    if (env.status !== 'ok')
      throw new Error(`${command}: ${env.error?.code ?? 'error'} ${env.error?.message ?? ''}`);
    return (env.result ?? {}) as Json;
  };
  const platform: Platform = platformOf(navigator.userAgent);
  const modifiers = LAYOUT.modifiers[platform];
  const keysOf = (roles: readonly Role[] = []): string[] => [
    ...new Set(roles.flatMap((r) => [...(modifiers?.[r] ?? [])])),
  ];

  // The recorder: the panel's gesture controller calls `engine.timingGesture` at each phase.
  let rec: Recording | null = null;
  const original = engine.timingGesture.bind(engine);
  const uiState = async (): Promise<UiState> => {
    const sel = await run('edit.selection.get', { context: 'timeline' });
    const app = await run('app.state.get', {});
    const element = app['element'] as Json | null | undefined;
    return {
      timelineIds: Array.isArray(sel['ids']) ? (sel['ids'] as string[]) : [],
      element: typeof element?.['id'] === 'string' ? element['id'] : null,
      frame: typeof app['frame'] === 'number' ? app['frame'] : 1,
      range: await run('timeline.playbackRange.get', {}),
    };
  };
  (engine as { timingGesture: typeof engine.timingGesture }).timingGesture = async (
    request: TimingGestureRequest,
  ) => {
    const r = rec;
    if (r && request.phase === 'press') r.layout = request.layout;
    if (r && request.phase === 'abort') r.aborted = true;
    if (r && request.phase === 'release') {
      r.samples = request.samples;
      r.before = await uiState();
      r.replay = await host
        .timingReplay({ layout: r.layout, samples: request.samples })
        .catch((e: unknown) => ({ error: String(e) }));
    }
    const answer = await original(request);
    if (r && request.phase === 'press')
      r.pressAnswer = answer.ok ? { ok: true } : { ok: false, code: answer.error.code };
    if (r && request.phase === 'release') {
      r.answer = answer;
      r.resolve();
    }
    return answer;
  };

  const state = async () => {
    const history = await run('edit.history.get', { limit: 1000 });
    const entries = Array.isArray(history['entries']) ? (history['entries'] as Json[]) : [];
    const ui = await uiState();
    return {
      rows: await run('timeline.rows.list', {}),
      cells: await run('cell.selection.get', {}),
      ui,
      history: { position: history['position'], labels: entries.map((e) => e['label']) },
    };
  };

  const rows: Json[] = [];
  const external = plan.external;
  const shared = window as unknown as { __ttTimingCheck?: Json };
  const only = plan.only;
  for (const [index, c] of CASES.entries()) {
    if (only !== null && !c.name.includes(only)) continue;
    const row: Json = { name: c.name, spec: c.spec, surface: c.surface };
    // A probe's evidence for a dropped gesture: blurs of the window during the case.
    let blurs = 0;
    const onBlurAll = () => {
      blurs += 1;
    };
    window.addEventListener('blur', onBlurAll);
    const seen: string[] = [];
    const onAny = (e: Event) => {
      seen.push(e.type);
    };
    for (const t of ['pointercancel', 'lostpointercapture', 'dragstart'])
      document.addEventListener(t, onAny, true);
    const selectedAtStart = String(document.getSelection() ?? '');
    let caseIds: Ids = {};
    try {
      await host.timingCheckReset();
      await sleep(300);
      const listed = await run('element.list', {});
      const ids: Record<string, string> = {};
      caseIds = ids;
      for (const e of (listed['elements'] as Json[] | undefined) ?? []) {
        if (typeof e['name'] === 'string' && typeof e['id'] === 'string') ids[e['name']] = e['id'];
      }
      await run('window.panel.set', { panelId: 'timeline', visible: true, focus: true });
      for (const [command, params] of c.setup?.(ids) ?? []) await run(command, params);
      await run('timeline.view.set', { zoomLevel: ZOOM, firstFrame: 1 });
      // The panel shows the view and the rows (a wait for a state; the bound guards a hang).
      for (let i = 0; i < 100; i++) {
        const t = el('timeline.tracks');
        if (
          t?.dataset['ttZoom'] === String(ZOOM) &&
          t.dataset['ttFirstFrame'] === '1' &&
          el(`timeline.row.${ids['Bob'] ?? ''}`)
        )
          break;
        await sleep(50);
      }
      const list = el('timeline.layers');
      if (list) {
        list.scrollTop = (c.scrollRow ?? 0) * LAYOUT.rowHeight;
        list.dispatchEvent(new Event('scroll'));
      }
      await sleep(250);
      const shownRows = ((await run('timeline.rows.list', {}))['rows'] as Json[] | undefined) ?? [];
      const order = shownRows.map((r) => r['name'] as string);
      const scroll = list?.scrollTop ?? 0;
      const z = CELL;
      const g: Geometry = {
        row: (name) => order.indexOf(name),
        frame: (f) => (f - 1) * (z.cellWidth / z.framesPerCell) + z.cellWidth / 2,
        edge: (f) => (f - 1) * (z.cellWidth / z.framesPerCell),
        y: (name, at) => {
          const top = order.indexOf(name) * LAYOUT.rowHeight - scroll;
          const h = LAYOUT.rowHeight;
          return top + (at === 'top' ? 2 : at === 'upper' ? 5 : at === 'middle' ? h / 2 : h - 6);
        },
      };
      const surface = el(SURFACE_ID[c.surface]);
      if (!surface) throw new Error(`no ${SURFACE_ID[c.surface]}`);
      const points = c.points(g);
      const keys = keysOf(c.roles);
      const before = await state();
      let resolve: () => void = () => undefined;
      const released = new Promise<void>((r) => {
        resolve = r;
      });
      rec = { released, resolve };
      // A probe's evidence for a dropped gesture: the layer list replaced during the case (a remount).
      const listAtStart = el('timeline.layers');
      row['focusedAtStart'] = document.hasFocus();
      const box = surface.getBoundingClientRect();
      if (external) {
        // A script makes the gesture with real input at these viewport points (CSS pixels), then waits for `done`.
        shared.__ttTimingCheck = {
          index,
          name: c.name,
          points: points.map((p) => ({ x: box.left + p.x, y: box.top + p.y })),
          // What a real press there reaches (a probe's evidence when the press does not reach the surface).
          under: points.map((p) => {
            const hit = document.elementFromPoint(box.left + p.x, box.top + p.y);
            return hit instanceof HTMLElement ? (hit.dataset['ttId'] ?? hit.tagName) : null;
          }),
          modifiers: keys,
          escape: c.escape === true,
          done: index - 1,
        };
      } else {
        const fire = (type: string, p: Point, buttons: number) => {
          surface.dispatchEvent(
            new PointerEvent(type, {
              bubbles: true,
              cancelable: true,
              composed: true,
              clientX: box.left + p.x,
              clientY: box.top + p.y,
              button: 0,
              buttons,
              pointerId: 41,
              pointerType: 'mouse',
              isPrimary: true,
              shiftKey: keys.includes('shift'),
              ctrlKey: keys.includes('ctrl'),
              altKey: keys.includes('alt'),
              metaKey: keys.includes('meta'),
            }),
          );
        };
        const [first, ...rest] = points;
        if (!first) throw new Error('no points');
        fire('pointerdown', first, 1);
        await sleep(60);
        for (const p of rest.slice(0, -1)) {
          fire('pointermove', p, 1);
          await sleep(60);
        }
        const last = rest[rest.length - 1] ?? first;
        if (c.escape) {
          fire('pointermove', last, 1);
          await sleep(60);
          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
          await sleep(60);
        }
        fire('pointerup', last, 0);
      }
      const answered = await Promise.race([
        released.then(() => true),
        sleep(external ? 20_000 : 10_000).then(() => false),
      ]);
      if (c.escape) {
        await sleep(500);
        const after = await state();
        row['sent'] = answered;
        row['unchanged'] = same(before, after);
        row['ok'] = !answered && row['unchanged'] === true;
      } else {
        const r = rec;
        const value = r.answer?.ok ? (r.answer.value as Json) : null;
        const windowPlan = value?.['plan'];
        const replay = (r.replay ?? {}) as Json;
        row['pressSeen'] = r.layout !== undefined;
        row['pressAnswer'] = r.pressAnswer;
        row['blurs'] = blurs;
        row['remounted'] = el('timeline.layers') !== listAtStart;
        row['aborted'] = r.aborted === true;
        row['samples'] = r.samples;
        row['layout'] = r.layout;
        row['windowPlan'] = windowPlan;
        row['replayPlan'] = replay['plan'];
        const planOk = answered && windowPlan !== undefined && same(windowPlan, replay['plan']);
        const commands =
          ((windowPlan as Json | undefined)?.['commands'] as [string, Json][] | undefined) ?? [];
        const hits = c.command === null || commands.some(([name]) => name === c.command);
        const envelopes = (value?.['envelopes'] as Json[] | undefined) ?? [];
        const code = (e: Json) => (e['error'] as Json | undefined)?.['code'];
        // A refused case: every envelope answers the refusal (nothing committed).
        const committed = c.refused
          ? envelopes.length > 0 && envelopes.every((e) => e['status'] !== 'ok' && code(e) === c.refused)
          : envelopes.length > 0 && envelopes.every((e) => e['status'] === 'ok');
        const windowState = await state();
        // The same start: a document change undone; a UI change's state restored.
        const changedDocument = JSON.stringify(windowState.history) !== JSON.stringify(before.history);
        if (changedDocument) {
          await run('edit.undo', {});
        } else if (r.before) {
          await run('edit.selection.set', { context: 'timeline', ids: r.before.timelineIds });
          if (r.before.element)
            await run('app.context.set', { elementId: r.before.element, frame: r.before.frame });
          const range = r.before.range;
          await run(
            'timeline.playbackRange.set',
            range['mode'] === 'free'
              ? { mode: 'free', start: range['start'], end: range['end'] }
              : { mode: 'auto' },
          );
        }
        for (const [name, params] of commands) {
          if (c.refused) {
            const env = await engine.run(name, params).done;
            row['directError'] = env.status === 'ok' ? null : (env.error?.code ?? 'error');
          } else await run(name, params);
        }
        const directState = await state();
        row['planEqual'] = planOk;
        row['hitsTheGesture'] = hits;
        row['committed'] = committed;
        row['stateEqual'] = same(windowState, directState);
        if (row['stateEqual'] !== true) row['difference'] = { window: windowState, direct: directState };
        row['ok'] = planOk && hits && committed && row['stateEqual'] === true;
        if (c.refused) {
          row['refused'] = c.refused;
          row['unchanged'] = !changedDocument && same(before.history, windowState.history);
          row['ok'] = row['ok'] === true && row['directError'] === c.refused && row['unchanged'] === true;
        }
      }
    } catch (e: unknown) {
      row['ok'] = false;
      row['error'] = e instanceof Error ? e.message : String(e);
    }
    window.removeEventListener('blur', onBlurAll);
    for (const t of ['pointercancel', 'lostpointercapture', 'dragstart'])
      document.removeEventListener(t, onAny, true);
    row['events'] = seen;
    row['selectedTextAtStart'] = selectedAtStart;
    rows.push(row);
    // A preference the case set is put back (the sample document's reopening keeps preferences).
    for (const [command, params] of c.teardown?.(caseIds) ?? []) {
      await run(command, params).catch(() => undefined);
    }
    if (external) shared.__ttTimingCheck = { ...(shared.__ttTimingCheck ?? {}), done: index };
    rec = null;
  }
  const failed = rows.filter((r) => r['ok'] !== true).map((r) => r['name']);
  await host.timingCheckReport({ ok: failed.length === 0, platform, external, rows, failed });
  return true;
}
