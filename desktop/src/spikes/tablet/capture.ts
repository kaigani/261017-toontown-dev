// C0.2 tablet spike (spike code): web-path capture of pointer events, including coalesced events, per animation
// frame. Measures what the webview delivers; it is not the product's input pipeline (that arrives in M4).
import type { FrameRecord, WebSample } from './analysis';

export const MAX_SAMPLES = 200_000;

// lib.dom declares getCoalescedEvents as always present, but older WebKit lacks it: feature-detect it.
interface MaybeCoalescing {
  getCoalescedEvents?: (this: PointerEvent) => PointerEvent[];
}

function sample(
  e: PointerEvent,
  kind: WebSample['kind'],
  dispatch: number,
  count: number,
  handlerT: number,
  index = 0,
): WebSample {
  return {
    t: e.timeStamp,
    handlerT,
    kind,
    pointerType: e.pointerType,
    pointerId: e.pointerId,
    x: e.clientX,
    y: e.clientY,
    pressure: e.pressure,
    tiltX: e.tiltX,
    tiltY: e.tiltY,
    twist: e.twist,
    dispatch,
    coalescedCount: count,
    coalescedIndex: index,
  };
}

/** Records pointer samples on `element` until `stop()` is called. */
export class WebCapture {
  readonly samples: WebSample[] = [];
  readonly frames: FrameRecord[] = [];
  /** Whether `getCoalescedEvents` exists in this webview at all. */
  readonly coalescedApi: boolean;
  /** Samples not kept because MAX_SAMPLES was reached (they still reached `onSample`). */
  truncated = 0;
  private dispatch = 0;
  private pendingDispatched = 0;
  private pendingSamples = 0;
  private raf = 0;
  private readonly onSample: (s: WebSample) => void;
  private readonly element: HTMLElement;

  constructor(element: HTMLElement, onSample: (s: WebSample) => void = () => undefined) {
    this.element = element;
    this.onSample = onSample;
    this.coalescedApi =
      typeof (PointerEvent.prototype as unknown as MaybeCoalescing).getCoalescedEvents === 'function';
    element.addEventListener('pointerdown', this.down);
    element.addEventListener('pointermove', this.move);
    element.addEventListener('pointerup', this.up);
    element.addEventListener('pointercancel', this.cancel);
    this.raf = requestAnimationFrame(this.frame);
  }

  stop(): void {
    this.element.removeEventListener('pointerdown', this.down);
    this.element.removeEventListener('pointermove', this.move);
    this.element.removeEventListener('pointerup', this.up);
    this.element.removeEventListener('pointercancel', this.cancel);
    cancelAnimationFrame(this.raf);
  }

  clear(): void {
    this.samples.length = 0;
    this.frames.length = 0;
    this.truncated = 0;
  }

  private push(s: WebSample): void {
    if (this.samples.length < MAX_SAMPLES) this.samples.push(s);
    else this.truncated++;
    this.onSample(s);
  }

  private readonly down = (e: PointerEvent): void => {
    this.element.setPointerCapture(e.pointerId);
    e.preventDefault();
    this.push(sample(e, 'down', this.dispatch, 1, performance.now()));
  };

  private readonly move = (e: PointerEvent): void => {
    const handlerT = performance.now();
    const get = (e as unknown as MaybeCoalescing).getCoalescedEvents;
    const coalesced = typeof get === 'function' ? get.call(e) : [];
    const events = coalesced.length > 0 ? coalesced : [e];
    this.dispatch++;
    this.pendingDispatched++;
    this.pendingSamples += events.length;
    events.forEach((c, i) => {
      this.push(sample(c, 'move', this.dispatch, events.length, handlerT, i));
    });
  };

  private readonly up = (e: PointerEvent): void => {
    this.push(sample(e, 'up', this.dispatch, 1, performance.now()));
  };

  private readonly cancel = (e: PointerEvent): void => {
    this.push({ ...sample(e, 'up', this.dispatch, 1, performance.now()), cancelled: true });
  };

  private readonly frame = (t: number): void => {
    if (this.pendingDispatched > 0 || this.pendingSamples > 0) {
      this.frames.push({ t, dispatched: this.pendingDispatched, samples: this.pendingSamples });
    }
    this.pendingDispatched = 0;
    this.pendingSamples = 0;
    this.raf = requestAnimationFrame(this.frame);
  };
}
