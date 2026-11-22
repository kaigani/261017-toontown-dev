// Preferences › Agent access (01 §7.3 lines 783-789; 11 §6.4, §15.1; M4 C4.4a, design note 2 §2). The settings are
// staged like the rest of the dialog; at OK each changed group sends its one command in table order (`agentAccess.ts`),
// never `prefs.set`. Each control declares the command it contributes. Regenerate token acts at once, after a question.
// The status line re-reads on the session events and on `prefs.changed`.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { copyText } from '../shell/clipboard';
import { optionRef } from '../shell/listControl';
import { declaredRange, type NumericRange } from '../shell/numeric';
import { NumericField } from '../shell/NumericField';
import { useMessages } from '../shell/messagesApi';
import type { ShellServices } from '../state/context';
import { LIMITS, namespacesOf, type AgentDraft } from './agentAccess';
import { tokenController } from './tokenController';

const POLICIES: readonly [string, string][] = [
  ['observe', 'Observe (read only)'],
  ['assist', 'Assist (asks before risky changes)'],
  ['autonomous', 'Autonomous'],
];

/** A limit's range in the field's unit: the command's declared range (bytes shown as MB are scaled), else the
 *  table's. */
function limitRange(l: (typeof LIMITS)[number]): NumericRange {
  const d = declaredRange('agent.policy.defaults.set', l.key);
  const scale = l.scale ?? 1;
  return {
    min: d.min !== undefined ? Math.ceil(d.min / scale) : l.min,
    max: d.max !== undefined ? Math.floor(d.max / scale) : l.max,
    integer: true,
  };
}

const TOGGLES: readonly [string, string][] = [
  ['askInApp', 'Ask me in the app when an agent needs confirmation'],
  ['letAgentMoveView', 'Let the agent move my view'],
  ['highlightAgentChanges', 'Highlight agent changes'],
];

const NO_REMOTE = 'needs the MCP authorization flow (11 §4), which this version does not have';

type Json = Record<string, unknown>;

/** What the masked field shows: a fixed run of dots, nothing derived from the token. */
const MASK = '\u2022'.repeat(16);

/**
 * The agent token (11 §4 lines 107-108: "shown and regenerable in Preferences > Agent access"; ruling D-62 item 2,
 * C44A-R1-41). No surface an agent can read ever contains it:
 * - the field is masked by default; Show reveals it only while the person asks for it (until Hide, leaving the page,
 *   the window losing focus, a regenerated token, or a window capture);
 * - the window's UI tree always reports the field as `redacted` (`data-tt-value`), revealed or not;
 * - the host is the authority, not the page (D-65 item 2): every answer of `agent_token` is a numbered delivery the
 *   host counts as possibly on screen until the page acknowledges it hidden (`token_hidden`); a window capture is
 *   granted only when no delivery is outstanding (it asks the page to hide the token, `token-hide`, and waits for the
 *   acknowledgement), and while it runs `agent_token` delivers nothing;
 * - in the page, one reveal in flight at a time (Show disabled), each with a generation: Hide, leaving the page, the
 *   window losing focus or closing, a regenerated token and `token-hide` end the current generation, and an answer of
 *   an ended generation is acknowledged hidden without being painted;
 * - every acknowledgement goes through the window's controller (`tokenController.ts`, D-69 item 1), which outlives this
 *   component: a painted delivery is acknowledged only after its replacement has been painted (Hide, leaving the page,
 *   closing Preferences), a lost acknowledgement is sent again, and the field's presence and place are reported for
 *   the host's overpaint of captures;
 * - the token is fetched from the host only for a reveal or a Copy and held only while shown; it is never a registry
 *   command's answer, so neither the activity log, its export nor the event stream can carry it, and the page logs
 *   nothing.
 */
