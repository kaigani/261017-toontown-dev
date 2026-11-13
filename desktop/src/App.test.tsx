// M4 C4.1 component tests of the shell (builder-owned). Each GUI action is checked against the exact command stream
// it produces — the stream an agent would send (gesture equivalence, 11 §2). No test relies on a real timer shorter
// than 1 s: waits are event-driven (`findBy*`, `vi.waitFor`) or use fake timers.
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { App } from './App';
import { createHostApi, HostProvider } from './ipc';
import {
  createScriptedHost,
  DEFAULT_LAYOUT,
  fail,
  NO_PROJECT_STATE,
  ok,
  type ScriptedHost,
} from './test-support/scriptedHost';

afterEach(() => {
  cleanup();
});

function renderApp(host: ScriptedHost = createScriptedHost()) {
  render(
    <HostProvider api={createHostApi(host)}>
      <App />
    </HostProvider>,
  );
  return host;
}

const PROJECT_STATE = {
  ...NO_PROJECT_STATE,
  project: {
    id: 'pr_1',
    name: 'Walk',
    path: '/p/Walk.animpkg',
    dirty: true,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

describe('start screen and new project (01 §2, §3.1)', () => {
  it('shows the start screen without a project and creates a project with the form values', async () => {
    const host = renderApp();
    const screenEl = await screen.findByRole('dialog', { name: 'Start screen' });
    const name = within(screenEl).getByDisplayValue('Untitled');
    fireEvent.change(name, { target: { value: 'Bounce' } });
    fireEvent.click(within(screenEl).getByRole('button', { name: 'Create' }));
    await vi.waitFor(() => {
      expect(host.actions().map((a) => a.command)).toContain('project.create');
    });
    const create = host.actions().find((a) => a.command === 'project.create');
    expect(create?.params).toEqual({
      name: 'Bounce',
      frameRate: 24,
      cameraWidth: 1920,
      cameraHeight: 1080,
      preset: 'default',
    });
  });

  it('validates the name and the sizes inline, Enter creates only when valid, hand edits switch to Custom', async () => {
    const host = renderApp();
    const screenEl = await screen.findByRole('dialog', { name: 'Start screen' });
    const name = within(screenEl).getByDisplayValue('Untitled');
    fireEvent.change(name, { target: { value: 'bad/name' } });
    expect(within(screenEl).getByRole('button', { name: 'Create' }).hasAttribute('disabled')).toBe(true);
    fireEvent.keyDown(name, { key: 'Enter' });
    fireEvent.change(name, { target: { value: 'Ok' } });
    const width = within(screenEl).getByDisplayValue('1920');
    fireEvent.change(width, { target: { value: '8' } });
    expect(within(screenEl).getByText('Camera size is 16 to 8192 pixels.')).toBeTruthy();
    fireEvent.change(width, { target: { value: '1000' } });
    expect(within(screenEl).getByRole<HTMLSelectElement>('combobox').value).toBe('custom');
    fireEvent.keyDown(width, { key: 'Enter' });
    await vi.waitFor(() => {
      expect(host.actions().filter((a) => a.command === 'project.create')).toHaveLength(1);
    });
    expect(host.actions().find((a) => a.command === 'project.create')?.params).toEqual({
      name: 'Ok',
      frameRate: 24,
      cameraWidth: 1000,
      cameraHeight: 1080,
    });
  });

  it('opens a recent project and closes on Escape with ui.startScreen.set', async () => {
    const host = renderApp();
    const screenEl = await screen.findByRole('dialog', { name: 'Start screen' });
    fireEvent.click(within(screenEl).getByRole('button', { name: 'Walk' }));
    await vi.waitFor(() => {
      expect(
        host.actions().some((a) => a.command === 'project.open' && a.params['path'] === '/p/Walk.animpkg'),
      ).toBe(true);
    });
    fireEvent.keyDown(screenEl, { key: 'Escape' });
    await vi.waitFor(() => {
      expect(
        host.actions().some((a) => a.command === 'ui.startScreen.set' && a.params['visible'] === false),
      ).toBe(true);
    });
  });
});

describe('menus and the unsaved-changes rule (01 §3.5)', () => {
  it('asks Save / Don’t Save / Cancel before closing a modified project and sends onUnsaved', async () => {
    const host = createScriptedHost({ 'app.state.get': () => ok(PROJECT_STATE) });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('project.close');
    });
    const box = await screen.findByRole('alertdialog');
    expect(within(box).getByTestId('message-id').textContent).toBe('project.unsavedChanges');
    fireEvent.click(within(box).getByRole('button', { name: "Don't Save" }));
    await vi.waitFor(() => {
      expect(host.actions().find((a) => a.command === 'project.close')?.params).toEqual({
        onUnsaved: 'discard',
      });
    });
  });

  it('Cancel aborts the operation: nothing is sent', async () => {
    const host = createScriptedHost({ 'app.state.get': () => ok(PROJECT_STATE) });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('app.quit');
    });
    fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Cancel' }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(host.actions().some((a) => a.command === 'app.quit')).toBe(false);
  });

  it('a menu command with fixed parameters is sent as is', async () => {
    const host = renderApp();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('window.panel.set', { panelId: 'history', visible: true });
    });
    await vi.waitFor(() => {
      expect(host.actions().find((a) => a.command === 'window.panel.set')?.params).toEqual({
        panelId: 'history',
        visible: true,
      });
    });
  });
});

