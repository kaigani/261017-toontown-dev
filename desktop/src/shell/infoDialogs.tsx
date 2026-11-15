// The result dialogs of the File and Help menus (M4 C4.3b; ruling D-30, group 1): About (`app.info.get`), Project
// Info (`project.info.get`), Check Project (`doc.validate`, with `doc.repair`), Open Backup (`project.backup.list`,
// with `project.backup.open`). Each reads its query when it opens and shows the answer; its buttons run commands
// through the one entry point (`services.run`), so the parity manifest binds each to its command.

import { useEffect, useState } from 'react';

import type { ShellServices } from '../state/context';
import { Dialog } from './dialogs';

type Loaded =
  | { status: 'loading' }
  | { status: 'ok'; result: Record<string, unknown> }
  | { status: 'error'; message: string };

function useQuery(
  services: ShellServices,
  command: string,
  params: Record<string, unknown>,
  refresh = 0,
): Loaded {
  const [state, setState] = useState<Loaded>({ status: 'loading' });
  const key = JSON.stringify(params);
  useEffect(() => {
    let live = true;
    void services.engine
      .run(command, JSON.parse(key) as Record<string, unknown>)
      .done.then((env) => {
        if (!live) return;
        if (env.status === 'error') setState({ status: 'error', message: env.error?.message ?? 'failed' });
        else setState({ status: 'ok', result: (env.result ?? {}) as Record<string, unknown> });
      })
      .catch((e: unknown) => {
        if (live) setState({ status: 'error', message: String(e) });
      });
    return () => {
      live = false;
    };
  }, [services, command, key, refresh]);
  return state;
}

/** A query's answer as a list of `name: value` rows (nested values as JSON). */
function Fields({ id, value }: { id: string; value: Record<string, unknown> }) {
  return (
    <dl className="info-fields" data-tt-role="list" data-tt-id={`${id}.fields`}>
      {Object.entries(value).map(([k, v]) => (
        <div key={k} className="info-field">
          <dt>{k}</dt>
          <dd>
            {typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean'
              ? String(v)
              : JSON.stringify(v)}
          </dd>
        </div>
      ))}
    </dl>
  );
}

function Body({ id, state }: { id: string; state: Loaded }) {
  if (state.status === 'loading') return <p>Loading…</p>;
  if (state.status === 'error')
    return (
      <p className="field-problem" role="alert">
        {state.message}
      </p>
    );
  return <Fields id={id} value={state.result} />;
}

/** Help › About Toontown (01 §5: the About window; `app.info.get`). */
export function AboutDialog({ services, close }: { services: ShellServices; close: () => void }) {
  const info = useQuery(services, 'app.info.get', {});
  return (
    <Dialog id="about" title="About Toontown" onCancel={close}>
      <Body id="about" state={info} />
    </Dialog>
  );
}

/** File › Project Info (`project.info.get`). */
export function ProjectInfoDialog({ services, close }: { services: ShellServices; close: () => void }) {
  const info = useQuery(services, 'project.info.get', {});
  return (
    <Dialog id="projectInfo" title="Project Info" onCancel={close} wide>
      <Body id="projectInfo" state={info} />
    </Dialog>
  );
}

/** File › Check Project (`doc.validate`), with Repair (`doc.repair {all: true}`). */
export function CheckProjectDialog({ services, close }: { services: ShellServices; close: () => void }) {
  const [round, setRound] = useState(0);
  const report = useQuery(services, 'doc.validate', {}, round);
  const issues =
    report.status === 'ok' && Array.isArray(report.result['issues']) ? report.result['issues'] : [];
  return (
    <Dialog id="checkProject" title="Check Project" onCancel={close} wide>
      {report.status === 'ok' ? (
        <ul className="info-list" data-tt-role="list" data-tt-id="checkProject.issues">
          {issues.length === 0 && <li>No problems found.</li>}
          {issues.map((i, n) => (
            <li key={n}>{JSON.stringify(i)}</li>
          ))}
        </ul>
      ) : (
        <Body id="checkProject" state={report} />
      )}
      <button
        type="button"
        disabled={issues.length === 0}
        data-tt-role="button"
        data-tt-id="checkProject.repair"
        data-tt-command="doc.repair"
        data-tt-params={JSON.stringify({ all: true })}
        onClick={() => {
          void services.run('doc.repair', { all: true }).then(() => {
            setRound((r) => r + 1);
          });
        }}
      >
        Repair
      </button>
    </Dialog>
  );
}

/** File › Open Backup (`project.backup.list`, then `project.backup.open {backupId}`). */
export function BackupsDialog({ services, close }: { services: ShellServices; close: () => void }) {
  const list = useQuery(services, 'project.backup.list', {});
  const backups = list.status === 'ok' && Array.isArray(list.result['backups']) ? list.result['backups'] : [];
  return (
    <Dialog id="openBackup" title="Open Backup" onCancel={close} wide>
      {list.status === 'ok' ? (
        <ul className="info-list" data-tt-role="list" data-tt-id="openBackup.list">
          {backups.length === 0 && <li>No backups of this project.</li>}
          {backups.map((b, n) => {
            const o = (b ?? {}) as Record<string, unknown>;
            const backupId = typeof o['backupId'] === 'string' ? o['backupId'] : String(n);
            return (
              <li key={backupId}>
                <span>{typeof o['createdAt'] === 'string' ? o['createdAt'] : backupId}</span>
                <button
                  type="button"
                  data-tt-role="button"
                  data-tt-id={`openBackup.open.${String(n)}`}
                  data-tt-command="project.backup.open"
                  data-tt-params={JSON.stringify({ backupId })}
                  onClick={() => {
                    void services.run('project.backup.open', { backupId }).then(close);
                  }}
                >
                  Open
                </button>
              </li>
            );
          })}
        </ul>
      ) : (
        <Body id="openBackup" state={list} />
      )}
    </Dialog>
  );
}
