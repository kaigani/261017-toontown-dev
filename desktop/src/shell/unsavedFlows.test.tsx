// Ruling D-44: the unsaved-changes question (01 §3.5) in front of every command that takes `onUnsaved` (New, Open,
// Open Recent, Open Backup, recovery open, Close, Quit), each with Save, Don't Save and Cancel: the commands and
// parameters each choice produces. Save saves first, then sends the command without `onUnsaved` (the project is clean);
// Don't Save sends it with `onUnsaved: discard`, carried through any dialog on the way (C43B-R2-07: Open Backup);
// Cancel sends nothing. The real-window rows are `scripts/c43-d44-flows.py`'s.
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

const DIRTY = {
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

/** Each command: how the person starts it, how the flow completes after the question, and what it then sends. */
const FLOWS: {
  name: string;
  command: string;
  params: Record<string, unknown>;
  complete?: () => Promise<void>;
  sends: Record<string, unknown>;
}[] = [
  {
    name: 'New',
    command: 'project.create',
    params: {},
    complete: async () => {
      const dialog = await screen.findByRole('dialog', { name: 'New Project' });
      const name = must(dialog.querySelector('[data-tt-id="newProject.name"]'));
      fireEvent.change(name, { target: { value: 'Fresh' } });
      await waitFor(() => {
        expect(dialog.querySelector<HTMLButtonElement>('[data-tt-id="newProject.create"]')?.disabled).toBe(
          false,
        );
      });
      fireEvent.click(must(dialog.querySelector('[data-tt-id="newProject.create"]')));
    },
    sends: { name: 'Fresh' },
  },
  { name: 'Open', command: 'project.open', params: {}, sends: { path: '/p/Other.animpkg' } },
  {
    name: 'Open Recent',
    command: 'project.open',
    params: { path: '/p/Recent.animpkg' },
    sends: { path: '/p/Recent.animpkg' },
  },
  {
    name: 'Open Backup',
    command: 'project.backup.open',
    params: {},
    complete: async () => {
      const dialog = await screen.findByRole('dialog', { name: 'Open Backup' });
      await waitFor(() => {
        expect(dialog.querySelector('[data-tt-id="openBackup.open.0"]')).not.toBeNull();
      });
      fireEvent.click(must(dialog.querySelector('[data-tt-id="openBackup.open.0"]')));
    },
    sends: { backupId: 'b1' },
  },
  {
    name: 'recovery open',
    command: 'project.recovery.open',
    params: { recoveryId: 'r1' },
    sends: { recoveryId: 'r1' },
  },
  { name: 'Close', command: 'project.close', params: {}, sends: {} },
  { name: 'Quit', command: 'app.quit', params: {}, sends: {} },
];

describe('the unsaved-changes question in front of every discarding command (D-44)', () => {
  for (const flow of FLOWS) {
    for (const choice of ['save', 'discard', 'cancel'] as const) {
      it(`${flow.name} › ${choice === 'save' ? 'Save' : choice === 'discard' ? "Don't Save" : 'Cancel'}`, async () => {
        const h = createScriptedHost({
          'app.state.get': () => ok(DIRTY),
          'project.backup.list': () =>
            ok([{ backupId: 'b1', savedAt: '2026-11-01T09:00:00Z', sizeBytes: 10, documentRev: 3 }]),
          // Answers without a job, so no progress window follows.
          'project.open': () => ok({}),
          'project.recovery.open': () => ok({}),
          'project.save': () => ok({}),
        });
        h.picked = '/p/Other.animpkg';
        render(
          <HostProvider api={createHostApi(h)}>
            <App />
          </HostProvider>,
        );
        await screen.findByRole('toolbar', { name: 'Main toolbar' });
        const before = h.actions().length;
        act(() => {
          h.menu(flow.command, flow.params);
        });
        const box = await screen.findByRole('alertdialog');
        expect(box.getAttribute('data-tt-id')).toBe('message.project.unsavedChanges');
        expect(Array.from(box.querySelectorAll('button')).map((b) => b.textContent)).toEqual(
          expect.arrayContaining(['Save', "Don't Save", 'Cancel']),
        );
        fireEvent.click(must(box.querySelector(`[data-tt-id="message.${choice}"]`)));
        if (choice !== 'cancel') await flow.complete?.();
        const relevant = () =>
          h
            .actions()
            .slice(before)
            .filter((a) => a.command === flow.command || a.command === 'project.save')
            // The command, the parameters the flow gives (other form values aside) and `onUnsaved` (absent: the
            // kernel's `fail`, the project being clean after a save).
            .map((a) => ({
              command: a.command,
              params: Object.fromEntries(Object.keys(flow.sends).map((k) => [k, a.params[k]])),
              onUnsaved: a.params['onUnsaved'],
            }));
        const expected =
          choice === 'cancel'
            ? []
            : [
                ...(choice === 'save' ? [{ command: 'project.save', params: {}, onUnsaved: undefined }] : []),
                {
                  command: flow.command,
                  params: flow.sends,
                  onUnsaved: choice === 'discard' ? 'discard' : undefined,
                },
              ];
        await waitFor(() => {
          expect(relevant()).toEqual(expected);
        });
        if (choice === 'cancel') {
          // Nothing follows a Cancel: no dialog of the command opens.
          await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
          });
          expect(relevant()).toEqual([]);
          expect(screen.queryByRole('dialog', { name: /New Project|Open Backup/ })).toBeNull();
        }
      });
    }
  }
});
