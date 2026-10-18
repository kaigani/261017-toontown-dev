// Appended to the amended reviewer R3 suite by page_probe.py, using its capture() helper.
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const repo = resolve(process.cwd(), '..') + '/';
const evidence = `${repo}reviews/evidence/M0-C0.4-r4/`;

function analysePage(name: string, log: Record<string, unknown>) {
  const input = `${evidence}page-${name}.json`;
  const output = `${evidence}page-${name}.analysis.json`;
  writeFileSync(input, JSON.stringify(log));
  const r = spawnSync('python', ['-X', 'utf8', 'scripts/analyse-tablet-log.py', input, '--json', output],
    { cwd: repo, encoding: 'utf8' });
  writeFileSync(`${evidence}page-${name}.log`, r.stdout + r.stderr);
  return { status: r.status, stdout: r.stdout,
    json: r.status === 0 ? JSON.parse(readFileSync(output, 'utf8')) : null };
}

function deliberate(k: string) {
  fireEvent.keyDown(window, { key: k });
  fireEvent.keyUp(window, { key: k });
  clock += 500;
}

it('closure: pen-button keydown-only sequences record every key and make no transition', async () => {
  const log = await capture(() => {
    fireEvent.keyDown(window, { key: ' ' });
    clock += 500;
    fireEvent.keyDown(window, { key: ' ', repeat: true });
    fireEvent.keyDown(window, { key: 'k' });
    clock += 500;
    fireEvent.keyDown(window, { key: 'k', repeat: true });
  });
  expect(log['actions']).toEqual([]);
  expect(log['keyEvents']).toHaveLength(4);
  const r = analysePage('keydown-only', log);
  expect(r.status).toBe(0);
  expect(r.json.actions.keyEvents.transitions).toBe(0);
  expect(r.json.decision.selected).toBeNull();
});

it('closure: a 499 ms double press is ignored, a 500 ms press transitions', async () => {
  const log = await capture(() => {
    deliberate(' ');
    clock -= 1; // exactly 499 ms since first transition
    fireEvent.keyDown(window, { key: ' ' });
    fireEvent.keyUp(window, { key: ' ' });
    clock += 1; // exactly 500 ms since first transition
    fireEvent.keyDown(window, { key: ' ' });
    fireEvent.keyUp(window, { key: ' ' });
  });
  expect(log['actions']).toMatchObject([{ id: 'A1', state: 'done', startPerf: 1000, endPerf: 1500 }]);
  expect(log['actions']).toHaveLength(1);
  expect(log['keyEvents']).toHaveLength(6);
  expect((log['keyEvents'] as { outcome: string }[]).filter(k => k.outcome.startsWith('transition'))).toHaveLength(2);
  expect(analysePage('499-500', log).status).toBe(0);
});

it('closure: performed without events is distinct from omitted through the real page and CLI', async () => {
  const log = await capture(() => {
    for (let i = 0; i < 3; i++) { deliberate(' '); deliberate(' '); }
    deliberate('k'); // A4 omitted
    deliberate(' '); deliberate(' '); // A5 performed, no contact events
    deliberate(' '); deliberate(' '); // A6 performed, no pressure updates
  });
  const r = analysePage('performed-omitted', log);
  expect(r.status).toBe(0);
  expect(r.stdout.match(/### Decision \(/g)).toHaveLength(1);
  const by = Object.fromEntries(r.json.actions.actions.map((a: { id: string }) => [a.id, a]));
  expect(by['A4']).toMatchObject({ state: 'skipped', web: { outcome: 'omitted' } });
  expect(by['A5']).toMatchObject({ state: 'done', web: { outcome: 'performed, no events' } });
  expect(by['A6']).toMatchObject({ state: 'done', web: { outcome: 'performed, no events' } });
  expect(r.json.decision.rows.W['9'].result).toBe('fail');
});

it('closure: malformed key sequence from a saved page log is rejected even when transition counts match', async () => {
  const log = await capture(() => { deliberate(' '); deliberate(' '); });
  expect(analysePage('valid-markers', log).status).toBe(0);
  const damaged = { ...log, keyEvents: (log['keyEvents'] as { type: string }[]).filter(k => k.type === 'keyup') };
  expect(analysePage('missing-keydowns', damaged).status).toBe(2);
});
