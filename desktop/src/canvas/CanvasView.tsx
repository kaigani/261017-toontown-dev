// The interactive canvas of a Drawing or Camera view window (M4 C4.2b, C4.3b; design notes 1 and 2, camera-view note
// §6). One per window (D-7a): its own model (the pull loop), back end and frame loop. The canvas follows the kernel:
// the composition from `canvas_scene` (its `viewType` decides which view it draws), geometry from `canvas_load`, the
// drawing mapping from `shell.view` (Drawing view) and `draw.viewMapping.get` (both views), the Camera view's
// occurrence matrices and overlays from the scene answer (it never derives a mapping or a projection: CV3).
//
// The element's `data-tt-value` carries what the canvas shows (back end, generation, each resident drawing and its
// revision), so the UI tree (`inspect_ui`) and the running-application tests can check it.

import { useEffect, useRef } from 'react';

import { GENERATION_EVENT, type AgentInput, type EngineApi } from '../ipc';
import { stringifyWireJson } from '../ipc/wireJson';
import { bucketOf } from './bucket';
import { gridGeometry, IDENTITY, scaleOf, ViewTris, type Affine } from './geometry';
import {
  ACCENT,
  GestureCollector,
  gestureOverlay,
  gestureRecord,
  type DomSample,
  type OverlayStyle,
  type WireEvent,
} from './input';
import { selectBackend, startingBackend, startRenderer } from './backend';
import type { BackendKind } from './renderer';
import { fingerprint, HEIGHT as MEASURE_H, measureBackends, WIDTH as MEASURE_W } from './selftest';
import { createWebGL2 } from './webgl2';
import { createWebGPU } from './webgpu';
import { canvasLaunch } from './launch';
import { cameraGridMatrix, cameraOverlay } from './cameraOverlay';
import { CanvasMetrics } from './metrics';
import { CanvasModel } from './model';
import { selectionCursor, selectionOverlay, type SelectionOverlay } from './selection';
import type { Renderer } from './renderer';

export interface CanvasHandle {
  /** The overlay's triangles for the next frames (layer 9); `null` clears it. */
  setOverlay(t: ViewTris | null): void;
  /** The current drawing → view mapping. */
  mapping(): Affine;
  renderer(): Renderer | null;
  model(): CanvasModel | null;
}

/** How long the canvas must be quiet (no gesture, no redraw) before the back ends are measured (SI-136). */
const QUIET_MS = 30_000;

