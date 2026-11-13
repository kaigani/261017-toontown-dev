// The action router (M4 C4.1; ADR-0021): every GUI action (a menu item, a toolbar button, a shortcut, a dialog's
// OK) ends in a command invocation through the one engine entry point, with the parameters an agent would send
// (gesture equivalence, 11 §2). Commands that need parameters the user must give open a dialog first (01 §0);
// commands that would discard the open project apply the unsaved-changes rule (01 §3.5); jobs show a progress window
// with Cancel (`job.cancel`); a long action shows Cancel for its request (request cancellation, ADR-0020 decision 8),
// which never cancels a job.
import type { Envelope } from '../ipc';
import type { ShellServices } from '../state/context';
import type { AppState } from '../state/types';
import { BUSY_AFTER_MS, DISCARDING, isQuery, type ActionsApi, type Busy } from './actionsApi';
import { NamePrompt } from './dialogs';
import type { DialogsApi } from './dialogsApi';
import type { MessagesApi } from './messagesApi';
import { JobProgress, NewProjectDialog, ProjectPropertiesDialog } from './projectForms';

function jobIdOf(env: Envelope): string | null {
  const r = env.result as Record<string, unknown> | undefined;
  const id = r?.['jobId'];
  return typeof id === 'string' && id !== 'dryRun' ? id : null;
}

const PACKAGE_FILTERS = [
  { name: 'Project packages', extensions: ['animpkg'] },
  { name: 'All files', extensions: ['*'] },
];