function AgentToken({ services }: { services: ShellServices }) {
  const [shown, setShown] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  /** The reveal generation: an answer is painted only if no hide happened since it was asked for. */
  const generation = useRef(0);
  /** A reveal is waiting for the host's answer. */
  const inFlight = useRef(false);
  /** The same, for the Show button: disabled while a reveal is in flight (D-67, C44A-R1-41). */
  const [pending, setPending] = useState(false);
  /** The delivery on screen now, if any. */
  const painted = useRef<number | null>(null);
  const field = useRef<HTMLInputElement | null>(null);
  const engine = services.engine;
  const controller = useMemo(() => tokenController(engine), [engine]);
  /** Ends the current reveal generation and masks the field; the host hears the painted delivery hidden once the mask
   *  is painted (two frames), so a capture waiting for it never sees the token. */
  const hide = useCallback(() => {
    generation.current += 1;
    const delivery = painted.current;
    if (delivery === null) return;
    painted.current = null;
    setShown(null);
    controller.hiddenAfterPaint(delivery);
  }, [controller]);
  useEffect(() => {
    controller.fieldMounted(() => field.current);
    let off: (() => void) | null = null;
    let offEvents: (() => void) | null = null;
    let stopped = false;
    // A window capture is about to run.
    void engine.onTokenHide(hide).then((u) => {
      if (stopped) u();
      else off = u;
    });
    // A regenerated token: the shown one is stale.
    void engine
      .onEvent((e) => {
        if (e.name === 'shell.agentToken') hide();
      })
      .then((u) => {
        if (stopped) u();
        else offEvents = u;
      });
    window.addEventListener('blur', hide);
    window.addEventListener('pagehide', hide);
    return () => {
      stopped = true;
      off?.();
      offEvents?.();
      window.removeEventListener('blur', hide);
      window.removeEventListener('pagehide', hide);
      // Leaving the page (another category, the dialog closed): an answer still in flight belongs to an ended
      // generation, and the delivery on screen is acknowledged by the window's controller once the page without the
      // field has been painted (D-69 item 1b), not by this unmounting component.
      generation.current += 1;
      const delivery = painted.current;
      painted.current = null;
      if (delivery !== null) controller.hiddenAfterPaint(delivery);
      controller.fieldUnmounted();
    };
  }, [engine, hide, controller]);
  const reveal = async () => {
    if (inFlight.current || painted.current !== null) return;
    setProblem(null);
    inFlight.current = true;
    setPending(true);
    const asked = generation.current;
    const answer = await engine.agentToken().finally(() => {
      inFlight.current = false;
      setPending(false);
    });
    if (answer === null) {
      setProblem('The token cannot be shown now (the window is being captured). Try again.');
      return;
    }
    if (asked !== generation.current) {
      // A late answer of an ended reveal: dropped without painting (C44A-R1-41).
      controller.hiddenNow(answer.delivery);
      return;
    }
    painted.current = answer.delivery;
    setShown(answer.token);
  };
  const copy = async () => {
    setProblem(null);
    const answer = await engine.agentToken();
    if (answer === null) {
      setProblem('The token cannot be copied now (the window is being captured). Try again.');
      return;
    }
    // Copied to the OS clipboard, never painted: the delivery is acknowledged hidden at once.
    try {
      await copyText(answer.token);
    } finally {
      controller.hiddenNow(answer.delivery);
    }
    setCopied(true);
  };
  return (
    <div className="agent-token" data-tt-role="group" data-tt-id="agentAccess.tokenGroup">
      <label>
        Token{' '}
        <input
          ref={field}
          readOnly
          className="agent-token-value"
          value={shown ?? MASK}
          aria-label="Agent token"
          data-tt-role="textField"
          data-tt-id="agentAccess.token"
          // The window tree reports the field redacted, revealed or not (D-62 item 2).
          data-tt-value="redacted"
          data-tt-gui-only="focus"
        />
      </label>
      <button
        type="button"
        aria-pressed={shown !== null}
        // One reveal in flight at a time: a second press cannot start another (D-67).
        disabled={pending}
        data-tt-role="button"
        data-tt-id="agentAccess.token.show"
        data-tt-gui-only="disclose"
        onClick={() => {
          if (painted.current !== null) hide();
          else void reveal();
        }}
      >
        {shown !== null ? 'Hide' : 'Show'}
      </button>
      <button
        type="button"
        data-tt-role="button"
        data-tt-id="agentAccess.token.copy"
        data-tt-gui-only="clipboard"
        onClick={() => {
          void copy();
        }}
      >
        Copy
      </button>
      {/* The notes have a line of their own, always present: a note appearing (a reveal refused during a capture)
          never moves the field, so what a capture shows at the field's place stays the field (D-67). */}
      <div className="agent-token-notes">
        {copied && <small className="field-hint">Copied to the clipboard.</small>}
        {problem && (
          <small className="field-problem" role="status">
            {problem}
          </small>
        )}
      </div>
    </div>
  );
}

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
  // The saved `agent.server.enabled` (what the check box was opened with), to say why the server is on (D-62 item 1).
  const [saved, setSaved] = useState<boolean | null>(null);
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
      void services.engine
        .run('prefs.get', { keys: ['agent.server.enabled'] })
        .done.then((env) => {
          const values = (env.result as { values?: Json } | null)?.values ?? {};
          if (!stopped && env.status === 'ok') setSaved(values['agent.server.enabled'] === true);
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
        {endpoint && endpoint !== 'stdio'
          ? `The agent server is running at ${endpoint}`
          : status['enabled'] === true
            ? 'The agent server is running'
            : 'The agent server is off'}{' '}
        · {sessions} session{sessions === 1 ? '' : 's'}
        {/* D-62 item 1: why it is on. The check box shows the saved setting, which a launch option does not change. */}
        {endpoint && endpoint !== 'stdio' && saved === false
          ? ' · started for this run by the --mcp-http launch option; the saved setting below is off'
          : endpoint && endpoint !== 'stdio' && saved === true
            ? ' · on in the saved setting below'
            : ''}
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
        <NumericField
          id="agentAccess.port"
          label="Port"
          role="field"
          value={server.port}
          range={{ min: 1024, max: 65535, integer: true, ...declaredRange('agent.server.set', 'port') }}
          disabled={!server.enabled}
          command="agent.server.set"
          onCommit={(v) => {
            setServer({ port: v });
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
        const shown = typeof raw === 'number' ? raw / (l.scale ?? 1) : l.min;
        return (
          <label key={l.key}>
            {l.label}{' '}
            <NumericField
              id={`agentAccess.${l.key}`}
              label={l.label}
              role="field"
              value={shown}
              range={limitRange(l)}
              command="agent.policy.defaults.set"
              onCommit={(v) => {
                setPolicy(l.key, Math.round(v * (l.scale ?? 1)));
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
        <AgentToken services={services} />
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
