// The status bar (01 §9.1; M4 C4.1): description, context, pointer and selection, playback, and the agent indicator
// (01 §12.1 rule 9, 11 §6.4) with Pause, Stop and Activity. The segment texts come from `ui.statusBar.get`, so the
// GUI and an agent read the same thing; the left end shows the last status line.
import { useEffect, useRef, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { commandLabel } from './commandLabel';
import { useShell, useSlice, useStoreState } from '../state/context';
import type { AgentServerStatus, StatusBarTexts } from '../state/types';
import { useActions } from './actionsApi';
import type { StatusLine } from './messagesApi';

export function StatusBar() {
  const texts = useSlice('statusBar') as StatusBarTexts | undefined;
  const [line, setLine] = useState<StatusLine | null>(null);
  // A highlighted native menu item's hint (Open Recent's full path, 01 §3.3 line 190): shown instead of the status
  // line while it is set; the status line is back when it clears.
  const [hint, setHint] = useState<string | null>(null);
  useEffect(() => {
    const on = (e: Event) => {
      setLine((e as CustomEvent<StatusLine | null>).detail);
    };
    const onHint = (e: Event) => {
      const t = (e as CustomEvent<string | null>).detail;
      setHint(typeof t === 'string' && t !== '' ? t : null);
    };
    window.addEventListener('tt-status-line', on);
    window.addEventListener('tt-menu-hint', onHint);
    return () => {
      window.removeEventListener('tt-status-line', on);
      window.removeEventListener('tt-menu-hint', onHint);
    };
  }, []);
  if (texts && !texts.visible) return null;
  return (
    <footer className="status-bar" role="status" data-tt-role="statusBar" data-tt-id="statusBar">
      <span
        className={`status-message status-${hint !== null ? 'info' : (line?.kind ?? 'info')}`}
        data-tt-role="text"
        data-tt-id="status.line"
      >
        {hint ?? line?.text ?? ''}
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
      <JobsIndicator />
      <AgentIndicator />
    </footer>
  );
}

/** A job as `job.list` answers it (11 §15.2). */
interface JobRow {
  jobId: string;
  commandId: string;
  state: string;
  progress: number;
  message: string;
  session: string;
  cancellable?: boolean;
}

const isRunning = (j: JobRow) => j.state === 'running' || j.state === 'queued';

/** The progress area (01 §9.1; ruling D-30, group 1): the running jobs of every session (`job.list {all: true}`, the
 *  GUI's own listing, 11 §15.2), read again when a job reports progress or finishes. C4.3b round 1 (D-39): the answer
 *  is the array of jobs, and without `all` only the window's own session's jobs are listed, so an agent's job showed
 *  nothing. C43B-R2-06 (01 §9.5 lines 925-927): the segment opens the progress window, which lists every running job
 *  with its title, step and fraction, and a Cancel (`job.cancel {jobId}`) where the job is still cancellable, an agent
 *  session's jobs included (the user's priority, 11 §6.4; SI-160). The window opens by itself when a job of another
 *  session starts (the window's own jobs have their progress dialog) and closes when no job runs. */
function JobsIndicator() {
  const { engine } = useShell();
  const actions = useActions();
  const agents = useSlice('agents') as AgentServerStatus | undefined;
  const [jobs, setJobs] = useState<readonly JobRow[]>([]);
  const [open, setOpen] = useState(false);
  const seen = useRef(new Set<string>());
  const running = jobs.filter(isRunning);
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    const read = () => {
      void engine
        .run('job.list', { all: true })
        .done.then((env) => {
          const answer: unknown = env.status === 'ok' ? env.result : null;
          const list = (Array.isArray(answer) ? answer : []) as JobRow[];
          if (stopped) return;
          setJobs(list);
          // A running job of another session not seen before opens the window (01 §9.5: long operations show it).
          const fresh = list.filter((j) => isRunning(j) && !seen.current.has(j.jobId));
          for (const j of fresh) seen.current.add(j.jobId);
          if (fresh.some((j) => j.session.startsWith('agent:'))) setOpen(true);
          if (!list.some(isRunning)) setOpen(false);
        })
        .catch(() => undefined);
    };
    read();
    void engine
      .onEvent((e) => {
        if (e.name === 'job.progress' || e.name === 'job.finished') read();
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
  const count = running.length;
  const who = (session: string) => {
    if (!session.startsWith('agent:')) return 'You';
    const id = session.slice('agent:'.length);
    const s = agents?.sessions.find((x) => x.sessionId === id);
    return `Agent: ${s?.client ?? id}`;
  };
  return (
    <span className="status-jobs-area">
      <button
        type="button"
        className="status-jobs"
        disabled={count === 0}
        aria-expanded={open && count > 0}
        aria-hidden={count > 0 ? undefined : true}
        data-tt-role="button"
        data-tt-id="status.jobs"
        data-tt-gui-only="disclose"
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        {count > 0 ? `${String(count)} job${count === 1 ? '' : 's'} running` : ''}
      </button>
      {open && count > 0 && (
        <span
          className="jobs-window"
          role="dialog"
          aria-modal="false"
          aria-label="Jobs"
          data-tt-role="dialog"
          data-tt-id="jobsWindow"
          data-tt-surface="jobsWindow"
          onKeyDown={(e) => {
            if (e.key === 'Escape') setOpen(false);
          }}
        >
          <strong>Jobs</strong>
          {running.map((j) => {
            const title = (COMMAND_INDEX as Record<string, { label?: string } | undefined>)[j.commandId]
              ?.label;
            const percent = Math.round(Math.min(Math.max(j.progress, 0), 1) * 100);
            return (
              <span
                key={j.jobId}
                className="jobs-row"
                data-tt-role="group"
                data-tt-id={`jobsWindow.job.${j.jobId}`}
              >
                <span className="jobs-title" data-tt-role="text" data-tt-id={`jobsWindow.title.${j.jobId}`}>
                  {title ?? j.commandId} ({who(j.session)})
                </span>
                <span className="jobs-step" data-tt-role="text" data-tt-id={`jobsWindow.step.${j.jobId}`}>
                  {j.message || (j.state === 'queued' ? 'Waiting' : '')}
                </span>
                <progress
                  max={100}
                  value={percent}
                  aria-label={`${title ?? j.commandId} progress`}
                  data-tt-role="progress"
                  data-tt-id={`jobsWindow.progress.${j.jobId}`}
                  data-tt-value={String(percent)}
                />
                {j.cancellable !== false && (
                  <button
                    type="button"
                    data-tt-role="button"
                    data-tt-id={`jobsWindow.cancel.${j.jobId}`}
                    data-tt-command="job.cancel"
                    data-tt-params={JSON.stringify({ jobId: j.jobId })}
                    onClick={() => {
                      void actions.run('job.cancel', { jobId: j.jobId });
                    }}
                  >
                    Cancel
                  </button>
                )}
              </span>
            );
          })}
          <button
            type="button"
            data-tt-role="button"
            data-tt-id="jobsWindow.close"
            data-tt-gui-only="dismiss"
            onClick={() => {
              setOpen(false);
            }}
          >
            Close
          </button>
        </span>
      )}
    </span>
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
        data-tt-gui-only="disclose"
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
          ? `Agent acting: ${acting.map((a) => `${a.client} (${commandLabel(a.commandId, a)})`).join(', ')}`
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
            <button
              type="button"
              data-tt-role="button"
              data-tt-id="agent.openActivity"
              data-tt-command="window.panel.set"
              data-tt-params={JSON.stringify({ panelId: 'activity', visible: true, focus: true })}
              onClick={() => {
                setOpen(false);
                void actions.run('window.panel.set', { panelId: 'activity', visible: true, focus: true });
              }}
            >
              Activity…
            </button>
            {activity.map((a) => (
              <span key={String(a['seq'])} className="activity-entry" data-tt-role="text">
                {typeof a['commandId'] === 'string' ? commandLabel(a['commandId'], a['params']) : ''} —{' '}
                {typeof a['status'] === 'string' ? a['status'] : ''}
              </span>
            ))}
          </span>
        </span>
      )}
    </span>
  );
}
