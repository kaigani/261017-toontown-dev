// The interactive canvas of a Drawing view window (M4 C4.2b; design notes 1 and 2). One per window (D-7a): its own
// model (the pull loop), back end and frame loop. The canvas follows the kernel: the composition from `canvas_scene`,
// geometry from `canvas_load`, the mapping from `shell.view` and `draw.viewMapping.get` (it never derives a mapping).
//
// The element's `data-tt-value` carries what the canvas shows (back end, generation, each resident drawing and its
// revision), so the UI tree (`inspect_ui`) and the running-application tests can check it.

import { useEffect, useRef } from 'react';

import type { AgentInput, EngineApi } from '../ipc';
import { bucketOf } from './bucket';
import { gridGeometry, IDENTITY, scaleOf, ViewTris, type Affine } from './geometry';
import { ACCENT, GestureCollector, gestureOverlay, type DomSample, type OverlayStyle } from './input';
import { CanvasModel } from './model';
import type { Renderer } from './renderer';
import { createRenderer } from './backend';

export interface CanvasHandle {
  /** The overlay's triangles for the next frames (layer 9); `null` clears it. */
  setOverlay(t: ViewTris | null): void;
  /** The current drawing → view mapping. */
  mapping(): Affine;
  renderer(): Renderer | null;
  model(): CanvasModel | null;
}

function asAffine(v: unknown): Affine | null {
  return Array.isArray(v) && v.length === 6 && v.every((x) => typeof x === 'number' && Number.isFinite(x))
    ? (v as unknown as Affine)
    : null;
}

