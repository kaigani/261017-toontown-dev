// The window's input path (M4 C4.2b; design note 2 §2 and §6; design note 3 §2; ruling D-9): one input abstraction
// over pointer events producing the `pointer` request's events, and the overlay of a gesture in progress (02 §4.2
// layer 9; SI-138). Pure: the DOM wiring is in `CanvasView.tsx`, so the tests drive it with synthetic events.
//
// Rules (design note 2 §6):
// - a gesture is press, samples as given (coalesced events included, no resampling), release; or press and cancel;
// - `dt` is the difference of consecutive `timeStamp`s in whole milliseconds;
// - a mouse press has pressure 1 (02 §13.3); a pen reports its own; the pen's eraser button or end gives `penEraser`;
// - nothing is sent before the release; a cancel (Escape, a lost capture, the window closing) sends nothing.

import type { Rgba } from './scene';
import { ViewTris } from './geometry';

export type PointerKind = 'mouse' | 'pen' | 'penEraser';
export type Modifier = 'shift' | 'alt' | 'ctrl' | 'meta';

/** One DOM pointer sample, as `CanvasView` reads it from a `PointerEvent` (or a coalesced one). */
export interface DomSample {
  x: number;
  y: number;
  pressure: number;
  tiltX: number;
  tiltY: number;
  pointerType: string;
  /** `PointerEvent.button` at the press (0 left, 1 middle, 2 right). */
  button: number;
  /** `PointerEvent.buttons` (bit 32: the pen's eraser). */
  buttons: number;
  shiftKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  timeStamp: number;
}

/** One event of the `pointer` request (the Rust `gesture::PointerEvent`). */
export interface WireEvent {
  type: 'down' | 'move' | 'up' | 'wheel';
  x: number;
  y: number;
  pressure?: number;
  tiltX?: number;
  tiltY?: number;
  button?: 'left' | 'right' | 'middle';
  modifiers: Modifier[];
  dt: number;
  wheelDelta?: number;
}

export interface GestureRequest {
  view: string;
  pointerType: PointerKind;
  events: WireEvent[];
}

export function kindOf(s: Pick<DomSample, 'pointerType' | 'buttons'>): PointerKind {
  if (s.pointerType === 'pen') return (s.buttons & 32) !== 0 ? 'penEraser' : 'pen';
  if (s.pointerType === 'eraser') return 'penEraser';
  return 'mouse';
}

function modifiers(s: DomSample): Modifier[] {
  const m: Modifier[] = [];
  if (s.shiftKey) m.push('shift');
  if (s.altKey) m.push('alt');
  if (s.ctrlKey) m.push('ctrl');
  if (s.metaKey) m.push('meta');
  return m;
}

const BUTTONS = ['left', 'middle', 'right'] as const;

const clampTilt = (t: number) => Math.max(-90, Math.min(90, Number.isFinite(t) ? t : 0));

/** Collects one gesture at a time. */
export class GestureCollector {
  private events: WireEvent[] = [];
  private kind: PointerKind = 'mouse';
  private last = 0;
  private button: 'left' | 'right' | 'middle' = 'left';
  /** The view points of the gesture so far (the overlay draws them). */
  points: [number, number][] = [];

  constructor(readonly view: string) {}

  get active(): boolean {
    return this.events.length > 0;
  }

  private event(type: WireEvent['type'], s: DomSample): WireEvent {
    const dt = this.events.length === 0 ? 0 : Math.max(0, Math.round(s.timeStamp - this.last));
    this.last = s.timeStamp;
    const e: WireEvent = { type, x: s.x, y: s.y, modifiers: modifiers(s), dt };
    // A mouse has pressure 1 (02 §13.3); a pen's own pressure, within 0..1.
    e.pressure =
      this.kind === 'mouse' ? 1 : Math.max(0, Math.min(1, Number.isFinite(s.pressure) ? s.pressure : 1));
    if (this.kind !== 'mouse') {
      e.tiltX = clampTilt(s.tiltX);
      e.tiltY = clampTilt(s.tiltY);
    }
    if (type === 'down') e.button = this.button;
    return e;
  }

