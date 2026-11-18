// The parity gate, front-end half (M4 C4.3b; parity note 9, rulings D-25, D-30, D-31, D-35): PA1 (every actionable
// element of every rendered surface, conditional surfaces and states included, sends a command or is GUI-only for a
// listed reason) and PA4 (the manifest is the generator's output). `TT_PARITY_WRITE=1` (`pnpm parity:gen`) writes
// `parity.json` instead of comparing; the Rust half (`crates/executor/tests/common/parity_gate.rs`, run by
// `tests/c43/parity.rs`) checks PA2, PA3, the declared reads against the call sites found here, the window gestures and
// the kernel-derived sections.
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { COMMAND_INDEX } from '../ipc/generated/commands';
import {
  createScriptedHost,
  DEFAULT_LAYOUT,
  NO_PROJECT_STATE,
  ok,
  fail,
  type ScriptedHost,
} from '../test-support/scriptedHost';
import { CATEGORIES } from '../prefs/categories';
import { itemRef } from '../shell/toolbarItems';
import { callSitesOf, isProductFile, rustCallSites, tsCallSites } from '../test-support/callSites';
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
const WINDOW_GESTURES = readJson('desktop/src/ui-bindings/window-gestures.json') as {
  rows: { id: string; surface: string; commands: string[] }[];
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

const SESSIONS = {
  enabled: true,
  endpoint: 'http://127.0.0.1:47215/mcp',
  sessions: [{ sessionId: 's1', client: 'mcp', policy: 'assist' }],
};

/** A host whose answers show every conditional part of a surface: an agent connected, history entries, a recovery
 *  entry, keymap bindings that can clash. */
function host(project: boolean): ScriptedHost {
  return createScriptedHost({
    'app.state.get': () => ok(project ? PROJECT_STATE : NO_PROJECT_STATE),
    'workspace.get': () =>
      ok({ ...DEFAULT_LAYOUT, mainToolbar: { visible: true, items: TOOLBAR }, fullScreen: DEFAULT_LAYOUT }),
    'tool.list': () => ok({ tools: TOOLS.map((toolId) => ({ toolId })), activeToolId: 'brush' }),
    'view.list': () => ok([{ viewId: 'cameraView', type: 'camera' }]),
    'agent.server.status': () => ok(SESSIONS),
    'agent.activity.list': () => ok([{ seq: 1, commandId: 'app.state.get', status: 'ok' }]),
    'edit.history.get': () =>
      project
        ? ok({
            entries: [{ index: 1, stepId: 'u-1', label: 'Rectangle Tool', source: 'user' }],
            position: 1,
            cleanPosition: 0,
            total: 1,
          })
        : fail('project.none', 'no project is open'),
    'project.recovery.list': () =>
      ok([{ recoveryId: 'r1', name: 'Walk', path: '/p/Walk.animpkg', savedAt: '2026-11-01T10:00:00Z' }]),
    // Two backups: the Open Backup dialog shows its Open buttons (C43B-R1-07: the answer is the array itself).
    'project.backup.list': () =>
      ok([
        { backupId: 'b1', savedAt: '2026-11-01T09:00:00Z', sizeBytes: 1000, documentRev: 3 },
        { backupId: 'b2', savedAt: '2026-11-01T10:00:00Z', sizeBytes: 1100, documentRev: 4 },
      ]),
    // Bindings that can clash in the shortcut editor; its dry run answers.
    'keymap.list': () =>
      ok([
        { commandId: 'project.save', keys: 'Primary+S', context: 'global', isDefault: true },
        { commandId: 'edit.undo', keys: 'Primary+Z', context: 'global', isDefault: true },
      ]),
    'keymap.set': () => ok({}),
    // The Help dialog's text.
    'ui.help.get': () => ok({ title: 'Help', text: 'History panel' }),
    // A job that never ends keeps its progress window open.
    'project.open': () => ok({ jobId: 'j1' }),
    'job.wait': () => new Promise(() => undefined),
    // A refusal shows its message box.
    'project.close': () => fail('project.busy', 'the project is busy'),
    // The Pen panel (M4 C4.4a): two brush pens and an eraser, the second brush pen current.
    'tool.pen.list': () =>
      ok({
        pens: [
          {
            penId: 'pen_1',
            name: 'Pen 1',
            type: 'brush',
            tipShape: 'round',
            minSize: 1,
            maxSize: 4,
            smoothness: 3,
            tipAngle: 0,
          },
          {
            penId: 'pen_2',
            name: 'Pen 2',
            type: 'brush',
            tipShape: 'flat',
            minSize: 2,
            maxSize: 8,
            smoothness: 5,
            tipAngle: 30,
          },
          {
            penId: 'pen_3',
            name: 'Eraser 1',
            type: 'eraser',
            tipShape: 'round',
            minSize: 4,
            maxSize: 12,
            smoothness: 0,
            tipAngle: 0,
          },
        ],
        currentBrushPenId: 'pen_2',
        currentEraserPenId: 'pen_3',
      }),
    'tool.active.get': () => ok({ toolId: 'brush' }),
    // The main toolbar's camera list (M4 C4.4a): two cameras, the first current.
    'anim.camera.list': () =>
      ok({
        cameras: [
          { id: 'cam_1', name: 'Camera', current: true },
          { id: 'cam_2', name: 'Wide', current: false },
        ],
      }),
    'anim.camera.current.get': () => ok({ cameraId: 'cam_1' }),
    // The toolbar customisation sheet (M4 C4.4a): the main toolbar's items and the items it allows.
    'window.toolbar.list': () =>
      ok([
        {
          toolbarId: 'main',
          visible: true,
          items: ['project.save', 'separator', 'edit.undo'],
          allowedItems: ['project.save', 'edit.undo', 'edit.redo', 'separator', 'space'],
        },
      ]),
    // The Properties panel's Tool options page (M4 C4.4a): each tool's options, the drawing settings, gap closing.
    'tool.options.get': (p) => ok({ toolId: p['toolId'], options: TOOL_OPTIONS[String(p['toolId'])] ?? {} }),
    'tool.settings.get': () =>
      ok({ placeBehind: false, drawTopLayer: false, snapToContour: false, symmetry: { mode: 'none' } }),
    'colour.gapClosing.get': () => ok({ mode: 'off', tolerancesPx: { small: 3, medium: 8, large: 16 } }),
  });
}

/** The tools' options as the kernel's defaults give them (`drawing.rs` `default_options`). */
const TOOL_OPTIONS: Record<string, Record<string, unknown>> = {
  select: { rectRule: 'inside' },
  pencil: { thicknessSource: 'penMax', thickness: null },
  polyline: { snap: true, snapRadius: 8, thicknessSource: 'penMax', thickness: null },
  // An outline set, so the Text panel offers to remove it.
  text: {
    font: null,
    size: 36,
    bold: false,
    italic: false,
    baseline: 'normal',
    kerning: 0,
    autoKern: true,
    align: 'left',
    leftMargin: 0,
    rightMargin: 0,
    indent: 0,
    lineSpacing: 0,
    outlineColourId: 'cl_1',
  },
};

function mount(h: ScriptedHost, floatPanel?: string) {
  render(
    <HostProvider api={createHostApi(h)}>
      <App {...(floatPanel ? { floatPanel } : {})} />
    </HostProvider>,
  );
}

interface Surface {
  name: string;
  root: Element;
  /** The queries sent from the moment the surface was opened until it settled (C43B-R3S-01, ruling D-48). */
  reads: string[];
}

/** C43B-R3S-01 (ruling D-48): each query is attributed to the rendered surface that sends it. `mark` is called just
 *  before the action that opens a surface; the queries the host receives from then until the surface has settled are
 *  that surface's reads, a `useQuery` hook's included. */
let reading: { host: ScriptedHost; since: number } | null = null;
function mark(host: ScriptedHost): void {
  reading = { host, since: host.sent.length };
}
const isQueryCommand = (id: string) =>
  (COMMAND_INDEX as Record<string, { kind?: string } | undefined>)[id]?.kind === 'query';

const snapshot = (name: string, el: Element): Surface => {
  const sent = reading ? reading.host.sent.slice(reading.since) : [];
  const reads = [
    ...new Set(sent.filter((x) => !x.dryRun && isQueryCommand(x.command)).map((x) => x.command)),
  ].sort();
  return { name, root: el.cloneNode(true) as Element, reads };
};

/** The surface as rendered once the answers it asked for have arrived (a slow runner otherwise snapshots a loading
 *  state): pending promises and timers run first. */
async function settled(name: string, el: Element): Promise<Surface> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  return snapshot(name, el);
}

