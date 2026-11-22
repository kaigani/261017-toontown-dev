// C43B-R2-06 (01 §9.5 lines 925-927): the status bar's jobs segment opens the progress window, which lists every running
// job with its title, step and fraction, and a Cancel that sends `job.cancel {jobId}` where the job is cancellable, an
// agent session's jobs included (SI-160). An agent's job opens the window by itself; it closes when no job runs.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, NO_PROJECT_STATE, ok } from '../test-support/scriptedHost';

afterEach(cleanup);

/** The element, or a failure naming what is absent. */
function must<T>(v: T | null | undefined, what: string): T {
  if (v === null || v === undefined) throw new Error(`absent: ${what}`);
  return v;
}

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

const job = (jobId: string, session: string, cancellable: boolean, progress = 0.5) => ({
  jobId,
  commandId: 'render.range.run',
  state: 'running',
  progress,
  message: `step of ${jobId}`,
  session,
  cancellable,
});

function setup() {
  let jobs: unknown[] = [];
  const h = createScriptedHost({
    'app.state.get': () => ok(PROJECT_STATE),
    'agent.server.status': () =>
      ok({ enabled: true, sessions: [{ sessionId: 's1', client: 'mcp-client', policy: 'assist' }] }),
    'job.list': () => ok(jobs),
    'job.cancel': (p) => ok({ jobId: p['jobId'] }),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  const set = (next: unknown[]) => {
    jobs = next;
    act(() => {
      h.event('job.progress', {});
    });
  };
  return { h, set };
}

describe('the progress window (C43B-R2-06)', () => {
  it('opens for an agent job and cancels it; a job past its commit point offers no Cancel', async () => {
    const { h, set } = setup();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    set([job('job-1', 'agent:s1', true, 0.25), job('job-2', 'user', false, 0.8)]);
    const win = await screen.findByRole('dialog', { name: 'Jobs' });
    expect(win.getAttribute('aria-modal')).toBe('false');
    const segment = document.querySelector<HTMLButtonElement>('[data-tt-id="status.jobs"]');
    expect(segment?.textContent).toBe('2 jobs running');
    expect(within(win).getByText(/\(Agent: mcp-client\)/)).toBeTruthy();
    expect(within(win).getByText('step of job-1')).toBeTruthy();
    expect(win.querySelector('[data-tt-id="jobsWindow.progress.job-1"]')?.getAttribute('data-tt-value')).toBe(
      '25',
    );
    expect(win.querySelector('[data-tt-id="jobsWindow.cancel.job-2"]')).toBeNull();
    const cancel = win.querySelector<HTMLButtonElement>('[data-tt-id="jobsWindow.cancel.job-1"]');
    expect(cancel?.getAttribute('data-tt-params')).toBe('{"jobId":"job-1"}');
    fireEvent.click(must(cancel, 'Cancel'));
    await waitFor(() => {
      expect(h.actions().filter((a) => a.command === 'job.cancel')).toEqual([
        expect.objectContaining({ command: 'job.cancel', params: { jobId: 'job-1' } }),
      ]);
    });
    // No job left: the window closes and the segment is empty.
    set([]);
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Jobs' })).toBeNull();
    });
    expect(segment?.textContent).toBe('');
    expect(segment?.disabled).toBe(true);
  });

  it("the window's own job does not open it; the segment opens and closes it", async () => {
    const { set } = setup();
    await screen.findByRole('toolbar', { name: 'Main toolbar' });
    set([job('job-3', 'user', true)]);
    const segment = document.querySelector<HTMLButtonElement>('[data-tt-id="status.jobs"]');
    await waitFor(() => {
      expect(segment?.textContent).toBe('1 job running');
    });
    expect(screen.queryByRole('dialog', { name: 'Jobs' })).toBeNull();
    fireEvent.click(must(segment, 'jobs segment'));
    const win = await screen.findByRole('dialog', { name: 'Jobs' });
    expect(win.querySelector('[data-tt-id="jobsWindow.cancel.job-3"]')).not.toBeNull();
    fireEvent.click(must(segment, 'jobs segment'));
    expect(screen.queryByRole('dialog', { name: 'Jobs' })).toBeNull();
  });
});
