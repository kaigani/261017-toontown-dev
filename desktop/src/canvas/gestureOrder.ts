// The order of a view window's gesture messages (C42B-R2-03, SI-157): the modal state's reports and the gesture's
// commit go out one after another, never overtaking each other, and the end of the modal state is reported only once
// the commit has answered, so an agent change waiting on the gesture runs after its step, never between.

/** The modal state a gesture reports: on, with the acting tool's name, or ended. */
export type GestureModal = { kind: 'gesture'; title: string } | null;

/** What the ordering needs from the engine. */
export interface GestureChannel<Request, Answer> {
  report(report: { modal: GestureModal }): Promise<unknown>;
  gestureCommit(request: Request): Promise<Answer>;
}

/** The window's gesture messages, in order. */
export interface GestureOrder<Request, Answer> {
  /** Reports the modal state, after every message before it. */
  report(modal: GestureModal): void;
  /** Sends a gesture's commit after every message before it, then reports the end of the modal state once the commit
   *  has answered (or failed). Resolves with the commit's answer. */
  commitThenEnd(request: Request): Promise<Answer>;
}

export function gestureOrder<Request, Answer>(
  engine: GestureChannel<Request, Answer>,
): GestureOrder<Request, Answer> {
  let chain: Promise<unknown> = Promise.resolve();
  return {
    report(modal) {
      chain = chain.then(() => engine.report({ modal })).catch(() => undefined);
    },
    commitThenEnd(request) {
      const sent = chain.then(() => engine.gestureCommit(request));
      const end = () => engine.report({ modal: null });
      chain = sent.then(end, end).catch(() => undefined);
      return sent;
    },
  };
}
