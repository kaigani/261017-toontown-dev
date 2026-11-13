// Front-end side of the UI-bindings contract (manifest v2, M4 C4.1). The Rust side (desktop/src-tauri/src/bindings.rs)
// validates the same file structurally and the registry parity test diffs it against the registry; this test checks
// that every command reference is well formed and known to the generated command index the front end validates with.
import { describe, expect, it } from 'vitest';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { paramProblems } from '../ipc';
import manifest from './menu.json';

interface CommandItem {
  kind: 'command';
  command: string;
  params?: Record<string, unknown>;
  check?: string;
  dialog?: string;
}

interface AnyItem {
  kind: string;
  items?: AnyItem[];
}

function commands(items: readonly AnyItem[]): CommandItem[] {
  return items.flatMap((i) =>
    i.kind === 'command'
      ? [i as unknown as CommandItem]
      : i.kind === 'submenu'
        ? commands(i.items ?? [])
        : [],
  );
}

const commandItems = (manifest.menus as unknown as { items: AnyItem[] }[]).flatMap((m) => commands(m.items));

describe('menu bindings manifest', () => {
  it('is version 2', () => {
    expect(manifest.version).toBe(2);
  });

  it('uses command ids of the form namespace.object[.verb] that the generated index knows', () => {
    for (const c of commandItems) {
      expect(c.command).toMatch(/^[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)+$/);
      expect(Object.prototype.hasOwnProperty.call(COMMAND_INDEX, c.command), c.command).toBe(true);
    }
  });

  it('fixed parameters are members the command accepts', () => {
    for (const c of commandItems) {
      if (!c.params || c.dialog) continue;
      // Partial references (a toggle completed at click time) only need their given members to be valid.
      const problems = paramProblems(c.command, c.params).filter((p) => !p.includes('required'));
      expect(problems, `${c.command} ${JSON.stringify(c.params)}`).toEqual([]);
    }
  });

  it('has File, Edit, View, Window and Help menus', () => {
    expect(manifest.menus.map((m) => m.id)).toEqual(
      expect.arrayContaining(['file', 'edit', 'view', 'window', 'help']),
    );
  });

  it('every panel of 01 §5.2 has a checkable Window item', () => {
    const checks = commandItems.map((c) => c.check).filter((c): c is string => typeof c === 'string');
    expect(checks.filter((c) => c.startsWith('panel:'))).toHaveLength(21);
  });
});
