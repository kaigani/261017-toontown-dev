// The desktop shell (M4 C4.1; ADR-0021). The window renders executor state and sends every user action back as a
// command: the mirror store follows the engine's numbered events; menus, shortcuts, toolbars, docking gestures and
// dialogs go through the action router; facts only the shell knows (modal state, text focus, view sizes, the rendered
// UI tree with the last event it reflects) are reported to the executor.
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { DockArea } from './dock/DockArea';
import { useHost, type Envelope, type SpikeMode } from './ipc';
import { isMac, isUnmodified, sequenceOf } from './keys/sequence';
import { PreferencesDialog } from './prefs/PreferencesDialog';
import { ActionsProvider } from './shell/actions';
import { useActions } from './shell/actionsApi';
import { Dialog, DialogHost } from './shell/dialogs';
import { useDialogs } from './shell/dialogsApi';
import { MessageHost } from './shell/messages';
import { useMessages } from './shell/messagesApi';
import { StatusBar } from './shell/StatusBar';
import { MainToolbar, ToolPaletteRail } from './shell/Toolbars';
import { collectUiTree } from './shell/uiTree';
import { StartScreen } from './start/StartScreen';
import { useShell, useSlice, useStoreState, type ShellServices } from './state/context';
import { ShellProvider } from './state/react';
import { createStore } from './state/store';
import type { AppState, Binding, Layout, PrefEntry, Theme, Tip } from './state/types';
import { applyTheme } from './theme/theme';

// M0 spike pages (C0.2 tablet, C0.3 canvas), loaded only when the host was started with --spike. They exist only in
// the diagnostics build (src/m0Diagnostics.d.ts): the default bundle contains neither page nor its chunk (C0.4).
const TabletSpike = __TT_M0_DIAGNOSTICS__
  ? lazy(() => import('./spikes/tablet/TabletSpike').then((m) => ({ default: m.TabletSpike })))
  : null;
const CanvasSpike = __TT_M0_DIAGNOSTICS__
  ? lazy(() => import('./spikes/canvas/CanvasSpike').then((m) => ({ default: m.CanvasSpike })))
  : null;

/** Start-up runs once per application, even when React mounts the shell twice (development). */
const startupDone = { value: false };

/** How long after the last change the rendered UI tree is reported (a frame or two; product value). */
export const TREE_REPORT_MS = 30;

/** Browser accelerators the webview must not act on outside text fields (reload, find, print, zoom, navigation). */
function isBrowserAccelerator(seq: string): boolean {
  return (
    seq.includes('Primary+') || seq.startsWith('Alt+') || /^(Shift\+)?F\d+$/.test(seq) || seq === 'Backspace'
  );
}

function isTextField(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  if (el.isContentEditable) return true;
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  return (
    el instanceof HTMLInputElement && !['checkbox', 'radio', 'button', 'submit', 'reset'].includes(el.type)
  );
}

export function App() {
  const host = useHost();
  const [spike, setSpike] = useState<SpikeMode | null>(null);
  useEffect(() => {
    host.spike.mode().then(setSpike, () => {
      setSpike(null);
    });
  }, [host]);

  const services = useMemo<ShellServices>(() => {
    const store = createStore(host.engine);
    return {
      engine: host.engine,
      store,
      run: (command, params, options) => host.engine.run(command, params, options).done,
    };
  }, [host]);

  if (spike && TabletSpike && CanvasSpike) {
    return (
      <Suspense fallback={<p>Loading spike…</p>}>
        {spike.name === 'canvas' ? <CanvasSpike mode={spike} /> : <TabletSpike mode={spike} />}
      </Suspense>
    );
  }
  return (
    <ShellProvider services={services}>
      <Shell />
    </ShellProvider>
  );
}

function Shell() {
  const services = useShell();
  const tips = useSlice('tips') as readonly Tip[] | undefined;
  const suppressed = useMemo(
    () => new Set((tips ?? []).filter((t) => t.suppressed).map((t) => t.messageId)),
    [tips],
  );
  return (
    <MessageHost engine={services.engine} suppressed={suppressed}>
      <DialogHost>
        <ShellWithActions />
      </DialogHost>
    </MessageHost>
  );
}

function ShellWithActions() {
  const services = useShell();
  return (
    <ActionsProvider services={services}>
      <Window />
    </ActionsProvider>
  );
}

