// The one context-menu mechanism (01 §5.3 line 373, §5.4 lines 387-388; M4 C4.4a, reviews/M4-design-panels.md §9):
// opened at the pointer by a secondary click, or at the focused control by the context-menu key or Shift+F10 (the
// browser's `contextmenu` event covers all three). A `role="menu"` of `menuitem` items (`menuitemcheckbox` with
// `aria-checked` for a check mark), in groups with a heading; arrows move, Home and End jump, Enter or Space activate,
// Escape, Tab or a press outside closes and returns focus. Every item is a command reference with the main menu's
// availability (ruling O-10): until the kernel has answered it is disabled, a disabled item shows the kernel's reason,
// a toggle's reference is sent as the kernel completes it, and a choice is performed as a menu click is (a command
// whose parameters the user must give opens its dialog).
import { useEffect, useLayoutEffect, useMemo, useRef, type KeyboardEvent } from 'react';

import { useActions } from './actionsApi';
import type { CommandRef } from './availability';
import type { MenuAt, MenuCommand, MenuGroup } from './contextMenuModel';
import { enabledIn, useAvailability } from './useAvailability';

const refOf = (i: MenuCommand): CommandRef => ({ commandId: i.command, params: i.params ?? {} });

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

  return (
    <div
      ref={root}
      className="context-menu"
      role="menu"
      aria-label={label}
      style={{ left: at.x, top: at.y }}
      data-tt-role="menu"
      data-tt-id={id}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => {
        e.preventDefault();
      }}
    >
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
  );
}
