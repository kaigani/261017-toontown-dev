// Preferences › Agent access (01 §7.3; 11 §15.1; M4 C4.4a, design note 2 §2): one command per changed group at OK, in
// table order, never `prefs.set`; a refusal part-way keeps the dialog open with the later groups staged; Regenerate
// token asks first; the status line follows the session events.
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { App } from '../App';
import { createHostApi, HostProvider } from '../ipc';
import { createScriptedHost, fail, ok, type ScriptedHost } from '../test-support/scriptedHost';
import { agentCommands, draftOf } from './agentAccess';

afterEach(cleanup);

const PREFS = [
  {
    key: 'agent.server.enabled',
    type: 'bool',
    default: false,
    value: false,
    applies: 'live',
    description: '',
  },
  {
    key: 'agent.server.port',
    type: 'int',
    default: 47215,
    value: 47215,
    min: 1024,
    max: 65535,
    applies: 'live',
    description: '',
  },
  {
    key: 'agent.server.allowRemote',
    type: 'bool',
    default: false,
    value: false,
    applies: 'live',
    description: '',
  },
  {
    key: 'agent.policy.defaults',
    type: 'object',
    default: {},
    value: {
      policy: 'assist',
      maxJobs: 2,
      maxBatchSteps: 1000,
      maxInlineImagePx: 2048,
      maxInlineBytes: 52428800,
      askInApp: true,
      letAgentMoveView: true,
      highlightAgentChanges: true,
    },
    applies: 'live',
    description: '',
  },
  {
    key: 'agent.activity.persist',
    type: 'bool',
    default: false,
    value: false,
    applies: 'live',
    description: '',
  },
  {
    key: 'agent.typedTools',
    type: 'object',
    default: { enabled: false, namespaces: [] },
    value: { enabled: false, namespaces: [] },
    applies: 'live',
    description: '',
  },
];

async function page(setup?: (h: ScriptedHost) => void): Promise<{ h: ScriptedHost; dialog: HTMLElement }> {
  let sessions: unknown[] = [];
  const h = createScriptedHost({
    'prefs.list': () => ok(PREFS),
    'agent.server.status': () =>
      ok({ enabled: sessions.length > 0, endpoint: null, sessions, pendingConfirmations: [] }),
  });
  setup?.(h);
  render(
    <HostProvider api={createHostApi(h)}>
      <App />
    </HostProvider>,
  );
  await screen.findByRole('toolbar', { name: 'Main toolbar' });
  act(() => {
    h.event('shell.preferences', { category: 'agent' });
  });
  const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
  await within(dialog).findByText(/The agent server is off/);
  // The status line follows a session connecting.
  sessions = [{ sessionId: 's1', client: 'mcp' }];
  act(() => {
    h.event('agent.session.connected', { sessionId: 's1' });
  });
  await within(dialog).findByText(/The agent server is running · 1 session$/);
  return { h, dialog };
}

const el = (id: string) => {
  const e = document.querySelector(`[data-tt-id="${id}"]`);
  if (!(e instanceof HTMLElement)) throw new Error(`no ${id}`);
  return e;
};
const AGENT = /^agent\.(server\.set|policy\.defaults\.set|typedTools\.set|activity\.persist\.set)$/;
const sent = (h: ScriptedHost) =>
  h
    .actions()
    .filter((a) => AGENT.test(a.command) || a.command === 'prefs.set')
    .map((a) => [a.command, a.params]);

function editAll() {
  fireEvent.click(el('agentAccess.server'));
  fireEvent.change(el('agentAccess.port'), { target: { value: '47300' } });
  fireEvent.blur(el('agentAccess.port'));
  fireEvent.change(el('agentAccess.policy'), { target: { value: 'autonomous' } });
  fireEvent.change(el('agentAccess.maxInlineBytes'), { target: { value: '8' } });
  fireEvent.blur(el('agentAccess.maxInlineBytes'));
  fireEvent.click(el('agentAccess.letAgentMoveView'));
  fireEvent.click(el('agentAccess.typedTools'));
  fireEvent.click(el('agentAccess.namespace.edit'));
  fireEvent.click(el('agentAccess.namespace.doc'));
  fireEvent.click(el('agentAccess.persist'));
}

describe('Preferences › Agent access (design note 2 §2)', () => {
  it('OK sends one command per changed group in table order, and no prefs.set', async () => {
    const { h, dialog } = await page();
    // One event at a time, as a person makes them: a numeric field commits what it shows at focus loss (D-61).
    editAll();
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    });
    expect(sent(h)).toEqual([
      ['agent.server.set', { enabled: true, port: 47300 }],
      [
        'agent.policy.defaults.set',
        { policy: 'autonomous', maxInlineBytes: 8388608, letAgentMoveView: false },
      ],
      ['agent.typedTools.set', { enabled: true, namespaces: ['doc', 'edit'] }],
      ['agent.activity.persist.set', { enabled: true }],
    ]);
  });

  it('a refusal part-way keeps the dialog open; the next OK sends only the groups not yet applied', async () => {
    let refuse = true;
    const { h, dialog } = await page((host) => {
      host.handlers['agent.policy.defaults.set'] = () =>
        refuse ? fail('arg.invalid', 'maxJobs must be between 1 and 8') : ok({});
    });
    // One event at a time, as a person makes them: a numeric field commits what it shows at focus loss (D-61).
    editAll();
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    });
    const alert = await within(dialog).findByRole('alert');
    expect(alert.textContent).toContain('arg.invalid');
    expect(alert.textContent).toContain('The Agent access settings before it were applied.');
    expect(sent(h).map(([c]) => c)).toEqual(['agent.server.set', 'agent.policy.defaults.set']);
    refuse = false;
    act(() => {
      fireEvent.click(within(dialog).getByRole('button', { name: 'OK' }));
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Preferences' })).toBeNull();
    });
    expect(sent(h).map(([c]) => c)).toEqual([
      'agent.server.set',
      'agent.policy.defaults.set',
      'agent.policy.defaults.set',
      'agent.typedTools.set',
      'agent.activity.persist.set',
    ]);
  });

  it('Allow remote connections is disabled with its reason; Regenerate token asks first', async () => {
    const { h } = await page();
    expect((el('agentAccess.allowRemote') as HTMLInputElement).disabled).toBe(true);
    expect(el('agentAccess.allowRemote').closest('label')?.textContent).toContain('MCP authorization flow');
    act(() => {
      fireEvent.click(el('agentAccess.regenerate'));
    });
    let box = await screen.findByRole('alertdialog');
    act(() => {
      fireEvent.click(within(box).getByRole('button', { name: 'Cancel' }));
    });
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
    expect(h.actions().some((a) => a.command === 'agent.token.regenerate')).toBe(false);
    act(() => {
      fireEvent.click(el('agentAccess.regenerate'));
    });
    box = await screen.findByRole('alertdialog');
    act(() => {
      fireEvent.click(within(box).getByRole('button', { name: 'Regenerate' }));
    });
    await waitFor(() => {
      expect(h.actions().filter((a) => a.command === 'agent.token.regenerate')).toHaveLength(1);
    });
  });

  it('an unchanged page sends nothing', () => {
    const d = draftOf(PREFS as never);
    expect(agentCommands(d, d)).toEqual([]);
  });
});
