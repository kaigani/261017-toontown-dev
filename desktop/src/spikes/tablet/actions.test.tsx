// C04-R2-02 / C04-R3-02: the real tablet page records action markers (Space = start/finish, K = skip) on the keyup of
// a deliberate press only, at least 500 ms apart, and logs every raw Space/K event; the offline analyzer attributes
// contact episodes to actions by time. Episodes outside every action, omitted actions and performed actions without
// events are told apart; auto-repeat cannot advance the procedure.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';

import { createHostApi, createMockTransport, HostProvider } from '../../ipc';
import { TabletSpike } from './TabletSpike';

const ROOT = resolve(process.cwd(), '..');

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

class TestPointer extends Event {}

/** The page's clock: advanced explicitly, so marker spacing is controlled. */
let clock = 1000;
const wait = (ms: number) => {
  clock += ms;
};

/** One deliberate press: keydown, optional auto-repeat keydowns, keyup; then 600 ms pass. */
function press(k: string, repeats = 0, target: Element | Window = window) {
  fireEvent.keyDown(target, { key: k });
  for (let i = 0; i < repeats; i++) {
    wait(30);
    fireEvent.keyDown(target, { key: k, repeat: true });
  }
  wait(10);
  fireEvent.keyUp(target, { key: k });
  wait(600);
}

/** One pen contact on the canvas: down, `moves` moves (`still` = no movement, pressure only), up. */
function contact(moves: number, still = false) {
  const canvas = screen.getByTestId('spike-canvas');
  const send = (type: string, i: number, pressure: number) => {
    const e = new TestPointer(type, { cancelable: true });
    Object.assign(e, {
      pointerId: 1,
      pointerType: 'pen',
      pressure,
      tiltX: 0,
      tiltY: 0,
      twist: 0,
      clientX: still ? 50 : 10 + i,
      clientY: 20,
    });
    // The page records `timeStamp`, which in a browser is on the performance clock the markers use.
    Object.defineProperty(e, 'timeStamp', { value: performance.now() });
    canvas.dispatchEvent(e);
    wait(5);
  };
  send('pointerdown', 0, 0.2);
  for (let i = 1; i <= moves; i++) send('pointermove', i, 0.2 + (i % 5) / 10);
  send('pointerup', moves + 1, 0);
}

async function savedLog(script: () => void): Promise<Record<string, unknown>> {
  clock = 1000;
  vi.spyOn(performance, 'now').mockImplementation(() => clock);
  vi.stubGlobal('PointerEvent', TestPointer);
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
  const transport = createMockTransport();
  let saved: Record<string, unknown> | undefined;
  const invoke = transport.invoke.bind(transport);
  transport.invoke = (command, args) => {
    if (command === 'spike_save_log') {
      saved = args?.['log'] as Record<string, unknown>;
      return Promise.resolve({ path: 'test.json', bytes: 1 });
    }
    return invoke(command, args);
  };
  render(
    <HostProvider api={createHostApi(transport)}>
      <TabletSpike
        mode={{
          name: 'tablet',
          logPath: 'test.json',
          auto: false,
          penSimulation: false,
          nativeBackend: null,
        }}
      />
    </HostProvider>,
  );
  Object.assign(screen.getByTestId('spike-canvas'), { setPointerCapture: () => undefined });
  act(script);
  await act(async () => {
    fireEvent.click(screen.getByText('Save log'));
    await Promise.resolve();
  });
  if (!saved) throw new Error('Save log did not call the host');
  return saved;
}

interface Analysis {
  actions: {
    actions: { id: string; state: string; web: { outcome: string; measured: number | null } }[];
    webOutside: number[];
    notRecorded: string[];
    excludedSamples: { web: number; native: number };
    keyEvents: { events: number; transitions: number; ignored: number } | null;
  };
  decision: { outcome: string };
}

