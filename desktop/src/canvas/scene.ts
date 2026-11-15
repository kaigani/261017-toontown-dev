// The Drawing view's composition as `canvas_scene` answers it (M4 C4.2b; design note 1 §9; the Rust type is
// `tt_executor::canvas::Scene`). Validated here: a malformed answer is refused, never half-applied.
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
  const entriesRaw = s['entries'];
  if (!Array.isArray(entriesRaw)) throw new TypeError('entries: not an array');
  const entries = entriesRaw.map((e: unknown, i): SceneEntry => {
    const o = obj(e);
    if (!o) throw new TypeError(`entries/${String(i)}: not an object`);
    const role = str(o['role'], `entries/${String(i)}/role`);
    if (!ROLES.has(role)) throw new TypeError(`entries/${String(i)}/role: unknown ${role}`);
    const tint = o['tint'];
    return {
      drawingId: str(o['drawingId'], `entries/${String(i)}/drawingId`),
      rev: u64(o['rev'], `entries/${String(i)}/rev`),
      role: role as SceneEntry['role'],
      opacity: num(o['opacity'], `entries/${String(i)}/opacity`),
      tint: tint === null ? null : rgba(tint, `entries/${String(i)}/tint`),
      shade: bool(o['shade'], `entries/${String(i)}/shade`),
      outline: bool(o['outline'], `entries/${String(i)}/outline`),
    };
  });
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
    },
  };
}

/** The resident key of an entry: the drawing, and its outline-only form apart (02 §10.2). */
export function residentKey(e: Pick<SceneEntry, 'drawingId' | 'outline'>): string {
  return e.outline ? `${e.drawingId}#outline` : e.drawingId;
}
