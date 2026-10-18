// C0.3 canvas spike (spike code): the interface both GPU back ends implement.
import type { Camera } from './camera';
import type { Geometry } from './decode';

export type Backend = 'webgpu' | 'webgl2';

export interface UploadTiming {
  /** CPU time of the buffer/texture creation and data calls. */
  cpuMs: number;
  /** Until the GPU reports the upload complete (WebGL2: `gl.finish()`; WebGPU: `onSubmittedWorkDone`). */
  completeMs: number;
}

export interface FrameTiming {
  /** CPU time to update uniforms/groups and record + submit the frame. */
  cpuMs: number;
  /**
   * Resolves with the time from submission until the GPU work completed, if the back end can tell (WebGPU:
   * `onSubmittedWorkDone`), or with null (WebGL2: no non-blocking completion query is used).
   */
  gpuDone: Promise<number | null>;
}

export interface Renderer {
  readonly backend: Backend;
  /** Adapter / context facts for the log (GPU name, limits, MSAA). */
  readonly info: Record<string, unknown>;
  upload(geometry: Geometry): Promise<UploadTiming>;
  /** Draws one frame with the given camera and group transforms (8 floats per group). */
  frame(camera: Camera, groups: Float32Array): FrameTiming;
  /** Host-raster fallback estimate: uploads one RGBA8 frame into a texture; resolves with the time to complete. */
  uploadPixels(rgba: Uint8Array, width: number, height: number): Promise<number>;
  destroy(): void;
}

/** Unpacks the renderers' shared colour convention: RGBA8 straight alpha in a little-endian u32. */
export const COLOUR_NOTE =
  'RGBA8 straight alpha, premultiplied in the fragment shader; blend ONE, ONE_MINUS_SRC_ALPHA';
