// React provider of the shell services.
import type { ReactNode } from 'react';

import { ShellContext, type ShellServices } from './context';

export function ShellProvider({ services, children }: { services: ShellServices; children: ReactNode }) {
  return <ShellContext.Provider value={services}>{children}</ShellContext.Provider>;
}
