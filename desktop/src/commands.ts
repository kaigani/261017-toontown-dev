// Front-end handling of menu commands forwarded by the host (handler `frontend` or `pending` in
// src/ui-bindings/menu.json). From M1 these become command invocations on the engine's registry.
import type { MenuCommandEvent } from './ipc';
import type { StatusMessage } from './status/StatusBar';

export interface FrontendContext {
  resetWorkspace(): void;
}

/** Commands whose `handler` is `frontend` in the manifest. The bindings test checks the two lists agree. */
export const FRONTEND_COMMANDS: Record<
  string,
  (ctx: FrontendContext, params: Record<string, unknown>) => void
> = {
  'workspace.reset': (ctx) => {
    ctx.resetWorkspace();
  },
};

/** Executes a forwarded menu command and returns the status message to show. */
export function handleMenuCommand(ctx: FrontendContext, event: MenuCommandEvent): StatusMessage {
  if (event.handler === 'pending') {
    return {
      kind: 'warning',
      text: `${event.command} is not implemented yet (${event.milestone ?? 'later milestone'})`,
    };
  }
  const run = FRONTEND_COMMANDS[event.command];
  if (!run) return { kind: 'error', text: `No front-end handler for ${event.command}` };
  run(ctx, event.params);
  return { kind: 'info', text: `${event.command} done` };
}
