// A view's composition as `canvas_scene` answers it: the scene answer of docs/canvas-format.md §11, form 1.1 (the
// Drawing view, M4 C4.2b; the Camera view, M4 C4.3; the Rust type is `tt_executor::canvas::Scene`). Validated here,
// rules R1-R6 of §11.5 included: a malformed answer is refused, never half-applied.
//
// 64-bit identities (the engine generation and drawing revisions, u64 in Rust) are carried exactly as `bigint`
// (docs/canvas-format.md §6): the host's JSON is parsed losslessly (`parseWireJson`), and an integer the parser kept
// as a number is converted. Coordinates and opacities stay floating numbers.

import type { Box, Pt, SceneSelection } from './selection';

export type Rgba = [number, number, number, number];

export interface SceneEntry {
  drawingId: string;
  /** The drawing's revision (u64, exact). */
  rev: bigint;
  role: 'current' | 'onion' | 'autoLight' | 'staticUnder' | 'staticOver';
  /** Coverage multiplier 0-1 (SI-139). */
  opacity: number;
  /** A tint replacing the colour (SI-139), or null. */
  tint: Rgba | null;
  shade: boolean;
  outline: boolean;
  /** Camera view: the element this occurrence draws (§11.3). */
  elementId?: string;
  /** Camera view: drawing units → camera output pixels, the final renderer's plane matrix (§11.3). */
  matrix?: Matrix;
}

/** An affine `[a, b, c, d, e, f]`: `(x, y) → (a x + c y + e, b x + d y + f)`. */
export type Matrix = [number, number, number, number, number, number];

/** The Camera view's camera (§11.2). */
export interface SceneCamera {
  elementId: string;
  /** Output size in pixels. */
  size: [number, number];
  /** Camera output pixels → view pixels. */
  cameraToView: Matrix;
}

/** The Camera view's overlays, in camera output pixels (§11.4). */
export interface SceneOverlays {
  cameraFrame: Box;
  pivots: Pt[];
  selectionBoxes: Box[];
  selectionUnion: Box | null;
  otherCameras: [Pt, Pt, Pt, Pt][];
  selectionDisplay: { outlineOnSelection: boolean; boxHighlighting: boolean; keepSelectionVisible: boolean };
  currentOnTop: boolean;
}

export interface SceneGrid {
  visible: boolean;
  type: 'normal' | 'field12' | 'field16';
  overlay: boolean;
}

export interface Scene {
  viewId: string;
  background: Rgba;
  grid: SceneGrid;
  entries: SceneEntry[];
  /** Solid colour id → RGBA. */
  colours: Record<string, Rgba>;
  paletteStamp: number;
  /** The current drawing's pivot (drawing units), or null (frame-loop note §7.1). */
  drawingPivot: Pt | null;
  /** The current drawing's selection, or null (frame-loop note §7.1). */
  selection: SceneSelection | null;
  /** The answer's form (§11.2): `1.0` when absent. */
  sceneForm: string;
  /** `drawing` (absent) or `camera`. */
  viewType: 'drawing' | 'camera';
  /** Camera view: the scene frame shown. */
  frame: number | null;
  /** Camera view: the camera, `null` without a current camera (SI-147); `null` in the Drawing view too. */
  camera: SceneCamera | null;
  /** Camera view: the overlays; `null` in the Drawing view. */
  overlays: SceneOverlays | null;
}

export interface SceneAnswer {
  /** The engine generation (u64, exact). */
  generation: bigint;
  scene: Scene;
}

const ROLES = new Set(['current', 'onion', 'autoLight', 'staticUnder', 'staticOver']);
const GRIDS = new Set(['normal', 'field12', 'field16']);

const obj = (v: unknown): Record<string, unknown> | null =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null;

function rgba(v: unknown, where: string): Rgba {
  if (Array.isArray(v) && v.length === 4 && v.every((c) => Number.isInteger(c) && c >= 0 && c <= 255)) {
    return v as Rgba;
  }
  throw new TypeError(`${where}: not an RGBA8 quadruple`);
}

function num(v: unknown, where: string): number {
  if (typeof v === 'number' && Number.isFinite(v)) return v;
  throw new TypeError(`${where}: not a finite number`);
}

const U64_MAX = (1n << 64n) - 1n;

/** An unsigned 64-bit integer, exactly: a `bigint` in range, or a number that is a safe non-negative integer. */
export function u64(v: unknown, where: string): bigint {
  if (typeof v === 'bigint' && v >= 0n && v <= U64_MAX) return v;
  if (typeof v === 'number' && Number.isSafeInteger(v) && v >= 0) return BigInt(v);
  throw new TypeError(`${where}: not an unsigned 64-bit integer`);
}

function point(v: unknown, where: string): Pt {
  if (Array.isArray(v) && v.length === 2) return [num(v[0], where), num(v[1], where)];
  throw new TypeError(`${where}: not a point`);
}