/** Dialogs whose actions depend on a query's answer: what to wait for before collecting them. */
const LOADED_ACTIONS: Readonly<Record<string, { selector: string; count: number }>> = {
  'Open Backup': { selector: '[data-tt-command="project.backup.open"]', count: 2 },
};

/** Every surface and state the GUI can show (D-35), each with what opens it. */
async function surfaces(): Promise<Surface[]> {
  const out: Surface[] = [];
  // The start screen (no project), with a recovery entry.
  const start = host(false);
  mark(start);
  mount(start);
  out.push(await settled('startScreen', await screen.findByRole('dialog', { name: 'Start screen' })));
  cleanup();

  // The main window with a project: toolbars, tool palettes, the dock's panels (History with an entry) and view
  // chrome, the status bar with an agent connected.
  const h = host(true);
  mark(h);
  mount(h);
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  await screen.findByText('brush');
  await screen.findByText(/Agent: mcp/);
  out.push(await settled('main', document.body));

  // The agent popover open.
  mark(h);
  act(() => {
    fireEvent.click(screen.getByText(/Agent: mcp/));
  });
  out.push(await settled('agentPopover', await screen.findByRole('dialog', { name: 'Agent sessions' })));
  act(() => {
    fireEvent.click(screen.getByText(/Agent: mcp/));
  });

  // The progress window (C43B-R2-06): an agent session's running job opens it, with its Cancel.
  h.handlers['job.list'] = () =>
    ok([
      {
        jobId: 'job-7',
        commandId: 'render.range.run',
        state: 'running',
        progress: 0.25,
        message: 'Frame 3 of 12',
        session: 'agent:s1',
        cancellable: true,
      },
    ]);
  mark(h);
  act(() => {
    h.event('job.progress', { jobId: 'job-7' });
  });
  out.push(await settled('jobsWindow', await screen.findByRole('dialog', { name: 'Jobs' })));
  h.handlers['job.list'] = () => ok([]);
  act(() => {
    h.event('job.finished', { jobId: 'job-7' });
  });
  await waitFor(() => {
    expect(screen.queryByRole('dialog', { name: 'Jobs' })).toBeNull();
  });

  // The dock tab's context menu.
  const tab = document.querySelector('[data-tt-id^="tab."]');
  if (!tab) throw new Error('no dock tab rendered');
  mark(h);
  act(() => {
    fireEvent.contextMenu(tab);
  });
  const checklist = await screen.findByRole('menu');
  out.push(await settled('dockTabContextMenu', checklist));
  act(() => {
    fireEvent.keyDown(checklist, { key: 'Escape' });
  });

  // The views' context menus (M4 C4.4a, design note 1 §9): the Camera view's scene family while a scene tool runs (a new
  // session runs the scene tools, SI-148), then the Drawing family once a drawing tool is chosen (02 §4.4 line 293).
  const view = document.querySelector('[data-tt-id="cameraView.canvas"]');
  if (!view) throw new Error('no Camera view canvas rendered');
  for (const family of ['scene', 'drawing'] as const) {
    if (family === 'drawing') {
      act(() => {
        h.event('tool.active.changed', { toolId: 'brush' });
      });
    }
    mark(h);
    act(() => {
      fireEvent.contextMenu(view, { clientX: 40, clientY: 40 });
    });
    const menu = await waitFor(() => {
      const m = document.querySelector(`[data-tt-id="cameraView.${family}Menu"]`);
      if (!m) throw new Error(`no ${family} menu`);
      return m;
    });
    out.push(await settled(`viewContextMenu:${family}`, menu));
    act(() => {
      fireEvent.keyDown(menu, { key: 'Escape' });
    });
  }

  // Each dialog the menus and events open.
  const dialogs: [string, string][] = [
    ['About Toontown', 'app.info.get'],
    ['Project Info', 'project.info.get'],
    ['Check Project', 'doc.validate'],
    ['Open Backup', 'project.backup.open'],
    ['New Project', 'project.create'],
    ['Project Properties', 'project.properties.set'],
    ['New Workspace', 'workspace.create'],
    ['Rename Workspace', 'workspace.rename'],
    ['Customize Toolbar', 'window.toolbar.items.set'],
  ];
  for (const [title, command] of dialogs) {
    mark(h);
    act(() => {
      h.menu(command, {});
    });
    const d = await screen.findByRole('dialog', { name: title });
    // A dialog whose actions arrive with its query's answer is collected once they are there (Open Backup's Open
    // buttons follow `project.backup.list`; a slow runner snapshotted the loading state).
    const loaded = LOADED_ACTIONS[title];
    if (loaded) {
      await waitFor(() => {
        expect(d.querySelectorAll(loaded.selector).length).toBe(loaded.count);
      });
    }
    out.push(await settled(`dialog:${title}`, d));
    act(() => {
      d.querySelector<HTMLElement>('[data-tt-id$=".cancel"]')?.click();
    });
  }
  // Help.
  mark(h);
  act(() => {
    h.event('shell.help', {});
  });
  const help = await screen.findByRole('dialog', { name: /Help|Documentation/ });
  out.push(await settled('dialog:Help', help));
  act(() => {
    help.querySelector<HTMLElement>('[data-tt-id$=".cancel"]')?.click();
  });
  // A job's progress window (Cancel).
  mark(h);
  act(() => {
    h.menu('project.open', { path: '/p/Walk.animpkg' });
  });
  const progress = await screen.findByRole('dialog', { name: 'Opening project' });
  out.push(await settled('dialog:Opening project', progress));
  // A message box (a refused command).
  mark(h);
  act(() => {
    h.menu('project.close', {});
  });
  const message = await screen.findByRole('alertdialog');
  out.push(await settled('messageBox', message));

  // Preferences: every category, and the shortcut editor with a command selected and with a conflict. The dialog's
  // opening reads are its first category's.
  mark(h);
  act(() => {
    h.event('shell.preferences', { category: 'general' });
  });
  const prefs = await screen.findByRole('dialog', { name: 'Preferences' });
  for (const [k, c] of CATEGORIES.entries()) {
    if (k > 0) mark(h);
    act(() => {
      prefs.querySelector<HTMLElement>(`[data-tt-id="preferences.category.${c.id}"]`)?.click();
    });
    out.push(await settled(`preferences:${c.id}`, prefs));
  }
  act(() => {
    prefs.querySelector<HTMLElement>('[data-tt-id="preferences.category.keyboard"]')?.click();
  });
  const undo = await waitFor(() => {
    const el = prefs.querySelector<HTMLElement>('[data-tt-id="keymap.command.edit.undo"]');
    if (!el) throw new Error('no edit.undo in the shortcut editor');
    return el;
  });
  mark(h);
  act(() => {
    undo.click();
  });
  out.push(await settled('preferences:keyboard:details', prefs));
  const capture = prefs.querySelector<HTMLElement>('[data-tt-id="keymap.capture"]');
  if (!capture) throw new Error('no capture control');
  mark(h);
  act(() => {
    capture.click();
  });
  await act(async () => {
    fireEvent.keyDown(capture, { key: 's', code: 'KeyS', ctrlKey: true });
    await Promise.resolve();
  });
  await screen.findByText(/is bound to/);
  out.push(await settled('preferences:keyboard:conflict', prefs));
  cleanup();

  // A floating window.
  const floating = host(true);
  mark(floating);
  mount(floating, 'history');
  await screen.findByText('Rectangle Tool');
  out.push(await settled('float:history', document.body));
  cleanup();

  // The Pen panel in a floating window (M4 C4.4a): the brush pens, the current one's settings, the advanced options.
  const pen = host(true);
  mark(pen);
  mount(pen, 'pen');
  await screen.findByText('Pen 2');
  out.push(await settled('float:pen', document.body));
  // Its in-place rename (a double click on a pen).
  mark(pen);
  act(() => {
    fireEvent.doubleClick(screen.getByText('Pen 2'));
  });
  await screen.findByRole('textbox', { name: 'Rename Pen 2' });
  out.push(await settled('float:pen:rename', document.body));
  cleanup();

  // The Properties panel in a floating window, its Tool options page for each kind of tool (M4 C4.4a).
  for (const tool of ['brush', 'eraser', 'select', 'pencil', 'polyline', 'paint']) {
    const props = host(true);
    props.handlers['tool.active.get'] = () => ok({ toolId: tool });
    mark(props);
    mount(props, 'properties');
    await screen.findByText('Draw on Top Layer');
    if (tool === 'brush') await screen.findByText('Pen 2');
    out.push(await settled(`float:properties:${tool}`, document.body));
    cleanup();
  }

  // The Text panel as the defaults for new text, in a floating window (M4 C4.4a).
  const text = host(true);
  mark(text);
  mount(text, 'text');
  await screen.findByText('Defaults for new text');
  out.push(await settled('float:text', document.body));
  cleanup();

  // The Properties panel's Drawing page, the drawing inspector (M4 C4.4a): the current element's drawing at frame 1.
  const inspector = host(true);
  inspector.handlers['app.state.get'] = () =>
    ok({ ...PROJECT_STATE, element: { id: 'el_1', name: 'Bob', type: 'drawing' } });
  inspector.handlers['element.drawing.list'] = () =>
    ok({
      drawings: [
        { id: 'dr_1', name: '1', exposures: [{ from: 1, to: 12 }] },
        { id: 'dr_2', name: '2', exposures: [{ from: 13, to: 24 }] },
      ],
    });
  inspector.handlers['draw.drawing.render'] = () =>
    ok({ png: 'iVBORw0KGgo=', width: 160, height: 90, drawingToPixel: [1, 0, 0, 1, 0, 0] });
  inspector.handlers['prefs.get'] = () => ok({ values: { 'drawing.inspectorPreview': true } });
  mount(inspector, 'properties');
  await screen.findByText('Draw on Top Layer');
  mark(inspector);
  act(() => {
    fireEvent.click(screen.getByRole('tab', { name: 'Drawing' }));
  });
  await screen.findByText('Show preview');
  out.push(await settled('float:properties:drawing', document.body));
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

function productFiles(): string[] {
  const out: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(`${repo}${dir}`)) {
      const path = `${dir}/${name}`;
      if (statSync(`${repo}${path}`).isDirectory()) {
        walk(path);
        continue;
      }
      if (isProductFile(path)) out.push(path);
    }
  };
  for (const r of ['desktop/src', 'desktop/src-tauri/src']) walk(r);
  return out.sort();
}

