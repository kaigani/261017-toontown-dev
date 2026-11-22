// The product's back-end measurement (M4 C4.2b; ADR-0007 canvas rule set step 1: both back ends measured at first
// launch and after a GPU, driver or webview change; SI-136: 2 s per cell; SI-140: persisted in the preferences).
//
// Each back end draws the rule set's three scenes (paths100k, groups1k, brush10k), generated here as decoded drawings,
// on a detached canvas: after one untimed warm-up frame, every frame redraws the static layers through an animated
// view and waits for the GPU (`Renderer.finish`), yielding to the page between frames. A cell ends after its time, or
// at its first timed frame over
// `FRAME_ABORT_MS` (a software adapter can take seconds per frame: the cell is then missed without holding the machine).

import { cellOf, SCENES, type Cell, type Measurement, type SceneName } from './backend';
import { VERTEX_LEN, type DecodedDrawing } from './format';
import type { Affine } from './geometry';
import type { BackendKind, FrameInput, Renderer } from './renderer';
import type { Scene, SceneEntry } from './scene';

export const CELL_MS = 2000;
export const FRAME_ABORT_MS = 500;
export const WIDTH = 1280;
export const HEIGHT = 720;

/** A deterministic generator (the scenes are the same on every machine). */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** A decoded solid drawing of `quads` quads and `fans` triangle fans, with one slot. */
function drawing(id: string, build: (push: (pts: [number, number][]) => void) => void): DecodedDrawing {
  const verts: number[] = [];
  const idx: number[] = [];
  build((pts) => {
    const base = verts.length / 2;
    for (const [x, y] of pts) verts.push(x, y);
    for (let i = 1; i + 1 < pts.length; i++) idx.push(base, base + i, base + i + 1);
  });
  const n = verts.length / 2;
  const bytes = new Uint8Array(n * VERTEX_LEN);
  const v = new DataView(bytes.buffer);
  for (let i = 0; i < n; i++) {
    v.setFloat32(i * VERTEX_LEN, verts[i * 2] ?? 0, true);
    v.setFloat32(i * VERTEX_LEN + 4, verts[i * 2 + 1] ?? 0, true);
  }
  return {
    generation: 0n,
    rev: 0n,
    drawingUlid: id,
    bucket: 0,
    flags: 0,
    bounds: [-600, -450, 600, 450],
    slots: [{ id: 'measure', kind: 'solid', rgba: [40, 40, 40, 255] }],
    records: [{ kind: 'solid', vertexCount: n, vertices: bytes, indices: new Uint32Array(idx) }],
    byteLength: bytes.byteLength + idx.length * 4,
  };
}

/** The rule set's scene `name`: its drawings and the entries that show them. */
export function measureScene(name: SceneName): { drawings: Map<string, DecodedDrawing>; scene: Scene } {
  const r = rng(name.length * 7919);
  const drawings = new Map<string, DecodedDrawing>();
  const quad = (x: number, y: number, dx: number, dy: number, w: number): [number, number][] => {
    const len = Math.hypot(dx, dy) || 1;
    const nx = (-dy / len) * w;
    const ny = (dx / len) * w;
    return [
      [x + nx, y + ny],
      [x + dx + nx, y + dy + ny],
      [x + dx - nx, y + dy - ny],
      [x - nx, y - ny],
    ];
  };
  const fan = (x: number, y: number, rad: number, k: number): [number, number][] => [
    [x, y],
    ...Array.from({ length: k + 1 }, (_, i): [number, number] => [
      x + rad * Math.cos((i / k) * 2 * Math.PI),
      y + rad * Math.sin((i / k) * 2 * Math.PI),
    ]),
  ];
  if (name === 'paths100k') {
    drawings.set(
      'paths',
      drawing('paths', (push) => {
        for (let i = 0; i < 100_000; i++)
          push(quad(r() * 1200 - 600, r() * 900 - 450, r() * 20 - 10, r() * 20 - 10, 0.6));
      }),
    );
  } else if (name === 'groups1k') {
    for (let g = 0; g < 1000; g++) {
      const cx = (g % 40) * 30 - 600;
      const cy = Math.floor(g / 40) * 36 - 450;
      drawings.set(
        `g${String(g)}`,
        drawing(`g${String(g)}`, (push) => {
          for (let i = 0; i < 20; i++) push(quad(cx + r() * 20, cy + r() * 20, r() * 10, r() * 10, 1));
        }),
      );
    }
  } else {
    drawings.set(
      'brush',
      drawing('brush', (push) => {
        for (let i = 0; i < 10_000; i++) push(fan(r() * 1200 - 600, r() * 900 - 450, 2 + r() * 6, 16));
      }),
    );
  }
  const entries: SceneEntry[] = [...drawings.keys()].map((drawingId) => ({
    drawingId,
    rev: 0n,
    role: 'current',
    opacity: 1,
    tint: null,
    shade: false,
    outline: false,
  }));
  return {
    drawings,
    scene: {
      viewId: 'measure',
      documentRev: null,
      background: [255, 255, 255, 255],
      grid: { visible: false, type: 'normal', overlay: false },
      entries,
      colours: {},
      paletteStamp: 0,
      drawingPivot: null,
      selection: null,
      sceneForm: '1.1',
      viewType: 'drawing',
      frame: null,
      camera: null,
      overlays: null,
      contours: null,
      toolBox: null,
      textEdit: null,
      textBoxes: null,
    },
  };
}