function box(v: unknown, where: string): Box {
  if (Array.isArray(v) && v.length === 4)
    return [num(v[0], where), num(v[1], where), num(v[2], where), num(v[3], where)];
  throw new TypeError(`${where}: not a box`);
}

function list<T>(v: unknown, where: string, item: (x: unknown, w: string) => T): T[] {
  if (!Array.isArray(v)) throw new TypeError(`${where}: not an array`);
  return v.map((x: unknown, i) => item(x, `${where}/${String(i)}`));
}

/** D-7e: absent and `null` both mean nothing selected (a host before the selection overlay omits it). */
function selectionOf(v: unknown): SceneSelection | null {
  if (v === null || v === undefined) return null;
  const o = obj(v);
  if (!o) throw new TypeError('selection: not an object or null');
  return {
    objects: list(o['objects'], 'selection/objects', box),
    bounds: o['bounds'] === null ? null : box(o['bounds'], 'selection/bounds'),
    pivot: o['pivot'] === null ? null : point(o['pivot'], 'selection/pivot'),
    points: list(o['points'], 'selection/points', point),
  };
}

function bool(v: unknown, where: string): boolean {
  if (typeof v === 'boolean') return v;
  throw new TypeError(`${where}: not a boolean`);
}

function str(v: unknown, where: string): string {
  if (typeof v === 'string') return v;
  throw new TypeError(`${where}: not a string`);
}

function matrixOf(v: unknown, where: string): Matrix {
  if (Array.isArray(v) && v.length === 6) {
    const m = v.map((x: unknown, i) => num(x, `${where}/${String(i)}`)) as Matrix;
    if (m[0] * m[3] - m[1] * m[2] === 0) throw new TypeError(`${where}: a singular matrix`);
    return m;
  }
  throw new TypeError(`${where}: not a matrix`);
}

function orderedBox(v: unknown, where: string): Box {
  const b = box(v, where);
  if (!(b[0] <= b[2] && b[1] <= b[3])) throw new TypeError(`${where}: min above max`);
  return b;
}

function positiveInt(v: unknown, where: string): number {
  const n = num(v, where);
  if (!Number.isInteger(n) || n < 1) throw new TypeError(`${where}: not an integer of at least 1`);
  return n;
}

/** R3: the camera member of a Camera view answer. */
function cameraOf(v: unknown): SceneCamera | null {
  if (v === null) return null;
  const o = obj(v);
  if (!o) throw new TypeError('camera: not an object or null');
  const id = str(o['elementId'], 'camera.elementId');
  if (id === '') throw new TypeError('camera.elementId: empty');
  const size = o['size'];
  if (!Array.isArray(size) || size.length !== 2) throw new TypeError('camera.size: not two integers');
  return {
    elementId: id,
    size: [positiveInt(size[0], 'camera.size/0'), positiveInt(size[1], 'camera.size/1')],
    cameraToView: matrixOf(o['cameraToView'], 'camera.cameraToView'),
  };
}

/** R5: the overlays of a Camera view answer. */
function overlaysOf(v: unknown): SceneOverlays {
  const o = obj(v);
  if (!o) throw new TypeError('overlays: not an object');
  const d = obj(o['selectionDisplay']);
  if (!d) throw new TypeError('overlays.selectionDisplay: not an object');
  const corners = (x: unknown, w: string): [Pt, Pt, Pt, Pt] => {
    const pts = list(x, w, point);
    if (pts.length !== 4) throw new TypeError(`${w}: not four corners`);
    return pts as [Pt, Pt, Pt, Pt];
  };
  return {
    cameraFrame: orderedBox(o['cameraFrame'], 'overlays.cameraFrame'),
    pivots: list(o['pivots'], 'overlays.pivots', point),
    selectionBoxes: list(o['selectionBoxes'], 'overlays.selectionBoxes', orderedBox),
    selectionUnion:
      o['selectionUnion'] == null ? null : orderedBox(o['selectionUnion'], 'overlays.selectionUnion'),
    otherCameras: list(o['otherCameras'], 'overlays.otherCameras', corners),
    selectionDisplay: {
      outlineOnSelection: bool(d['outlineOnSelection'], 'overlays.selectionDisplay.outlineOnSelection'),
      boxHighlighting: bool(d['boxHighlighting'], 'overlays.selectionDisplay.boxHighlighting'),
      keepSelectionVisible: bool(d['keepSelectionVisible'], 'overlays.selectionDisplay.keepSelectionVisible'),
    },
    currentOnTop: bool(o['currentOnTop'], 'overlays.currentOnTop'),
  };
}

