// The context menus (01 §5.3 line 373, §5.4 lines 387-388; 02 §4.4; M4 C4.4a, design note 1 §9): one mechanism for the
// dock tab's panel checklist and the views' menus; items with the main menu's availability and the command each sends;
// the keyboard; a secondary drag opens nothing.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

const STATE = {
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

async function app(setup?: (h: ScriptedHost) => void): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'app.state.get': () => ok(STATE),
    'view.list': () => ok([{ viewId: 'cameraView', type: 'camera' }]),
  });
  setup?.(h);
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const sent = (h: ScriptedHost, command: string) =>
  h
    .actions()
    .filter((a) => a.command === command)
    .map((a) => a.params);

describe('the dock tab checklist (01 §5.4)', () => {
  it('shows each panel with its check mark and sends the toggle as the kernel completes it', async () => {
    const h = await app((host) => {
      host.available = (ref) =>
        ref.commandId === 'window.panel.set'
          ? {
              enabled: true,
              checked: ref.params['panelId'] === 'colour',
              params: { ...ref.params, visible: ref.params['panelId'] !== 'colour' },
            }
          : { enabled: true };
    });
    const tab = await waitFor(() => el('tab.colour'));
    act(() => {
      fireEvent.contextMenu(tab, { clientX: 30, clientY: 30 });
    });
    const menu = await screen.findByRole('menu', { name: 'Panels' });
    await waitFor(() => {
      expect(el('panelChecklist.colour').getAttribute('aria-checked')).toBe('true');
    });
    expect(el('panelChecklist.history').getAttribute('aria-checked')).toBe('false');
    expect(el('panelChecklist.colour').getAttribute('role')).toBe('menuitemcheckbox');
    // The first item has the focus; the keyboard moves it and Enter chooses.
    expect(document.activeElement).toBe(menu.querySelector('[role="menuitemcheckbox"]'));
    act(() => {
      el('panelChecklist.colour').focus();
      fireEvent.keyDown(document.activeElement ?? menu, { key: 'Enter' });
    });
    await waitFor(() => {
      expect(sent(h, 'window.panel.set')).toEqual([{ panelId: 'colour', visible: false }]);
    });
    expect(screen.queryByRole('menu', { name: 'Panels' })).toBeNull();
  });

  it('Home, End and the arrows move the focus; Escape closes and gives the focus back', async () => {
    await app();
    const tab = await waitFor(() => el('tab.history'));
    expect(tab.tabIndex).toBe(0);
    act(() => {
      tab.focus();
      fireEvent.contextMenu(tab, { clientX: 30, clientY: 30 });
    });
    const menu = await screen.findByRole('menu', { name: 'Panels' });
    const items = [...menu.querySelectorAll('[role="menuitemcheckbox"]')];
    act(() => {
      fireEvent.keyDown(menu, { key: 'End' });
    });
    expect(document.activeElement).toBe(items[items.length - 1]);
    act(() => {
      fireEvent.keyDown(menu, { key: 'ArrowDown' });
    });
    expect(document.activeElement).toBe(items[0]);
    act(() => {
      fireEvent.keyDown(menu, { key: 'ArrowUp' });
    });
    expect(document.activeElement).toBe(items[items.length - 1]);
    act(() => {
      fireEvent.keyDown(menu, { key: 'Home' });
    });
    expect(document.activeElement).toBe(items[0]);
    act(() => {
      fireEvent.keyDown(menu, { key: 'Escape' });
    });
    expect(screen.queryByRole('menu', { name: 'Panels' })).toBeNull();
    expect(document.activeElement).toBe(tab);
  });
});