export function CanvasView({
  engine,
  viewId,
  onReady,
}: {
  engine: EngineApi;
  viewId: string;
  onReady?: (h: CanvasHandle | null) => void;
}) {
  const host = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const el = host.current;
    const canvas = canvasRef.current;
    if (!el || !canvas) return undefined;
    let renderer: Renderer | null = null;
    let model: CanvasModel | null = null;
    let d2v: Affine = IDENTITY;
    let size = { width: 0, height: 0 };
    let dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;
    let overlay: ViewTris | null = null;
    let viewVersion = 1;
    let drawnVersion = -1;
    let drawnView = -1;
    let grid: ViewTris | null = null;
    let lastColours: unknown = null;
    let summary = '';
    let raf = 0;
    let stopped = false;
    // Input (design note 2 §2 and §6): one gesture at a time; its overlay stays until the committed revision is drawn.
    const collector = new GestureCollector(viewId);
    let style: OverlayStyle = { tool: 'select', ink: ACCENT, width: 1 };
    let penSizes = { brush: 3, eraser: 10 };
    let currentColour: string | null = null;
    let clearOverlayAt: { version: number; deadline: number } | null = null;
    let lastGesture: { status: string; code?: string } | null = null;
    const refreshStyle = () => {
      const zoom = scaleOf(d2v);
      const tool = style.tool;
      const width =
        tool === 'eraser'
          ? penSizes.eraser * zoom
          : ['brush', 'pencil', 'polyline'].includes(tool)
            ? penSizes.brush * zoom
            : 1;
      const ink = (currentColour ? model?.scene?.colours[currentColour] : undefined) ?? ACCENT;
      style = { tool, ink, width };
    };
    const refreshTool = () => {
      void engine.run('tool.active.get', {}).done.then((env) => {
        const id = (env.result as Record<string, unknown> | undefined)?.['toolId'];
        if (typeof id === 'string') style = { ...style, tool: id };
        refreshStyle();
      });
      void engine.run('tool.pen.list', {}).done.then((env) => {
        const r = env.result as
          | {
              pens?: { penId: string; maxSize?: number }[];
              currentBrushPenId?: string;
              currentEraserPenId?: string;
            }
          | undefined;
        const size = (id?: string) => r?.pens?.find((p) => p.penId === id)?.maxSize;
        penSizes = {
          brush: size(r?.currentBrushPenId) ?? penSizes.brush,
          eraser: size(r?.currentEraserPenId) ?? penSizes.eraser,
        };
        refreshStyle();
      });
    };
    // Agent gestures routed to this window (design note 2 §6; P1, P2): one at a time, played through the overlay, then
    // committed unchanged through gesture_commit with their request id; a user press aborts the one playing.
    const agentQueue: AgentInput[] = [];
    let playing: { input: AgentInput; shown: number; points: [number, number][] } | null = null;
    const reportPresence = (present: boolean) => {
      const r = canvas.getBoundingClientRect();
      void engine
        .canvasPresence(
          viewId,
          present ? { x: r.left, y: r.top, w: r.width, h: r.height } : null,
          window.devicePixelRatio || 1,
        )
        .catch(() => undefined);
    };
    const reportGesture = (on: boolean) => {
      void engine
        .report({ modal: on ? { kind: 'gesture', title: style.tool } : null })
        .catch(() => undefined);
    };
    const cancelGesture = () => {
      if (collector.cancel()) {
        overlay = null;
        reportGesture(false);
      }
    };

    const refreshMapping = () => {
      void engine
        .run('draw.viewMapping.get', { view: viewId })
        .done.then((env) => {
          const m = asAffine((env.result as Record<string, unknown> | undefined)?.['drawingToView']);
          if (m && !stopped) {
            d2v = m;
            viewVersion++;
            model?.setBucket(bucketOf(scaleOf(m)));
            refreshStyle();
          }
        })
        .catch(() => undefined);
    };

    try {
      renderer = createRenderer(canvas);
    } catch (e) {
      el.setAttribute('data-tt-value', JSON.stringify({ error: e instanceof Error ? e.message : String(e) }));
    }
    if (renderer) {
      model = new CanvasModel(viewId, engine, renderer, { now: () => performance.now() });
      model.setGeneration(engine.generation());
    }
    refreshMapping();

    const ro =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver((entries) => {
            const r = entries[0]?.contentRect;
            if (!r || r.width < 1 || r.height < 1) return;
            size = { width: r.width, height: r.height };
            dpr = window.devicePixelRatio || 1;
            canvas.width = Math.max(1, Math.round(r.width * dpr));
            canvas.height = Math.max(1, Math.round(r.height * dpr));
            viewVersion++;
            reportPresence(true);
            void engine
              .report({ viewSizes: { [viewId]: { width: r.width, height: r.height } } })
              .then(refreshMapping, () => undefined);
          });
    ro?.observe(el);

    let unlisten: (() => void) | null = null;
    void engine
      .onEvent((event) => {
        if (typeof event.generation === 'number' && event.generation !== engine.generation()) cancelGesture();
        if (typeof event.generation === 'number') model?.setGeneration(event.generation);
        if (event.name === 'tool.active.changed' || event.name === 'tool.pen.changed') refreshTool();
        if (event.name === 'colour.current.changed') {
          const c = (event.payload as Record<string, unknown> | null)?.['colourId'];
          if (typeof c === 'string') currentColour = c;
          refreshStyle();
        }
        if (event.name === 'shell.view') {
          const p = event.payload as Record<string, unknown> | null;
          const m = asAffine(p?.['drawingToView']);
          if (p?.['viewId'] === viewId && m) {
            d2v = m;
            viewVersion++;
            model?.setBucket(bucketOf(scaleOf(m)));
            refreshStyle();
          }
          return;
        }
        model?.markDirty();
      })
      .then((u) => {
        if (stopped) u();
        else unlisten = u;
      });

    const tick = () => {
      if (stopped) return;
      raf = requestAnimationFrame(tick);
      if (!model || !renderer) return;
      model.frame();
      const scene = model.scene;
      playAgent();
      if (
        clearOverlayAt &&
        (model.version > clearOverlayAt.version || performance.now() > clearOverlayAt.deadline)
      ) {
        overlay = null;
        clearOverlayAt = null;
      }
      if (scene && scene.colours !== lastColours) {
        renderer.setColours(scene.colours);
        lastColours = scene.colours;
      }
      const staticDirty = model.version !== drawnVersion || viewVersion !== drawnView;
      if (staticDirty) {
        grid = null;
        if (scene?.grid.visible) {
          grid = new ViewTris();
          gridGeometry(scene.grid, d2v, size.width, size.height, grid);
        }
        drawnVersion = model.version;
        drawnView = viewVersion;
      }
      renderer.frame({
        scene,
        drawingToView: d2v,
        width: size.width,
        height: size.height,
        dpr,
        staticDirty,
        grid,
        overlay,
      });
      const next = JSON.stringify({
        backend: renderer.kind,
        generation: engine.generation(),
        drawings: [...model.resident.entries()].map(([id, r]) => ({ id, rev: r.rev, bucket: r.bucket })),
        failures: model.failures.slice(-3),
        gesture: lastGesture,
        ...(({ drawn, residentDrawings, residentBytes, lastError }) => ({
          drawn,
          residentDrawings,
          residentBytes,
          lastError,
        }))(renderer.stats()),
      });
      if (next !== summary) {
        summary = next;
        el.setAttribute('data-tt-value', next);
      }
    };
    raf = requestAnimationFrame(tick);

    // The input path (design note 2 §2): pointer events in view pixels (the canvas fills its view window).
    const sample = (e: PointerEvent | WheelEvent): DomSample => {
      const r = canvas.getBoundingClientRect();
      const pe = e as Partial<PointerEvent>;
      return {
        x: e.clientX - r.left,
        y: e.clientY - r.top,
        pressure: pe.pressure ?? 1,
        tiltX: pe.tiltX ?? 0,
        tiltY: pe.tiltY ?? 0,
        pointerType: pe.pointerType ?? 'mouse',
        button: e.button,
        buttons: e.buttons,
        shiftKey: e.shiftKey,
        altKey: e.altKey,
        ctrlKey: e.ctrlKey,
        metaKey: e.metaKey,
        timeStamp: e.timeStamp,
      };
    };
    const commit = (req: ReturnType<GestureCollector['up']>) => {
      if (!req) return;
      const before = model?.version ?? 0;
      void engine.gestureCommit(req).then((env) => {
        lastGesture =
          env.status === 'error'
            ? { status: 'error', code: env.error?.code ?? 'error' }
            : { status: env.status };
        // The overlay stays until the committed drawing is drawn (or briefly, when nothing changed).
        clearOverlayAt = {
          version: before,
          deadline: performance.now() + (env.status === 'error' ? 0 : 400),
        };
        if (env.status === 'error') overlay = null;
      });
    };
    // Up to this many of an agent gesture's events are shown per frame (its simulated time is not waited for: P3).
    const AGENT_EVENTS_PER_FRAME = 16;
    const playAgent = () => {
      if (!playing) {
        if (collector.active) return;
        const next = agentQueue.shift();
        if (!next) return;
        playing = { input: next, shown: 0, points: [] };
        reportGesture(true);
      }
      const events = Array.isArray(playing.input.request['events'])
        ? (playing.input.request['events'] as Record<string, unknown>[])
        : [];
      const end = Math.min(events.length, playing.shown + AGENT_EVENTS_PER_FRAME);
      for (let i = playing.shown; i < end; i++) {
        const e = events[i];
        if (e && typeof e['x'] === 'number' && typeof e['y'] === 'number')
          playing.points.push([e['x'], e['y']]);
      }
      playing.shown = end;
      overlay = gestureOverlay(playing.points, style);
      if (playing.shown < events.length) return;
      const done = playing;
      playing = null;
      reportGesture(false);
      const before = model?.version ?? 0;
      void engine
        .gestureCommit({
          ...(done.input.request as { view: string; events: unknown[] }),
          agentRequest: done.input.requestId,
        })
        .then((env) => {
          lastGesture =
            env.status === 'error'
              ? { status: 'error', code: env.error?.code ?? 'error' }
              : { status: env.status };
          clearOverlayAt = {
            version: before,
            deadline: performance.now() + (env.status === 'error' ? 0 : 400),
          };
        });
    };
    const onDown = (e: PointerEvent) => {
      if (playing) {
        // P2: the user's press wins; the agent's gesture is discarded before any command.
        void engine.gestureAbort(playing.input.requestId).catch(() => undefined);
        playing = null;
        overlay = null;
      }
      if (!collector.down(sample(e))) return;
      e.preventDefault();
      canvas.setPointerCapture(e.pointerId);
      clearOverlayAt = null;
      reportGesture(true);
      overlay = gestureOverlay(collector.points, style);
    };
    const onMove = (e: PointerEvent) => {
      if (!collector.active) return;
      const list = typeof e.getCoalescedEvents === 'function' ? e.getCoalescedEvents() : [];
      collector.move((list.length > 0 ? list : [e]).map(sample));
      overlay = gestureOverlay(collector.points, style);
    };
    const onUp = (e: PointerEvent) => {
      if (!collector.active) return;
      const req = collector.up(sample(e));
      if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
      reportGesture(false);
      commit(req);
    };
    const onCancel = () => {
      cancelGesture();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && collector.active) {
        e.preventDefault();
        cancelGesture();
      }
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      commit(collector.wheel(sample(e), e.deltaY));
    };
    canvas.addEventListener('pointerdown', onDown);
    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('pointerup', onUp);
    canvas.addEventListener('pointercancel', onCancel);
    canvas.addEventListener('lostpointercapture', onCancel);
    canvas.addEventListener('wheel', onWheel, { passive: false });
    window.addEventListener('keydown', onKey, true);
    refreshTool();
    let unlistenAgent: (() => void) | null = null;
    void engine
      .onAgentInput((input) => {
        if (input.viewId === viewId) agentQueue.push(input);
      })
      .then((u) => {
        if (stopped) u();
        else unlistenAgent = u;
      });

    onReady?.({
      setOverlay(t) {
        overlay = t;
      },
      mapping: () => d2v,
      renderer: () => renderer,
      model: () => model,
    });

    return () => {
      stopped = true;
      // A window closing mid-gesture commits nothing (D-7a); an agent gesture it was playing is aborted.
      cancelGesture();
      if (playing) void engine.gestureAbort(playing.input.requestId).catch(() => undefined);
      for (const q of agentQueue.splice(0)) void engine.gestureAbort(q.requestId).catch(() => undefined);
      unlistenAgent?.();
      reportPresence(false);
      canvas.removeEventListener('pointerdown', onDown);
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('pointerup', onUp);
      canvas.removeEventListener('pointercancel', onCancel);
      canvas.removeEventListener('lostpointercapture', onCancel);
      canvas.removeEventListener('wheel', onWheel);
      window.removeEventListener('keydown', onKey, true);
      cancelAnimationFrame(raf);
      ro?.disconnect();
      unlisten?.();
      onReady?.(null);
      renderer?.destroy();
    };
  }, [engine, viewId, onReady]);

  return (
    <div ref={host} className="view-canvas" data-tt-role="canvas" data-tt-id={`${viewId}.canvas`}>
      <canvas ref={canvasRef} className="view-canvas-surface" />
    </div>
  );
}