export interface MeasureEnv {
  now(): number;
  /** Lets the page run between frames. */
  yieldFrame(): Promise<void>;
}

/** One cell: the scene drawn on `renderer` until `CELL_MS` passes or a frame exceeds `FRAME_ABORT_MS`. */
/** Frame `t` of the rule set's pan and zoom: a slow animated view over the scene. */
function frameInput(scene: Scene, t: number): FrameInput {
  const z = 0.8 + 0.2 * Math.sin(t / 40);
  const d2v: Affine = [z, 0, 0, -z, WIDTH / 2 + 80 * Math.sin(t / 60), HEIGHT / 2 + 40 * Math.cos(t / 50)];
  return {
    scene,
    drawingToView: d2v,
    cameraToView: null,
    preview: null,
    split: null,
    moved: null,
    width: WIDTH,
    height: HEIGHT,
    dpr: 1,
    staticDirty: true,
    grid: null,
    overlay: null,
  };
}

export async function measureCell(renderer: Renderer, name: SceneName, env: MeasureEnv): Promise<Cell> {
  const { drawings, scene } = measureScene(name);
  try {
    for (const [id, d] of drawings) renderer.upload(id, d);
    // One untimed warm-up frame: the first use of freshly uploaded buffers carries one-time costs (the upload itself
    // on some drivers) that are not the scene's frame rate; the C0.3 spike also timed the pan/zoom after its upload.
    renderer.frame(frameInput(scene, 0));
    await renderer.finish();
    const start = env.now();
    let frames = 0;
    let t = 0;
    while (env.now() - start < CELL_MS) {
      const a = env.now();
      renderer.frame(frameInput(scene, t));
      await renderer.finish();
      frames++;
      t++;
      if (env.now() - a > FRAME_ABORT_MS) break;
      await env.yieldFrame();
    }
    const elapsed = env.now() - start;
    // One frame is enough evidence: a cell stopped by a frame over the abort bound is missed at that frame's rate.
    return cellOf({ fps: frames >= 1 && elapsed > 0 ? (frames * 1000) / elapsed : null });
  } catch (e) {
    return cellOf({ fps: null, error: e instanceof Error ? e.message : String(e) });
  } finally {
    for (const id of drawings.keys()) renderer.drop(id);
  }
}

/** Both back ends on the three scenes: the measurement step 1 decides on (`selectBackend`). */
export async function measureBackends(
  start: (kind: BackendKind) => Promise<Renderer>,
  env: MeasureEnv,
): Promise<Measurement> {
  const out: Measurement = { webgpu: {}, webgl2: {} };
  for (const kind of ['webgpu', 'webgl2'] as const) {
    let renderer: Renderer;
    try {
      renderer = await start(kind);
    } catch (e) {
      const cell = cellOf({ fps: null, error: e instanceof Error ? e.message : String(e) });
      for (const s of SCENES) out[kind][s] = cell;
      continue;
    }
    for (const s of SCENES) out[kind][s] = await measureCell(renderer, s, env);
    renderer.destroy();
  }
  return out;
}

/** What identifies this machine for the measurement: the GPU adapter, the WebGL renderer and the webview. */
export async function fingerprint(): Promise<string> {
  const parts: string[] = [typeof navigator === 'undefined' ? '' : navigator.userAgent];
  try {
    const gl = document.createElement('canvas').getContext('webgl2');
    const ext = gl?.getExtension('WEBGL_debug_renderer_info');
    if (gl && ext) parts.push(String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)));
  } catch {
    parts.push('no webgl2');
  }
  try {
    const gpu = typeof navigator !== 'undefined' && 'gpu' in navigator ? navigator.gpu : null;
    const adapter = gpu ? await gpu.requestAdapter() : null;
    const info = adapter?.info;
    parts.push(info ? `${info.vendor}/${info.architecture}/${info.device}/${info.description}` : 'no webgpu');
  } catch {
    parts.push('no webgpu');
  }
  return parts.join(' | ');
}
