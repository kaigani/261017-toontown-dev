// The parity gate, front-end half (M4 C4.3b; parity note 9, rulings D-25, D-30, D-31): PA1 (every actionable element
// of every rendered surface sends a command or is GUI-only for a listed reason) and PA4 (the manifest is the
// generator's output). `TT_PARITY_WRITE=1` (`pnpm parity:gen`) writes `parity.json` instead of comparing; the Rust
// half (`crates/executor/tests/c43/parity.rs`) checks PA2, PA3 and the kernel-derived sections.
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';

import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import {
  createScriptedHost,
  DEFAULT_LAYOUT,
  NO_PROJECT_STATE,
  ok,
  type ScriptedHost,
} from '../test-support/scriptedHost';
import { itemRef } from '../shell/toolbarItems';
import menu from './menu.json';
import { collect, GUI_ONLY_REASONS, type Action } from './parity';

afterEach(cleanup);

// Vitest runs from `desktop/` (the jsdom environment has no file URL for this module).
const repo = `${process.cwd().split('\\').join('/')}/../`;
const here = `${repo}desktop/src/ui-bindings/`;
const readJson = (path: string): unknown => JSON.parse(readFileSync(`${repo}${path}`, 'utf8')) as unknown;

const KEYMAP = readJson('crates/executor/src/keymap-default.json') as {
  bindings: { commandId: string; keys: string; params?: unknown; context?: string; platforms?: string[] }[];
};
const TOOLBAR = readJson('crates/executor/src/toolbar-default.json') as string[];
const GESTURES = readJson('crates/executor/src/gesture-table.json') as {
  rows: { view: string; tool: string; commands: string[] }[];
};
const TOOLS = [
  ...new Set(GESTURES.rows.filter((r) => r.view === 'drawing' && r.tool !== '*').map((r) => r.tool)),
];

const PROJECT_STATE = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: false,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

function host(project: boolean): ScriptedHost {
  return createScriptedHost({
    'app.state.get': () => ok(project ? PROJECT_STATE : NO_PROJECT_STATE),
    'window.layout.get': () =>
      ok({ ...DEFAULT_LAYOUT, mainToolbar: { visible: true, items: TOOLBAR }, fullScreen: DEFAULT_LAYOUT }),
    'tool.list': () => ok({ tools: TOOLS.map((toolId) => ({ toolId })), activeToolId: 'brush' }),
  });
}

function mount(h: ScriptedHost) {
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
}

/** The surfaces the gate renders, each with what opens it. */
async function surfaces(): Promise<{ name: string; root: Element }[]> {
  const out: { name: string; root: Element }[] = [];
  // The start screen (no project).
  mount(host(false));
  out.push({
    name: 'startScreen',
    root: (await screen.findByRole('dialog', { name: 'Start screen' })).cloneNode(true) as Element,
  });
  cleanup();
  // The main window with a project: toolbars, tool palettes, the dock's panels and view chrome, the status bar.
  const h = host(true);
  mount(h);
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  await screen.findByText('brush');
  out.push({ name: 'main', root: document.body.cloneNode(true) as Element });
  // Each dialog the menus open.
  const dialogs: [string, string, Record<string, unknown>?][] = [
    ['About Toontown', 'app.info.get'],
    ['Project Info', 'project.info.get'],
    ['Check Project', 'doc.validate'],
    ['Open Backup', 'project.backup.open'],
    ['New Project', 'project.create'],
    ['Project Properties', 'project.properties.set'],
    ['New Workspace', 'workspace.create'],
  ];
  for (const [title, command, params] of dialogs) {
    act(() => {
      h.menu(command, params ?? {});
    });
    const d = await screen.findByRole('dialog', { name: title });
    out.push({ name: `dialog:${title}`, root: d.cloneNode(true) as Element });
    act(() => {
      d.querySelector<HTMLElement>('[data-tt-id$=".cancel"]')?.click();
    });
  }
  // Preferences.
  act(() => {
    h.event('shell.preferences', { category: 'general' });
  });
  const prefs = await screen.findByRole('dialog', { name: 'Preferences' });
  out.push({ name: 'dialog:Preferences', root: prefs.cloneNode(true) as Element });
  cleanup();
  return out;
}

/** The menu manifest's command items (the native menu, built by the host from `menu.json`). */
function menuActions(): Action[] {
  const out: Action[] = [];
  const walk = (items: unknown[], path: string) => {
    for (const raw of items) {
      const i = raw as {
        kind: string;
        command?: string;
        label?: string;
        params?: unknown;
        items?: unknown[];
        id?: string;
      };
      if (i.kind === 'command' && i.command) {
        const a: Action = {
          id: `menu:${path}/${i.label ?? i.command}`,
          surface: 'menu',
          commands: [i.command],
        };
        if (i.params !== undefined) a.params = i.params;
        out.push(a);
      }
      if (i.items) walk(i.items, `${path}/${i.label ?? i.id ?? ''}`);
    }
  };
  for (const m of menu.menus as { label: string; items: unknown[] }[]) walk(m.items, m.label);
  return out;
}

