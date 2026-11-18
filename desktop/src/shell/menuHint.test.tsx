// Open Recent's full path (01 §3.3 line 190: "the full path is shown in the status bar or tooltip"; ruling D-48 item 8;
// reviews/M4-design-panels.md §13). The host sends a highlighted Open Recent entry's path as `menu-hint`; the status
// bar's left line shows it while it is set and shows its status line again when it clears. A hint sends no command.
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

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

function line(): string {
  return document.querySelector('[data-tt-id="status.line"]')?.textContent ?? '';
}

describe("Open Recent's full path on the status bar (01 §3.3)", () => {
  it('shows the highlighted entry path, then the status line again', async () => {
    const h = createScriptedHost({ 'app.state.get': () => ok(OPEN) });
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      window.dispatchEvent(new CustomEvent('tt-status-line', { detail: { text: 'Saved', kind: 'info' } }));
    });
    await waitFor(() => {
      expect(line()).toBe('Saved');
    });
    const before = h.actions().length;
    act(() => {
      h.emit('menu-hint', { text: 'C:\\Projects\\Walk.animpkg' });
    });
    await waitFor(() => {
      expect(line()).toBe('C:\\Projects\\Walk.animpkg');
    });
    act(() => {
      h.emit('menu-hint', { text: null });
    });
    await waitFor(() => {
      expect(line()).toBe('Saved');
    });
    // A highlight sends no command.
    expect(h.actions().length).toBe(before);
  });

  it('a malformed hint is dropped', async () => {
    const h = createScriptedHost({ 'app.state.get': () => ok(OPEN) });
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      h.emit('menu-hint', { text: 42 });
      h.emit('menu-hint', { path: '/p/x.animpkg' });
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });
    expect(line()).toBe('');
  });
});
