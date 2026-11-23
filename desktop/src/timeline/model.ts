// The Timeline panel's read model (reviews/M5-design-timeline-view.md section 1): the kernel's answers, checked, never
// trusted as typed. Each parser answers null for an answer of the wrong shape, so a surface keeps what it last drew.

export interface RowBar {
  readonly start: number;
  readonly end: number;
  readonly loops: number;
}

export type Glyph = 'position' | 'scale' | 'rotation' | 'skew';

export interface RowKeyframe {
  readonly frame: number;
  readonly channels: readonly Glyph[];
  readonly segmentAfter: 'constant' | 'interpolated';
}

/** One row of `timeline.rows.list` (04 §17.5). */
export interface TimelineRow {
  readonly elementId: string;
  readonly name: string;
  readonly type: string;
  readonly depth: number;
  readonly parentId: string | null;
  readonly collapsed: boolean;
  readonly visibleInScene: boolean;
  readonly hiddenByParent: boolean;
  readonly locked: boolean;
  readonly bar: RowBar | null;
  readonly keyframes: readonly RowKeyframe[];
  readonly childKeyframeFrames: readonly number[];
}

export interface RowsAnswer {
  readonly sceneId: string;
  readonly sceneLength: number;
  readonly currentFrame: number;
  readonly playbackRange: { readonly mode: 'auto' | 'free'; readonly start: number; readonly end: number };
  /** Every row in Timeline order. */
  readonly rows: readonly TimelineRow[];
}

export interface ViewAnswer {
  readonly zoomLevel: number;
  readonly firstFrame: number;
  readonly lastFrame: number;
}

export interface SelectionAnswer {
  readonly elementIds: readonly string[];
  readonly from: number;
  readonly to: number;
  readonly rowsSelected: number;
  readonly currentElementId: string | null;
  readonly currentFrame: number;
}

/** An exposed run of one column in the read window. */
export interface Run {
  readonly drawing: string;
  readonly drawingId: string;
  readonly from: number;
  readonly to: number;
}

/** What `element.get` tells the panel about one visible row. */
export interface RowDisplay {
  readonly markerColour: string;
  readonly backgroundColour: string | null;
  readonly note: string;
}

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown): v is string => typeof v === 'string';
const bool = (v: unknown): v is boolean => typeof v === 'boolean';
const GLYPHS: readonly Glyph[] = ['position', 'scale', 'rotation', 'skew'];

function bar(v: unknown): RowBar | null | undefined {
  if (v === null) return null;
  if (!isObj(v) || !num(v['start']) || !num(v['end']) || !num(v['loops'])) return undefined;
  return { start: v['start'], end: v['end'], loops: v['loops'] };
}

function keyframe(v: unknown): RowKeyframe | null {
  if (!isObj(v) || !num(v['frame']) || !Array.isArray(v['channels'])) return null;
  const channels = v['channels'].filter((c): c is Glyph => GLYPHS.includes(c as Glyph));
  const seg = v['segmentAfter'] === 'constant' ? 'constant' : 'interpolated';
  return { frame: v['frame'], channels, segmentAfter: seg };
}

function row(v: unknown): TimelineRow | null {
  if (!isObj(v)) return null;
  const b = bar(v['bar']);
  if (
    !str(v['elementId']) ||
    !str(v['name']) ||
    !str(v['type']) ||
    !num(v['depth']) ||
    !bool(v['collapsed']) ||
    !bool(v['visibleInScene']) ||
    !bool(v['hiddenByParent']) ||
    !bool(v['locked']) ||
    b === undefined ||
    !Array.isArray(v['keyframes'])
  ) {
    return null;
  }
  const parent = v['parentId'];
  const childFrames = Array.isArray(v['childKeyframeFrames']) ? v['childKeyframeFrames'].filter(num) : [];
  return {
    elementId: v['elementId'],
    name: v['name'],
    type: v['type'],
    depth: v['depth'],
    parentId: str(parent) ? parent : null,
    collapsed: v['collapsed'],
    visibleInScene: v['visibleInScene'],
    hiddenByParent: v['hiddenByParent'],
    locked: v['locked'],
    bar: b,
    keyframes: v['keyframes'].map(keyframe).filter((k): k is RowKeyframe => k !== null),
    childKeyframeFrames: childFrames,
  };
}

/** `timeline.rows.list`'s answer. */
export function parseRows(v: unknown): RowsAnswer | null {
  if (!isObj(v) || !str(v['sceneId']) || !num(v['sceneLength']) || !num(v['currentFrame'])) return null;
  const r = v['playbackRange'];
  if (!isObj(r) || !num(r['start']) || !num(r['end']) || !Array.isArray(v['rows'])) return null;
  const rows: TimelineRow[] = [];
  for (const x of v['rows']) {
    const parsed = row(x);
    if (!parsed) return null;
    rows.push(parsed);
  }
  return {
    sceneId: v['sceneId'],
    sceneLength: v['sceneLength'],
    currentFrame: v['currentFrame'],
    playbackRange: { mode: r['mode'] === 'free' ? 'free' : 'auto', start: r['start'], end: r['end'] },
    rows,
  };
}

