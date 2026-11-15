// The Drawing view's composition as `canvas_scene` answers it (M4 C4.2b; design note 1 §9; the Rust type is
// `tt_executor::canvas::Scene`). Validated here: a malformed answer is refused, never half-applied.

export type Rgba = [number, number, number, number];

export interface SceneEntry {
  drawingId: string;
  rev: number;
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
}

export interface SceneAnswer {
  generation: number;
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
  const generation = num(a['generation'], 'generation');
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
      rev: num(o['rev'], `entries/${String(i)}/rev`),
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
    },
  };
}

/** The resident key of an entry: the drawing, and its outline-only form apart (02 §10.2). */
export function residentKey(e: Pick<SceneEntry, 'drawingId' | 'outline'>): string {
  return e.outline ? `${e.drawingId}#outline` : e.drawingId;
}
