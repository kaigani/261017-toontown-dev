// Found by the D-39 table (C4.3b round 3): the keymap's Enter (Play) took the Enter meant for a message box's default
// button, so keymap Import's question could not be answered from the keyboard. Keys inside a modal dialog are the
// dialog's, and Enter or Space on a focused control activates it; outside, the keymap decides as before (01 §8.4).
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

afterEach(cleanup);

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

describe('keys inside a modal question are the question’s', () => {
  it('Enter on the default button is left to the button; Enter elsewhere goes to the keymap', async () => {
    const h = createScriptedHost({
      'app.state.get': () => ok(OPEN),
      'keymap.list': () =>
        ok([{ commandId: 'playback.start', keys: 'Enter', context: 'global', isDefault: true }]),
      'project.saveAs': () => fail('project.exists', 'the file exists'),
    });
    h.picked = '/p/Other.animpkg';
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
    act(() => {
      h.menu('project.saveAs', {});
    });
    const box = await screen.findByRole('alertdialog');
    const replace = box.querySelector<HTMLButtonElement>('[data-tt-id="message.replace"]');
    expect(replace).not.toBeNull();
    // The real window focuses the default button (seen in the D-39 diagnostic); here it is focused by hand.
    replace?.focus();
    // On the focused button the key keeps its default (the button's activation).
    expect(fireEvent.keyDown(replace as HTMLButtonElement, { key: 'Enter', code: 'Enter' })).toBe(true);
    expect(fireEvent.keyDown(replace as HTMLButtonElement, { key: ' ', code: 'Space' })).toBe(true);
  });
});
