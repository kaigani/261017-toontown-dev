// The Timeline panel (04 §11; reviews/M5-design-timeline-view.md): the layer-list toolbar, the virtualised layer list,
// the frame ruler and the tracks (two canvases drawn for the visible window only), the zoom control and the scroll
// bars. The window keeps no copy of document or session state beyond its last read (the M4 mirror rule): every row,
// bar, cell and selection drawn is the kernel's answer, read again after the events that change it, each source behind
// its read gate (D-89). Every press, drag and release goes to the kernel's timing-gesture interpreter
// (`timing_gesture`), which commits or cancels; the window draws only a local preview in between.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ContextMenu } from '../shell/ContextMenu';
import { menuAt, type MenuAt } from '../shell/contextMenuModel';
import { useActions } from '../shell/actionsApi';
import type { CommandRef } from '../shell/availability';
import { enabledIn, useAvailability } from '../shell/useAvailability';
import { useMessages } from '../shell/messagesApi';
import { useShell, useSlice } from '../state/context';
import { useReadGate } from '../state/readGate';
import type { AppState } from '../state/types';
import type { TimingLayout, TimingSample } from '../ipc';
import { drawRuler, drawTracks, type Ctx2D } from './draw';
import {
  LAYOUT,
  maxZoom,
  modifiersOf,
  platformOf,
  rowWindow,
  visibleFrames,
  type ViewPosition,
} from './geometry';
import { TimelineGesture, type Preview } from './gesture';
import { cellMenu, rowMenu, rulerMenu, type DialogItem, type TimelineMenu } from './menus';
import {
  CELL_BASED,
  cssColour,
  parseDisplay,
  parseExposures,
  parseRows,
  parseSelection,
  parseTransferOptions,
  parseView,
  shownRows,
  type RowDisplay,
  type RowsAnswer,
  type Run,
  type SelectionAnswer,
  type TimelineRow,
  type TransferOptions,
  type ViewAnswer,
} from './model';
import { dialogCommand, openTimelineDialog } from './dialogOpen';
import { TransferOptionsDialog } from './dialogs';
import { useDialogs } from '../shell/dialogsApi';

/** The layer list's width (CSS pixels). */
const LAYER_LIST_WIDTH = 220;

/** Events after which the rows, the cells and the selection are read again (view note §1). */
const REREAD = new Set([
  'element.state.changed',
  'element.created',
  'element.deleted',
  'cell.exposures.changed',
  'anim.changed',
  'doc.changed',
  'doc.undone',
  'doc.redone',
  'timeline.playbackRange.changed',
  'prefs.changed',
  'project.opened',
  'project.closed',
]);

/** Events that change only the selection (the rows do not depend on it). */
const SELECTION = new Set(['cell.selection.changed', 'edit.selection.changed']);

/**
 * App-state fields the rows do not depend on (`timeline.rows.list` answers the scene's rows; the current frame is
 * followed from the event, the current element and the selection by `cell.selection.get`). A change of any other field
 * reads everything again. On the plan's §6.2 scene a click on a cell re-read all 1,003 rows ahead of the selection
 * the click's draw waits for.
 */
const ROWS_UNCHANGED = new Set([
  'frame',
  'element',
  'drawing',
  'selection',
  'mode',
  'activeView',
  'focusedPanel',
  'activeTool',
  'playback',
]);

/** The arrow keys' steps of the current cell (plan §6.5). */
const ARROWS: Readonly<Record<string, { readonly rows: number; readonly frames: number }>> = {
  ArrowLeft: { rows: 0, frames: -1 },
  ArrowRight: { rows: 0, frames: 1 },
  ArrowUp: { rows: -1, frames: 0 },
  ArrowDown: { rows: 1, frames: 0 },
};

const PLATFORM = platformOf(typeof navigator === 'undefined' ? '' : navigator.userAgent);

type Json = Record<string, unknown>;
const obj = (v: unknown): Json =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Json) : {};

/** An open context menu: where, which, and its model. */
interface OpenMenu {
  readonly at: MenuAt;
  readonly id: string;
  readonly label: string;
  readonly menu: TimelineMenu;
}

/** A canvas sized to its box at device-pixel scale; returns the 2-D context in CSS pixels. */
function prepare(canvas: HTMLCanvasElement | null, width: number, height: number): Ctx2D | null {
  if (!canvas || width < 1 || height < 1) return null;
  const dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
  const w = Math.round(width * dpr);
  const h = Math.round(height * dpr);
  if (canvas.width !== w) canvas.width = w;
  if (canvas.height !== h) canvas.height = h;
  let ctx: CanvasRenderingContext2D | null;
  try {
    ctx = canvas.getContext('2d');
  } catch {
    return null;
  }
  if (!ctx) return null;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return ctx;
}

