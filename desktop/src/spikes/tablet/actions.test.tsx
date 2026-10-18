// C04-R2-02: the real tablet page records action markers (Space = start/finish, K = skip), and the offline analyzer
// attributes contact episodes to actions by time. Episodes outside every action, omitted actions and performed
// actions without events are told apart.
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

function key(k: string) {
  fireEvent.keyDown(window, { key: k });
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
  };
  send('pointerdown', 0, 0.2);
  for (let i = 1; i <= moves; i++) send('pointermove', i, 0.2 + (i % 5) / 10);
  send('pointerup', moves + 1, 0);
}

async function savedLog(script: () => void): Promise<Record<string, unknown>> {
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
      r.status === 0
        ? (JSON.parse(readFileSync(join(dir, 'out.json'), 'utf8')) as {
            actions: {
              actions: { id: string; state: string; web: { outcome: string; measured: number | null } }[];
              webOutside: number[];
              notRecorded: string[];
            };
            decision: { outcome: string };
          })
        : null;
    return { status: r.status, stdout: r.stdout, stderr: r.stderr, json };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

it('maps the six procedure actions to their contact episodes and ignores contacts outside them', async () => {
  const log = await savedLog(() => {
    key(' '); // A1 pressure ramp
    contact(30);
    key(' ');
    key(' '); // A2 speed
    contact(40);
    key(' ');
    key(' '); // A3 tilt
    contact(20);
    key(' ');
    contact(3); // a stray contact between actions: belongs to none
    key('k'); // A4 barrel rotation skipped: the pen has none
    key(' '); // A5 hover: performed, no contact
    key(' ');
    key(' '); // A6 stationary pressure
    contact(25, true);
    key(' ');
  });
  expect(log['source']).toBe('manual');
  const r = analyse(log);
  expect(r.status, r.stderr).toBe(0);
  const actions = r.json?.actions;
  expect(actions?.notRecorded).toEqual([]);
  expect(actions?.webOutside).toEqual([4]);
  const by = Object.fromEntries((actions?.actions ?? []).map((a) => [a.id, a]));
  expect(by['A1']?.web).toMatchObject({ outcome: 'episode 1', measured: 1 });
  expect(by['A2']?.web).toMatchObject({ outcome: 'episode 2', measured: 2 });
  expect(by['A3']?.web).toMatchObject({ outcome: 'episode 3', measured: 3 });
  expect(by['A4']).toMatchObject({ state: 'skipped', web: { outcome: 'omitted' } });
  expect(by['A5']).toMatchObject({ state: 'done', web: { outcome: 'performed, no events' } });
  expect(by['A6']?.web).toMatchObject({ outcome: 'episode 5', measured: 5 });
  expect(r.stdout).toContain('### Actions');
  // The rule set is applied too. The form says nothing about the pen's tilt support and the strokes carry none, so
  // tilt cannot be told apart from n/a: insufficient evidence, not a decision (ADR-0009 Step 0).
  expect(r.stdout).toContain('### Decision (ADR-0009 tablet rule set)');
  expect(r.json?.decision.outcome).toMatch(/^insufficient evidence/);
});

it('records an action still running at Save as unfinished, and actions never reached as not recorded', async () => {
  const log = await savedLog(() => {
    key(' ');
    contact(10);
  });
  const r = analyse(log);
  expect(r.status, r.stderr).toBe(0);
  expect(r.json?.actions.actions).toMatchObject([{ id: 'A1', state: 'unfinished' }]);
  expect(r.json?.actions.notRecorded).toEqual(['A2', 'A3', 'A4', 'A5', 'A6']);
});

it('rejects malformed action records (out of order, reversed times)', async () => {
  const log = await savedLog(() => {
    key(' ');
    contact(5);
    key(' ');
  });
  const actions = log['actions'] as { id: string; startPerf: number; endPerf: number }[];
  const first = actions[0];
  if (!first) throw new Error('no action recorded');
  const outOfOrder = { ...log, actions: [{ ...first, id: 'A2' }] };
  expect(analyse(outOfOrder).status).toBe(2);
  const reversed = { ...log, actions: [{ ...first, endPerf: first.startPerf - 1 }] };
  expect(analyse(reversed).status).toBe(2);
});