/** The keymap's default shortcuts (`keymap-default.json`), every platform. */
function keymapActions(): Action[] {
  return KEYMAP.bindings.map((b) => {
    const a: Action = {
      id: `key:${b.keys}@${b.context ?? 'global'}${b.platforms ? `[${b.platforms.join(',')}]` : ''}`,
      surface: 'keymap',
      commands: [b.commandId],
    };
    if (b.params !== undefined) a.params = b.params;
    return a;
  });
}

/** The default main toolbar (`toolbar-default.json`; a group item's command from `itemRef`). */
function toolbarActions(): Action[] {
  const out: Action[] = [];
  for (const item of TOOLBAR) {
    const ref = itemRef(item);
    if (ref)
      out.push({ id: `toolbar:${item}`, surface: 'toolbar', commands: [ref.commandId], params: ref.params });
  }
  return out;
}

/** The commands the front end and the host use by id in their source (the queries a surface reads to show its state,
 *  and the commands they send outside a `data-tt-command`), with the files. */
function sourceUses(): { command: string; files: string[] }[] {
  const ids = new Map<string, Set<string>>();
  const roots = ['desktop/src', 'desktop/src-tauri/src'];
  const walk = (dir: string) => {
    for (const name of readdirSync(`${repo}${dir}`)) {
      const path = `${dir}/${name}`;
      if (statSync(`${repo}${path}`).isDirectory()) {
        if (!['generated', 'test', 'test-support', '__probes__', 'testdata'].includes(name)) walk(path);
        continue;
      }
      if (!/\.(ts|tsx|rs)$/.test(name) || /\.test\.tsx?$/.test(name)) continue;
      const text = readFileSync(`${repo}${path}`, 'utf8');
      for (const m of text.matchAll(/['"]([a-z][A-Za-z]*\.[A-Za-z][A-Za-z.]*[A-Za-z])['"]/g)) {
        const id = m[1] ?? '';
        if (!ids.has(id)) ids.set(id, new Set());
        ids.get(id)?.add(path);
      }
    }
  };
  for (const r of roots) walk(r);
  return [...ids.entries()]
    .map(([command, files]) => ({ command, files: [...files].sort() }))
    .sort((a, b) => a.command.localeCompare(b.command));
}

const byId = (a: { id: string; surface: string }, b: { id: string; surface: string }) =>
  a.surface.localeCompare(b.surface) || a.id.localeCompare(b.id);

describe('the parity manifest (parity note 9)', () => {
  it('PA1, PA4: every rendered action is declared, and the manifest is the generator output', async () => {
    const dom: Action[] = [];
    const guiOnly: { id: string; surface: string; reason: string }[] = [];
    const unbound: { surface: string; what: string }[] = [];
    for (const s of await surfaces()) {
      const c = collect(s.root, s.name);
      dom.push(...c.actions);
      guiOnly.push(...c.guiOnly);
      unbound.push(...c.unbound);
    }
    const dedupe = <T extends { id: string; surface: string }>(l: T[]) => {
      const seen = new Set<string>();
      return l.filter((x) => {
        const k = `${x.surface}\u0000${x.id}`;
        if (seen.has(k)) return false;
        seen.add(k);
        return true;
      });
    };
    const manifest = {
      $comment:
        'Generated by `pnpm --dir desktop parity:gen` (desktop/src/ui-bindings/parity.test.tsx); never edited by hand. Every GUI action of every surface with the commands it sends (parity note 9, ruling D-25).',
      version: 1,
      dom: dedupe(dom).sort(byId),
      guiOnly: dedupe(guiOnly).sort(byId),
      menu: menuActions(),
      keymap: keymapActions(),
      toolbar: toolbarActions(),
      gestures: GESTURES.rows,
      uses: sourceUses(),
    };
    const text = `${JSON.stringify(manifest, null, 2)}\n`;
    const path = `${here}parity.json`;
    if (process.env['TT_PARITY_WRITE'] === '1') writeFileSync(path, text);
    // PA1: no actionable element without a command or a listed GUI-only reason.
    expect(unbound).toEqual([]);
    expect(guiOnly.filter((g) => !(GUI_ONLY_REASONS as readonly string[]).includes(g.reason))).toEqual([]);
    // PA4: the committed manifest is exactly this.
    expect(readFileSync(path, 'utf8')).toBe(text);
  });
});
