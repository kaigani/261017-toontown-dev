// Found by the D-39 table (C4.3b round 3): the keymap's Enter (Play) took the Enter meant for a message box's default
// button, so keymap Import's question could not be answered from the keyboard. Keys inside a modal dialog are the
// dialog's, and Enter or Space on a focused control activates it; outside, the keymap decides as before (01 §8.4).
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

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
    dirty: true,
    globalLibraryDirty: false,
    readOnly: false,
  },
  scene: { id: 'sc_1', name: 'Scene 1', frameCount: 24 },
  frame: 1,
  mode: 'staging',
  startScreenVisible: false,
};

describe('keys inside a modal question are the question’s', () => {
  it('Enter on the default button is left to the button; Enter elsewhere goes to the keymap', async () => {
    const h = createScriptedHost({
      'app.state.get': () => ok(OPEN),
      'keymap.list': () =>
        ok([{ commandId: 'playback.start', keys: 'Enter', context: 'global', isDefault: true }]),
    });
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    await waitFor(() => {
      expect(h.sent.some((s) => s.command === 'keymap.list')).toBe(true);
    });
    // Outside a dialog the bound key is the keymap's: its default is prevented.
    expect(fireEvent.keyDown(document.body, { key: 'Enter', code: 'Enter' })).toBe(false);
    // The unsaved-changes question (Close of a dirty project): its default button is Save (D-49 item 2).
    act(() => {
      h.menu('project.close', {});
    });
    const box = await screen.findByRole('alertdialog');
    const save = box.querySelector<HTMLButtonElement>('[data-tt-id="message.save"]');
    expect(save?.getAttribute('data-tt-default')).toBe('true');
    // The real window focuses the default button (seen in the D-39 diagnostic); here it is focused by hand.
    save?.focus();
    // On the focused button the key keeps its default (the button's activation); the keymap gets nothing.
    expect(fireEvent.keyDown(must(save), { key: 'Enter', code: 'Enter' })).toBe(true);
    expect(fireEvent.keyDown(must(save), { key: ' ', code: 'Space' })).toBe(true);
  });
});
