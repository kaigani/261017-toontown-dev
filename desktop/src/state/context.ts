// The shell services context and the hooks that read the mirror store.
import { createContext, useCallback, useContext, useSyncExternalStore } from 'react';

import type { EngineApi, Envelope, RunOptions } from '../ipc';
import type { SliceName, Store, StoreState } from './store';

export interface ShellServices {
  readonly engine: EngineApi;
  readonly store: Store;
  /** Runs a command as the user and returns its envelope. */
  readonly run: (
    command: string,
    params?: Record<string, unknown>,
    options?: RunOptions,
  ) => Promise<Envelope>;
}

export const ShellContext = createContext<ShellServices | null>(null);

export function useShell(): ShellServices {
  const s = useContext(ShellContext);
  if (!s) throw new Error('useShell outside ShellProvider');
  return s;
}

export function useStoreState(): StoreState {
  const { store } = useShell();
  const subscribe = useCallback((l: () => void) => store.subscribe(l), [store]);
  const get = useCallback(() => store.getState(), [store]);
  return useSyncExternalStore(subscribe, get, get);
}

/** A slice of the mirror store (the shapes are the commands' result schemas; see state/types.ts). */
export function useSlice(name: SliceName): unknown {
  return useStoreState().slices[name];
}
