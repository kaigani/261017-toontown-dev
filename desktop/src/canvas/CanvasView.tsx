// The interactive canvas of a Drawing or Camera view window (M4 C4.2b, C4.3b; design notes 1 and 2, camera-view note
// §6). One per window (D-7a): its own model (the pull loop), back end and frame loop. The canvas follows the kernel:
// the composition from `canvas_scene` (its `viewType` decides which view it draws), geometry from `canvas_load`, the
// drawing mapping from `shell.view` (Drawing view) and `draw.viewMapping.get` (both views), the Camera view's
// occurrence matrices and overlays from the scene answer (it never derives a mapping or a projection: CV3).
//
// The element's `data-tt-value` carries what the canvas shows (back end, generation, each resident drawing and its
// revision), so the UI tree (`inspect_ui`) and the running-application tests can check it.

import { useEffect, useRef, useState } from 'react';

import { GENERATION_EVENT, type AgentInput, type EngineApi } from '../ipc';
import { stringifyWireJson } from '../ipc/wireJson';
import { bucketOf } from './bucket';
import {
  apply,
  compose,
  gridGeometry,
  IDENTITY,
  invert,
  mapTris,
  scaleOf,
  ViewTris,
  type Affine,
} from './geometry';
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
import { activeViewOf } from './activeView';
import { decodeDrawing } from './format';
import {
  answerMadeStep,
  answerRevision,
  drawnInView,
  GestureLifecycle,
  type ClearAt,
  type CommitAnswer,
  type GestureToken,
  type GestureView,
} from './gestureLifecycle';
import { gestureOrder } from './gestureOrder';
import { drawingPreview, grabberPreview, scenePreview, type Box } from './preview';
import { CanvasMetrics } from './metrics';
import { CanvasModel } from './model';
import { ContextMenu } from '../shell/ContextMenu';
import { menuAt, type MenuAt } from '../shell/contextMenuModel';
import { isMac } from '../keys/sequence';
import { sceneFamilyChosen } from './toolFamily';
import { DRAWING_MENU, SCENE_MENU } from './viewMenus';
import { selectionCursor, selectionOverlay, type SelectionOverlay } from './selection';
import {
  CLICK_SLOP_PX,
  contourCursor,
  contourHitAt,
  contourOverlay,
  contourPreview,
  drawContourPreview,
  drawingDelta,
  type ContourOverlay,
  type ContourPress,
} from './contour';
import {
  toolBoxCursor,
  toolBoxOverlay,
  toolHandleAt,
  toolPreview,
  type PreviewMap,
  type ToolPress,
} from './envelope';
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

/**
 * The mounted canvases of each view, each with its own presence report. The host knows which window shows a view only
 * from these reports, keyed by the view: when the dock re-creates a view's panel (a workspace switch) the new canvas can
 * mount before the old one unmounts, and the old one's "gone" would then wipe the new one's presence (CI run
 * 44401155529: "no window shows `drawingView`" with the Drawing view on screen). A canvas reports "gone" only when no
 * other canvas of its view is mounted; otherwise one that is reports itself again.
 */
const LIVE_CANVASES = new Map<string, Set<() => void>>();

/** A secondary press dragged farther than this (CSS pixels) opens no menu at its release (design note 1 §9). */
const MENU_DRAG_PX = 4;

