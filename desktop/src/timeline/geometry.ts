// The Timeline's preview arithmetic (M5 C5.1b; reviews/M5-design-timeline-view.md section 3): the frame under x, the
// row under y, the visible window, the modifier roles. The kernel's timing-gesture interpreter decides what a gesture
// does (crates/executor/src/timing_gesture.rs); this file only lets the window draw the preview between the press and
// the release. Both read the same layout (`timeline-layout.json`, a byte copy of the kernel's, kept equal by a test)
// and check the same case table (`timeline-geometry-cases.json`), so they cannot drift.
import layoutJson from './timeline-layout.json';

export interface ZoomLevel {
  readonly cellWidth: number;
  readonly framesPerCell: number;
}

export interface Modifiers {
  readonly extend: readonly string[];
  readonly toggle: readonly string[];
  readonly singleCell: readonly string[];
  readonly copy: readonly string[];
  readonly link: readonly string[];
}

export interface Layout {
  readonly rowHeight: number;
  readonly rulerHeight: number;
  readonly zoomLevels: readonly ZoomLevel[];
  readonly defaultZoomLevel: number;
  readonly startHandleWidth: number;
  readonly endHandleWidth: number;
  readonly keyframeRadius: number;
  readonly dragThreshold: number;
  readonly dropZoneBetween: number;
  readonly emptyFramesPastEnd: number;
  readonly rangeMarkerHalfWidth: number;
  readonly modifiers: Readonly<Record<string, Modifiers>>;
}

export const LAYOUT: Layout = layoutJson;

/** The Timeline's view position as the kernel keeps it (`timeline.view.set`). */
export interface ViewPosition {
  readonly zoomLevel: number;
  readonly firstFrame: number;
}

/** A zoom level by index, clamped to the ladder (the kernel's `Layout::zoom`). */
export function zoomOf(index: number, l: Layout = LAYOUT): ZoomLevel {
  const last = Math.max(0, l.zoomLevels.length - 1);
  const i = Math.min(Math.max(0, Math.floor(index)), last);
  return l.zoomLevels[i] ?? { cellWidth: 16, framesPerCell: 1 };
}

/** The highest zoom index. */
export function maxZoom(l: Layout = LAYOUT): number {
  return Math.max(0, l.zoomLevels.length - 1);
}

/** The frame under `x` in the tracks or the ruler: the first frame of the cell under it (the kernel's `frame_at`). */
export function frameAt(v: ViewPosition, x: number, l: Layout = LAYOUT): number {
  const z = zoomOf(v.zoomLevel, l);
  const raw = Math.floor(x / Math.max(1, z.cellWidth));
  const cell = Number.isFinite(raw) ? raw : 0;
  return Math.max(1, v.firstFrame + cell * Math.max(1, z.framesPerCell));
}

/** The left edge of the cell holding `frame` (the kernel's `x_of`). */
export function xOf(v: ViewPosition, frame: number, l: Layout = LAYOUT): number {
  const z = zoomOf(v.zoomLevel, l);
  const per = Math.max(1, z.framesPerCell);
  const cells = Math.floor((frame - v.firstFrame) / per);
  return cells * z.cellWidth;
}

/** The row index under `y` (from the top of the first row), or null above the first row. */
export function rowAt(y: number, scrollTop: number, rows: number, l: Layout = LAYOUT): number | null {
  const r = Math.floor((y + scrollTop) / Math.max(1, l.rowHeight));
  return Number.isFinite(r) && r >= 0 && r < rows ? r : null;
}

/** How many frames a width shows at a zoom level (the kernel's `visible_frames`). */
export function visibleFrames(width: number, zoomLevel: number, l: Layout = LAYOUT): number {
  const z = zoomOf(zoomLevel, l);
  return Math.max(1, Math.floor(width / Math.max(1, z.cellWidth))) * Math.max(1, z.framesPerCell);
}

/** The rows a list of `count` rows shows at `scrollTop` in `height` pixels, with `margin` rows above and below
 *  (the virtualised layer list keeps only these in the DOM). */