export function TimelinePanel() {
  const { engine } = useShell();
  const actions = useActions();
  const messages = useMessages();
  const dialogs = useDialogs();
  const app = useSlice('appState') as AppState | null | undefined;
  const open = Boolean(app?.project);

  const [rows, setRows] = useState<RowsAnswer | null>(null);
  const [view, setView] = useState<ViewAnswer | null>(null);
  const [selection, setSelection] = useState<SelectionAnswer | null>(null);
  const [runs, setRuns] = useState<ReadonlyMap<string, readonly Run[]>>(new Map());
  const [display, setDisplay] = useState<ReadonlyMap<string, RowDisplay>>(new Map());
  const [background, setBackground] = useState<string>('#ffffff');
  const [labels, setLabels] = useState<ReadonlyMap<number, string>>(new Map());
  // Counts the preference changes: the ruler's labels follow `interface.timeMeasurement` (an agent's change too, E-6).
  const [prefsRev, setPrefsRev] = useState(0);
  // The current frame from `timeline.currentFrame.changed` since the last rows read: a frame change (ten a second during
  // playback) moves the playhead only. Re-reading the rows, the selection and each row's display fields for it
  // saturated the window during playback on a slow machine (run 45304471459).
  const [frameNow, setFrameNow] = useState<number | null>(null);
  const [range, setRange] = useState<RowsAnswer['playbackRange'] | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  // Show Timeline Selection's row (the notice's `revealElementId`, each reveal counted): scrolled into the visible window
  // once the rows that hold it are read (C51B-R1S-01).
  const [reveal, setReveal] = useState<{ readonly id: string; readonly count: number } | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [preview, setPreview] = useState<Preview | null>(null);
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const [renaming, setRenaming] = useState<{ elementId: string; name: string } | null>(null);

  const listRef = useRef<HTMLDivElement | null>(null);
  const tracksRef = useRef<HTMLCanvasElement | null>(null);
  const rulerRef = useRef<HTMLCanvasElement | null>(null);
  const boxRef = useRef<HTMLDivElement | null>(null);
  // The revision a release committed at, and the revision of the newest rows read: the preview holds until a read at
  // the committed revision is drawn (D-53).
  const [committed, setCommitted] = useState<number | null>(null);
  const [rowsRev, setRowsRev] = useState(0);

  const rowsGate = useReadGate();
  const viewGate = useReadGate();
  const selGate = useReadGate();
  const cellsGate = useReadGate();
  const displayGate = useReadGate();
  const prefsGate = useReadGate();
  const labelGate = useReadGate();
  const rangeGate = useReadGate();

  const shown = useMemo(() => shownRows(rows?.rows ?? []), [rows]);
  const currentFrame = frameNow ?? rows?.currentFrame ?? 1;
  // The rows with children (every row of the answer, shown or under a collapsed parent): they show the disclosure.
  const parents = useMemo(
    () => new Set((rows?.rows ?? []).flatMap((r) => (r.parentId ? [r.parentId] : []))),
    [rows],
  );
  // Memoised on the values, not on the answer object: a read answering the same view keeps every callback that depends
  // on the position (each read sets a new object; depending on it re-read without end).
  const zoomLevel = view?.zoomLevel ?? LAYOUT.defaultZoomLevel;
  const firstFrame = view?.firstFrame ?? 1;
  const position: ViewPosition = useMemo(() => ({ zoomLevel, firstFrame }), [zoomLevel, firstFrame]);
  // The tracks area's own size (the observed box holds the tracks canvas alone: no ruler, no scroll bar).
  const tracksHeight = Math.max(0, size.height);
  // The preview holds until a read at the committed revision has arrived (D-53).
  const shownPreview = preview && (committed === null || rowsRev < committed) ? preview : null;
  // The last frame in view: the kernel's answer for the reported viewport width (`timeline.view.set`), else the local
  // width's.
  const lastFrame =
    view?.lastFrame ?? position.firstFrame + visibleFrames(size.width, position.zoomLevel) - 1;

  // ----- reads --------------------------------------------------------------------------------------------------------

  const readView = useCallback(() => {
    const live = viewGate();
    void engine
      .run('timeline.view.set', {})
      .done.then((env) => {
        if (live() && env.status === 'ok') setView(parseView(env.result));
      })
      .catch(() => undefined);
  }, [engine, viewGate]);

  // One rows read in flight: the engine runs one command at a time, and a zoom stepped each frame queued a full read
  // per step behind which every other read waited (5 s on the plan's §6.2 scene). A newer request makes the answer in
  // flight stale (the read gate) and is sent when that answer returns; only the newest waiting request is kept.
  const rowsFlight = useRef<{ busy: boolean; queued: (() => void) | null }>({ busy: false, queued: null });
  // Frame notices and rows answers in one order (C51B-R1S-02): each frame notice is counted; a rows answer replaces the
  // notice's frame only when no frame notice arrived after its read was sent (an older snapshot never replaces a newer
  // frame, so the playhead cannot rest on an old frame after playback stops).
  const frameSeq = useRef(0);
  const readRows = useCallback(() => {
    const flight = rowsFlight.current;
    const live = rowsGate();
    const from = Math.max(1, position.firstFrame);
    const to = Math.max(from, lastFrame);
    const send = () => {
      flight.busy = true;
      const framesAtSend = frameSeq.current;
      void engine
        .run('timeline.rows.list', { from, to })
        .done.then((env) => {
          if (!live()) return;
          setRows(env.status === 'ok' ? parseRows(env.result) : null);
          if (frameSeq.current === framesAtSend) setFrameNow(null);
          setRowsRev(Number(env.documentRev ?? 0));
        })
        .catch(() => undefined)
        .finally(() => {
          flight.busy = false;
          const next = flight.queued;
          flight.queued = null;
          next?.();
        });
    };
    if (flight.busy) flight.queued = send;
    else send();
  }, [engine, rowsGate, position.firstFrame, lastFrame]);

  const readSelection = useCallback(() => {
    const live = selGate();
    void engine
      .run('cell.selection.get', {})
      .done.then((env) => {
        if (live()) setSelection(env.status === 'ok' ? parseSelection(env.result) : null);
      })
      .catch(() => undefined);
  }, [engine, selGate]);

  const readPrefs = useCallback(() => {
    const live = prefsGate();
    void engine
      .run('prefs.get', { keys: ['interface.background.timeline'] })
      .done.then((env) => {
        if (!live() || env.status !== 'ok') return;
        const v = obj(obj(env.result)['values'])['interface.background.timeline'];
        setBackground(cssColour(typeof v === 'string' ? v : null, '#ffffff'));
      })
      .catch(() => undefined);
  }, [engine, prefsGate]);

  // The ruler's playback-range markers (`timeline.playbackRange.get`, 04 line 608).
  const readRange = useCallback(() => {
    const live = rangeGate();
    void engine
      .run('timeline.playbackRange.get', {})
      .done.then((env) => {
        if (!live() || env.status !== 'ok') return;
        const r = obj(env.result);
        if (typeof r['start'] === 'number' && typeof r['end'] === 'number')
          setRange({ mode: r['mode'] === 'free' ? 'free' : 'auto', start: r['start'], end: r['end'] });
      })
      .catch(() => undefined);
  }, [engine, rangeGate]);

  const readAll = useCallback(() => {
    if (!open) return;
    readView();
    readRows();
    readSelection();
    readPrefs();
    readRange();
  }, [open, readView, readRows, readSelection, readPrefs, readRange]);

  // The visible rows' cells and display fields, and the ruler's labels: read for the window in view.
  const visible = useMemo(() => {
    const w = rowWindow(shown.length, scrollTop, tracksHeight, 2);
    return shown.slice(w.first, w.last + 1);
  }, [shown, scrollTop, tracksHeight]);
  const visibleKey = visible.map((r) => r.elementId).join('|');
  useEffect(() => {
    if (!open || visible.length === 0) return;
    const ids = visible.filter((r) => CELL_BASED.has(r.type)).map((r) => r.elementId);
    const from = Math.max(1, position.firstFrame);
    const to = Math.max(from, lastFrame);
    if (ids.length > 0) {
      const live = cellsGate();
      void engine
        .run('xsheet.exposures.get', { elementIds: ids, from, to })
        .done.then((env) => {
          if (live() && env.status === 'ok') setRuns(parseExposures(env.result) ?? new Map());
        })
        .catch(() => undefined);
    }
    const liveDisplay = displayGate();
    void Promise.all(
      visible.map((r) =>
        engine
          .run('element.get', { elementId: r.elementId })
          .done.then((env) => [r.elementId, env.status === 'ok' ? parseDisplay(env.result) : null] as const)
          .catch(() => [r.elementId, null] as const),
      ),
    ).then((pairs) => {
      if (!liveDisplay()) return;
      const m = new Map<string, RowDisplay>();
      for (const [id, d] of pairs) if (d) m.set(id, d);
      setDisplay(m);
    });
    // `rows` is a dependency through `visibleKey` and the answer's revision.
  }, [open, engine, visibleKey, rows, position.firstFrame, lastFrame, cellsGate, displayGate, visible]);

  useEffect(() => {
    if (!open || lastFrame < position.firstFrame) return;
    // About one label every 48 pixels: the frames per label from the zoom level's pixels per frame.
    const z = LAYOUT.zoomLevels[Math.min(position.zoomLevel, maxZoom())];
    const pxPerFrame = (z?.cellWidth ?? 16) / Math.max(1, z?.framesPerCell ?? 1);
    const step = Math.max(1, Math.ceil(48 / Math.max(0.01, pxPerFrame)));
    const frames: number[] = [];
    for (let f = position.firstFrame; f <= lastFrame && frames.length < 400; f += step) frames.push(f);
    const live = labelGate();
    void engine
      .run('timeline.frameLabel.get', { frames })
      .done.then((env) => {
        if (!live() || env.status !== 'ok') return;
        const list = obj(env.result)['labels'];
        const m = new Map<number, string>();
        if (Array.isArray(list))
          list.forEach((l, i) => {
            const f = frames[i];
            if (typeof l === 'string' && f !== undefined) m.set(f, l);
          });
        setLabels(m);
      })
      .catch(() => undefined);
  }, [
    open,
    engine,
    labelGate,
    position.firstFrame,
    position.zoomLevel,
    lastFrame,
    rows?.sceneLength,
    prefsRev,
  ]);

  // The newest readers, for the subscription: it subscribes once per engine. Depending on the readers subscribed again
  // and read everything twice at each change of the frames in view (each step of a zoom; plan §6.2's budgets).
  const readAllRef = useRef(readAll);
  const readSelectionRef = useRef(readSelection);
  useEffect(() => {
    readAllRef.current = readAll;
    readSelectionRef.current = readSelection;
  });
  // A project opened (or the panel's first project): everything is read for it.
  useEffect(() => {
    if (open) readAllRef.current();
  }, [open]);

  // Subscribe, then read (D-85, D-86): nothing between the first read and the subscription is lost.
  useEffect(() => {
    let off: (() => void) | null = null;
    let stopped = false;
    const readAll = () => {
      readAllRef.current();
    };
    const readSelection = () => {
      readSelectionRef.current();
    };
    // The view notice's fields other than the view (the display toggles, Show Timeline Selection's reveal, the scene):
    // a change of them reads everything again (E-40). A change of the view alone sets it; the rows are read again when
    // the frames in view change (the effect on `readRows`).
    let notice: string | null = null;
    let revealCount: number | null = null;
    const frameNotice = (f: number) => {
      frameSeq.current += 1;
      setFrameNow(f);
    };
    readAll();
    void engine
      .onEvent((e) => {
        if (e.name === 'shell.timelineView') {
          const payload = obj(e.payload);
          const v = parseView(payload['view']);
          if (v) setView(v);
          // Show Timeline Selection: each reveal counts, so a reveal of the same row again scrolls again (the first
          // notice the panel sees only records the count).
          const id = payload['revealElementId'];
          const count = payload['revealCount'];
          if (typeof count === 'number') {
            if (revealCount !== null && count !== revealCount && typeof id === 'string')
              setReveal({ id, count });
            revealCount = count;
          }
          const rest = JSON.stringify({ ...payload, view: null });
          if (rest !== notice) {
            notice = rest;
            readAll();
          }
        }
        if (e.name === 'prefs.changed') setPrefsRev((r) => r + 1);
        if (e.name === 'timeline.currentFrame.changed') {
          const f = obj(e.payload)['frame'];
          if (typeof f === 'number') frameNotice(f);
        }
        if (REREAD.has(e.name)) readAll();
        if (SELECTION.has(e.name)) readSelection();
        // An app-state change of fields the rows do not depend on moves the playhead (each playback frame) and reads
        // the selection again; any other field reads everything again.
        if (e.name === 'app.state.changed') {
          const p = obj(e.payload);
          const changed = Array.isArray(p['changed']) ? (p['changed'] as unknown[]) : [];
          const f = obj(p['state'])['frame'];
          if (changed.length > 0 && changed.every((c) => typeof c === 'string' && ROWS_UNCHANGED.has(c))) {
            if (changed.includes('frame') && typeof f === 'number') frameNotice(f);
            if (changed.some((c) => c !== 'frame' && c !== 'playback')) readSelection();
          } else readAll();
        }
      })
      .then((u) => {
        if (stopped) u();
        else {
          off = u;
          readAll();
        }
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine]);

  // The rows are read again when the window in view changes (a scroll that leaves the read window, a zoom).
  useEffect(() => {
    if (open) readRows();
  }, [open, readRows]);

  // ----- size ---------------------------------------------------------------------------------------------------------

  useEffect(() => {
    const el = boxRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (!r) return;
      setSize({ width: Math.max(0, r.width), height: Math.max(0, r.height) });
      // The kernel answers `timeline.view.set`'s `lastFrame` for this width (an agent's view of the window).
      if (r.width >= 1) void engine.report({ timelineViewport: Math.round(r.width) }).catch(() => undefined);
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [engine]);

  // ----- drawing ------------------------------------------------------------------------------------------------------

  useEffect(() => {
    const ctx = prepare(tracksRef.current, size.width, tracksHeight);
    if (!ctx) return;
    const began = performance.now();
    drawTracks(
      ctx,
      {
        rows: shown,
        runs,
        display,
        selectedRows: new Set(selection && selection.rowsSelected > 0 ? selection.elementIds : []),
        cells:
          selection?.rowsSelected === 0 && selection.from > 0
            ? { elementIds: selection.elementIds, from: selection.from, to: selection.to }
            : null,
        currentElementId: selection?.currentElementId ?? null,
        currentFrame,
        background,
      },
      { view: position, width: size.width, height: tracksHeight, scrollTop },
    );
    if (shownPreview?.dragged) drawPreview(ctx, shownPreview, position, scrollTop, tracksHeight);
    // M0 diagnostics only (plan §6.2's window budgets, desktop/src/diagnostics/timelineBudgets.ts): each draw's work and
    // what it showed. Absent from the default build.
    if (__TT_M0_DIAGNOSTICS__) {
      const probe = (window as { __ttTimelineDraws?: unknown[] }).__ttTimelineDraws;
      probe?.push({
        at: began,
        ms: performance.now() - began,
        scrollTop,
        firstFrame: position.firstFrame,
        zoomLevel: position.zoomLevel,
        currentFrame,
        cells: selection ? `${selection.elementIds.join(',')}:${selection.from}-${selection.to}` : '',
      });
    }
  }, [
    shown,
    runs,
    display,
    selection,
    rows,
    currentFrame,
    background,
    position,
    size.width,
    tracksHeight,
    scrollTop,
    shownPreview,
  ]);

  useEffect(() => {
    const ctx = prepare(rulerRef.current, size.width, LAYOUT.rulerHeight);
    if (!ctx || !rows) return;
    drawRuler(
      ctx,
      {
        currentFrame,
        sceneLength: rows.sceneLength,
        range: range ?? rows.playbackRange,
        labels,
      },
      position,
      size.width,
    );
  }, [rows, currentFrame, range, labels, position, size.width]);

  // ----- gestures -----------------------------------------------------------------------------------------------------

  const gesture = useMemo(
    () =>
      new TimelineGesture(
        {
          timingGesture: (request) => engine.timingGesture(request),
          scrub: async (frame) => {
            await engine.run('timeline.currentFrame.set', { frame }).done;
          },
        },
        {
          preview: (p) => {
            setPreview(p);
            if (p) setCommitted(null);
          },
          refusal: (error) => {
            if (!messages.refused(error, 'timeline.gesture')) {
              messages.status(`${error.code}: ${error.message}`, 'warning');
            }
          },
          committed: (rev) => {
            if (rev === null) setPreview(null);
            else setCommitted(rev);
          },
          // A click's cell selection is drawn from the release's own answer (plan §6.2: a click to the selection drawn
          // within two frames). Waiting for `cell.selection.get` put a third round trip, queued behind the other
          // panels' reads of the same change, between the click and the draw (41-47 ms against 33 ms, release build).
          // The read gate drops a selection read sent before; the reads the change's events make confirm it.
          applied: (commands, envelopes) => {
            const at = commands.findIndex(([name]) => name === 'cell.select');
            const env = envelopes[at];
            if (at < 0 || env?.['status'] !== 'ok') return;
            const result = (env['result'] ?? {}) as Record<string, unknown>;
            const params = commands[at]?.[1] ?? {};
            const ids = Array.isArray(result['elementIds']) ? result['elementIds'] : params['elementIds'];
            const from = typeof result['from'] === 'number' ? result['from'] : params['from'];
            const to = typeof result['to'] === 'number' ? result['to'] : params['to'];
            if (!Array.isArray(ids) || typeof from !== 'number' || typeof to !== 'number') return;
            const cell = commands.find(([name]) => name === 'xsheet.currentCell.set')?.[1];
            const next: SelectionAnswer = {
              elementIds: ids.filter((x): x is string => typeof x === 'string'),
              from,
              to,
              rowsSelected: 0,
              currentElementId: typeof cell?.['elementId'] === 'string' ? cell['elementId'] : null,
              currentFrame: typeof cell?.['frame'] === 'number' ? cell['frame'] : from,
            };
            selGate();
            setSelection((prev) => ({
              ...next,
              currentElementId: next.currentElementId ?? prev?.currentElementId ?? null,
            }));
          },
        },
      ),
    [engine, messages, selGate],
  );
  useEffect(
    () => () => {
      gesture.abort();
    },
    [gesture],
  );
  useEffect(() => {
    const blur = () => {
      gesture.abort();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && gesture.active) gesture.abort();
    };
    window.addEventListener('blur', blur);
    window.addEventListener('keydown', key, true);
    return () => {
      window.removeEventListener('blur', blur);
      window.removeEventListener('keydown', key, true);
    };
  }, [gesture]);

  const layoutFor = (surface: TimingLayout['surface']): TimingLayout => ({
    surface,
    zoomLevel: position.zoomLevel,
    firstFrame: position.firstFrame,
    scrollTop: surface === 'ruler' ? 0 : scrollTop,
    platform: PLATFORM,
  });
  const sampleOf = (e: React.PointerEvent, el: Element): TimingSample => {
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top, modifiers: modifiersOf(e) };
  };
  const pointerHandlers = (surface: TimingLayout['surface']) => ({
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => {
      if (e.button !== 0 || !open) return;
      const el = e.currentTarget;
      try {
        el.setPointerCapture(e.pointerId);
      } catch {
        // A synthetic event has no capturable pointer.
      }
      // The surface takes the keyboard focus: the arrow keys then move the current cell (plan §6.5).
      el.focus({ preventScroll: true });
      gesture.press(layoutFor(surface), sampleOf(e, el));
    },
    onPointerMove: (e: React.PointerEvent<HTMLElement>) => {
      if (gesture.active) gesture.move(sampleOf(e, e.currentTarget));
    },
    onPointerUp: (e: React.PointerEvent<HTMLElement>) => {
      if (gesture.active) void gesture.release(sampleOf(e, e.currentTarget));
    },
    onLostPointerCapture: () => {
      if (gesture.active) gesture.abort();
    },
  });

  // ----- keys ---------------------------------------------------------------------------------------------------------

  // The arrow keys in the Timeline move the current cell (plan §6.5): Left and Right a frame, Up and Down a shown row
  // (`xsheet.currentCell.set`, the cell a click selects). A text field or a slider keeps its own arrows.
  const onKeyDown = (e: React.KeyboardEvent<HTMLElement>) => {
    if (!open || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const t = e.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement)
      return;
    const step = ARROWS[e.key];
    if (!step || shown.length === 0) return;
    e.preventDefault();
    const at = shown.findIndex((r) => r.elementId === selection?.currentElementId);
    const row = shown[Math.max(0, Math.min(shown.length - 1, (at < 0 ? 0 : at) + step.rows))];
    if (!row) return;
    const frame = Math.max(1, currentFrame + step.frames);
    void actions.run('xsheet.currentCell.set', { elementId: row.elementId, frame });
  };

  // ----- menus --------------------------------------------------------------------------------------------------------

  const selectedRows = selection && selection.rowsSelected > 0 ? selection.elementIds : [];
  // Solo mode, a toggle (04 line 604): the reference names the selected rows (none: the kernel's default, the selected
  // rows, else the current element); the kernel completes `enabled` to the next state and says whether solo is on.
  const selectedKey = selectedRows.join('|');
  const soloRefs = useMemo<readonly CommandRef[]>(
    () => [
      {
        commandId: 'element.solo.set',
        params: selectedKey === '' ? {} : { elementIds: selectedKey.split('|') },
      },
    ],
    [selectedKey],
  );
  const soloAvailability = useAvailability(soloRefs);
  const soloRef = soloRefs[0];
  const solo = soloRef ? soloAvailability(soloRef) : undefined;
  // The parameters as the kernel completes them (`enabled`, the next state), as the main toolbar sends them (O-10).
  const soloRun: Record<string, unknown> =
    solo && solo.params !== null && typeof solo.params === 'object'
      ? (solo.params as Record<string, unknown>)
      : (soloRef?.params ?? {});
  const rowAtY = (y: number): TimelineRow | undefined =>
    shown[Math.floor((y + scrollTop) / LAYOUT.rowHeight)];
  const frameAtX = (x: number): number => {
    const z = LAYOUT.zoomLevels[Math.min(position.zoomLevel, maxZoom())];
    const cw = Math.max(1, z?.cellWidth ?? 16);
    return position.firstFrame + Math.floor(x / cw) * Math.max(1, z?.framesPerCell ?? 1);
  };
  const openRowMenu = (e: React.MouseEvent<HTMLElement>, row: TimelineRow) => {
    e.preventDefault();
    setMenu({
      at: menuAt(e),
      id: 'timeline.rowMenu',
      label: 'Timeline row',
      menu: rowMenu(row, shown, selectedRows, currentFrame, (display.get(row.elementId)?.note ?? '') !== ''),
    });
  };
  // A menu the keyboard opened (the context-menu key or Shift+F10: no pointer position, D-66 item 3) acts on the current
  // cell: the current element's row and the current frame (ruling E-48).
  const byKeyboard = (e: React.MouseEvent) => e.clientX === 0 && e.clientY === 0;
  const openTracksMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const r = e.currentTarget.getBoundingClientRect();
    const keyboard = byKeyboard(e);
    const row = keyboard
      ? (shown.find((x) => x.elementId === selection?.currentElementId) ?? shown[0])
      : rowAtY(e.clientY - r.top);
    if (!row) return;
    setMenu({
      at: menuAt(e),
      id: 'timeline.cellMenu',
      label: 'Timeline cells',
      menu: cellMenu(row, keyboard ? currentFrame : frameAtX(e.clientX - r.left)),
    });
  };
  const openRulerMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      at: menuAt(e),
      id: 'timeline.rulerMenu',
      label: 'Timeline ruler',
      menu: rulerMenu(byKeyboard(e) ? currentFrame : frameAtX(e.clientX - r.left)),
    });
  };

  // Show Timeline Selection (C51B-R1S-01): the revealed row scrolled into the visible window, the least distance.
  useEffect(() => {
    if (!reveal) return;
    const index = shown.findIndex((x) => x.elementId === reveal.id);
    const list = listRef.current;
    if (index < 0 || !list) return;
    const top = index * LAYOUT.rowHeight;
    let next = list.scrollTop;
    if (top < next) next = top;
    else if (top + LAYOUT.rowHeight > next + list.clientHeight)
      next = top + LAYOUT.rowHeight - list.clientHeight;
    if (next !== list.scrollTop) {
      list.scrollTop = next;
      setScrollTop(list.scrollTop);
    }
    setReveal(null);
  }, [reveal, shown]);
  const runDialog = (item: DialogItem) => {
    // Rename is one thing in the Timeline (04 §4.1; ruling E-50 item 1): the context item opens the editor a double click
    // opens, in the row, prefilled with the name (the main menu's Rename keeps its dialog).
    if (item.dialog === 'rename') {
      const row = shown.find((r) => r.elementId === item.elementId);
      if (row) setRenaming({ elementId: row.elementId, name: row.name });
      return;
    }
    void openTimelineDialog(dialogs, item, {
      rows: shown,
      selected: selectedRows,
      scene: rows,
      display,
      currentFrame,
    }).then((ref) => {
      if (ref) void actions.perform(ref.command, ref.params);
    });
  };

  // ----- view controls ------------------------------------------------------------------------------------------------

  const setViewParams = (params: { zoomLevel?: number; firstFrame?: number }) => {
    void actions.run('timeline.view.set', params);
  };
  const onWheel = (e: React.WheelEvent) => {
    if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      const d =
        Math.sign(e.deltaX || e.deltaY) *
        Math.max(1, Math.round(visibleFrames(size.width, position.zoomLevel) / 10));
      setViewParams({ firstFrame: Math.max(1, position.firstFrame + d) });
    } else if (listRef.current) {
      listRef.current.scrollTop += e.deltaY;
    }
  };

  const total = (rows?.sceneLength ?? 1) + LAYOUT.emptyFramesPastEnd;
  const list = rowWindow(shown.length, scrollTop, tracksHeight);

  return (
    <div
      className="timeline-panel"
      data-tt-role="panel"
      data-tt-id="timeline"
      data-tt-label="Timeline"
      onKeyDown={onKeyDown}
      // A column that fills its dock area: the layout, not the canvases' pixel sizes, sizes every box.
      style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0, overflow: 'hidden' }}
    >
      <div className="timeline-toolbar" role="toolbar" data-tt-role="toolbar" data-tt-id="timeline.toolbar">
        <button
          type="button"
          disabled={!open}
          title="Add a drawing element above the current row (element.create)"
          data-tt-role="button"
          data-tt-id="timeline.add"
          data-tt-command="element.create"
          data-tt-params={JSON.stringify({ type: 'drawing' })}
          onClick={() => void actions.run('element.create', { type: 'drawing' })}
        >
          Add
        </button>
        <button
          type="button"
          disabled={!open || selectedRows.length === 0}
          title="Add a parent peg to the selected rows (element.parentPeg.create)"
          data-tt-role="button"
          data-tt-id="timeline.addParentPeg"
          data-tt-command="element.parentPeg.create"
          data-tt-params={JSON.stringify({ elementIds: selectedRows })}
          onClick={() => void actions.run('element.parentPeg.create', { elementIds: selectedRows })}
        >
          Parent Peg
        </button>
        <button
          type="button"
          disabled={!open || !enabledIn(solo)}
          aria-pressed={solo?.checked === true}
          title="Solo mode: show only the selected rows (element.solo.set)"
          data-tt-role="button"
          data-tt-id="timeline.solo"
          data-tt-command="element.solo.set"
          data-tt-params={JSON.stringify(soloRun)}
          onClick={() => void actions.perform('element.solo.set', soloRun)}
        >
          Solo
        </button>
        <button
          type="button"
          disabled={!open}
          title="Show every element (element.visibility.set)"
          data-tt-role="button"
          data-tt-id="timeline.showAll"
          data-tt-command="element.visibility.set"
          data-tt-params={JSON.stringify({ scope: 'scene', visible: true, apply: 'all' })}
          onClick={() =>
            void actions.run('element.visibility.set', { scope: 'scene', visible: true, apply: 'all' })
          }
        >
          Show All
        </button>
        <button
          type="button"
          disabled={!open}
          title="The options a cell drag uses (timeline.transferOptions.set)"
          data-tt-role="button"
          data-tt-id="timeline.dragOptions"
          data-tt-command="timeline.transferOptions.set"
          data-tt-params={JSON.stringify({ operation: 'dragDrop' })}
          onClick={() => {
            const render = (close: (o: TransferOptions | null) => void) => (
              <TransferOptionsDialog
                title="Drag and Drop Options"
                load={() =>
                  engine
                    .run('timeline.transferOptions.get', { operation: 'dragDrop' })
                    .done.then((env) => (env.status === 'ok' ? parseTransferOptions(env.result) : null))
                }
                onDone={close}
              />
            );
            // The title written where the dialog opens (the D-44 inventory, scripts/c43_d44_coverage.py).
            const chosen = dialogs.open<TransferOptions>('Drag and Drop Options', render);
            void chosen.then((options) => {
              if (options)
                void actions.run('timeline.transferOptions.set', { operation: 'dragDrop', options });
            });
          }}
        >
          Drag Options
        </button>
        <label className="timeline-zoom" data-tt-role="text" data-tt-id="timeline.zoomLabel">
          Zoom
          <input
            type="range"
            min={0}
            max={maxZoom()}
            step={1}
            value={position.zoomLevel}
            disabled={!open}
            aria-label="Zoom"
            data-tt-role="slider"
            data-tt-id="timeline.zoom"
            data-tt-command="timeline.view.set"
            data-tt-params={JSON.stringify({ zoomLevel: position.zoomLevel })}
            onChange={(e) => {
              setViewParams({ zoomLevel: Number(e.currentTarget.value) });
            }}
          />
        </label>
      </div>
      <div className="timeline-body" style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div
          className="timeline-layers"
          style={{ width: LAYER_LIST_WIDTH, display: 'flex', flexDirection: 'column' }}
        >
          <div className="timeline-ruler-spacer" style={{ height: LAYOUT.rulerHeight }} />
          <div
            ref={listRef}
            className="timeline-layer-list"
            role="list"
            data-tt-role="list"
            data-tt-id="timeline.layers"
            tabIndex={0}
            // Rename in place (04 §4.1; ruling E-50 item 1): a double click on a row. The list's pointer handler captures
            // the pointer at each press, so the browser sends the click and the double click to the list, not to the row
            // under the pointer (found by real input only): the row is the one under the pointer.
            onDoubleClick={(e) => {
              const t = e.target;
              if (t instanceof HTMLElement && t.closest('button, input')) return;
              const box = e.currentTarget.getBoundingClientRect();
              const row =
                shown[Math.floor((e.clientY - box.top + e.currentTarget.scrollTop) / LAYOUT.rowHeight)];
              if (row) setRenaming({ elementId: row.elementId, name: row.name });
            }}
            // The keyboard on the list itself (the context-menu key, Shift+F10, an agent's `key`; ruling E-48): the row
            // context of the current element's row. A row's own secondary click opens its menu in the row.
            onContextMenu={(e) => {
              if (e.target !== e.currentTarget || !byKeyboard(e)) return;
              const row = shown.find((x) => x.elementId === selection?.currentElementId) ?? shown[0];
              if (row) openRowMenu(e, row);
            }}
            // Not selectable: a drag across the row names selected their text, and the next press on that text began the
            // browser's own drag (pointercancel), which dropped the row drag (found by the three-way rows' real input).
            style={{
              flex: 1,
              overflowY: 'auto',
              position: 'relative',
              userSelect: 'none',
              WebkitUserSelect: 'none',
            }}
            onScroll={(e) => {
              setScrollTop(e.currentTarget.scrollTop);
            }}
            {...pointerHandlers('layers')}
          >
            <div style={{ height: shown.length * LAYOUT.rowHeight, position: 'relative' }}>
              {shown.slice(list.first, list.last + 1).map((r, k) => (
                <LayerRow
                  key={r.elementId}
                  row={r}
                  top={(list.first + k) * LAYOUT.rowHeight}
                  display={display.get(r.elementId)}
                  selected={selectedRows.includes(r.elementId)}
                  current={selection?.currentElementId === r.elementId}
                  parent={parents.has(r.elementId)}
                  renaming={renaming?.elementId === r.elementId ? renaming.name : null}
                  onRenameChange={(name) => {
                    setRenaming({ elementId: r.elementId, name });
                  }}
                  onRenameEnd={(commit) => {
                    const name = renaming?.name ?? r.name;
                    setRenaming(null);
                    if (commit && name.trim() !== '' && name !== r.name)
                      void actions.run('element.rename', { elementId: r.elementId, name });
                  }}
                  onContextMenu={(e) => {
                    openRowMenu(e, r);
                  }}
                  onToggleCollapsed={() =>
                    void actions.run('element.collapsed.set', {
                      elementIds: [r.elementId],
                      collapsed: !r.collapsed,
                    })
                  }
                  onToggleVisible={() =>
                    void actions.run('element.visibility.set', {
                      elementIds: [r.elementId],
                      scope: 'scene',
                      visible: !r.visibleInScene,
                    })
                  }
                />
              ))}
            </div>
          </div>
        </div>
        <div
          className="timeline-tracks-box"
          style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', position: 'relative' }}
          onWheel={onWheel}
        >
          <canvas
            ref={rulerRef}
            className="timeline-ruler"
            data-tt-role="canvas"
            data-tt-id="timeline.ruler"
            data-tt-labels={JSON.stringify([...labels.values()].slice(0, 8))}
            style={{ width: '100%', height: LAYOUT.rulerHeight, display: 'block', touchAction: 'none' }}
            onContextMenu={openRulerMenu}
            {...pointerHandlers('ruler')}
          />
          {/* The tracks canvas fills its area and never sizes it: a canvas's pixel height as its layout height grew the
              box at every resize, which the size observer fed back without end. */}
          <div
            ref={boxRef}
            className="timeline-tracks-area"
            style={{ flex: 1, minHeight: 0, position: 'relative' }}
          >
            <canvas
              ref={tracksRef}
              className="timeline-tracks"
              data-tt-role="canvas"
              data-tt-id="timeline.tracks"
              tabIndex={0}
              data-tt-first-frame={position.firstFrame}
              data-tt-zoom={position.zoomLevel}
              data-tt-rows={shown.length}
              data-tt-background={background}
              data-tt-current-frame={currentFrame}
              // The document revision of the rows drawn (the window tree's value): a client waits for the rows of its
              // change before it opens a menu built from them (D-45).
              data-tt-value={String(rowsRev)}
              style={{
                position: 'absolute',
                inset: 0,
                width: '100%',
                height: '100%',
                display: 'block',
                touchAction: 'none',
              }}
              onContextMenu={openTracksMenu}
              {...pointerHandlers('tracks')}
            />
          </div>
          <input
            type="range"
            className="timeline-hscroll"
            min={1}
            max={Math.max(1, total)}
            step={1}
            value={position.firstFrame}
            disabled={!open}
            aria-label="First frame shown"
            data-tt-role="slider"
            data-tt-id="timeline.scroll"
            data-tt-command="timeline.view.set"
            data-tt-params={JSON.stringify({ firstFrame: position.firstFrame })}
            onChange={(e) => {
              setViewParams({ firstFrame: Number(e.currentTarget.value) });
            }}
          />
        </div>
      </div>
      {menu && (
        <ContextMenu
          id={menu.id}
          label={menu.label}
          at={menu.at}
          groups={[
            ...menu.menu.groups,
            ...(menu.menu.dialogs.length > 0
              ? [
                  {
                    items: menu.menu.dialogs.map((d) => ({
                      id: d.id,
                      label: d.label,
                      ...dialogCommand(d),
                      onSelect: () => {
                        runDialog(d);
                      },
                    })),
                  },
                ]
              : []),
          ]}
          onClose={() => {
            setMenu(null);
          }}
        />
      )}
    </div>
  );
}