/** `timeline.view.set`'s answer (and `shell.timelineView`'s payload). */
export function parseView(v: unknown): ViewAnswer | null {
  if (!isObj(v) || !num(v['zoomLevel']) || !num(v['firstFrame']) || !num(v['lastFrame'])) return null;
  return { zoomLevel: v['zoomLevel'], firstFrame: v['firstFrame'], lastFrame: v['lastFrame'] };
}

/** `cell.selection.get`'s answer. */
export function parseSelection(v: unknown): SelectionAnswer | null {
  if (!isObj(v) || !Array.isArray(v['elementIds']) || !num(v['from']) || !num(v['to'])) return null;
  const cur = v['currentElementId'];
  return {
    elementIds: v['elementIds'].filter(str),
    from: v['from'],
    to: v['to'],
    rowsSelected: num(v['rowsSelected']) ? v['rowsSelected'] : 0,
    currentElementId: str(cur) ? cur : null,
    currentFrame: num(v['currentFrame']) ? v['currentFrame'] : 1,
  };
}

/** `xsheet.exposures.get`'s columns: the runs of each element in the window. */
export function parseExposures(v: unknown): Map<string, Run[]> | null {
  if (!isObj(v) || !Array.isArray(v['columns'])) return null;
  const out = new Map<string, Run[]>();
  for (const c of v['columns']) {
    if (!isObj(c) || !str(c['elementId'])) continue;
    const runs = Array.isArray(c['exposures']) ? c['exposures'] : [];
    out.set(
      c['elementId'],
      runs
        .filter(isObj)
        .filter((r) => str(r['drawing']) && str(r['drawingId']) && num(r['from']) && num(r['to']))
        .map((r) => ({
          drawing: r['drawing'] as string,
          drawingId: r['drawingId'] as string,
          from: r['from'] as number,
          to: r['to'] as number,
        })),
    );
  }
  return out;
}

/** `element.get`'s display fields. */
export function parseDisplay(v: unknown): RowDisplay | null {
  if (!isObj(v) || !str(v['effectiveMarkerColour'])) return null;
  const bg = v['backgroundColour'];
  return {
    markerColour: v['effectiveMarkerColour'],
    backgroundColour: str(bg) ? bg : null,
    note: str(v['note']) ? v['note'] : '',
  };
}

/** The rows the Timeline shows, in order: the rule the kernel's interpreter hit-tests with (ruling E-37, `rows.ts`). */
export { shownRows } from './rows';

/** Cell-based element types (the kernel's `is_cell_based`): a row of exposures rather than a bar. */
export const CELL_BASED = new Set(['drawing', 'image', 'annotation', 'media']);

/** Types with a loopable bar (04 §11.10; the interpreter's `loopable`). */
export function loopable(type: string): boolean {
  return !CELL_BASED.has(type) && type !== 'camera';
}

/** Types that can be parents (04 §9.2; the kernel's `parentable`). */
export const PARENTABLE = new Set(['peg', 'effect', 'skeleton', 'bone', 'camera']);

/** A CSS colour from the kernel's `#RRGGBBAA`. */
export function cssColour(hex: string | null | undefined, fallback = '#808080'): string {
  if (!hex || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(hex)) return fallback;
  if (hex.length === 7) return hex;
  const a = parseInt(hex.slice(7, 9), 16) / 255;
  const rgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return `rgba(${rgb.join(', ')}, ${a.toFixed(3)})`;
}

/** The transfer options of one operation (04 §11.14; `timeline.transferOptions.get`'s answer, 04's names). */
export interface TransferOptions {
  overwriteDrawings: boolean;
  updateColours: boolean;
  exposure: boolean;
  autoExtend: boolean;
  keyframeSubstitution: boolean;
  boundaryKeys: boolean;
  pegKeys: {
    motion: boolean;
    rotation: boolean;
    scale: boolean;
    pivot: boolean;
    velocity: boolean;
    skew: boolean;
  };
}

/** A transfer-options answer, checked (`null` for an answer of the wrong shape). */
export function parseTransferOptions(v: unknown): TransferOptions | null {
  const o = typeof v === 'object' && v !== null ? (v as Record<string, unknown>)['options'] : null;
  if (typeof o !== 'object' || o === null) return null;
  const r = o as Record<string, unknown>;
  const pk = (typeof r['pegKeys'] === 'object' && r['pegKeys'] !== null ? r['pegKeys'] : {}) as Record<
    string,
    unknown
  >;
  const b = (x: unknown, d: boolean) => (typeof x === 'boolean' ? x : d);
  return {
    overwriteDrawings: b(r['overwriteDrawings'], false),
    updateColours: b(r['updateColours'], false),
    exposure: b(r['exposure'], true),
    autoExtend: b(r['autoExtend'], false),
    keyframeSubstitution: b(r['keyframeSubstitution'], false),
    boundaryKeys: b(r['boundaryKeys'], false),
    pegKeys: {
      motion: b(pk['motion'], true),
      rotation: b(pk['rotation'], true),
      scale: b(pk['scale'], true),
      pivot: b(pk['pivot'], true),
      velocity: b(pk['velocity'], true),
      skew: b(pk['skew'], true),
    },
  };
}
