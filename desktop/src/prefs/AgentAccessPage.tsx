// Preferences › Agent access (01 §7.3 lines 783-789; 11 §6.4, §15.1; M4 C4.4a, design note 2 §2). The settings are
// staged like the rest of the dialog; at OK each changed group sends its one command in table order (`agentAccess.ts`),
// never `prefs.set`. Each control declares the command it contributes. Regenerate token acts at once, after a question.
// The status line re-reads on the session events and on `prefs.changed`.
import { useEffect, useMemo, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { optionRef } from '../shell/listControl';
import { useMessages } from '../shell/messagesApi';
import type { ShellServices } from '../state/context';
import { LIMITS, namespacesOf, type AgentDraft } from './agentAccess';

const POLICIES: readonly [string, string][] = [
  ['observe', 'Observe (read only)'],
  ['assist', 'Assist (asks before risky changes)'],
  ['autonomous', 'Autonomous'],
];

const TOGGLES: readonly [string, string][] = [
  ['askInApp', 'Ask me in the app when an agent needs confirmation'],
  ['letAgentMoveView', 'Let the agent move my view'],
  ['highlightAgentChanges', 'Highlight agent changes'],
];

const NO_REMOTE = 'needs the MCP authorization flow (11 §4), which this version does not have';

type Json = Record<string, unknown>;

export function AgentAccessPage({
  services,
  draft,
  projectOpen,
  onChange,
}: {
  services: ShellServices;
  draft: AgentDraft;
  projectOpen: boolean;
  onChange: (d: AgentDraft) => void;
}) {
  const messages = useMessages();
  const [status, setStatus] = useState<Json>({});
  const namespaces = useMemo(
    () => namespacesOf(COMMAND_INDEX as unknown as Record<string, { event?: boolean }>),
    [],
  );
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    const read = () => {
      void services.engine
        .run('agent.server.status', {})
        .done.then((env) => {
          if (!stopped && env.status === 'ok' && typeof env.result === 'object' && env.result !== null)
            setStatus(env.result as Json);
        })
        .catch(() => undefined);
    };
    read();
    void services.engine
      .onEvent((e) => {
        if (
          e.name === 'agent.session.connected' ||
          e.name === 'agent.session.disconnected' ||
          e.name === 'prefs.changed'
        )
          read();
      })
      .then((u) => {
        if (stopped) u();
        else off = u;
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [services]);

  const server = draft.server;
  const setServer = (s: Partial<AgentDraft['server']>) => {
    onChange({ ...draft, server: { ...server, ...s } });
  };
  const setPolicy = (k: string, v: unknown) => {
    onChange({ ...draft, policy: { ...draft.policy, [k]: v } });
  };
  const sessions = Array.isArray(status['sessions']) ? status['sessions'].length : 0;
  const endpoint = typeof status['endpoint'] === 'string' ? status['endpoint'] : null;
  return (
    <div className="agent-access" data-tt-role="group" data-tt-id="agentAccess">
      <p data-tt-role="text" data-tt-id="agentAccess.status">
        {status['enabled'] === true ? 'The agent server is running' : 'The agent server is off'}
        {endpoint ? ` at ${endpoint}` : ''} · {sessions} session{sessions === 1 ? '' : 's'}
      </p>
      <label>
        <input
          type="checkbox"
          checked={server.enabled}
          data-tt-role="checkbox"
          data-tt-id="agentAccess.server"
          data-tt-command="agent.server.set"
          onChange={(e) => {
            setServer({ enabled: e.target.checked });
          }}
        />
        Run the agent server
      </label>
      <label>
        Port{' '}
        <input
          type="number"
          min={1024}
          max={65535}
          value={server.port}
          disabled={!server.enabled}
          data-tt-role="field"
          data-tt-id="agentAccess.port"
          data-tt-command="agent.server.set"
          onChange={(e) => {
            setServer({ port: Number(e.target.value) });
          }}
        />
      </label>
      <label title={NO_REMOTE}>
        <input
          type="checkbox"
          checked={server.allowRemote}
          disabled
          data-tt-role="checkbox"
          data-tt-id="agentAccess.allowRemote"
          data-tt-command="agent.server.set"
          onChange={(e) => {
            setServer({ allowRemote: e.target.checked });
          }}
        />
        Allow remote connections <span className="field-hint">({NO_REMOTE})</span>
      </label>
      <label>
        Default policy{' '}
        <select
          value={String(draft.policy['policy'])}
          data-tt-role="field"
          data-tt-id="agentAccess.policy"
          data-tt-command="agent.policy.defaults.set"
          onChange={(e) => {
            setPolicy('policy', e.target.value);
          }}
        >
          {POLICIES.map(([v, label]) => (
            <option
              key={v}
              value={v}
              {...optionRef('agentAccess.policy', v, label, 'agent.policy.defaults.set', { policy: v })}
            >
              {label}
            </option>
          ))}
        </select>
      </label>
      {LIMITS.map((l) => {
        const raw = draft.policy[l.key];
        const shown = typeof raw === 'number' ? raw / (l.scale ?? 1) : '';
        return (
          <label key={l.key}>
            {l.label}{' '}
            <input
              type="number"
              min={l.min}
              max={l.max}
              value={shown}
              data-tt-role="field"
              data-tt-id={`agentAccess.${l.key}`}
              data-tt-command="agent.policy.defaults.set"
              onChange={(e) => {
                setPolicy(l.key, Math.round(Number(e.target.value) * (l.scale ?? 1)));
              }}
            />
          </label>
        );
      })}
      {TOGGLES.map(([k, label]) => (
        <label key={k}>
          <input
            type="checkbox"
            checked={draft.policy[k] === true}
            data-tt-role="checkbox"
            data-tt-id={`agentAccess.${k}`}
            data-tt-command="agent.policy.defaults.set"
            onChange={(e) => {
              setPolicy(k, e.target.checked);
            }}
          />
          {label}
        </label>
      ))}
      <fieldset>
        <legend>
          <label>
            <input
              type="checkbox"
              checked={draft.typed.enabled}
              data-tt-role="checkbox"
              data-tt-id="agentAccess.typedTools"
              data-tt-command="agent.typedTools.set"
              onChange={(e) => {
                onChange({ ...draft, typed: { ...draft.typed, enabled: e.target.checked } });
              }}
            />
            Expose commands as individual tools
          </label>
        </legend>
        <p className="field-hint">Limit to these namespaces (none ticked: every namespace).</p>
        <div className="agent-namespaces">
          {namespaces.map((n) => (
            <label key={n}>
              <input
                type="checkbox"
                checked={draft.typed.namespaces.includes(n)}
                disabled={!draft.typed.enabled}
                data-tt-role="checkbox"
                data-tt-id={`agentAccess.namespace.${n}`}
                data-tt-command="agent.typedTools.set"
                onChange={(e) => {
                  const list = draft.typed.namespaces.filter((x) => x !== n);
                  onChange({
                    ...draft,
                    typed: { ...draft.typed, namespaces: e.target.checked ? [...list, n] : list },
                  });
                }}
              />
              {n}
            </label>
          ))}
        </div>
      </fieldset>
      <label title={projectOpen ? undefined : 'nothing is kept while no project is open'}>
        <input
          type="checkbox"
          checked={draft.persist}
          data-tt-role="checkbox"
          data-tt-id="agentAccess.persist"
          data-tt-command="agent.activity.persist.set"
          onChange={(e) => {
            onChange({ ...draft, persist: e.target.checked });
          }}
        />
        Keep the activity log beside the project
      </label>
      <div className="buttons">
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="agentAccess.regenerate"
          data-tt-command="agent.token.regenerate"
          onClick={() => {
            void messages
              .show({
                severity: 'warning',
                messageId: 'agent.tokenRegenerate',
                title: 'Regenerate the agent token?',
                text: 'Connected HTTP clients will be disconnected and need the new token from the discovery file.',
                buttons: [
                  { id: 'regenerate', label: 'Regenerate', role: 'destructive' },
                  { id: 'cancel', label: 'Cancel', role: 'cancel' },
                ],
              })
              .then((a) => {
                if (a.button !== 'regenerate') return;
                void services.engine.run('agent.token.regenerate', {}).done.then((env) => {
                  if (env.status !== 'ok') void messages.failure(env, 'Regenerate token');
                });
              });
          }}
        >
          Regenerate token…
        </button>
      </div>
    </div>
  );
}