describe('typed errors and cancellation (M3 merge note §8 item 1)', () => {
  it('shows a failure in a message box with its message id and details', async () => {
    const host = createScriptedHost({
      'workspace.select': () => fail('workspace.notFound', 'no workspace named Nope', { name: 'Nope' }),
    });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('workspace.select', { name: 'Nope' });
    });
    const box = await screen.findByRole('alertdialog');
    expect(within(box).getByTestId('message-id').textContent).toBe('workspace.notFound');
    expect(within(box).getByText('no workspace named Nope')).toBeTruthy();
    // The message was reported to the executor (ui.message.shown) and the dialog is a modal state while open. The
    // reports leave from effects after the render, so they are awaited (event-driven, no timer).
    await vi.waitFor(() => {
      expect(
        host.reports.some(
          (r) =>
            (r['messageShown'] as { messageId?: string } | undefined)?.messageId === 'workspace.notFound',
        ),
      ).toBe(true);
      expect(
        host.reports.some((r) => (r['modal'] as { kind?: string } | null | undefined)?.kind === 'dialog'),
      ).toBe(true);
    });
  });

  it('closing a message box sends exactly {"modal":null} with its generation (C41-R1-01)', async () => {
    const host = createScriptedHost({
      'workspace.select': () => fail('workspace.notFound', 'no workspace named Nope', { name: 'Nope' }),
    });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('workspace.select', { name: 'Nope' });
    });
    const box = await screen.findByRole('alertdialog');
    await vi.waitFor(() => {
      expect(
        host.rawReports.some(
          (r) => r.startsWith('{"modal":{"kind":"dialog","title":') && r.endsWith(',"generation":0}'),
        ),
      ).toBe(true);
    });
    const before = host.rawReports.length;
    fireEvent.click(within(box).getByRole('button', { name: 'OK' }));
    await vi.waitFor(() => {
      expect(host.rawReports.slice(before)).toContain('{"modal":null,"generation":0}');
    });
  });

  it('a cancelled request is shown as cancelled, never as a timeout or an error box', async () => {
    const host = createScriptedHost({
      'workspace.save': () =>
        fail('command.timeout', 'workspace.save was not run: its caller cancelled the request', {
          reason: 'cancelled',
        }),
    });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('workspace.save');
    });
    expect(await screen.findByText('workspace.save: cancelled')).toBeTruthy();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    // Actions are sent cancellable (a token), queries are not.
    expect(host.sent.find((s) => s.command === 'workspace.save')?.token).toMatch(/^r\d+$/);
    expect(host.sent.find((s) => s.command === 'app.state.get')?.token).toBeNull();
  });
});

