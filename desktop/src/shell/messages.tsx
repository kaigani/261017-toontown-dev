// Messages (01 §9.4-§9.5, §12.1 rule 6; M4 C4.1): message boxes with the message id in a details area and specific
// verbs, "don't show again" for suppressible tips, non-modal notifications for agent-originated messages, a status
// line, and the modal-state reports the executor needs for user priority (ADR-0021).
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { isCancelled, type EngineApi } from '../ipc';
import {
  MessagesContext,
  NOTIFICATION_MS,
  OK,
  type MessageAnswer,
  type MessageSpec,
  type MessagesApi,
  type Notification,
  type StatusLine,
} from './messagesApi';
import { useModalFocus } from './modalFocus';

interface Pending {
  readonly spec: MessageSpec;
  readonly resolve: (a: MessageAnswer) => void;
}

export function MessageHost({
  engine,
  suppressed,
  children,
}: {
  engine: EngineApi;
  suppressed: ReadonlySet<string>;
  children: ReactNode;
}) {
  const [queue, setQueue] = useState<readonly Pending[]>([]);
  const [notes, setNotes] = useState<readonly Notification[]>([]);
  const [line, setLine] = useState<StatusLine | null>(null);
  const modals = useRef<{ kind: string; title: string }[]>([]);
  const nextKey = useRef(1);
  const suppressedRef = useRef(suppressed);
  useEffect(() => {
    suppressedRef.current = suppressed;
  }, [suppressed]);

  const reportModal = useCallback(() => {
    const top = modals.current[modals.current.length - 1];
    void engine
      .report({
        modal: top ? { kind: top.kind as 'dialog' | 'progress' | 'gesture', title: top.title } : null,
      })
      .catch(() => undefined);
  }, [engine]);

  const enterModal = useCallback(
    (kind: 'dialog' | 'progress' | 'gesture', title: string) => {
      const entry = { kind, title };
      modals.current.push(entry);
      reportModal();
      let closed = false;
      return () => {
        if (closed) return;
        closed = true;
        modals.current = modals.current.filter((m) => m !== entry);
        reportModal();
      };
    },
    [reportModal],
  );

  const api = useMemo<MessagesApi>(() => {
    const shown = (spec: MessageSpec, isSuppressed: boolean) => {
      void engine
        .report({
          messageShown: {
            messageId: spec.messageId,
            severity: spec.severity,
            text: spec.text,
            details: spec.details,
            suppressed: isSuppressed,
          },
        })
        .catch(() => undefined);
    };
    const show = (spec: MessageSpec): Promise<MessageAnswer> => {
      const buttons = spec.buttons ?? OK;
      if (spec.suppressible && suppressedRef.current.has(spec.messageId)) {
        // Suppressed: the default action is taken silently (01 §9.4).
        shown(spec, true);
        const def = buttons.find((b) => b.role === 'default') ?? buttons[0];
        return Promise.resolve({ button: def?.id ?? 'ok', suppress: true });
      }
      shown(spec, false);
      return new Promise((resolve) => {
        setQueue((q) => [...q, { spec, resolve }]);
      });
    };
    return {
      show,
      notify(spec) {
        shown(spec, false);
        const key = nextKey.current++;
        setNotes((n) => [...n, { ...spec, key }]);
        setTimeout(() => {
          setNotes((n) => n.filter((x) => x.key !== key));
        }, NOTIFICATION_MS);
      },
      status(text, kind = 'info') {
        setLine({ text, kind });
      },
      async failure(env, what) {
        if (env.status === 'ok') return;
        if (isCancelled(env)) {
          // A cancelled request is the user's choice, never shown as a timeout (SI-111).
          setLine({ text: `${what}: cancelled`, kind: 'info' });
          return;
        }
        const e = env.error;
        const details: Record<string, unknown> = { code: e?.code ?? 'error' };
        if (e?.details !== undefined) details['details'] = e.details;
        if (e?.hint) details['hint'] = e.hint;
        await show({
          severity: 'error',
          messageId: e?.code ?? 'internal',
          title: what,
          text: e?.message ?? 'The command failed.',
          details,
        });
      },
      enterModal,
    };
  }, [engine, enterModal]);

  const current = queue[0];
  useEffect(() => {
    if (!current) return undefined;
    return enterModal('dialog', current.spec.title ?? current.spec.messageId);
  }, [current, enterModal]);

  const answer = (button: string, suppress: boolean) => {
    if (!current) return;
    if (suppress && current.spec.suppressible) {
      void engine
        .run('ui.tips.set', { messageId: current.spec.messageId, suppressed: true })
        .done.catch(() => undefined);
    }
    current.resolve({ button, suppress });
    setQueue((q) => q.slice(1));
  };

  return (
    <MessagesContext.Provider value={api}>
      {children}
      <StatusLineSink line={line} />
      {current && <MessageBox spec={current.spec} onAnswer={answer} />}
      <div className="notifications" role="log" aria-label="Notifications" data-tt-role="notifications">
        {notes.map((n) => (
          <div
            key={n.key}
            className={`notification notification-${n.severity}`}
            data-tt-role="notification"
            data-tt-id={n.messageId}
          >
            <strong>{n.title ?? n.messageId}</strong> {n.text}
          </div>
        ))}
      </div>
    </MessagesContext.Provider>
  );
}

