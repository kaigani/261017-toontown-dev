// The Activity panel (11 §11, §6.4; M4 C4.4a, design note 2 §3; SI-174): the agent activity log of this project session
// and the connected sessions. It reads `agent.activity.list` once, then appends what the host's `shell.agentActivity`
// notice brings (no polling); a gap in `seq` and every open or close read the log again. A row's Undo back to before
// it moves the history (`edit.history.move`) after naming the steps; Copy as batch JSON writes the entry in the form the
// batch export writes (`clipboard`); Export… writes the log. The sessions strip grants a policy or scopes, pauses,
// resumes and disconnects, each through its command.
import { useCallback, useEffect, useState } from 'react';

import { useActions } from '../shell/actionsApi';
import { copyText } from '../shell/clipboard';
import { commandLabel } from '../shell/commandLabel';
import { optionRef, runChosen } from '../shell/listControl';
import { useMessages } from '../shell/messagesApi';
import { useShell, useSlice } from '../state/context';
import {
  appended,
  asEntries,
  batchOf,
  hasMediaReference,
  SHOWN,
  undoBack,
  type ActivityEntry,
  type HistoryStep,
} from './activityModel';

type Json = Record<string, unknown>;

/** A member as text: strings as they are, anything else empty. */
const text = (v: unknown, fallback = ''): string => (typeof v === 'string' ? v : fallback);

/** D-62 item 3: the registry's label; a batch by its own label or "Batch (n steps)" (`commandLabel`). */
const label = (id: string, params?: unknown) => commandLabel(id, params);

const SCOPES = [
  'read',
  'edit',
  'ui',
  'input',
  'files.read',
  'files.write',
  'prefs',
  'library.global',
  'network',
  'print',
];
const POLICIES = ['observe', 'assist', 'autonomous'];

function useActivity(): ActivityEntry[] {
  const { engine } = useShell();
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  const read = useCallback(() => {
    void engine
      .run('agent.activity.list', { limit: SHOWN })
      .done.then((env) => {
        setEntries(env.status === 'ok' ? asEntries(env.result) : []);
      })
      .catch(() => undefined);
  }, [engine]);
  useEffect(() => {
    read();
    let stopped = false;
    let off: (() => void) | null = null;
    void engine
      .onEvent((e) => {
        if (e.name === 'project.opened' || e.name === 'project.closed') read();
        if (e.name !== 'shell.agentActivity') return;
        const entry = asEntries([(e.payload as { entry?: unknown } | null)?.entry])[0];
        if (!entry) return;
        setEntries((list) => {
          const next = appended(list, entry);
          if (next === null) {
            read();
            return list;
          }
          return next;
        });
      })
      .then((u) => {
        if (stopped) {
          u();
          return;
        }
        off = u;
        // Read again once subscribed (D-86 item 2): nothing between the first read and the subscription is lost.
        read();
      });
    return () => {
      stopped = true;
      off?.();
    };
  }, [engine, read]);
  return entries;
}