describe('IPC freshness', () => {
  it('reports the rendered UI tree with the last engine event it reflects', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const host = renderApp();
      await screen.findByRole('toolbar', { name: 'Main toolbar' });
      act(() => {
        host.event('workspace.changed', { name: 'Default', modified: true });
      });
      await vi.waitFor(() => {
        const last = [...host.reports].reverse().find((r) => r['renderedSeq'] !== undefined);
        expect(last?.['renderedSeq']).toBe(1);
        const tree = last?.['uiTree'] as { role: string; children?: { id?: string }[] } | undefined;
        expect(tree?.role).toBe('window');
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it('a gap in the event numbers triggers a full resync', async () => {
    const host = renderApp();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.event('keymap.changed', { commandIds: ['edit.undo'] });
    });
    await vi.waitFor(() => {
      expect(host.sent.filter((s) => s.command === 'keymap.list').length).toBeGreaterThanOrEqual(2);
    });
    const before = host.sent.filter((s) => s.command === 'project.presets.list').length;
    act(() => {
      // Event 2 is lost: event 3 arrives after event 1, and the store resyncs every slice.
      host.skip();
      host.event('keymap.changed', {});
    });
    await vi.waitFor(() => {
      expect(host.sent.filter((s) => s.command === 'project.presets.list').length).toBeGreaterThan(before);
    });
  });
});

describe('shortcuts (01 §8.4)', () => {
  it('a bound key goes through the kernel dispatch and runs the command it resolves to', async () => {
    const host = createScriptedHost({ 'app.state.get': () => ok(PROJECT_STATE) });
    host.keys['Primary+S'] = {
      outcome: 'command',
      keys: 'Primary+S',
      commandId: 'project.save',
      params: {},
      enabled: true,
      dialog: false,
    };
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    await vi.waitFor(() => {
      expect(host.sent.some((s) => s.command === 'keymap.list')).toBe(true);
    });
    fireEvent.keyDown(window, { key: 's', code: 'KeyS', ctrlKey: true });
    await vi.waitFor(() => {
      expect(host.actions().some((a) => a.command === 'project.save')).toBe(true);
    });
  });

  it('a disabled command consumes its key silently; a text field keeps unmodified keys', async () => {
    const host = createScriptedHost({ 'app.state.get': () => ok(PROJECT_STATE) });
    host.keys['Primary+Z'] = {
      outcome: 'command',
      keys: 'Primary+Z',
      commandId: 'edit.undo',
      params: {},
      enabled: false,
      dialog: false,
    };
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    fireEvent.keyDown(window, { key: 'z', code: 'KeyZ', ctrlKey: true });
    await act(async () => {
      await Promise.resolve();
    });
    expect(host.actions().some((a) => a.command === 'edit.undo')).toBe(false);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});

describe('preferences (01 §7.1)', () => {
  it('a dialog opened by the engine takes the focus: Escape closes it and {"modal":null} is sent with its generation (C41-R1-01)', async () => {
    const host = renderApp();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.event('shell.preferences', { category: 'files' });
    });
    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    expect(dialog.contains(document.activeElement)).toBe(true);
    const before = host.rawReports.length;
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    await vi.waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    });
    await vi.waitFor(() => {
      expect(host.rawReports.slice(before)).toContain('{"modal":null,"generation":0}');
    });
    expect(host.actions().some((a) => a.command === 'prefs.set')).toBe(false);
  });

  it('OK applies the staged values with one prefs.set; Cancel sends nothing', async () => {
    const host = renderApp();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.event('shell.preferences', { category: 'files' });
    });
    let dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    fireEvent.change(within(dialog).getByDisplayValue('3'), { target: { value: '7' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    await vi.waitFor(() => {
      expect(host.actions().find((a) => a.command === 'prefs.set')?.params).toEqual({
        values: { 'files.backups.count': 7 },
      });
    });
    await vi.waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    });
    act(() => {
      host.event('shell.preferences', { category: 'files' });
    });
    dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    fireEvent.change(within(dialog).getByDisplayValue('3'), { target: { value: '9' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(host.actions().filter((a) => a.command === 'prefs.set')).toHaveLength(1);
  });

  it('an out-of-range value keeps the window open and focuses the field', async () => {
    const host = renderApp();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.event('shell.preferences', { category: 'files' });
    });
    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    const field = within(dialog).getByDisplayValue('3');
    fireEvent.change(field, { target: { value: '99' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByRole('dialog', { name: 'Preferences' })).toBeTruthy();
    expect(document.activeElement).toBe(field);
    expect(host.actions().some((a) => a.command === 'prefs.set')).toBe(false);
  });
});

describe('history panel (01 §10)', () => {
  it('a click on an entry is one edit.history.move to just after it', async () => {
    const host = createScriptedHost({
      'app.state.get': () => ok(PROJECT_STATE),
      'workspace.get': () =>
        ok({
          centralPanel: 'history',
          corners: 'sides',
          docks: { left: [], right: [], top: [], bottom: [] },
          floating: [],
          toolPalettes: [],
          mainToolbar: { visible: true, items: [] },
          fullScreen: {
            centralPanel: 'history',
            corners: 'sides',
            docks: { left: [], right: [], top: [], bottom: [] },
            floating: [],
            toolPalettes: [],
            mainToolbar: { visible: true, items: [] },
          },
        }),
      'edit.history.get': () =>
        ok({
          entries: [
            { index: 0, label: 'Add Stroke', source: 'user', time: '10:00' },
            { index: 1, label: 'Move Peg', source: 'agent', agentName: 'Helper', time: '10:01' },
          ],
          position: 2,
          cleanPosition: 0,
        }),
    });
    renderApp(host);
    const entry = await screen.findByText('Add Stroke');
    expect(screen.getByText('agent: Helper')).toBeTruthy();
    fireEvent.click(entry);
    await vi.waitFor(() => {
      expect(host.actions().find((a) => a.command === 'edit.history.move')?.params).toEqual({ position: 1 });
    });
  });
});

describe('M0 diagnostics (C0.4)', () => {
  it('runs the IPC benchmark the host asks for and sends the report', async () => {
    const host = createScriptedHost();
    const reports: unknown[] = [];
    const invoke = host.invoke.bind(host);
    host.invoke = (command, args) => {
      if (command === 'bench_plan') return Promise.resolve({ count: 1, payloadBytes: [16] });
      if (command === 'bench_report') {
        reports.push(args?.['report']);
        return Promise.resolve(null);
      }
      return invoke(command, args);
    };
    renderApp(host);
    await vi.waitFor(() => {
      expect(reports).toHaveLength(1);
    });
    expect(reports[0]).toMatchObject({ count: 1 });
  });
});

describe('one source of availability (ruling O-10)', () => {
  /** Every toolbar button and rail item against the kernel's facts the scripted host answers. */
  function compare(host: ScriptedHost) {
    const toolbar = screen.getByRole('toolbar', { name: 'Main toolbar' });
    const buttons = within(toolbar).getAllByRole('button');
    expect(buttons.length).toBe(6);
    for (const b of buttons) {
      const command = b.getAttribute('data-tt-command');
      if (command === null) {
        // A group item of a later checkpoint runs no command and is disabled.
        expect((b as HTMLButtonElement).disabled).toBe(true);
        continue;
      }
      const params = JSON.parse(b.getAttribute('data-tt-params') ?? '{}') as Record<string, unknown>;
      const facts = host.available({ commandId: command, params });
      expect({ command, disabled: (b as HTMLButtonElement).disabled }).toEqual({
        command,
        disabled: !facts.enabled,
      });
      if (facts.label !== undefined) expect(b.getAttribute('data-tt-label')).toBe(facts.label);
    }
    // The tool rail's palettes carry no command reference (their tools arrive with their owners).
    const rail = screen.queryByRole('navigation', { name: 'Tool palettes' });
    for (const item of rail ? Array.from(rail.querySelectorAll('[data-tt-role="toolPalette"]')) : []) {
      expect(item.getAttribute('data-tt-command')).toBeNull();
    }
  }

  it('the toolbar follows the kernel in the no-project and the project state', async () => {
    let project = false;
    const items = ['edit.undo', 'edit.redo', 'separator', 'firstFrame', 'nextFrame', 'gridType', 'playStop'];
    const layout = { ...DEFAULT_LAYOUT, mainToolbar: { visible: true, items: [...items, 'workspaceList'] } };
    const host = createScriptedHost({
      'app.state.get': () => ok(project ? PROJECT_STATE : NO_PROJECT_STATE),
      'workspace.get': () => ok({ ...layout, fullScreen: layout }),
    });
    host.available = (ref) => {
      if (ref.commandId === 'edit.undo')
        return project
          ? { enabled: true, label: 'Undo Move Peg' }
          : { enabled: false, reason: 'no project is open' };
      if (ref.commandId === 'edit.redo') return { enabled: false, reason: 'there is nothing to redo' };
      if (ref.commandId.startsWith('app.frame')) return { enabled: project };
      return { enabled: true };
    };
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Undo' })).toHaveProperty('disabled', true);
    });
    compare(host);
    project = true;
    act(() => {
      host.event('project.opened', { projectId: 'pr_1' });
    });
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Undo Move Peg' })).toHaveProperty('disabled', false);
    });
    compare(host);
  });
});

