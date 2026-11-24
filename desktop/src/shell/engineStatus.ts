// Whether the engine answers (the host's watchdog; reviews/M4-design-engine-restart.md), held for the window's controls:
// every window's subscription sets it (`useEngineStatusMirror`), a control opened later reads it (a context menu opened while the engine is
// stopped shows why its items are disabled; ruling E-50 item 3).
import { useEffect, useSyncExternalStore } from 'react';

import type { HostApi } from '../ipc';

let notResponding = false;
const listeners = new Set<() => void>();

/** Records the watchdog's status. */
export function setEngineNotResponding(value: boolean): void {
  if (notResponding === value) return;
  notResponding = value;
  listeners.forEach((l) => {
    l();
  });
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Whether the engine is not responding now. */
export function useEngineNotResponding(): boolean {
  return useSyncExternalStore(subscribe, () => notResponding);
}

/** The reason a control shows while the engine is not responding. */
export const ENGINE_DOWN_REASON = 'the engine is not responding';

/** Follows the host's watchdog status for this window (mounted once per window, floating panels included). */
export function useEngineStatusMirror(host: Pick<HostApi, 'onEngineStatus'>): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void host
      .onEngineStatus((s) => {
        setEngineNotResponding(s.notResponding);
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [host]);
}
