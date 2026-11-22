// One frame's layers in 02 §4.2's order (M4 C4.2b; design note 2 §1 step 2-3; invariant F3). The back ends implement
// `LayerOps`; this module alone decides the order, and a recording implementation tests it.

import { packMap, type PreviewMap } from './envelope';
import { compose, type Affine } from './geometry';
import { residentKey, type Rgba, type Scene, type SceneEntry, type SceneGrid } from './scene';

export interface LayerOps {
  /** Layer 1: the background. Starts the static layers. */
  background(colour: Rgba): void;
  /** Layer 2 (`under`) or 8 (`over`): the grid. */
  grid(grid: SceneGrid, placement: 'under' | 'over'): void;
  /** Layers 3 to 7: one resident drawing. */
  drawing(entry: SceneEntry): void;
  /** The static layers are complete (overlay buffering: they are reused while only the overlay changes). */
  endStatic(): void;
  /** Layer 9: tool feedback. */
  overlay(): void;
}

/** 02 §4.2 rank of each role: static underlays, auto light table, onion skin, the current drawing, static overlays. */
export const ROLE_RANK: Record<SceneEntry['role'], number> = {
  staticUnder: 3,
  autoLight: 4,
  onion: 5,
  current: 6,
  staticOver: 7,
};

/** The matrix an entry is drawn through: its occurrence matrix through the Camera view's `cameraToView`, else the
 *  Drawing view's `drawingToView` (camera-view note §6; the canvas composes what it received, CV3). */
export function entryToView(
  e: SceneEntry,
  cameraToView: Affine | null,
  drawingToView: Affine,
  preview: { transform: Affine; elements: ReadonlySet<string> } | null = null,
): Affine {
  const m = e.matrix && cameraToView ? compose(cameraToView, e.matrix) : drawingToView;
  // The live preview moves the selected occurrences (preview note §3).
  return preview && e.elementId && preview.elements.has(e.elementId) ? compose(preview.transform, m) : m;
}

/** The Camera view's order (camera-view note §6; CV1): the kernel's back-to-front order; with `currentOnTop` (02
 *  §10.5) the current drawing's occurrences after the others; the selection's outline entries last. */
export function cameraOrdered(entries: readonly SceneEntry[], currentOnTop: boolean): SceneEntry[] {
  const plain = entries.filter((e) => !e.outline);
  const outlines = entries.filter((e) => e.outline);
  const body = currentOnTop
    ? [...plain.filter((e) => e.role !== 'current'), ...plain.filter((e) => e.role === 'current')]
    : plain;
  return [...body, ...outlines];
}

/** The scene's drawings in layer order (stable within a layer: the kernel's order); the Camera view's order for a
 *  Camera-view answer. */
export function ordered(entries: readonly SceneEntry[], scene?: Scene | null): SceneEntry[] {
  if (scene?.viewType === 'camera') return cameraOrdered(entries, scene.overlays?.currentOnTop ?? false);
  return entries
    .map((e, i) => ({ e, i }))
    .sort((a, b) => ROLE_RANK[a.e.role] - ROLE_RANK[b.e.role] || a.i - b.i)
    .map((x) => x.e);
}

/**
 * Draws one frame. `staticLayers` false reuses the cached static layers (only the overlay changed); `isResident`
 * (by resident key) skips drawings whose geometry has not arrived yet.
 */
export function drawFrame(
  scene: Scene | null,
  ops: LayerOps,
  isResident: (key: string) => boolean,
  staticLayers: boolean,
): void {
  if (staticLayers) {
    ops.background(scene?.background ?? [240, 240, 240, 255]);
    if (scene) {
      if (scene.grid.visible && !scene.grid.overlay) ops.grid(scene.grid, 'under');
      for (const e of ordered(scene.entries, scene)) if (isResident(residentKey(e))) ops.drawing(e);
      if (scene.grid.visible && scene.grid.overlay) ops.grid(scene.grid, 'over');
    }
    ops.endStatic();
  }
  ops.overlay();
}

/**
 * The drawing-to-view matrix and the vertex stage's map for a preview `map` (design note 4 §6): an affine map is
 * composed into the matrix; a projective or bilinear one is applied to each vertex in drawing units (`packMap`).
 */
export function mappedEntry(
  base: Affine,
  map: PreviewMap | null | undefined,
): { toView: Affine; kind: number; data: Float32Array | null } {
  if (!map) return { toView: base, kind: 0, data: null };
  const packed = packMap(map);
  return packed.affine
    ? { toView: compose(base, packed.affine), kind: 0, data: null }
    : { toView: base, kind: packed.kind, data: packed.data };
}
