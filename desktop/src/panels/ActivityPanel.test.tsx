// The Activity panel (11 §11; M4 C4.4a, design note 2 §3): appends from the host's notice alone and re-reads on a gap;
// Undo back to before an entry names the person's steps first; Copy as batch JSON writes the export's batch form; the
// sessions strip sends its commands.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { batchOf, type ActivityEntry } from './activityModel';

const writeText = vi.fn<(text: string) => Promise<void>>();
beforeEach(() => {
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
});
afterEach(cleanup);

const entry = (seq: number, over: Partial<ActivityEntry> = {}): ActivityEntry => ({
  seq,
  time: `2026-11-18T10:00:0${String(seq)}Z`,
  sessionId: 's1',
  client: 'mcp',
  tool: 'run_command',
  commandId: 'draw.rectangle.add',
  params: { from: { x: 0, y: 0 } },
  status: 'ok',
  undoStepId: `u-${String(seq)}`,
  changedCount: 1,
  durationMs: 3,
  ...over,
});

async function panel(log: () => ActivityEntry[], history?: unknown): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'agent.activity.list': () => ok(log()),
    'agent.server.status': () =>
      ok({
        enabled: true,
        endpoint: null,
        sessions: [
          { sessionId: 's1', client: 'mcp', policy: 'assist', scopes: ['read', 'edit'], paused: false },
        ],
        pendingConfirmations: [],
      }),
    ...(history ? { 'edit.history.get': () => ok(history) } : {}),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App floatPanel="activity" />
    </HostProvider>,
  );
  await waitFor(() => {
    expect(document.querySelector('[data-tt-id="activity.session.s1"]')).not.toBeNull();
  });
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const rows = () =>
  [...document.querySelectorAll('[data-tt-id^="activity.entry."]')].map((e) => e.getAttribute('data-tt-id'));

describe('the Activity panel (11 §11)', () => {
  it("appends the host's notices without polling, and re-reads the log on a gap", async () => {
    let log = [entry(1)];
    const h = await panel(() => log);
    await waitFor(() => {
      expect(rows()).toEqual(['activity.entry.1']);
    });
    const reads = () => h.sent.filter((s) => s.command === 'agent.activity.list').length;
    const before = reads();
    act(() => {
      h.event('shell.agentActivity', { entry: entry(2) });
    });
    await waitFor(() => {
      expect(rows()).toEqual(['activity.entry.2', 'activity.entry.1']);
    });
    expect(reads()).toBe(before);
    // seq 4 after 2: a notice was missed, so the log is read again.
    log = [entry(1), entry(2), entry(3), entry(4)];
    act(() => {
      h.event('shell.agentActivity', { entry: entry(4) });
    });
    await waitFor(() => {
      expect(rows()).toEqual([
        'activity.entry.4',
        'activity.entry.3',
        'activity.entry.2',
        'activity.entry.1',
      ]);
    });
    expect(reads()).toBe(before + 1);
  });

  it('Undo back to before an entry moves the history, naming the steps that are yours first', async () => {
    const history = {
      entries: [
        { index: 0, stepId: 'u-1', label: 'Rectangle', source: 'agent', agentName: 'mcp', time: '' },
        { index: 1, stepId: 'x-9', label: 'Brush', source: 'user', time: '' },
      ],
      position: 2,
      cleanPosition: null,
    };
    const h = await panel(() => [entry(1)], history);
    act(() => {
      fireEvent.click(await_('activity.entry.1'));
    });
    await waitFor(() => {
      expect((el('activity.undoBack') as HTMLButtonElement).disabled).toBe(false);
    });
    act(() => {
      fireEvent.click(el('activity.undoBack'));
    });
    const box = await screen.findByRole('alertdialog');
    expect(box.textContent).toContain('Undo 2 steps, 1 of them yours?');
    act(() => {
      fireEvent.click(within(box).getByRole('button', { name: 'Undo' }));
    });
    await waitFor(() => {
      expect(
        h
          .actions()
          .filter((a) => a.command === 'edit.history.move')
          .map((a) => a.params),
      ).toEqual([{ position: 0 }]);
    });
  });

  it('an entry whose step is gone or undone cannot be undone back to', async () => {
    await panel(() => [entry(1, { undoStepId: 'gone' })], { entries: [], position: 0, cleanPosition: null });
    act(() => {
      fireEvent.click(await_('activity.entry.1'));
    });
    await waitFor(() => {
      expect(el('activity.undoBack').textContent).toContain('its step is no longer in the history');
    });
    expect((el('activity.undoBack') as HTMLButtonElement).disabled).toBe(true);
  });

  it('Copy as batch JSON writes the batch export form and runs nothing', async () => {
    const batch = entry(1, {
      commandId: 'batch',
      tool: 'batch',
      params: { label: 'Two', atomic: true, steps: [{ id: 'edit.undo', params: {}, as: null }] },
    });
    const h = await panel(() => [batch]);
    act(() => {
      fireEvent.click(await_('activity.entry.1'));
    });
    const before = h.actions().length;
    act(() => {
      fireEvent.click(el('activity.copyBatch'));
    });
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(JSON.parse(writeText.mock.calls[0]?.[0] ?? 'null')).toEqual({
      label: 'Agent activity',
      atomic: false,
      stopOnError: true,
      steps: [
        { id: 'batch', params: { label: 'Two', atomic: true, steps: [{ id: 'edit.undo', params: {} }] } },
      ],
    });
    expect(batchOf(entry(2)).steps).toEqual([{ id: 'draw.rectangle.add', params: { from: { x: 0, y: 0 } } }]);
    expect(h.actions().length).toBe(before);
  });

  it('the sessions strip grants a policy at once and scopes at OK; Pause and Disconnect send theirs', async () => {
    const h = await panel(() => []);
    act(() => {
      fireEvent.change(el('activity.session.s1.policy'), { target: { value: 'autonomous' } });
    });
    act(() => {
      fireEvent.click(el('activity.session.s1.scopes'));
    });
    act(() => {
      fireEvent.click(el('activity.session.s1.scope.ui'));
    });
    act(() => {
      fireEvent.click(el('activity.session.s1.scopes.ok'));
    });
    act(() => {
      fireEvent.click(el('activity.session.s1.pause'));
      fireEvent.click(el('activity.session.s1.disconnect'));
    });
    await waitFor(() => {
      expect(
        h
          .actions()
          .filter((a) => a.command.startsWith('agent.session.'))
          .map((a) => [a.command, a.params]),
      ).toEqual([
        ['agent.session.grant', { sessionId: 's1', policy: 'autonomous' }],
        ['agent.session.grant', { sessionId: 's1', scopes: ['read', 'edit', 'ui'] }],
        ['agent.session.pause', { sessionId: 's1' }],
        ['agent.session.disconnect', { sessionId: 's1' }],
      ]);
    });
  });
});

function await_(id: string): HTMLElement {
  return el(id);
}
