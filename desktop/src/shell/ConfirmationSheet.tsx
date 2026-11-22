// The confirmation sheet (11 §6.3 step 2, §6.4; M4 C4.4a, design note 2 §4; ruling D-58). In attached mode with
// `askInApp` on, an agent's confirmation is asked here only and the agent's call waits for the answer. The sheet is
// not modal, is not a modal state (`userBusy`) and never takes the keyboard focus when it appears. Decline is the
// default: Return or Escape with the focus inside the sheet declines, and Accept needs a click, or the focus moved to it
// and activated (D-49 item 2: a destructive confirmation never defaults to the destructive choice; on macOS too).
// Several requests queue ("1 of 3"). A request decided in another window, expired, or overtaken by a document change
// leaves through the host-only `shell.agentConfirmation` notice or a re-read of `agent.server.status` (D-57), and the
// sheet says why when it ended unanswered.
import { useCallback, useEffect, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { useShell } from '../state/context';

type Json = Record<string, unknown>;

interface Request {
  readonly confirmationId: string;
  readonly sessionId: string;
  readonly commandId: string;
  readonly summary: string;
  readonly consequences: readonly unknown[];
  /** When the token expires (local clock), from `expiresInMs`. */
  readonly expiresAt: number;
}

const text = (v: unknown, fallback = ''): string => (typeof v === 'string' ? v : fallback);

const label = (id: string) =>
  (COMMAND_INDEX as Record<string, { label?: string } | undefined>)[id]?.label ?? id;

function consequenceText(c: unknown): string {
  if (typeof c === 'string') return c;
  if (typeof c !== 'object' || c === null) return String(c);
  const o = c as Json;
  return (
    text(o['description']) ||
    text(o['summary']) ||
    Object.entries(o)
      .map(([k, v]) => `${k}: ${typeof v === 'string' ? v : JSON.stringify(v)}`)
      .join(', ')
  );
}

export function ConfirmationSheet() {
  const { engine } = useShell();
  const [requests, setRequests] = useState<readonly Request[]>([]);
  const [clients, setClients] = useState<Readonly<Record<string, string>>>({});
  const [ended, setEnded] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  const read = useCallback(() => {
    void engine
      .run('agent.server.status', {})
      .done.then((env) => {
        if (env.status !== 'ok' || typeof env.result !== 'object' || env.result === null) return;
        const r = env.result as Json;
        const pending = Array.isArray(r['pendingConfirmations']) ? (r['pendingConfirmations'] as Json[]) : [];
        // D-63 item 2 (C44A-R1-61): the time left is the kernel's `expiresInMs` at the moment of its answer; the clock
        // the countdown measures against is read at that same moment, never a value from before the request arrived.
        const at = Date.now();
        setNow(at);
        setRequests(
          pending
            .filter((p) => p['inApp'] === true && p['decision'] == null)
            .map((p) => ({
              confirmationId: text(p['confirmationId']),
              sessionId: text(p['sessionId']),
              commandId: text(p['commandId']),
              summary: text(p['summary']),
              consequences: Array.isArray(p['consequences']) ? (p['consequences'] as unknown[]) : [],
              expiresAt: at + (typeof p['expiresInMs'] === 'number' ? p['expiresInMs'] : 0),
            })),
        );
        const sessions = Array.isArray(r['sessions']) ? (r['sessions'] as Json[]) : [];
        setClients(Object.fromEntries(sessions.map((s) => [text(s['sessionId']), text(s['client'])])));
      })
      .catch(() => undefined);
  }, [engine]);

  useEffect(() => {
    read();
    let stopped = false;
    let off: (() => void) | null = null;
    void engine
      .onEvent((e) => {
        if (e.name === 'shell.agentConfirmation') {
          const p = (e.payload ?? {}) as Json;
          if (p['outcome'] === 'expired')
            setEnded(`The request expired: ${text(p['reason'], 'it is no longer valid')}.`);
          read();
        }
        if (
          e.name === 'agent.confirmation.requested' ||
          e.name === 'agent.session.disconnected' ||
          e.name === 'project.opened' ||
          e.name === 'project.closed'
        )
          read();
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

  // The countdown, and a request whose token expired leaves (read again: the kernel no longer lists it).
  const first = requests[0];
  useEffect(() => {
    if (!first) return undefined;
    const t = setInterval(() => {
      const at = Date.now();
      setNow(at);
      if (at >= first.expiresAt) {
        setEnded('The request expired: it was not answered in time.');
        read();
      }
    }, 1000);
    return () => {
      clearInterval(t);
    };
  }, [first, read]);

  if (!first) {
    if (!ended) return null;
    return (
      <div className="confirm-sheet" role="status" data-tt-role="text" data-tt-id="confirmSheet.ended">
        {ended}{' '}
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="confirmSheet.dismissEnded"
          data-tt-gui-only="dismiss"
          onClick={() => {
            setEnded(null);
          }}
        >
          OK
        </button>
      </div>
    );
  }
  const respond = (accept: boolean) => {
    setEnded(null);
    void engine
      .run('agent.confirmation.respond', { confirmationId: first.confirmationId, accept })
      .done.then(read, read);
  };
  // Whole seconds left, truncated as the kernel's own count is (299 999 ms left shows 4:59).
  const left = Math.max(0, Math.floor((first.expiresAt - now) / 1000));
  return (
    <div
      className="confirm-sheet"
      role="dialog"
      aria-modal="false"
      aria-label="Agent confirmation"
      data-tt-role="dialog"
      data-tt-id="confirmSheet"
      data-tt-surface="confirmSheet"
      onKeyDown={(e) => {
        // With the focus inside the sheet: Escape declines, and Return declines unless the focus is on Accept.
        const onAccept = (e.target as HTMLElement).dataset['ttId'] === 'confirmSheet.accept';
        if (e.key === 'Escape' || (e.key === 'Enter' && !onAccept)) {
          e.preventDefault();
          respond(false);
        }
      }}
    >
      <strong>
        {(clients[first.sessionId] ?? '') || 'An agent'} asks to {label(first.commandId)}
      </strong>
      {requests.length > 1 && (
        <span className="field-hint" data-tt-role="text" data-tt-id="confirmSheet.queue">
          {' '}
          1 of {requests.length}
        </span>
      )}
      <p data-tt-role="text" data-tt-id="confirmSheet.summary">
        {first.summary}
      </p>
      {first.consequences.length > 0 && (
        <ul data-tt-role="list" data-tt-id="confirmSheet.consequences">
          {first.consequences.map((c, i) => (
            <li key={String(i)}>{consequenceText(c)}</li>
          ))}
        </ul>
      )}
      <p className="field-hint" data-tt-role="text" data-tt-id="confirmSheet.countdown">
        Expires in {Math.floor(left / 60)}:{String(left % 60).padStart(2, '0')}
      </p>
      <div className="buttons">
        <button
          type="button"
          className="default"
          data-tt-role="button"
          data-tt-id="confirmSheet.decline"
          data-tt-command="agent.confirmation.respond"
          data-tt-params={JSON.stringify({ confirmationId: first.confirmationId, accept: false })}
          onClick={() => {
            respond(false);
          }}
        >
          Decline
        </button>
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="confirmSheet.accept"
          data-tt-command="agent.confirmation.respond"
          data-tt-params={JSON.stringify({ confirmationId: first.confirmationId, accept: true })}
          onClick={() => {
            respond(true);
          }}
        >
          Accept
        </button>
      </div>
    </div>
  );
}
