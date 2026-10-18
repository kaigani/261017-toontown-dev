// The swappable transport underneath the typed host API. Two implementations exist: Tauri (production) and an
// in-memory mock (tests, and `pnpm dev` in a plain browser). A different shell would add a third.
//
// Transports deal in `unknown`: values crossing the process boundary are validated by hostApi.ts before UI code
// sees them.

export type Unlisten = () => void;

export interface HostTransport {
  readonly kind: 'tauri' | 'mock';
  /** Invokes a host command. Rejects with the host's error value (normally an `ErrorInfo`). */
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Subscribes to a host event. Resolves to a function that removes the subscription. */
  listen(event: string, handler: (payload: unknown) => void): Promise<Unlisten>;
  /**
   * Invokes a host command that streams messages back over a channel passed as argument `channelArg`. Resolves
   * with the command's result; `onMessage` receives every streamed message (still `unknown`). Optional: a transport
   * without streaming (e.g. a test double) omits it and streaming commands fail with `ipc.unsupported`.
   */
  invokeWithChannel?(
    command: string,
    args: Record<string, unknown>,
    channelArg: string,
    onMessage: (message: unknown) => void,
  ): Promise<unknown>;
}