/** Two overlays as one (`a` under `b`); either may be absent. */
function combine(a: ViewTris | null, b: ViewTris | null): ViewTris | null {
  if (!a) return b;
  if (!b) return a;
  const t = new ViewTris();
  t.data.push(...a.data);
  for (const x of b.data) t.data.push(x);
  return t;
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
    // The selection overlay (layer 9; frame-loop note §7): rebuilt when the selection, the mapping or the tool changed.
    let selection: SelectionOverlay = selectionOverlay(null, null, '', IDENTITY);
    let selectionKey = '';
    let lastColours: unknown = null;
    // The Camera view (camera-view note §6): the scene tool and family (SI-148, SI-152: a new session runs the scene
    // tools, Select), the active view for click-to-activate (V9), the drawing mapping asked again per scene revision.
    let sceneTool = 'select';
    let sceneFamily = true;
    let activeView: string | null = null;
    let mappedVersion = -1;
    let cameraShown: ReturnType<typeof cameraOverlay>['shown'] | null = null;
    let summary = '';
    let raf = 0;
    let stopped = false;
    // Measurements (`--canvas-metrics`; plan §6, D-5): published in the summary at most twice a second.
    const launch = canvasLaunch();
    const metrics = launch.metrics ? new CanvasMetrics() : null;
    let metricsAt = 0;
    let fallback: string | null = null;
    // Input (design note 2 §2 and §6): one gesture at a time; its overlay stays until the committed revision is drawn.
    const collector = new GestureCollector(viewId);
    let style: OverlayStyle = { tool: 'select', ink: ACCENT, width: 1 };
    let penSizes = { brush: 3, eraser: 10 };
    let currentColour: string | null = null;
    let clearOverlayAt: { version: number; deadline: number } | null = null;
    let lastGesture: { status: string; code?: string } | null = null;
    // The last committed gesture's record (C42B-R1-03; SI-141 item 9), as JSON text: written once per commit and
    // spliced into the summary, so a long gesture is not serialised again every frame.
    let lastRecord = '';
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
    const refreshSceneTool = () => {
      void engine
        .run('anim.tool.get', {})
        .done.then((env) => {
          const t = (env.result as Record<string, unknown> | undefined)?.['tool'];
          if (typeof t === 'string') sceneTool = t;
          viewVersion++;
        })
        .catch(() => undefined);
    };
    const refreshActiveView = () => {
      void engine
        .run('app.state.get', {})
        .done.then((env) => {
          const v = (env.result as Record<string, unknown> | undefined)?.['activeView'];
          if (typeof v === 'string') activeView = v;
        })
        .catch(() => undefined);
    };
    const refreshTool = () => {
      void engine.run('tool.active.get', {}).done.then((env) => {
        const id = (env.result as Record<string, unknown> | undefined)?.['toolId'];
        if (typeof id === 'string' && id !== style.tool) {
          // Another tool discards a pending multi-click gesture (a Polyline between its clicks).
          if (!collector.pressed) cancelGesture();
          style = { ...style, tool: id };
        }
        collector.multi = style.tool === 'polyline';
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
    // The engine generation this window follows (D-7b): input of another generation is never played or committed.
    let followed = engine.generation();
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
    // The modal state is reported in order: an end is never overtaken by the start before it (C42B-R2-03).
    let reports: Promise<unknown> = Promise.resolve();
    const reportGesture = (on: boolean) => {
      const modal = on ? { kind: 'gesture' as const, title: style.tool } : null;
      reports = reports.then(() => engine.report({ modal })).catch(() => undefined);
    };
    const cancelGesture = () => {
      if (collector.cancel()) {
        overlay = null;
        reportGesture(false);
      }
    };
    // D-7b: a new engine generation discards every gesture in progress, the user's and the agent's (playing or
    // queued), with nothing committed; the host has already answered the agent's calls (the SI-120 refusal). Run on
    // the first event of the new generation and when the window adopts it, whichever comes first.
    const adoptGeneration = (generation: number) => {
      if (generation === followed) return;
      followed = generation;
      cancelGesture();
      if (playing) reportGesture(false);
      playing = null;
      agentQueue.splice(0);
      overlay = null;
      clearOverlayAt = null;
    };

    // The display preferences the canvas honours (01 §7.2, 06 §6.9): `display.antiAliasing` and
    // `display.smoothToolOverlays` live; `display.fastPencilRendering` at the next start (its "restart").
    const DISPLAY_PREFS = ['display.antiAliasing', 'display.smoothToolOverlays'];
    const refreshDisplayPrefs = () => {
      void engine
        .run('prefs.get', { keys: DISPLAY_PREFS })
        .done.then((env) => {
          const values = (env.result as { values?: Record<string, unknown> } | undefined)?.values ?? {};
          const aa = values['display.antiAliasing'];
          const smooth = values['display.smoothToolOverlays'];
          if (!renderer) return;
          if (typeof aa === 'boolean') renderer.setAntialias(aa);
          if (typeof smooth === 'boolean') renderer.setSmoothOverlays(smooth);
          viewVersion++;
        })
        .catch(() => undefined);
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

    // The back end (ADR-0007 rule set step 1; SI-136, SI-140): the launch flag, else the user's override
    // (`display.canvasBackend`), else this machine's measured selection, else WebGL2. Without an override or a
    // measurement for this machine (GPU, driver, webview), both back ends are measured once the canvas has been quiet
    // for a while, and the selection applies at the next start.
    let lastActivity = performance.now();
    let measureTimer: ReturnType<typeof setInterval> | null = null;
    const scheduleMeasurement = (fp: string) => {
      measureTimer = setInterval(() => {
        if (stopped || document.hidden || performance.now() - lastActivity < QUIET_MS) return;
        if (measureTimer) clearInterval(measureTimer);
        measureTimer = null;
        const start = async (kind: BackendKind) => {
          const c = document.createElement('canvas');
          c.width = MEASURE_W;
          c.height = MEASURE_H;
          return kind === 'webgpu' ? createWebGPU(c) : createWebGL2(c);
        };
        const env = {
          now: () => performance.now(),
          yieldFrame: () => new Promise<void>((r) => setTimeout(r, 0)),
        };
        void measureBackends(start, env).then((m) => {
          const selected = selectBackend(m) ?? '';
          const value = { fingerprint: fp, selected, cells: JSON.stringify(m) };
          void engine
            .run('prefs.set', { values: { 'display.canvasBackendMeasured': value } })
            .done.catch(() => undefined);
        });
      }, 1000);
    };
    const decide = async (): Promise<{ want: BackendKind; measure: string | null }> => {
      const env = await engine.run('prefs.get', {
        keys: ['display.canvasBackend', 'display.canvasBackendMeasured'],
      }).done;
      const values = (env.result as { values?: Record<string, unknown> } | undefined)?.values ?? {};
      const pref = values['display.canvasBackend'];
      const measured = values['display.canvasBackendMeasured'] as Record<string, unknown> | undefined;
      const fp = await fingerprint();
      const override = launch.backend ?? (pref === 'webgpu' || pref === 'webgl2' ? pref : null);
      const sameMachine = measured?.['fingerprint'] === fp;
      const sel = measured?.['selected'];
      const selected = sameMachine && (sel === 'webgpu' || sel === 'webgl2') ? sel : null;
      // An inconclusive measurement of this machine is not repeated until it changes (SI-136).
      return {
        want: startingBackend(override, selected),
        measure: override === null && !sameMachine ? fp : null,
      };
    };
    void decide()
      .catch(() => ({ want: startingBackend(launch.backend, null), measure: null }))
      .then(async ({ want, measure }) => {
        const started = await startRenderer(canvas, want);
        if (measure !== null) scheduleMeasurement(measure);
        return started;
      })
      .then((started) => {
        if (stopped) {
          started.renderer.destroy();
          return;
        }
        renderer = started.renderer;
        fallback = started.fallback;
        model = new CanvasModel(viewId, engine, started.renderer, { now: () => performance.now() });
        model.setGeneration(engine.generation());
        if (metrics) {
          model.onLoaded = (key, bytes, ms) => {
            metrics.load(key, bytes, ms);
          };
          model.onScene = (ms) => {
            metrics.scene(ms);
          };
        }
        model.setBucket(bucketOf(scaleOf(d2v)));
        refreshDisplayPrefs();
        // `display.fastPencilRendering` applies at the next start (01 §7.2 "restart"): read once, before any load.
        const m = model;
        void engine
          .run('prefs.get', { keys: ['display.fastPencilRendering'] })
          .done.then((env) => {
            const v = (env.result as { values?: Record<string, unknown> } | undefined)?.values?.[
              'display.fastPencilRendering'
            ];
            if (v === true) m.fastPencil = true;
          })
          .catch(() => undefined);
      })
      .catch((e: unknown) => {
        el.setAttribute(
          'data-tt-value',
          JSON.stringify({ error: e instanceof Error ? e.message : String(e) }),
        );
      });
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
        if (typeof event.generation === 'number') {
          adoptGeneration(event.generation);
          model?.setGeneration(event.generation);
        }
        if (event.name === 'tool.active.changed' || event.name === 'tool.pen.changed') refreshTool();
        // SI-148: the family chosen last runs in the Camera view.
        if (event.name === 'tool.active.changed') sceneFamily = false;
        if (event.name === 'anim.tool.changed') {
          sceneFamily = true;
          refreshSceneTool();
        }
        if (event.name === 'prefs.changed') {
          const keys = (event.payload as { keys?: unknown } | null)?.keys;
          if (Array.isArray(keys) && keys.some((k) => DISPLAY_PREFS.includes(String(k))))
            refreshDisplayPrefs();
        }
        if (event.name === 'colour.current.changed') {
          const c = (event.payload as Record<string, unknown> | null)?.['colourId'];
          if (typeof c === 'string') currentColour = c;
          refreshStyle();
        }
        if (event.name === 'shell.view') {
          const p = event.payload as Record<string, unknown> | null;
          refreshActiveView();
          if (p?.['viewId'] !== viewId) return;
          if (p['type'] === 'drawing') {
            const m = asAffine(p['drawingToView']);
            if (m) {
              d2v = m;
              viewVersion++;
              model?.setBucket(bucketOf(scaleOf(m)));
              refreshStyle();
            }
            // A Camera view switched back to the Drawing type: its composition changes too.
            if (model?.scene?.viewType === 'camera') model.markDirty();
            return;
          }
          // A Camera view's transform is part of its answer (`cameraToView`) and of its drawing mapping (C43A-R1-01).
          model?.markDirty();
          refreshMapping();
          return;
        }
        model?.markDirty();
      })
      .then((u) => {
        if (stopped) u();
        else unlisten = u;
      });

    const tick = (frameAt: number) => {
      if (stopped) return;
      raf = requestAnimationFrame(tick);
      if (!model || !renderer) return;
      const began = performance.now();
      model.frame();
      const scene = model.scene;
      playAgent();
      if (
        clearOverlayAt &&
        (model.settledVersion > clearOverlayAt.version || performance.now() > clearOverlayAt.deadline)
      ) {
        overlay = null;
        clearOverlayAt = null;
      }
      if (scene && scene.colours !== lastColours) {
        renderer.setColours(scene.colours);
        lastColours = scene.colours;
      }
      const camera = scene?.viewType === 'camera' ? scene : null;
      // The Camera view's drawing mapping depends on the composition (the current element, the frame, the camera):
      // asked again once per scene revision (C43A-R1-01).
      if (camera && model.version !== mappedVersion) {
        mappedVersion = model.version;
        refreshMapping();
      }
      const actingTool = camera && sceneFamily ? sceneTool : style.tool;
      const key = `${String(model.selectionVersion)}/${String(viewVersion)}/${String(model.version)}/${actingTool}`;
      if (key !== selectionKey) {
        selectionKey = key;
        if (camera) {
          const c = cameraOverlay(camera, size.width, size.height, sceneFamily ? sceneTool : null);
          selection = {
            tris: c.tris,
            shown: { objects: 0, handles: c.handles.length, points: 0, pivot: false },
            handles: c.handles,
          };
          cameraShown = c.shown;
        } else {
          selection = selectionOverlay(
            scene?.selection ?? null,
            scene?.drawingPivot ?? null,
            style.tool,
            d2v,
          );
          cameraShown = null;
        }
      }
      const staticDirty = model.version !== drawnVersion || viewVersion !== drawnView;
      if (staticDirty || collector.active || playing) lastActivity = performance.now();
      if (staticDirty) {
        grid = null;
        if (scene?.grid.visible) {
          // The Camera view's field guide lies on the camera frame (SI-153).
          const g = camera ? cameraGridMatrix(camera, scene.grid) : d2v;
          if (g) {
            grid = new ViewTris();
            gridGeometry(scene.grid, g, size.width, size.height, grid);
          }
        }
        drawnVersion = model.version;
        drawnView = viewVersion;
      }
      renderer.frame({
        scene,
        drawingToView: d2v,
        cameraToView: camera?.camera?.cameraToView ?? null,
        width: size.width,
        height: size.height,
        dpr,
        staticDirty,
        grid,
        // The selection under the gesture in progress (the drag's own feedback on top).
        overlay: combine(selection.tris, overlay),
      });
      if (metrics) {
        const now = performance.now();
        // The frame time is the interval between frames (the rAF timestamps); the work is this callback's.
        metrics.frame(frameAt, now - began, collector.active || playing !== null);
        if (overlay) metrics.inkDrawn(now);
        // Committed when the window shows the committed state: every drawing resident at its scene revision.
        metrics.drawn(model.settledVersion, now);
      }
      const publish = metrics !== null && began - metricsAt >= 500;
      if (publish) metricsAt = began;
      // Written losslessly: revisions are exact 64-bit integers (`bigint`).
      let next = stringifyWireJson({
        backend: renderer.kind,
        generation: engine.generation(),
        drawings: [...model.resident.entries()].map(([id, r]) => ({ id, rev: r.rev, bucket: r.bucket })),
        failures: model.failures.slice(-3),
        gesture: lastGesture,
        selection: selection.shown,
        viewType: scene?.viewType ?? null,
        ...(cameraShown ? { camera: cameraShown } : {}),
        ...(({ drawn, residentDrawings, residentBytes, lastError }) => ({
          drawn,
          residentDrawings,
          residentBytes,
          lastError,
        }))(renderer.stats()),
        ...(fallback ? { fallback } : {}),
        ...(metrics && publish ? { metrics: metrics.snapshot() } : {}),
      });
      // Between metric publications the summary keeps its last metrics, so it changes only when something did.
      if (metrics && !publish) return;
      if (lastRecord) next = `${next.slice(0, -1)},"lastGesture":${lastRecord}}`;
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
      const tool = style.tool;
      const mapping = d2v;
      void engine.gestureCommit(req).then((env) => {
        lastRecord = stringifyWireJson(gestureRecord('user', tool, mapping, req, env));
        metrics?.answered(performance.now());
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
    // An agent gesture is shown over at most about two seconds of frames, at least 16 events a frame (its simulated
    // time is not waited for: P3).
    const AGENT_EVENTS_PER_FRAME = 16;
    const AGENT_PLAYBACK_FRAMES = 120;
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
      const perFrame = Math.max(AGENT_EVENTS_PER_FRAME, Math.ceil(events.length / AGENT_PLAYBACK_FRAMES));
      const end = Math.min(events.length, playing.shown + perFrame);
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
      metrics?.released(performance.now(), model?.version ?? 0);
      const before = model?.version ?? 0;
      const request = done.input.request as { view: string; events: WireEvent[]; tool?: unknown };
      const mapping = d2v;
      void engine.gestureCommit({ ...request, agentRequest: done.input.requestId }).then((env) => {
        const tool = typeof request.tool === 'string' ? request.tool : style.tool;
        lastRecord = stringifyWireJson(gestureRecord('agent', tool, mapping, request, env));
        metrics?.answered(performance.now());
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
      // V9 (answer 4; SI-142): a press in a view window that is not the active view activates it first (user origin,
      // UI state, no undo step); the same press is then the tool's press.
      if (activeView !== null && activeView !== viewId) {
        activeView = viewId;
        void engine.run('view.active.set', { viewId }).done.catch(() => undefined);
      }
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
      if (!collector.active) {
        // Hovering: the Select tool's cursor over its handles and inside the box (frame-loop note §7.3).
        const b = style.tool === 'select' ? (model?.scene?.selection?.bounds ?? null) : null;
        const p = sample(e);
        canvas.style.cursor = selectionCursor(selection.handles, b, d2v, [p.x, p.y]);
        return;
      }
      // Input-to-ink counts from the sample's arrival at the page (plan §6: "sample received").
      metrics?.sample(performance.now());
      const list = typeof e.getCoalescedEvents === 'function' ? e.getCoalescedEvents() : [];
      collector.move((list.length > 0 ? list : [e]).map(sample));
      overlay = gestureOverlay(collector.points, style);
    };
    const onUp = (e: PointerEvent) => {
      if (!collector.pressed) return;
      const req = collector.up(sample(e));
      if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
      // A Polyline between its clicks stays one gesture in progress (the modal state stays reported).
      if (!req) return;
      metrics?.released(performance.now(), model?.version ?? 0);
      reportGesture(false);
      commit(req);
    };
    const onDouble = (e: MouseEvent) => {
      const req = collector.doubleClick(sample(e as PointerEvent));
      if (!req) return;
      reportGesture(false);
      commit(req);
    };
    const onCancel = () => {
      // Only a pressed gesture loses anything to a lost capture: a Polyline between clicks holds no capture.
      if (collector.pressed) cancelGesture();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && collector.active) {
        e.preventDefault();
        cancelGesture();
      }
      // Enter ends an open Polyline (02 §7.2.5).
      if (e.key === 'Enter' && collector.active && !collector.pressed) {
        const req = collector.end();
        if (req) {
          e.preventDefault();
          reportGesture(false);
          commit(req);
        }
      }
    };
    // The window losing focus between a Polyline's clicks discards it (no capture is held between clicks).
    const onBlur = () => {
      if (collector.active && !collector.pressed) cancelGesture();
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
    canvas.addEventListener('dblclick', onDouble);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('blur', onBlur);
    const onGeneration = (e: Event) => {
      const g = (e as CustomEvent<unknown>).detail;
      if (typeof g === 'number') adoptGeneration(g);
    };
    window.addEventListener(GENERATION_EVENT, onGeneration);
    refreshTool();
    refreshSceneTool();
    refreshActiveView();
    let unlistenAgent: (() => void) | null = null;
    void engine
      .onAgentInput((input) => {
        // Routed on another engine generation than the one this window follows: dropped (D-7b).
        if (input.viewId === viewId && input.generation === engine.generation()) agentQueue.push(input);
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
      if (measureTimer) clearInterval(measureTimer);
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
      canvas.removeEventListener('dblclick', onDouble);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('blur', onBlur);
      window.removeEventListener(GENERATION_EVENT, onGeneration);
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
