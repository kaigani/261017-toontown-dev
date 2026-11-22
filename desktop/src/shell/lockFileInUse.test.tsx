// Ruling D-86 item 1 (finding C44B-R3S-01): a project open refused because a program outside the lock protocol keeps a
// lock file open (`fs.ioError`, `reason: "lockFileInUse"`, the file's path) tells the person which file is in use and
// offers to try again; Try Again sends the open again, Cancel sends nothing more; no generic error box is shown.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, ok } from '../test-support/scriptedHost';

afterEach(cleanup);

const LOCK = '/p/Locked.animpkg.lock.takeover';

describe('a lock file in use by another program (D-86 item 1)', () => {
  for (const choice of ['retry', 'cancel'] as const) {
    it(`Open › ${choice === 'retry' ? 'Try Again' : 'Cancel'}`, async () => {
      let calls = 0;
      const h = createScriptedHost({
        // Refused once, then opened (the other program let go), answering without a job.
        'project.open': () => {
          calls += 1;
          return calls === 1
            ? fail('fs.ioError', 'the lock file is in use', { reason: 'lockFileInUse', path: LOCK })
            : ok({});
        },
      });
      render(
        <HostProvider api={createHostApi(h)}>
          <App />
        </HostProvider>,
      );
      await screen.findByRole('toolbar', { name: 'Main toolbar' });
      act(() => {
        h.menu('project.open', { path: '/p/Locked.animpkg' });
      });
      const box = await screen.findByRole('alertdialog');
      expect(box.getAttribute('data-tt-id')).toBe('message.project.lockFileInUse');
      expect(box.textContent).toContain(LOCK);
      expect(screen.getAllByRole('alertdialog')).toHaveLength(1);
      const button = box.querySelector(`[data-tt-id="message.${choice}"]`);
      if (!button) throw new Error('no button');
      fireEvent.click(button);
      const opens = () => h.actions().filter((a) => a.command === 'project.open').length;
      await waitFor(() => {
        expect(opens()).toBe(choice === 'retry' ? 2 : 1);
      });
      await act(async () => {
        await new Promise((r) => setTimeout(r, 50));
      });
      expect(opens()).toBe(choice === 'retry' ? 2 : 1);
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
  }
});
