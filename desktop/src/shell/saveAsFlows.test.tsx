// Ruling D-44 (found by the flow table): Save As onto an existing file asks one question, Replace or Cancel, with the
// application's message box; the kernel's `project.exists` refusal is that question's cause, not an error box shown
// before it. Replace saves over the file (`onExists: overwrite`); Cancel sends nothing more.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

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

describe('Save As onto an existing file (D-44)', () => {
  for (const [choice, more] of [
    ['replace', [{ path: '/p/Other.animpkg', onExists: 'overwrite' }]],
    ['cancel', []],
  ] as const) {
    it(`asks once; ${choice}`, async () => {
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
      const box = await screen.findByRole('alertdialog');
      // The question, not an error: Replace and Cancel.
      expect(Array.from(box.querySelectorAll('button')).map((b) => b.textContent)).toEqual([
        'Replace',
        'Cancel',
      ]);
      fireEvent.click(must(box.querySelector(`[data-tt-id="message.${choice}"]`)));
      await waitFor(() => {
        expect(screen.queryByRole('alertdialog')).toBeNull();
      });
      await act(async () => {
        await new Promise((r) => setTimeout(r, 30));
      });
      // No second message box follows.
      expect(screen.queryByRole('alertdialog')).toBeNull();
      expect(
        h
          .actions()
          .filter((a) => a.command === 'project.saveAs')
          .map((a) => a.params),
      ).toEqual([{ path: '/p/Other.animpkg' }, ...more]);
    });
  }
});
