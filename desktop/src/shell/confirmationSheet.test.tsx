// The confirmation sheet (11 §6.3, §6.4; ruling D-58): not modal and never taking the focus; Decline the default
// (Return or Escape with the focus inside); Accept only when chosen; queued requests; a request decided elsewhere or
// expired leaves, the sheet saying why it ended unanswered.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, ok, type ScriptedHost } from '../test-support/scriptedHost';

afterEach(cleanup);

const request = (n: number) => ({
  confirmationId: `cf_${String(n)}`,
  sessionId: 's1',
  commandId: 'agent.activity.export',
  summary: `Overwrite log ${String(n)}.jsonl`,
  consequences: [{ kind: 'overwrite', path: `/w/log${String(n)}.jsonl` }],
  category: 'overwrite',
  decision: null,
  inApp: true,
  expiresInMs: 300000,
});

async function sheet(pending: () => unknown[]): Promise<ScriptedHost> {
  const h = createScriptedHost({
    'agent.server.status': () =>
      ok({
        enabled: true,
        endpoint: null,
        sessions: [{ sessionId: 's1', client: 'Helper' }],
        pendingConfirmations: pending(),
      }),
  });
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('dialog', { name: 'Start screen' });
  return h;
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const responses = (h: ScriptedHost) =>
  h
    .actions()
    .filter((a) => a.command === 'agent.confirmation.respond')
    .map((a) => a.params);

describe('the confirmation sheet (D-58)', () => {
  it('appears without taking the focus; Return inside declines; Accept needs to be chosen', async () => {
    let pending = [request(1), request(2), request(3)];
    const h = await sheet(() => pending);
    const before = document.activeElement;
    act(() => {
      h.event('agent.confirmation.requested', { confirmationId: 'cf_1' });
    });
    const win = await screen.findByRole('dialog', { name: 'Agent confirmation' });
    expect(win.getAttribute('aria-modal')).toBe('false');
    expect(document.activeElement).toBe(before);
    expect(win.textContent).toContain('Helper asks to');
    expect(el('confirmSheet.queue').textContent).toContain('1 of 3');
    expect(el('confirmSheet.summary').textContent).toBe('Overwrite log 1.jsonl');
    // Return with the focus inside the sheet (on Decline) declines.
    act(() => {
      el('confirmSheet.decline').focus();
      fireEvent.keyDown(el('confirmSheet.decline'), { key: 'Enter' });
    });
    await waitFor(() => {
      expect(responses(h)).toEqual([{ confirmationId: 'cf_1', accept: false }]);
    });
    pending = [request(2), request(3)];
    act(() => {
      h.event('shell.agentConfirmation', { confirmationId: 'cf_1', outcome: 'declined' });
    });
    await waitFor(() => {
      expect(el('confirmSheet.summary').textContent).toBe('Overwrite log 2.jsonl');
    });
    // Escape inside declines too; Accept is sent only when chosen (the focus moved to it and Return).
    act(() => {
      el('confirmSheet.accept').focus();
      fireEvent.keyDown(el('confirmSheet.accept'), { key: 'Enter' });
    });
    act(() => {
      fireEvent.click(el('confirmSheet.accept'));
    });
    await waitFor(() => {
      expect(responses(h).slice(1)).toEqual([{ confirmationId: 'cf_2', accept: true }]);
    });
    pending = [request(3)];
    act(() => {
      h.event('shell.agentConfirmation', { confirmationId: 'cf_2', outcome: 'accepted' });
    });
    await waitFor(() => {
      expect(el('confirmSheet.summary').textContent).toBe('Overwrite log 3.jsonl');
    });
    act(() => {
      fireEvent.keyDown(el('confirmSheet.summary'), { key: 'Escape' });
    });
    await waitFor(() => {
      expect(responses(h).slice(2)).toEqual([{ confirmationId: 'cf_3', accept: false }]);
    });
  });

  it('a request decided in another window or overtaken by a document change leaves; the sheet says why', async () => {
    let pending = [request(1)];
    const h = await sheet(() => pending);
    act(() => {
      h.event('agent.confirmation.requested', { confirmationId: 'cf_1' });
    });
    await screen.findByRole('dialog', { name: 'Agent confirmation' });
    pending = [];
    act(() => {
      h.event('shell.agentConfirmation', {
        confirmationId: 'cf_1',
        outcome: 'expired',
        reason: 'the document changed',
      });
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Agent confirmation' })).toBeNull();
    });
    expect(el('confirmSheet.ended').textContent).toContain('The request expired: the document changed.');
    expect(responses(h)).toEqual([]);
  });
});
