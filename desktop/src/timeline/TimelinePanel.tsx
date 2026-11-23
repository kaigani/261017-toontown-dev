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
  parseView,
  shownRows,
  type RowDisplay,
  type RowsAnswer,
  type Run,
  type SelectionAnswer,
  type TimelineRow,
  type ViewAnswer,
} from './model';
import { dialogCommand, openTimelineDialog } from './dialogOpen';
import { useDialogs } from '../shell/dialogsApi';

/** The layer list's width (CSS pixels). */
const LAYER_LIST_WIDTH = 220;

/** Events after which the rows, the cells and the selection are read again (view note §1). */
const REREAD = new Set([
  'element.state.changed',
  'element.created',
  'element.deleted',
  'cell.exposures.changed',
  'cell.selection.changed',
  'edit.selection.changed',
  'anim.changed',
  'doc.changed',
  'doc.undone',
  'doc.redone',
  'timeline.currentFrame.changed',
  'timeline.playbackRange.changed',
  'prefs.changed',
  'shell.timelineView',
  'project.opened',
  'project.closed',
  'app.state.changed',
]);

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
  const [range, setRange] = useState<RowsAnswer['playbackRange'] | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
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
  const position: ViewPosition = useMemo(
    () => ({ zoomLevel: view?.zoomLevel ?? LAYOUT.defaultZoomLevel, firstFrame: view?.firstFrame ?? 1 }),
    [view],
  );
  const tracksHeight = Math.max(0, size.height - LAYOUT.rulerHeight);
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

  const readRows = useCallback(() => {
    const live = rowsGate();
    const from = Math.max(1, position.firstFrame);
    const to = Math.max(from, lastFrame);
    void engine
      .run('timeline.rows.list', { from, to })
      .done.then((env) => {
        if (!live()) return;
        setRows(env.status === 'ok' ? parseRows(env.result) : null);
        setRowsRev(Number(env.documentRev ?? 0));
      })
      .catch(() => undefined);
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
  }, [open, engine, labelGate, position.firstFrame, position.zoomLevel, lastFrame, rows?.sceneLength]);

  // Subscribe, then read (D-85, D-86): nothing between the first read and the subscription is lost.
  useEffect(() => {
    let off: (() => void) | null = null;
    let stopped = false;
    readAll();
    void engine
      .onEvent((e) => {
        if (e.name === 'shell.timelineView') {
          const v = parseView(obj(e.payload)['view']);
          if (v) setView(v);
        }
        if (REREAD.has(e.name)) readAll();
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
  }, [engine, readAll]);

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
        currentFrame: rows?.currentFrame ?? 1,
        background,
      },
      { view: position, width: size.width, height: tracksHeight, scrollTop },
    );
    if (shownPreview?.dragged) drawPreview(ctx, shownPreview, position, scrollTop, tracksHeight);
  }, [
    shown,
    runs,
    display,
    selection,
    rows,
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
        currentFrame: rows.currentFrame,
        sceneLength: rows.sceneLength,
        range: range ?? rows.playbackRange,
        labels,
      },
      position,
      size.width,
    );
  }, [rows, range, labels, position, size.width]);

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
        },
      ),
    [engine, messages],
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

  // ----- menus --------------------------------------------------------------------------------------------------------

  const selectedRows = selection && selection.rowsSelected > 0 ? selection.elementIds : [];
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
      menu: rowMenu(row, shown, selectedRows),
    });
  };
  const openTracksMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const r = e.currentTarget.getBoundingClientRect();
    const row = rowAtY(e.clientY - r.top);
    if (!row) return;
    setMenu({
      at: menuAt(e),
      id: 'timeline.cellMenu',
      label: 'Timeline cells',
      menu: cellMenu(row, frameAtX(e.clientX - r.left)),
    });
  };
  const openRulerMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      at: menuAt(e),
      id: 'timeline.rulerMenu',
      label: 'Timeline ruler',
      menu: rulerMenu(frameAtX(e.clientX - r.left)),
    });
  };
  const runDialog = (item: DialogItem) => {
    void openTimelineDialog(dialogs, item, { rows: shown, selected: selectedRows, scene: rows }).then(
      (ref) => {
        if (ref) void actions.perform(ref.command, ref.params);
      },
    );
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
    <div className="timeline-panel" data-tt-role="panel" data-tt-id="timeline" data-tt-label="Timeline">
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
          disabled={!open}
          title="Solo mode: show only the selected rows (element.solo.set)"
          data-tt-role="button"
          data-tt-id="timeline.solo"
          data-tt-command="element.solo.set"
          data-tt-params={JSON.stringify({ elementIds: selectedRows })}
          onClick={() => void actions.perform('element.solo.set', { elementIds: selectedRows })}
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
            style={{ flex: 1, overflowY: 'auto', position: 'relative' }}
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
                  renaming={renaming?.elementId === r.elementId ? renaming.name : null}
                  onRenameChange={(name) => {
                    setRenaming({ elementId: r.elementId, name });
                  }}
                  onRenameStart={() => {
                    setRenaming({ elementId: r.elementId, name: r.name });
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
          ref={boxRef}
          className="timeline-tracks-box"
          style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', position: 'relative' }}
          onWheel={onWheel}
        >
          <canvas
            ref={rulerRef}
            className="timeline-ruler"
            data-tt-role="canvas"
            data-tt-id="timeline.ruler"
            style={{ width: '100%', height: LAYOUT.rulerHeight, display: 'block', touchAction: 'none' }}
            onContextMenu={openRulerMenu}
            {...pointerHandlers('ruler')}
          />
          <canvas
            ref={tracksRef}
            className="timeline-tracks"
            data-tt-role="canvas"
            data-tt-id="timeline.tracks"
            data-tt-first-frame={position.firstFrame}
            data-tt-zoom={position.zoomLevel}
            data-tt-rows={shown.length}
            style={{ width: '100%', flex: 1, minHeight: 0, display: 'block', touchAction: 'none' }}
            onContextMenu={openTracksMenu}
            {...pointerHandlers('tracks')}
          />
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
  renaming: string | null;
  onRenameChange: (name: string) => void;
  onRenameStart: () => void;
  onRenameEnd: (commit: boolean) => void;
  onContextMenu: (e: React.MouseEvent<HTMLElement>) => void;
  onToggleCollapsed: () => void;
  onToggleVisible: () => void;
}) {
  const { row: r } = props;
  const parent = r.childKeyframeFrames.length > 0 || r.collapsed;
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
      onDoubleClick={props.onRenameStart}
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
          className="timeline-rename"
          autoFocus
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
