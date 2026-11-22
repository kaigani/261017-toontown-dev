// The load report window (10 §7.3; M4 C4.4a, design note 2 §7; D-40 answer 5): after every open whose report has a
// warning or an error, a repair, a migration or a read-only reason, one non-modal window summarises it (no issue is a
// separate message). It is shown whoever opened the project (an agent's open in attached mode tells the person too),
// once per `project.opened`, and it is not a modal state: the work and the agents go on. Its read is
// `doc.loadReport.get`; Copy details writes the report as text (`clipboard`); Close changes nothing (`dismiss`).
import { useEffect, useMemo, useState } from 'react';

import { useShell } from '../state/context';
import { copyText } from './clipboard';
import { asReport, reportText, summaryOf, worthShowing, type Issue, type LoadReport } from './loadReport';

export function LoadReportWindow() {
  const { engine } = useShell();
  const [report, setReport] = useState<LoadReport | null>(null);
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    let stopped = false;
    let off: (() => void) | null = null;
    void engine
      .onEvent((e) => {
        if (e.name === 'project.closed') setReport(null);
        if (e.name !== 'project.opened') return;
        // Once per open: the event that announced it, never a re-render.
        void engine
          .run('doc.loadReport.get', {})
          .done.then((env) => {
            const r = env.status === 'ok' ? asReport(env.result) : null;
            if (!stopped) {
              setOpen(new Set());
              setReport(r && worthShowing(r) ? r : null);
            }
          })
          .catch(() => undefined);
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
  // The issues grouped by code and severity, the worst first.
  const groups = useMemo(() => {
    const rank: Record<string, number> = { error: 0, warning: 1, info: 2 };
    const m = new Map<string, Issue[]>();
    for (const i of report?.issues ?? []) {
      const k = `${i.severity}\u0000${i.code}`;
      m.set(k, [...(m.get(k) ?? []), i]);
    }
    return [...m.entries()]
      .map(([k, list]) => {
        const [severity = '', code = ''] = k.split('\u0000');
        return { key: k, severity, code, list };
      })
      .sort((a, b) => (rank[a.severity] ?? 3) - (rank[b.severity] ?? 3) || a.code.localeCompare(b.code));
  }, [report]);
  if (!report) return null;
  const close = () => {
    setReport(null);
  };
  return (
    <div
      className="load-report"
      role="dialog"
      aria-modal="false"
      aria-label="Load report"
      data-tt-role="dialog"
      data-tt-id="loadReport"
      data-tt-surface="loadReport"
      onKeyDown={(e) => {
        if (e.key === 'Escape') close();
      }}
    >
      <strong>Load report</strong>
      <p data-tt-role="text" data-tt-id="loadReport.summary">
        {summaryOf(report)}
      </p>
      <ul className="load-report-groups">
        {groups.map((g) => {
          const shown = open.has(g.key);
          return (
            <li key={g.key}>
              <button
                type="button"
                aria-expanded={shown}
                data-tt-role="button"
                data-tt-id={`loadReport.group.${g.severity}.${g.code}`}
                data-tt-gui-only="disclose"
                onClick={() => {
                  setOpen((s) => {
                    const n = new Set(s);
                    if (n.has(g.key)) n.delete(g.key);
                    else n.add(g.key);
                    return n;
                  });
                }}
              >
                {g.severity} · {g.code} ({g.list.length})
              </button>
              {shown && (
                <ul>
                  {g.list.map((i) => (
                    <li key={i.issueId} data-tt-role="text" data-tt-id={`loadReport.issue.${i.issueId}`}>
                      {i.path ?? i.entry ?? i.objectId ?? ''} {i.message}
                    </li>
                  ))}
                </ul>
              )}
            </li>
          );
        })}
      </ul>
      <div className="buttons">
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="loadReport.copy"
          data-tt-gui-only="clipboard"
          onClick={() => {
            void copyText(reportText(report)).catch(() => undefined);
          }}
        >
          Copy details
        </button>
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="loadReport.close"
          data-tt-gui-only="dismiss"
          onClick={close}
        >
          Close
        </button>
      </div>
    </div>
  );
}
