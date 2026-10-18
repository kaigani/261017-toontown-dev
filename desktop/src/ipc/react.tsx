import type { ReactNode } from 'react';

import { HostContext } from './context';
import type { HostApi } from './hostApi';

export function HostProvider({ api, children }: { api: HostApi; children: ReactNode }) {
  return <HostContext value={api}>{children}</HostContext>;
}
