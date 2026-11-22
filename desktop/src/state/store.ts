// The mirror store (M4 C4.1; ADR-0021). The front end never holds authoritative state: every slice here is a copy of
// executor state, refreshed by the engine events that announce a change, and every user action goes back to the
// executor as a command. Each refresh is a query command through the same entry point; the store remembers the
// number of the last engine event it applied, so the host can tell when the GUI has caught up (`inspect_ui`
// freshness) and a gap in the numbering triggers a full resync.
import type { EngineApi, EngineEvent, Envelope } from '../ipc';
import { refKey, type Availability, type CommandRef } from '../shell/availability';
import { noteToolFamily, resetToolFamily } from '../canvas/toolFamily';

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

/** Events that change what a watched reference's availability says (its check mark, its completed parameters, its
 *  precheck) without making a slice stale: the drawing aids, the drawing tools and their settings, the selections, the
 *  view windows. After one, the availability is read again (C4.3b round 1, D-39: after the toolbar's Grid was clicked
 *  its next click sent the same visibility, the check mark and the completed reference being the old ones). */
export const AVAILABILITY_EVENTS: ReadonlySet<string> = new Set([
  'drawAid.changed',
  'tool.options.changed',
  'tool.active.changed',
  'draw.selection.changed',
  'edit.selection.changed',
  'shell.view',
]);

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
  /** A batch's own label and step count (D-62 item 3). */
  readonly label?: string;
  readonly steps?: number;
}

export interface StoreState {
  readonly slices: Partial<Record<SliceName, unknown>>;
  /** Errors of the last refresh of each slice (a failed query is shown, never hidden). */
  readonly errors: Partial<Record<SliceName, string>>;
  /** The last engine event applied. */
  readonly seq: number;
  /** The engine generation of those events (ruling O-14). */
  readonly generation: number;
  /** Agents acting now (from `shell.agentActing`). */
  readonly acting: readonly AgentActivity[];
  /** The number of the last `shell.agentActing` notice or snapshot `acting` reflects (D-86 item 2). */
  readonly actingSeq: number;
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
   * The command references the GUI shows (toolbars, rail, view chrome): their availability is kept in the
   * `availability` slice, keyed by `refKey`, and re-read from the kernel after every refresh (ruling O-10). Each
   * surface registers its own references under its `owner` (C43B-R1S-09); the store asks for the union.
   */
  watch(refs: readonly CommandRef[], owner?: string): void;
}

/** One running agent request, from a `shell.agentActing` notice's payload or the snapshot's entries (D-86 item 2). */
function activityOf(p: Record<string, unknown>): AgentActivity {
  const text = (v: unknown) => (typeof v === 'string' ? v : '');
  return {
    sessionId: text(p['sessionId']),
    client: text(p['client']),
    commandId: text(p['commandId']),
    ...(typeof p['label'] === 'string' ? { label: p['label'] } : {}),
    ...(typeof p['steps'] === 'number' ? { steps: p['steps'] } : {}),
  };
}

export function createStore(engine: EngineApi): Store {
  resetToolFamily();
  let state: StoreState = { slices: {}, errors: {}, seq: 0, generation: 0, acting: [], actingSeq: 0 };
  const listeners = new Set<Listener>();
  const hooks = new Set<EventHook>();
  const set = (next: StoreState) => {
    state = next;
    listeners.forEach((l) => {
      l();
    });
  };

  const owners = new Map<string, readonly CommandRef[]>();
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

  // D-86 item 2 (finding C44B-R3S-02): the agents' requests running now, read after the subscription (the engine
  // mirror resyncs once it is established); a notice newer than the snapshot (a higher number) wins.
  const readActing = async () => {
    try {
      const snap = await engine.agentActing();
      if (state.generation !== 0 && snap.generation !== state.generation) return;
      if (snap.seq < state.actingSeq) return;
      const acting = snap.acting.map((p) => activityOf((p ?? {}) as Record<string, unknown>));
      set({ ...state, acting, actingSeq: snap.seq });
    } catch {
      // A host without the snapshot (an older host, a test double): the notices alone.
    }
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
      // A new engine generation numbers its events from 1 again: the numbering restarts and everything is read anew.
      if (event.generation !== undefined && event.generation !== state.generation) {
        engine.setGeneration(event.generation);
        set({ ...state, generation: event.generation, seq: 0, acting: [], actingSeq: 0 });
        await store.resync();
        set({ ...state, seq: event.seq });
        hooks.forEach((h) => {
          h(event);
        });
        return;
      }
      const gap = state.seq !== 0 && event.seq > state.seq + 1;
      // SI-148: the family chosen last, for a Camera view canvas that mounts later.
      noteToolFamily(event.name);
      // Ruling D-89 item 2 (C44B-R4S-02): a notice the acting snapshot already covers (a number not above the one the
      // store holds, within this generation) is not applied again; the rest of the event is processed as before.
      if (event.name === 'shell.agentActing' && event.seq > state.actingSeq) {
        const p = (event.payload ?? {}) as Record<string, unknown>;
        const entry = activityOf(p);
        const others = state.acting.filter((a) => a.sessionId !== entry.sessionId);
        set({ ...state, acting: p['acting'] === true ? [...others, entry] : others, actingSeq: event.seq });
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
        else if (AVAILABILITY_EVENTS.has(event.name)) await readAvailability();
      }
      set({ ...state, seq: Math.max(state.seq, event.seq) });
      hooks.forEach((h) => {
        h(event);
      });
    },
    async resync() {
      await refresh(Object.keys(SLICES) as (keyof typeof SLICES)[]);
      await readActing();
    },
    refresh,
    onEvent(hook) {
      hooks.add(hook);
      return () => {
        hooks.delete(hook);
      };
    },
    watch(refs, owner = 'main') {
      const before = new Set(watched.map(refKey));
      if (refs.length === 0) owners.delete(owner);
      else owners.set(owner, refs);
      const union = new Map<string, CommandRef>();
      for (const list of owners.values()) for (const r of list) union.set(refKey(r), r);
      watched = [...union.values()];
      if (watched.some((r) => !before.has(refKey(r))) || watched.length !== before.size)
        void readAvailability();
    },
  };
  return store;
}
