// The one context-menu mechanism (01 §5.3 line 373, §5.4 lines 387-388; M4 C4.4a, reviews/M4-design-panels.md §9):
// opened at the pointer by a secondary click, or at the focused control by the context-menu key or Shift+F10 (the
// browser's `contextmenu` event covers all three). A `role="menu"` of `menuitem` items (`menuitemcheckbox` with
// `aria-checked` for a check mark), in groups with a heading; arrows move, Home and End jump, Enter or Space activate,
// Escape, Tab or a press outside closes and returns focus. Every item is a command reference with the main menu's
// availability (ruling O-10): until the kernel has answered it is disabled, a disabled item shows the kernel's reason,
// a toggle's reference is sent as the kernel completes it, and a choice is performed as a menu click is (a command
// whose parameters the user must give opens its dialog). The menu is always fully inside the window's
// client area (D-60 item 1, `placeMenu`): measured before it is painted, it opens toward the side with room,
// flips or shifts at an edge, and a menu taller than the window scrolls, with an arrow at each end that shows
// there is more and scrolls to it.
// D-66 item 1 (C44A-R1-01, at 800 by 500): the menu's outer box (border and padding included: it is sized
// `border-box`, and has no margin) and both arrows fit the client area at every window size down to the product's
// minimum (the main window's 800 by 500, a float's 160 by 120). An arrow scrolls on a click and while the pointer rests
// on it; moving the focus with the keyboard scrolls the focused item fully into view; the wheel scrolls the items. The
// menu's natural size is measured without losing the items' scroll position (measuring it unbounded had reset it, so
// no scrolling stayed).
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';

import { createPortal } from 'react-dom';

import { useActions } from './actionsApi';
import type { CommandRef } from './availability';
import {
  MENU_MARGIN,
  placeMenu,
  type MenuAt,
  type MenuCommand,
  type MenuGroup,
  type MenuPlacement,
} from './contextMenuModel';
import { enabledIn, useAvailability } from './useAvailability';

const refOf = (i: MenuCommand): CommandRef => ({ commandId: i.command, params: i.params ?? {} });

/** The window's client area (without scroll bars), in CSS pixels. */
function clientArea(): { width: number; height: number } {
  const d = document.documentElement;
  return { width: d.clientWidth || window.innerWidth, height: d.clientHeight || window.innerHeight };
}

/** How far one press on a scroll arrow scrolls: about three items. */
export const ARROW_STEP_PX = 72;
/** While the pointer rests on (or holds) an arrow: after this delay, the items scroll by `HOLD_STEP_PX` every
 *  `HOLD_REPEAT_MS` until the end is reached or the pointer leaves. */
export const HOLD_DELAY_MS = 250;
export const HOLD_REPEAT_MS = 40;
export const HOLD_STEP_PX = 12;

/** Scrolls `list` so that `item` is fully inside its visible part (D-66 item 1: keyboard focus never rests on an item
 *  outside the client area). */
function reveal(list: HTMLElement, item: HTMLElement): void {
  const l = list.getBoundingClientRect();
  const r = item.getBoundingClientRect();
  if (r.top < l.top) list.scrollTop -= l.top - r.top;
  else if (r.bottom > l.bottom) list.scrollTop += Math.min(r.bottom - l.bottom, r.top - l.top);
}