function Window() {
  const services = useShell();
  const host = useHost();
  const actions = useActions();
  const dialogs = useDialogs();
  const messages = useMessages();
  const state = useStoreState();
  const appState = state.slices.appState as AppState | undefined;
  const layout = state.slices.layout as Layout | undefined;
  const prefs = (state.slices.prefs as readonly PrefEntry[] | undefined) ?? [];
  const themes = useMemo(
    () => (state.slices.themes as readonly Theme[] | undefined) ?? [],
    [state.slices.themes],
  );
  const keymap = useMemo(
    () => (state.slices.keymap as readonly Binding[] | undefined) ?? [],
    [state.slices.keymap],
  );
  const root = useRef<HTMLDivElement | null>(null);
  const mac = isMac();

  // Engine events, applied in order; a resync at start and after a gap (IPC freshness).
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    let chain: Promise<void> = Promise.resolve();
    void services.engine
      .onEvent((e) => {
        chain = chain.then(() => services.store.apply(e)).catch(() => undefined);
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    void services.store.resync();
    void services.engine
      .report({
        attached: true,
        screens: [
          {
            x: 0,
            y: 0,
            width: Math.max(1, window.screen.availWidth || window.innerWidth),
            height: Math.max(1, window.screen.availHeight || window.innerHeight),
            scale: window.devicePixelRatio || 1,
          },
        ],
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [services]);

  // Start-up (01 §1.1 rules 7-8): documents requested before the window existed take precedence over the start-up
  // behaviour preference. Runs once per application (React may mount twice in development).
  useEffect(() => {
    if (startupDone.value) return;
    startupDone.value = true;
    void (async () => {
      const { open } = await host.startup();
      if (open.length > 0) {
        for (const path of open) await actions.perform('project.open', { path });
        return;
      }
      const env = await services.engine.run('prefs.get', { keys: ['general.startup.behaviour'] }).done;
      const values = (env.result as { values?: Record<string, unknown> } | undefined)?.values ?? {};
      switch (values['general.startup.behaviour']) {
        case 'lastProject': {
          const recent = await services.engine.run('project.recent.list', {}).done;
          const first = Array.isArray(recent.result) ? (recent.result[0] as { path?: unknown } | undefined) : undefined;
          if (typeof first?.path === 'string') await actions.perform('project.open', { path: first.path });
          break;
        }
        case 'openDialog':
          await actions.perform('project.open');
          break;
        case 'newDialog':
          await actions.perform('project.create');
          break;
        case 'newDefaultProject':
          await actions.perform('project.create', { name: 'Untitled' });
          break;
        default:
          break;
      }
    })();
  }, [actions, host, services]);

  // Host notices: open the preferences window or the help viewer when a command asks for them.
  const openPreferences = useCallback(
    async (category?: string) => {
      const s = services.store.getState().slices;
      await dialogs.open<boolean>('Preferences', (close) => (
        <PreferencesDialog
          services={services}
          prefs={(s.prefs as readonly PrefEntry[] | undefined) ?? []}
          keymap={(s.keymap as readonly Binding[] | undefined) ?? []}
          tips={(s.tips as readonly Tip[] | undefined) ?? []}
          themes={(s.themes as readonly Theme[] | undefined) ?? []}
          {...(category ? { initialCategory: category } : {})}
          close={close}
        />
      ));
    },
    [dialogs, services],
  );
  const openHelp = useCallback(
    async (topic?: string) => {
      const env = await services.engine.run(
        'ui.help.get',
        topic?.includes('.') ? { commandId: topic } : { panelId: 'history' },
      ).done;
      const r = (env.result ?? { title: 'Documentation', text: '' }) as { title: string; text: string };
      await dialogs.open<null>('Help', (close) => (
        <Dialog
          id="help"
          title={r.title}
          onCancel={() => {
            close(null);
          }}
        >
          <pre className="help-text" data-tt-role="text" data-tt-id="help.text">
            {r.text}
          </pre>
        </Dialog>
      ));
    },
    [dialogs, services],
  );
  useEffect(
    () =>
      services.store.onEvent((e) => {
        const p = (e.payload ?? {}) as Record<string, unknown>;
        if (e.name === 'shell.preferences')
          void openPreferences(typeof p['category'] === 'string' ? p['category'] : undefined);
        if (e.name === 'shell.help') void openHelp(typeof p['topic'] === 'string' ? p['topic'] : undefined);
        if (e.name === 'ui.message.shown' && p['source'] === 'agent') {
          // Agent-originated messages are non-modal (01 §12.1 rule 6).
          messages.notify({
            severity: p['severity'] === 'warning' || p['severity'] === 'error' ? p['severity'] : 'info',
            messageId: typeof p['messageId'] === 'string' ? p['messageId'] : 'agent',
            text: typeof p['text'] === 'string' ? p['text'] : '',
          });
        }
      }),
    [messages, openHelp, openPreferences, services],
  );

  // Menu items forwarded by the host: one action each.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void host
      .onMenuCommand((event) => {
        void actions.perform(event.command, event.params);
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [actions, host]);

  // The theme follows `interface.theme` (01 §5.8).
  const themeId = prefs.find((p) => p.key === 'interface.theme')?.value;
  useEffect(() => {
    applyTheme(typeof themeId === 'string' ? themeId : 'system', themes);
  }, [themeId, themes]);

  // Shortcuts (01 §8.4): the kernel's keymap decides; a text field keeps unmodified keys.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const seq = sequenceOf(e, mac);
      if (!seq) return;
      const field = isTextField(e.target);
      if (field && isUnmodified(seq)) return;
      const bound = keymap.some((b) => b.keys === seq);
      if (!bound && !isBrowserAccelerator(seq)) return;
      e.preventDefault();
      if (!bound) return;
      void services.engine
        .dispatchKey(seq)
        .then(async (outcome) => {
          if (outcome.outcome !== 'command' || !outcome.commandId) return;
          // A disabled command consumes its key silently (01 §8.4).
          if (outcome.enabled === false) return;
          await actions.perform(outcome.commandId, outcome.params ?? {});
        })
        .catch(() => undefined);
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [actions, keymap, mac, services]);

  // Text focus is a sensor fact (the kernel's key dispatch and the `key` tool respect it).
  useEffect(() => {
    const report = () => {
      void services.engine.report({ textFocus: isTextField(document.activeElement) }).catch(() => undefined);
    };
    window.addEventListener('focusin', report);
    window.addEventListener('focusout', report);
    return () => {
      window.removeEventListener('focusin', report);
      window.removeEventListener('focusout', report);
    };
  }, [services]);

  // The rendered UI tree, with the last engine event it reflects (inspect_ui freshness).
  useEffect(() => {
    const timer = setTimeout(() => {
      if (!root.current) return;
      const tree = collectUiTree(root.current, {
        role: 'window',
        id: 'main',
        label: appState?.project ? `Toontown — ${appState.project.name}` : 'Toontown',
      });
      void services.engine.report({ uiTree: tree, renderedSeq: state.seq }).catch(() => undefined);
    }, TREE_REPORT_MS);
    return () => {
      clearTimeout(timer);
    };
  });

  // The window title (01 §5.1): project, modified, read-only, scene, frame.
  useEffect(() => {
    const p = appState?.project;
    if (!p) {
      document.title = 'Toontown';
      return;
    }
    const parts = [`${p.name}${p.dirty ? ' *' : ''}${p.readOnly ? ' [read-only]' : ''}`];
    if (appState.scene) parts.push(appState.scene.name);
    if (appState.frame !== null) parts.push(`frame ${String(appState.frame)}`);
    document.title = `${parts.join(' — ')} — Toontown`;
  }, [appState]);

  const fullScreen = appState?.fullScreen === true;
  return (
    <div className="app" ref={root} data-tt-role="group" data-tt-id="app">
      <MainToolbar layout={layout} fullScreen={fullScreen} />
      <div className="workspace-row">
        <ToolPaletteRail layout={layout} fullScreen={fullScreen} />
        <main className="dock-main">
          <DockArea layout={layout} fullScreen={fullScreen} />
        </main>
      </div>
      <StatusBar />
      {appState?.startScreenVisible && <StartScreen />}
      <RefreshErrors errors={state.errors} />
    </div>
  );
}

/** A slice that cannot be read is shown, not hidden. */
function RefreshErrors({ errors }: { errors: Partial<Record<string, string>> }) {
  const entries = Object.entries(errors);
  if (entries.length === 0) return null;
  return (
    <div className="refresh-errors" role="alert" data-tt-role="text" data-tt-id="refreshErrors">
      {entries.map(([k, v]) => (
        <div key={k}>
          {k}: {v}
        </div>
      ))}
    </div>
  );
}

export type { Envelope };
