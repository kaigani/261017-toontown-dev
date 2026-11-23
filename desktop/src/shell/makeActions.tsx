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
import { AboutDialog, BackupsDialog, CheckProjectDialog, ProjectInfoDialog } from './infoDialogs';
import { JobProgress, NewProjectDialog, ProjectPropertiesDialog } from './projectForms';
import { ToolbarSheet } from './toolbarSheet';
import {
  ColoursDialog,
  NewElementsDialog,
  NumberDialog,
  TextDialog,
  TransferOptionsDialog,
} from '../timeline/dialogs';
import { parseTransferOptions, type TransferOptions } from '../timeline/model';
import type { CompletedRef } from '../timeline/dialogOpen';

/** The file a refusal names when a program outside the lock protocol keeps a lock file open (ruling D-86 item 1;
 *  SI-200: `fs.ioError`, `reason: "lockFileInUse"`), or null for any other answer. */
export function lockFileInUse(env: Envelope | null): string | null {
  const e = env?.status === 'error' ? env.error : undefined;
  const d = (e?.details ?? {}) as Record<string, unknown>;
  return e?.code === 'fs.ioError' && d['reason'] === 'lockFileInUse' && typeof d['path'] === 'string'
    ? d['path']
    : null;
}

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
  /** Runs a command; a failure is surfaced, except the refusals in `handled`, which the caller answers itself (D-44:
   *  Save As asks Replace / Cancel on `project.exists`, Save turns `project.readOnly` into Save As; surfacing them too
   *  showed an error box before the question). */
  const run = async (
    command: string,
    params: Record<string, unknown> = {},
    handled: readonly string[] = [],
  ): Promise<Envelope> => {
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
    const answered =
      (env.status === 'error' && env.error !== undefined && handled.includes(env.error.code)) ||
      // The open flows ask the person about a lock file in use themselves (D-86 item 1).
      lockFileInUse(env) !== null;
    if (env.status !== 'ok' && env.status !== 'needs_confirmation' && !answered)
      await messages.failure(env, command);
    return env;
  };

  const waitJob = async (env: Envelope, title: string, cancellable: boolean): Promise<Envelope> => {
    const jobId = jobIdOf(env);
    if (!jobId || env.status !== 'ok') return env;
    const done = await dialogs.open<Envelope>(title, (close) => (
      <JobProgress services={services} jobId={jobId} title={title} cancellable={cancellable} close={close} />
    ));
    const final = done ?? env;
    if (final.status !== 'ok') {
      if (lockFileInUse(final) === null) await messages.failure(final, title);
      return final;
    }
    // `job.wait` answers `ok` with the job's record: a job that failed is a failure the user is told about (01 §9.5),
    // with the job's own error; a cancelled job is the user's choice, shown on the status line (SI-111).
    const record = final.result as { state?: unknown; error?: Envelope['error'] } | undefined;
    if (record?.state === 'failed') {
      const failed: Envelope = {
        ...final,
        status: 'error',
        error: record.error ?? { code: 'job.failed', message: 'The job failed.' },
      };
      if (lockFileInUse(failed) === null) await messages.failure(failed, title);
      return failed;
    }
    if (record?.state === 'cancelled') messages.status(`${title}: cancelled`);
    return final;
  };

  const pick = (kind: 'open' | 'save' | 'folder', title: string, filters = PACKAGE_FILTERS) =>
    services.engine.pickPath({ kind, title, filters }).catch(() => null);

  /** Save As with the native location choice; null when the user cancels the choice. */
  const saveAs = async (): Promise<Envelope | null> => {
    const path = await pick('save', 'Save As');
    if (!path) return null;
    // Ruling D-49 item 1 (C43B-R3-01): one replacement decision per operation (01 §9.5). The native save picker asks
    // whether to replace an existing file itself on Windows and macOS (reviews/M4-design-dialogs.md §1), so its path
    // is saved over without a second question; the kernel's `project.exists` refusal stays for agents.
    const env = await run('project.saveAs', { path, onExists: 'overwrite' });
    return waitJob(env, 'Saving', false);
  };

  const save = async (): Promise<Envelope | null> => {
    if (!state()?.project?.path) return saveAs();
    const env = await run('project.save', {}, ['project.readOnly']);
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

  const prompt = (
    id: string,
    title: string,
    label: string,
    initial: string,
    submitLabel: string,
    command: string,
  ) =>
    dialogs.open<string>(title, (close) => (
      <NamePrompt
        id={id}
        title={title}
        label={label}
        initial={initial}
        submitLabel={submitLabel}
        command={command}
        validate={(n) => (n.trim() ? null : 'A name cannot be empty.')}
        close={close}
      />
    ));

  /** Runs an open flow; when it is refused because a program outside the lock protocol keeps a lock file open
   *  (D-86 item 1), the person is told which file and may try again (the project and its lock are unchanged). */
  const withLockRetry = async (attempt: () => Promise<Envelope | null>): Promise<Envelope | null> => {
    for (;;) {
      const env = await attempt();
      const path = lockFileInUse(env);
      if (path === null) return env;
      const a = await messages.show({
        severity: 'warning',
        messageId: 'project.lockFileInUse',
        title: 'Lock file in use',
        text:
          `Another program has the project's lock file open: ${path}. The project was not changed. ` +
          'Close that program, or wait for it to finish, then try again.',
        buttons: [
          { id: 'retry', label: 'Try Again', role: 'default' },
          { id: 'cancel', label: 'Cancel', role: 'cancel' },
        ],
      });
      if (a.button !== 'retry') return env;
    }
  };

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
        return withLockRetry(() => run('project.create', p));
      }
      case 'project.open': {
        if (p['path'] === undefined) {
          const path = await pick('open', 'Open Project');
          if (!path) return null;
          p['path'] = path;
        }
        return withLockRetry(async () => waitJob(await run('project.open', p), 'Opening project', true));
      }
      case 'project.recovery.open':
        return withLockRetry(async () => waitJob(await run(command, p), 'Recovering project', true));
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
          const name = await prompt(
            'workspaceNew',
            'New Workspace',
            'Name',
            'User Workspace',
            'Create',
            'workspace.create',
          );
          if (!name) return null;
          p['name'] = name;
        }
        return run(command, p);
      }
      case 'workspace.rename': {
        const current = state()?.workspace.name ?? '';
        if (p['name'] === undefined) p['name'] = current;
        if (p['newName'] === undefined) {
          const name = await prompt(
            'workspaceRename',
            'Rename Workspace',
            'New name',
            current,
            'Rename',
            'workspace.rename',
          );
          if (!name) return null;
          p['newName'] = name;
        }
        return run(command, p);
      }
      // The Element and Play menus' items that collect a value first (M5 C5.1b; view note section 7): the Timeline's
      // dialogs, for the current element where the item names none.
      case 'element.create': {
        if (p['type'] === undefined) {
          const ref = await dialogs.open<CompletedRef>('New Elements', (close) => (
            <NewElementsDialog onDone={close} />
          ));
          if (!ref) return null;
          Object.assign(p, ref.params);
        }
        return run(command, p);
      }
      case 'element.rename':
      case 'element.note.set': {
        const key = command === 'element.rename' ? 'name' : 'text';
        if (p[key] === undefined) {
          const el = state()?.element;
          const id = typeof p['elementId'] === 'string' ? p['elementId'] : el?.id;
          if (!id) return run(command, p);
          const rename = command === 'element.rename';
          const title = rename ? 'Rename Element' : 'Element Note';
          const render = (close: (t: string | null) => void) => (
            <TextDialog
              id={rename ? 'timelineRename' : 'timelineNote'}
              title={title}
              label={rename ? 'Name' : 'Note (empty removes it)'}
              initial={rename && el?.id === id ? el.name : ''}
              command={command}
              multiline={!rename}
              required={rename}
              onDone={close}
            />
          );
          // Each title written where the dialog opens (the D-44 inventory, scripts/c43_d44_coverage.py).
          const text = rename
            ? await dialogs.open<string>('Rename Element', render)
            : await dialogs.open<string>('Element Note', render);
          if (text === null) return null;
          p['elementId'] = id;
          p[key] = text;
        }
        return run(command, p);
      }
      case 'element.clone': {
        if (p['elementId'] === undefined) {
          const id = state()?.element?.id;
          if (id) p['elementId'] = id;
        }
        return run(command, p);
      }
      case 'element.colour.set': {
        if (!('marker' in p) && !('background' in p)) {
          const chosen = await dialogs.open<Record<string, unknown>>('Element Colours', (close) => (
            <ColoursDialog onDone={close} />
          ));
          if (!chosen) return null;
          Object.assign(p, chosen);
        }
        return run(command, p);
      }
      case 'app.frame.set':
      case 'timeline.currentFrame.set':
      case 'playback.range.set': {
        const field =
          command !== 'playback.range.set'
            ? p['frame'] === undefined
              ? 'frame'
              : null
            : 'start' in p && p['start'] === null
              ? 'start'
              : 'end' in p && p['end'] === null
                ? 'end'
                : null;
        if (field) {
          const title =
            field === 'frame'
              ? 'Change Current Frame'
              : field === 'start'
                ? 'Change Playback Start Frame'
                : 'Change Playback End Frame';
          const render = (close: (n: number | null) => void) => (
            <NumberDialog
              id={
                field === 'frame'
                  ? 'timelineCurrentFrame'
                  : field === 'start'
                    ? 'timelineRangeStart'
                    : 'timelineRangeEnd'
              }
              title={title}
              label={field === 'frame' ? 'Frame' : field === 'start' ? 'Start frame' : 'End frame'}
              initial={1}
              min={1}
              max={100_000}
              command={command}
              onDone={close}
            />
          );
          const n =
            field === 'frame'
              ? await dialogs.open<number>('Change Current Frame', render)
              : field === 'start'
                ? await dialogs.open<number>('Change Playback Start Frame', render)
                : await dialogs.open<number>('Change Playback End Frame', render);
          if (n === null) return null;
          p[field] = n;
        }
        return run(command, p);
      }
      // Paste Special and Cut Special in a cell context open the transfer-options dialog (04 §11.14; view note section 7):
      // OK stores the set for the operation (`timeline.transferOptions.set`) and runs it; Paste Special Again (`useLast`)
      // and an explicit set skip the dialog.
      case 'edit.pasteSpecial':
      case 'edit.cutSpecial': {
        const ctx = typeof p['context'] === 'string' ? p['context'] : state()?.focusedPanel;
        if (p['options'] === undefined && p['useLast'] !== true && (ctx === 'timeline' || ctx === 'xsheet')) {
          const operation = command === 'edit.pasteSpecial' ? 'paste' : 'cut';
          const title = command === 'edit.pasteSpecial' ? 'Paste Special' : 'Cut Special';
          const render = (close: (o: TransferOptions | null) => void) => (
            <TransferOptionsDialog
              title={title}
              load={() =>
                services.engine
                  .run('timeline.transferOptions.get', { operation })
                  .done.then((env) => (env.status === 'ok' ? parseTransferOptions(env.result) : null))
              }
              onDone={close}
            />
          );
          const options =
            operation === 'paste'
              ? await dialogs.open<TransferOptions>('Paste Special', render)
              : await dialogs.open<TransferOptions>('Cut Special', render);
          if (!options) return null;
          const stored = await run('timeline.transferOptions.set', { operation, options });
          if (stored.status !== 'ok') return stored;
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
      // The result dialogs of the File and Help menus (ruling D-30, group 1): each reads its query when it opens.
      case 'app.info.get':
        await dialogs.open<null>('About Toontown', (close) => (
          <AboutDialog
            services={services}
            close={() => {
              close(null);
            }}
          />
        ));
        return null;
      case 'project.info.get':
        await dialogs.open<null>('Project Info', (close) => (
          <ProjectInfoDialog
            services={services}
            close={() => {
              close(null);
            }}
          />
        ));
        return null;
      case 'doc.validate':
        if (Object.keys(p).length > 0) return run(command, p);
        await dialogs.open<null>('Check Project', (close) => (
          <CheckProjectDialog
            services={services}
            close={() => {
              close(null);
            }}
          />
        ));
        return null;
      case 'project.backup.open':
        if (p['backupId'] !== undefined) return run(command, p);
        await dialogs.open<null>('Open Backup', (close) => (
          <BackupsDialog
            services={services}
            {...(p['onUnsaved'] === 'discard' ? { onUnsaved: 'discard' as const } : {})}
            close={() => {
              close(null);
            }}
          />
        ));
        return null;
      case 'window.toolbar.items.set': {
        // Window › Customize Toolbar…: the sheet stages the items; Apply sends them (01 §5.7; design note 1 §7).
        if (p['items'] === undefined) {
          const chosen = await dialogs.open<Record<string, unknown>>('Customize Toolbar', (close) => (
            <ToolbarSheet services={services} close={close} />
          ));
          if (!chosen) return null;
          Object.assign(p, chosen);
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

  return { perform, run, refused: (error, commandId) => messages.refused(error, commandId) };
}
