// Front-end side of the UI-bindings contract. The Rust side (desktop/src-tauri/src/bindings.rs) validates the
// same file structurally; this test checks the front end handles every command the manifest routes to it.
import { describe, expect, it } from 'vitest';

import { FRONTEND_COMMANDS } from '../commands';
import manifest from './menu.json';

interface CommandItem {
  kind: 'command';
  command: string;
  handler: 'host' | 'frontend' | 'pending';
  milestone?: string;
}

const commandItems = manifest.menus.flatMap((m) =>
  m.items.filter((i): i is typeof i & CommandItem => i.kind === 'command'),
);

describe('menu bindings manifest', () => {
  it('routes only to commands the front end implements', () => {
    const routed = new Set(commandItems.filter((c) => c.handler === 'frontend').map((c) => c.command));
    expect([...routed].sort()).toEqual(Object.keys(FRONTEND_COMMANDS).sort());
  });

  it('uses command ids of the form namespace.object[.verb]', () => {
    for (const c of commandItems) expect(c.command).toMatch(/^[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)+$/);
  });

  it('gives every pending command a milestone', () => {
    for (const c of commandItems.filter((x) => x.handler === 'pending')) expect(c.milestone).toBeTruthy();
  });

  it('has File, Edit, View and Help menus', () => {
    expect(manifest.menus.map((m) => m.id)).toEqual(expect.arrayContaining(['file', 'edit', 'view', 'help']));
  });
});
