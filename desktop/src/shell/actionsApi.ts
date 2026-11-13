// The actions API (M4 C4.1; ADR-0021): every GUI action ends in a command through the one engine entry point.
import { createContext, useContext } from 'react';

import type { Envelope } from '../ipc';

/** Commands that replace or close the open project and take `onUnsaved` (01 §12.1 rule 3). */
export const DISCARDING = new Set([
  'project.create',
  'project.open',
  'project.close',
  'project.recovery.open',
  'project.backup.open',
  'app.quit',
]);

/** After this long without an answer, a running action offers Cancel (product value; tests use fake timers). */
export const BUSY_AFTER_MS = 400;

export interface ActionsApi {
  /** Performs a command as the user would: dialogs, unsaved-changes rule, jobs, errors surfaced. */
  perform(command: string, params?: Record<string, unknown>): Promise<Envelope | null>;
  /** Runs a command with no dialog; failures are surfaced; long actions offer Cancel. */
  run(command: string, params?: Record<string, unknown>): Promise<Envelope>;
}

/** A running action that offers Cancel. */
export interface Busy {
  readonly title: string;
  readonly token: string | null;
}

export const ActionsContext = createContext<ActionsApi | null>(null);

export function useActions(): ActionsApi {
  const a = useContext(ActionsContext);
  if (!a) throw new Error('useActions outside ActionsProvider');
  return a;
}

/** Commands that are queries (never cancellable, never wait overlays). */
export function isQuery(command: string): boolean {
  return /\.(get|list|status)$/.test(command) || command === 'app.state.get';
}
