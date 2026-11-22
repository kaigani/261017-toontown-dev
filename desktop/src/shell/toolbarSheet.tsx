// The toolbar customisation sheet (01 §5.7 lines 445-464; M4 C4.4a, reviews/M4-design-panels.md §7): the main
// toolbar's items and the items it allows, with a filter. Add inserts after the selection, Remove takes the selection
// out, Move Up and Move Down (Alt+Up, Alt+Down) reorder it; the edits are staged in the sheet and Apply sends one
// `window.toolbar.items.set {toolbarId: main, items}` (workspace state, no undo step); Cancel discards them. Restore
// Default sends `window.toolbar.items.reset` at once, as the Window menu's item does, and closes the sheet.
import { useEffect, useMemo, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import type { ShellServices } from '../state/context';
import { Dialog } from './dialogs';
import { ITEM_LABELS } from './toolbarItems';

type Json = Record<string, unknown>;

/** An item's label: a group item's or a command's, `separator` and the spaces by name. */
function toolbarItemLabel(item: string): string {
  if (item === 'separator') return '— separator —';
  if (item === 'space') return '(space)';
  if (item === 'flexibleSpace') return '(flexible space)';
  return ITEM_LABELS[item] ?? (COMMAND_INDEX as Record<string, { label?: string }>)[item]?.label ?? item;
}

export function ToolbarSheet({
  services,
  close,
}: {
  services: ShellServices;
  close: (r: Record<string, unknown> | null) => void;
}) {
  const [items, setItems] = useState<string[] | null>(null);
  const [allowed, setAllowed] = useState<string[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [candidate, setCandidate] = useState<string | null>(null);
  const [filter, setFilter] = useState('');
  useEffect(() => {
    void services.engine.run('window.toolbar.list', {}).done.then((env) => {
      const list = Array.isArray(env.result) ? (env.result as Json[]) : [];
      const main = list.find((t) => t['toolbarId'] === 'main') ?? {};
      setItems(Array.isArray(main['items']) ? (main['items'] as string[]) : []);
      setAllowed(Array.isArray(main['allowedItems']) ? (main['allowedItems'] as string[]) : []);
    });
  }, [services]);
  const shownAllowed = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return allowed.filter(
      (a) => !f || a.toLowerCase().includes(f) || toolbarItemLabel(a).toLowerCase().includes(f),
    );
  }, [allowed, filter]);
  const list = items ?? [];
  const add = () => {
    if (!candidate) return;
    const at = selected === null ? list.length : selected + 1;
    setItems([...list.slice(0, at), candidate, ...list.slice(at)]);
    setSelected(at);
  };
  const remove = () => {
    if (selected === null) return;
    setItems(list.filter((_, i) => i !== selected));
    setSelected(list.length > 1 ? Math.min(selected, list.length - 2) : null);
  };
  const move = (by: -1 | 1) => {
    if (selected === null) return;
    const to = selected + by;
    if (to < 0 || to >= list.length) return;
    const next = [...list];
    const a = next[selected];
    const b = next[to];
    if (a === undefined || b === undefined) return;
    next[selected] = b;
    next[to] = a;
    setItems(next);
    setSelected(to);
  };
  return (
    <Dialog
      id="toolbarSheet"
      title="Customize Toolbar"
      command="window.toolbar.items.set"
      submitLabel="Apply"
      wide
      submitDisabled={items === null}
      onCancel={() => {
        close(null);
      }}
      onSubmit={() => {
        close({ toolbarId: 'main', items: list });
      }}
    >
      <div className="toolbar-sheet">
        <div className="toolbar-sheet-list">
          <h3>Main toolbar</h3>
          <ul role="listbox" aria-label="Main toolbar items">
            {list.map((item, i) => (
              <li key={`${item}-${String(i)}`}>
                <button
                  type="button"
                  role="option"
                  aria-selected={selected === i}
                  data-tt-role="button"
                  data-tt-id={`toolbarSheet.item.${String(i)}`}
                  data-tt-label={toolbarItemLabel(item)}
                  data-tt-gui-only="navigate"
                  onClick={() => {
                    setSelected(i);
                  }}
                  onKeyDown={(e) => {
                    if (e.altKey && e.key === 'ArrowUp') move(-1);
                    if (e.altKey && e.key === 'ArrowDown') move(1);
                    if (e.key === 'Delete') remove();
                  }}
                >
                  {toolbarItemLabel(item)}
                </button>
              </li>
            ))}
          </ul>
          <div className="buttons">
            {(
              [
                ['remove', 'Remove', remove, selected === null],
                [
                  'up',
                  'Move Up',
                  () => {
                    move(-1);
                  },
                  selected === null || selected === 0,
                ],
                [
                  'down',
                  'Move Down',
                  () => {
                    move(1);
                  },
                  selected === null || selected === list.length - 1,
                ],
              ] as const
            ).map(([id, label, act, off]) => (
              <button
                key={id}
                type="button"
                disabled={off}
                data-tt-role="button"
                data-tt-id={`toolbarSheet.${id}`}
                data-tt-gui-only="typing"
                onClick={act}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="toolbar-sheet-list">
          <h3>Available items</h3>
          <input
            type="search"
            value={filter}
            placeholder="Filter"
            aria-label="Filter the available items"
            data-tt-role="field"
            data-tt-id="toolbarSheet.filter"
            data-tt-gui-only="typing"
            onChange={(e) => {
              setFilter(e.target.value);
            }}
          />
          <ul role="listbox" aria-label="Available items" className="toolbar-sheet-allowed">
            {shownAllowed.map((item) => (
              <li key={item}>
                <button
                  type="button"
                  role="option"
                  aria-selected={candidate === item}
                  data-tt-role="button"
                  data-tt-id={`toolbarSheet.allowed.${item}`}
                  data-tt-label={toolbarItemLabel(item)}
                  data-tt-gui-only="navigate"
                  onClick={() => {
                    setCandidate(item);
                  }}
                  onDoubleClick={() => {
                    setCandidate(item);
                    add();
                  }}
                >
                  {toolbarItemLabel(item)}
                </button>
              </li>
            ))}
          </ul>
          <div className="buttons">
            <button
              type="button"
              disabled={candidate === null}
              data-tt-role="button"
              data-tt-id="toolbarSheet.add"
              data-tt-gui-only="typing"
              onClick={add}
            >
              Add
            </button>
            <button
              type="button"
              data-tt-role="button"
              data-tt-id="toolbarSheet.restore"
              data-tt-command="window.toolbar.items.reset"
              data-tt-params="{}"
              onClick={() => {
                void services.engine.run('window.toolbar.items.reset', {}).done.then(() => {
                  close(null);
                });
              }}
            >
              Restore Default
            </button>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
