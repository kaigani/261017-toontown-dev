// A job's failure is told to the user (01 §9.5; found by the D-44 row "error box › OK"): `job.wait` answers `ok` with
// the job's record, and a record whose state is `failed` opens the error box with the job's own error. A cancelled job
// is the user's choice: no error box. A job that succeeded shows nothing.
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

afterEach(cleanup);

function record(state: string, error?: { code: string; message: string }) {
  return ok({
    jobId: 'job-1',
    commandId: 'project.open',
    state,
    progress: 0,
    cancellable: false,
    ...(error ? { error } : {}),
  });
}

async function openWith(waitAnswer: ReturnType<typeof ok>) {
  const h = createScriptedHost({
    'app.state.get': () => ok(NO_PROJECT_STATE),
    'project.open': () => ok({ jobId: 'job-1' }),
    'job.wait': () => waitAnswer,
  });
  h.picked = '/p/not-a-project.animpkg';
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  act(() => {
    h.menu('project.open', {});
  });
  await waitFor(() => {
    expect(h.actions().filter((a) => a.command === 'project.open').length).toBe(1);
  });
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return h;
}

describe('a job that does not succeed (01 §9.5)', () => {
  it('a failed job opens the error box with the job error', async () => {
    await openWith(
      record('failed', {
        code: 'project.invalidFormat',
        message: 'no end-of-central-directory record (not a zip archive, or truncated)',
      }),
    );
    const box = await screen.findByRole('alertdialog');
    expect(box.getAttribute('data-tt-id')).toBe('message.project.invalidFormat');
    expect(box.textContent).toContain('no end-of-central-directory record');
  });

  it('a cancelled job opens no error box', async () => {
    await openWith(record('cancelled'));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('a job that succeeded opens no error box', async () => {
    await openWith(record('succeeded'));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});
