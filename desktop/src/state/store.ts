// The mirror store (M4 C4.1; ADR-0021). The front end never holds authoritative state: every slice here is a copy of
// executor state, refreshed by the engine events that announce a change, and every user action goes back to the
// executor as a command. Each refresh is a query command through the same entry point; the store remembers the
// number of the last engine event it applied, so the host can tell when the GUI has caught up (`inspect_ui`
// freshness) and a gap in the numbering triggers a full resync.
import type { EngineApi, EngineEvent, Envelope } from '../ipc';
import { refKey, type Availability, type CommandRef } from '../shell/availability';

/** A slice name and the query that fills it. */
export const SLICES = {
  appState: ['app.state.get', {}],
  layout: ['workspace.get', {}],
  workspaces: ['workspace.list', {}],
  panels: ['window.panel.list', {}],
  toolbars: ['window.toolbar.list', {}],
  prefs: ['prefs.list', {}],
  keymap: ['keymap.list', {}],
  history: ['edit.history.get', { limit: 1000 }],
  statusBar: ['ui.statusBar.get', {}],
  themes: ['ui.theme.list', {}],
  tips: ['ui.tips.list', {}],
  recent: ['project.recent.list', {}],
  recovery: ['project.recovery.list', {}],
  presets: ['project.presets.list', {}],
  agents: ['agent.server.status', {}],
} as const satisfies Record<string, readonly [string, Record<string, unknown>]>;

export type SliceName = keyof typeof SLICES | 'availability';

/** Which slices an engine event makes stale (01 §12.3 events). */
export function slicesFor(event: string): SliceName[] {
  switch (event) {
    case 'app.state.changed':
      return ['appState', 'statusBar', 'panels', 'layout', 'workspaces'];
    case 'workspace.changed':
      return ['layout', 'workspaces', 'panels', 'toolbars'];
    case 'prefs.changed':
      return ['prefs', 'statusBar', 'toolbars', 'layout'];
    case 'keymap.changed':
      return ['keymap'];
    case 'edit.history.changed':
    case 'doc.undone':
    case 'doc.redone':
    case 'doc.changed':
      return ['history', 'appState'];
    case 'project.opened':
    case 'project.closed':
    case 'project.saved':
    case 'project.dirty.changed':
      return ['appState', 'history', 'recent', 'recovery', 'statusBar'];
    case 'project.autosaved':
      return ['recovery'];
    case 'agent.session.connected':
    case 'agent.session.disconnected':
    case 'shell.agentActing':
      return ['agents', 'statusBar'];
    default:
      return [];
  }
}

export interface AgentActivity {
  readonly sessionId: string;
  readonly client: string;
  readonly commandId: string;
}

export interface StoreState {
  readonly slices: Partial<Record<SliceName, unknown>>;
  /** Errors of the last refresh of each slice (a failed query is shown, never hidden). */
  readonly errors: Partial<Record<SliceName, string>>;
  /** The last engine event applied. */
  readonly seq: number;
  /** Agents acting now (from `shell.agentActing`). */
  readonly acting: readonly AgentActivity[];
}

type Listener = () => void;
type EventHook = (event: EngineEvent) => void;

export interface Store {
  getState(): StoreState;
  subscribe(listener: Listener): () => void;
  /** Applies one engine event (refreshes the slices it makes stale). */
  apply(event: EngineEvent): Promise<void>;
  /** Refreshes every slice (start, reconnect, a gap in the event numbers). */
  resync(): Promise<void>;
  refresh(names: readonly SliceName[]): Promise<void>;
  /** Called for every event after it is applied (dialogs opened by `shell.*` notices, notifications). */
  onEvent(hook: EventHook): () => void;
  /**
   * The command references the GUI shows (toolbars, rail): their availability is kept in the `availability` slice,
   * keyed by `refKey`, and re-read from the kernel after every refresh (ruling O-10).
   */
  watch(refs: readonly CommandRef[]): void;
}

export function createStore(engine: EngineApi): Store {
  let state: StoreState = { slices: {}, errors: {}, seq: 0, acting: [] };
  const listeners = new Set<Listener>();
  const hooks = new Set<EventHook>();
  const set = (next: StoreState) => {
    state = next;
    listeners.forEach((l) => {
      l();
    });
  };

  let watched: CommandRef[] = [];
  const readAvailability = async () => {
    if (watched.length === 0) return;
    const asked = watched;
    try {
      const list = await engine.availability(asked);
      const map: Record<string, Availability> = {};
      list.forEach((a, i) => {
        const ref = asked[i];
        if (ref) map[refKey(ref)] = a;
      });
      set({ ...state, slices: { ...state.slices, availability: map } });
    } catch {
      // Without an answer every watched item stays disabled (the safe default).
    }
  };

  const refresh = async (names: readonly SliceName[]) => {
    const unique = [...new Set(names)];
    const results = await Promise.all(
      unique.map(async (name): Promise<[SliceName, Envelope]> => {
        const [command, params] = SLICES[name as keyof typeof SLICES];
        return [name, await engine.run(command, params).done];
      }),
    );
    const slices = { ...state.slices };
    const failed: Partial<Record<SliceName, string>> = {};
    const fresh = new Set<SliceName>();
    for (const [name, env] of results) {
      fresh.add(name);
      if (env.status === 'ok') {
        slices[name] = env.result;
      } else if (env.error?.code === 'project.none' || env.error?.code === 'command.notImplemented') {
        slices[name] = null;
      } else {
        failed[name] = `${env.error?.code ?? 'error'}: ${env.error?.message ?? ''}`;
      }
    }
    // Errors of the slices refreshed now are replaced; the others keep theirs.
    const kept = Object.fromEntries(Object.entries(state.errors).filter(([k]) => !fresh.has(k as SliceName)));
    set({ ...state, slices, errors: { ...kept, ...failed } });
    await readAvailability();
  };

  const store: Store = {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    async apply(event) {
      const gap = state.seq !== 0 && event.seq > state.seq + 1;
      if (event.name === 'shell.agentActing') {
        const p = (event.payload ?? {}) as Record<string, unknown>;
        const text = (v: unknown) => (typeof v === 'string' ? v : '');
        const entry: AgentActivity = {
          sessionId: text(p['sessionId']),
          client: text(p['client']),
          commandId: text(p['commandId']),
        };
        const others = state.acting.filter((a) => a.sessionId !== entry.sessionId);
        set({ ...state, acting: p['acting'] === true ? [...others, entry] : others });
      }
      if (gap) {
        await store.resync();
      } else {
        const stale = slicesFor(event.name);
        if (event.name === 'app.state.changed') {
          const p = event.payload as { state?: unknown } | null;
          if (p?.state !== undefined) set({ ...state, slices: { ...state.slices, appState: p.state } });
        }
        if (stale.length > 0) await refresh(stale);
      }
      set({ ...state, seq: Math.max(state.seq, event.seq) });
      hooks.forEach((h) => {
        h(event);
      });
    },
    async resync() {
      await refresh(Object.keys(SLICES) as (keyof typeof SLICES)[]);
    },
    refresh,
    onEvent(hook) {
      hooks.add(hook);
      return () => {
        hooks.delete(hook);
      };
    },
    watch(refs) {
      const before = new Set(watched.map(refKey));
      watched = [...refs];
      if (watched.some((r) => !before.has(refKey(r))) || watched.length !== before.size)
        void readAvailability();
    },
  };
  return store;
}