export function rowWindow(
  count: number,
  scrollTop: number,
  height: number,
  margin = 10,
  l: Layout = LAYOUT,
): { first: number; last: number } {
  if (count <= 0) return { first: 0, last: -1 };
  const top = Math.floor(Math.max(0, scrollTop) / l.rowHeight);
  const shown = Math.ceil(Math.max(0, height) / l.rowHeight) + 1;
  return { first: Math.max(0, top - margin), last: Math.min(count - 1, top + shown + margin) };
}

export type Platform = 'windows' | 'macos' | 'linux';

/** The platform the modifier roles follow (SI-211). */
export function platformOf(userAgent: string): Platform {
  if (/Mac/i.test(userAgent)) return 'macos';
  if (/Linux|X11/i.test(userAgent)) return 'linux';
  return 'windows';
}

/** The modifiers held in a pointer event, in the kernel's names. */
export function modifiersOf(e: {
  shiftKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
}): string[] {
  const m: string[] = [];
  if (e.shiftKey) m.push('shift');
  if (e.altKey) m.push('alt');
  if (e.ctrlKey) m.push('ctrl');
  if (e.metaKey) m.push('meta');
  return m;
}

/** Whether every modifier of a role is held (the kernel's `held`). */
export function holds(role: readonly string[], held: readonly string[]): boolean {
  return role.length > 0 && role.every((m) => held.some((h) => h.toLowerCase() === m.toLowerCase()));
}

/** The roles of a platform (falls back to Windows', as the kernel does). */
export function rolesOf(platform: Platform, l: Layout = LAYOUT): Modifiers {
  return (
    l.modifiers[platform] ??
    l.modifiers['windows'] ?? { extend: [], toggle: [], singleCell: [], copy: [], link: [] }
  );
}

/** The cell-move mode a drag of selected cells takes from the modifiers at the release (the kernel's rule: link before
 *  copy, since link's modifiers include copy's on Windows). */
export function moveMode(
  platform: Platform,
  held: readonly string[],
  l: Layout = LAYOUT,
): 'move' | 'copy' | 'link' {
  const r = rolesOf(platform, l);
  if (holds(r.link, held)) return 'link';
  if (holds(r.copy, held)) return 'copy';
  return 'move';
}

/** What a press in a row of the tracks hits (ruling E-35; the rule is stated in `timeline-layout.json`). */
export type Zone =
  | { readonly zone: 'keyframe'; readonly frame: number }
  | { readonly zone: 'loopHandle' | 'barEnd' | 'barStart' | 'barBody' | 'cell' };

/** The zone under `x` in a row, in the kernel's priority order (`timing_gesture::zone_at`), each zone including its edge
 *  pixels: a keyframe symbol (`keys`: the keyed frames in the row's keyframe band, empty outside it; the earliest within
 *  the radius), the end column (the loop handle in the upper half of a loopable bar, else the end handle), the start
 *  handle, the body, a cell. `bar` is the bar's first and last frame (its first cycle). */
export function zoneAt(
  v: ViewPosition,
  x: number,
  upper: boolean,
  bar: readonly [number, number] | null,
  loopable: boolean,
  keys: readonly number[],
  l: Layout = LAYOUT,
): Zone {
  const cw = zoomOf(v.zoomLevel, l).cellWidth;
  const key = keys.find((k) => Math.abs(x - (xOf(v, k, l) + cw / 2)) <= l.keyframeRadius);
  if (key !== undefined) return { zone: 'keyframe', frame: key };
  if (bar) {
    const xs = xOf(v, bar[0], l);
    const xe = xOf(v, bar[1], l) + cw;
    if (Math.abs(x - xe) <= l.endHandleWidth) return { zone: upper && loopable ? 'loopHandle' : 'barEnd' };
    if (Math.abs(x - xs) <= l.startHandleWidth) return { zone: 'barStart' };
    if (x > xs && x < xe) return { zone: 'barBody' };
  }
  return { zone: 'cell' };
}
