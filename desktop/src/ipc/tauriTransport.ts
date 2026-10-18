// The only module allowed to import @tauri-apps/* (enforced by ESLint no-restricted-imports).
import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { HostTransport, Unlisten } from './transport';

export function isTauriRuntime(): boolean {
  return isTauri();
}

export function createTauriTransport(): HostTransport {
  return {
    kind: 'tauri',
    invoke: (command, args) => invoke<unknown>(command, args),
    listen: async (event, handler): Promise<Unlisten> =>
      listen<unknown>(event, (e) => {
        handler(e.payload);
      }),
    invokeWithChannel: (command, args, channelArg, onMessage) => {
      const channel = new Channel<unknown>();
      channel.onmessage = onMessage;
      return invoke<unknown>(command, { ...args, [channelArg]: channel });
    },
  };
}