export function makeActions(
  services: ShellServices,
  messages: MessagesApi,
  dialogs: DialogsApi,
  state: () => AppState | undefined,
  setBusy: (b: Busy | null) => void,
  onQuit?: () => void,
): ActionsApi {
  const run = async (command: string, params: Record<string, unknown> = {}): Promise<Envelope> => {
    const query = isQuery(command);
    const running = services.engine.run(command, params, { cancellable: !query });
    const busy = { shown: false };
    const timer = query
      ? null
      : setTimeout(() => {
          busy.shown = true;
          setBusy({ title: command, token: running.token });
        }, BUSY_AFTER_MS);
    const env = await running.done;
    if (timer) clearTimeout(timer);
    if (busy.shown) setBusy(null);
    if (env.status !== 'ok' && env.status !== 'needs_confirmation') await messages.failure(env, command);
    return env;
  };

  const waitJob = async (env: Envelope, title: string, cancellable: boolean): Promise<Envelope> => {
    const jobId = jobIdOf(env);
    if (!jobId || env.status !== 'ok') return env;
    const done = await dialogs.open<Envelope>(title, (close) => (
      <JobProgress services={services} jobId={jobId} title={title} cancellable={cancellable} close={close} />
    ));
    const final = done ?? env;
    if (final.status !== 'ok') await messages.failure(final, title);
    return final;
  };

  const pick = (kind: 'open' | 'save' | 'folder', title: string, filters = PACKAGE_FILTERS) =>
    services.engine.pickPath({ kind, title, filters }).catch(() => null);

  /** Save As with the native location choice; null when the user cancels the choice. */
  const saveAs = async (): Promise<Envelope | null> => {
    const path = await pick('save', 'Save As');
    if (!path) return null;
    let env = await run('project.saveAs', { path });
    if (env.status === 'error' && env.error?.code === 'project.exists') {
      const a = await messages.show({
        severity: 'question',
        messageId: 'project.exists',
        title: 'Save As',
        text: `"${path}" exists. Replace it?`,
        buttons: [
          { id: 'replace', label: 'Replace', role: 'destructive' },
          { id: 'cancel', label: 'Cancel', role: 'cancel' },
        ],
      });
      if (a.button !== 'replace') return null;
      env = await run('project.saveAs', { path, onExists: 'overwrite' });
    }
    return waitJob(env, 'Saving', false);
  };

  const save = async (): Promise<Envelope | null> => {
    if (!state()?.project?.path) return saveAs();
    const env = await run('project.save', {});
    if (env.status === 'error' && env.error?.code === 'project.readOnly') return saveAs();
    return waitJob(env, 'Saving', false);
  };

  /** The unsaved-changes rule (01 §3.5): `fail` when clean, otherwise Save / Don't Save / Cancel. */
  const unsaved = async (): Promise<'fail' | 'discard' | null> => {
    const p = state()?.project;
    if (!p || (!p.dirty && !p.globalLibraryDirty)) return 'fail';
    const a = await messages.show({
      severity: 'question',
      messageId: 'project.unsavedChanges',
      title: 'Unsaved changes',
      text: `Save the changes to "${p.name}"?`,
      buttons: [
        { id: 'save', label: 'Save', role: 'default' },
        { id: 'discard', label: "Don't Save", role: 'destructive' },
        { id: 'cancel', label: 'Cancel', role: 'cancel' },
      ],
    });
    if (a.button === 'cancel') return null;
    if (a.button === 'discard') return 'discard';
    const saved = await save();
    // A failed save or a cancelled location choice aborts the operation (01 §3.5).
    return saved?.status === 'ok' ? 'fail' : null;
  };

  const prompt = (id: string, title: string, label: string, initial: string, submitLabel: string) =>
    dialogs.open<string>(title, (close) => (
      <NamePrompt
        id={id}
        title={title}
        label={label}
        initial={initial}
        submitLabel={submitLabel}
        validate={(n) => (n.trim() ? null : 'A name cannot be empty.')}
        close={close}
      />
    ));

  const perform = async (command: string, params: Record<string, unknown> = {}): Promise<Envelope | null> => {
    const p: Record<string, unknown> = { ...params };
    if (DISCARDING.has(command) && p['onUnsaved'] === undefined) {
      const choice = await unsaved();
      if (choice === null) return null;
      if (choice === 'discard') p['onUnsaved'] = 'discard';
    }
    switch (command) {
      case 'project.create': {
        if (p['name'] === undefined) {
          const chosen = await dialogs.open<Record<string, unknown>>('New Project', (close) => (
            <NewProjectDialog services={services} close={close} />
          ));
          if (!chosen) return null;
          Object.assign(p, chosen);
        }
        return run('project.create', p);
      }
      case 'project.open': {
        if (p['path'] === undefined) {
          const path = await pick('open', 'Open Project');
          if (!path) return null;
          p['path'] = path;
        }
        return waitJob(await run('project.open', p), 'Opening project', true);
      }
      case 'project.recovery.open':
        return waitJob(await run(command, p), 'Recovering project', true);
      case 'project.save':
        return save();
      case 'project.saveAs':
        return p['path'] === undefined ? saveAs() : waitJob(await run(command, p), 'Saving', false);
      case 'app.quit': {
        const env = await run('app.quit', p);
        if (env.status === 'ok') onQuit?.();
        return env;
      }
      case 'workspace.create': {
        if (p['name'] === undefined) {
          const name = await prompt('workspaceNew', 'New Workspace', 'Name', 'User Workspace', 'Create');
          if (!name) return null;
          p['name'] = name;
        }
        return run(command, p);
      }
      case 'workspace.rename': {
        const current = state()?.workspace.name ?? '';
        if (p['name'] === undefined) p['name'] = current;
        if (p['newName'] === undefined) {
          const name = await prompt('workspaceRename', 'Rename Workspace', 'New name', current, 'Rename');
          if (!name) return null;
          p['newName'] = name;
        }
        return run(command, p);
      }
      case 'workspace.delete': {
        if (p['name'] === undefined) p['name'] = state()?.workspace.name ?? '';
        return run(command, p);
      }
      case 'workspace.export':
      case 'keymap.export': {
        if (command === 'workspace.export' && p['name'] === undefined)
          p['name'] = state()?.workspace.name ?? '';
        if (p['path'] === undefined) {
          const path = await pick(
            'save',
            command === 'keymap.export' ? 'Export Keymap' : 'Export Workspace',
            [{ name: 'JSON', extensions: ['json', 'ttworkspace'] }],
          );
          if (!path) return null;
          p['path'] = path;
          p['onExists'] = 'overwrite';
        }
        return run(command, p);
      }
      case 'workspace.import':
      case 'keymap.import': {
        if (p['path'] === undefined) {
          const path = await pick(
            'open',
            command === 'keymap.import' ? 'Import Keymap' : 'Import Workspace',
            [{ name: 'JSON', extensions: ['json', 'ttworkspace'] }],
          );
          if (!path) return null;
          p['path'] = path;
          if (command === 'workspace.import') p['onExists'] = 'rename';
        }
        return run(command, p);
      }
      case 'project.properties.set': {
        if (Object.keys(p).length === 0) {
          const chosen = await dialogs.open<Record<string, unknown>>('Project Properties', (close) => (
            <ProjectPropertiesDialog services={services} close={close} />
          ));
          if (!chosen) return null;
          Object.assign(p, chosen);
        }
        return run(command, p);
      }
      default:
        return run(command, p);
    }
  };

  return { perform, run };
}
