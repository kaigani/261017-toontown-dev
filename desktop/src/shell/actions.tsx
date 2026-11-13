// The actions provider (M4 C4.1): exposes the action router and shows Cancel for a long-running action's request.
import { useMemo, useState, type ReactNode } from 'react';

import type { ShellServices } from '../state/context';
import type { AppState } from '../state/types';
import { ActionsContext, type ActionsApi, type Busy } from './actionsApi';
import { useDialogs } from './dialogsApi';
import { makeActions } from './makeActions';
import { useMessages } from './messagesApi';

export function ActionsProvider({
  services,
  children,
  onQuit,
}: {
  services: ShellServices;
  children: ReactNode;
  onQuit?: () => void;
}) {
  const messages = useMessages();
  const dialogs = useDialogs();
  const [busy, setBusy] = useState<Busy | null>(null);
  // The router reads the application state from the mirror store when an action runs.
  const api = useMemo<ActionsApi>(
    () =>
      makeActions(
        services,
        messages,
        dialogs,
        () => services.store.getState().slices.appState as AppState | undefined,
        setBusy,
        onQuit,
      ),
    [services, messages, dialogs, onQuit],
  );

  return (
    <ActionsContext.Provider value={api}>
      {children}
      {busy && (
        <div
          className="busy-overlay"
          role="status"
          data-tt-role="dialog"
          data-tt-id="busy"
          data-tt-label={busy.title}
        >
          <span>{busy.title}…</span>
          {busy.token && (
            <button
              type="button"
              data-tt-role="button"
              data-tt-id="busy.cancel"
              onClick={() => {
                if (busy.token) void services.engine.cancel(busy.token);
              }}
            >
              Cancel
            </button>
          )}
        </div>
      )}
    </ActionsContext.Provider>
  );
}
