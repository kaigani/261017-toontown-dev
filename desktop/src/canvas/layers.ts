// One frame's layers in 02 §4.2's order (M4 C4.2b; design note 2 §1 step 2-3; invariant F3). The back ends implement
// `LayerOps`; this module alone decides the order, and a recording implementation tests it.

import type { Rgba, Scene, SceneEntry, SceneGrid } from './scene';

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

/** The scene's drawings in layer order (stable within a layer: the kernel's order). */
export function ordered(entries: readonly SceneEntry[]): SceneEntry[] {
  return entries
    .map((e, i) => ({ e, i }))
    .sort((a, b) => ROLE_RANK[a.e.role] - ROLE_RANK[b.e.role] || a.i - b.i)
    .map((x) => x.e);
}

/**
 * Draws one frame. `staticLayers` false reuses the cached static layers (only the overlay changed); `isResident`
 * skips drawings whose geometry has not arrived yet.
 */
export function drawFrame(
  scene: Scene | null,
  ops: LayerOps,
  isResident: (drawingId: string) => boolean,
  staticLayers: boolean,
): void {
  if (staticLayers) {
    ops.background(scene?.background ?? [240, 240, 240, 255]);
    if (scene) {
      if (scene.grid.visible && !scene.grid.overlay) ops.grid(scene.grid, 'under');
      for (const e of ordered(scene.entries)) if (isResident(e.drawingId)) ops.drawing(e);
      if (scene.grid.visible && scene.grid.overlay) ops.grid(scene.grid, 'over');
    }
    ops.endStatic();
  }
  ops.overlay();
}
