// The agent token's window-level controller (ruling D-69 item 1, C44A-R1-41; design note 2 §2). It belongs to the
// window, not to the Agent access page, so it outlives the component that shows the token:
// - a delivery that was painted is acknowledged hidden (`token_hidden`) only after its replacement has been painted
//   (two frames after the commit that removes it: Hide, leaving the page, closing Preferences), never by the
//   unmounting component itself;
// - a delivery never painted (a Copy, an answer of an ended reveal) is acknowledged at once;
// - an acknowledgement that does not reach the host is sent again until it does, so a delivery never stays outstanding
//   because one message was lost;
// - it reports whether the token field exists (`token_field`) and answers the host's `token-rect` requests with the
//   field's rectangle, which the host overpaints in a capture.
import type { EngineApi, TokenRect } from '../ipc';

/** How long the first resend waits; each later one waits twice as long, up to `RETRY_MAX_MS`. */
export const RETRY_FIRST_MS = 100;
export const RETRY_MAX_MS = 2000;

export interface TokenController {
  /** A delivery never painted: acknowledged now (and resent until the host has it). */
  hiddenNow(delivery: number): void;
  /** A painted delivery whose replacement is being committed: acknowledged once the replacement has been painted. */
  hiddenAfterPaint(delivery: number): void;
  /** The token field is mounted; `field` reads its element for the host's rectangle requests. */
  fieldMounted(field: () => HTMLElement | null): void;
  /** The token field is unmounted: reported once the page without it has been painted. */
  fieldUnmounted(): void;
}

/** Two animation frames: the commit that changed the page has been painted. */
function afterPaint(then: () => void): void {
  requestAnimationFrame(() => {
    requestAnimationFrame(then);
  });
}

function rectOf(el: HTMLElement | null): TokenRect | null {
  if (!el?.isConnected) return null;
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, width: r.width, height: r.height };
}

function create(engine: EngineApi): TokenController {
  let field: (() => HTMLElement | null) | null = null;
  const send = (delivery: number, wait: number) => {
    engine.tokenHidden(delivery).catch(() => {
      setTimeout(() => {
        send(delivery, Math.min(wait * 2, RETRY_MAX_MS));
      }, wait);
    });
  };
  // The host's rectangle requests, for the life of the window.
  void engine
    .onTokenRect((request) => {
      void engine.tokenRect(request, rectOf(field?.() ?? null)).catch(() => undefined);
    })
    .catch(() => undefined);
  return {
    hiddenNow(delivery) {
      send(delivery, RETRY_FIRST_MS);
    },
    hiddenAfterPaint(delivery) {
      afterPaint(() => {
        send(delivery, RETRY_FIRST_MS);
      });
    },
    fieldMounted(get) {
      field = get;
      void engine.tokenField(true).catch(() => undefined);
    },
    fieldUnmounted() {
      field = null;
      afterPaint(() => {
        if (field === null) void engine.tokenField(false).catch(() => undefined);
      });
    },
  };
}

const controllers = new WeakMap<EngineApi, TokenController>();

/** The window's controller (one per engine connection, created on first use). */
export function tokenController(engine: EngineApi): TokenController {
  let c = controllers.get(engine);
  if (!c) {
    c = create(engine);
    controllers.set(engine, c);
  }
  return c;
}
