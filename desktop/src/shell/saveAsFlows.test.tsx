// Ruling D-49 item 1 (C43B-R3-01): one replacement decision per Save As. The native save picker has already asked
// whether to replace an existing file (Windows and macOS), so the window saves over it with `onExists: overwrite` and
// shows no question of its own; a cancelled picker sends nothing.
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
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

describe('Save As onto an existing file (D-49 item 1)', () => {
  it('the native picker decided: the window saves over the file with no question of its own', async () => {
    const h = createScriptedHost({
      'app.state.get': () => ok(OPEN),
      'project.saveAs': (p) =>
        p['onExists'] === 'overwrite' ? ok({}) : fail('project.exists', 'the file exists'),
    });
    h.picked = '/p/Other.animpkg';
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      h.menu('project.saveAs', {});
    });
    await waitFor(() => {
      expect(h.actions().filter((a) => a.command === 'project.saveAs').length).toBe(1);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });
    // One decision, the picker's: no message box of the window's.
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(
      h
        .actions()
        .filter((a) => a.command === 'project.saveAs')
        .map((a) => a.params),
    ).toEqual([{ path: '/p/Other.animpkg', onExists: 'overwrite' }]);
  });

  it('a cancelled picker sends nothing', async () => {
    const h = createScriptedHost({ 'app.state.get': () => ok(OPEN) });
    h.picked = null;
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    act(() => {
      h.menu('project.saveAs', {});
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.actions().filter((a) => a.command === 'project.saveAs')).toEqual([]);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});
