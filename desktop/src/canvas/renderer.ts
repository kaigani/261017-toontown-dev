// The canvas back-end contract (M4 C4.2b; ADR-0007 decision 3: WebGPU and WebGL2 ship with the same features; design
// note 2 §3). A back end holds resident drawings (it is the model's `ResidentSink`), draws one frame through
// `drawFrame`'s layer order, and reports what it holds.

import type { Affine, ViewTris } from './geometry';
import type { ResidentSink } from './model';
import type { Scene } from './scene';

export type BackendKind = 'webgpu' | 'webgl2';

export interface FrameInput {
  scene: Scene | null;
  /** Drawing → view pixels (the announced `drawingToView`). */
  drawingToView: Affine;
  /** The view's size in CSS pixels (= view pixels). */
  width: number;
  height: number;
  /** Device pixels per view pixel. */
  dpr: number;
  /** The static layers changed since the last frame (or the view did): redraw them; else reuse them (06 §6.9). */
  staticDirty: boolean;
  /** The grid's triangles in view pixels (built by `gridGeometry`), under or over the drawings. */
  grid: ViewTris | null;
  /** Layer 9's triangles in view pixels. */
  overlay: ViewTris | null;
}

export interface BackendStats {
  kind: BackendKind;
  /** Bytes of geometry and slot data held on the GPU (the resident account of design note 1 §5). */
  residentBytes: number;
  residentDrawings: number;
  /** Drawings drawn in the last redraw of the static layers. */
  drawn: number;
  /** Frames whose static layers were redrawn / reused (overlay buffering, 06 §6.9). */
  staticRedraws: number;
  staticReuses: number;
  /** The back end's last error report (a GL error code, a device error), or null. */
  lastError: string | null;
}

export interface Renderer extends ResidentSink {
  readonly kind: BackendKind;
  /** Solid colour values from the scene (06 §6.3): slots of these ids take the new value without geometry. */
  setColours(colours: Record<string, [number, number, number, number]>): void;
  /** Draws one frame. */
  frame(input: FrameInput): void;
  /** `display.antiAliasing` (01 §7.2, 06 §6.9; live): multisampled drawing layers on or off. */
  setAntialias(on: boolean): void;
  /** `display.smoothToolOverlays` (01 §7.2, 06 §6.9; live): the tool overlay multisampled or not. */
  setSmoothOverlays(on: boolean): void;
  stats(): BackendStats;
  destroy(): void;
}

/** A back end that cannot start: the error text starts with "webgpu unavailable" or "webgl2 unavailable" (the canvas
 *  rule set's step 0 keys on these). */
export class BackendUnavailable extends Error {
  constructor(kind: BackendKind, why: string) {
    super(`${kind} unavailable: ${why}`);
    this.name = 'BackendUnavailable';
  }
}