export function ContextMenu({
  id,
  label,
  at,
  groups,
  onClose,
}: {
  id: string;
  label: string;
  at: MenuAt;
  groups: readonly MenuGroup[];
  onClose: () => void;
}) {
  const actions = useActions();
  const refs = useMemo(() => groups.flatMap((g) => g.items.map(refOf)), [groups]);
  const availability = useAvailability(refs);
  const root = useRef<HTMLDivElement | null>(null);
  const opener = useRef<Element | null>(null);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  const list = useRef<HTMLDivElement | null>(null);
  const [place, setPlace] = useState<MenuPlacement | null>(null);
  // Where `left: 0; top: 0` puts the menu in the client area: an ancestor (the dock's tab) can be the containing block
  // of a fixed element, so the placement, in client-area coordinates, is applied relative to that origin.
  const [origin, setOrigin] = useState({ x: 0, y: 0 });
  const [more, setMore] = useState({ above: false, below: false });

  // Placed from its natural size before it is painted (D-60 item 1), measured at the client area's corner so the
  // edge does not narrow it; measured again after every render (the kernel's availability answers add reasons and
  // labels, which change its size: the live run saw a menu grow 15 px after it was placed) and when the window's size
  // changes. Once shown it keeps its place while its new size still fits inside the client area, so the item under
  // the pointer does not move away from it; it is placed again only when it would cross an edge.
  const measure = useCallback(() => {
    const el = root.current;
    if (!el) return;
    const prev = {
      left: el.style.left,
      top: el.style.top,
      maxHeight: el.style.maxHeight,
      maxWidth: el.style.maxWidth,
      minWidth: el.style.minWidth,
    };
    // Unbounded for a moment the items do not overflow, and the browser drops their scroll position: it is kept and
    // put back (D-66 item 1: without this, every scroll re-rendered, re-measured and jumped back to the top).
    const scrollTop = list.current?.scrollTop ?? 0;
    el.style.left = '0px';
    el.style.top = '0px';
    el.style.maxHeight = '';
    el.style.maxWidth = '';
    el.style.minWidth = '';
    const size = { width: el.offsetWidth, height: el.offsetHeight };
    const at0 = el.getBoundingClientRect();
    Object.assign(el.style, prev);
    if (list.current && list.current.scrollTop !== scrollTop) list.current.scrollTop = scrollTop;
    setOrigin((o) => (o.x === at0.left && o.y === at0.top ? o : { x: at0.left, y: at0.top }));
    const area = clientArea();
    const next = placeMenu(at, size, area);
    const keeps = (old: MenuPlacement | null) =>
      old !== null &&
      ((old.x === next.x &&
        old.y === next.y &&
        old.maxHeight === next.maxHeight &&
        old.maxWidth === next.maxWidth) ||
        (old.maxHeight === null &&
          old.maxWidth === null &&
          next.maxHeight === null &&
          next.maxWidth === null &&
          old.x >= MENU_MARGIN &&
          old.y >= MENU_MARGIN &&
          old.x + size.width <= area.width - MENU_MARGIN &&
          old.y + size.height <= area.height - MENU_MARGIN));
    setPlace((old) => (keeps(old) ? old : next));
  }, [at]);
  useLayoutEffect(() => {
    measure();
  });
  useEffect(() => {
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('resize', measure);
    };
  }, [measure]);
  const scrolled = useCallback(() => {
    const l = list.current;
    if (!l) return;
    const above = l.scrollTop > 0;
    const below = l.scrollTop + l.clientHeight < l.scrollHeight - 1;
    // Unchanged: no new render (a render measures the menu again).
    setMore((m) => (m.above === above && m.below === below ? m : { above, below }));
  }, []);
  useLayoutEffect(() => {
    scrolled();
  }, [place, scrolled]);
  const scrollList = useCallback(
    (dy: number) => {
      const l = list.current;
      if (!l) return;
      l.scrollTop = Math.min(Math.max(l.scrollTop + dy, 0), Math.max(l.scrollHeight - l.clientHeight, 0));
      scrolled();
    },
    [scrolled],
  );
  // An arrow scrolls while the pointer rests on it or holds it (D-66 item 1).
  const hold = useRef<{ delay: number | null; repeat: number | null }>({ delay: null, repeat: null });
  const stopHold = useCallback(() => {
    const h = hold.current;
    if (h.delay !== null) window.clearTimeout(h.delay);
    if (h.repeat !== null) window.clearInterval(h.repeat);
    hold.current = { delay: null, repeat: null };
  }, []);
  const startHold = useCallback(
    (dir: 1 | -1) => {
      stopHold();
      hold.current.delay = window.setTimeout(() => {
        hold.current.delay = null;
        hold.current.repeat = window.setInterval(() => {
          scrollList(dir * HOLD_STEP_PX);
        }, HOLD_REPEAT_MS);
      }, HOLD_DELAY_MS);
    },
    [scrollList, stopHold],
  );
  useEffect(() => stopHold, [stopHold]);
  const arrowProps = (dir: 1 | -1) => ({
    onPointerDown: (e: { preventDefault: () => void }) => {
      // Not a press outside, and the focused item keeps the focus.
      e.preventDefault();
      scrollList(dir * ARROW_STEP_PX);
      startHold(dir);
    },
    onPointerEnter: () => {
      startHold(dir);
    },
    onPointerLeave: stopHold,
  });

  // Focus moves into the menu when it opens and back to what had it when it closes.
  useLayoutEffect(() => {
    opener.current = document.activeElement;
    root.current?.querySelector<HTMLElement>('[role^="menuitem"]')?.focus();
    return () => {
      const back = opener.current;
      if (back instanceof HTMLElement && back.isConnected) back.focus();
    };
  }, []);
  // A press anywhere outside the menu closes it.
  useEffect(() => {
    const outside = (e: PointerEvent) => {
      if (root.current && e.target instanceof Node && !root.current.contains(e.target)) close.current();
    };
    window.addEventListener('pointerdown', outside, true);
    return () => {
      window.removeEventListener('pointerdown', outside, true);
    };
  }, []);

  const items = () => [...(root.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])];
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const all = items();
    const at = all.indexOf(document.activeElement as HTMLElement);
    const go = (i: number) => {
      e.preventDefault();
      const next = all[(i + all.length) % all.length];
      if (!next) return;
      next.focus({ preventScroll: true });
      if (list.current) reveal(list.current, next);
      scrolled();
    };
    if (e.key === 'ArrowDown') go(at + 1);
    else if (e.key === 'ArrowUp') go(at < 0 ? all.length - 1 : at - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(all.length - 1);
    else if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };

  const scrolls = place !== null && place.maxHeight !== null;
  // Rendered at the top of the page (a portal into the document's body), whoever opened it: inside the dock's tab or a
  // view it would sit in that container's stacking context, drawn beneath later panels, and a person's click on an
  // item would land on the panel above it (D-60 item 1: every item reachable).
  return createPortal(
    <div
      ref={root}
      className={scrolls ? 'context-menu scrolls' : 'context-menu'}
      role="menu"
      aria-label={label}
      // Measured at the client area's corner, then placed in the same layout pass: the first paint is in
      // place (and the first item can take the focus, which a hidden menu could not).
      style={
        place
          ? {
              left: place.x - origin.x,
              top: place.y - origin.y,
              // The bounds are the outer box's: border and padding inside them (D-66 item 1).
              boxSizing: 'border-box',
              ...(place.maxHeight !== null ? { maxHeight: place.maxHeight } : {}),
              // A bounded width wins over the menu's least width (a float's 160 px client area).
              ...(place.maxWidth !== null ? { maxWidth: place.maxWidth, minWidth: 0 } : {}),
            }
          : { left: 0, top: 0, boxSizing: 'border-box' }
      }
      data-tt-role="menu"
      data-tt-id={id}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => {
        e.preventDefault();
      }}
    >
      {scrolls && (
        <div
          className={more.above ? 'context-menu-arrow' : 'context-menu-arrow spent'}
          aria-hidden="true"
          data-tt-arrow="up"
          {...arrowProps(-1)}
        >
          {'\u25B2'}
        </div>
      )}
      <div ref={list} className="context-menu-items" onScroll={scrolled}>
        {groups.map((g, gi) => (
          <div
            key={g.label ?? `group-${String(gi)}`}
            role="group"
            aria-label={g.label}
            className="context-menu-group"
          >
            {gi > 0 && <div role="separator" className="context-menu-separator" />}
            {g.label && (
              <div className="context-menu-heading" aria-hidden="true">
                {g.label}
              </div>
            )}
            {g.items.map((item) => {
              const facts = availability(refOf(item));
              const enabled = enabledIn(facts);
              // The parameters as the kernel completes them now (a toggle's next state; O-10).
              const params =
                facts && facts.params !== null && typeof facts.params === 'object'
                  ? (facts.params as Record<string, unknown>)
                  : (item.params ?? {});
              const text = facts?.label ?? item.label;
              const checked = item.check ? facts?.checked === true : undefined;
              return (
                <button
                  key={item.id}
                  type="button"
                  role={item.check ? 'menuitemcheckbox' : 'menuitem'}
                  {...(checked !== undefined ? { 'aria-checked': checked } : {})}
                  aria-disabled={!enabled}
                  title={!enabled && facts?.reason ? `${text} — ${facts.reason}` : undefined}
                  tabIndex={-1}
                  data-tt-role="menuitem"
                  data-tt-id={`${id}.${item.id}`}
                  data-tt-label={text}
                  data-tt-command={item.command}
                  data-tt-params={JSON.stringify(params)}
                  onClick={() => {
                    if (!enabled) return;
                    onClose();
                    void actions.perform(item.command, params);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      e.currentTarget.click();
                    }
                  }}
                >
                  <span className="context-menu-check" aria-hidden="true">
                    {checked ? '✓' : ''}
                  </span>
                  {text}
                  {!enabled && facts?.reason && <span className="context-menu-reason">{facts.reason}</span>}
                </button>
              );
            })}
          </div>
        ))}
      </div>
      {scrolls && (
        <div
          className={more.below ? 'context-menu-arrow' : 'context-menu-arrow spent'}
          aria-hidden="true"
          data-tt-arrow="down"
          {...arrowProps(1)}
        >
          {'\u25BC'}
        </div>
      )}
    </div>,
    document.body,
  );
}
