import { expect, it } from 'vitest';
import manifest from '../ui-bindings/menu.json';
import { readFileSync } from 'node:fs';
const keymap = JSON.parse(readFileSync('../crates/executor/src/keymap-default.json', 'utf8')) as { bindings: { commandId: string; params?: Record<string, unknown> }[] };
import { COMMAND_INDEX } from '../ipc/generated/commands';
import { paramProblems } from '../ipc';
function refs(x: unknown): { command: string; params?: Record<string, unknown> }[] {
  if (Array.isArray(x)) return x.flatMap(refs);
  if (!x || typeof x !== 'object') return [];
  const o = x as Record<string, unknown>;
  return [...(o['kind'] === 'command' ? [o as unknown as { command: string; params?: Record<string, unknown> }] : []), ...Object.values(o).flatMap(refs)];
}
it('manifest and default shortcuts reference registered commands with valid supplied parameters', () => {
  const entries = [...refs(manifest), ...keymap.bindings.map(b => ({ command: b.commandId, params: 'params' in b ? b.params as Record<string, unknown> : {} }))];
  for (const c of entries) {
    expect(Object.hasOwn(COMMAND_INDEX, c.command), c.command).toBe(true);
    expect(paramProblems(c.command, c.params ?? {}).filter(p => !p.includes('required')), c.command).toEqual([]);
  }
});

