import { DockviewReact, themeDark, type DockviewApi, type DockviewReadyEvent } from 'dockview-react';
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { handleMenuCommand, type FrontendContext } from './commands';
import { maybeRunIpcBench } from './diagnostics/ipcBench';
import { useHost, type SpikeMode } from './ipc';
import { applyDefaultLayout } from './layout/defaultLayout';
import { panelComponents } from './panels';
import { StatusBar, type StatusMessage } from './status/StatusBar';

// M0 spike page (C0.2), loaded only when the host was started with --spike tablet.
const TabletSpike = lazy(() =>
  import('./spikes/tablet/TabletSpike').then((m) => ({ default: m.TabletSpike })),
);

export function App() {
  const host = useHost();
  const dockApi = useRef<DockviewApi | null>(null);
  const [message, setMessage] = useState<StatusMessage | null>(null);
  const [spike, setSpike] = useState<SpikeMode | null>(null);

  useEffect(() => {
    host.spike.mode().then(setSpike, () => {
      setSpike(null);
    });
  }, [host]);

  const ctx = useMemo<FrontendContext>(
    () => ({
      resetWorkspace: () => {
        if (dockApi.current) applyDefaultLayout(dockApi.current);
      },
    }),
    [],
  );

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    host
      .onMenuCommand((event) => {
        setMessage(handleMenuCommand(ctx, event));
      })
      .then(
        (u) => {
          if (disposed) u();
          else unlisten = u;
        },
        (e: unknown) => {
          setMessage({ kind: 'error', text: `Cannot listen for menu commands: ${String(e)}` });
        },
      );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [host, ctx]);

  useEffect(() => {
    maybeRunIpcBench(host).then(
      (ran) => {
        if (ran) setMessage({ kind: 'info', text: 'IPC benchmark reported' });
      },
      (e: unknown) => {
        setMessage({ kind: 'error', text: `IPC benchmark failed: ${String(e)}` });
      },
    );
  }, [host]);

  const onReady = useCallback((event: DockviewReadyEvent) => {
    dockApi.current = event.api;
    applyDefaultLayout(event.api);
  }, []);

  if (spike) {
    return (
      <Suspense fallback={<p>Loading spike…</p>}>
        <TabletSpike mode={spike} />
      </Suspense>
    );
  }

  return (
    <div className="app">
      <main className="dock-host">
        <DockviewReact components={panelComponents} onReady={onReady} theme={themeDark} />
      </main>
      <StatusBar message={message} />
    </div>
  );
}
