// The Timeline's tracks and ruler drawing (reviews/M5-design-timeline-view.md section 2): pure functions of a model and
// a geometry, drawing only the rows and frames in view, so a Vitest recording context can count their operations (the
// CI shape of the plan's section 6.2: the 1,000-row scene costs at most three times the 10-row scene).
import { LAYOUT, xOf, zoomOf, type ViewPosition } from './geometry';
import {
  CELL_BASED,
  cssColour,
  loopable,
  type Glyph,
  type RowDisplay,
  type Run,
  type TimelineRow,
} from './model';

/** The subset of a 2-D context the Timeline draws with (a recording double in tests). */
export interface Ctx2D {
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  globalAlpha: number;
  font: string;
  textBaseline: CanvasTextBaseline;
  fillRect(x: number, y: number, w: number, h: number): void;
  strokeRect(x: number, y: number, w: number, h: number): void;
  clearRect(x: number, y: number, w: number, h: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  closePath(): void;
  arc(x: number, y: number, r: number, a0: number, a1: number): void;
  fill(): void;
  stroke(): void;
  fillText(text: string, x: number, y: number): void;
  save(): void;
  restore(): void;
}

/** What the tracks show. */
export interface TracksModel {
  /** The rows the Timeline shows, in order (`shownRows`). */
  readonly rows: readonly TimelineRow[];
  /** The visible cell-based rows' exposure runs in the read window. */
  readonly runs: ReadonlyMap<string, readonly Run[]>;
  /** `element.get`'s display fields of the visible rows. */
  readonly display: ReadonlyMap<string, RowDisplay>;
  readonly selectedRows: ReadonlySet<string>;
  readonly cells: {
    readonly elementIds: readonly string[];
    readonly from: number;
    readonly to: number;
  } | null;
  readonly currentElementId: string | null;
  readonly currentFrame: number;
  readonly background: string;
}

/** Where the tracks are drawn. */
export interface TracksGeometry {
  readonly view: ViewPosition;
  readonly width: number;
  readonly height: number;
  readonly scrollTop: number;
}

/** Theme-neutral greys (the only colours not read from the kernel). */
const GREY = {
  grid: 'rgba(128, 128, 128, 0.18)',
  held: 'rgba(0, 0, 0, 0.55)',
  selection: 'rgba(80, 140, 255, 0.35)',
  current: 'rgba(80, 140, 255, 0.9)',
  playhead: 'rgba(220, 60, 60, 0.9)',
  hidden: 'rgba(128, 128, 128, 0.45)',
};

/** The visible frame window of a geometry: first and last frame drawn. */
export function frameWindow(g: TracksGeometry): { first: number; last: number } {
  const z = zoomOf(g.view.zoomLevel);
  const cells = Math.ceil(g.width / Math.max(1, z.cellWidth)) + 1;
  return { first: g.view.firstFrame, last: g.view.firstFrame + cells * Math.max(1, z.framesPerCell) - 1 };
}

/** The visible row window: first and last row index drawn. */
export function rowsInView(g: TracksGeometry, count: number): { first: number; last: number } {
  const rh = LAYOUT.rowHeight;
  const first = Math.max(0, Math.floor(g.scrollTop / rh));
  const last = Math.min(count - 1, Math.floor((g.scrollTop + Math.max(1, g.height) - 1) / rh));
  return { first, last };
}

/** One keyframe glyph (04 line 627): position a circle, scale a square, rotation a triangle, skew a diamond; several
 *  channels at one frame split the glyph into quadrants (drawn here as the glyph of each channel, smaller). */
function glyph(ctx: Ctx2D, kind: Glyph, cx: number, cy: number, r: number): void {
  ctx.beginPath();
  switch (kind) {
    case 'position':
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      break;
    case 'scale':
      ctx.moveTo(cx - r, cy - r);
      ctx.lineTo(cx + r, cy - r);
      ctx.lineTo(cx + r, cy + r);
      ctx.lineTo(cx - r, cy + r);
      break;
    case 'rotation':
      ctx.moveTo(cx, cy - r);
      ctx.lineTo(cx + r, cy + r);
      ctx.lineTo(cx - r, cy + r);
      break;
    case 'skew':
      ctx.moveTo(cx, cy - r);
      ctx.lineTo(cx + r, cy);
      ctx.lineTo(cx, cy + r);
      ctx.lineTo(cx - r, cy);
      break;
  }
  ctx.closePath();
  ctx.fill();
}

/** Draws the tracks: background, rows in view (bar, cells, keyframes, child markers, states), the cell selection and the
 *  playhead. Draws nothing outside the rows and frames in view. */
export function drawTracks(ctx: Ctx2D, m: TracksModel, g: TracksGeometry): void {
  const rh = LAYOUT.rowHeight;
  const z = zoomOf(g.view.zoomLevel);
  const cw = z.cellWidth;
  const win = frameWindow(g);
  const x = (f: number) => xOf(g.view, f);
  const span = (a: number, b: number): [number, number] | null => {
    const lo = Math.max(a, win.first);
    const hi = Math.min(b, win.last);
    return lo > hi ? null : [x(lo), x(hi) + cw];
  };
  ctx.clearRect(0, 0, g.width, g.height);
  ctx.fillStyle = cssColour(m.background, '#2b2b2b');
  ctx.fillRect(0, 0, g.width, g.height);
  const rows = rowsInView(g, m.rows.length);
  for (let i = rows.first; i <= rows.last; i++) {
    const row = m.rows[i];
    if (!row) continue;
    const top = i * rh - g.scrollTop;
    const d = m.display.get(row.elementId);
    const marker = cssColour(d?.markerColour, '#7f9fbf');
    if (d?.backgroundColour) {
      ctx.fillStyle = cssColour(d.backgroundColour);
      ctx.fillRect(0, top, g.width, rh);
    }
    if (m.selectedRows.has(row.elementId)) {
      ctx.fillStyle = GREY.selection;
      ctx.fillRect(0, top, g.width, rh);
    }
    ctx.globalAlpha = row.visibleInScene && !row.hiddenByParent ? 1 : 0.4;
    if (CELL_BASED.has(row.type)) {
      // Exposures: a symbol at each run start (lower half), held frames as a line.
      for (const r of m.runs.get(row.elementId) ?? []) {
        const s = span(r.from, r.to);
        if (!s) continue;
        ctx.fillStyle = marker;
        ctx.fillRect(s[0], top + rh / 2, s[1] - s[0], rh / 2 - 2);
        if (r.from >= win.first && cw >= 6) {
          ctx.fillStyle = GREY.held;
          ctx.fillRect(x(r.from) + 1, top + rh / 2 + 2, Math.max(1, Math.min(4, cw - 2)), rh / 2 - 6);
        }
      }
    } else if (row.bar) {
      const b = row.bar;
      const cycle = b.end - b.start + 1;
      const s = span(b.start, b.end);
      if (s) {
        ctx.fillStyle = marker;
        ctx.fillRect(s[0], top + 3, s[1] - s[0], rh - 6);
      }
      if (loopable(row.type) && b.loops > 1) {
        const looped = span(b.end + 1, b.start + cycle * b.loops - 1);
        if (looped) {
          ctx.globalAlpha *= 0.5;
          ctx.fillStyle = marker;
          ctx.fillRect(looped[0], top + 3, looped[1] - looped[0], rh - 6);
          ctx.globalAlpha = row.visibleInScene && !row.hiddenByParent ? 1 : 0.4;
        }
      }
    }
    // Keyframes: upper half of a cell-based row, centred on a bar row; a line to the next interpolated key.
    const cy = CELL_BASED.has(row.type) ? top + rh / 4 + 1 : top + rh / 2;
    const r = Math.min(LAYOUT.keyframeRadius, Math.max(2, cw / 2));
    for (let k = 0; k < row.keyframes.length; k++) {
      const key = row.keyframes[k];
      if (!key || key.frame < win.first || key.frame > win.last) continue;
      const cx = x(key.frame) + cw / 2;
      const next = row.keyframes[k + 1];
      if (next && key.segmentAfter === 'interpolated') {
        ctx.strokeStyle = GREY.held;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.lineTo(x(Math.min(next.frame, win.last + 1)) + cw / 2, cy);
        ctx.stroke();
      }
      ctx.fillStyle = '#f0f0f0';
      const kinds = key.channels.length > 0 ? key.channels : (['position'] as const);
      const each = kinds.length > 1 ? r * 0.7 : r;
      for (const kind of kinds) glyph(ctx, kind, cx, cy, each);
    }
    // A child's keyframe frame: a small tick on the parent.
    ctx.fillStyle = GREY.held;
    for (const f of row.childKeyframeFrames) {
      if (f < win.first || f > win.last) continue;
      ctx.fillRect(x(f) + cw / 2 - 1, top + rh - 4, 2, 3);
    }
    ctx.globalAlpha = 1;
    if (m.currentElementId === row.elementId) {
      ctx.strokeStyle = GREY.current;
      ctx.lineWidth = 1;
      ctx.strokeRect(0.5, top + 0.5, g.width - 1, rh - 1);
    }
  }
  // The cell selection.
  if (m.cells) {
    const s = span(m.cells.from, m.cells.to);
    if (s) {
      for (let i = rows.first; i <= rows.last; i++) {
        const row = m.rows[i];
        if (row && m.cells.elementIds.includes(row.elementId)) {
          ctx.fillStyle = GREY.selection;
          ctx.fillRect(s[0], i * rh - g.scrollTop, s[1] - s[0], rh);
        }
      }
    }
  }
  // The playhead.
  if (m.currentFrame >= win.first && m.currentFrame <= win.last) {
    ctx.fillStyle = GREY.playhead;
    ctx.fillRect(x(m.currentFrame) + cw / 2 - 0.5, 0, 1, g.height);
  }
}

/** What the ruler shows. */
export interface RulerModel {
  readonly currentFrame: number;
  readonly sceneLength: number;
  readonly range: { readonly mode: 'auto' | 'free'; readonly start: number; readonly end: number };
  /** Labels of the frames that carry one (from `timeline.frameLabel.get`), by frame. */
  readonly labels: ReadonlyMap<number, string>;
}

/** The frames that carry a label at a zoom level: every frame when cells are wide, else every 5th or 10th. */
export function labelledFrames(view: ViewPosition, width: number): number[] {
  const z = zoomOf(view.zoomLevel);
  const pxPerFrame = z.cellWidth / Math.max(1, z.framesPerCell);
  const step = pxPerFrame >= 24 ? 1 : pxPerFrame >= 8 ? 5 : pxPerFrame >= 2 ? 10 : 50;
  const g: TracksGeometry = { view, width, height: 0, scrollTop: 0 };
  const win = frameWindow(g);
  const out: number[] = [];
  const start = Math.ceil(win.first / step) * step;
  for (let f = Math.max(step, start); f <= win.last; f += step) out.push(f);
  if (win.first <= 1) out.unshift(1);
  return Array.from(new Set(out));
}

/** Draws the ruler: ticks, labels, the scene end, the playback range (markers when free) and the playhead. */
export function drawRuler(ctx: Ctx2D, m: RulerModel, view: ViewPosition, width: number): void {
  const h = LAYOUT.rulerHeight;
  const z = zoomOf(view.zoomLevel);
  const cw = z.cellWidth;
  const g: TracksGeometry = { view, width, height: h, scrollTop: 0 };
  const win = frameWindow(g);
  const x = (f: number) => xOf(view, f);
  ctx.clearRect(0, 0, width, h);
  // The playback range, shaded.
  const lo = Math.max(m.range.start, win.first);
  const hi = Math.min(m.range.end, win.last);
  if (lo <= hi) {
    ctx.fillStyle = 'rgba(80, 140, 255, 0.18)';
    ctx.fillRect(x(lo), 0, x(hi) + cw - x(lo), h);
  }
  // Past the scene end, dimmed.
  if (m.sceneLength < win.last) {
    const from = Math.max(m.sceneLength + 1, win.first);
    ctx.fillStyle = GREY.hidden;
    ctx.fillRect(x(from), 0, width - x(from), h);
  }
  ctx.fillStyle = '#c8c8c8';
  ctx.font = '10px sans-serif';
  ctx.textBaseline = 'top';
  for (const f of labelledFrames(view, width)) {
    ctx.fillRect(x(f), h - 6, 1, 6);
    ctx.fillText(m.labels.get(f) ?? String(f), x(f) + 2, 2);
  }
  if (m.range.mode === 'free') {
    ctx.fillStyle = GREY.current;
    for (const f of [m.range.start, m.range.end]) {
      if (f < win.first || f > win.last) continue;
      const cx = x(f) + cw / 2;
      ctx.beginPath();
      ctx.moveTo(cx - LAYOUT.rangeMarkerHalfWidth, h);
      ctx.lineTo(cx + LAYOUT.rangeMarkerHalfWidth, h);
      ctx.lineTo(cx, h - 8);
      ctx.closePath();
      ctx.fill();
    }
  }
  if (m.currentFrame >= win.first && m.currentFrame <= win.last) {
    ctx.fillStyle = GREY.playhead;
    ctx.fillRect(x(m.currentFrame), 0, Math.max(2, cw), h);
  }
}
