// Ruling D-49 items 2 and 3 (C43B-R3-02, C43B-R3-03): every application modal keeps the focus inside (Tab and
// Shift+Tab cycle), makes the background inert, takes Cancel on Escape wherever the focus is, and lets no key reach the
// keymap while it is open; every message box opens on a focused default button, a destructive question on Cancel.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

/** The element, or a failure naming what is absent. */
function must<T>(v: T | null | undefined, what = 'element'): T {
  if (v === null || v === undefined) throw new Error(`absent: ${what}`);
  return v;
}

const OPEN = {
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

async function preferences(): Promise<{ h: ScriptedHost; prefs: HTMLElement }> {
  const h = createScriptedHost({ 'app.state.get': () => ok(OPEN) });
  h.keys['Primary+S'] = {
    outcome: 'command',
    keys: 'Primary+S',
    commandId: 'project.save',
    params: {},
    enabled: true,
    dialog: false,
  };
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  act(() => {
    h.event('shell.preferences', { category: 'general' });
  });
  const prefs = await screen.findByRole('dialog', { name: 'Preferences' });
  return { h, prefs };
}

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), summary';

describe('application modals (D-49 item 3)', () => {
  it('Tab and Shift+Tab cycle inside the modal; the background is inert', async () => {
    const { prefs } = await preferences();
    const items = Array.from(prefs.querySelectorAll<HTMLElement>(FOCUSABLE));
    const first = must(items[0], 'first control');
    const last = must(items[items.length - 1], 'last control');
    last.focus();
    fireEvent.keyDown(last, { key: 'Tab' });
    expect(document.activeElement).toBe(first);
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);
    // The main toolbar is outside the modal: inert while it is open.
    const toolbar = must(document.querySelector('[data-tt-id="mainToolbar"]'), 'toolbar');
    expect(toolbar.closest('[inert]')).not.toBeNull();
  });

  it('Escape takes Cancel wherever the focus is, and the background is live again', async () => {
    const { prefs } = await preferences();
    // The focus left on the body (outside the modal): Escape still cancels it.
    (document.activeElement as HTMLElement | null)?.blur();
    fireEvent.keyDown(document.body, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    });
    expect(prefs.isConnected).toBe(false);
    expect(document.querySelector('[inert]')).toBeNull();
  });

  it('no key reaches the keymap while a modal is open, wherever the focus is', async () => {
    const { h } = await preferences();
    (document.activeElement as HTMLElement | null)?.blur();
    fireEvent.keyDown(document.body, { key: 's', code: 'KeyS', ctrlKey: true });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });
    expect(h.actions().some((a) => a.command === 'project.save')).toBe(false);
  });

  it('a message box over a form is live; closing it leaves the form modal', async () => {
    const { prefs } = await preferences();
    act(() => {
      must(prefs.querySelector<HTMLElement>('[data-tt-id="preferences.category.keyboard"]'), 'keyboard').click();
    });
    await waitFor(() => {
      expect(prefs.querySelector('[data-tt-id="keymap.resetAll"]')).not.toBeNull();
    });
    fireEvent.click(must(prefs.querySelector('[data-tt-id="keymap.resetAll"]'), 'reset all'));
    const box = await screen.findByRole('alertdialog');
    expect(box.closest('[inert]')).toBeNull();
    // D-49 item 2: Reset all is destructive and names no default: the box opens on Cancel.
    expect(box.querySelector('[data-tt-default="true"]')?.getAttribute('data-tt-id')).toBe('message.cancel');
    fireEvent.keyDown(document.body, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
    // The form is still open and still the modal: the background stays inert.
    expect(prefs.isConnected).toBe(true);
    expect(must(document.querySelector('[data-tt-id="mainToolbar"]'), 'toolbar').closest('[inert]')).not.toBeNull();
  });
});
