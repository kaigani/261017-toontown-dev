// The status bar (01 §9.1; M4 C4.1): description, context, pointer and selection, playback, and the agent indicator
// (01 §12.1 rule 9, 11 §6.4) with Pause, Stop and Activity. The segment texts come from `ui.statusBar.get`, so the
// GUI and an agent read the same thing; the left end shows the last status line.
import { useEffect, useState } from 'react';

import { useShell, useSlice, useStoreState } from '../state/context';
import type { AgentServerStatus, StatusBarTexts } from '../state/types';
import { useActions } from './actionsApi';
import type { StatusLine } from './messagesApi';

export function StatusBar() {
  const texts = useSlice('statusBar') as StatusBarTexts | undefined;
  const [line, setLine] = useState<StatusLine | null>(null);
  useEffect(() => {
    const on = (e: Event) => {
      setLine((e as CustomEvent<StatusLine | null>).detail);
    };
    window.addEventListener('tt-status-line', on);
    return () => {
      window.removeEventListener('tt-status-line', on);
    };
  }, []);
  if (texts && !texts.visible) return null;
  return (
    <footer className="status-bar" role="status" data-tt-role="statusBar" data-tt-id="statusBar">
      <span
        className={`status-message status-${line?.kind ?? 'info'}`}
        data-tt-role="text"
        data-tt-id="status.line"
      >
        {line?.text ?? ''}
      </span>
      <span className="status-description" data-tt-role="text" data-tt-id="status.description">
        {texts?.description ?? ''}
      </span>
      <span className="status-context" data-tt-role="text" data-tt-id="status.context">
        {texts?.context ?? ''}
      </span>
      <span className="status-pointer" data-tt-role="text" data-tt-id="status.pointer">
        {texts?.pointer ?? ''}
      </span>
      <span className="status-playback" data-tt-role="text" data-tt-id="status.playback">
        {texts?.playback ?? ''}
      </span>
      <AgentIndicator />
    </footer>
  );
}

/** Persistent while an agent is connected; a distinct acting state; Pause/Resume, Stop, Activity (11 §6.4). */
export function AgentIndicator() {
  const status = useSlice('agents') as AgentServerStatus | undefined;
  const { acting } = useStoreState();
  const actions = useActions();
  const { engine } = useShell();
  const [open, setOpen] = useState(false);
  const [activity, setActivity] = useState<readonly Record<string, unknown>[]>([]);
  const sessions = status?.sessions ?? [];
  if (sessions.length === 0) {
    return (
      <span
        className="agent-indicator idle"
        data-tt-role="agentIndicator"
        data-tt-id="agent"
        data-tt-value="none"
      >
        {status?.enabled ? 'Agent server on' : ''}
      </span>
    );
  }
  const busy = acting.length > 0;
  const names = sessions.map((s) => s.client).join(', ');
  return (
    <span
      className={busy ? 'agent-indicator acting' : 'agent-indicator connected'}
      data-tt-role="agentIndicator"
      data-tt-id="agent"
      data-tt-value={busy ? 'acting' : 'connected'}
      data-tt-label={names}
    >
      <button
        type="button"
        aria-expanded={open}
        data-tt-role="button"
        data-tt-id="agent.toggle"
        onClick={() => {
          setOpen(!open);
          if (!open) {
            void engine.run('agent.activity.list', { limit: 20 }).done.then((env) => {
              setActivity(Array.isArray(env.result) ? (env.result as Record<string, unknown>[]) : []);
            });
          }
        }}
      >
        {busy
          ? `Agent acting: ${acting.map((a) => `${a.client} (${a.commandId})`).join(', ')}`
          : `Agent: ${names}`}
      </button>
      {open && (
        <span
          className="agent-popover"
          role="dialog"
          aria-label="Agent sessions"
          data-tt-role="popover"
          data-tt-id="agent.popover"
        >
          {sessions.map((s) => (
            <span
              key={s.sessionId}
              className="agent-session"
              data-tt-role="group"
              data-tt-id={`agent.${s.sessionId}`}
            >
              <strong>{s.client}</strong> ({s.policy})
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`agent.${s.sessionId}.pause`}
                data-tt-command="agent.session.pause"
                onClick={() => {
                  void actions.run('agent.session.pause', { sessionId: s.sessionId });
                }}
              >
                Pause
              </button>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`agent.${s.sessionId}.resume`}
                data-tt-command="agent.session.resume"
                onClick={() => {
                  void actions.run('agent.session.resume', { sessionId: s.sessionId });
                }}
              >
                Resume
              </button>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id={`agent.${s.sessionId}.stop`}
                data-tt-command="agent.session.disconnect"
                onClick={() => {
                  void actions.run('agent.session.disconnect', { sessionId: s.sessionId });
                }}
              >
                Stop
              </button>
            </span>
          ))}
          <span className="agent-activity" data-tt-role="list" data-tt-id="agent.activity">
            <strong>Activity</strong>
            {activity.map((a) => (
              <span key={String(a['seq'])} className="activity-entry" data-tt-role="text">
                {typeof a['commandId'] === 'string' ? a['commandId'] : ''} —{' '}
                {typeof a['status'] === 'string' ? a['status'] : ''}
              </span>
            ))}
          </span>
        </span>
      )}
    </span>
  );
}