describe('engine restart is a generation change (ruling O-14)', () => {
  it('the confirmation names the last autosave; after the restart reports name the new generation', async () => {
    const host = renderApp(createScriptedHost({ 'app.state.get': () => ok(PROJECT_STATE) }));
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.event('project.autosaved', {
        projectId: 'pr_1',
        recoveryId: 'rc_1',
        savedAt: '2026-11-13T10:20:30Z',
      });
      host.emit('engine-status', { notResponding: true, generation: 0 });
    });
    const banner = await vi.waitFor(() => {
      const b = document.querySelector('[data-tt-id="engineNotResponding"]');
      if (!(b instanceof HTMLElement)) throw new Error('no banner');
      return b;
    });
    const shown = new Date('2026-11-13T10:20:30Z').toLocaleTimeString();
    await vi.waitFor(() => {
      expect(banner.textContent).toContain(shown);
    });
    fireEvent.click(within(banner).getByRole('button', { name: 'Restart the engine' }));
    const box = await screen.findByRole('alertdialog');
    expect(box.textContent).toContain(`last autosave (${shown})`);
    fireEvent.click(within(box).getByRole('button', { name: 'Restart' }));
    await vi.waitFor(() => {
      expect(host.restarts).toBe(1);
    });
    // The window now names generation 1 in its reports, and an event of generation 1 is applied from seq 1.
    const before = host.rawReports.length;
    act(() => {
      host.event('app.state.changed', { changed: ['modal'], state: PROJECT_STATE });
    });
    await vi.waitFor(() => {
      expect(host.rawReports.slice(before).some((r) => r.includes('"generation":1'))).toBe(true);
    });
    expect(host.rawReports.slice(before).every((r) => !r.includes('"generation":0'))).toBe(true);
  });

  it('a request cut off by a restart is shown as such, not as an error box', async () => {
    const host = createScriptedHost({
      'workspace.select': () =>
        fail('command.timeout', 'the engine was restarted while this request ran', {
          reason: 'engineRestarted',
          generation: 0,
          currentGeneration: 1,
        }),
    });
    renderApp(host);
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      host.menu('workspace.select', { name: 'Drawing' });
    });
    await screen.findByText(/the engine restarted while it ran/);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});
