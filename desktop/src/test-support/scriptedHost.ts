// A scripted host for front-end tests (M4 C4.1): it records every command the GUI sends — the command stream an
// agent would send for the same gesture — and answers from per-command handlers, with defaults that describe an
// application with no project open. Engine events can be emitted as the host would.
import type { HostTransport, Unlisten } from '../ipc';
import { parseWireJson } from '../ipc/wireJson';

export interface Sent {
  readonly command: string;
  readonly params: Record<string, unknown>;
  readonly dryRun: boolean;
  readonly token: string | null;
}

export type Handler = (params: Record<string, unknown>, sent: Sent) => unknown;

/** An ok envelope with a result. */
export function ok(result: unknown): unknown {
  return { status: 'ok', result, changes: [], warnings: [], documentRev: null };
}

/** An error envelope. */
export function fail(code: string, message: string, details?: unknown): unknown {
  return {
    status: 'error',
    error: { code, message, details: details ?? {} },
    changes: [],
    warnings: [],
    documentRev: null,
  };
}

export const NO_PROJECT_STATE = {
  project: null,
  scene: null,
  frame: null,
  element: null,
  drawing: null,
  mode: null,
  activeView: null,
  focusedPanel: 'cameraView',
  activeTool: null,
  selection: { context: 'drawing', ids: [], kinds: [] },
  workspace: { name: 'Default', modified: false },
  playback: { playing: false },
  startScreenVisible: true,
  modal: null,
  agentSessions: [],
  fullScreen: false,
};

export const DEFAULT_LAYOUT = {
  centralPanel: 'cameraView',
  corners: 'sides',
  docks: {
    left: [],
    right: [
      { tabs: ['colour', 'properties', 'library'], current: 'colour', size: 330 },
      { tabs: ['history'], current: 'history' },
    ],
    top: [],
    bottom: [{ tabs: ['timeline', 'xsheet'], current: 'timeline', size: 190 }],
  },
  floating: [],
  toolPalettes: [{ id: 'drawingTools', visible: true, floating: false }],
  mainToolbar: { visible: true, items: ['edit.undo', 'edit.redo', 'separator', 'workspaceList'] },
};

export const DEFAULT_PRESETS = [
  { id: 'default', name: 'Default', kind: 'default', frameRate: 24, width: 1920, height: 1080 },
  { id: 'web-small', name: 'Web small 4:3', kind: 'bundled', frameRate: 12, width: 480, height: 360 },
];

export interface ScriptedHost extends HostTransport {
  readonly sent: Sent[];
  handlers: Record<string, Handler>;
  /** Emits an engine event (numbered like the host). */
  event(name: string, payload: unknown): void;
  /** Skips an event number (a lost event). */
  skip(): void;
  /** Emits a menu command. */
  menu(command: string, params?: Record<string, unknown>): void;
  /** Answers to `dispatch_key`. */
  keys: Record<string, unknown>;
  /** Answers to `availability` (ruling O-10): the kernel's facts for one reference. */
  available: (ref: { commandId: string; params: Record<string, unknown> }) => {
    enabled: boolean;
    checked?: boolean;
    label?: string;
    reason?: string;
  };
  /** Answers to `pick_path` (null: cancelled). */
  picked: string | null;
  readonly reports: Record<string, unknown>[];
  /** The exact JSON text of each `report_shell` call, as the host receives it. */
  readonly rawReports: string[];
  readonly cancelled: string[];
  /** Commands sent, in order, without the queries the store makes. */
  actions(): Sent[];
}

const QUERY = /\.(get|list|status)$/;