/** Every command id quoted in product code, with its files: mentions, which bind nothing (D-35). */
function sourceUses(): { command: string; files: string[] }[] {
  const ids = new Map<string, Set<string>>();
  for (const path of productFiles()) {
    const text = readFileSync(`${repo}${path}`, 'utf8');
    for (const m of text.matchAll(/['"]([a-z][A-Za-z]*\.[A-Za-z][A-Za-z.]*[A-Za-z])['"]/g)) {
      const id = m[1] ?? '';
      if (!ids.has(id)) ids.set(id, new Set());
      ids.get(id)?.add(path);
    }
  }
  return [...ids.entries()]
    .map(([command, files]) => ({ command, files: [...files].sort() }))
    .sort((a, b) => a.command.localeCompare(b.command));
}

/** The calls of commands in product code (a test module of a Rust file, after `#[cfg(test)]`, is not product code):
 *  the gate checks the declared reads (`reads.json`) and the window gestures against them. */
function callSites(): { command: string; file: string }[] {
  const out: { command: string; file: string }[] = [];
  for (const path of productFiles()) {
    for (const command of callSitesOf(path, readFileSync(`${repo}${path}`, 'utf8')))
      out.push({ command, file: path });
  }
  return out.sort((a, b) => a.command.localeCompare(b.command) || a.file.localeCompare(b.file));
}

const byId = (a: { id: string; surface: string }, b: { id: string; surface: string }) =>
  a.surface.localeCompare(b.surface) || a.id.localeCompare(b.id);

describe('the parity manifest (parity note 9)', () => {
  it('PA1, PA4: every rendered action is declared, and the manifest is the generator output', async () => {
    const dom: Action[] = [];
    const guiOnly: { id: string; surface: string; reason: string }[] = [];
    const unbound: { surface: string; what: string }[] = [];
    const rendered = await surfaces();
    for (const s of rendered) {
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
        'Generated by `pnpm --dir desktop parity:gen` (desktop/src/ui-bindings/parity.test.tsx); never edited by hand. Every GUI action of every surface with the commands it sends (parity note 9, rulings D-25, D-35); `callSites` are the product code calls the gate checks the declared reads and window gestures against; `uses` are mentions, which bind nothing.',
      version: 2,
      surfaces: rendered.map((s) => s.name),
      dom: dedupe(dom).sort(byId),
      guiOnly: dedupe(guiOnly).sort(byId),
      menu: menuActions(),
      keymap: keymapActions(),
      toolbar: toolbarActions(),
      gestures: GESTURES.rows,
      windowGestures: WINDOW_GESTURES.rows.map((r) => ({
        id: r.id,
        surface: r.surface,
        commands: r.commands,
      })),
      callSites: callSites(),
      // C43B-R3S-01 (ruling D-48): the queries each rendered surface sent while it opened and settled.
      surfaceReads: [
        ...new Map(
          rendered.flatMap((s) => s.reads.map((q) => [`${s.name}\u0000${q}`, { surface: s.name, query: q }])),
        ).values(),
      ].sort((a, b) => a.surface.localeCompare(b.surface) || a.query.localeCompare(b.query)),
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

  it('D-35: each surface is collected (an undeclared element added to it fails PA1)', async () => {
    for (const s of await surfaces()) {
      const root = s.root.cloneNode(true) as Element;
      const stray = document.createElement('button');
      stray.setAttribute('data-tt-id', 'fixture.stray');
      root.appendChild(stray);
      const c = collect(root, s.name);
      expect(
        c.unbound.some((u) => u.what.startsWith('fixture.stray')),
        `${s.name}: an undeclared button is not reported`,
      ).toBe(true);
      const own = collect(s.root, s.name);
      expect(own.actions.length + own.guiOnly.length, `${s.name}: nothing collected`).toBeGreaterThan(0);
    }
  });

  it('D-35: a mention in the source is not a call site', () => {
    const text = "const label = 'keymap.export'; // see agent.server.set\nawait engine.run('job.list', {});";
    const found = tsCallSites(text, 'desktop/src/x.ts');
    expect(found).toEqual(['job.list']);
  });

  // C43B-R2S-01: call sites are executable call expressions, found with a parser. Each fixture holds one real call and
  // the same call where it does not execute; only the real one counts.
  it('C43B-R2S-01: a call in a comment, a string or a test helper is not a call site', () => {
    const comment = [
      "// engine.run('tool.settings.get', {})",
      "/* services.perform('edit.undo', {}) */",
      "/** useQuery(services, 'prefs.get', {}) */",
      "engine.run('job.list', {});",
    ].join('\n');
    expect(tsCallSites(comment, 'desktop/src/x.ts')).toEqual(['job.list']);
    const strings = [
      'const text = "engine.run(\'tool.settings.get\', {})";',
      "const tpl = `actions.perform('edit.undo', {})`;",
      "engine.run('job.list', {});",
    ].join('\n');
    expect(tsCallSites(strings, 'desktop/src/x.tsx')).toEqual(['job.list']);
    // A test helper and a test are not product code, whatever they call.
    const helper = "export const seed = () => engine.run('tool.settings.get', {});";
    expect(callSitesOf('desktop/src/test-support/helper.ts', helper)).toEqual([]);
    expect(callSitesOf('desktop/src/panels/x.test.tsx', helper)).toEqual([]);
    expect(callSitesOf('desktop/src/panels/x.ts', helper)).toEqual(['tool.settings.get']);
    // The store's slice table and `useQuery`'s second argument are read by the parser too.
    expect(
      tsCallSites(
        "export const SLICES = { a: ['app.state.get', {}] } as const;",
        'desktop/src/state/store.ts',
      ),
    ).toEqual(['app.state.get']);
    expect(tsCallSites("const r = useQuery(services, 'doc.validate', {});", 'desktop/src/x.tsx')).toEqual([
      'doc.validate',
    ]);
    // The host's Rust: comments, strings, raw strings and the test module do not count.
    const rust = [
      '// host.run("tool.settings.get", json!({}));',
      '/* q("prefs.get") /* nested */ still a comment */',
      'let s = "self.run(\\"edit.undo\\")";',
      'let r = r#"q("app.state.get")"#;',
      'host.run("job.list", json!({}));',
      'let a = Invocation::user("view.list", json!({}));',
      'Request { command: "window.panel.list" }',
      '#[cfg(test)]',
      'mod tests { fn t() { host.run("keymap.list", json!({})); } }',
    ].join('\n');
    expect(rustCallSites(rust)).toEqual(['job.list', 'view.list', 'window.panel.list']);
    expect(callSitesOf('desktop/src-tauri/src/x.rs', rust)).toEqual([
      'job.list',
      'view.list',
      'window.panel.list',
    ]);
    // Ruling D-48 item 7: a `#[cfg(test)]` on anything but the trailing test module fails closed (the file is refused,
    // not read only up to it): a test-only function followed by product code, and a test module with code after it.
    const early = [
      '#[cfg(test)]',
      'fn helper() {}',
      'fn product() { host.run("job.list", json!({})); }',
    ].join('\n');
    expect(() => rustCallSites(early)).toThrow(/trailing test module/);
    const notLast = [
      '#[cfg(test)]',
      'mod tests { fn t() {} }',
      'fn product() { host.run("job.list", json!({})); }',
    ].join('\n');
    expect(() => rustCallSites(notLast)).toThrow(/trailing test module/);
  });

  // C43B-R2S-01, C43B-R2-01: every surface key of `reads.json` is a surface the collector renders (or the native menu,
  // which the manifest collects from `menu.json`): an invented surface, or a real one renamed, is refused.
  it('C43B-R2-01: the surfaces reads.json names are collected surfaces', async () => {
    const reads = readJson('desktop/src/ui-bindings/reads.json') as { surfaces: Record<string, unknown> };
    const known = new Set([...(await surfaces()).map((x) => x.name), 'menu']);
    const unknown = (table: typeof reads) => Object.keys(table.surfaces).filter((name) => !known.has(name));
    expect(unknown(reads)).toEqual([]);
    const { 'dialog:Help': help, ...rest } = reads.surfaces;
    expect(unknown({ surfaces: { ...reads.surfaces, 'dialog:Nowhere': {} } })).toEqual(['dialog:Nowhere']);
    expect(unknown({ surfaces: { ...rest, helpWindow: help } })).toEqual(['helpWindow']);
  });
});