function useSessions(): Json[] {
  const { engine } = useShell();
  const [sessions, setSessions] = useState<Json[]>([]);
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    const read = () => {
      void engine
        .run('agent.server.status', {})
        .done.then((env) => {
          const s = (env.result as { sessions?: unknown } | undefined)?.sessions;
          if (!stopped) setSessions(Array.isArray(s) ? (s as Json[]) : []);
        })
        .catch(() => undefined);
    };
    read();
    void engine
      .onEvent((e) => {
        if (
          e.name === 'agent.session.connected' ||
          e.name === 'agent.session.disconnected' ||
          e.name === 'agent.paused' ||
          e.name === 'agent.resumed' ||
          e.name === 'shell.agentSession'
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
  }, [engine]);
  return sessions;
}

export function ActivityPanel() {
  const { engine } = useShell();
  const actions = useActions();
  const messages = useMessages();
  const entries = useActivity();
  const sessions = useSessions();
  const history = useSlice('history') as
    { entries: readonly HistoryStep[]; position: number } | null | undefined;
  const [selected, setSelected] = useState<number | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [scopesOf, setScopesOf] = useState<{ sessionId: string; scopes: string[] } | null>(null);
  const shown = [...entries].reverse();
  const entry = entries.find((e) => e.seq === selected) ?? null;
  const back = entry ? undoBack(entry, history) : null;

  const exportLog = () => {
    void engine
      .pickPath({
        kind: 'save',
        title: 'Export Agent Activity',
        filters: [{ name: 'JSON lines', extensions: ['jsonl'] }],
      })
      .catch(() => null)
      .then((path) => {
        if (!path) return;
        // The platform's save dialog has asked about an existing file already.
        void actions
          .run('agent.activity.export', { path, format: 'jsonl', onExists: 'overwrite' })
          .then((env) => {
            if (env.status === 'ok') setNotice(`Exported to ${path}.`);
          });
      });
  };

  return (
    <div className="activity-panel" data-tt-role="panel" data-tt-id="activity" data-tt-label="Activity">
      <section className="activity-sessions" aria-label="Agent sessions">
        {sessions.length === 0 && <p className="empty">No agent is connected.</p>}
        {sessions.map((s) => {
          const id = String(s['sessionId']);
          const paused = s['paused'] === true;
          return (
            <div
              key={id}
              className="activity-session"
              data-tt-role="group"
              data-tt-id={`activity.session.${id}`}
            >
              <strong>{text(s['client'], id)}</strong>{' '}
              <span className="field-hint">since {text(s['connectedAt'])}</span>{' '}
              <select
                aria-label={`Policy of ${text(s['client'], id)}`}
                value={text(s['policy'], 'assist')}
                data-tt-role="field"
                data-tt-id={`activity.session.${id}.policy`}
                data-tt-command="agent.session.grant"
                onChange={runChosen(actions)}
              >
                {POLICIES.map((p) => (
                  <option
                    key={p}
                    value={p}
                    {...optionRef(`activity.session.${id}.policy`, p, p, 'agent.session.grant', {
                      sessionId: id,
                      policy: p,
                    })}
                  >
                    {p}
                  </option>
                ))}
              </select>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`activity.session.${id}.scopes`}
                data-tt-gui-only="disclose"
                onClick={() => {
                  setScopesOf({
                    sessionId: id,
                    scopes: Array.isArray(s['scopes']) ? (s['scopes'] as string[]) : [],
                  });
                }}
              >
                Scopes…
              </button>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`activity.session.${id}.${paused ? 'resume' : 'pause'}`}
                data-tt-command={paused ? 'agent.session.resume' : 'agent.session.pause'}
                data-tt-params={JSON.stringify({ sessionId: id })}
                onClick={() => {
                  void actions.run(paused ? 'agent.session.resume' : 'agent.session.pause', {
                    sessionId: id,
                  });
                }}
              >
                {paused ? 'Resume' : 'Pause'}
              </button>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`activity.session.${id}.disconnect`}
                data-tt-command="agent.session.disconnect"
                data-tt-params={JSON.stringify({ sessionId: id })}
                onClick={() => {
                  void actions.run('agent.session.disconnect', { sessionId: id });
                }}
              >
                Disconnect
              </button>
              {scopesOf?.sessionId === id && (
                <fieldset
                  className="activity-scopes"
                  data-tt-role="group"
                  data-tt-id={`activity.session.${id}.scopeList`}
                >
                  <legend>Scopes</legend>
                  {SCOPES.map((sc) => (
                    <label key={sc}>
                      <input
                        type="checkbox"
                        checked={scopesOf.scopes.includes(sc)}
                        data-tt-role="checkbox"
                        data-tt-id={`activity.session.${id}.scope.${sc}`}
                        data-tt-gui-only="typing"
                        onChange={(e) => {
                          const rest = scopesOf.scopes.filter((x) => x !== sc);
                          setScopesOf({ sessionId: id, scopes: e.target.checked ? [...rest, sc] : rest });
                        }}
                      />
                      {sc}
                    </label>
                  ))}
                  <button
                    type="button"
                    data-tt-role="button"
                    data-tt-id={`activity.session.${id}.scopes.ok`}
                    data-tt-command="agent.session.grant"
                    onClick={() => {
                      const scopes = SCOPES.filter((x) => scopesOf.scopes.includes(x));
                      setScopesOf(null);
                      void actions.run('agent.session.grant', { sessionId: id, scopes });
                    }}
                  >
                    OK
                  </button>
                  <button
                    type="button"
                    data-tt-role="button"
                    data-tt-id={`activity.session.${id}.scopes.cancel`}
                    data-tt-gui-only="dismiss"
                    onClick={() => {
                      setScopesOf(null);
                    }}
                  >
                    Cancel
                  </button>
                </fieldset>
              )}
            </div>
          );
        })}
      </section>
      <div className="activity-tools">
        <button
          type="button"
          disabled={!back?.ok}
          title={back && !back.ok ? back.reason : undefined}
          data-tt-role="button"
          data-tt-id="activity.undoBack"
          data-tt-command="edit.history.move"
          {...(back?.ok ? { 'data-tt-params': JSON.stringify({ position: back.position }) } : {})}
          onClick={() => {
            if (!back?.ok || !entry) return;
            const go = () => void actions.run('edit.history.move', { position: back.position });
            // Every later step that is not this agent's is named first (11 §11: a linear history).
            if (back.yours === 0) {
              go();
              return;
            }
            void messages
              .show({
                severity: 'warning',
                messageId: 'agent.undoBack',
                title: 'Undo back to before this entry?',
                text: `Undo ${String(back.steps)} steps, ${String(back.yours)} of them yours?`,
                buttons: [
                  { id: 'undo', label: 'Undo', role: 'destructive' },
                  { id: 'cancel', label: 'Cancel', role: 'cancel' },
                ],
              })
              .then((a) => {
                if (a.button === 'undo') go();
              });
          }}
        >
          Undo back to before this entry{back && !back.ok ? ` (${back.reason})` : ''}
        </button>
        <button
          type="button"
          disabled={!entry}
          data-tt-role="button"
          data-tt-id="activity.copyBatch"
          data-tt-gui-only="clipboard"
          onClick={() => {
            if (!entry) return;
            void copyText(JSON.stringify(batchOf(entry), null, 2)).catch(() => undefined);
            setNotice(
              hasMediaReference(entry)
                ? 'Copied: contains media references; not replayable as is.'
                : 'Copied.',
            );
          }}
        >
          Copy as batch JSON
        </button>
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="activity.export"
          data-tt-command="agent.activity.export"
          onClick={exportLog}
        >
          Export…
        </button>
        {notice && (
          <span className="field-hint" role="status" data-tt-role="text" data-tt-id="activity.notice">
            {notice}
          </span>
        )}
      </div>
      {shown.length === 0 ? (
        <p className="empty">No agent activity yet.</p>
      ) : (
        <ol className="activity-list" aria-label="Agent activity">
          {shown.map((e) => (
            <li key={e.seq}>
              <button
                type="button"
                aria-pressed={selected === e.seq}
                className={`activity-row activity-${e.status}`}
                title={e.commandId}
                data-tt-role="button"
                data-tt-id={`activity.entry.${String(e.seq)}`}
                data-tt-gui-only="navigate"
                onClick={() => {
                  setSelected(e.seq);
                  setNotice(null);
                }}
              >
                <time>{e.time.slice(11, 19)}</time> <span className="client">{e.client}</span>{' '}
                <span className="what">{label(e.commandId, e.params)}</span>{' '}
                <span className="status">
                  {e.status}
                  {e.errorCode ? ` ${e.errorCode}` : ''}
                </span>{' '}
                <span className="changed">{e.changedCount} changed</span>{' '}
                <span className="duration">{e.durationMs} ms</span>
                {e.dryRun && <span className="tag"> dry run</span>}
                {e.cachedRetry && <span className="tag"> retry</span>}
              </button>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
