// Which back end runs (M4 C4.2b; ADR-0007 decision 3 and the canvas rule set's step 1, normative; design note 2 §3 and
// §6; SI-136). `selectBackend` is step 1 applied to measured cells; its Vitest cases are the rule table's own (F4).

import type { BackendKind, Renderer } from './renderer';
import { BackendUnavailable } from './renderer';
import { createWebGL2 } from './webgl2';
import { createWebGPU, webgpuApi } from './webgpu';

/** The rule set's three scenes. */
export const SCENES = ['paths100k', 'groups1k', 'brush10k'] as const;
export type SceneName = (typeof SCENES)[number];

/** Step 0: what one back end × scene cell is. */
export type Cell =
  | { status: 'met' | 'missed'; fps: number }
  | { status: 'insufficient' | 'failed' | 'unavailable' | 'notMeasured' };

/** The fps target of a cell (≥ 57 fps). */
export const TARGET_FPS = 57;

/** Step 0 for one measured run: fps null is insufficient evidence; an error after start is failed. */
export function cellOf(run: { fps: number | null; error?: string } | undefined): Cell {
  if (!run) return { status: 'notMeasured' };
  if (run.error !== undefined) {
    return /^(webgpu|webgl2) unavailable/.test(run.error) ? { status: 'unavailable' } : { status: 'failed' };
  }
  if (run.fps === null) return { status: 'insufficient' };
  return { status: run.fps >= TARGET_FPS ? 'met' : 'missed', fps: run.fps };
}

export type Measurement = Record<BackendKind, Partial<Record<SceneName, Cell>>>;

/** A back end's score: its worst scene's fps when all three cells are met or missed (scorable), else null. */
export function score(cells: Partial<Record<SceneName, Cell>>): number | null {
  let worst = Infinity;
  for (const s of SCENES) {
    const c = cells[s];
    if (!c || (c.status !== 'met' && c.status !== 'missed')) return null;
    worst = Math.min(worst, c.fps);
  }
  return worst;
}

/**
 * Step 1: the scorable back end with the higher score; a tie goes to WebGPU; a back end without a valid measurement is
 * never selected over one with. With no scorable back end the result is null (no decision: keep WebGL2, the mandatory
 * path, until a measurement completes).
 */
export function selectBackend(m: Measurement): BackendKind | null {
  const gpu = score(m.webgpu);
  const gl = score(m.webgl2);
  if (gpu === null && gl === null) return null;
  if (gpu === null) return 'webgl2';
  if (gl === null) return 'webgpu';
  return gpu >= gl ? 'webgpu' : 'webgl2';
}

/** The back end the canvas starts with: a user override, else the stored selection, else WebGL2 (SI-136). */
export function startingBackend(override: BackendKind | null, selected: BackendKind | null): BackendKind {
  return override ?? selected ?? 'webgl2';
}

/**
 * Creates the canvas back end. WebGPU needs an adapter obtained asynchronously, so a synchronous start always runs
 * WebGL2 and `upgradeToWebGPU` swaps when the selection is WebGPU and an adapter exists.
 */
export function createRenderer(canvas: HTMLCanvasElement): Renderer {
  return createWebGL2(canvas);
}

/** WebGPU on `canvas` when the API and an adapter exist; throws `BackendUnavailable` otherwise. */
export async function upgradeToWebGPU(canvas: HTMLCanvasElement): Promise<Renderer> {
  if (!webgpuApi()) throw new BackendUnavailable('webgpu', 'no WebGPU API');
  return createWebGPU(canvas);
}
