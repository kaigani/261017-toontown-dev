// The messages API (01 §9.4-§9.5): types, context and hook. The host component is in messages.tsx.
import { createContext, useContext } from 'react';

import type { Envelope } from '../ipc';

export type Severity = 'info' | 'warning' | 'error' | 'question';

export interface MessageButton {
  readonly id: string;
  readonly label: string;
  readonly role?: 'default' | 'cancel' | 'destructive';
}

export interface MessageSpec {
  readonly severity: Severity;
  readonly messageId: string;
  readonly title?: string;
  readonly text: string;
  readonly details?: unknown;
  readonly buttons?: readonly MessageButton[];
  /** A suppressible tip (01 §9.4): offers "Don't show this again". */
  readonly suppressible?: boolean;
}

export interface MessageAnswer {
  readonly button: string;
  readonly suppress: boolean;
}

export interface StatusLine {
  readonly text: string;
  readonly kind: 'info' | 'warning' | 'error';
}

export interface Notification extends MessageSpec {
  readonly key: number;
}

export interface MessagesApi {
  /** A modal message box; resolves with the button chosen. */
  show(spec: MessageSpec): Promise<MessageAnswer>;
  /** A non-modal notification (agent-originated messages, 01 §12.1 rule 6). */
  notify(spec: MessageSpec): void;
  /** The status line on the left of the status bar. */
  status(text: string, kind?: StatusLine['kind']): void;
  /** Surfaces a failed command to the user: a cancellation as a status line, anything else as a message box. */
  failure(env: Envelope, what: string): Promise<void>;
  /** Opens a modal state the executor must know about (dialogs, progress); returns the function that closes it. */
  enterModal(kind: 'dialog' | 'progress' | 'gesture', title: string): () => void;
}

export const OK: readonly MessageButton[] = [{ id: 'ok', label: 'OK', role: 'default' }];

export const MessagesContext = createContext<MessagesApi | null>(null);

export function useMessages(): MessagesApi {
  const m = useContext(MessagesContext);
  if (!m) throw new Error('useMessages outside MessageHost');
  return m;
}

/** How long a notification stays (product timer; tests use fake timers). */
export const NOTIFICATION_MS = 8000;