/** Validates a `canvas_scene` answer; throws a TypeError naming the first malformed field. */
export function parseSceneAnswer(v: unknown): SceneAnswer {
  const a = obj(v);
  if (!a) throw new TypeError('scene answer: not an object');
  const generation = u64(a['generation'], 'generation');
  const s = obj(a['scene']);
  if (!s) throw new TypeError('scene: not an object');
  const g = obj(s['grid']);
  if (!g) throw new TypeError('grid: not an object');
  const gridType = str(g['type'], 'grid.type');
  if (!GRIDS.has(gridType)) throw new TypeError(`grid.type: unknown ${gridType}`);
  // R1: the form's major version is 1.
  const formRaw = s['sceneForm'];
  const sceneForm = formRaw === undefined ? '1.0' : str(formRaw, 'sceneForm');
  if (!/^1\.\d+$/.test(sceneForm)) throw new TypeError(`sceneForm: unknown ${sceneForm}`);
  // R2.
  const viewTypeRaw = s['viewType'] === undefined ? 'drawing' : str(s['viewType'], 'viewType');
  if (viewTypeRaw !== 'drawing' && viewTypeRaw !== 'camera')
    throw new TypeError(`viewType: unknown ${viewTypeRaw}`);
  const camera = viewTypeRaw === 'camera';
  const entriesRaw = s['entries'];
  if (!Array.isArray(entriesRaw)) throw new TypeError('entries: not an array');
  const entries = entriesRaw.map((e: unknown, i): SceneEntry => {
    const o = obj(e);
    if (!o) throw new TypeError(`entries/${String(i)}: not an object`);
    const role = str(o['role'], `entries/${String(i)}/role`);
    if (!ROLES.has(role)) throw new TypeError(`entries/${String(i)}/role: unknown ${role}`);
    const tint = o['tint'];
    const at = `entries/${String(i)}`;
    // R4 (Camera view: every occurrence names its element and matrix) and R6 (Drawing view: no matrix).
    let occurrence: { elementId?: string; matrix?: Matrix } = {};
    if (camera) {
      const id = str(o['elementId'], `${at}/elementId`);
      if (id === '') throw new TypeError(`${at}/elementId: empty`);
      occurrence = { elementId: id, matrix: matrixOf(o['matrix'], `${at}/matrix`) };
    } else if (o['matrix'] !== undefined) {
      throw new TypeError(`${at}/matrix: a Drawing view entry has no matrix`);
    }
    return {
      ...occurrence,
      drawingId: str(o['drawingId'], `entries/${String(i)}/drawingId`),
      rev: u64(o['rev'], `entries/${String(i)}/rev`),
      role: role as SceneEntry['role'],
      opacity: num(o['opacity'], `entries/${String(i)}/opacity`),
      tint: tint === null ? null : rgba(tint, `entries/${String(i)}/tint`),
      shade: bool(o['shade'], `entries/${String(i)}/shade`),
      outline: bool(o['outline'], `entries/${String(i)}/outline`),
    };
  });
  // R3, R4, R5, R6: the Camera view's members.
  let frame: number | null = null;
  let cam: SceneCamera | null = null;
  let overlays: SceneOverlays | null = null;
  if (camera) {
    frame = positiveInt(s['frame'], 'frame');
    cam = cameraOf(s['camera']);
    if (cam === null && entries.length > 0)
      throw new TypeError('entries: a Camera view without a camera has none');
    overlays = overlaysOf(s['overlays']);
  } else if (s['camera'] !== undefined || s['overlays'] !== undefined) {
    throw new TypeError('camera: a Drawing view answer has no camera or overlays');
  }
  const coloursRaw = obj(s['colours']);
  if (!coloursRaw) throw new TypeError('colours: not an object');
  const colours: Record<string, Rgba> = {};
  for (const [k, c] of Object.entries(coloursRaw)) colours[k] = rgba(c, `colours/${k}`);
  return {
    generation,
    scene: {
      viewId: str(s['viewId'], 'viewId'),
      background: rgba(s['background'], 'background'),
      grid: {
        visible: bool(g['visible'], 'grid.visible'),
        type: gridType as SceneGrid['type'],
        overlay: bool(g['overlay'], 'grid.overlay'),
      },
      entries,
      colours,
      paletteStamp: num(s['paletteStamp'], 'paletteStamp'),
      // D-7e: absent and `null` both mean no current drawing (no pivot to show).
      drawingPivot: s['drawingPivot'] == null ? null : point(s['drawingPivot'], 'drawingPivot'),
      selection: selectionOf(s['selection']),
      sceneForm,
      viewType: viewTypeRaw,
      frame,
      camera: cam,
      overlays,
    },
  };
}

/** The resident key of an entry: the drawing, and its outline-only form apart (02 §10.2). */
export function residentKey(e: Pick<SceneEntry, 'drawingId' | 'outline'>): string {
  return e.outline ? `${e.drawingId}#outline` : e.drawingId;
}
