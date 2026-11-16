// C4.3b round 1 findings on bindings declared and never exercised (ruling D-39): C43B-R1-07, File › Open Backup lists
// `project.backup.list`'s answer, which is the array of backups (01 §12.2 lines 1083-1084), and offers Open for each;
// C43B-R1-08, the main toolbar's Grid sends the next visibility the kernel completes, and two clicks alternate.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, DEFAULT_LAYOUT, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

afterEach(cleanup);

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

describe('Open Backup (C43B-R1-07)', () => {
  for (const [what, backups] of [
    [
      'two backups: each listed with its Open',
      [
        { backupId: 'b1', savedAt: '2026-11-01T09:00:00Z', sizeBytes: 10, documentRev: 3 },
        { backupId: 'b2', savedAt: '2026-11-01T10:00:00Z', sizeBytes: 11, documentRev: 4 },
      ],
    ],
    ['no backup: the empty-list notice and no Open', []],
  ] as const) {
    it(what, async () => {
      const h = createScriptedHost({
        'app.state.get': () => ok(PROJECT_STATE),
        'project.backup.list': () => ok(backups),
      });
      render(
        <HostProvider api={createHostApi(h)}>
          <App />
        </HostProvider>,
      );
      await screen.findByRole('toolbar', { name: 'Main toolbar' });
      act(() => {
        h.menu('project.backup.open', {});
      });
      const dialog = await screen.findByRole('dialog', { name: 'Open Backup' });
      await waitFor(() => {
        expect(dialog.querySelector('[data-tt-id="openBackup.list"]')).not.toBeNull();
      });
      const opens = dialog.querySelectorAll<HTMLButtonElement>('[data-tt-command="project.backup.open"]');
      expect(opens.length).toBe(backups.length);
      expect(dialog.textContent.includes('No backups of this project.')).toBe(backups.length === 0);
      if (backups.length > 0) {
        expect(dialog.textContent).toContain('2026-11-01T09:00:00Z');
        act(() => {
          opens[1]?.click();
        });
        await waitFor(() => {
          expect(
            h.actions().some((a) => a.command === 'project.backup.open' && a.params['backupId'] === 'b2'),
          ).toBe(true);
        });
      }
    });
  }
});

describe('the main toolbar Grid (C43B-R1-08)', () => {
  it('sends the next visibility and alternates', async () => {
    let visible = false;
    const layout = { ...DEFAULT_LAYOUT, mainToolbar: { visible: true, items: ['drawAid.grid.set'] } };
    const h = createScriptedHost({
      'app.state.get': () => ok(PROJECT_STATE),
      'workspace.get': () => ok({ ...layout, fullScreen: layout }),
      'drawAid.grid.set': (p) => {
        if (typeof p['visible'] === 'boolean') visible = p['visible'];
        return ok({ visible, type: 'normal', overlay: false });
      },
    });
    h.available = (ref) =>
      ref.commandId === 'drawAid.grid.set'
        ? { enabled: true, checked: visible, params: { visible: !visible } }
        : { enabled: true };
    render(
      <HostProvider api={createHostApi(h)}>
        <App />
      </HostProvider>,
    );
    const grid = await screen.findByRole('button', { name: 'Grid' });
    await waitFor(() => {
      expect(grid).toHaveProperty('disabled', false);
    });
    const sent = () =>
      h
        .actions()
        .filter((a) => a.command === 'drawAid.grid.set')
        .map((a) => a.params);
    fireEvent.click(grid);
    await waitFor(() => {
      expect(sent()).toEqual([{ visible: true }]);
    });
    act(() => {
      h.event('drawAid.changed', { aid: 'grid' });
    });
    await waitFor(() => {
      expect(grid.getAttribute('aria-pressed')).toBe('true');
    });
    fireEvent.click(grid);
    await waitFor(() => {
      expect(sent()).toEqual([{ visible: true }, { visible: false }]);
    });
  });
});
