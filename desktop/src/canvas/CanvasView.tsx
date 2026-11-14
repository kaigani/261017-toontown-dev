// The interactive canvas of a Drawing view window (M4 C4.2b; design notes 1 and 2). One per window (D-7a): its own
// model (the pull loop), back end and frame loop. The canvas follows the kernel: the composition from `canvas_scene`,
// geometry from `canvas_load`, the mapping from `shell.view` and `draw.viewMapping.get` (it never derives a mapping).
//
// The element's `data-tt-value` carries what the canvas shows (back end, generation, each resident drawing and its
// revision), so the UI tree (`inspect_ui`) and the running-application tests can check it.

import { useEffect, useRef } from 'react';

import type { EngineApi } from '../ipc';
import { bucketOf } from './bucket';
import { gridGeometry, IDENTITY, scaleOf, ViewTris, type Affine } from './geometry';
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

    const refreshMapping = () => {
      void engine
        .run('draw.viewMapping.get', { view: viewId })
        .done.then((env) => {
          const m = asAffine((env.result as Record<string, unknown> | undefined)?.['drawingToView']);
          if (m && !stopped) {
            d2v = m;
            viewVersion++;
            model?.setBucket(bucketOf(scaleOf(m)));
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
            void engine
              .report({ viewSizes: { [viewId]: { width: r.width, height: r.height } } })
              .then(refreshMapping, () => undefined);
          });
    ro?.observe(el);

    let unlisten: (() => void) | null = null;
    void engine
      .onEvent((event) => {
        if (typeof event.generation === 'number') model?.setGeneration(event.generation);
        if (event.name === 'shell.view') {
          const p = event.payload as Record<string, unknown> | null;
          const m = asAffine(p?.['drawingToView']);
          if (p?.['viewId'] === viewId && m) {
            d2v = m;
            viewVersion++;
            model?.setBucket(bucketOf(scaleOf(m)));
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