  /** A press: starts a gesture (false when one is already in progress, or for a button no tool uses). */
  down(s: DomSample): boolean {
    if (this.active) return false;
    const b = BUTTONS[s.button];
    if (!b) return false;
    this.kind = kindOf(s);
    this.button = b;
    this.events = [this.event('down', s)];
    this.points = [[s.x, s.y]];
    return true;
  }

  /** Samples between the press and the release (coalesced ones included, in order). */
  move(samples: readonly DomSample[]): void {
    if (!this.active) return;
    for (const s of samples) {
      this.events.push(this.event('move', s));
      this.points.push([s.x, s.y]);
    }
  }

  /** The release: the gesture's request, and the collector is idle again. */
  up(s: DomSample): GestureRequest | null {
    if (!this.active) return null;
    this.events.push(this.event('up', s));
    this.points.push([s.x, s.y]);
    const req: GestureRequest = { view: this.view, pointerType: this.kind, events: this.events };
    this.events = [];
    this.points = [];
    return req;
  }

  /** Escape, a lost capture, the window closing: nothing is sent (D-7c, F5, F6). */
  cancel(): boolean {
    const was = this.active;
    this.events = [];
    this.points = [];
    return was;
  }

  /** A wheel step: a one-event gesture (navigation, 02 §4.3), refused while a gesture is in progress. */
  wheel(s: DomSample, delta: number): GestureRequest | null {
    if (this.active) return null;
    const e: WireEvent = { type: 'wheel', x: s.x, y: s.y, modifiers: modifiers(s), dt: 0, wheelDelta: delta };
    return { view: this.view, pointerType: kindOf(s), events: [e] };
  }
}

/** What the overlay draws with (SI-138). */
export interface OverlayStyle {
  tool: string;
  /** Ink colour (the current colour's value, else the accent). */
  ink: Rgba;
  /** Ink width in view pixels (the pen's size at the current zoom). */
  width: number;
}

/** The theme accent of selection and marquee (SI-138). */
export const ACCENT: Rgba = [32, 128, 255, 255];
const ERASER_INK: Rgba = [128, 128, 128, 140];

/** The overlay of a gesture in progress, in view pixels (layer 9). Empty for tools that show nothing while dragging. */
export function gestureOverlay(points: readonly [number, number][], style: OverlayStyle): ViewTris | null {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return null;
  const t = new ViewTris();
  const rect = (dashed: boolean) => {
    const r: [number, number][] = [first, [last[0], first[1]], last, [first[0], last[1]]];
    if (dashed) t.dashed(r, 1, ACCENT, 6, true);
    else t.polyline(r, Math.max(1, style.width), style.ink, true);
  };
  switch (style.tool) {
    case 'brush':
    case 'pencil':
    case 'polyline':
      t.polyline(points, Math.max(1, style.width), style.ink);
      break;
    case 'eraser':
      t.polyline(points, Math.max(1, style.width), ERASER_INK);
      break;
    case 'rectangle':
      rect(false);
      break;
    case 'ellipse': {
      const cx = (first[0] + last[0]) / 2;
      const cy = (first[1] + last[1]) / 2;
      const rx = Math.abs(last[0] - first[0]) / 2;
      const ry = Math.abs(last[1] - first[1]) / 2;
      const n = 64;
      const ring: [number, number][] = [];
      for (let i = 0; i < n; i++)
        ring.push([cx + rx * Math.cos((i / n) * 2 * Math.PI), cy + ry * Math.sin((i / n) * 2 * Math.PI)]);
      t.polyline(ring, Math.max(1, style.width), style.ink, true);
      break;
    }
    case 'line':
    case 'closeGap':
      t.line(first[0], first[1], last[0], last[1], Math.max(1, style.width), style.ink);
      break;
    case 'select':
    case 'zoom':
      rect(true);
      break;
    case 'cutter':
    case 'contourEditor':
      t.dashed(points, 1, ACCENT, 6, true);
      break;
    default:
      return null;
  }
  return t.vertexCount > 0 ? t : null;
}
