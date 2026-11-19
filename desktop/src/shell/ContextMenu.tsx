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
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';

import { useActions } from './actionsApi';
import type { CommandRef } from './availability';
import {
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
const ARROW_STEP_PX = 72;

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
  const [more, setMore] = useState({ above: false, below: false });

  // Placed from its natural size before it is painted (D-60 item 1); again when the window's size changes.
  const measure = useCallback(() => {
    const el = root.current;
    if (!el) return;
    const prev = { maxHeight: el.style.maxHeight, maxWidth: el.style.maxWidth };
    el.style.maxHeight = '';
    el.style.maxWidth = '';
    const size = { width: el.offsetWidth, height: el.offsetHeight };
    el.style.maxHeight = prev.maxHeight;
    el.style.maxWidth = prev.maxWidth;
    setPlace(placeMenu(at, size, clientArea()));
  }, [at]);
  useLayoutEffect(() => {
    measure();
  }, [measure, groups]);
  useEffect(() => {
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('resize', measure);
    };
  }, [measure]);
  const scrolled = useCallback(() => {
    const l = list.current;
    if (!l) return;
    setMore({ above: l.scrollTop > 0, below: l.scrollTop + l.clientHeight < l.scrollHeight - 1 });
  }, []);
  useLayoutEffect(() => {
    scrolled();
  }, [place, scrolled]);
  const scrollList = (dy: number) => {
    list.current?.scrollBy({ top: dy });
    scrolled();
  };

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
      all[(i + all.length) % all.length]?.focus();
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
  return (
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
              left: place.x,
              top: place.y,
              ...(place.maxHeight !== null ? { maxHeight: place.maxHeight } : {}),
              ...(place.maxWidth !== null ? { maxWidth: place.maxWidth } : {}),
            }
          : { left: 0, top: 0 }
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
          onPointerDown={(e) => {
            // Not a press outside, and the focused item keeps the focus.
            e.preventDefault();
            scrollList(-ARROW_STEP_PX);
          }}
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
          onPointerDown={(e) => {
            e.preventDefault();
            scrollList(ARROW_STEP_PX);
          }}
        >
          {'\u25BC'}
        </div>
      )}
    </div>
  );
}