/** Publishes the status line to the status bar through a DOM-free channel (a custom event on window). */
function StatusLineSink({ line }: { line: StatusLine | null }) {
  useEffect(() => {
    window.dispatchEvent(new CustomEvent('tt-status-line', { detail: line }));
  }, [line]);
  return null;
}

function MessageBox({
  spec,
  onAnswer,
}: {
  spec: MessageSpec;
  onAnswer: (button: string, suppress: boolean) => void;
}) {
  const [suppress, setSuppress] = useState(false);
  const [open, setOpen] = useState(false);
  const buttons = spec.buttons ?? OK;
  const cancel =
    buttons.find((b) => b.role === 'cancel') ?? buttons.find((b) => b.role === 'default') ?? buttons[0];
  const ref = useRef<HTMLDivElement>(null);
  useModalFocus(ref);
  return (
    <div className="modal-backdrop">
      <div
        ref={ref}
        tabIndex={-1}
        className={`message-box message-${spec.severity}`}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="tt-message-title"
        data-tt-role="dialog"
        data-tt-id={`message.${spec.messageId}`}
        data-tt-label={spec.title ?? spec.messageId}
        onKeyDown={(e) => {
          if (e.key === 'Escape' && cancel) onAnswer(cancel.id, suppress);
        }}
      >
        <h2 id="tt-message-title">
          <span className="severity">{spec.severity}</span> {spec.title ?? 'Toontown'}
        </h2>
        <p className="message-text">{spec.text}</p>
        {spec.suppressible && (
          <label className="suppress">
            <input
              type="checkbox"
              checked={suppress}
              onChange={(e) => {
                setSuppress(e.target.checked);
              }}
            />{' '}
            Don&apos;t show this again
          </label>
        )}
        <details
          open={open}
          onToggle={(e) => {
            setOpen((e.target as HTMLDetailsElement).open);
          }}
        >
          <summary>Details</summary>
          <p>
            Message id: <code data-testid="message-id">{spec.messageId}</code>
          </p>
          {spec.details !== undefined && <pre>{JSON.stringify(spec.details, null, 2)}</pre>}
        </details>
        <div className="buttons">
          {buttons.map((b) => (
            <button
              key={b.id}
              type="button"
              className={b.role ? `button-${b.role}` : undefined}
              autoFocus={b.role === 'default'}
              data-tt-role="button"
              data-tt-id={`message.${b.id}`}
              onClick={() => {
                onAnswer(b.id, suppress);
              }}
            >
              {b.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
