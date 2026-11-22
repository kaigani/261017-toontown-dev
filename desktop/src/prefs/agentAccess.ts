// Preferences › Agent access, its model (01 §7.3 lines 783-789; 11 §6.4, §15.1; M4 C4.4a, design note 2 §2): the
// staged settings and, at OK, one command per changed group in table order, never `prefs.set`.
import type { PrefEntry } from '../state/types';

export interface AgentDraft {
  readonly server: { readonly enabled: boolean; readonly port: number; readonly allowRemote: boolean };
  readonly policy: Readonly<Record<string, unknown>>;
  readonly typed: { readonly enabled: boolean; readonly namespaces: readonly string[] };
  readonly persist: boolean;
}

/** The policy defaults' members the page edits (11 §15.1 `agent.policy.defaults.set`), with their factory values. */
export const POLICY_DEFAULTS: Readonly<Record<string, unknown>> = {
  policy: 'assist',
  maxJobs: 2,
  maxBatchSteps: 1000,
  maxInlineImagePx: 2048,
  maxInlineBytes: 52428800,
  askInApp: true,
  letAgentMoveView: true,
  highlightAgentChanges: true,
};

/** The limits' ranges (11 §15.1; `prefs.rs` `domain`). */
export const LIMITS: readonly { key: string; label: string; min: number; max: number; scale?: number }[] = [
  { key: 'maxJobs', label: 'Jobs at once', min: 1, max: 8 },
  { key: 'maxBatchSteps', label: 'Steps in one batch', min: 1, max: 100000 },
  { key: 'maxInlineImagePx', label: 'Largest inline image (pixels)', min: 256, max: 8192 },
  { key: 'maxInlineBytes', label: 'Largest inline answer (MB)', min: 1, max: 1024, scale: 1048576 },
];

const obj = (v: unknown): Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : {};

/** The settings as the preferences hold them now. */
export function draftOf(prefs: readonly PrefEntry[]): AgentDraft {
  const v = (key: string): unknown => prefs.find((p) => p.key === key)?.value;
  const typed = obj(v('agent.typedTools'));
  return {
    server: {
      enabled: v('agent.server.enabled') === true,
      port: typeof v('agent.server.port') === 'number' ? (v('agent.server.port') as number) : 47215,
      allowRemote: v('agent.server.allowRemote') === true,
    },
    policy: { ...POLICY_DEFAULTS, ...obj(v('agent.policy.defaults')) },
    typed: {
      enabled: typed['enabled'] === true,
      namespaces: Array.isArray(typed['namespaces'])
        ? (typed['namespaces'] as unknown[]).filter((n): n is string => typeof n === 'string')
        : [],
    },
    persist: v('agent.activity.persist') === true,
  };
}

export interface AgentCommand {
  readonly command: string;
  readonly params: Record<string, unknown>;
}

const sameList = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && [...a].sort().every((x, i) => x === [...b].sort()[i]);

/** The commands OK sends for the changed groups, in the table's order (design note 2 §2). */
export function agentCommands(before: AgentDraft, after: AgentDraft): AgentCommand[] {
  const out: AgentCommand[] = [];
  const s0 = before.server;
  const s1 = after.server;
  if (s0.enabled !== s1.enabled || s0.port !== s1.port || s0.allowRemote !== s1.allowRemote) {
    out.push({
      command: 'agent.server.set',
      params: {
        enabled: s1.enabled,
        ...(s0.port !== s1.port ? { port: s1.port } : {}),
        ...(s0.allowRemote !== s1.allowRemote ? { allowRemote: s1.allowRemote } : {}),
      },
    });
  }
  const members = Object.keys(after.policy).filter((k) => after.policy[k] !== before.policy[k]);
  if (members.length > 0) {
    out.push({
      command: 'agent.policy.defaults.set',
      params: Object.fromEntries(members.map((k) => [k, after.policy[k]])),
    });
  }
  if (
    before.typed.enabled !== after.typed.enabled ||
    !sameList(before.typed.namespaces, after.typed.namespaces)
  ) {
    out.push({
      command: 'agent.typedTools.set',
      params: { enabled: after.typed.enabled, namespaces: [...after.typed.namespaces].sort() },
    });
  }
  if (before.persist !== after.persist) {
    out.push({ command: 'agent.activity.persist.set', params: { enabled: after.persist } });
  }
  return out;
}

/** The registry's namespaces that have invocable commands (the typed tools' checklist). */
export function namespacesOf(index: Readonly<Record<string, { readonly event?: boolean }>>): string[] {
  const out = new Set<string>();
  for (const [id, e] of Object.entries(index)) {
    if (e.event === true || !id.includes('.')) continue;
    out.add(id.slice(0, id.indexOf('.')));
  }
  return [...out].sort();
}

/** `base` with the group `command` sets taken from `draft`: what is applied after that command succeeded. */
export function withGroup(base: AgentDraft, draft: AgentDraft, command: string): AgentDraft {
  switch (command) {
    case 'agent.server.set':
      return { ...base, server: draft.server };
    case 'agent.policy.defaults.set':
      return { ...base, policy: draft.policy };
    case 'agent.typedTools.set':
      return { ...base, typed: draft.typed };
    case 'agent.activity.persist.set':
      return { ...base, persist: draft.persist };
    default:
      return base;
  }
}