/** The drag preview over the tracks: a ghost of the target moved by the travel (view note §3 item 2). */
function drawPreview(ctx: Ctx2D, p: Preview, v: ViewPosition, scrollTop: number, height: number): void {
  const row = typeof p.target['row'] === 'number' ? p.target['row'] : null;
  if (row === null) return;
  const z = LAYOUT.zoomLevels[Math.min(v.zoomLevel, maxZoom())];
  const pxPerFrame = (z?.cellWidth ?? 16) / Math.max(1, z?.framesPerCell ?? 1);
  const y = (row + (p.target.kind === 'row' ? p.rows : 0)) * LAYOUT.rowHeight - scrollTop;
  if (y < -LAYOUT.rowHeight || y > height) return;
  ctx.save();
  ctx.globalAlpha = 0.35;
  ctx.fillStyle = 'rgba(80, 140, 255, 0.6)';
  ctx.fillRect(
    p.press.x + p.frames * pxPerFrame - p.press.x,
    y,
    Math.max(2, Math.abs(p.last.x - p.press.x)),
    LAYOUT.rowHeight,
  );
  ctx.restore();
}

/** One row of the layer list: disclosure, visibility, name (double click renames in place), state glyphs. */
function LayerRow(props: {
  row: TimelineRow;
  top: number;
  display: RowDisplay | undefined;
  selected: boolean;
  current: boolean;
  /** The row has children: it shows the disclosure (04 line 605, "elements with children"). */
  parent: boolean;
  renaming: string | null;
  onRenameChange: (name: string) => void;
  onRenameEnd: (commit: boolean) => void;
  onContextMenu: (e: React.MouseEvent<HTMLElement>) => void;
  onToggleCollapsed: () => void;
  onToggleVisible: () => void;
}) {
  const { row: r } = props;
  const parent = props.parent || r.collapsed;
  // The editor takes the focus once mounted, with the name selected (04 §4.1: prefilled), in an effect rather than with
  // `autoFocus`: a context menu that opened it gives the focus back to its opener as it closes, which would blur an
  // editor focused earlier and commit it at once.
  const editor = useRef<HTMLInputElement | null>(null);
  const editing = props.renaming !== null;
  useEffect(() => {
    if (!editing) return;
    editor.current?.focus({ preventScroll: true });
    editor.current?.select();
  }, [editing]);
  const stop = (e: React.PointerEvent) => {
    e.stopPropagation();
  };
  return (
    <div
      role="listitem"
      className={`timeline-row${props.selected ? ' selected' : ''}${props.current ? ' current' : ''}${r.visibleInScene && !r.hiddenByParent ? '' : ' hidden'}`}
      data-tt-role="listitem"
      data-tt-id={`timeline.row.${r.elementId}`}
      data-tt-label={r.name}
      style={{
        position: 'absolute',
        top: props.top,
        left: 0,
        right: 0,
        height: LAYOUT.rowHeight,
        paddingLeft: 4 + r.depth * 12,
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        borderLeft: `4px solid ${cssColour(props.display?.markerColour)}`,
      }}
      onContextMenu={props.onContextMenu}
    >
      <button
        type="button"
        className="timeline-disclosure"
        aria-expanded={!r.collapsed}
        title={r.collapsed ? 'Expand (element.collapsed.set)' : 'Collapse (element.collapsed.set)'}
        data-tt-role="button"
        data-tt-id={`timeline.row.${r.elementId}.collapsed`}
        data-tt-command="element.collapsed.set"
        data-tt-params={JSON.stringify({ elementIds: [r.elementId], collapsed: !r.collapsed })}
        style={{ visibility: parent ? 'visible' : 'hidden' }}
        onPointerDown={stop}
        onClick={props.onToggleCollapsed}
      >
        {r.collapsed ? '+' : '-'}
      </button>
      <button
        type="button"
        className="timeline-visibility"
        aria-pressed={r.visibleInScene}
        title="Show or hide in the scene (element.visibility.set)"
        data-tt-role="button"
        data-tt-id={`timeline.row.${r.elementId}.visible`}
        data-tt-command="element.visibility.set"
        data-tt-params={JSON.stringify({
          elementIds: [r.elementId],
          scope: 'scene',
          visible: !r.visibleInScene,
        })}
        onPointerDown={stop}
        onClick={props.onToggleVisible}
      >
        {r.visibleInScene ? 'o' : '-'}
      </button>
      {props.renaming !== null ? (
        <input
          ref={editor}
          className="timeline-rename"
          style={{ userSelect: 'text', WebkitUserSelect: 'text' }}
          value={props.renaming}
          aria-label="Element name"
          data-tt-role="textField"
          data-tt-id={`timeline.row.${r.elementId}.name`}
          data-tt-command="element.rename"
          data-tt-params={JSON.stringify({ elementId: r.elementId })}
          onPointerDown={stop}
          onChange={(e) => {
            props.onRenameChange(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') props.onRenameEnd(true);
            if (e.key === 'Escape') props.onRenameEnd(false);
            e.stopPropagation();
          }}
          onBlur={() => {
            props.onRenameEnd(true);
          }}
        />
      ) : (
        <span className="timeline-name" data-tt-role="text" data-tt-id={`timeline.row.${r.elementId}.label`}>
          {r.name}
        </span>
      )}
      {r.locked && (
        <span
          className="timeline-lock"
          title="Locked"
          data-tt-role="text"
          data-tt-id={`timeline.row.${r.elementId}.locked`}
        >
          L
        </span>
      )}
      {props.display?.note ? (
        <span
          className="timeline-note"
          title={props.display.note}
          data-tt-role="text"
          data-tt-id={`timeline.row.${r.elementId}.note`}
        >
          N
        </span>
      ) : null}
    </div>
  );
}