export function createScriptedHost(overrides: Record<string, Handler> = {}): ScriptedHost {
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  let seq = 0;
  const host: ScriptedHost = {
    kind: 'mock',
    sent: [],
    reports: [],
    rawReports: [],
    cancelled: [],
    keys: {},
    available: (ref) =>
      ref.commandId === 'edit.undo' || ref.commandId === 'edit.redo'
        ? { enabled: false, reason: 'there is nothing to undo' }
        : { enabled: true },
    picked: null,
    handlers: {
      'app.state.get': () => ok(NO_PROJECT_STATE),
      'workspace.get': () => ok({ ...DEFAULT_LAYOUT, fullScreen: DEFAULT_LAYOUT }),
      'workspace.list': () =>
        ok([{ name: 'Default', builtIn: true, modified: false, position: 1, current: true }]),
      'window.panel.list': () => ok([]),
      'window.toolbar.list': () => ok([]),
      'prefs.list': () =>
        ok([
          {
            key: 'interface.theme',
            type: 'enum',
            default: 'system',
            value: 'system',
            values: ['system', 'light', 'dark'],
            applies: 'live',
            description: 'Interface theme.',
          },
          {
            key: 'general.startup.behaviour',
            type: 'enum',
            default: 'startScreen',
            value: 'startScreen',
            values: ['startScreen', 'lastProject', 'openDialog', 'newDialog', 'newDefaultProject'],
            applies: 'nextLaunch',
            description: 'What happens at launch.',
          },
          {
            key: 'files.backups.count',
            type: 'int',
            default: 3,
            value: 3,
            min: 0,
            max: 20,
            applies: 'live',
            description: 'Previous versions kept per project.',
          },
        ]),
      'keymap.list': () =>
        ok([
          { commandId: 'project.save', keys: 'Primary+S', context: 'global', isDefault: true },
          { commandId: 'edit.undo', keys: 'Primary+Z', context: 'global', isDefault: true },
        ]),
      'edit.history.get': () => fail('project.none', 'no project is open'),
      'ui.statusBar.get': () =>
        ok({ visible: true, description: '', context: '', pointer: '', playback: '', agentIndicator: '' }),
      'ui.theme.list': () =>
        ok([
          { themeId: 'system', name: 'Follow system', builtIn: true, dark: false },
          { themeId: 'light', name: 'Light', builtIn: true, dark: false },
          { themeId: 'dark', name: 'Dark', builtIn: true, dark: true },
        ]),
      'ui.tips.list': () =>
        ok([{ messageId: 'drawing.strokeHidden', description: 'A stroke is hidden.', suppressed: false }]),
      'project.recent.list': () =>
        ok([{ path: '/p/Walk.animpkg', name: 'Walk', lastOpened: '2026-11-01T10:00:00Z', exists: true }]),
      'project.recovery.list': () => ok([]),
      'project.presets.list': () => ok(DEFAULT_PRESETS),
      'agent.server.status': () => ok({ enabled: false, endpoint: null, sessions: [] }),
      ...overrides,
    },
    async invoke(command, args) {
      const json =
        args && typeof args['json'] === 'string'
          ? (parseWireJson(args['json']) as Record<string, unknown>)
          : {};
      switch (command) {
        case 'run_command': {
          const sent: Sent = {
            command: String(json['command']),
            params: (json['params'] ?? {}) as Record<string, unknown>,
            dryRun: (json['options'] as { dryRun?: boolean } | undefined)?.dryRun === true,
            token: typeof json['requestToken'] === 'string' ? json['requestToken'] : null,
          };
          host.sent.push(sent);
          const h = host.handlers[sent.command];
          return Promise.resolve(h ? h(sent.params, sent) : ok({}));
        }
        case 'cancel_request':
          host.cancelled.push(String(json['token']));
          return Promise.resolve({});
        case 'report_shell':
          host.reports.push(json);
          host.rawReports.push(String(args?.['json']));
          return Promise.resolve({});
        case 'dispatch_key':
          return Promise.resolve(
            host.keys[String(json['keys'])] ?? { outcome: 'unbound', keys: json['keys'] },
          );
        case 'availability': {
          const refs = (json['refs'] ?? []) as { commandId: string; params?: Record<string, unknown> }[];
          return Promise.resolve(
            refs.map((r) => ({
              commandId: r.commandId,
              params: r.params ?? {},
              dialog: false,
              ...host.available({ commandId: r.commandId, params: r.params ?? {} }),
            })),
          );
        }
        case 'pick_path':
          return Promise.resolve({ path: host.picked });
        case 'startup':
          return Promise.resolve({ open: [] });
        case 'app_info':
          return Promise.resolve({
            name: 'Toontown',
            version: '0.0.1',
            platform: 'browser',
            arch: 'mock',
            formatVersion: '1.0',
            registryVersion: '0.1.0',
            runMode: 'attached',
          });
        default:
          return Promise.reject(new Error(`no such command: ${command}`));
      }
    },
    listen(event, handler): Promise<Unlisten> {
      const set = listeners.get(event) ?? new Set();
      set.add(handler);
      listeners.set(event, set);
      return Promise.resolve(() => {
        set.delete(handler);
      });
    },
    event(name, payload) {
      seq += 1;
      for (const h of listeners.get('engine-event') ?? []) h({ seq, name, payload });
    },
    skip() {
      seq += 1;
    },
    menu(command, params = {}) {
      for (const h of listeners.get('menu-command') ?? [])
        h({ command, params, handler: 'frontend', milestone: null });
    },
    actions() {
      return host.sent.filter((s) => !QUERY.test(s.command) && s.command !== 'app.state.get');
    },
  };
  return host;
}
