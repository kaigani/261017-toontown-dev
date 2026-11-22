// The canvas's launch options (M4 C4.2b measurements; plan §6, ruling D-5): `--canvas-metrics` publishes frame,
// input and load timings in the canvas's UI-tree value; `--canvas-backend` forces a back end (the rule set's user
// override). Set once from the host's startup answer; read by every canvas the window mounts afterwards.

export interface CanvasLaunch {
  metrics: boolean;
  backend: 'webgpu' | 'webgl2' | null;
}

let current: CanvasLaunch = { metrics: false, backend: null };

export function setCanvasLaunch(v: CanvasLaunch): void {
  current = v;
}

export function canvasLaunch(): CanvasLaunch {
  return current;
}
