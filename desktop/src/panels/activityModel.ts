// The Activity panel's model (11 §11; M4 C4.4a, design note 2 §3): the entries as `agent.activity.list` answers them,
// what Copy as batch JSON writes, and when Undo back to before an entry is possible.

export interface ActivityEntry {
  readonly seq: number;
  readonly time: string;
  readonly sessionId: string;
  readonly client: string;
  readonly tool: string;
  readonly commandId: string;
  readonly params: unknown;
  readonly status: string;
  readonly errorCode?: string;
  readonly undoStepId?: string | null;
  readonly changedCount: number;
  readonly dryRun?: boolean;
  readonly cachedRetry?: boolean;
  readonly durationMs: number;
}

/** At most this many entries are shown; the log itself keeps them all. */
export const SHOWN = 500;

export function asEntries(v: unknown): ActivityEntry[] {
  if (!Array.isArray(v)) return [];
  return v.filter(
    (e): e is ActivityEntry =>
      typeof e === 'object' && e !== null && typeof (e as { seq?: unknown }).seq === 'number',
  );
}

/** The entries with `next` appended in `seq` order, the oldest dropped beyond {@link SHOWN}; `null` when `next` does
 *  not follow the last one shown (a gap: the panel re-reads the log). */
export function appended(list: readonly ActivityEntry[], next: ActivityEntry): ActivityEntry[] | null {
  const last = list[list.length - 1];
  if (last && next.seq <= last.seq) return [...list];
  if (last && next.seq !== last.seq + 1) return null;
  const out = [...list, next];
  return out.length > SHOWN ? out.slice(out.length - SHOWN) : out;
}

/** The entry as a replayable batch, in the form `agent.activity.export {format: batch}` writes it (`replay_batch`): a
 *  call is one step `{id, params}`; an atomic batch one `batch` step with its label, steps and precondition; a
 *  sequential batch the steps it committed. */
export function batchOf(e: ActivityEntry & { committedSteps?: number[] }): ReplayBatch {
  return { label: 'Agent activity', atomic: false, stopOnError: true, steps: stepsOf(e) };
}

/** The file's batch around the steps (`agent.activity.export {format: batch}`). */
export interface ReplayBatch {
  readonly label: string;
  readonly atomic: false;
  readonly stopOnError: true;
  readonly steps: { id: string; params: unknown }[];
}

function stepsOf(e: ActivityEntry & { committedSteps?: number[] }): { id: string; params: unknown }[] {
  const p = (e.params ?? {}) as Record<string, unknown>;
  if (e.commandId !== 'batch') return [{ id: e.commandId, params: e.params ?? {} }];
  const steps = (Array.isArray(p['steps']) ? p['steps'] : []) as Record<string, unknown>[];
  if (p['atomic'] !== false) {
    const group: Record<string, unknown> = {
      label: p['label'] ?? 'Batch',
      atomic: true,
      steps: steps.map((s) => {
        if (s['as'] !== null) return s;
        const rest = { ...s };
        delete rest['as'];
        return rest;
      }),
    };
    const expect = p['expectRev'];
    if (expect && typeof expect === 'object' && Object.keys(expect).length > 0) group['expectRev'] = expect;
    return [{ id: 'batch', params: group }];
  }
  const committed = e.committedSteps ?? steps.map((_, i) => i);
  return committed
    .map((i) => steps[i])
    .filter((s): s is Record<string, unknown> => s !== undefined)
    .map((s) => ({ id: typeof s['id'] === 'string' ? s['id'] : '', params: s['params'] ?? {} }));
}

/** Whether the parameters carry inline media replaced by a hash (not replayable as is). */
export function hasMediaReference(e: ActivityEntry): boolean {
  return /"sha256:[0-9a-f]{8,}"/.test(JSON.stringify(e.params ?? null));
}

export interface HistoryStep {
  readonly index: number;
  readonly stepId?: string;
  readonly source: 'user' | 'agent';
  readonly agentName?: string;
}

/** Undo back to before an entry (11 §11, a linear history): the position to move to, the steps that undoes and how
 *  many of them are the person's; or why it cannot. */
export function undoBack(
  e: ActivityEntry,
  history: { entries: readonly HistoryStep[]; position: number } | null | undefined,
): { ok: true; position: number; steps: number; yours: number } | { ok: false; reason: string } {
  if (!e.undoStepId) return { ok: false, reason: 'it made no undo step' };
  const step = history?.entries.find((h) => h.stepId === e.undoStepId);
  if (!history || !step) return { ok: false, reason: 'its step is no longer in the history' };
  // `index` is 1-based (the position just after the step): applied while it is at most the position.
  if (step.index > history.position) return { ok: false, reason: 'already undone' };
  const undone = history.entries.filter((h) => h.index >= step.index && h.index <= history.position);
  return {
    ok: true,
    position: step.index - 1,
    steps: undone.length,
    yours: undone.filter((h) => h.source === 'user').length,
  };
}