describe("the views' context menus (02 §4.4; D-40 answers 11, 12)", () => {
  it('the Camera view with a scene tool: the general edit items only, as the Edit menu sends them', async () => {
    const h = await app();
    const view = await waitFor(() => el('cameraView.canvas'));
    await waitFor(() => {
      expect(h.sent.some((x) => x.command === 'view.list')).toBe(true);
    });
    act(() => {
      fireEvent.contextMenu(view, { clientX: 50, clientY: 50 });
    });
    const menu = await screen.findByRole('menu', { name: 'Scene' });
    expect([...menu.querySelectorAll('[role="menuitem"]')].map((e) => e.textContent)).toEqual([
      'Cut',
      'Copy',
      'Paste',
      'Select All',
      'Deselect All',
    ]);
    await waitFor(() => {
      expect(el('cameraView.sceneMenu.copy').getAttribute('aria-disabled')).toBe('false');
    });
    act(() => {
      fireEvent.click(el('cameraView.sceneMenu.copy'));
    });
    await waitFor(() => {
      expect(sent(h, 'edit.copy')).toEqual([{ context: 'staging' }]);
    });
  });

  it("D-60 item 2: each family's edit items name its selection context, in either view", async () => {
    const h = await app((host) => {
      host.available = () => ({ enabled: true });
    });
    const view = await waitFor(() => el('cameraView.canvas'));
    // The scene family (a scene tool): the staging selection.
    act(() => {
      fireEvent.contextMenu(view, { clientX: 50, clientY: 50 });
    });
    await screen.findByRole('menu', { name: 'Scene' });
    for (const item of ['cut', 'copy', 'paste', 'selectAll', 'deselectAll']) {
      expect(JSON.parse(el(`cameraView.sceneMenu.${item}`).getAttribute('data-tt-params') ?? '{}')).toEqual({
        context: 'staging',
      });
    }
    act(() => {
      fireEvent.keyDown(screen.getByRole('menu', { name: 'Scene' }), { key: 'Escape' });
    });
    // The Drawing family (a drawing tool) in the Camera view: the drawing's selection, not the staging one.
    act(() => {
      h.event('tool.active.changed', { toolId: 'select' });
    });
    act(() => {
      fireEvent.contextMenu(view, { clientX: 50, clientY: 50 });
    });
    await screen.findByRole('menu', { name: 'Drawing' });
    for (const item of ['cut', 'copy', 'paste', 'selectAll', 'deselectAll']) {
      expect(JSON.parse(el(`cameraView.drawingMenu.${item}`).getAttribute('data-tt-params') ?? '{}')).toEqual(
        {
          context: 'drawing',
        },
      );
    }
    await waitFor(() => {
      expect(el('cameraView.drawingMenu.selectAll').getAttribute('aria-disabled')).toBe('false');
    });
    act(() => {
      fireEvent.click(el('cameraView.drawingMenu.selectAll'));
    });
    await waitFor(() => {
      expect(sent(h, 'edit.selectAll')).toEqual([{ context: 'drawing' }]);
    });
  });

  it('a drawing tool: the Drawing family, each item sending its main-menu command; Feather Edges has no item', async () => {
    const h = await app();
    const view = await waitFor(() => el('cameraView.canvas'));
    act(() => {
      h.event('tool.active.changed', { toolId: 'brush' });
    });
    act(() => {
      fireEvent.contextMenu(view, { clientX: 50, clientY: 50 });
    });
    const menu = await screen.findByRole('menu', { name: 'Drawing' });
    expect(screen.getByRole('group', { name: 'Arrange' })).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Transform' })).toBeTruthy();
    const labels = [...menu.querySelectorAll('[role="menuitem"]')].map((e) => e.textContent);
    expect(labels).toHaveLength(22);
    expect(labels.some((l) => l.includes('Feather'))).toBe(false);
    await waitFor(() => {
      expect(el('cameraView.drawingMenu.flip.horizontal').getAttribute('aria-disabled')).toBe('false');
    });
    act(() => {
      fireEvent.click(el('cameraView.drawingMenu.flip.horizontal'));
    });
    await waitFor(() => {
      expect(sent(h, 'draw.flip.apply')).toEqual([{ axis: 'horizontal' }]);
    });
  });

  it('an unavailable item is disabled with the reason and sends nothing', async () => {
    const h = await app((host) => {
      host.available = (ref) =>
        ref.commandId === 'draw.group.create'
          ? { enabled: false, reason: 'nothing is selected' }
          : { enabled: true };
    });
    const view = await waitFor(() => el('cameraView.canvas'));
    act(() => {
      h.event('tool.active.changed', { toolId: 'brush' });
      fireEvent.contextMenu(view, { clientX: 50, clientY: 50 });
    });
    await screen.findByRole('menu', { name: 'Drawing' });
    await waitFor(() => {
      expect(el('cameraView.drawingMenu.group').getAttribute('aria-disabled')).toBe('true');
    });
    expect(el('cameraView.drawingMenu.group').textContent).toContain('nothing is selected');
    act(() => {
      fireEvent.click(el('cameraView.drawingMenu.group'));
    });
    expect(sent(h, 'draw.group.create')).toEqual([]);
    expect(screen.getByRole('menu', { name: 'Drawing' })).toBeTruthy();
  });

  it('a secondary press is never a tool gesture, and a dragged one opens no menu', async () => {
    const h = await app();
    const view = await waitFor(() => el('cameraView.canvas'));
    const surface = view.querySelector('canvas');
    if (!surface) throw new Error('no canvas');
    act(() => {
      fireEvent.pointerDown(surface, { button: 2, buttons: 2, clientX: 50, clientY: 50, pointerId: 1 });
      fireEvent.pointerUp(surface, { button: 2, clientX: 90, clientY: 50, pointerId: 1 });
      fireEvent.contextMenu(view, { clientX: 90, clientY: 50 });
    });
    expect(screen.queryByRole('menu')).toBeNull();
    // Without a drag the release opens it.
    act(() => {
      fireEvent.pointerDown(surface, { button: 2, buttons: 2, clientX: 50, clientY: 50, pointerId: 1 });
      fireEvent.pointerUp(surface, { button: 2, clientX: 51, clientY: 50, pointerId: 1 });
      fireEvent.contextMenu(view, { clientX: 51, clientY: 50 });
    });
    expect(await screen.findByRole('menu', { name: 'Scene' })).toBeTruthy();
    expect(h.sent.some((s) => s.command.startsWith('gesture'))).toBe(false);
  });
});

// D-66 item 3: Shift+F10 opens the focused control's context menu, as the context-menu key does (the platform's
// convention for keyboard reach). The dispatcher's function-key guard had cancelled the browser's own event for it.
describe('Shift+F10 (D-66 item 3)', () => {
  it("opens the focused dock tab's checklist and the focused view's menu at the control, sending nothing", async () => {
    const h = await app();
    const tab = await waitFor(() => el('tab.colour'));
    tab.focus();
    const before = h.actions().length;
    let kept = true;
    act(() => {
      kept = fireEvent.keyDown(tab, { key: 'F10', code: 'F10', shiftKey: true });
    });
    // Consumed: the browser does not act on it a second time.
    expect(kept).toBe(false);
    const menu = await screen.findByRole('menu', { name: 'Panels' });
    expect(menu.contains(document.activeElement)).toBe(true);
    act(() => {
      fireEvent.keyDown(menu, { key: 'Escape' });
    });
    await waitFor(() => {
      expect(screen.queryByRole('menu')).toBeNull();
    });
    const view = await waitFor(() => el('cameraView.canvas'));
    act(() => {
      fireEvent.keyDown(view, { key: 'F10', code: 'F10', shiftKey: true });
    });
    expect(await screen.findByRole('menu', { name: 'Scene' })).toBeTruthy();
    expect(h.actions().slice(before)).toEqual([]);
  });
});
