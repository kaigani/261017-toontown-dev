import { createContext, useContext } from 'react';

import type { HostApi } from './hostApi';

export const HostContext = createContext<HostApi | null>(null);

/** The host API provided by the nearest `HostProvider`. */
export function useHost(): HostApi {
  const api = useContext(HostContext);
  if (!api) throw new Error('useHost() needs a <HostProvider>');
  return api;
}