function analyse(log: unknown) {
  const dir = mkdtempSync(join(tmpdir(), 'tablet-actions-'));
  try {
    writeFileSync(join(dir, 'log.json'), JSON.stringify(log));
    const r = spawnSync(
      process.platform === 'win32' ? 'python' : 'python3',
      ['-I', 'scripts/analyse-tablet-log.py', join(dir, 'log.json'), '--json', join(dir, 'out.json')],
      { cwd: ROOT, encoding: 'utf8' },
    );
    const json =
      r.status === 0 ? (JSON.parse(readFileSync(join(dir, 'out.json'), 'utf8')) as Analysis) : null;
    return { status: r.status, stdout: r.stdout, stderr: r.stderr, json };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const states = (log: Record<string, unknown>) =>
  (log['actions'] as { id: string; state: string }[]).map((a) => `${a.id}:${a.state}`);

it('maps the six procedure actions to their contact episodes and ignores contacts outside them', async () => {
  const log = await savedLog(() => {
    press(' '); // A1 pressure ramp
    contact(30);
    press(' ');
    press(' '); // A2 speed
    contact(40);
    press(' ');
    press(' '); // A3 tilt
    contact(20);
    press(' ');
    contact(3); // a stray contact between actions: belongs to none
    press('k'); // A4 barrel rotation skipped: the pen has none
    press(' '); // A5 hover: performed, no contact
    press(' ');
    press(' '); // A6 stationary pressure
    contact(25, true);
    press(' ');
  });
  expect(log['source']).toBe('manual');
  const r = analyse(log);
  expect(r.status, r.stderr).toBe(0);
  const actions = r.json?.actions;
  expect(actions?.notRecorded).toEqual([]);
  expect(actions?.webOutside).toEqual([4]);
  // The stray contact's 5 samples are excluded from every decision input (C04-R3-01).
  expect(actions?.excludedSamples.web).toBe(5);
  expect(actions?.keyEvents).toMatchObject({ transitions: 11, ignored: 0 });
  const by = Object.fromEntries((actions?.actions ?? []).map((a) => [a.id, a]));
  expect(by['A1']?.web).toMatchObject({ outcome: 'episode 1', measured: 1 });
  expect(by['A2']?.web).toMatchObject({ outcome: 'episode 2', measured: 2 });
  expect(by['A3']?.web).toMatchObject({ outcome: 'episode 3', measured: 3 });
  expect(by['A4']).toMatchObject({ state: 'skipped', web: { outcome: 'omitted' } });
  expect(by['A5']).toMatchObject({ state: 'done', web: { outcome: 'performed, no events' } });
  expect(by['A6']?.web).toMatchObject({ outcome: 'episode 5', measured: 5 });
  expect(r.stdout).toContain('### Actions');
  expect(r.stdout).toContain('excluded from every decision input: web 5, native 0');
  // The rule set is applied too. The form says nothing about the pen's tilt support and the strokes carry none, so
  // tilt cannot be told apart from n/a: insufficient evidence, not a decision (ADR-0009 Step 0).
  expect(r.stdout).toContain('### Decision (ADR-0009 tablet rule set)');
  expect(r.json?.decision.outcome).toMatch(/^insufficient evidence/);
});

it('C04-R3-02: a held Space (auto-repeat) starts A1 once and finishes nothing', async () => {
  const log = await savedLog(() => {
    press(' ', 2);
  });
  expect(states(log)).toEqual(['A1:unfinished']);
  const keys = log['keyEvents'] as { type: string; repeat: boolean; outcome: string }[];
  expect(keys.filter((k) => k.repeat).map((k) => k.outcome)).toEqual([
    'ignored: auto-repeat',
    'ignored: auto-repeat',
  ]);
  expect(analyse(log).status).toBe(0);
});

it('C04-R3-02: one held K skips only the current action', async () => {
  const log = await savedLog(() => {
    for (let i = 0; i < 3; i++) {
      press(' ');
      press(' ');
    }
    press('k', 2);
  });
  expect(states(log)).toEqual(['A1:done', 'A2:done', 'A3:done', 'A4:skipped']);
});

it('C04-R3-02: two presses within 500 ms make one transition; a keyup without its keydown makes none', async () => {
  const log = await savedLog(() => {
    fireEvent.keyDown(window, { key: ' ' });
    fireEvent.keyUp(window, { key: ' ' }); // A1 started
    wait(100);
    fireEvent.keyDown(window, { key: ' ' });
    fireEvent.keyUp(window, { key: ' ' }); // 100 ms later: ignored
    wait(600);
    fireEvent.keyUp(window, { key: ' ' }); // no keydown: ignored
  });
  expect(states(log)).toEqual(['A1:unfinished']);
  const outcomes = (log['keyEvents'] as { outcome: string }[]).map((k) => k.outcome);
  expect(outcomes).toContain('ignored: within 500 ms of the last transition');
  expect(outcomes).toContain('ignored: keyup without its keydown');
});

it('C04-R3-02: Space on a focused toolbar button marks an action and does not activate the button', async () => {
  const log = await savedLog(() => {
    const button = screen.getByText('Start native listener');
    button.focus();
    expect(fireEvent.keyDown(button, { key: ' ', cancelable: true })).toBe(false);
    expect(fireEvent.keyUp(button, { key: ' ', cancelable: true })).toBe(false);
  });
  expect(states(log)).toEqual(['A1:unfinished']);
});

it('typing Space and K in the form creates no marker; the events are logged as ignored', async () => {
  const log = await savedLog(() => {
    const input = screen.getByLabelText('Notes');
    input.focus();
    fireEvent.keyDown(input, { key: 'k' });
    fireEvent.keyUp(input, { key: 'k' });
    fireEvent.keyDown(input, { key: ' ' });
    fireEvent.keyUp(input, { key: ' ' });
  });
  expect(log['actions']).toEqual([]);
  const outcomes = (log['keyEvents'] as { outcome: string }[]).map((k) => k.outcome);
  expect(outcomes).toEqual(Array(4).fill('ignored: typing in the form'));
});

it('records an action still running at Save as unfinished, and actions never reached as not recorded', async () => {
  const log = await savedLog(() => {
    press(' ');
    contact(10);
  });
  const r = analyse(log);
  expect(r.status, r.stderr).toBe(0);
  expect(r.json?.actions.actions).toMatchObject([{ id: 'A1', state: 'unfinished' }]);
  expect(r.json?.actions.notRecorded).toEqual(['A2', 'A3', 'A4', 'A5', 'A6']);
});

it('rejects malformed action records and key logs that disagree with them', async () => {
  const log = await savedLog(() => {
    press(' ');
    contact(5);
    press(' ');
  });
  const actions = log['actions'] as { id: string; startPerf: number; endPerf: number }[];
  const first = actions[0];
  if (!first) throw new Error('no action recorded');
  expect(analyse({ ...log, actions: [{ ...first, id: 'A2' }] }).status).toBe(2);
  expect(analyse({ ...log, actions: [{ ...first, endPerf: first.startPerf - 1 }] }).status).toBe(2);
  const keys = log['keyEvents'] as { outcome: string; type: string }[];
  // A transition lost from the raw key log, or claimed on a keydown, is a corrupted sequence.
  const lost = keys.filter((k) => !k.outcome.endsWith('done'));
  expect(analyse({ ...log, keyEvents: lost }).status).toBe(2);
  const onKeydown = keys.map((k) => (k.outcome.startsWith('transition') ? { ...k, type: 'keydown' } : k));
  expect(analyse({ ...log, keyEvents: onKeydown }).status).toBe(2);
});