export function CanvasView({
  engine,
  viewId,
  viewType = null,
  onReady,
}: {
  engine: EngineApi;
  viewId: string;
  /** The window's type from `view.list` (the view panel's), used until the scene answer gives it. */
  viewType?: string | null;
  onReady?: (h: CanvasHandle | null) => void;
}) {
  const host = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  // The view's context menu (design note 1 §9): the family the view runs when it opened.
  const [menu, setMenu] = useState<{ at: MenuAt; family: 'drawing' | 'scene' } | null>(null);
  const openMenu = useRef(setMenu);
  const windowType = useRef(viewType);
  useEffect(() => {
    windowType.current = viewType;
  }, [viewType]);

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
    // D-53 (C43B-R4-01; preview note PV5): the view the current gesture was made in. Its preview and overlay are drawn
    // only there, with that view's mapping: not under the Camera type after a Drawing-type press, again on return.
    let owner: GestureView | null = null;
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
    // The family chosen last in this window, even before this canvas mounted (SI-148).
    let sceneFamily = sceneFamilyChosen();
    // Where a secondary press went down (design note 1 §9); the menu opens at its release unless it was dragged.
    let secondary: { x: number; y: number } | null = null;
    const mac = isMac();
    let activeView: string | null = null;
    let mappedVersion = -1;
    let cameraShown: ReturnType<typeof cameraOverlay>['shown'] | null = null;
    // The live transform preview (preview note 8): the drag's press, the kind of preview it takes, its view-pixel
    // affine, and the selection's two overlay parts (only the selection moves with it). The staging selection's ids
    // name the Camera-view occurrences that move.
    let selectedIds: ReadonlySet<string> = new Set();
    let press: { p: [number, number]; constrain: boolean; alternate: boolean; tempSelect: boolean } | null =
      null;
    let previewKind: 'scene' | 'drawing' | 'grab' | 'contour' | 'envelope' | 'reposition' | null = null;
    // Perspective and Reposition All (design note 4 §2, §6): the press on the scene answer's `toolBox`, the press
    // mapping, and the drag's map in drawing units.
    let toolPress: (ToolPress & { d2v: Affine; locked: boolean }) | null = null;
    let toolMap: PreviewMap | null = null;
    let toolShown: { tool: string; locked: boolean; handles: number } | null = null;
    let hoverAt: [number, number] | null = null;
    // The Contour Editor (design note 3 §4, §6): its overlay from the scene answer's `contours`, the press's target and
    // mapping, and the last preview's changed anchors in view pixels (for the e2e, PV2) and recomputed segments.
    let contourShown: ContourOverlay['shown'] | null = null;
    let contourPress: (ContourPress & { d2v: Affine; dragged: boolean }) | null = null;
    let contourDrawn: { anchors: [number, number][]; segments: number } | null = null;
    let preview: Affine | null = null;
    let overlayParts: { frame: ViewTris | null; selection: ViewTris | null } = {
      frame: null,
      selection: null,
    };
    // The Drawing view's Select preview draws the current drawing as its rest and its selected objects, two payloads
    // loaded at the press (preview note §3); `ready` counts the parts resident.
    let parts: { drawingId: string; rest: string; selection: string; ready: number } | null = null;
    let summary = '';
    // What the summary was built from: it is rebuilt only when one of these changed (a 1,000-drawing summary
    // stringified every frame cost milliseconds per frame).
    let summaryKey = '';
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
    let lastGesture: { status: string; code?: string } | null = null;
    // The user's gesture in progress or last released (C43B-R1S-06): its commit's answer acts only while it is current.
    let gestureToken: GestureToken | null = null;
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
          activeView = activeViewOf(env.result);
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
    let playing: {
      input: AgentInput;
      shown: number;
      points: [number, number][];
      token: GestureToken;
    } | null = null;
    // The engine generation this window follows (D-7b): input of another generation is never played or committed.
    // Each gesture is tagged by gesture and generation (C43B-R1S-06): a commit's answer clears the overlay and preview
    // only while its gesture is the current one; a new generation and unmounting drop the preview's resources at once.
    const life = new GestureLifecycle(engine.generation(), () => {
      overlay = null;
      endPreview();
    });
    const reportPresence = (present: boolean) => {
      if (!present && (LIVE_CANVASES.get(viewId)?.size ?? 0) > 0) {
        // Another canvas of this view is mounted: it reports itself instead of this one's "gone".
        LIVE_CANVASES.get(viewId)?.values().next().value?.();
        return;
      }
      const r = canvas.getBoundingClientRect();
      void engine
        .canvasPresence(
          viewId,
          present ? { x: r.left, y: r.top, w: r.width, h: r.height } : null,
          window.devicePixelRatio || 1,
        )
        .catch(() => undefined);
    };
    // The modal state and the commits go out in order: an end is never overtaken by the start before it
    // (C42B-R2-03), and a gesture's end follows its commit's answer (SI-157).
    const order = gestureOrder(engine);
    /** The tool a gesture of this view is made with: in a Camera view running the scene family, the scene tool
     *  (05 §5.0); otherwise the drawing tool. */
    const viewNow = (): GestureView => {
      const sc = model?.scene ?? null;
      if (sc?.viewType === 'camera') return { viewType: 'camera', mapping: sc.camera?.cameraToView ?? null };
      return { viewType: sc?.viewType ?? null, mapping: d2v };
    };
    const actingTool = () => (model?.scene?.viewType === 'camera' && sceneFamily ? sceneTool : style.tool);
    const reportGesture = (on: boolean) => {
      order.report(on ? { kind: 'gesture', title: actingTool() } : null);
    };
    const dropParts = () => {
      if (parts && renderer) {
        renderer.drop(parts.rest);
        renderer.drop(parts.selection);
      }
      parts = null;
    };
    const endPreview = () => {
      if (preview || contourDrawn) viewVersion++;
      preview = null;
      previewKind = null;
      press = null;
      contourPress = null;
      contourDrawn = null;
      if (toolMap) viewVersion++;
      toolPress = null;
      toolMap = null;
      dropParts();
    };
    /** Perspective or Reposition All runs in this view now (in the Camera view, the drawing family). */
    const boxTool = (): 'perspective' | 'repositionAll' | null =>
      (style.tool === 'perspective' || style.tool === 'repositionAll') &&
      !(model?.scene?.viewType === 'camera' && sceneFamily)
        ? style.tool
        : null;
    /** The Contour Editor runs in this view now: its tool is active and, in the Camera view, the drawing family. */
    const contourActive = () =>
      style.tool === 'contourEditor' && !(model?.scene?.viewType === 'camera' && sceneFamily);
    /** Loads the current drawing's two parts at its scene revision (preview note §3). */
    const loadParts = () => {
      const m = model;
      const e = m?.scene?.entries.find((x) => x.role === 'current' && !x.outline);
      if (!m || !e || !m.scene?.selection?.bounds) return;
      const mine = {
        drawingId: e.drawingId,
        rest: `${e.drawingId}#rest`,
        selection: `${e.drawingId}#selection`,
        ready: 0,
      };
      parts = mine;
      const generation = BigInt(engine.generation());
      for (const [part, key] of [
        ['rest', mine.rest],
        ['selection', mine.selection],
      ] as const) {
        void engine
          .canvasLoad(e.drawingId, m.currentBucket, false, m.fastPencil, part)
          .then((bytes) => {
            // Only for the drag that asked, at the revision and generation shown (T1, T6).
            if (parts !== mine || !renderer) return;
            const d = decodeDrawing(bytes);
            if (d.rev !== e.rev || d.generation !== generation) return;
            renderer.upload(key, d);
            mine.ready++;
            viewVersion++;
          })
          .catch(() => undefined);
      }
    };
    const cancelGesture = () => {
      if (collector.cancel()) {
        overlay = null;
        // Escape restores the state before the drag (05 line 399; PV1): the preview is dropped, nothing was sent.
        endPreview();
        reportGesture(false);
      }
    };
    const refreshSelectedIds = () => {
      void engine
        .run('edit.selection.get', { context: 'staging' })
        .done.then((env) => {
          const ids = (env.result as Record<string, unknown> | undefined)?.['ids'];
          if (Array.isArray(ids))
            selectedIds = new Set(ids.filter((x): x is string => typeof x === 'string'));
        })
        .catch(() => undefined);
    };
    // D-7b: a new engine generation discards every gesture in progress, the user's and the agent's (playing or
    // queued), with nothing committed; the host has already answered the agent's calls (the SI-120 refusal). Run on
    // the first event of the new generation and when the window adopts it, whichever comes first.
    const adoptGeneration = (generation: number) => {
      if (generation === life.generation) return;
      cancelGesture();
      if (playing) reportGesture(false);
      playing = null;
      agentQueue.splice(0);
      // Discards the overlay and the preview with its parts even after the release, while a commit is unanswered.
      life.adopt(generation);
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
    // This canvas among its view's mounted canvases (presence above); it reports once it has a size.
    const reportMounted = () => {
      if (size.width >= 1 && size.height >= 1) reportPresence(true);
    };
    const mounted = LIVE_CANVASES.get(viewId) ?? new Set<() => void>();
    mounted.add(reportMounted);
    LIVE_CANVASES.set(viewId, mounted);

    let unlisten: (() => void) | null = null;
    void engine
      .onEvent((event) => {
        if (typeof event.generation === 'number') {
          adoptGeneration(event.generation);
          model?.setGeneration(event.generation);
        }
        if (event.name === 'tool.active.changed' || event.name === 'tool.pen.changed') refreshTool();
        // The arrangement changed (a workspace switch, a panel moved): the canvas may have moved without being resized,
        // which the resize observer does not see; its presence is reported again once laid out (CI run 44442647850:
        // "no window shows `drawingView`" after the Drawing workspace was selected, the view on screen).
        if (event.name === 'workspace.changed') {
          requestAnimationFrame(() => {
            if (!stopped && size.width >= 1 && size.height >= 1) reportPresence(true);
          });
        }
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
        if (event.name === 'edit.selection.changed') {
          const p = event.payload as Record<string, unknown> | null;
          if (p?.['context'] === 'staging' && Array.isArray(p['ids']))
            selectedIds = new Set(p['ids'].filter((x): x is string => typeof x === 'string'));
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
      // The committed state is drawn (or the commit was refused): the overlay and preview give way to it (preview
      // note §4).
      life.frame(
        { settledVersion: model.settledVersion, settledDocumentRev: model.settledDocumentRev },
        performance.now(),
      );
      if (scene && scene.colours !== lastColours) {
        renderer.setColours(scene.colours);
        lastColours = scene.colours;
      }
      const camera = scene?.viewType === 'camera' ? scene : null;
      // D-53: the gesture's preview and overlay, as drawn in this view now (none under another view type or mapping).
      const here = drawnInView(owner, viewNow());
      const pv = here ? preview : null;
      const ov = here ? overlay : null;
      // The Camera view's drawing mapping depends on the composition (the current element, the frame, the camera):
      // asked again once per scene revision (C43A-R1-01).
      if (camera && model.version !== mappedVersion) {
        mappedVersion = model.version;
        refreshMapping();
      }
      const key = `${String(model.selectionVersion)}/${String(viewVersion)}/${String(model.version)}/${actingTool()}`;
      if (key !== selectionKey) {
        selectionKey = key;
        if (camera) {
          const c = cameraOverlay(camera, size.width, size.height, sceneFamily ? sceneTool : null);
          selection = {
            tris: c.tris,
            shown: { objects: 0, handles: c.handles.length, points: 0, pivot: false },
            handles: c.handles,
          };
          overlayParts = { frame: c.frame, selection: c.selection };
          cameraShown = c.shown;
        } else {
          selection = selectionOverlay(
            scene?.selection ?? null,
            scene?.drawingPivot ?? null,
            style.tool,
            d2v,
          );
          overlayParts = { frame: null, selection: selection.tris };
          cameraShown = null;
        }
        toolShown = null;
        const tb = scene?.toolBox ?? null;
        if (boxTool() && tb?.tool === boxTool()) {
          const map = here ? toolMap : null;
          const handles = toolBoxOverlay(tb.box, d2v, tb.locked, map);
          toolShown = { tool: tb.tool, locked: tb.locked, handles: 8 };
          selection = { ...selection, tris: combine(selection.tris, handles) };
          overlayParts = { frame: overlayParts.frame, selection: selection.tris };
        }
        contourShown = null;
        if (contourActive() && scene?.contours) {
          const c = contourOverlay(scene.contours, d2v);
          contourShown = c.shown;
          if (camera) selection = { ...selection, tris: combine(selection.tris, c.tris) };
          else {
            selection = { tris: c.tris, shown: { ...selection.shown, objects: 0 }, handles: [] };
            overlayParts = { frame: null, selection: selection.tris };
          }
        }
      }
      const staticDirty = model.version !== drawnVersion || viewVersion !== drawnView;
      if (staticDirty || collector.active || playing) lastActivity = performance.now();
      if (staticDirty) {
        grid = null;
        if (scene?.grid.visible) {
          // The Camera view's field guide lies on the camera frame (SI-153).
          const g0 = camera ? cameraGridMatrix(camera, scene.grid) : d2v;
          // The Grabber's preview pans everything drawn, the grid included (preview note §3).
          const g = g0 && previewKind === 'grab' && pv ? compose(pv, g0) : g0;
          if (g) {
            grid = new ViewTris();
            gridGeometry(scene.grid, g, size.width, size.height, grid);
          }
        }
        drawnVersion = model.version;
        drawnView = viewVersion;
      }
      // The preview (preview note §3): the Grabber pans both mappings; a scene-tool drag moves the selected
      // occurrences; the selection's overlay moves with a scene or Select drag.
      const grab = previewKind === 'grab' && pv ? pv : null;
      const c2v = camera?.camera?.cameraToView ?? null;
      let overlayNow = selection.tris;
      if (pv && (previewKind === 'scene' || previewKind === 'drawing') && overlayParts.selection) {
        overlayNow = combine(overlayParts.frame, mapTris(overlayParts.selection, pv));
      }
      renderer.frame({
        scene,
        drawingToView: grab ? compose(grab, d2v) : d2v,
        cameraToView: grab && c2v ? compose(grab, c2v) : c2v,
        preview: previewKind === 'scene' && pv ? { transform: pv, elements: selectedIds } : null,
        split:
          previewKind === 'drawing' && pv && parts?.ready === 2
            ? { drawingId: parts.drawingId, rest: parts.rest, selection: parts.selection, transform: pv }
            : previewKind === 'envelope' && here && toolMap && parts?.ready === 2
              ? {
                  drawingId: parts.drawingId,
                  rest: parts.rest,
                  selection: parts.selection,
                  transform: IDENTITY,
                  map: toolMap,
                }
              : null,
        moved:
          previewKind === 'reposition' && here && toolMap && toolPress && !toolPress.locked
            ? (() => {
                const e = scene?.entries.find((x) => x.role === 'current' && !x.outline);
                return e ? { drawingId: e.drawingId, map: toolMap } : null;
              })()
            : null,
        width: size.width,
        height: size.height,
        dpr,
        staticDirty,
        grid,
        // The selection under the gesture in progress (the drag's own feedback on top).
        overlay: combine(overlayNow, ov),
      });
      if (metrics) {
        const now = performance.now();
        // The frame time is the interval between frames (the rAF timestamps); the work is this callback's.
        metrics.frame(frameAt, now - began, collector.active || playing !== null);
        // Input-to-ink, and for a previewed drag its feedback: the frame that draws the samples so far (plan §6).
        if (ov || pv) metrics.inkDrawn(now);
        // Committed when the window shows the committed state: every drawing resident at its scene revision.
        metrics.drawn(model.settledVersion, now);
      }
      const publish = metrics !== null && began - metricsAt >= 500;
      if (publish) metricsAt = began;
      const stats = renderer.stats();
      const inputs = JSON.stringify([
        renderer.kind,
        engine.generation(),
        model.residentVersion,
        model.failures.length,
        model.failures[model.failures.length - 1] ?? null,
        lastGesture,
        selection.shown,
        scene?.viewType ?? null,
        previewKind,
        pv,
        ov !== null,
        cameraShown,
        toolShown,
        toolMap,
        contourShown,
        contourDrawn,
        stats.drawn,
        stats.residentDrawings,
        stats.residentBytes,
        stats.lastError,
        fallback ?? null,
        lastRecord.length,
      ]);
      if (!publish && inputs === summaryKey) return;
      summaryKey = inputs;
      // Written losslessly: revisions are exact 64-bit integers (`bigint`).
      let next = stringifyWireJson({
        backend: renderer.kind,
        generation: engine.generation(),
        drawings: [...model.resident.entries()].map(([id, r]) => ({ id, rev: r.rev, bucket: r.bucket })),
        failures: model.failures.slice(-3),
        gesture: lastGesture,
        selection: selection.shown,
        viewType: scene?.viewType ?? null,
        // The preview's affine and corners of the selection box under it, for the e2e (PV2).
        ...(pv ? { preview: { kind: previewKind, transform: pv } } : {}),
        // Whether a gesture's overlay is drawn in this view (a drawing tool's stroke until its commit is drawn; rulings
        // D-50, D-53: not under another view type or mapping).
        overlay: ov !== null,
        ...(cameraShown ? { camera: cameraShown } : {}),
        // The Contour Editor's overlay and, during a drag, its preview's changed anchors in view pixels (PV2).
        ...(contourShown ? { contours: contourShown } : {}),
        // Perspective and Reposition All: the box's handles, and during a drag the preview's map (drawing units).
        ...(toolShown ? { toolBox: toolShown } : {}),
        ...(toolMap && here ? { toolPreview: toolMap } : {}),
        ...(contourDrawn && here ? { contourPreview: contourDrawn } : {}),
        ...(({ drawn, residentDrawings, residentBytes, lastError }) => ({
          drawn,
          residentDrawings,
          residentBytes,
          lastError,
        }))(stats),
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
    /** When a commit's answer lets the overlay and preview go (C43B-R2S-02, C43B-R3S-02; preview note §4): after a step,
     *  a scene resident at the revision the commit answered with; without one, a short deadline. */
    const clearAt = (env: CommitAnswer): ClearAt => {
      const step = answerMadeStep(env);
      return {
        documentRev: step ? answerRevision(env) : null,
        deadline: step ? null : performance.now() + (env.status === 'error' ? 0 : 400),
        afterVersion: model?.version ?? 0,
      };
    };
    const commit = (req: ReturnType<GestureCollector['up']>) => {
      if (!req) return;
      // The record names the tool that acted (a scene tool in the scene family), as the modal state does.
      const tool = actingTool();
      const mapping = d2v;
      const token = gestureToken;
      void order.commitThenEnd(req).then((env) => {
        lastRecord = stringifyWireJson(gestureRecord('user', tool, mapping, req, env));
        metrics?.answered(performance.now());
        lastGesture =
          env.status === 'error'
            ? { status: 'error', code: env.error?.code ?? 'error' }
            : { status: env.status };
        // The overlay stays until the committed drawing is drawn (C43B-R2S-02: no deadline after a step), briefly when
        // nothing changed, and not at all after a refusal; an answer for a gesture that is no longer current (a later
        // press, another generation, a closed window) clears nothing.
        const armed = token !== null && life.answered(token, clearAt(env));
        if (armed && env.status === 'error') overlay = null;
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
        // The agent's gesture is a gesture of its own from its first frame: an earlier commit's answer does not clear
        // its overlay, and a user press makes its own answer obsolete.
        playing = { input: next, shown: 0, points: [], token: life.begin() };
        owner = viewNow();
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
      metrics?.released(performance.now(), model?.version ?? 0);
      const request = done.input.request as { view: string; events: WireEvent[]; tool?: unknown };
      const mapping = d2v;
      const token = done.token;
      void order.commitThenEnd({ ...request, agentRequest: done.input.requestId }).then((env) => {
        const tool = typeof request.tool === 'string' ? request.tool : style.tool;
        lastRecord = stringifyWireJson(gestureRecord('agent', tool, mapping, request, env));
        metrics?.answered(performance.now());
        lastGesture =
          env.status === 'error'
            ? { status: 'error', code: env.error?.code ?? 'error' }
            : { status: env.status };
        life.answered(token, clearAt(env));
      });
    };
    const onDown = (e: PointerEvent) => {
      // V9 (answer 4; SI-142): a press in a view window that is not the active view activates it first (user origin,
      // UI state, no undo step); the same press is then the tool's press.
      if (activeView !== null && activeView !== viewId) {
        activeView = viewId;
        void engine.run('view.active.set', { viewId }).done.catch(() => undefined);
      }
      // The secondary click is the context menu's, never a tool gesture (design note 1 §9): a secondary press, and on
      // macOS a Control-press, is remembered and opens the menu at release unless it was dragged (SI-172).
      if (e.button === 2 || (mac && e.button === 0 && e.ctrlKey)) {
        secondary = { x: e.clientX, y: e.clientY };
        return;
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
      // A new gesture: an earlier commit still unanswered no longer clears anything (its overlay gives way here).
      gestureToken = life.begin();
      owner = viewNow();
      reportGesture(true);
      overlay = gestureOverlay(collector.points, style);
      // Which preview this drag takes (preview note §3).
      const s = sample(e);
      endPreview();
      press = {
        p: [s.x, s.y],
        constrain: e.shiftKey,
        alternate: e.altKey,
        // SI-172: on macOS Control-click is the context menu, so Command alone is the temporary-select modifier.
        tempSelect: mac ? e.metaKey : e.ctrlKey || e.metaKey,
      };
      const inCamera = model?.scene?.viewType === 'camera';
      if (inCamera && sceneFamily && (sceneTool === 'select' || sceneTool === 'transform'))
        previewKind = 'scene';
      else if (style.tool === 'grabber') previewKind = 'grab';
      else if (boxTool()) {
        // Perspective and Reposition All (design note 4 §5): the press is classified on the kernel's box; the release's
        // command is the kernel's. A refused command (grey handles) draws no preview.
        const tool = boxTool() ?? 'perspective';
        const tb = model?.scene?.toolBox ?? null;
        const box = tb?.tool === tool ? tb.box : null;
        toolPress = {
          tool,
          box,
          handle: box ? toolHandleAt(box, d2v, [s.x, s.y], tool === 'repositionAll') : null,
          alternate: e.altKey,
          constrain: e.shiftKey,
          mapping: tb?.mapping ?? 'auto',
          d2v,
          locked: tb?.locked ?? false,
        };
        previewKind = tool === 'perspective' ? 'envelope' : 'reposition';
        if (tool === 'perspective' && toolPress.handle && !toolPress.locked) loadParts();
      } else if (contourActive()) {
        // The press classifies its target with the kernel's rule (`contourHitAt`); the release's command is the
        // kernel's. A locked drawing draws no preview (the refusal is the kernel's).
        previewKind = 'contour';
        const c = model?.scene?.contours ?? null;
        contourPress = { target: contourHitAt(c, d2v, [s.x, s.y]), alternate: e.altKey, d2v, dragged: false };
      } else if (!inCamera && style.tool === 'select') {
        previewKind = 'drawing';
        loadParts();
      }
    };
    /** The preview's affine for the drag from the press to `now` (view pixels), or null. */
    const previewAt = (now: [number, number]): Affine | null => {
      const scene = model?.scene;
      if (!press || !scene) return null;
      if (previewKind === 'grab') return grabberPreview(press.p, now);
      if (previewKind === 'drawing') {
        const b = scene.selection?.bounds ?? null;
        return drawingPreview(b, scene.drawingPivot, d2v, press.p, now, press.constrain);
      }
      if (previewKind !== 'scene' || !scene.camera || !scene.overlays?.selectionUnion) return null;
      const v = scene.camera.cameraToView;
      const u = scene.overlays.selectionUnion;
      const c = [apply(v, u[0], u[1]), apply(v, u[2], u[1]), apply(v, u[2], u[3]), apply(v, u[0], u[3])];
      const xs = c.map((q) => q[0]);
      const ys = c.map((q) => q[1]);
      const box: Box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
      // 05 §5.0 lines 393-394: the temporary-select modifier gives Select; the alternate modifier with Transform the
      // Skew tool (SI-151).
      const tool = press.tempSelect ? 'select' : sceneTool;
      const skew = tool === 'transform' && press.alternate;
      const p0 = scene.overlays.pivots[0];
      const pivot = tool === 'transform' && p0 ? apply(v, p0[0], p0[1]) : null;
      // One selected node: the preview turns, scales and skews about its pivot, as the commit does (SI-150).
      const anchor = scene.overlays.pivots.length === 1 && p0 ? apply(v, p0[0], p0[1]) : null;
      return scenePreview(box, pivot, press.p, now, press.constrain, skew, anchor);
    };
    const onMove = (e: PointerEvent) => {
      if (!collector.active) {
        const p = sample(e);
        hoverAt = [p.x, p.y];
        const bt = boxTool();
        if (bt) {
          const tb = model?.scene?.toolBox ?? null;
          canvas.style.cursor = toolBoxCursor(
            bt,
            tb?.tool === bt ? tb.box : null,
            d2v,
            [p.x, p.y],
            e.altKey,
            tb?.locked ?? false,
          );
          return;
        }
        if (contourActive()) {
          // The Contour Editor's cursor (design note 3 §4).
          const temp = mac ? e.metaKey : e.ctrlKey || e.metaKey;
          canvas.style.cursor = contourCursor(model?.scene?.contours ?? null, d2v, [p.x, p.y], temp);
          return;
        }
        // Hovering: the Select tool's cursor over its handles and inside the box (frame-loop note §7.3).
        const b = style.tool === 'select' ? (model?.scene?.selection?.bounds ?? null) : null;
        canvas.style.cursor = selectionCursor(selection.handles, b, d2v, [p.x, p.y]);
        return;
      }
      // Input-to-ink counts from the sample's arrival at the page (plan §6: "sample received").
      metrics?.sample(performance.now());
      const list = typeof e.getCoalescedEvents === 'function' ? e.getCoalescedEvents() : [];
      collector.move((list.length > 0 ? list : [e]).map(sample));
      overlay = gestureOverlay(collector.points, style);
      if (previewKind === 'envelope' || previewKind === 'reposition') {
        // The preview's map (design note 4 §6): the kernel's rule in drawing units through the press mapping; the
        // marquee of a Perspective drag off its handles stays.
        const s = sample(e);
        const tp = toolPress;
        const v2d = tp ? invert(tp.d2v) : null;
        if (tp && press && v2d && !tp.locked && (tp.handle || tp.tool === 'repositionAll')) {
          const next = toolPreview(tp, apply(v2d, press.p[0], press.p[1]), apply(v2d, s.x, s.y));
          if (next || toolMap) viewVersion++;
          toolMap = next;
          if (next) overlay = null;
        }
      } else if (previewKind === 'contour') {
        // The preview (design note 3 §6): the strokes the command will change, drawn from the affected segments only;
        // a drag on nothing keeps its marquee.
        const s = sample(e);
        const c = model?.scene?.contours ?? null;
        // A press that stays within the kernel's click slop is a click: no preview (design note 3 §6).
        const p0 = press?.p;
        if (contourPress && p0 && !contourPress.dragged)
          contourPress.dragged = collector.points.some(
            (q) => Math.hypot(q[0] - p0[0], q[1] - p0[1]) > CLICK_SLOP_PX,
          );
        if (contourPress?.target && contourPress.dragged && c && press && !c.locked) {
          const d = drawingDelta(contourPress.d2v, press.p, [s.x, s.y]);
          const drawn = d ? drawContourPreview(contourPreview(c, contourPress, d), d2v) : null;
          overlay = drawn?.tris ?? null;
          contourDrawn = drawn ? { anchors: drawn.anchors, segments: drawn.segments } : null;
          viewVersion++;
        } else if (contourPress?.target) {
          overlay = null;
        }
      } else if (previewKind) {
        const s = sample(e);
        const next = previewAt([s.x, s.y]);
        if (next || preview) viewVersion++;
        preview = next;
        // A drag the preview shows needs no marquee or ink of its own.
        if (preview && previewKind !== 'grab') overlay = null;
      }
    };
    // The `contextmenu` event: a secondary click (at release on Windows, at the press on macOS), the context-menu key or
    // Shift+F10 with the view focused. It opens the family the view runs now: the Camera view's scene family while a
    // scene tool runs, else the Drawing family (02 §4.4 line 293).
    const onMenu = (e: MouseEvent) => {
      e.preventDefault();
      const press = secondary;
      secondary = null;
      if (collector.active) return;
      if (press && Math.hypot(e.clientX - press.x, e.clientY - press.y) > MENU_DRAG_PX) return;
      const camera = (model?.scene?.viewType ?? windowType.current) === 'camera';
      const family = camera && sceneFamily ? 'scene' : 'drawing';
      openMenu.current({ at: menuAt({ clientX: e.clientX, clientY: e.clientY, currentTarget: el }), family });
    };
    const onUp = (e: PointerEvent) => {
      if (!collector.pressed) return;
      const req = collector.up(sample(e));
      if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
      // A Polyline between its clicks stays one gesture in progress (the modal state stays reported).
      if (!req) return;
      metrics?.released(performance.now(), model?.version ?? 0);
      commit(req);
    };
    const onDouble = (e: MouseEvent) => {
      const req = collector.doubleClick(sample(e as PointerEvent));
      if (!req) return;
      commit(req);
    };
    const onCancel = () => {
      // Only a pressed gesture loses anything to a lost capture: a Polyline between clicks holds no capture.
      if (collector.pressed) cancelGesture();
    };
    // The alternate key's release restores the cursor over a side (design note 4 §2).
    const onAltUp = (e: KeyboardEvent) => {
      const bt = boxTool();
      if (!bt || !hoverAt || collector.active || e.key !== 'Alt') return;
      const tb = model?.scene?.toolBox ?? null;
      canvas.style.cursor = toolBoxCursor(
        bt,
        tb?.tool === bt ? tb.box : null,
        d2v,
        hoverAt,
        false,
        tb?.locked ?? false,
      );
    };
    const onKey = (e: KeyboardEvent) => {
      const bt = boxTool();
      if (bt && hoverAt && !collector.active && e.key === 'Alt') {
        const tb = model?.scene?.toolBox ?? null;
        canvas.style.cursor = toolBoxCursor(
          bt,
          tb?.tool === bt ? tb.box : null,
          d2v,
          hoverAt,
          e.type === 'keydown',
          tb?.locked ?? false,
        );
      }
      if (e.key === 'Escape' && collector.active) {
        e.preventDefault();
        cancelGesture();
      }
      // Enter ends an open Polyline (02 §7.2.5).
      if (e.key === 'Enter' && collector.active && !collector.pressed) {
        const req = collector.end();
        if (req) {
          e.preventDefault();
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
    el.addEventListener('contextmenu', onMenu);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('keyup', onAltUp, true);
    window.addEventListener('blur', onBlur);
    const onGeneration = (e: Event) => {
      const g = (e as CustomEvent<unknown>).detail;
      if (typeof g === 'number') adoptGeneration(g);
    };
    window.addEventListener(GENERATION_EVENT, onGeneration);
    refreshTool();
    refreshSceneTool();
    refreshActiveView();
    refreshSelectedIds();
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
      LIVE_CANVASES.get(viewId)?.delete(reportMounted);
      reportPresence(false);
      canvas.removeEventListener('pointerdown', onDown);
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('pointerup', onUp);
      canvas.removeEventListener('pointercancel', onCancel);
      canvas.removeEventListener('lostpointercapture', onCancel);
      canvas.removeEventListener('wheel', onWheel);
      canvas.removeEventListener('dblclick', onDouble);
      el.removeEventListener('contextmenu', onMenu);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('keyup', onAltUp, true);
      window.removeEventListener('blur', onBlur);
      window.removeEventListener(GENERATION_EVENT, onGeneration);
      cancelAnimationFrame(raf);
      ro?.disconnect();
      unlisten?.();
      onReady?.(null);
      // The preview's parts are dropped while the renderer is alive; a commit answering later acts on nothing.
      life.unmount();
      renderer?.destroy();
    };
  }, [engine, viewId, onReady]);

  return (
    <div
      ref={host}
      className="view-canvas"
      tabIndex={0}
      aria-label="Canvas"
      data-tt-role="canvas"
      data-tt-id={`${viewId}.canvas`}
    >
      <canvas ref={canvasRef} className="view-canvas-surface" />
      {menu && (
        <ContextMenu
          id={`${viewId}.${menu.family}Menu`}
          label={menu.family === 'scene' ? 'Scene' : 'Drawing'}
          at={menu.at}
          groups={menu.family === 'scene' ? SCENE_MENU : DRAWING_MENU}
          onClose={() => {
            setMenu(null);
          }}
        />
      )}
    </div>
  );
}
