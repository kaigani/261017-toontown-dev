// The dialogs API: type, context and hook. The host and the dialog chrome are in dialogs.tsx.
import { createContext, useContext, type ReactNode } from 'react';

export type Close<T> = (result: T | null) => void;

export interface DialogsApi {
  open<T>(title: string, render: (close: Close<T>) => ReactNode): Promise<T | null>;
}

export const DialogsContext = createContext<DialogsApi | null>(null);

export function useDialogs(): DialogsApi {
  const d = useContext(DialogsContext);
  if (!d) throw new Error('useDialogs outside DialogHost');
  return d;
}
